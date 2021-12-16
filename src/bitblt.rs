/// Attempts to take a screenshot of the game window. Finds game window by window title and
/// window class name. I have not been able to get it to work. Ever. May put this as a fallback
/// for windows 7? Format is BGRA8.
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use std::sync::atomic::{AtomicUsize, Ordering};

use super::get_last_error;

static PTR: AtomicUsize = AtomicUsize::new(0);
pub fn take_screenshot() -> Option<Vec<u8>> {
    unsafe {
        EnumWindows(Some(enum_window), 0);
    }
    let ptr = PTR.load(Ordering::SeqCst);
    if ptr == 0 {
        None
    } else {
        Some(unsafe { *Box::from_raw(ptr as *mut Vec<u8>) })
    }
}

extern "system" fn enum_window(window: HWND, _: LPARAM) -> BOOL {
    let mut text: [u16; 512] = [0; 512];
    let len = unsafe { GetWindowTextW(window, PWSTR(text.as_mut_ptr()), text.len() as i32) };
    let text = String::from_utf16_lossy(&text[..len as usize]);

    let found = text == "Project Gorgon";
    if found {
        let mut class_text: [u16; 512] = [0; 512];
        let len = unsafe {
            GetClassNameW(
                window,
                PWSTR(class_text.as_mut_ptr()),
                class_text.len() as i32,
            )
        };
        let class_text = String::from_utf16_lossy(&class_text[..len as usize]);
        if class_text == "UnityWndClass" {
            let mut rect: RECT = Default::default();
            if unsafe { GetWindowRect(window, &mut rect) } == BOOL(0) {
                panic!("GetWindowRect failed");
            }

            //let (xx, yy, hh, ww) = (rect.left, rect.top, rect.right - rect.left, rect.bottom -
            // rect.top);
            let (ww, hh) = (rect.right, rect.bottom);
            if ww * hh * 4 > 1024 * 1024 {
                return BOOL(1);
            }
            if unsafe { GetClientRect(window, &mut rect) } == BOOL(0) {
                panic!("GetWindowRect failed");
            }
            unsafe {
                MapWindowPoints(window, 0, &mut rect as *mut _ as *mut POINT, 2);
            }
            let (xx, _yy) = (rect.left, rect.top);
            //let (ww,hh) = (2048,2048);
            let hdc = unsafe { GetDC(window) };
            if xx < -10000 {
                unsafe {
                    ReleaseDC(window, hdc);
                }
                return BOOL(1);
            }
            let (our_hdc, bitmap) =
                unsafe { (CreateCompatibleDC(hdc), CreateCompatibleBitmap(hdc, ww, hh)) };
            let blt_res = unsafe {
                SetStretchBltMode(our_hdc, COLORONCOLOR);
                SelectObject(our_hdc, bitmap);
                BitBlt(our_hdc, 0, 0, ww, hh, hdc, 0, 0, SRCCOPY | CAPTUREBLT)
            };
            if blt_res == BOOL(0) {
                panic!(
                    "BitBlt failed {:?} {:#X}",
                    get_last_error(),
                    get_last_error()
                );
            }
            let mut binfo: BITMAPINFO = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: ww,
                    biHeight: hh,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB as u32,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut buffer = vec![0u8; (ww * hh * 4) as usize];
            if unsafe {
                GetDIBits(
                    our_hdc,
                    bitmap,
                    0,
                    -hh as u32,
                    buffer.as_mut_ptr() as *mut _,
                    &mut binfo,
                    DIB_RGB_COLORS,
                )
            } == 0
            {
                println!(
                    "GetDIBits failed {:?} {:#X}",
                    get_last_error(),
                    get_last_error()
                );
                unsafe {
                    DeleteDC(our_hdc);
                    ReleaseDC(window, hdc);
                    DeleteObject(bitmap);
                }
                return BOOL(1);
            }
            unsafe {
                DeleteDC(our_hdc);
                ReleaseDC(window, hdc);
                DeleteObject(bitmap);
            }

            //super::bmp::save_bmp(&buffer[..], ww, hh, "out_blt.bmp");
            PTR.store(
                Box::leak(Box::new(buffer)) as *mut _ as usize,
                Ordering::SeqCst,
            );
        }
    }

    BOOL(!found as _)
}
