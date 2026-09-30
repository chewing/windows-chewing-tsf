// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    cell::Cell,
    ffi::{c_int, c_void},
    sync::atomic::Ordering,
};

use log::error;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::{Foundation::*, UI::HiDpi::SetThreadDpiAwarenessContext};
use windows::Win32::{Graphics::Gdi::*, UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::core::*;

use crate::com::G_HINSTANCE;

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
                    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, create_data.lpCreateParams as isize);
                    #[cfg(target_arch = "x86")]
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, create_data.lpCreateParams as i32);
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
        let hinst = HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut c_void);
        let hwnd = unsafe {
            // Switch to DPI aware context. Window HWND created after this will
            // inherit the setting and become DPI aware independent of the host
            // application's setting.
            let old_context =
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let dpi_aware_hwnd = CreateWindowExW(
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
            // Restore previous DPI context so we don't interfere with the
            // host application.
            SetThreadDpiAwarenessContext(old_context);
            dpi_aware_hwnd
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
            let _ = SetWindowPos(
                self.hwnd(),
                Some(HWND_TOPMOST),
                x,
                y,
                0,
                0,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            );
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
