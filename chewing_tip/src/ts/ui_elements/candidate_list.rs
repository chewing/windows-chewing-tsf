// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::{Cell, RefCell};

use anyhow::{Context, Result};
use log::{debug, error};
use windows::Win32::{
    Foundation::{E_FAIL, E_INVALIDARG, HWND, LPARAM, LRESULT, POINT, TRUE, WPARAM},
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
        Gdi::{
            BeginPaint, EndPaint, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO,
            MonitorFromPoint, PAINTSTRUCT,
        },
    },
    UI::{
        Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_DOWN, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP},
        TextServices::{
            ITfCandidateListUIElement, ITfCandidateListUIElement_Impl, ITfDocumentMgr,
            ITfThreadMgr, ITfUIElement, ITfUIElement_Impl, ITfUIElementMgr, TF_CLUIE_COUNT,
            TF_CLUIE_CURRENTPAGE, TF_CLUIE_DOCUMENTMGR, TF_CLUIE_PAGEINDEX, TF_CLUIE_SELECTION,
            TF_CLUIE_STRING,
        },
        WindowsAndMessaging::{
            WINDOWPOS, WM_PAINT, WM_WINDOWPOSCHANGING, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW,
            WS_EX_TOPMOST, WS_POPUP,
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

#[implement(ITfUIElement, ITfCandidateListUIElement, IWndProc)]
pub(crate) struct CandidateList {
    thread_mgr: ITfThreadMgr,
    element_id: Cell<u32>,
    parent: HWND,
    model: RefCell<Model>,
    view: RefCell<Box<dyn View>>,
}

#[derive(Default)]
pub(crate) struct Model {
    pub(crate) items: Vec<String>,
    pub(crate) selkeys: Vec<u16>,
    pub(crate) font_family: HSTRING,
    pub(crate) font_size: f32,
    pub(crate) cand_per_row: u32,
    pub(crate) use_cursor: bool,
    pub(crate) current_sel: usize,
    pub(crate) selkey_color: D2D1_COLOR_F,
    pub(crate) fg_color: D2D1_COLOR_F,
    pub(crate) bg_color: D2D1_COLOR_F,
    pub(crate) highlight_fg_color: D2D1_COLOR_F,
    pub(crate) highlight_bg_color: D2D1_COLOR_F,
}

pub(crate) enum FilterKeyResult {
    Handled,
    HandledCommit,
    NotHandled,
}

trait View {
    fn window(&self) -> Option<&Window>;
    fn calculate_client_rect(&self, model: &Model) -> Result<RenderedMetrics>;
    fn on_paint(&self, model: &Model) -> Result<()>;
}

impl IWndProc_Impl for CandidateList_Impl {
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
                    debug!("calculat client rect wndproc: {metrics:?}");
                    let pos = lparam.0 as *mut WINDOWPOS;
                    unsafe {
                        (*pos).cx = metrics.hw_width as i32;
                        (*pos).cy = metrics.hw_height as i32;
                    }
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

#[derive(Debug)]
struct RenderedMetrics {
    width: f32,
    height: f32,
    hw_width: f32,
    hw_height: f32,
    selkey_width: f32,
    text_width: f32,
    item_height: f32,
}

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

const ROW_SPACING: f32 = 4.0;
const COL_SPACING: f32 = 8.0;

impl View for DummyView {
    fn window(&self) -> Option<&Window> {
        None
    }
    fn calculate_client_rect(&self, _model: &Model) -> Result<RenderedMetrics> {
        Ok(RenderedMetrics {
            width: 0.0,
            height: 0.0,
            hw_width: 0.0,
            hw_height: 0.0,
            selkey_width: 0.0,
            text_width: 0.0,
            item_height: 0.0,
        })
    }
    fn on_paint(&self, _model: &Model) -> Result<()> {
        Ok(())
    }
}

impl View for RenderedView {
    fn window(&self) -> Option<&Window> {
        Some(&self.window)
    }
    fn calculate_client_rect(&self, model: &Model) -> Result<RenderedMetrics> {
        // Create a text format for the candidate list
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
        // Recalculate the size of the window
        let margin: f32 = 10.0;
        let mut selkey_width: f32 = 0.0;
        let mut text_width: f32 = 0.0;
        let mut item_height: f32 = 0.0;
        let mut selkey = "?.".to_string().encode_utf16().collect::<Vec<_>>();
        for (key, text) in model.selkeys.iter().zip(model.items.iter()) {
            let mut selkey_metrics = DWRITE_TEXT_METRICS::default();
            let mut item_metrics = DWRITE_TEXT_METRICS::default();

            selkey[0] = *key;
            unsafe {
                self.dwrite_factory
                    .CreateTextLayout(&selkey, &text_format, f32::MAX, f32::MAX)?
                    .GetMetrics(&mut selkey_metrics)?;

                self.dwrite_factory
                    .CreateTextLayout(&HSTRING::from(text), &text_format, f32::MAX, f32::MAX)?
                    .GetMetrics(&mut item_metrics)?;
            }

            selkey_width = selkey_width.max(selkey_metrics.widthIncludingTrailingWhitespace);
            text_width = text_width.max(item_metrics.widthIncludingTrailingWhitespace);
            item_height = item_height
                .max(item_metrics.height)
                .max(selkey_metrics.height);
        }
        selkey_width += 1.0;

        let items_len = model.items.len() as f32;
        let cand_per_row = items_len
            .min(model.cand_per_row as f32)
            .clamp(1.0, f32::MAX);
        let rows = (items_len / cand_per_row).clamp(1.0, f32::MAX).ceil();
        let width = cand_per_row * (selkey_width + text_width)
            + (cand_per_row - 1.0) * COL_SPACING
            + 2.0 * margin;
        let height = rows * item_height + (rows - 1.0) * ROW_SPACING + 2.0 * margin;

        // Convert to HW pixels
        let hw_width = (width * scale + 25.0).ceil();
        let hw_height = (height * scale + 25.0).ceil();
        Ok(RenderedMetrics {
            width,
            height,
            hw_width,
            hw_height,
            selkey_width,
            text_width,
            item_height,
        })
    }
    fn on_paint(&self, model: &Model) -> Result<()> {
        if model.items.is_empty() {
            return Ok(());
        }
        let scale = get_scale_for_window(self.window.hwnd());
        let interop = unsafe { self.dwrite_factory.GetGdiInterop()? };
        let font_family = dwrite_family_from_gdi_name(&interop, &model.font_family)
            .unwrap_or_else(|_| model.font_family.clone());
        // Create a text format for the candidate list
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

        let rm = self.calculate_client_rect(model)?;
        debug!("calculat client rect on_paint: {rm:?}");
        let RenderedMetrics {
            width,
            height,
            hw_width,
            hw_height,
            selkey_width,
            text_width,
            item_height,
        } = rm;
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
        unsafe {
            dc.BeginDraw();

            draw_message_box(dc, 0.0, 0.0, width, height, model.bg_color)?;

            let mut col = 0;
            let margin = 10.0;
            let mut x = margin;
            let mut y = margin;

            for i in 0..model.items.len() {
                let mut text_rect = D2D_RECT_F {
                    left: x,
                    top: y,
                    right: x + selkey_width + text_width,
                    bottom: y + item_height,
                };
                let mut selkey = "?.".to_string().encode_utf16().collect::<Vec<_>>();
                selkey[0] = model.selkeys.get(i.clamp(0, 9)).cloned().unwrap_or(0x3F);

                let selkey_brush = dc.CreateSolidColorBrush(&model.selkey_color, None)?;
                dc.DrawText(
                    &selkey,
                    &text_format,
                    &text_rect,
                    &selkey_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );

                text_rect.left += selkey_width;
                text_rect.right = text_rect.left + text_width;

                if let Some(item) = model.items.get(i) {
                    let text_brush = dc.CreateSolidColorBrush(&model.fg_color, None)?;
                    let highlight_brush =
                        dc.CreateSolidColorBrush(&model.highlight_bg_color, None)?;
                    let selected_text_brush =
                        dc.CreateSolidColorBrush(&model.highlight_fg_color, None)?;
                    let text = HSTRING::from(item);

                    if model.use_cursor && i == model.current_sel {
                        dc.FillRectangle(&text_rect, &highlight_brush);
                        dc.DrawText(
                            &text,
                            &text_format,
                            &text_rect,
                            &selected_text_brush,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
                    } else {
                        dc.DrawText(
                            &text,
                            &text_format,
                            &text_rect,
                            &text_brush,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
                    }
                }

                col += 1;
                if col >= model.cand_per_row {
                    col = 0;
                    x = margin;
                    y += item_height + ROW_SPACING;
                } else {
                    x += selkey_width + text_width + COL_SPACING;
                }
            }

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

impl CandidateList {
    pub(crate) fn new(parent: HWND, thread_mgr: ITfThreadMgr) -> Result<ComObject<CandidateList>> {
        let ui_manager: ITfUIElementMgr = thread_mgr.cast()?;
        let candidate_list = CandidateList {
            thread_mgr,
            element_id: Cell::new(0),
            parent,
            model: RefCell::new(Model::default()),
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
    pub(crate) fn set_model(&self, model: Model) {
        *self.model.borrow_mut() = model;
        if let Err(error) = self.update_ui_element() {
            error!("Failed to update UI element: {error}");
        }
    }
    pub(crate) fn filter_key_event(&self, key_code: u16) -> FilterKeyResult {
        let mut res = FilterKeyResult::NotHandled;
        {
            let mut model = self.model.borrow_mut();
            let old_sel = model.current_sel;
            let cand_per_row = model.cand_per_row as usize;
            match VIRTUAL_KEY(key_code) {
                VK_UP => {
                    if model.current_sel >= cand_per_row {
                        model.current_sel -= cand_per_row;
                    }
                }
                VK_DOWN => {
                    if model.current_sel + cand_per_row < model.items.len() {
                        model.current_sel += cand_per_row;
                    }
                }
                VK_LEFT => {
                    if model.current_sel >= 1 {
                        model.current_sel -= 1;
                    }
                }
                VK_RIGHT => {
                    if model.current_sel < model.items.len() - 1 {
                        model.current_sel += 1;
                    }
                }
                VK_RETURN => {
                    res = FilterKeyResult::HandledCommit;
                }
                _ => res = FilterKeyResult::NotHandled,
            }

            if model.current_sel != old_sel {
                res = FilterKeyResult::Handled;
            }
        }
        if let Err(error) = self.update_ui_element() {
            error!("Failed to update UI element: {error}");
        }
        res
    }
    pub(crate) fn current_sel(&self) -> usize {
        self.model.borrow().current_sel
    }
    pub(crate) fn set_position(&self, mut x: i32, mut y: i32) {
        let view = self.view.borrow();
        if let Some(window) = view.window() {
            let hmonitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
            let mut monitor_info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let has_monitor_info = unsafe { GetMonitorInfoW(hmonitor, &mut monitor_info) };
            if has_monitor_info.as_bool() {
                if let Ok(size) = view.calculate_client_rect(&self.model.borrow()) {
                    // constraint window to screen
                    debug!("calculate client rect set_pos: {size:?}");
                    x = x.min(monitor_info.rcMonitor.right - size.hw_width as i32 - 10);
                    y = y.min(monitor_info.rcMonitor.bottom - size.hw_height as i32 - 10);
                }
            }
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

impl ITfUIElement_Impl for CandidateList_Impl {
    fn GetDescription(&self) -> WindowsResult<BSTR> {
        Ok(BSTR::from("Candidate List"))
    }

    fn GetGUID(&self) -> WindowsResult<GUID> {
        Ok(GUID::from_u128(0x4b7f55c3_2ae5_4077_a1c0_d17c5cb3c88a))
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

impl ITfCandidateListUIElement_Impl for CandidateList_Impl {
    fn GetUpdatedFlags(&self) -> WindowsResult<u32> {
        Ok(TF_CLUIE_DOCUMENTMGR
            | TF_CLUIE_COUNT
            | TF_CLUIE_SELECTION
            | TF_CLUIE_STRING
            | TF_CLUIE_PAGEINDEX
            | TF_CLUIE_CURRENTPAGE)
    }

    fn GetDocumentMgr(&self) -> WindowsResult<ITfDocumentMgr> {
        unsafe { self.thread_mgr.GetFocus() }
    }

    fn GetCount(&self) -> WindowsResult<u32> {
        let model = self.model.borrow();
        Ok(model.items.len() as u32)
    }

    fn GetSelection(&self) -> WindowsResult<u32> {
        let model = self.model.borrow();
        Ok(model.current_sel as u32)
    }

    fn GetString(&self, uindex: u32) -> WindowsResult<BSTR> {
        let model = self.model.borrow();
        if uindex as usize >= model.items.len() {
            return Err(E_INVALIDARG.into());
        }
        Ok(BSTR::from(model.items[uindex as usize].clone()))
    }

    fn GetPageIndex(
        &self,
        _pindex: *mut u32,
        _usize: u32,
        pupagecnt: *mut u32,
    ) -> WindowsResult<()> {
        unsafe {
            *pupagecnt = 1; // Assuming single page for simplicity
        }
        Ok(())
    }

    fn SetPageIndex(&self, _pindex: *const u32, _upagecnt: u32) -> WindowsResult<()> {
        Ok(())
    }

    fn GetCurrentPage(&self) -> WindowsResult<u32> {
        Ok(0) // Assuming single page for simplicity
    }
}
