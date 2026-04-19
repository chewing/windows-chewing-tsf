// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{cell::RefCell, ops::Div, rc::Rc};

use exn::{Result, ResultExt};
use log::debug;
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
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
    UI::WindowsAndMessaging::{
        CS_IME, GWLP_USERDATA, GetWindowLongPtrW, HWND_DESKTOP, IDC_ARROW, LoadCursorW,
        RegisterClassExW, WINDOWPOS, WM_PAINT, WM_WINDOWPOSCHANGING, WNDCLASSEXW, WS_CLIPCHILDREN,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
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

#[derive(Debug)]
pub(crate) struct CandidateList {
    model: RefCell<CandidateListModel>,
    view: RefCell<RenderedView>,
}

#[derive(Default, Debug)]
pub(crate) struct CandidateListModel {
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

trait View {
    fn window(&self) -> &Window;
    fn calculate_client_rect(&self, model: &CandidateListModel)
    -> Result<RenderedMetrics, UiError>;
    fn on_paint(&self, model: &CandidateListModel) -> Result<(), UiError>;
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let get_this = || unsafe {
        let this_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const CandidateList;
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
                debug!("calculat client rect wndproc: {metrics:?}");
                let pos = lparam.0 as *mut WINDOWPOS;
                unsafe {
                    (*pos).cx = metrics.hw_width as i32;
                    (*pos).cy = metrics.hw_height as i32;
                }
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
    fn new(user_data: *const CandidateList) -> Result<RenderedView, UiError> {
        let err = || UiError(format!("failed to create new RenderedView"));

        let window = Window::new();
        window.create(
            HWND_DESKTOP,
            w!("ChewingCandidateListWindow"),
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

const ROW_SPACING: f32 = 4.0;
const COL_SPACING: f32 = 8.0;

impl View for RenderedView {
    fn window(&self) -> &Window {
        &self.window
    }
    fn calculate_client_rect(
        &self,
        model: &CandidateListModel,
    ) -> Result<RenderedMetrics, UiError> {
        let err = || UiError(format!("failed to calculate client area"));

        // Create a text format for the candidate list
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
        let page_number_format = unsafe {
            self.dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size.div(1.5).clamp(0.0, f32::MAX),
                    w!("zh-TW"),
                )
                .or_raise(err)?
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
                    .CreateTextLayout(&selkey, &text_format, f32::MAX, f32::MAX)
                    .or_raise(err)?
                    .GetMetrics(&mut selkey_metrics)
                    .or_raise(err)?;

                self.dwrite_factory
                    .CreateTextLayout(&HSTRING::from(text), &text_format, f32::MAX, f32::MAX)
                    .or_raise(err)?
                    .GetMetrics(&mut item_metrics)
                    .or_raise(err)?;
            }

            selkey_width = selkey_width.max(selkey_metrics.widthIncludingTrailingWhitespace);
            text_width = text_width.max(item_metrics.widthIncludingTrailingWhitespace);
            item_height = item_height
                .max(item_metrics.height)
                .max(selkey_metrics.height);
        }
        selkey_width += 1.0;

        let page_number = HSTRING::from(format!("{} / {}", model.current_page, model.total_page));

        // Calculate the size of the page_numer box
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe {
            self.dwrite_factory
                .CreateTextLayout(&page_number, &page_number_format, f32::MAX, f32::MAX)
                .or_raise(err)?
                .GetMetrics(&mut metrics)
                .or_raise(err)?;
        }

        let items_len = model.items.len() as f32;
        let cand_per_row = items_len
            .min(model.cand_per_row as f32)
            .clamp(1.0, f32::MAX);
        let rows = (items_len / cand_per_row).clamp(1.0, f32::MAX).ceil();
        let width = cand_per_row * (selkey_width + text_width)
            + (cand_per_row - 1.0) * COL_SPACING
            + 2.0 * margin;
        let height = rows * item_height + (rows - 1.0) * ROW_SPACING + 2.0 * margin;
        let window_width = width.max(metrics.width + margin);
        let window_height = height + metrics.height + margin + 2.0;

        // Convert to HW pixels
        let hw_width = (window_width * scale + 25.0).ceil();
        let hw_height = (window_height * scale + 25.0).ceil();
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
    fn on_paint(&self, model: &CandidateListModel) -> Result<(), UiError> {
        let err = || UiError("failed to paint UI".to_string());

        if model.items.is_empty() {
            return Ok(());
        }
        // Create a text format for the candidate list
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
        let page_number_format = unsafe {
            self.dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size.div(1.5).clamp(0.0, f32::MAX),
                    w!("zh-TW"),
                )
                .or_raise(err)?
        };

        let rm = self.calculate_client_rect(model).or_raise(err)?;
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

            let mut col = 0;
            let margin = 10.0;
            let mut x = margin;
            let mut y = margin;

            let page_number =
                HSTRING::from(format!("{} / {}", model.current_page, model.total_page));

            // Calculate the size of the page_numer box
            let mut metrics = DWRITE_TEXT_METRICS::default();
            self.dwrite_factory
                .CreateTextLayout(&page_number, &page_number_format, f32::MAX, f32::MAX)
                .or_raise(err)?
                .GetMetrics(&mut metrics)
                .or_raise(err)?;

            // Draw the background of the main message box
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
            // Draw the background of the page_number box
            draw_message_box(
                dc,
                width - metrics.width - margin,
                height + 2.0,
                metrics.width + margin,
                metrics.height + margin,
                model.bg_color,
                model.border_color,
            )
            .or_raise(err)?;

            let selkey_brush = dc
                .CreateSolidColorBrush(&model.selkey_color, None)
                .or_raise(err)?;
            let text_brush = dc
                .CreateSolidColorBrush(&model.fg_color, None)
                .or_raise(err)?;
            let highlight_brush = dc
                .CreateSolidColorBrush(&model.highlight_bg_color, None)
                .or_raise(err)?;
            let selected_text_brush = dc
                .CreateSolidColorBrush(&model.highlight_fg_color, None)
                .or_raise(err)?;

            dc.DrawText(
                &page_number,
                &page_number_format,
                &D2D_RECT_F {
                    left: width - metrics.width - margin / 2.0,
                    top: height + 2.0 + margin / 2.0,
                    right: f32::MAX,
                    bottom: f32::MAX,
                },
                &text_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            for i in 0..model.items.len() {
                let mut text_rect = D2D_RECT_F {
                    left: x,
                    top: y,
                    right: x + selkey_width + text_width,
                    bottom: y + item_height,
                };
                let mut selkey = "?.".to_string().encode_utf16().collect::<Vec<_>>();
                selkey[0] = model.selkeys.get(i.clamp(0, 9)).cloned().unwrap_or(0x3F);

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
    pub(crate) fn new() -> Result<Rc<CandidateList>, UiError> {
        let err = || UiError("failed to create candidate list window".to_string());
        let mut candidate_list = Rc::new_uninit();
        let user_data = Rc::as_ptr(&candidate_list);
        Rc::get_mut(&mut candidate_list)
            .unwrap()
            .write(CandidateList {
                model: RefCell::new(CandidateListModel::default()),
                view: RefCell::new(RenderedView::new(user_data.cast()).or_raise(err)?),
            });
        // SAFETY: candidate list is unconditionally initialized
        unsafe { Ok(candidate_list.assume_init()) }
    }
    pub(crate) fn set_model(&self, model: CandidateListModel) {
        *self.model.borrow_mut() = model;
    }
    // pub(crate) fn filter_key_event(&self, ksym: Keysym) -> FilterKeyResult {
    //     let mut res = FilterKeyResult::NotHandled;
    //     {
    //         let mut model = self.model.borrow_mut();
    //         let old_sel = model.current_sel;
    //         let uiless_mode = self.view.borrow().window().is_none();
    //         if uiless_mode {
    //             match ksym {
    //                 SYM_DOWN | SYM_RIGHT => {
    //                     model.current_sel = model
    //                         .current_sel
    //                         .saturating_add(1)
    //                         .clamp(0, model.items.len() - 1);
    //                 }
    //                 SYM_UP | SYM_LEFT => {
    //                     model.current_sel = model.current_sel.saturating_sub(1);
    //                 }
    //                 SYM_RETURN => {
    //                     res = FilterKeyResult::HandledCommit;
    //                 }
    //                 _ => res = FilterKeyResult::NotHandled,
    //             }
    //         } else {
    //             let cand_per_row = model.cand_per_row as usize;
    //             match ksym {
    //                 SYM_UP => {
    //                     if model.current_sel >= cand_per_row {
    //                         model.current_sel -= cand_per_row;
    //                     }
    //                 }
    //                 SYM_DOWN => {
    //                     if model.current_sel + cand_per_row < model.items.len() {
    //                         model.current_sel += cand_per_row;
    //                     }
    //                 }
    //                 SYM_LEFT => {
    //                     if cand_per_row > 1 {
    //                         model.current_sel = model.current_sel.saturating_sub(1);
    //                     }
    //                 }
    //                 SYM_RIGHT => {
    //                     if cand_per_row > 1 {
    //                         model.current_sel = model
    //                             .current_sel
    //                             .saturating_add(1)
    //                             .clamp(0, model.items.len() - 1);
    //                     }
    //                 }
    //                 SYM_RETURN => {
    //                     res = FilterKeyResult::HandledCommit;
    //                 }
    //                 _ => res = FilterKeyResult::NotHandled,
    //             }
    //         }

    //         if model.current_sel != old_sel {
    //             res = FilterKeyResult::Handled;
    //         }
    //     }
    //     if let Err(error) = self.update_ui_element() {
    //         error!("Failed to update UI element: {error}");
    //     }
    //     res
    // }
    pub(crate) fn current_sel(&self) -> usize {
        self.model.borrow().current_sel
    }
    pub(crate) fn current_phrase(&self) -> String {
        let sel = self.current_sel();
        self.model.borrow().items[sel].clone()
    }
    pub(crate) fn set_position(&self, mut x: i32, mut y: i32) {
        let view = self.view.borrow();
        let window = view.window();
        let hmonitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let has_monitor_info = unsafe { GetMonitorInfoW(hmonitor, &mut monitor_info) };
        if has_monitor_info.as_bool()
            && let Ok(size) = view.calculate_client_rect(&self.model.borrow())
        {
            // constraint window to screen
            debug!("calculate client rect set_pos: {size:?}");
            x = x.min(monitor_info.rcMonitor.right - size.hw_width as i32 - 10);
            y = y.min(monitor_info.rcMonitor.bottom - size.hw_height as i32 - 10);
        }
        window.set_position(x, y);
    }
    pub(crate) fn show(&self) {
        let view = self.view.borrow();
        let window = view.window();
        window.show();
        window.refresh();
    }
    pub(crate) fn hide(&self) {
        let view = self.view.borrow();
        let window = view.window();
        window.hide();
    }
}
