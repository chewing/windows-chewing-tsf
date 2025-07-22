use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

use anyhow::{Context, Result};
use log::error;
use windows::Win32::{
    Foundation::{E_FAIL, FALSE, HWND, LPARAM, LRESULT, TRUE, WPARAM},
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
            KillTimer, SetTimer, WINDOWPOS, WM_PAINT, WM_TIMER, WM_WINDOWPOSCHANGING,
            WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};
use windows_core::{
    BOOL, BSTR, ComObject, ComObjectInner, ComObjectInterface, GUID, HSTRING, Interface,
    InterfaceRef, Result as WindowsResult, implement, w,
};

use crate::{
    gfx::{
        create_device, create_render_target, create_swapchain, create_swapchain_bitmap,
        dwrite_family_from_gdi_name, get_dpi_for_window, get_scale_for_window,
        setup_direct_composition,
    },
    ts::ui_elements::message_box::draw_message_box,
    window::{IWndProc, IWndProc_Impl, Window},
};

const ID_TIMEOUT: usize = 1;

#[implement(ITfUIElement, IWndProc)]
pub(crate) struct Notification {
    thread_mgr: ITfThreadMgr,
    element_id: Cell<u32>,
    parent: HWND,
    model: RefCell<NotificationModel>,
    view: RefCell<Box<dyn View>>,
}

#[derive(Default)]
pub(crate) struct NotificationModel {
    pub(crate) text: HSTRING,
    pub(crate) font_family: HSTRING,
    pub(crate) font_size: f32,
}

trait View {
    fn window(&self) -> Option<&Window>;
    fn calculate_client_rect(&self, model: &NotificationModel) -> Result<RenderedMetrics>;
    fn on_paint(&self, model: &NotificationModel) -> Result<()>;
}

// TODO: make this generic - complete same as CandidateList
impl IWndProc_Impl for Notification_Impl {
    unsafe fn wnd_proc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_PAINT => {
                let view = self.view.borrow();
                let Some(window) = view.window() else {
                    return LRESULT(0);
                };
                let model = self.model.borrow();
                let mut ps = PAINTSTRUCT::default();
                unsafe { BeginPaint(window.hwnd(), &mut ps) };
                let _ = view.on_paint(&model);
                let _ = unsafe { EndPaint(window.hwnd(), &ps) };
                LRESULT(0)
            }
            WM_WINDOWPOSCHANGING => {
                let view = self.view.borrow();
                // Recalculate the client rect
                let model = self.model.borrow();
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
                if wparam.0 == ID_TIMEOUT {
                    let _ = self.Show(FALSE);
                    let view = self.view.borrow();
                    let Some(window) = view.window() else {
                        return LRESULT(0);
                    };
                    let _ = unsafe { KillTimer(Some(window.hwnd()), ID_TIMEOUT) };
                }
                LRESULT(0)
            }
            _ => {
                let view = self.view.borrow();
                let Some(window) = view.window() else {
                    return LRESULT(0);
                };
                window.wnd_proc(msg, wparam, lparam)
            }
        }
    }
}

struct DummyView;

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
    fn new(parent: HWND) -> Result<RenderedView> {
        let window = Window::new();
        window.create(
            parent,
            WS_POPUP | WS_CLIPCHILDREN,
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        );
        unsafe {
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite_factory: IDWriteFactory1 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let device = create_device()?;
            let target = create_render_target(&factory, &device)?;
            let swapchain = create_swapchain(&device, 10, 10)?;
            let dpi = get_dpi_for_window(window.hwnd());
            target.SetDpi(dpi, dpi);
            create_swapchain_bitmap(&swapchain, &target, dpi)?;
            let dcomptarget = setup_direct_composition(&device, window.hwnd(), &swapchain)?;
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

// TODO: make this generic - complete same as CandidateList (almost)
impl View for DummyView {
    fn window(&self) -> Option<&Window> {
        None
    }
    fn calculate_client_rect(&self, _model: &NotificationModel) -> Result<RenderedMetrics> {
        Ok(RenderedMetrics {
            width: 0.0,
            height: 0.0,
            hw_width: 0.0,
            hw_height: 0.0,
        })
    }
    fn on_paint(&self, _model: &NotificationModel) -> Result<()> {
        Ok(())
    }
}

impl View for RenderedView {
    fn window(&self) -> Option<&Window> {
        Some(&self.window)
    }
    fn calculate_client_rect(&self, model: &NotificationModel) -> Result<RenderedMetrics> {
        let scale = get_scale_for_window(self.window.hwnd());
        let interop = unsafe { self.dwrite_factory.GetGdiInterop()? };
        let font_family = dwrite_family_from_gdi_name(&interop, &model.font_family)
            .unwrap_or_else(|_| model.font_family.clone());
        let text_format = unsafe {
            self.dwrite_factory.CreateTextFormat(
                &font_family,
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                model.font_size * scale,
                w!("zh-TW"),
            )?
        };
        let text_layout = unsafe {
            self.dwrite_factory
                .CreateTextLayout(&model.text, &text_format, f32::MAX, f32::MAX)?
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

    fn on_paint(&self, model: &NotificationModel) -> Result<()> {
        if model.text.is_empty() {
            return Ok(());
        }
        let scale = get_scale_for_window(self.window.hwnd());
        let interop = unsafe { self.dwrite_factory.GetGdiInterop()? };
        let font_family = dwrite_family_from_gdi_name(&interop, &model.font_family)
            .unwrap_or_else(|_| model.font_family.clone());
        let text_format = unsafe {
            self.dwrite_factory.CreateTextFormat(
                &font_family,
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                model.font_size * scale,
                w!("zh-TW"),
            )?
        };

        let RenderedMetrics {
            width,
            height,
            hw_width,
            hw_height,
        } = self.calculate_client_rect(model)?;
        unsafe {
            self.target.SetTarget(None);
            self.swapchain.ResizeBuffers(
                0,
                hw_width as u32,
                hw_height as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
        }
        let dpi = get_dpi_for_window(self.window.hwnd());
        create_swapchain_bitmap(&self.swapchain, &self.target, dpi)?;

        // Begin drawing
        let dc = &self.target;
        let text_color = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let bg_color = D2D1_COLOR_F {
            r: 0.988,
            g: 0.984,
            b: 0.855,
            a: 1.0,
        };
        unsafe {
            dc.BeginDraw();

            draw_message_box(dc, 0.0, 0.0, width, height, bg_color)?;

            let margin = 10.0;
            let text_rect = D2D_RECT_F {
                left: margin,
                top: margin,
                right: margin + width,
                bottom: margin + height,
            };
            let brush = dc.CreateSolidColorBrush(&text_color, None)?;
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
            self.swapchain
                .Present(1, DXGI_PRESENT(0))
                .ok()
                .context("unable to present buffer")?;
        }
        Ok(())
    }
}

impl Notification {
    pub(crate) fn new(parent: HWND, thread_mgr: ITfThreadMgr) -> Result<ComObject<Notification>> {
        let ui_manager: ITfUIElementMgr = thread_mgr.cast()?;
        let candidate_list = Notification {
            thread_mgr,
            element_id: Cell::new(0),
            parent,
            model: RefCell::new(NotificationModel::default()),
            view: RefCell::new(Box::new(DummyView)),
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
        if let Some(window) = self.view.borrow().window() {
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
        *self.model.borrow_mut() = model;
        if let Err(error) = self.update_ui_element() {
            error!("Failed to update UI element: {error}");
        }
    }
    pub(crate) fn set_position(&self, x: i32, y: i32) {
        if let Some(window) = self.view.borrow().window() {
            window.set_position(x, y);
        }
    }
    pub(crate) fn show(&self) {
        if let Some(window) = self.view.borrow().window() {
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
            let view = RenderedView::new(self.parent).map_err(|_| E_FAIL)?;
            self.view.replace(Box::new(view));
            let iwndproc: InterfaceRef<IWndProc> = self.as_interface_ref();
            if let Some(window) = self.view.borrow().window() {
                // Register the window handle for message routing
                Window::register_hwnd(window.hwnd(), iwndproc.to_owned());
            }
            self.show();
        } else {
            self.view.replace(Box::new(DummyView));
        }
        Ok(())
    }

    fn IsShown(&self) -> WindowsResult<BOOL> {
        Ok(self.view.borrow().window().is_some().into())
    }
}
