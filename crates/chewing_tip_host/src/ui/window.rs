// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    cell::Cell,
    ffi::{c_int, c_void},
};

use log::error;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::{System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*};
use windows::core::*;

#[derive(Debug)]
pub(crate) struct Window {
    hwnd: Cell<HWND>,
}

impl Window {
    pub(crate) fn new() -> Window {
        Window {
            hwnd: Cell::new(HWND::default()),
        }
    }
}

pub(crate) extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_ptr = lparam.0 as *const CREATESTRUCTW;
            unsafe {
                if let Some(create_data) = create_ptr.as_ref() {
                    // Attach user_data to window
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, create_data.lpCreateParams as isize);
                }
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcA(hwnd, msg, wparam, lparam) },
    }
}

impl Window {
    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd.get()
    }

    pub(crate) fn create(
        &self,
        parent: HWND,
        class_name: PCWSTR,
        style: WINDOW_STYLE,
        ex_style: WINDOW_EX_STYLE,
        user_data: *const c_void,
    ) -> bool {
        let hwnd = unsafe {
            let hinst = match GetModuleHandleW(None) {
                Ok(hinst) => hinst,
                Err(error) => {
                    error!("Failed to create window: {error:?}");
                    return false;
                }
            };
            let hwnd = CreateWindowExW(
                ex_style,
                class_name,
                None,
                style,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                Some(hinst.into()),
                Some(user_data),
            );
            hwnd
        };
        match hwnd {
            Ok(hwnd) => {
                self.hwnd.set(hwnd);
                true
            }
            Err(error) => {
                error!("Failed to create window: {error:?}");
                false
            }
        }
    }

    pub(crate) fn set_position(&self, x: c_int, y: c_int) {
        unsafe {
            let _ = SetWindowPos(self.hwnd(), None, x, y, 0, 0, SWP_SHOWWINDOW);
        }
    }

    pub(crate) fn show(&self) {
        unsafe {
            if !self.hwnd().is_invalid() {
                let _ = ShowWindow(self.hwnd(), SW_SHOWNA);
            }
        }
    }

    pub(crate) fn hide(&self) {
        unsafe {
            if !self.hwnd().is_invalid() {
                let _ = ShowWindow(self.hwnd(), SW_HIDE);
            }
        }
    }

    pub(crate) fn refresh(&self) {
        unsafe {
            if !self.hwnd().is_invalid() {
                let _ = InvalidateRect(Some(self.hwnd()), None, true);
                let _ = UpdateWindow(self.hwnd());
            }
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if !self.hwnd.get().is_invalid() {
            unsafe {
                let _ = DestroyWindow(self.hwnd.get());
            }
        }
    }
}
