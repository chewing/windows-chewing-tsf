use std::cmp::Ordering;

use anyhow::{Result, bail};
use slint::SharedString;
use windows::Win32::{
    Foundation::LPARAM,
    Graphics::Gdi::{
        DEFAULT_CHARSET, EnumFontFamiliesExW, GetDC, LOGFONTW, ReleaseDC, TEXTMETRICW,
    },
};

// Callback function for EnumFontFamiliesEx
unsafe extern "system" fn enum_font_callback(
    logfont: *const LOGFONTW,
    _textmetric: *const TEXTMETRICW,
    _font_type: u32,
    lparam: LPARAM,
) -> i32 {
    let mut fonts: Box<Vec<String>> = unsafe { Box::from_raw(lparam.0 as *mut Vec<String>) };

    if let Some(lf) = unsafe { logfont.as_ref() } {
        // Extract font family name
        let family_name = String::from_utf16_lossy(
            &lf.lfFaceName
                .iter()
                .take_while(|&&c| c != 0)
                .copied()
                .collect::<Vec<u16>>(),
        );

        // Skip fonts starting with @ (vertical fonts)
        if !family_name.starts_with('@') {
            fonts.push(family_name);
        }
    }

    let _ = Box::into_raw(fonts);

    1 // Continue enumeration
}

// Enumerate fonts with GDI because DirectWrite skips some localized names.
pub fn enum_font_families() -> Result<Vec<SharedString>> {
    unsafe {
        // Get a device context for the screen
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            bail!("Unable to create DC from screen");
        }

        // Create LOGFONT structure for enumeration
        let logfont = LOGFONTW {
            lfCharSet: DEFAULT_CHARSET,
            ..Default::default()
        };

        // Vector to store font information
        let fonts: Box<Vec<String>> = Box::default();
        let fonts_ptr = Box::into_raw(fonts);

        // Enumerate font families
        let result = EnumFontFamiliesExW(
            hdc,
            &logfont,
            Some(enum_font_callback),
            LPARAM(fonts_ptr as isize),
            0,
        );

        // Release the device context
        ReleaseDC(None, hdc);

        // Take back fonts
        let fonts = Box::from_raw(fonts_ptr);

        if result == 0 {
            bail!("Unable to enumerate fonts");
        }

        let mut res: Vec<SharedString> = fonts.into_iter().map(|s| s.into()).collect();
        res.sort_by(|a, b| {
            let a_localized = a.is_ascii();
            let b_localized = b.is_ascii();
            match (a_localized, b_localized) {
                (true, true) => a.cmp(b),
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (false, false) => a.cmp(b),
            }
        });
        res.dedup();
        Ok(res)
    }
}
