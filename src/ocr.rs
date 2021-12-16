use std::ffi::CStr;
use std::num::NonZeroUsize;

use tesseract_plumbing::TessBaseApi;

/// It seems this isn't actually needed
const ENABLE_RESIZE: bool = false;
/// Output the images we use for OCR with x_y_ocrtext.bmp format
pub const ENABLE_DEBUG_IMAGE_OUTPUT: bool = false;
pub const DEBUG_CONSOLE_OUTPUT: bool = false;

pub struct OcrState {
    tess: TessBaseApi,
    resize_buffer: Vec<u8>,
}
impl OcrState {
    pub fn new() -> Option<Self> {
        let tess = tesseract_plumbing::TessBaseApi::create();
        let mut zelf = Self {
            tess,
            resize_buffer: vec![],
        };

        const TRAINEDDATA: &[u8] = include_bytes!("../tessdata/best/eng.traineddata");

        // See tesseract_init_stub.cpp and build.rs
        extern "C" {
            pub fn TessBaseAPI_CustomInitStub(
                handle: *mut (),
                data: *const u8,
                data_size: i32,
                language: *const u8,
                mode: u32,
                configs: *mut *mut u8,
                configs_size: i32,
                set_only_non_debug_params: i32,
            ) -> i32;
        }
        let ret = unsafe {
            TessBaseAPI_CustomInitStub(
                zelf.get_tess_ptr() as *mut _,
                TRAINEDDATA.as_ptr() as *mut u8,
                TRAINEDDATA.len() as i32,
                std::ptr::null_mut(),
                tesseract_sys::TessOcrEngineMode_OEM_DEFAULT as u32,
                std::ptr::null_mut(),
                0,
                0,
            )
        };
        if ret != 0 {
            panic!("Error initializing tesseract");
        }
        //tess.init_2(None, Some(CString::new("eng").ok()?).as_deref()).ok()?;

        zelf.tess
            .set_variable(cstr::cstr!("user_defined_dpi"), cstr::cstr!("70"))
            .ok()?;

        Some(zelf)
    }

    fn set_var(&mut self, field: &CStr, val: &CStr) -> Option<()> {
        self.tess.set_variable(field, val).ok()?;
        Some(())
    }

    fn set_page_mode(&mut self, mode: i32) {
        unsafe {
            tesseract_sys::TessBaseAPISetPageSegMode(
                self.get_tess_ptr(),
                mode, //tesseract_sys::TessPageSegMode_PSM_SINGLE_WORD,
            );
        }
    }

    /// Resizes an image using the internal buffer
    /// Returns (new_width, new_height, total_size_in_bytes_inside_resize_buffer)
    ///
    /// Used for preprocessing images before giving them to tesseract
    /// See https://tesseract-ocr.github.io/tessdoc/ImproveQuality.html#rescaling
    fn resize(
        &mut self,
        data: &[u8],
        w: usize,
        h: usize,
        stride: Option<NonZeroUsize>,
        scale: usize,
    ) -> (usize, usize, usize) {
        let (new_w, new_h) = (w * scale, h * scale);
        let needed = new_w * new_h * 3;
        self.resize_buffer.resize(needed, 0u8);

        use rgb::AsPixels;
        resize::new(w, h, new_w, new_h, resize::Pixel::RGB8, resize::Type::Point)
            .unwrap()
            .resize_stride(
                data[..].as_pixels(),
                stride.map(|x| x.get()).unwrap_or(w),
                self.resize_buffer[..needed].as_pixels_mut(),
            )
            .unwrap();

        (new_w, new_h, needed)
    }

    fn get_tess_ptr(&self) -> *mut tesseract_sys::TessBaseAPI {
        unsafe { std::ptr::read(&self.tess as *const _ as *const usize) as *mut _ }
    }

    pub fn print_tess_vars(&self) {
        // This is incredibly cursed, just pretend it's not here
        #[repr(transparent)]
        #[allow(clippy::upper_case_acronyms)]
        struct FILE(std::ffi::c_void);
        extern "cdecl" {
            fn __acrt_iob_func(ix: u32) -> *mut FILE;
        }
        fn stdout() -> *mut FILE {
            unsafe { __acrt_iob_func(1) }
        }
        let stdout = stdout();
        unsafe {
            tesseract_sys::TessBaseAPIPrintVariables(
                self.get_tess_ptr(),
                std::mem::transmute(stdout),
            );
        }
    }

    pub fn ocr_num(
        &mut self,
        data: &[u8],
        w: usize,
        h: usize,
        bytes_per_line: Option<NonZeroUsize>,
        pos: (usize, usize),
    ) -> Option<String> {
        self.set_var(cstr::cstr!("classify_bln_numeric_mode"), cstr::cstr!("1"));
        self.set_var(
            cstr::cstr!("tessedit_char_blacklist"),
            cstr::cstr!(
                "!?@#$%&*()<>_-+=/:;'\"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
            ),
        );
        self.set_var(
            cstr::cstr!("tessedit_char_whitelist"),
            cstr::cstr!("01234,"),
        );

        self.set_page_mode(tesseract_sys::TessPageSegMode_PSM_SINGLE_LINE);

        let (data, w, h) = if ENABLE_RESIZE {
            const SCALE: usize = 10;
            let (w, h, len) = self.resize(
                data,
                w,
                h,
                bytes_per_line.map(|x| NonZeroUsize::new(x.get() / 3).unwrap()),
                SCALE,
            );
            let data = &self.resize_buffer[..len];
            (data, w, h)
        } else {
            (data, w, h)
        };
        if ENABLE_DEBUG_IMAGE_OUTPUT {
            super::bmp::save_rgb_bmp(data, w, h, &format!("dbg/num_{}_{}.bmp", pos.0, pos.1));
        }

        let tess = &mut self.tess;

        let bytes_per_line = if ENABLE_RESIZE {
            (w * 3) as i32
        } else {
            bytes_per_line
                .unwrap_or_else(|| NonZeroUsize::new(w * 3).unwrap())
                .get() as i32
        };
        tess.set_image(data, w as i32, h as i32, 3, bytes_per_line)
            .ok()?;
        tess.recognize().ok()?;
        let nums = tess
            .get_utf8_text()
            .ok()?
            .as_ref()
            .to_string_lossy()
            .into_owned();
        Some(nums)
    }
    pub fn ocr_generic(
        &mut self,
        data: &[u8],
        w: usize,
        h: usize,
        bytes_per_line: Option<NonZeroUsize>,
        pos: (usize, usize),
    ) -> Option<String> {
        self.set_var(cstr::cstr!("classify_bln_numeric_mode"), cstr::cstr!("0"));
        self.set_var(cstr::cstr!("tessedit_char_blacklist"), cstr::cstr!(""));
        self.set_var(cstr::cstr!("tessedit_char_whitelist"), cstr::cstr!(""));

        self.set_page_mode(tesseract_sys::TessPageSegMode_PSM_AUTO);

        let (data, w, h) = if ENABLE_RESIZE {
            const SCALE: usize = 5;
            let (w, h, len) = self.resize(
                data,
                w,
                h,
                bytes_per_line.map(|x| NonZeroUsize::new(x.get() / 3).unwrap()),
                SCALE,
            );
            let data = &self.resize_buffer[..len];
            (data, w, h)
        } else {
            (data, w, h)
        };

        self.tess
            .set_image(
                data,
                w as i32,
                h as i32,
                3,
                bytes_per_line
                    .unwrap_or_else(|| NonZeroUsize::new(w * 3).unwrap())
                    .get() as i32,
            )
            .ok()?;
        self.tess.recognize().ok()?;
        let title = self
            .tess
            .get_utf8_text()
            .ok()?
            .as_ref()
            .to_string_lossy()
            .into_owned();
        if ENABLE_DEBUG_IMAGE_OUTPUT {
            super::bmp::save_rgb_bmp(
                data,
                w,
                h,
                &format!(
                    "dbg/generic_{}_{}_{}.bmp",
                    pos.0,
                    pos.1,
                    title
                        .replace(
                            &['|', '\n', '?', ')', '(', '<', '>', '\\', '/', '\n', '\"', '\''],
                            ""
                        )
                        .trim()
                ),
            );
        }

        Some(title)
    }
}
