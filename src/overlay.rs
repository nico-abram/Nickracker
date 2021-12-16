pub use windows::Win32::Foundation::*;
pub use windows::Win32::Graphics::Gdi::*;
pub use windows::Win32::System::LibraryLoader::GetModuleHandleW;
pub use windows::Win32::UI::Input::KeyboardAndMouse::SetCapture;
pub use windows::Win32::UI::WindowsAndMessaging::*;

use super::*;
use std::sync::atomic::AtomicPtr;

static BITMAP_HDCS: AtomicUsize = AtomicUsize::new(0);

pub fn get_last_error() -> WIN32_ERROR {
    unsafe { GetLastError() }
}

pub fn to_wstring(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
/// Turn a string into a windows wide string, leak it and make a PSWTR pointing to it
pub fn leak_pwstr(s: &str) -> PWSTR {
    PWSTR((*Box::leak(Box::new(to_wstring(s)))).as_ptr() as *mut _)
}

#[derive(Debug)]
enum OverlayMsg {
    UpdateSymbols([Option<(u8, usize, usize)>; ANSWER_SIZE]),
}

/// Used to send custom messages
pub struct OverlayWindowHandle(HWND, std::sync::mpsc::Sender<OverlayMsg>);

const WM_CUSTOM: u32 = WM_APP + 1;

struct UserData {
    msg_recvr: std::sync::mpsc::Receiver<OverlayMsg>,
    symbols: [Option<(u8, usize, usize)>; ANSWER_SIZE],
}
static USER_DATA: AtomicPtr<UserData> = AtomicPtr::new(std::ptr::null_mut());

pub fn create_window_in_another_thread() -> OverlayWindowHandle {
    let (hwnd_s, hwnd_r) = std::sync::mpsc::channel();
    let (msg_s, msg_recvr) = std::sync::mpsc::channel();
    USER_DATA.store(
        Box::into_raw(Box::new(UserData {
            msg_recvr,
            symbols: Default::default(),
        })),
        Ordering::SeqCst,
    );

    std::thread::spawn(move || {
        let hInstance = unsafe { GetModuleHandleW(PWSTR(std::ptr::null_mut())) } as HINSTANCE;
        if hInstance == 0 {
            panic!("GetModuleHandleW failed");
        }
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            hInstance,
            lpfnWndProc: Some(wndproc),
            lpszClassName: leak_pwstr("Nickracker Overlay"),

            ..Default::default()
        };
        if unsafe { RegisterClassExW(&wc) } == 0 {
            panic!("RegisterClassEx failed");
        }
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_COMPOSITED | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOPMOST,
                wc.lpszClassName,
                leak_pwstr("Nickracker Overlay"),
                WS_MAXIMIZE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                hInstance,
                std::ptr::null_mut(),
            )
        };
        if hwnd == 0 {
            panic!("CreateWindowEx failed");
        }
        unsafe {
            SetLayeredWindowAttributes(hwnd, 0x0000_0000, 255, LWA_COLORKEY);
            ShowCursor(false);
            SetCapture(hwnd);
        }
        hwnd_s.send(hwnd).unwrap();

        let mut message = MSG::default();
        unsafe {
            while GetMessageW(&mut message, 0, 0, 0) != BOOL(0) {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    });
    let hwnd = hwnd_r.recv().unwrap();

    OverlayWindowHandle(hwnd, msg_s)
}

impl OverlayWindowHandle {
    pub fn send_order(&self) {
        unsafe {
            self.1
                .send(OverlayMsg::UpdateSymbols([
                    Some((0, 50, 200)),
                    Some((1, 200, 200)),
                    None,
                    None,
                ]))
                .unwrap();
            PostMessageW(self.0, WM_CUSTOM, 123, 321);
        }
    }
}

fn rusty_wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let user_data: &mut UserData = unsafe { &mut *USER_DATA.load(Ordering::SeqCst) };
    match message as u32 {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(window, &mut ps) };

            let bitmap_hdcs = BITMAP_HDCS.load(SeqCst);
            if bitmap_hdcs != 0 {
                let bitmap_hdcs = bitmap_hdcs as *const [Option<CreatedHDC>; 12];
                let bitmap_hdcs: &[Option<CreatedHDC>; 12] = unsafe { &*bitmap_hdcs };
                for (i, bitmap_hdc) in bitmap_hdcs.iter().enumerate().take(12) {
                    if let Some(bitmap_hdc) = bitmap_hdc {
                        let ret = unsafe {
                            BitBlt(
                                hdc,
                                (100 + 54 * (i % 6)) as i32,
                                (100 + 54 * (i / 6)) as i32,
                                52,
                                52,
                                bitmap_hdc,
                                0,
                                0,
                                SRCCOPY,
                            )
                        }; //, WHITENESS);
                        if ret == BOOL(0) {
                            println!("Error {:?} {:#X}", get_last_error(), get_last_error());
                        }
                    }
                }
            }

            //unsafe { FillRect(hdc, &RECT{left:100, top:100, right: 250, bottom: 400},
            // HBRUSH((COLOR_WINDOW.0+1) as isize)); }
            unsafe {
                EndPaint(window, &ps);
            }
            0
        }
        WM_CUSTOM => {
            let msg = user_data.msg_recvr.recv().unwrap();
            println!("WM_CUSTOM {} {} msg: {:?}", wparam, lparam, &msg);
            match msg {
                OverlayMsg::UpdateSymbols(symbols) => unsafe {
                    user_data.symbols = symbols;
                    InvalidateRect(window, std::ptr::null_mut(), true);
                },
            }
            0
        }
        WM_NCCALCSIZE => 0,
        WM_DESTROY => {
            println!("WM_DESTROY");
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        WM_NCDESTROY => {
            println!("WM_NCDESTROY");
            drop(unsafe { Box::from_raw(user_data as *mut UserData) });
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let result = std::panic::catch_unwind(|| rusty_wndproc(window, message, wparam, lparam));
    match result {
        Ok(x) => x,
        Err(_err) => std::process::abort(),
    }
}

/*
let mut bitmap_hdcs: [Option<CreatedHDC>; 12] = [None; 12];
for (i, bitmap_hdc) in bitmap_hdcs.iter_mut().enumerate().take(12) {
    let bmp_bitmap = LoadImageW(
        0,
        leak_pwstr(&format!("src/bmps/symbol{}.bmp", i)),
        IMAGE_BITMAP,
        0,
        0,
        LR_LOADFROMFILE,
    );
    *bitmap_hdc = if bmp_bitmap == HANDLE(0) {
        println!("Could not find bmp {}", i);
        None
    } else {
        let hdc = CreateCompatibleDC(0);
        SelectObject(hdc, bmp_bitmap.0 as *const u8 as usize as isize);
        Some(hdc)
    };
}
let bitmap_hdcs_ptr = Box::leak(Box::new(bitmap_hdcs)) as *mut _ as usize;
BITMAP_HDCS.store(bitmap_hdcs_ptr, SeqCst);
InvalidateRect(hwnd, std::ptr::null_mut(), true);
*/
