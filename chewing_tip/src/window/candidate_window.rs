// SPDX-License-Identifier: GPL-3.0-or-later

use core::f32;
use std::{
    cell::{Cell, RefCell},
    ffi::{c_int, c_uint},
    ops::Deref,
    path::Path,
    rc::Rc,
};

use log::error;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_core::*;

use super::{Window, WndProc};
use crate::gfx::*;

#[derive(Debug)]
pub(crate) struct CandidateWindow {
    items: RefCell<Vec<HSTRING>>,
    sel_keys: RefCell<Vec<u16>>,
    current_sel: Cell<usize>,
    has_result: Cell<bool>,
    cand_per_row: Cell<usize>,
    use_cursor: Cell<bool>,
    selkey_width: Cell<f32>,
    text_width: Cell<f32>,
    item_height: Cell<f32>,
    factory: ID2D1Factory1,
    dwrite_factory: IDWriteFactory1,
    text_format: RefCell<IDWriteTextFormat>,
    dcomptarget: RefCell<Option<IDCompositionTarget>>,
    target: RefCell<Option<ID2D1DeviceContext>>,
    swapchain: RefCell<Option<IDXGISwapChain1>>,
    brush: RefCell<Option<ID2D1SolidColorBrush>>,
    nine_patch_bitmap: NinePatchBitmap,
    window: Window,
}

const ROW_SPACING: u32 = 4;
const COL_SPACING: u32 = 8;

impl CandidateWindow {
    pub(crate) fn new(parent: HWND, image_path: &Path) -> Result<Rc<CandidateWindow>> {
        unsafe {
            let window = Window::new();
            window.create(
                parent,
                (WS_POPUP | WS_CLIPCHILDREN).0,
                (WS_EX_TOOLWINDOW | WS_EX_TOPMOST).0,
            );

            let factory: ID2D1Factory1 = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                Some(&D2D1_FACTORY_OPTIONS::default()),
            )?;

            let dwrite_factory: IDWriteFactory1 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let text_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                16.0,
                w!(""),
            )?;

            let image_path = HSTRING::from(image_path.to_string_lossy().as_ref());
            let nine_patch_bitmap = NinePatchBitmap::new(&image_path)?;

            let candidate_window = Rc::new(CandidateWindow {
                items: RefCell::new(vec![]),
                sel_keys: RefCell::new(vec![]),
                current_sel: Cell::new(0),
                has_result: Cell::new(false),
                cand_per_row: Cell::new(10),
                use_cursor: Cell::new(true),
                selkey_width: Cell::new(0.0),
                text_width: Cell::new(0.0),
                item_height: Cell::new(0.0),
                factory,
                dwrite_factory,
                text_format: RefCell::new(text_format),
                dcomptarget: None.into(),
                target: None.into(),
                swapchain: None.into(),
                brush: None.into(),
                nine_patch_bitmap,
                window,
            });
            Window::register_hwnd(
                candidate_window.window.hwnd(),
                Rc::clone(&candidate_window) as Rc<dyn WndProc>,
            );
            Ok(candidate_window)
        }
    }
    pub(crate) fn recalculate_size(&self) -> Result<()> {
        let margin = self.nine_patch_bitmap.margin().ceil();
        if self.items.borrow().is_empty() {
            // Convert to HW pixels
            let dpi = unsafe { GetDpiForWindow(self.window.hwnd()) } as f32;
            let width = 2.0 * margin * dpi / USER_DEFAULT_SCREEN_DPI as f32;
            let height = 2.0 * margin * dpi / USER_DEFAULT_SCREEN_DPI as f32;

            self.window.resize(width as i32, height as i32);
            self.resize_swap_chain(width as u32, height as u32)?;
        }

        let mut selkey_width = 0.0_f32;
        let mut text_width = 0.0_f32;
        let mut item_height = 0.0_f32;
        let mut selkey = "?. ".to_string().encode_utf16().collect::<Vec<_>>();

        for (key, text) in self
            .sel_keys
            .borrow()
            .iter()
            .zip(self.items.borrow().iter())
        {
            let mut selkey_metrics = DWRITE_TEXT_METRICS::default();
            let mut item_metrics = DWRITE_TEXT_METRICS::default();

            selkey[0] = *key;
            unsafe {
                self.dwrite_factory
                    .CreateTextLayout(
                        &selkey,
                        self.text_format.borrow().deref(),
                        f32::MAX,
                        f32::MAX,
                    )?
                    .GetMetrics(&mut selkey_metrics)?;

                self.dwrite_factory
                    .CreateTextLayout(text, self.text_format.borrow().deref(), f32::MAX, f32::MAX)?
                    .GetMetrics(&mut item_metrics)?;
            }

            selkey_width = selkey_width.max(selkey_metrics.widthIncludingTrailingWhitespace);
            text_width = text_width.max(item_metrics.widthIncludingTrailingWhitespace);
            item_height = item_height
                .max(item_metrics.height)
                .max(selkey_metrics.height);
        }

        let items_len = self.items.borrow().len() as u32;
        self.selkey_width.set(selkey_width);
        self.text_width.set(text_width);
        self.item_height.set(item_height);
        let cand_per_row = items_len.min(self.cand_per_row.get() as u32);
        let rows = items_len.div_ceil(cand_per_row).clamp(1, u32::MAX);
        let width = cand_per_row * (selkey_width + text_width).ceil() as u32
            + (cand_per_row - 1) * COL_SPACING
            + 2 * margin as u32;
        let height = rows * item_height as u32 + (rows - 1) * ROW_SPACING + 2 * margin as u32;

        // Convert to HW pixels
        let dpi = unsafe { GetDpiForWindow(self.window.hwnd()) } as f32;
        let width = width as f32 * dpi / USER_DEFAULT_SCREEN_DPI as f32;
        let height = height as f32 * dpi / USER_DEFAULT_SCREEN_DPI as f32;

        self.window.resize(width as i32, height as i32);
        self.resize_swap_chain(width as u32, height as u32)?;

        Ok(())
    }
    // FIXME: extract
    fn resize_swap_chain(&self, width: u32, height: u32) -> Result<()> {
        let target = self.target.borrow();
        let swapchain = self.swapchain.borrow();

        if let Some(target) = target.as_ref() {
            if let Some(swapchain) = swapchain.as_ref() {
                unsafe { target.SetTarget(None) };

                if let Err(error) = unsafe {
                    swapchain.ResizeBuffers(
                        0,
                        width,
                        height,
                        DXGI_FORMAT_B8G8R8A8_UNORM,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )
                } {
                    error!("unable to resize swapchain: {error}");
                    self.target.take();
                    self.swapchain.take();
                    self.brush.take();
                } else {
                    let dpi = unsafe { GetDpiForWindow(self.window.hwnd()) } as f32;
                    create_swapchain_bitmap(swapchain, target, dpi)?;
                }

                self.on_paint()?;
            }
        }

        Ok(())
    }
    fn create_target(&self) -> Result<()> {
        if self.target.borrow().is_none() {
            let device = create_device()?;
            let target = create_render_target(&self.factory, &device)?;

            let mut rc = RECT::default();
            unsafe { GetClientRect(self.window.hwnd.get(), &mut rc)? };

            let swapchain = create_swapchain(
                &device,
                (rc.right - rc.left) as u32,
                (rc.bottom - rc.top) as u32,
            )?;
            let dpi = unsafe { GetDpiForWindow(self.window.hwnd()) } as f32;
            create_swapchain_bitmap(&swapchain, &target, dpi)?;
            let dcomptarget =
                setup_direct_composition(&device, self.window.hwnd.get(), &swapchain)?;

            self.brush
                .replace(create_brush(&target, create_color(COLOR_INFOTEXT)).ok());
            self.dcomptarget.replace(Some(dcomptarget));
            self.target.replace(Some(target));
            self.swapchain.replace(Some(swapchain));
        }
        Ok(())
    }
    fn on_paint(&self) -> Result<()> {
        self.create_target()?;

        let target = self.target.borrow();
        let swapchain = self.swapchain.borrow();

        if let Some(target) = target.as_ref() {
            unsafe {
                let mut rc = RECT::default();
                GetClientRect(self.window.hwnd.get(), &mut rc)?;
                let dpi = GetDpiForWindow(self.window.hwnd()) as f32;

                target.BeginDraw();
                let rect = D2D_RECT_F {
                    top: 0.0,
                    left: 0.0,
                    // Convert to DIPs
                    right: rc.right as f32 * USER_DEFAULT_SCREEN_DPI as f32 / dpi,
                    bottom: rc.bottom as f32 * USER_DEFAULT_SCREEN_DPI as f32 / dpi,
                };
                self.nine_patch_bitmap.draw_bitmap(target, rect)?;

                let mut col = 0;
                let margin = self.nine_patch_bitmap.margin();
                let mut x = margin;
                let mut y = margin;

                for i in 0..self.items.borrow().len() {
                    self.paint_item(target, i, x, y)?;
                    col += 1;
                    if col >= self.cand_per_row.get() {
                        col = 0;
                        x = margin;
                        y += self.item_height.get() + ROW_SPACING as f32;
                    } else {
                        x += COL_SPACING as f32 + self.selkey_width.get() + self.text_width.get();
                    }
                }
                target.EndDraw(None, None)?;

                if let Some(swapchain) = swapchain.as_ref() {
                    if let Err(error) = swapchain.Present(1, DXGI_PRESENT(0)).ok() {
                        error!("unable to present buffer: {error}");
                        _ = target;
                        _ = swapchain;
                        self.target.take();
                        self.swapchain.take();
                        self.brush.take();
                    }
                }
            }
        }

        Ok(())
    }

    fn paint_item(&self, dc: &ID2D1DeviceContext, i: usize, left: f32, top: f32) -> Result<()> {
        let mut text_rect = D2D_RECT_F {
            left,
            top,
            right: left + self.selkey_width.get(),
            bottom: top + self.item_height.get(),
        };

        // FIXME: make the color of strings configurable
        let sel_key_color = create_color(COLOR_HOTLIGHT);
        let text_color = create_color(COLOR_WINDOWTEXT);
        let selected_text_color = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let selkey = "?. ".to_string();
        let mut selkey = selkey.encode_utf16().collect::<Vec<_>>();
        selkey[0] = *self
            .sel_keys
            .borrow()
            .get(i.clamp(0, 9))
            .expect("sel_keys should have 10 elements");

        unsafe {
            let selkey_brush = dc.CreateSolidColorBrush(&sel_key_color, None)?;

            dc.DrawText(
                &selkey,
                self.text_format.borrow().deref(),
                &text_rect,
                &selkey_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }

        text_rect.left += self.selkey_width.get();
        text_rect.right = text_rect.left + self.text_width.get();

        let items = self.items.borrow();
        if let Some(item) = items.get(i.clamp(0, 9)) {
            unsafe {
                let text_brush = dc.CreateSolidColorBrush(&text_color, None)?;
                let selected_text_brush = dc.CreateSolidColorBrush(&selected_text_color, None)?;

                if self.use_cursor.get() && i == self.current_sel.get() {
                    dc.FillRectangle(&text_rect, &text_brush);
                    dc.DrawText(
                        item,
                        self.text_format.borrow().deref(),
                        &text_rect,
                        &selected_text_brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                } else {
                    dc.DrawText(
                        item,
                        self.text_format.borrow().deref(),
                        &text_rect,
                        &text_brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
            }
        }

        Ok(())
    }
    pub(crate) fn set_font_size(&self, font_size: i32) {
        if let Ok(text_format) = unsafe {
            self.dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size as f32,
                w!(""),
            )
        } {
            self.text_format.replace(text_format);
        }
    }
    pub(crate) fn add(&self, item: HSTRING, sel_key: u16) {
        self.items.borrow_mut().push(item);
        self.sel_keys.borrow_mut().push(sel_key);
    }
    pub(crate) fn current_sel_key(&self) -> u16 {
        *self
            .sel_keys
            .borrow()
            .get(self.current_sel.get())
            .unwrap_or(&0)
    }
    pub(crate) fn clear(&self) {
        self.items.borrow_mut().clear();
        self.sel_keys.borrow_mut().clear();
        self.current_sel.set(0);
        self.has_result.set(false);
    }
    pub(crate) fn set_cand_per_row(&self, n: c_int) {
        if n as usize != self.cand_per_row.get() {
            self.cand_per_row.set(n as usize);
        }
    }
    pub(crate) fn set_use_cursor(&self, r#use: bool) {
        self.use_cursor.set(r#use);
        if self.window.is_visible() {
            self.window.refresh();
        }
    }
    pub(crate) fn filter_key_event(&self, key_code: u16) -> bool {
        let mut current_sel = self.current_sel.get();
        let old_sel = self.current_sel.get();
        let cand_per_row = self.cand_per_row.get();
        match VIRTUAL_KEY(key_code) {
            VK_UP => {
                if current_sel >= cand_per_row {
                    current_sel -= cand_per_row;
                }
            }
            VK_DOWN => {
                if current_sel + cand_per_row < self.items.borrow().len() {
                    current_sel += cand_per_row;
                }
            }
            VK_LEFT => {
                if current_sel >= 1 {
                    current_sel -= 1;
                }
            }
            VK_RIGHT => {
                if current_sel < self.items.borrow().len() - 1 {
                    current_sel += 1;
                }
            }
            VK_RETURN => {
                self.has_result.set(true);
                self.current_sel.set(current_sel);
                return true;
            }
            _ => return false,
        }

        self.current_sel.set(current_sel);

        if current_sel != old_sel {
            self.window.refresh();
            return true;
        }
        false
    }
    pub(crate) fn has_result(&self) -> bool {
        self.has_result.get()
    }
    pub(crate) fn show(&self) {
        self.window.show()
    }
    pub(crate) fn refresh(&self) {
        self.window.refresh()
    }
    pub(crate) fn r#move(&self, x: c_int, y: c_int) {
        self.window.r#move(x, y)
    }
}

impl WndProc for CandidateWindow {
    fn wnd_proc(&self, msg: c_uint, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_PAINT => {
                    let mut ps = PAINTSTRUCT::default();
                    BeginPaint(self.window.hwnd(), &mut ps);
                    let _ = self.on_paint();
                    let _ = EndPaint(self.window.hwnd(), &ps);
                    LRESULT(0)
                }
                WM_NCDESTROY => {
                    self.target.take();
                    self.swapchain.take();
                    self.brush.take();
                    LRESULT(0)
                }
                _ => self.window.wnd_proc(msg, wp, lp),
            }
        }
    }
}
