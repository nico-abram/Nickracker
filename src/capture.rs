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

use std::sync::atomic::{AtomicIsize, Ordering};

fn create_capture_item_for_window(
    interop: &IGraphicsCaptureItemInterop,
    window_handle: HWND,
) -> Result<GraphicsCaptureItem> {
    /*
    println!(
        "creating capture item for {} IsWindow:{}",
        window_handle,
        unsafe { IsWindow(window_handle).0 }
    );
    */
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

/// Iterates over captured screenshots. Should be called in a dedicated thread.
/// Items are (rgb_bytes_vec, width, height)
pub fn for_each<F: FnMut(Vec<u8>, usize, usize) + 'static>(f: F) {
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED).unwrap();
    }

    let f = Box::leak(Box::new(std::cell::RefCell::new(f)));
    let call_f = |a, b, c| f.borrow_mut()(a, b, c);

    let mut window = find_window(None);

    let capture_interop =
        windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().unwrap();

    let item = create_capture_item_for_window(&capture_interop, window).unwrap();
    let mut item_size = item.Size().unwrap();

    let d3d_device = create_d3d_device().unwrap();
    let d3d_context = unsafe {
        let mut d3d_context = None;
        d3d_device.GetImmediateContext(&mut d3d_context);
        d3d_context.unwrap()
    };
    let device = create_direct3d_device(&d3d_device).unwrap();
    let mut frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        &item_size,
    )
    .unwrap();
    let mut session = frame_pool.CreateCaptureSession(item).unwrap();
    session.SetIsCursorCaptureEnabled(false).unwrap();

    let handler_maker = |this_event_listeners_window| {
        TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new({
            let d3d_device = d3d_device.clone();
            let d3d_context = d3d_context.clone();
            let mut copy_texture = None;

            move |frame_pool, _| unsafe {
                if this_event_listeners_window != LAST_WINDOW.load(Ordering::SeqCst) {
                    // Ignore events from closed windows, sometimes they seem to crash when
                    // calling get_d3d_interface_from_object
                    return Ok(());
                }
                #[cfg(feature = "trace")]
                let span = tracing::span!(tracing::Level::TRACE, "on_frame");
                #[cfg(feature = "trace")]
                let _enter = span.enter();

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
                let dims_changed = |copy_texture: &ID3D11Texture2D| {
                    let mut copy_desc = D3D11_TEXTURE2D_DESC::default();
                    copy_texture.GetDesc(&mut copy_desc);
                    copy_desc.Width != desc.Width || copy_desc.Height != desc.Height
                };
                if copy_texture.as_ref().map(dims_changed).unwrap_or(false) {
                    copy_texture = None;
                }
                if copy_texture.is_none() {
                    copy_texture = Some(d3d_device.CreateTexture2D(&desc, std::ptr::null())?);
                }
                let copy_texture = copy_texture.as_mut().unwrap();

                d3d_context.CopyResource(Some(copy_texture.cast()?), Some(source_texture.cast()?));
                if desc.Width == 0 || desc.Height == 0 {
                    return Ok(());
                }
                let bits = {
                    #[cfg(feature = "trace")]
                    let span = tracing::span!(tracing::Level::TRACE, "map copied texture");
                    #[cfg(feature = "trace")]
                    let _enter = span.enter();

                    let resource: ID3D11Resource = copy_texture.cast()?;
                    let mapped = d3d_context.Map(Some(resource.clone()), 0, D3D11_MAP_READ, 0)?;

                    // Get a slice of bytes
                    let slice: &[u8] = {
                        std::slice::from_raw_parts(
                            mapped.pData as *const _,
                            (desc.Height * mapped.RowPitch) as usize,
                        )
                    };
                    /*
                    let bytes_per_pixel_out = 3;
                    let bytes_per_pixel_in = 4;
                    let mut bits =
                        vec![0u8; (desc.Width * desc.Height * bytes_per_pixel_out) as usize];
                    let bit_slice =
                        &mut bits[0..(desc.Width * desc.Height * bytes_per_pixel_out) as usize];
                    for row in 0..desc.Height {
                        let data_begin = (row * (desc.Width * bytes_per_pixel_out)) as usize;
                        //let data_end = ((row + 1) * (desc.Width * bytes_per_pixel_out)) as usize;
                        let slice_begin = (row * mapped.RowPitch) as usize;
                        //let slice_end = slice_begin + (desc.Width * bytes_per_pixel_out) as
                        // usize;
                        for x in 0..desc.Width {
                            let data_begin = data_begin + (x * bytes_per_pixel_out) as usize;
                            let slice_begin = slice_begin + (x * bytes_per_pixel_in) as usize;
                            bit_slice[data_begin..data_begin + bytes_per_pixel_out as usize]
                                .copy_from_slice(
                                    &slice[slice_begin..slice_begin + bytes_per_pixel_out as usize],
                                );
                        }
                    }
                    */
                    #[cfg(feature = "trace")]
                    let span = tracing::span!(tracing::Level::TRACE, "rgba to rgb");
                    #[cfg(feature = "trace")]
                    let _enter = span.enter();

                    // Remove alpha channel
                    let mut bits =
                        Vec::with_capacity(desc.Width as usize * 3 * desc.Height as usize);
                    bits.extend(
                        slice
                            .chunks_exact(mapped.RowPitch as usize)
                            .flat_map(|row| {
                                row[..(desc.Width * 4) as usize]
                                    .chunks_exact(4)
                                    .flat_map(|bgra| [bgra[0], bgra[1], bgra[2]])
                            }),
                    );

                    d3d_context.Unmap(Some(resource), 0);

                    bits
                };

                call_f(bits, desc.Width as usize, desc.Height as usize);
                tracy_client::finish_continuous_frame!();
                //std::thread::sleep(std::time::Duration::from_millis(1000));
                Ok(())
            }
        })
    };
    frame_pool.FrameArrived(handler_maker(window)).unwrap();
    session.StartCapture().unwrap();
    loop {
        // Check if the window is still valid and has the same size
        let new_window = find_window(Some(window));
        let item = create_capture_item_for_window(
            &windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().unwrap(),
            new_window,
        )
        .unwrap();
        let new_size = item.Size().unwrap();
        if new_window != window {
            window = new_window;

            session.Close().unwrap();
            frame_pool.Close().unwrap();
            frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                1,
                &item_size,
            )
            .unwrap();
            session = frame_pool.CreateCaptureSession(item).unwrap();
            session.SetIsCursorCaptureEnabled(false).unwrap();
            frame_pool.FrameArrived(handler_maker(window)).unwrap();

            session.StartCapture().unwrap();
        } else if item_size != new_size {
            item_size = new_size;
            frame_pool
                .Recreate(
                    &device,
                    DirectXPixelFormat::B8G8R8A8UIntNormalized,
                    1,
                    &item_size,
                )
                .unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
