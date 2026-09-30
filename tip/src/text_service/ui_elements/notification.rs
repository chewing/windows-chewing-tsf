// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Duration,
};

use anyhow::{Context, Result};
use log::error;
use scoped_error::expect_error;
use windows::Win32::{
    Foundation::{E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, TRUE, WPARAM},
    Graphics::{
        Direct2D::{
            Common::{D2D_RECT_F, D2D1_COLOR_F},
            D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory,
            ID2D1DeviceContext, ID2D1Factory1,
        },
        DirectComposition::IDCompositionTarget,
        DirectWrite::{
            DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_METRICS,
            DWriteCreateFactory, IDWriteFactory1,
        },
        Dxgi::{
            Common::DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_PRESENT, DXGI_SWAP_CHAIN_FLAG, IDXGISwapChain1,
        },
        Gdi::{BeginPaint, EndPaint, PAINTSTRUCT},
    },
    UI::{
        TextServices::{ITfThreadMgr, ITfUIElement, ITfUIElement_Impl, ITfUIElementMgr},
        WindowsAndMessaging::{
            CS_IME, GWLP_USERDATA, GetWindowLongPtrW, IDC_ARROW, KillTimer, LoadCursorW,
            RegisterClassExW, SetTimer, WINDOWPOS, WM_NCDESTROY, WM_PAINT, WM_TIMER,
            WM_WINDOWPOSCHANGING, WNDCLASSEXW, WS_CLIPCHILDREN, WS_EX_NOREDIRECTIONBITMAP,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};
use windows_core::{
    BOOL, BSTR, ComObject, ComObjectInner, GUID, HSTRING, Interface, PCWSTR,
    Result as WindowsResult, implement, w,
};

use crate::{
    text_service::ui_elements::UiError,
    ui::{
        gfx::{
            clamp_point_to_monitor, create_render_target, create_swapchain,
            create_swapchain_bitmap, d3d11_device, get_dpi_for_point, get_dpi_for_window,
            setup_direct_composition,
        },
        window::Window,
    },
};

use super::message_box::draw_message_box;

const ID_TIMEOUT: usize = 1;

#[implement(ITfUIElement)]
pub(crate) struct Notification {
    thread_mgr: ITfThreadMgr,
    element_id: Cell<u32>,
    parent: HWND,
    inner: Rc<NotificationInner>,
}

struct NotificationInner {
    model: RefCell<NotificationModel>,
    view: RefCell<View>,
}

#[derive(Default)]
pub(crate) struct NotificationModel {
    pub(crate) text: HSTRING,
    pub(crate) font_family: HSTRING,
    pub(crate) font_size: f32,
    pub(crate) fg_color: D2D1_COLOR_F,
    pub(crate) bg_color: D2D1_COLOR_F,
    pub(crate) border_color: D2D1_COLOR_F,
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let this = unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const NotificationInner;
        if ptr.is_null() {
            // not initialized yet
            return crate::ui::window::wnd_proc(hwnd, msg, wparam, lparam);
        }
        let weak = Weak::from_raw(ptr);
        let this = weak.upgrade();
        let _ = weak.into_raw();
        match this {
            Some(t) => t,
            None => return crate::ui::window::wnd_proc(hwnd, msg, wparam, lparam),
        }
    };
    match msg {
        WM_NCDESTROY => unsafe {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const NotificationInner;
            let _ = Weak::from_raw(ptr);
            LRESULT(0)
        },
        WM_PAINT => {
            let view = this.view.borrow();
            let model = this.model.borrow();
            let mut ps = PAINTSTRUCT::default();
            unsafe { BeginPaint(hwnd, &mut ps) };
            let _ = view.on_paint(&model);
            let _ = unsafe { EndPaint(hwnd, &ps) };
            LRESULT(0)
        }
        WM_WINDOWPOSCHANGING => {
            let pos = lparam.0 as *mut WINDOWPOS;
            if let Some(pos) = unsafe { pos.as_mut() } {
                let view = this.view.borrow();
                let model = this.model.borrow();
                let dpi = get_dpi_for_point(POINT { x: pos.x, y: pos.y });
                if let Ok(size) = view.calculate_client_rect(&model, dpi) {
                    pos.cx = size.hw_width as i32;
                    pos.cy = size.hw_height as i32;
                    (pos.x, pos.y) = clamp_point_to_monitor(pos.x, pos.y, pos.cx, pos.cy);
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == ID_TIMEOUT {
                let view = this.view.borrow();
                let window = view.window().expect("View window");
                let _ = unsafe { KillTimer(Some(hwnd), ID_TIMEOUT) };
                window.hide();
            }
            LRESULT(0)
        }
        _ => crate::ui::window::wnd_proc(hwnd, msg, wparam, lparam),
    }
}

enum View {
    Dummy,
    Rendered {
        _factory: ID2D1Factory1,
        _dcomptarget: IDCompositionTarget,
        dwrite_factory: IDWriteFactory1,
        target: ID2D1DeviceContext,
        swapchain: IDXGISwapChain1,
        window: Window,
    },
}

#[derive(Debug, Default)]
struct RenderedMetrics {
    width: f32,
    height: f32,
    hw_width: f32,
    hw_height: f32,
}

// TODO: make this generic - complete same as CandidateList
impl View {
    fn rendered(parent: HWND, user_data: Weak<NotificationInner>) -> Result<View, UiError> {
        expect_error("Failed to create new RenderedView", || {
            let window = Window::new();
            window.create(
                parent,
                w!("ChewingNotificationWindow"),
                WS_POPUP | WS_CLIPCHILDREN,
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                user_data.into_raw().cast(),
            );
            unsafe {
                let factory: ID2D1Factory1 =
                    D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
                let dwrite_factory: IDWriteFactory1 =
                    DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
                let device = d3d11_device()?;
                let target = create_render_target(&factory, &device)?;
                let swapchain = create_swapchain(&device, 10, 10)?;
                let dpi = get_dpi_for_window(window.hwnd());
                target.SetDpi(dpi, dpi);
                create_swapchain_bitmap(&swapchain, &target)?;
                let dcomptarget = setup_direct_composition(&device, window.hwnd(), &swapchain)?;
                Ok(View::Rendered {
                    _factory: factory,
                    _dcomptarget: dcomptarget,
                    dwrite_factory,
                    target,
                    swapchain,
                    window,
                })
            }
        })
    }
}

impl View {
    fn window(&self) -> Option<&Window> {
        match self {
            View::Dummy => None,
            View::Rendered { window, .. } => Some(window),
        }
    }
    fn calculate_client_rect(
        &self,
        model: &NotificationModel,
        dpi: f32,
    ) -> Result<RenderedMetrics> {
        match self {
            View::Dummy => Ok(RenderedMetrics::default()),
            View::Rendered { dwrite_factory, .. } => {
                let scale = dpi / 96.0;
                let text_format = unsafe {
                    dwrite_factory.CreateTextFormat(
                        &model.font_family,
                        None,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        model.font_size,
                        w!("zh-TW"),
                    )?
                };
                let text_layout = unsafe {
                    dwrite_factory.CreateTextLayout(
                        &model.text,
                        &text_format,
                        f32::MAX,
                        f32::MAX,
                    )?
                };
                let mut metrics = DWRITE_TEXT_METRICS::default();
                unsafe { text_layout.GetMetrics(&mut metrics)? };

                let margin = 10.0;
                let width = metrics.width + margin * 2.0;
                let height = metrics.height + margin * 2.0;

                // Convert to HW pixels
                let hw_width = (width * scale + 25.0).ceil();
                let hw_height = (height * scale + 25.0).ceil();

                Ok(RenderedMetrics {
                    width,
                    height,
                    hw_width,
                    hw_height,
                })
            }
        }
    }

    fn on_paint(&self, model: &NotificationModel) -> Result<()> {
        match self {
            View::Dummy => Ok(()),
            View::Rendered {
                dwrite_factory,
                target,
                swapchain,
                window,
                ..
            } => {
                if model.text.is_empty() {
                    return Ok(());
                }
                let text_format = unsafe {
                    dwrite_factory.CreateTextFormat(
                        &model.font_family,
                        None,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        model.font_size,
                        w!("zh-TW"),
                    )?
                };

                let dpi = get_dpi_for_window(window.hwnd());
                let RenderedMetrics {
                    width,
                    height,
                    hw_width,
                    hw_height,
                } = self.calculate_client_rect(model, dpi)?;
                unsafe {
                    target.SetTarget(None);
                    swapchain.ResizeBuffers(
                        0,
                        hw_width as u32,
                        hw_height as u32,
                        DXGI_FORMAT_B8G8R8A8_UNORM,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )?;
                    target.SetDpi(dpi, dpi);
                }
                create_swapchain_bitmap(&swapchain, &target)?;

                // Begin drawing
                let dc = &target;
                unsafe {
                    dc.BeginDraw();

                    draw_message_box(
                        dc,
                        0.0,
                        0.0,
                        width,
                        height,
                        model.bg_color,
                        model.border_color,
                    )?;

                    let margin = 10.0;
                    let text_rect = D2D_RECT_F {
                        left: margin,
                        top: margin,
                        right: margin + width,
                        bottom: margin + height,
                    };
                    let brush = dc.CreateSolidColorBrush(&model.fg_color, None)?;
                    dc.DrawText(
                        &model.text,
                        &text_format,
                        &text_rect,
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                    dc.EndDraw(None, None)?;

                    // Present the draw buffer
                    swapchain
                        .Present(1, DXGI_PRESENT(0))
                        .ok()
                        .context("unable to present buffer")?;
                }
                Ok(())
            }
        }
    }
}

impl Notification {
    pub(crate) fn window_register_class(hinst: HINSTANCE) {
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_IME,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
            lpszMenuName: PCWSTR::null(),
            lpszClassName: w!("ChewingNotificationWindow"),
            ..Default::default()
        };
        unsafe { RegisterClassExW(&wc) };
    }
    pub(crate) fn new(parent: HWND, thread_mgr: ITfThreadMgr) -> Result<ComObject<Notification>> {
        let ui_manager: ITfUIElementMgr = thread_mgr.cast()?;
        let inner = Rc::new(NotificationInner {
            model: RefCell::new(NotificationModel::default()),
            view: RefCell::new(View::Dummy),
        });
        let candidate_list = Notification {
            thread_mgr,
            element_id: Cell::new(0),
            parent,
            inner,
        }
        .into_object();
        let mut should_show = TRUE;
        let mut ui_element_id = 0;
        let ui_element: ITfUIElement = candidate_list.cast()?;
        unsafe {
            ui_manager.BeginUIElement(&ui_element, &mut should_show, &mut ui_element_id)?;
            candidate_list.set_element_id(ui_element_id);
            candidate_list.Show(should_show)?;
        }
        Ok(candidate_list)
    }
    pub(crate) fn end_ui_element(&self) {
        let Ok(ui_manager): Result<ITfUIElementMgr, windows_core::Error> = self.thread_mgr.cast()
        else {
            error!("unable to cast thread manager to ITfUIElementMgr");
            return;
        };
        unsafe {
            let _ = ui_manager.EndUIElement(self.element_id.get());
        }
    }
    fn set_element_id(&self, id: u32) {
        self.element_id.set(id);
    }
    fn update_ui_element(&self) -> Result<()> {
        let ui_manager: ITfUIElementMgr = self.thread_mgr.cast()?;
        unsafe {
            ui_manager.UpdateUIElement(self.element_id.get())?;
        }
        Ok(())
    }
    pub(crate) fn set_timer(&self, dur: Duration) {
        if let Some(window) = self.inner.view.borrow().window() {
            if dur.is_zero() {
                unsafe {
                    let _ = KillTimer(Some(window.hwnd()), ID_TIMEOUT);
                }
            } else {
                unsafe {
                    SetTimer(
                        Some(window.hwnd()),
                        ID_TIMEOUT,
                        dur.as_millis() as u32,
                        None,
                    );
                }
            }
        }
    }
    pub(crate) fn set_model(&self, model: NotificationModel) {
        *self.inner.model.borrow_mut() = model;
        if let Err(error) = self.update_ui_element() {
            error!("Failed to update UI element: {error}");
        }
    }
    pub(crate) fn set_position(&self, x: i32, y: i32) {
        if let Some(window) = self.inner.view.borrow().window() {
            window.set_position(x, y);
        }
    }
    pub(crate) fn show(&self) {
        if let Some(window) = self.inner.view.borrow().window() {
            window.refresh();
            window.show();
        }
    }
}

impl ITfUIElement_Impl for Notification_Impl {
    fn GetDescription(&self) -> WindowsResult<BSTR> {
        Ok(BSTR::from("Candidate List"))
    }

    fn GetGUID(&self) -> WindowsResult<GUID> {
        Ok(GUID::from_u128(0x80cd1c64_5c4a_4478_8690_20c489534629))
    }

    fn Show(&self, show: BOOL) -> WindowsResult<()> {
        if show.as_bool() {
            let inner = Rc::downgrade(&self.inner);
            let view = View::rendered(self.parent, inner).map_err(|_| E_FAIL)?;
            self.inner.view.replace(view);
            self.show();
        } else {
            self.inner.view.replace(View::Dummy);
        }
        Ok(())
    }

    fn IsShown(&self) -> WindowsResult<BOOL> {
        Ok(self.inner.view.borrow().window().is_some().into())
    }
}
