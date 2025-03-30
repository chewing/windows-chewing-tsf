use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    ffi::{c_int, c_uint, c_void},
    ptr::null_mut,
};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

mod candidate_window;
mod message_window;

pub(crate) use candidate_window::{CandidateWindow, ICandidateWindow_Impl};
pub(crate) use message_window::{IMessageWindow_Impl, MessageWindow};

thread_local! {
    static MODULE_HINSTANCE: OnceCell<HINSTANCE> = const { OnceCell::new() };
    static HWND_MAP: RefCell<HashMap<*mut c_void, Weak<IWindow>>> = RefCell::new(HashMap::new());
}

#[interface("d73284e1-59aa-42ef-84ca-1633beca464b")]
pub(crate) unsafe trait IWindow: IUnknown {
    fn hwnd(&self) -> HWND;
    fn create(&self, parent: HWND, style: u32, ex_style: u32) -> bool;
    fn destroy(&self);
    fn is_visible(&self) -> bool;
    fn is_window(&self) -> bool;
    fn r#move(&self, x: c_int, y: c_int);
    fn size(&self, width: *mut c_int, height: *mut c_int);
    fn resize(&self, width: c_int, height: c_int);
    fn client_rect(&self, rect: *mut RECT);
    fn rect(&self, rect: *mut RECT);
    fn show(&self);
    fn hide(&self);
    fn refresh(&self);
    fn wnd_proc(&self, msg: c_uint, wp: WPARAM, lp: LPARAM) -> LRESULT;
}

#[derive(Debug)]
#[implement(IWindow)]
pub(crate) struct Window {
    hwnd: Cell<HWND>,
}

impl Window {
    pub(crate) fn new() -> Window {
        Window {
            hwnd: Cell::new(HWND::default()),
        }
    }
    fn register_hwnd(hwnd: HWND, window: IWindow) {
        let weak_ref = window
            .downgrade()
            .expect("unable to create weak ref from window");
        HWND_MAP.with_borrow_mut(|hwnd_map| {
            hwnd_map.insert(hwnd.0, weak_ref);
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn CreateImeWindow(ret: *mut *mut c_void) {
    let window: IWindow = Window::new().into();
    unsafe { ret.write(window.into_raw()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ImeWindowFromHwnd(hwnd: HWND) -> *mut IWindow {
    HWND_MAP.with_borrow(|hwnd_map| {
        if let Some(window) = hwnd_map.get(&hwnd.0).and_then(Weak::upgrade) {
            window.clone().into_raw().cast()
        } else {
            null_mut()
        }
    })
}

pub(crate) fn window_register_class(hinstance: HINSTANCE) -> bool {
    MODULE_HINSTANCE.with(|hinst_cell| {
        let hinst = hinst_cell.get_or_init(|| hinstance);
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_IME,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: *hinst,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).expect("failed to load cursor") },
            lpszMenuName: PCWSTR::null(),
            lpszClassName: w!("LibIme2Window"),
            ..Default::default()
        };

        unsafe { RegisterClassExW(&wc) > 0 }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ImeWindowRegisterClass(hinstance: HINSTANCE) -> bool {
    MODULE_HINSTANCE.with(|hinst_cell| {
        let hinst = hinst_cell.get_or_init(|| hinstance);
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_IME,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: *hinst,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).expect("failed to load cursor") },
            lpszMenuName: PCWSTR::null(),
            lpszClassName: w!("LibIme2Window"),
            ..Default::default()
        };

        unsafe { RegisterClassExW(&wc) > 0 }
    })
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let result = HWND_MAP.with_borrow(|hwnd_map| {
        if let Some(window) = hwnd_map.get(&hwnd.0).and_then(Weak::upgrade) {
            unsafe { window.wnd_proc(msg, wparam, lparam) }
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

impl IWindow_Impl for Window_Impl {
    unsafe fn hwnd(&self) -> HWND {
        self.hwnd.get()
    }

    unsafe fn create(&self, parent: HWND, style: u32, ex_style: u32) -> bool {
        MODULE_HINSTANCE.with(|hinst| {
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(ex_style),
                    w!("LibIme2Window"),
                    None,
                    WINDOW_STYLE(style),
                    0,
                    0,
                    0,
                    0,
                    Some(parent),
                    None,
                    hinst.get().copied(),
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
        })
    }

    unsafe fn destroy(&self) {
        unsafe {
            if !self.hwnd().is_invalid() {
                let _ = DestroyWindow(self.hwnd());
                self.hwnd.set(HWND::default());
            }
        }
    }

    unsafe fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd()).as_bool() }
    }

    unsafe fn is_window(&self) -> bool {
        unsafe { IsWindow(Some(self.hwnd())).as_bool() }
    }

    unsafe fn r#move(&self, mut x: c_int, mut y: c_int) {
        let mut w = 0;
        let mut h = 0;
        unsafe { self.size(&mut w, &mut h) };

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

    unsafe fn size(&self, width: *mut c_int, height: *mut c_int) {
        let mut rc = RECT::default();
        unsafe { GetWindowRect(self.hwnd(), &mut rc).expect("failed to get window rect") };
        unsafe {
            width.write(rc.right - rc.left);
            height.write(rc.bottom - rc.top);
        }
    }

    unsafe fn resize(&self, width: c_int, height: c_int) {
        unsafe {
            SetWindowPos(
                self.hwnd(),
                Some(HWND_TOP),
                0,
                0,
                width,
                height,
                SWP_NOZORDER | SWP_NOMOVE | SWP_NOACTIVATE,
            )
            .expect("failed to resize window")
        };
    }

    unsafe fn client_rect(&self, rect: *mut RECT) {
        unsafe { GetClientRect(self.hwnd(), rect).expect("failed to get client area") };
    }

    unsafe fn rect(&self, rect: *mut RECT) {
        unsafe {
            GetWindowRect(self.hwnd(), rect).expect("failed to get window rect");
        }
    }

    unsafe fn show(&self) {
        unsafe {
            if !self.hwnd().is_invalid() {
                let _ = ShowWindow(self.hwnd(), SW_SHOWNA);
            }
        }
    }

    unsafe fn hide(&self) {
        unsafe {
            if !self.hwnd().is_invalid() {
                let _ = ShowWindow(self.hwnd(), SW_HIDE);
            }
        }
    }

    unsafe fn refresh(&self) {
        unsafe {
            if !self.hwnd().is_invalid() {
                let _ = InvalidateRect(Some(self.hwnd()), None, true);
            }
        }
    }

    unsafe fn wnd_proc(&self, msg: c_uint, wp: WPARAM, lp: LPARAM) -> LRESULT {
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
