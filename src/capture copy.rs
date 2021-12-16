/// Take a screenshot of a window using the Windows.Graphics.Capture API
/// Based on https://github.com/robmikh/screenshot-rs and https://github.com/mmozeiko/wcap
/// Requires Windows 10 version 1903, May 2019 Update (19H1)
use windows::core::*;
use windows::Foundation::*;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::{
    Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
    Direct3D11::{
        D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION,
    },
    Dxgi::{IDXGIDevice, DXGI_ERROR_UNSUPPORTED},
};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::{
    Graphics::Capture::IGraphicsCaptureItemInterop, RoInitialize, RO_INIT_MULTITHREADED,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowTextW, IsWindow,
};

use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};

fn create_capture_item_for_window(
    interop: &IGraphicsCaptureItemInterop,
    window_handle: HWND,
) -> Result<GraphicsCaptureItem> {
    println!(
        "creating capture item for {} IsWindow:{}",
        window_handle,
        unsafe { IsWindow(window_handle).0 }
    );
    unsafe { interop.CreateForWindow(window_handle) }
}

fn create_d3d_device_with_type(
    driver_type: D3D_DRIVER_TYPE,
    flags: D3D11_CREATE_DEVICE_FLAG,
    device: *mut Option<ID3D11Device>,
) -> Result<()> {
    unsafe {
        D3D11CreateDevice(
            None,
            driver_type,
            None,
            flags,
            std::ptr::null(),
            0,
            D3D11_SDK_VERSION as u32,
            device,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    }
}

pub fn create_d3d_device() -> Result<ID3D11Device> {
    let mut device = None;
    let mut result = create_d3d_device_with_type(
        D3D_DRIVER_TYPE_HARDWARE,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        &mut device,
    );
    if let Err(error) = &result {
        if error.code() == DXGI_ERROR_UNSUPPORTED {
            result = create_d3d_device_with_type(
                D3D_DRIVER_TYPE_WARP,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                &mut device,
            );
        }
    }
    result?;
    Ok(device.unwrap())
}

pub fn create_direct3d_device(d3d_device: &ID3D11Device) -> Result<IDirect3DDevice> {
    let dxgi_device: IDXGIDevice = d3d_device.cast()?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(Some(dxgi_device))? };
    inspectable.cast()
}

pub fn get_d3d_interface_from_object<S: Interface, R: Interface + Abi>(object: &S) -> Result<R> {
    let access: IDirect3DDxgiInterfaceAccess = object.cast()?;
    let object = unsafe { access.GetInterface::<R>()? };
    Ok(object)
}

static LAST_WINDOW: AtomicIsize = AtomicIsize::new(-1);
/// Attempts to find the PG window, blocking until it finds it.
/// Also checks if previous_window is still valid, if given. If it is not, it
/// blocks until it finds a new one.
pub fn find_window(previous_window: Option<HWND>) -> HWND {
    static WINDOW: AtomicIsize = AtomicIsize::new(-1);

    if let Some(window) = previous_window {
        // If the previous window is still valid, use that
        if unsafe { IsWindow(window) } != BOOL(0) {
            LAST_WINDOW.store(window, Ordering::SeqCst);
            return window;
        }
    }
    println!("NOT reusing window");
    let mut window = -1;
    WINDOW.store(-1, Ordering::SeqCst);
    while window == -1 {
        unsafe {
            EnumWindows(Some(enum_windows), 0);
        }
        window = WINDOW.load(Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    extern "system" fn enum_windows(window: HWND, _: LPARAM) -> BOOL {
        let mut text = [0u16; 2048];
        let len =
            unsafe { GetWindowTextW(window, PWSTR(text.as_mut_ptr()), text.len() as i32 - 5) };
        let text = String::from_utf16_lossy(&text[..len as usize]);

        let found = text == "Project Gorgon";
        if found {
            let mut class_text = [0u16; 2048];

            let len = unsafe {
                GetClassNameW(
                    window,
                    PWSTR(class_text.as_mut_ptr()),
                    class_text.len() as i32 - 5,
                )
            };
            let class_text = String::from_utf16_lossy(&class_text[..len as usize]);
            if class_text == "UnityWndClass" {
                println!("found window {:?}", window);
                WINDOW.store(window, Ordering::SeqCst);
            }
        }

        BOOL(!found as _)
    }
    LAST_WINDOW.store(window, Ordering::SeqCst);
    window
}

static LAST_SEEN_ID: AtomicUsize = AtomicUsize::new(0);
pub struct CaptureState {
    recv: single_value_channel::Receiver<(Option<Vec<u8>>, usize)>,
    latest_seen: usize,
}
/// Performs one time initialization (COM initializaiton) and starts the capture thread
pub fn initialize() -> Result<impl IntoIterator<Item = (Vec<u8>, usize, usize)>> {
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }

    let (recv, updater) = single_value_channel::channel_starting_with((None, 0usize));

    std::thread::spawn(|| {
        capturer_thread(updater).unwrap();
    });
    Ok(CaptureState {
        recv,
        latest_seen: 0,
    })
}

fn capturer_thread(updater: single_value_channel::Updater<(Option<Vec<u8>>, usize)>) -> Result<()> {
    let mut window = find_window(None);

    let capture_interop =
        windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;

    let item = create_capture_item_for_window(&capture_interop, window)?;
    let mut item_size = item.Size()?;

    let d3d_device = create_d3d_device()?;
    let d3d_context = unsafe {
        let mut d3d_context = None;
        d3d_device.GetImmediateContext(&mut d3d_context);
        d3d_context.unwrap()
    };
    let device = create_direct3d_device(&d3d_device)?;
    let mut frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        &item_size,
    )?;
    let mut session = frame_pool.CreateCaptureSession(item)?;

    let handler_maker = |mut last_seen_id, this_event_listeners_window| {
        TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new({
            let d3d_device = d3d_device.clone();
            let d3d_context = d3d_context.clone();
            let updater = updater.clone();
            move |frame_pool, _| unsafe {
                if this_event_listeners_window != LAST_WINDOW.load(Ordering::SeqCst) {
                    // Ignore events from closed windows, sometimes they seem to crash when calling
                    // get_d3d_interface_from_object
                    println!("closed window event");
                    return Ok(());
                }

                let frame_pool = frame_pool.as_ref().unwrap();
                let frame = frame_pool.TryGetNextFrame()?;
                let source_texture: ID3D11Texture2D =
                    get_d3d_interface_from_object(&frame.Surface()?)?;
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                source_texture.GetDesc(&mut desc);
                desc.BindFlags = 0;
                desc.MiscFlags = 0;
                desc.Usage = D3D11_USAGE_STAGING;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
                let copy_texture = { d3d_device.CreateTexture2D(&desc, std::ptr::null())? };

                d3d_context.CopyResource(Some(copy_texture.cast()?), Some(source_texture.cast()?));
                if desc.Width == 0 || desc.Height == 0 {
                    return Ok(());
                }
                let bits = {
                    let resource: ID3D11Resource = copy_texture.cast()?;
                    let mapped = d3d_context.Map(Some(resource.clone()), 0, D3D11_MAP_READ, 0)?;

                    // Get a slice of bytes
                    let slice: &[u8] = {
                        std::slice::from_raw_parts(
                            mapped.pData as *const _,
                            (desc.Height * mapped.RowPitch) as usize,
                        )
                    };

                    let bytes_per_pixel = 4;
                    let mut bits = vec![0u8; (desc.Width * desc.Height * bytes_per_pixel) as usize];
                    for row in 0..desc.Height {
                        let data_begin = (row * (desc.Width * bytes_per_pixel)) as usize;
                        let data_end = ((row + 1) * (desc.Width * bytes_per_pixel)) as usize;
                        let slice_begin = (row * mapped.RowPitch) as usize;
                        let slice_end = slice_begin + (desc.Width * bytes_per_pixel) as usize;
                        bits[data_begin..data_end].copy_from_slice(&slice[slice_begin..slice_end]);
                    }

                    d3d_context.Unmap(Some(resource), 0);

                    bits
                };

                last_seen_id += 1;
                updater.update((Some(bits), last_seen_id)).unwrap();
                Ok(())
            }
        })
    };
    frame_pool.FrameArrived(handler_maker(LAST_SEEN_ID.load(Ordering::SeqCst), window))?;
    loop {
        let new_window = find_window(Some(window));
        if new_window != window {
            window = new_window;

            session.Close()?;
            frame_pool.Close()?;
            let item = create_capture_item_for_window(
                &windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?,
                window,
            )?;
            item_size = item.Size()?;
            frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                2,
                &item_size,
            )?;
            session = frame_pool.CreateCaptureSession(item)?;
            frame_pool.FrameArrived(handler_maker(LAST_SEEN_ID.load(Ordering::SeqCst), window))?;
        }
        let item = create_capture_item_for_window(
            &windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?,
            window,
        )?;
        let new_size = item.Size()?;
        if item_size != new_size {
            item_size = new_size;

            session.Close()?;
            frame_pool.Close()?;
            frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                2,
                &item_size,
            )?;
            session = frame_pool.CreateCaptureSession(item)?;
            frame_pool.FrameArrived(handler_maker(LAST_SEEN_ID.load(Ordering::SeqCst), window))?;
        }
        session.StartCapture()?;
    }
}

/// Retrieves the latest screenshot taken of the game.
/// If there has been none since the last call to this function, returns None.
/// Finds game window by window title and window class name.
/// Will fail if not on Windows 10 1903 (19H1) or newer, or if window not found.
/// Will return a black image if the window is minimized or not visible.
/// Format is BGRA8.
pub fn get_last_screenshot(state: &mut CaptureState) -> Option<Vec<u8>> {
    let latest = state.recv.latest();
    if state.latest_seen < latest.1 {
        state.latest_seen = latest.1;
        LAST_SEEN_ID.store(state.latest_seen, Ordering::SeqCst);
        latest.0.clone()
    } else {
        None
    }
}
