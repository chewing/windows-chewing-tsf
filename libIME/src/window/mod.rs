use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    ffi::{c_int, c_uint, c_void},
    ptr::null_mut,
};

use windows::{
    core::{implement, interface, w, IUnknown, IUnknown_Vtbl, Interface, Weak},
    Win32::{
        Foundation::{FALSE, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            GetMonitorInfoW, InvalidateRect, MonitorFromRect, HBRUSH, MONITORINFO,
            MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowRect, IsWindow,
            IsWindowVisible, LoadCursorW, MoveWindow, RegisterClassExW, SetWindowPos, ShowWindow,
            CS_IME, HICON, HWND_TOP, IDC_ARROW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_HIDE,
            SW_SHOWNA, WINDOW_EX_STYLE, WINDOW_STYLE, WM_NCDESTROY, WNDCLASSEXW,
        },
    },
};
use windows_core::PCWSTR;

mod message_window;

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
    fn new() -> Window {
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

#[no_mangle]
pub unsafe extern "C" fn CreateImeWindow(ret: *mut *mut c_void) {
    let window: IWindow = Window::new().into();
    ret.write(window.into_raw())
}

#[no_mangle]
pub unsafe extern "C" fn ImeWindowFromHwnd(hwnd: HWND) -> *mut IWindow {
    HWND_MAP.with_borrow(|hwnd_map| {
        if let Some(window) = hwnd_map.get(&hwnd.0).and_then(Weak::upgrade) {
            window.clone().into_raw().cast()
        } else {
            null_mut()
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn ImeWindowRegisterClass(hinstance: HINSTANCE) -> bool {
    MODULE_HINSTANCE.with(|hinst_cell| {
        let hinst = hinst_cell.get_or_init(|| hinstance);
        let mut wc = WNDCLASSEXW::default();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.style = CS_IME;
        wc.lpfnWndProc = Some(wnd_proc);
        wc.cbClsExtra = 0;
        wc.cbWndExtra = 0;
        wc.hInstance = *hinst;
        wc.hCursor = LoadCursorW(None, IDC_ARROW).expect("failed to load cursor");
        wc.hIcon = HICON::default();
        wc.lpszMenuName = PCWSTR::null();
        wc.lpszClassName = w!("LibIme2Window");
        wc.hbrBackground = HBRUSH::default();
        wc.hIconSm = HICON::default();

        RegisterClassExW(&wc) > 0
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
            window.wnd_proc(msg, wparam, lparam)
        } else {
            DefWindowProcW(hwnd, msg, wparam, lparam)
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
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(ex_style),
                w!("LibIme2Window"),
                None,
                WINDOW_STYLE(style),
                0,
                0,
                0,
                0,
                parent,
                None,
                hinst.get(),
                None,
            );
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
        if !self.hwnd().is_invalid() {
            unsafe {
                let _ = DestroyWindow(self.hwnd());
            }
            self.hwnd.set(HWND::default());
        }
    }

    unsafe fn is_visible(&self) -> bool {
        IsWindowVisible(self.hwnd()).as_bool()
    }

    unsafe fn is_window(&self) -> bool {
        IsWindow(self.hwnd()).as_bool()
    }

    unsafe fn r#move(&self, mut x: c_int, mut y: c_int) {
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
        let monitor = MonitorFromRect(&rc, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO::default();
        mi.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut mi).as_bool() {
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

        let _ = MoveWindow(self.hwnd(), x, y, w, h, true);
    }

    unsafe fn size(&self, width: *mut c_int, height: *mut c_int) {
        let mut rc = RECT::default();
        GetWindowRect(self.hwnd(), &mut rc).expect("failed to get window rect");
        unsafe {
            width.write(rc.right - rc.left);
            height.write(rc.bottom - rc.top);
        }
    }

    unsafe fn resize(&self, width: c_int, height: c_int) {
        SetWindowPos(
            self.hwnd(),
            HWND_TOP,
            0,
            0,
            width,
            height,
            SWP_NOZORDER | SWP_NOMOVE | SWP_NOACTIVATE,
        )
        .expect("failed to resize window");
    }

    unsafe fn client_rect(&self, rect: *mut RECT) {
        GetClientRect(self.hwnd(), rect).expect("failed to get client area");
    }

    unsafe fn rect(&self, rect: *mut RECT) {
        GetWindowRect(self.hwnd(), rect).expect("failed to get window rect");
    }

    unsafe fn show(&self) {
        if !self.hwnd().is_invalid() {
            let _ = ShowWindow(self.hwnd(), SW_SHOWNA);
        }
    }

    unsafe fn hide(&self) {
        if !self.hwnd().is_invalid() {
            let _ = ShowWindow(self.hwnd(), SW_HIDE);
        }
    }

    unsafe fn refresh(&self) {
        if !self.hwnd().is_invalid() {
            let _ = InvalidateRect(self.hwnd(), None, FALSE);
        }
    }

    unsafe fn wnd_proc(&self, msg: c_uint, wp: WPARAM, lp: LPARAM) -> LRESULT {
        DefWindowProcW(self.hwnd(), msg, wp, lp)
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
