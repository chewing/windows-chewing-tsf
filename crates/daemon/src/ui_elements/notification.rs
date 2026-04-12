// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{cell::RefCell, fmt::Debug, rc::Rc, time::Duration};

use exn::{Result, ResultExt};
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
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
    UI::WindowsAndMessaging::{
        CS_IME, GWLP_USERDATA, GetWindowLongPtrW, HWND_DESKTOP, IDC_ARROW, KillTimer, LoadCursorW,
        RegisterClassExW, SetTimer, WINDOWPOS, WM_PAINT, WM_TIMER, WM_WINDOWPOSCHANGING,
        WNDCLASSEXW, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    },
};
use windows_core::{HSTRING, PCWSTR, w};

use crate::ui::{
    UiError,
    gfx::{
        create_render_target, create_swapchain, create_swapchain_bitmap, d3d11_device,
        get_dpi_for_window, get_scale_for_window, setup_direct_composition,
    },
    message_box::draw_message_box,
    window::Window,
};

const ID_TIMEOUT: usize = 1;

#[derive(Debug)]
pub(crate) struct Notification {
    model: RefCell<NotificationModel>,
    view: RefCell<RenderedView>,
}

#[derive(Debug, Default)]
pub(crate) struct NotificationModel {
    pub(crate) text: HSTRING,
    pub(crate) font_family: HSTRING,
    pub(crate) font_size: f32,
    pub(crate) fg_color: D2D1_COLOR_F,
    pub(crate) bg_color: D2D1_COLOR_F,
    pub(crate) border_color: D2D1_COLOR_F,
}

trait View: Debug {
    fn window(&self) -> &Window;
    fn calculate_client_rect(&self, model: &NotificationModel) -> Result<RenderedMetrics, UiError>;
    fn on_paint(&self, model: &NotificationModel) -> Result<(), UiError>;
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let get_this = || unsafe {
        let this_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Notification;
        Rc::increment_strong_count(this_ptr);
        Rc::from_raw(this_ptr)
    };
    match msg {
        WM_PAINT => {
            let this = get_this();
            let view = this.view.borrow();
            let window = view.window();
            let model = this.model.borrow();
            let mut ps = PAINTSTRUCT::default();
            unsafe { BeginPaint(window.hwnd(), &mut ps) };
            let _ = view.on_paint(&model);
            let _ = unsafe { EndPaint(window.hwnd(), &ps) };
            LRESULT(0)
        }
        WM_WINDOWPOSCHANGING => {
            let this = get_this();
            let view = this.view.borrow();
            // Recalculate the client rect
            let model = this.model.borrow();
            if let Ok(metrics) = view.calculate_client_rect(&model) {
                let pos = lparam.0 as *mut WINDOWPOS;
                unsafe {
                    (*pos).cx = metrics.hw_width as i32;
                    (*pos).cy = metrics.hw_height as i32;
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let this = get_this();
            if wparam.0 == ID_TIMEOUT {
                let view = this.view.borrow();
                let window = view.window();
                let _ = unsafe { KillTimer(Some(window.hwnd()), ID_TIMEOUT) };
                window.hide();
            }
            LRESULT(0)
        }
        _ => crate::ui::window::wnd_proc(hwnd, msg, wparam, lparam),
    }
}

#[derive(Debug)]
struct RenderedView {
    _factory: ID2D1Factory1,
    _dcomptarget: IDCompositionTarget,
    dwrite_factory: IDWriteFactory1,
    target: ID2D1DeviceContext,
    swapchain: IDXGISwapChain1,
    window: Window,
}

struct RenderedMetrics {
    width: f32,
    height: f32,
    hw_width: f32,
    hw_height: f32,
}

// TODO: make this generic - complete same as CandidateList
impl RenderedView {
    fn new(user_data: *const Notification) -> Result<RenderedView, UiError> {
        let err = || UiError(format!("failed to create new RenderedView"));

        let window = Window::new();
        window.create(
            HWND_DESKTOP,
            w!("ChewingNotificationWindow"),
            WS_POPUP | WS_CLIPCHILDREN,
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            user_data.cast(),
        );
        unsafe {
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).or_raise(err)?;
            let dwrite_factory: IDWriteFactory1 =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).or_raise(err)?;
            let device = d3d11_device().or_raise(err)?;
            let target = create_render_target(&factory, &device).or_raise(err)?;
            let swapchain = create_swapchain(&device, 10, 10).or_raise(err)?;
            let dpi = get_dpi_for_window(window.hwnd());
            target.SetDpi(dpi, dpi);
            create_swapchain_bitmap(&swapchain, &target, dpi).or_raise(err)?;
            let dcomptarget =
                setup_direct_composition(&device, window.hwnd(), &swapchain).or_raise(err)?;
            Ok(RenderedView {
                _factory: factory,
                _dcomptarget: dcomptarget,
                dwrite_factory,
                target,
                swapchain,
                window,
            })
        }
    }
}

impl View for RenderedView {
    fn window(&self) -> &Window {
        &self.window
    }
    fn calculate_client_rect(&self, model: &NotificationModel) -> Result<RenderedMetrics, UiError> {
        let err = || UiError(format!("failed to calculate client area"));

        let scale = get_scale_for_window(self.window.hwnd());
        let text_format = unsafe {
            self.dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-TW"),
                )
                .or_raise(err)?
        };
        let text_layout = unsafe {
            self.dwrite_factory
                .CreateTextLayout(&model.text, &text_format, f32::MAX, f32::MAX)
                .or_raise(err)?
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe { text_layout.GetMetrics(&mut metrics).or_raise(err)? };

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

    fn on_paint(&self, model: &NotificationModel) -> Result<(), UiError> {
        let err = || UiError("failed to paint UI".to_string());

        if model.text.is_empty() {
            return Ok(());
        }
        let text_format = unsafe {
            self.dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-TW"),
                )
                .or_raise(err)?
        };

        let RenderedMetrics {
            width,
            height,
            hw_width,
            hw_height,
        } = self.calculate_client_rect(model).or_raise(err)?;
        unsafe {
            self.target.SetTarget(None);
            self.swapchain
                .ResizeBuffers(
                    0,
                    hw_width as u32,
                    hw_height as u32,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
                .or_raise(err)?;
        }
        let dpi = get_dpi_for_window(self.window.hwnd());
        unsafe {
            self.target.SetDpi(dpi, dpi);
        }
        create_swapchain_bitmap(&self.swapchain, &self.target, dpi).or_raise(err)?;

        // Begin drawing
        let dc = &self.target;
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
            )
            .or_raise(err)?;

            let margin = 10.0;
            let text_rect = D2D_RECT_F {
                left: margin,
                top: margin,
                right: margin + width,
                bottom: margin + height,
            };
            let brush = dc
                .CreateSolidColorBrush(&model.fg_color, None)
                .or_raise(err)?;
            dc.DrawText(
                &model.text,
                &text_format,
                &text_rect,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            dc.EndDraw(None, None).or_raise(err)?;

            // Present the draw buffer
            self.swapchain
                .Present(1, DXGI_PRESENT(0))
                .ok()
                .or_raise(err)?;
        }
        Ok(())
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
    pub(crate) fn new() -> Result<Rc<Notification>, UiError> {
        let err = || UiError("failed to create notification window".to_string());
        let mut notification = Rc::new_uninit();
        let user_data = Rc::as_ptr(&notification);
        Rc::get_mut(&mut notification).unwrap().write(Notification {
            model: RefCell::new(NotificationModel::default()),
            view: RefCell::new(RenderedView::new(user_data.cast()).or_raise(err)?),
        });
        // SAFETY: notification is unconditionally initialized
        unsafe { Ok(notification.assume_init()) }
    }
    pub(crate) fn set_timer(&self, dur: Duration) {
        let view = self.view.borrow();
        let window = view.window();
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
    pub(crate) fn set_model(&self, model: NotificationModel) {
        *self.model.borrow_mut() = model;
    }
    pub(crate) fn set_position(&self, x: i32, y: i32) {
        let view = self.view.borrow();
        let window = view.window();
        window.set_position(x, y);
    }
    pub(crate) fn show(&self) {
        let view = self.view.borrow();
        let window = view.window();
        window.show();
        window.refresh();
    }
}
