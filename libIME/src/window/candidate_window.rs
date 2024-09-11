use core::f32;
use std::{
    cell::{Cell, RefCell},
    ffi::{c_int, c_uint, c_void},
    ops::Deref,
};

use log::debug;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::{IWindow, IWindow_Impl, IWindow_Vtbl, Window};
use crate::gfx::*;

#[interface("d4eee9d6-60a0-4169-b3b8-d99f66ebe61a")]
unsafe trait ICandidateWindow: IWindow {
    fn set_font_size(&self, font_size: u32);
    fn add(&self, item: PCWSTR, sel_key: u16);
    fn current_sel_key(&self) -> u16;
    fn clear(&self);
    fn set_cand_per_row(&self, n: c_int);
    fn set_use_cursor(&self, r#use: bool);
    fn filter_key_event(&self, key_event: u16) -> bool;
    fn has_result(&self) -> bool;
    fn recalculate_size(&self);
}

#[derive(Debug)]
#[implement(ICandidateWindow, IWindow, ITfUIElement, ITfCandidateListUIElement)]
struct CandidateWindow {
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
    dpi: f32,
    window: ComObject<Window>,
}

const ROW_SPACING: u32 = 4;
const COL_SPACING: u32 = 8;

impl CandidateWindow {
    fn recalculate_size(&self) -> Result<()> {
        let margin = self.nine_patch_bitmap.margin() as u32;
        if self.items.borrow().is_empty() {
            unsafe { self.window.resize(margin as i32 * 2, margin as i32 * 2) };
            self.resize_swap_chain(margin * 2, margin * 2)?;
        }

        let items_len = self.items.borrow().len();
        let mut selkey_width = 0.0;
        let mut text_width = 0.0;
        let mut item_height = 0.0;
        let selkey = "?. ".to_string();
        let mut selkey = selkey.encode_utf16().collect::<Vec<_>>();
        for i in 0..items_len {
            selkey[0] = *self.sel_keys.borrow().get(i).unwrap();

            let mut selkey_metrics = DWRITE_TEXT_METRICS::default();
            let mut item_metrics = DWRITE_TEXT_METRICS::default();
            unsafe {
                let text_layout = self.dwrite_factory.CreateTextLayout(
                    &selkey,
                    self.text_format.borrow().deref(),
                    f32::MAX,
                    f32::MAX,
                )?;
                text_layout.GetMetrics(&mut selkey_metrics)?;

                let items = self.items.borrow();
                let text = items.get(i).unwrap().as_wide();
                let text_layout = self.dwrite_factory.CreateTextLayout(
                    text,
                    self.text_format.borrow().deref(),
                    f32::MAX,
                    f32::MAX,
                )?;
                text_layout.GetMetrics(&mut item_metrics)?;
            }

            selkey_width = f32::max(
                selkey_width,
                selkey_metrics.widthIncludingTrailingWhitespace,
            );
            text_width = f32::max(text_width, item_metrics.widthIncludingTrailingWhitespace);
            item_height = f32::max(item_metrics.height, selkey_metrics.height).max(item_height);
        }

        self.selkey_width.set(selkey_width);
        self.text_width.set(text_width);
        self.item_height.set(item_height);
        let cand_per_row = self.cand_per_row.get() as u32;
        let (width, height) = if items_len <= self.cand_per_row.get() {
            (
                items_len as u32 * (selkey_width + text_width) as u32
                    + COL_SPACING * (items_len - 1) as u32
                    + margin * 2,
                item_height as u32 + margin * 2,
            )
        } else {
            (
                cand_per_row * (selkey_width + text_width) as u32
                    + COL_SPACING * (cand_per_row)
                    + margin * 2,
                (item_height as u32 + ROW_SPACING) * (items_len as u32).div_ceil(cand_per_row)
                    + margin * 2,
            )
        };

        unsafe { self.window.resize(width as i32, height as i32) };
        self.resize_swap_chain(width, height)?;
        debug!("resize_swap_chain {} {}", width, height);

        Ok(())
    }
    // FIXME: extract
    fn resize_swap_chain(&self, width: u32, height: u32) -> Result<()> {
        let target = self.target.borrow();
        let swapchain = self.swapchain.borrow();

        if target.is_some() {
            let target = target.as_ref().unwrap();
            let swapchain = swapchain.as_ref().unwrap();
            unsafe { target.SetTarget(None) };

            if unsafe {
                swapchain
                    .ResizeBuffers(
                        0,
                        width,
                        height,
                        DXGI_FORMAT_B8G8R8A8_UNORM,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )
                    .is_ok()
            } {
                create_swapchain_bitmap(swapchain, target, self.dpi)?;
            } else {
                self.target.take();
                self.swapchain.take();
                self.brush.take();
            }

            self.on_paint()?;
        }

        Ok(())
    }
    fn create_target(&self) -> Result<()> {
        let create_target = self.target.borrow().is_none();
        if create_target {
            let device = create_device()?;
            let target = create_render_target(&self.factory, &device)?;
            unsafe { target.SetDpi(self.dpi, self.dpi) };

            let mut rc = RECT::default();
            unsafe { GetClientRect(self.window.hwnd.get(), &mut rc)? };

            let swapchain = create_swapchain(
                &device,
                (rc.right - rc.left) as u32,
                (rc.bottom - rc.top) as u32,
            )?;
            create_swapchain_bitmap(&swapchain, &target, self.dpi)?;
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
        let target = target.as_ref().unwrap();

        unsafe {
            let mut rc = RECT::default();
            GetClientRect(self.window.hwnd.get(), &mut rc)?;

            target.BeginDraw();
            let rect = D2D_RECT_F {
                top: rc.top as f32,
                left: rc.left as f32,
                right: rc.right as f32,
                bottom: rc.bottom as f32,
            };
            debug!("rect {:?}", rect);
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

            if swapchain
                .as_ref()
                .unwrap()
                .Present(1, DXGI_PRESENT(0))
                .is_err()
            {
                _ = target;
                _ = swapchain;
                self.target.take();
                self.swapchain.take();
                self.brush.take();
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
        selkey[0] = *self.sel_keys.borrow().get(i).unwrap();

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
        let item = items.get(i).unwrap();
        unsafe {
            let text_brush = dc.CreateSolidColorBrush(&text_color, None)?;
            let selected_text_brush = dc.CreateSolidColorBrush(&selected_text_color, None)?;

            if self.use_cursor.get() && i == self.current_sel.get() {
                dc.FillRectangle(&text_rect, &text_brush);
                dc.DrawText(
                    item.as_wide(),
                    self.text_format.borrow().deref(),
                    &text_rect,
                    &selected_text_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            } else {
                dc.DrawText(
                    item.as_wide(),
                    self.text_format.borrow().deref(),
                    &text_rect,
                    &text_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }

        Ok(())
    }
}

#[no_mangle]
unsafe extern "C" fn CreateCandidateWindow(
    parent: HWND,
    image_path: PCWSTR,
    ret: *mut *mut c_void,
) {
    let window = Window::new().into_object();
    window.create(
        parent,
        (WS_POPUP | WS_CLIPCHILDREN).0,
        (WS_EX_TOOLWINDOW | WS_EX_TOPMOST).0,
    );

    let factory: ID2D1Factory1 = D2D1CreateFactory(
        D2D1_FACTORY_TYPE_SINGLE_THREADED,
        Some(&D2D1_FACTORY_OPTIONS::default()),
    )
    .expect("failed to create Direct2D factory");

    let mut dpi = 0.0;
    let mut dpiy = 0.0;
    factory.GetDesktopDpi(&mut dpi, &mut dpiy);

    let dwrite_factory: IDWriteFactory1 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).unwrap();
    let text_format = dwrite_factory
        .CreateTextFormat(
            w!("Segoe UI"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            16.0,
            w!(""),
        )
        .unwrap();

    let nine_patch_bitmap = NinePatchBitmap::new(image_path).unwrap();

    let candidate_window = CandidateWindow {
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
        dpi,
        window,
    }
    .into_object();
    Window::register_hwnd(candidate_window.hwnd(), candidate_window.to_interface());
    ret.write(
        candidate_window
            .into_interface::<ICandidateWindow>()
            .into_raw(),
    )
}

impl ICandidateWindow_Impl for CandidateWindow_Impl {
    unsafe fn set_font_size(&self, font_size: u32) {
        self.text_format.replace(
            self.dwrite_factory
                .CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    font_size as f32,
                    w!(""),
                )
                .unwrap(),
        );
    }

    unsafe fn add(&self, item: PCWSTR, sel_key: u16) {
        self.items.borrow_mut().push(item.to_hstring().unwrap());
        self.sel_keys.borrow_mut().push(sel_key);
    }

    unsafe fn current_sel_key(&self) -> u16 {
        *self.sel_keys.borrow().get(self.current_sel.get()).unwrap()
    }

    unsafe fn clear(&self) {
        self.items.borrow_mut().clear();
        self.sel_keys.borrow_mut().clear();
        self.current_sel.set(0);
        self.has_result.set(false);
    }

    unsafe fn set_cand_per_row(&self, n: c_int) {
        if n as usize != self.cand_per_row.get() {
            self.cand_per_row.set(n as usize);
        }
    }

    unsafe fn set_use_cursor(&self, r#use: bool) {
        self.use_cursor.set(r#use);
        if self.is_visible() {
            self.refresh();
        }
    }

    unsafe fn filter_key_event(&self, key_code: u16) -> bool {
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
            self.refresh();
            return true;
        }
        false
    }

    unsafe fn has_result(&self) -> bool {
        self.has_result.get()
    }

    unsafe fn recalculate_size(&self) {
        let _ = self.this.recalculate_size();
    }
}

impl IWindow_Impl for CandidateWindow_Impl {
    unsafe fn hwnd(&self) -> HWND {
        self.window.hwnd()
    }

    unsafe fn create(&self, parent: HWND, style: u32, ex_style: u32) -> bool {
        self.window.create(parent, style, ex_style)
    }

    unsafe fn destroy(&self) {
        self.window.destroy()
    }

    unsafe fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    unsafe fn is_window(&self) -> bool {
        self.window.is_window()
    }

    unsafe fn r#move(&self, x: c_int, y: c_int) {
        self.window.r#move(x, y)
    }

    unsafe fn size(&self, width: *mut c_int, height: *mut c_int) {
        self.window.size(width, height)
    }

    unsafe fn resize(&self, width: c_int, height: c_int) {
        self.window.resize(width, height)
    }

    unsafe fn client_rect(&self, rect: *mut RECT) {
        self.window.client_rect(rect)
    }

    unsafe fn rect(&self, rect: *mut RECT) {
        self.window.rect(rect)
    }

    unsafe fn show(&self) {
        self.window.show()
    }

    unsafe fn hide(&self) {
        self.window.hide()
    }

    unsafe fn refresh(&self) {
        self.window.refresh()
    }

    unsafe fn wnd_proc(&self, msg: c_uint, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(self.hwnd(), &mut ps);
                let _ = self.on_paint();
                let _ = EndPaint(self.hwnd(), &ps);
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

impl ITfUIElement_Impl for CandidateWindow_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        Ok("Candidate window~".into())
    }

    fn GetGUID(&self) -> Result<GUID> {
        Ok(GUID::from_values(
            0xBD7CCC94,
            0x57CD,
            0x41D3,
            [0xA7, 0x89, 0xAF, 0x47, 0x89, 0xC, 0xEB, 0x29],
        ))
    }

    fn Show(&self, bshow: BOOL) -> Result<()> {
        unsafe {
            if bshow.as_bool() {
                self.show();
            } else {
                self.hide();
            }
        }
        Ok(())
    }

    fn IsShown(&self) -> Result<BOOL> {
        unsafe { Ok(self.is_visible().into()) }
    }
}

impl ITfCandidateListUIElement_Impl for CandidateWindow_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        // Update all
        Ok(TF_CLUIE_DOCUMENTMGR
            | TF_CLUIE_COUNT
            | TF_CLUIE_SELECTION
            | TF_CLUIE_STRING
            | TF_CLUIE_PAGEINDEX
            | TF_CLUIE_CURRENTPAGE)
    }

    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        // TODO
        Err(Error::empty())
    }

    fn GetCount(&self) -> Result<u32> {
        Ok(self.items.borrow().len().min(10) as u32)
    }

    fn GetSelection(&self) -> Result<u32> {
        Ok(self.current_sel.get() as u32)
    }

    fn GetString(&self, uindex: u32) -> Result<BSTR> {
        self.items
            .borrow()
            .get(uindex as usize)
            .map(|hstr| hstr.as_wide())
            .map(|wstr| BSTR::from_wide(wstr))
            .ok_or(Error::empty())?
    }

    fn GetPageIndex(&self, pindex: *mut u32, usize: u32, pupagecnt: *mut u32) -> Result<()> {
        unsafe {
            pupagecnt.write(1);
            if usize > 0 {
                pindex.write(0);
            }
        }
        Ok(())
    }

    fn SetPageIndex(&self, _pindex: *const u32, _upagecnt: u32) -> Result<()> {
        Ok(())
    }

    fn GetCurrentPage(&self) -> Result<u32> {
        Ok(0)
    }
}
