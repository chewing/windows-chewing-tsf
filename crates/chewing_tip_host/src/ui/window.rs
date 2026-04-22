// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    cell::Cell,
    ffi::{c_int, c_void},
};

use log::error;
use windows::Win32::{Foundation::*, UI::HiDpi::SetThreadDpiAwarenessContext};
use windows::Win32::{Graphics::Gdi::*, UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
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

    pub(crate) fn set_position(&self, mut x: c_int, mut y: c_int) {
        let mut w = 0;
        let mut h = 0;
        self.size(&mut w, &mut h);

        let mut rc = RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        };

        // ensure that the window does not fall outside of the screen.
        let monitor = unsafe { MonitorFromRect(&rc, MONITOR_DEFAULTTONEAREST) };
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            if GetMonitorInfoW(monitor, &mut mi).as_bool() {
                rc = mi.rcWork;
            }
        }

        if x < rc.left {
            x = rc.left;
        } else if (x + w) > rc.right {
            x = rc.right - w;
        }

        if y < rc.top {
            y = rc.top;
        } else if (y + h) > rc.bottom {
            y = rc.bottom - h;
        }

        let _ = unsafe { MoveWindow(self.hwnd(), x, y, w, h, true) };
    }

    pub(crate) fn size(&self, width: *mut c_int, height: *mut c_int) {
        let mut rc = RECT::default();
        unsafe {
            let _ = GetWindowRect(self.hwnd(), &mut rc);
            width.write(rc.right - rc.left);
            height.write(rc.bottom - rc.top);
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
