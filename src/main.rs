#![feature(array_chunks)]
#![feature(const_for)]
#![feature(const_try)]
#![feature(const_mut_refs)]
#![feature(const_option_ext)]
#![feature(const_fn_trait_bound)]
//#![feature(const_eval_limit)]
//#![const_eval_limit = "10000000000"]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(clippy::identity_op)]
///
/// The pieces of this program are structured as follows:
///
/// The solver library crate has the code for the puzzle solver. It exports essentially 2
/// interesting functions: apply_result and guess().The console binary crate has a simple
/// console program to run the solver.
///
/// capture.rs and bitblt.rs take care of taking screenshots of the game window. capture.rs
/// uses a newer API only available since a windows 10 update, and bitblt should have wider
/// support. bitblt.rs finds the window HWND handle and uses BitBlt and GetDIBits, and
/// capture.rs uses the newer Windows.Graphics.Capture API. The capture-test binary crate has a
/// simple test program for the Capture API. Please read the code before running it.
///
/// bmp.rs has a very basic bmp writer and reader.
///
/// ocr.rs takes care of converting an RGB image to generic text or text with only numbers. It
/// uses a library called tesseract.
///
/// overlay.rs takes care of making a frameless, borderless, always-on-top transparent window
/// in which we put things as a game overlay.
///
/// main.rs has the main window and the coordination between the different parts. This program
/// creates 3 threads:
///  - One for the overlay window. This should be blocked most of the time and consume little
///    resources.
///  - The main thread with the main window. This should be blocked most of the time and
///    consume little resources.
///  - One for capturing screenshots and running OCR on them. This might be a bit resource
///    intensive.
///
/// Communication is done through channels and some global state behind a mutex.
/// The capture thread periodically screenshots and analyzes the taken screenshots, and then
/// sends the results back to the main thread through a channel when it finds a minotaur vault.
/// The results have the wrong guesses/results and the state of the current guess being made.
///
/// The overlay thread receives messages through a channel, that tell it to position highlights
/// on specific positions.
///
/// The main thread receives analyzed state results and sends the overlay thread the new
/// required highlights, and also updates the main window with the current known state.
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{Ordering::*, *};

use solver::*;

mod bitblt;
mod bmp;
mod capture;
mod ocr;
mod overlay;
mod vault_analyzer;

use overlay::*;

pub const MAX_GUESSES: usize = 12;

const SYMBOL_BMPS: [&[u8]; SYMBOL_COUNT] = [
    include_bytes!("bmps/symbol0.bmp"),
    include_bytes!("bmps/symbol1.bmp"),
    include_bytes!("bmps/symbol2.bmp"),
    include_bytes!("bmps/symbol3.bmp"),
    include_bytes!("bmps/symbol4.bmp"),
    include_bytes!("bmps/symbol5.bmp"),
    include_bytes!("bmps/symbol6.bmp"),
    include_bytes!("bmps/symbol7.bmp"),
    include_bytes!("bmps/symbol8.bmp"),
    include_bytes!("bmps/symbol9.bmp"),
    include_bytes!("bmps/symbol10.bmp"),
    include_bytes!("bmps/symbol11.bmp"),
];
// Invert image, seems to help tesseract
// See here: https://tesseract-ocr.github.io/tessdoc/ImproveQuality.html#inverting-images
fn invert_bytes(bytes: &mut [u8]) {
    for b in bytes {
        *b = 255 - *b;
    }
}

enum OverlayChange {
    Todo,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    //std::fs::remove_file("\\\\?\\D:\\dev\\Nickracker\\347_913_7, i Doe 4 “- ").unwrap();

    let (analyzer_vault_sender, _analyzer_vualt_recvr) = std::sync::mpsc::channel();
    std::thread::spawn(|| {
        let mut vault_analyzer = vault_analyzer::VaultAnalyzerCtx::new().unwrap();
        capture::for_each(move |screenshot, width, height| {
            if let Some((analyzed, _, _)) =
                vault_analyzer.find_minotaur_vault(&screenshot, width, height)
            {
                println!("========= ANALYZED :: {:?}", &analyzed);
                analyzer_vault_sender.send(analyzed).unwrap();
            }
        });
    });
    let overlay = overlay::create_window_in_another_thread();

    overlay.send_order();

    let hInstance = unsafe { GetModuleHandleW(PWSTR(std::ptr::null_mut())) } as HINSTANCE;
    if hInstance == 0 {
        panic!("GetModuleHandleW failed");
    }
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        hInstance,
        lpfnWndProc: Some(wndproc),
        lpszClassName: leak_pwstr("Nickracker"),
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&wc) } == 0 {
        panic!("RegisterClassEx failed");
    }
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            wc.lpszClassName,
            leak_pwstr("Nickracker"),
            WS_VISIBLE | WS_OVERLAPPEDWINDOW,
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

    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, 0, 0, 0) != BOOL(0) {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    Ok(())
}

extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message as u32 {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _hdc = unsafe { BeginPaint(window, &mut ps) };

            unsafe {
                EndPaint(window, &ps);
            }
            0
        }
        WM_DESTROY => {
            println!("WM_DESTROY");
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
