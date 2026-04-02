//! Undocumented IMM32 API.

pub(crate) const IME_PROP_COMPLETE_ON_UNSELECT: u32 = 0x00100000;

use std::{error::Error, fmt::Display, mem};

use exn::{Result, ResultExt, bail};
use log::debug;
use windows::Win32::{
    Foundation::HINSTANCE,
    System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    UI::Input::KeyboardAndMouse::{GetKeyboardLayout, HKL},
};
use windows_core::{s, w};

// Retrieve the pointer to an IME instance
//
// See https://github.com/reactos/reactos/blob/80bd4608363683131411f131e8783749e51835c5/win32ss/user/imm32/ime.c#L531
type ImmLockImeDpiFn = unsafe extern "system" fn(hkl: HKL) -> *mut ImeDpi;

// Return the pointer of an IME instance
//
// See https://github.com/reactos/reactos/blob/80bd4608363683131411f131e8783749e51835c5/win32ss/user/imm32/ime.c#L561
type ImmUnlockImeDpiFn = unsafe extern "system" fn(pimedpi: *mut ImeDpi);

// IME info
//
// See https://github.com/reactos/reactos/blob/80bd4608363683131411f131e8783749e51835c5/sdk/include/ddk/immdev.h#L20
#[repr(C)]
pub(crate) struct ImeInfo {
    pub(crate) dw_private_data_size: u32,
    pub(crate) fdw_property: u32,
    pub(crate) fdw_conversion_caps: u32,
    pub(crate) fdw_sentence_caps: u32,
    pub(crate) fdw_uicaps: u32,
    pub(crate) fdw_scscaps: u32,
    pub(crate) fdw_select_caps: u32,
}

// IME instance struct
//
// Unused remaining fields are not included.
//
// See https://github.com/reactos/reactos/blob/80bd4608363683131411f131e8783749e51835c5/sdk/include/reactos/imm32_undoc.h#L91
#[repr(C)]
pub(crate) struct ImeDpi {
    pnext: *const ImeDpi,
    hinst: HINSTANCE,
    hkl: HKL,
    pub(crate) ime_info: ImeInfo,
}

pub(crate) fn patch_ime_info() -> Result<*mut ImeDpi, Imm32Error> {
    let err = || Imm32Error("failed to load imm32.dll");
    // Load imm32.dll
    let lib = unsafe { LoadLibraryW(w!("imm32.dll")).or_raise(err)? };
    if lib.is_invalid() {
        bail!(err());
    }

    let maybe_imm_lock_fn = unsafe { GetProcAddress(lib, s!("ImmLockImeDpi")) };

    let Some(imm_lock_fn) = maybe_imm_lock_fn else {
        bail!(Imm32Error("ImmLockImeDpi not found"));
    };
    let lock_fn: ImmLockImeDpiFn = unsafe { mem::transmute(imm_lock_fn) };

    let hkl = unsafe { GetKeyboardLayout(0) };
    let pimedpi = unsafe { lock_fn(hkl) };
    if let Some(imedpi) = unsafe { pimedpi.as_mut() } {
        imedpi.ime_info.fdw_property |= IME_PROP_COMPLETE_ON_UNSELECT;
        debug!("done adding IME_PROP_COMPLETE_ON_UNSELECT to IME property");
    } else {
        debug!("unable to get the PIMEDPI pointer");
    }

    Ok(pimedpi)
}

pub(crate) fn release_ime_info(pimedpi: *mut ImeDpi) -> Result<(), Imm32Error> {
    let err = || Imm32Error("failed to load imm32.dll");
    // Load imm32.dll
    let lib = unsafe { LoadLibraryW(w!("imm32.dll")).or_raise(err)? };
    if lib.is_invalid() {
        bail!(err());
    }
    let maybe_imm_unlock_fn = unsafe { GetProcAddress(lib, s!("ImmUnlockImeDpi")) };
    let Some(imm_unlock_fn) = maybe_imm_unlock_fn else {
        bail!(Imm32Error("ImmUnlockImeDpi not found"));
    };
    let unlock_fn: ImmUnlockImeDpiFn = unsafe { mem::transmute(imm_unlock_fn) };
    unsafe { unlock_fn(pimedpi) };

    Ok(())
}

#[derive(Debug)]
pub(crate) struct Imm32Error(&'static str);

impl Error for Imm32Error {}

impl Display for Imm32Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to patch ime_info.fdw_property: {}", self.0)
    }
}
