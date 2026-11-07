// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use anyhow::{Context, Result};
use chewing::input::keysym::{Keysym, SYM_DOWN, SYM_LEFT, SYM_RETURN, SYM_RIGHT, SYM_UP};
use log::error;
use scoped_error::expect_error;
use taffy::{
    geometry::{Rect as TaffyRect, Size as TaffySize},
    prelude::{AlignItems, Display, FlexDirection, FlexWrap, Style, auto, length, zero},
    tree::NodeId,
};
use windows::Win32::{
    Foundation::{E_FAIL, E_INVALIDARG, HINSTANCE, HWND, LPARAM, LRESULT, POINT, TRUE, WPARAM},
    Graphics::{
        Direct2D::{
            Common::D2D1_COLOR_F, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory,
            ID2D1DeviceContext, ID2D1Factory1,
        },
        DirectComposition::IDCompositionTarget,
        DirectWrite::{DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory1},
        Dxgi::{
            Common::DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_PRESENT, DXGI_SWAP_CHAIN_FLAG, IDXGISwapChain1,
        },
        Gdi::{BeginPaint, EndPaint, PAINTSTRUCT},
    },
    UI::{
        TextServices::{
            ITfCandidateListUIElement, ITfCandidateListUIElement_Impl, ITfDocumentMgr,
            ITfThreadMgr, ITfUIElement, ITfUIElement_Impl, ITfUIElementMgr, TF_CLUIE_COUNT,
            TF_CLUIE_CURRENTPAGE, TF_CLUIE_DOCUMENTMGR, TF_CLUIE_PAGEINDEX, TF_CLUIE_SELECTION,
            TF_CLUIE_STRING,
        },
        WindowsAndMessaging::{
            CS_IME, GWLP_USERDATA, GetWindowLongPtrW, IDC_ARROW, LoadCursorW, RegisterClassExW,
            SWP_NOMOVE, WINDOWPOS, WM_NCDESTROY, WM_PAINT, WM_WINDOWPOSCHANGING, WNDCLASSEXW,
            WS_CLIPCHILDREN, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            WS_EX_TRANSPARENT, WS_POPUP,
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
        render::{self, FrameGeometry, Panel, TextMeasurer, TextStyle, UiTree},
        window::Window,
    },
};

#[implement(ITfUIElement, ITfCandidateListUIElement)]
pub(crate) struct CandidateList {
    thread_mgr: ITfThreadMgr,
    element_id: Cell<u32>,
    parent: HWND,
    inner: Rc<CandidateListInner>,
}

struct CandidateListInner {
    model: RefCell<Model>,
    view: RefCell<View>,
}

#[derive(Default)]
pub(crate) struct Model {
    pub(crate) items: Vec<String>,
    pub(crate) selkeys: Vec<u16>,
    pub(crate) total_page: u32,
    pub(crate) current_page: u32,
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
    pub(crate) border_color: D2D1_COLOR_F,
}

pub(crate) enum FilterKeyResult {
    Handled,
    HandledCommit,
    NotHandled,
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let this = unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const CandidateListInner;
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
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const CandidateListInner;
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
                if let Ok(Some(scene)) = view.build_scene(&model) {
                    let (x, y) = scene.frame.window_origin(pos.x, pos.y, dpi);
                    let (cx, cy) = scene.frame.window_size(dpi);
                    if cx > 0.0 && cy > 0.0 {
                        pos.cx = cx as i32;
                        pos.cy = cy as i32;
                        if !pos.flags.contains(SWP_NOMOVE) {
                            (pos.x, pos.y) = clamp_point_to_monitor(x, y, pos.cx, pos.cy);
                        }
                    }
                }
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

const ROW_SPACING: f32 = 4.0;
const COL_SPACING: f32 = 8.0;
const MARGIN: f32 = 10.0;
const PAGE_PADDING: f32 = 5.0;
const PAGE_MARGIN_TOP: f32 = 2.0;

struct Scene {
    tree: UiTree,
    measurer: TextMeasurer,
    root: NodeId,
    frame: FrameGeometry,
}

impl View {
    fn rendered(parent: HWND, user_data: Weak<CandidateListInner>) -> Result<View, UiError> {
        expect_error("Failed to create new RenderedView", || {
            let window = Window::new();
            window.create(
                parent,
                w!("ChewingCandidateListWindow"),
                WS_POPUP | WS_CLIPCHILDREN,
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_TRANSPARENT,
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

    fn window(&self) -> Option<&Window> {
        match self {
            View::Dummy => None,
            View::Rendered { window, .. } => Some(window),
        }
    }

    fn build_scene(&self, model: &Model) -> Result<Option<Scene>> {
        let View::Rendered { dwrite_factory, .. } = self else {
            return Ok(None);
        };
        if model.items.is_empty() {
            return Ok(None);
        }

        let mut measurer = TextMeasurer::new(dwrite_factory);
        let font_family = model.font_family.to_string();
        let base = TextStyle::new(font_family.clone(), model.font_size, model.fg_color);
        let selkey_style = TextStyle {
            color: model.selkey_color,
            ..base.clone()
        };
        let page_style = TextStyle::new(&font_family, model.font_size / 1.5, model.fg_color);
        let panel = Panel::message_box(model.bg_color, model.border_color);
        let bleed = panel.bleed();

        // "1." style selkeys, the column padded to the widest one.
        let selkeys: Vec<String> = (0..model.items.len())
            .map(|i| {
                let key = model.selkeys.get(i).copied().unwrap_or(u16::from(b'?'));
                let key = char::from_u32(u32::from(key)).unwrap_or('?');
                format!("{key}.")
            })
            .collect();

        let mut selkey_width: f32 = 0.0;
        for selkey in &selkeys {
            selkey_width = selkey_width.max(measurer.measure_text(selkey, &selkey_style)?.0);
        }
        let selkey_width = selkey_width + 1.0;

        let mut text_width: f32 = 0.0;
        for item in &model.items {
            text_width = text_width.max(measurer.measure_text(item, &base)?.0);
        }

        let item_width = selkey_width + text_width;
        let cand_per_row = model.items.len().min(model.cand_per_row as usize).max(1) as f32;
        let content_width =
            cand_per_row * item_width + (cand_per_row - 1.0) * COL_SPACING + 2.0 * MARGIN;

        let mut tree = UiTree::new();
        let mut item_ids = Vec::with_capacity(model.items.len());

        for (i, item) in model.items.iter().enumerate() {
            let selected = model.use_cursor && i == model.current_sel;
            let color = if selected {
                model.highlight_fg_color
            } else {
                model.fg_color
            };

            let selkey = render::text_node(
                &mut tree,
                Style {
                    size: TaffySize {
                        width: length(selkey_width),
                        height: auto(),
                    },
                    ..Default::default()
                },
                selkeys[i].clone(),
                selkey_style.clone(),
            )?;
            let text = render::text_node(
                &mut tree,
                Style::default(),
                item.clone(),
                TextStyle {
                    color,
                    ..base.clone()
                },
            )?;

            item_ids.push(render::panel_node(
                &mut tree,
                Style {
                    display: Display::Flex,
                    size: TaffySize {
                        width: length(item_width),
                        height: auto(),
                    },
                    ..Default::default()
                },
                &[selkey, text],
                selected.then(|| Panel::highlight(model.highlight_bg_color)),
            )?);
        }

        let content = render::panel_node(
            &mut tree,
            Style {
                display: Display::Flex,
                flex_wrap: FlexWrap::Wrap,
                size: TaffySize {
                    width: length(content_width),
                    height: auto(),
                },
                gap: TaffySize {
                    width: length(COL_SPACING),
                    height: length(ROW_SPACING),
                },
                padding: TaffyRect {
                    left: length(MARGIN),
                    right: length(MARGIN),
                    top: length(MARGIN),
                    bottom: length(MARGIN),
                },
                ..Default::default()
            },
            &item_ids,
            Some(panel.clone()),
        )?;

        let page = render::panel_text(
            &mut tree,
            Style {
                padding: TaffyRect {
                    left: length(PAGE_PADDING),
                    right: length(PAGE_PADDING),
                    top: length(PAGE_PADDING),
                    bottom: length(PAGE_PADDING),
                },
                margin: TaffyRect {
                    left: zero(),
                    right: zero(),
                    top: length(PAGE_MARGIN_TOP),
                    bottom: zero(),
                },
                ..Default::default()
            },
            format!("{} / {}", model.current_page, model.total_page),
            page_style,
            Some(panel),
        )?;

        let root = tree.new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FLEX_END,
                padding: TaffyRect {
                    left: length(bleed),
                    right: length(bleed),
                    top: length(bleed),
                    bottom: length(bleed),
                },
                ..Default::default()
            },
            &[content, page],
        )?;

        let size = render::compute_layout(&mut tree, root, &mut measurer)?;
        let frame = FrameGeometry::new(bleed, size);

        Ok(Some(Scene {
            tree,
            measurer,
            root,
            frame,
        }))
    }

    fn on_paint(&self, model: &Model) -> Result<()> {
        let View::Rendered {
            target,
            swapchain,
            window,
            ..
        } = self
        else {
            return Ok(());
        };
        let Some(mut scene) = self.build_scene(model)? else {
            return Ok(());
        };

        let dpi = get_dpi_for_window(window.hwnd());
        let (hw_width, hw_height) = scene.frame.window_size(dpi);

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
        create_swapchain_bitmap(swapchain, target)?;

        unsafe {
            target.BeginDraw();
            render::paint(
                &scene.tree,
                scene.root,
                target,
                &mut scene.measurer,
                taffy::geometry::Point { x: 0.0, y: 0.0 },
            )?;
            target.EndDraw(None, None)?;

            swapchain
                .Present(1, DXGI_PRESENT(0))
                .ok()
                .context("unable to present buffer")?;
        }

        Ok(())
    }
}

impl CandidateList {
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
            lpszClassName: w!("ChewingCandidateListWindow"),
            ..Default::default()
        };
        unsafe { RegisterClassExW(&wc) };
    }
    pub(crate) fn new(parent: HWND, thread_mgr: ITfThreadMgr) -> Result<ComObject<CandidateList>> {
        let ui_manager: ITfUIElementMgr = thread_mgr.cast()?;
        let inner = Rc::new(CandidateListInner {
            model: RefCell::new(Model::default()),
            view: RefCell::new(View::Dummy),
        });
        let candidate_list = CandidateList {
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
        let Ok(ui_manager) = self.thread_mgr.cast::<ITfUIElementMgr>() else {
            error!("unable to cast thread manager to ITfUIElementMgr");
            return;
        };
        unsafe {
            if let Err(error) = ui_manager.EndUIElement(self.element_id.get()) {
                error!("Error calling EndUIElement on CandidateList: {error}");
            }
        }
    }
    fn set_element_id(&self, id: u32) {
        self.element_id.set(id);
    }
    fn update_ui_element(&self) {
        let Ok(ui_manager) = self.thread_mgr.cast::<ITfUIElementMgr>() else {
            error!("unable to cast thread manager to ITfUIElementMgr");
            return;
        };
        unsafe {
            if let Err(error) = ui_manager.UpdateUIElement(self.element_id.get()) {
                error!("Error calling UpdateUIElement on CandidateList: {error}");
            }
        }
    }
    pub(crate) fn set_model(&self, model: Model) {
        *self.inner.model.borrow_mut() = model;
        self.update_ui_element();
    }
    pub(crate) fn filter_key_event(&self, ksym: Keysym) -> FilterKeyResult {
        let mut res = FilterKeyResult::NotHandled;
        {
            let mut model = self.inner.model.borrow_mut();
            let old_sel = model.current_sel;
            let uiless_mode = self.inner.view.borrow().window().is_none();
            if uiless_mode {
                match ksym {
                    SYM_DOWN | SYM_RIGHT => {
                        model.current_sel = model
                            .current_sel
                            .saturating_add(1)
                            .clamp(0, model.items.len() - 1);
                    }
                    SYM_UP | SYM_LEFT => {
                        model.current_sel = model.current_sel.saturating_sub(1);
                    }
                    SYM_RETURN => {
                        res = FilterKeyResult::HandledCommit;
                    }
                    _ => res = FilterKeyResult::NotHandled,
                }
            } else {
                let cand_per_row = model.cand_per_row as usize;
                match ksym {
                    SYM_UP => {
                        if model.current_sel >= cand_per_row {
                            model.current_sel -= cand_per_row;
                        }
                    }
                    SYM_DOWN => {
                        if model.current_sel + cand_per_row < model.items.len() {
                            model.current_sel += cand_per_row;
                        }
                    }
                    SYM_LEFT => {
                        if cand_per_row > 1 {
                            model.current_sel = model.current_sel.saturating_sub(1);
                        }
                    }
                    SYM_RIGHT => {
                        if cand_per_row > 1 {
                            model.current_sel = model
                                .current_sel
                                .saturating_add(1)
                                .clamp(0, model.items.len() - 1);
                        }
                    }
                    SYM_RETURN => {
                        res = FilterKeyResult::HandledCommit;
                    }
                    _ => res = FilterKeyResult::NotHandled,
                }
            }

            if model.current_sel != old_sel {
                res = FilterKeyResult::Handled;
            }
        }
        self.update_ui_element();
        res
    }
    pub(crate) fn current_sel(&self) -> usize {
        self.inner.model.borrow().current_sel
    }
    pub(crate) fn current_phrase(&self) -> String {
        let sel = self.current_sel();
        self.inner.model.borrow().items[sel].clone()
    }
    pub(crate) fn set_position(&self, x: i32, y: i32) {
        let view = self.inner.view.borrow();
        if let Some(window) = view.window() {
            window.set_position(x, y);
        }
    }
    pub(crate) fn show(&self) {
        if let Some(window) = self.inner.view.borrow().window() {
            window.refresh();
            window.show();
        }
    }
    pub(crate) fn hide(&self) {
        if let Some(window) = self.inner.view.borrow().window() {
            window.hide();
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
        let model = self.inner.model.borrow();
        Ok(model.items.len() as u32)
    }

    fn GetSelection(&self) -> WindowsResult<u32> {
        let model = self.inner.model.borrow();
        Ok(model.current_sel as u32)
    }

    fn GetString(&self, uindex: u32) -> WindowsResult<BSTR> {
        let model = self.inner.model.borrow();
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
