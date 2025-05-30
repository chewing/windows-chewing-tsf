// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{c_int, c_uint, c_void},
    rc::{Rc, Weak},
    sync::atomic::Ordering,
};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

mod candidate_window;
mod message_window;

pub(crate) use candidate_window::CandidateWindow;
pub(crate) use message_window::MessageWindow;

use crate::G_HINSTANCE;

thread_local! {
    static HWND_MAP: RefCell<HashMap<*mut c_void, Weak<dyn WndProc>>> = RefCell::new(HashMap::new());
}

pub(crate) trait WndProc {
    fn wnd_proc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}

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
    fn register_hwnd(hwnd: HWND, window: Rc<dyn WndProc>) {
        let weak_ref = Rc::downgrade(&window);
        HWND_MAP.with_borrow_mut(|hwnd_map| {
            hwnd_map.insert(hwnd.0, weak_ref);
        })
    }
}

pub(crate) fn window_register_class() -> bool {
    let hinst = HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut c_void);
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_IME,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
        lpszMenuName: PCWSTR::null(),
        lpszClassName: w!("chewing_tip"),
        ..Default::default()
    };

    unsafe { RegisterClassExW(&wc) > 0 }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let result = HWND_MAP.with_borrow(|hwnd_map| {
        if let Some(window) = hwnd_map.get(&hwnd.0).and_then(Weak::upgrade) {
            window.wnd_proc(msg, wparam, lparam)
        } else {
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    });
    if msg == WM_NCDESTROY {
        HWND_MAP.with(|refcell| {
            if let Ok(mut hwnd_map) = refcell.try_borrow_mut() {
                hwnd_map.remove(&hwnd.0);
            }
        });
    }
    result
}

impl Window {
    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd.get()
    }

    pub(crate) fn create(&self, parent: HWND, style: u32, ex_style: u32) -> bool {
        let hinst = HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut c_void);
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(ex_style),
                w!("chewing_tip"),
                None,
                WINDOW_STYLE(style),
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                Some(hinst),
                None,
            )
        };
        match hwnd {
            Ok(hwnd) => {
                self.hwnd.set(hwnd);
                true
            }
            Err(_) => false,
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd()).as_bool() }
    }

    pub(crate) fn r#move(&self, mut x: c_int, mut y: c_int) {
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
        if unsafe { GetMonitorInfoW(monitor, &mut mi).as_bool() } {
            rc = mi.rcWork;
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

    pub(crate) fn resize(&self, width: c_int, height: c_int) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd(),
                Some(HWND_TOP),
                0,
                0,
                width,
                height,
                SWP_NOZORDER | SWP_NOMOVE | SWP_NOACTIVATE,
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
            }
        }
    }

    pub(crate) fn wnd_proc(&self, msg: c_uint, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(self.hwnd(), msg, wp, lp) }
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
