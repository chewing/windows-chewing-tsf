//! Undocumented IMM32 API.

pub(crate) const IME_PROP_COMPLETE_ON_UNSELECT: u32 = 0x00100000;

use windows::Win32::{Foundation::HINSTANCE, UI::Input::KeyboardAndMouse::HKL};

// Retrieve the pointer to an IME instance
//
// See https://github.com/reactos/reactos/blob/80bd4608363683131411f131e8783749e51835c5/win32ss/user/imm32/ime.c#L531
windows_core::link!("imm32.dll" "system" fn ImmLockImeDpi(hkl: HKL) -> *mut ImeDpi);

// Return the pointer of an IME instance
//
// See https://github.com/reactos/reactos/blob/80bd4608363683131411f131e8783749e51835c5/win32ss/user/imm32/ime.c#L561
windows_core::link!("imm32.dll" "system" fn ImmUnlockImeDpi(pimedpi: *mut ImeDpi));

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
