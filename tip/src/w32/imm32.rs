// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

//! Undocumented IMM32 API.

pub(crate) const IME_PROP_COMPLETE_ON_UNSELECT: u32 = 0x00100000;

use std::mem;

use error_plus::{expect_error, expect_error_fn, impl_context_error};
use log::debug;
use windows::Win32::{
    Foundation::HINSTANCE,
    System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    UI::Input::KeyboardAndMouse::{GetKeyboardLayout, HKL},
};
use windows_core::{PCSTR, PCWSTR, s, w};

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
    expect_error("Failed to patch PIMEDPI", || {
        let void_fn = load_dynamic_fn(w!("imm32.dll"), s!("ImmLockImeDpi"))?;

        unsafe {
            let lock_fn: ImmLockImeDpiFn = mem::transmute(void_fn);

            let hkl = GetKeyboardLayout(0);
            let pimedpi = lock_fn(hkl);
            if let Some(imedpi) = pimedpi.as_mut() {
                imedpi.ime_info.fdw_property |= IME_PROP_COMPLETE_ON_UNSELECT;
                debug!("done adding IME_PROP_COMPLETE_ON_UNSELECT to IME property");
            } else {
                debug!("unable to get the PIMEDPI pointer");
            }
            Ok(pimedpi)
        }
    })
}

pub(crate) fn release_ime_info(pimedpi: *mut ImeDpi) -> Result<(), Imm32Error> {
    expect_error("Failed to release imm32.dll", || {
        let void_fn = load_dynamic_fn(w!("imm32.dll"), s!("ImmUnlockImeDpi"))?;
        unsafe {
            let unlock_fn: ImmUnlockImeDpiFn = mem::transmute(void_fn);
            unlock_fn(pimedpi);
            Ok(())
        }
    })
}

fn load_dynamic_fn(
    lib: PCWSTR,
    name: PCSTR,
) -> Result<unsafe extern "system" fn() -> isize, Imm32Error> {
    unsafe {
        let err = || Imm32Error {
            message: format!(
                "Failed to load method {} from {}",
                lib.display(),
                name.display()
            )
            .into(),
            source: None,
            location: None,
        };
        expect_error_fn(err, || {
            let lib = LoadLibraryW(lib)?;
            if lib.is_invalid() {
                Err("Loaded DLL handle is invalid")?;
            }
            let Some(void_fn) = GetProcAddress(lib, name) else {
                return Err("Method not found".into());
            };
            Ok(void_fn)
        })
    }
}

impl_context_error!(Imm32Error);
