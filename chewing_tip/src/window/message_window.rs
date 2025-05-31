// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::RefCell,
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
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_core::*;

use super::{Window, WndProc};
use crate::gfx::*;

const ID_TIMEOUT: usize = 1;

#[derive(Debug)]
pub(crate) struct MessageWindow {
    text: RefCell<HSTRING>,
    factory: ID2D1Factory1,
    dwrite_factory: IDWriteFactory1,
    text_format: RefCell<IDWriteTextFormat>,
    target: RefCell<Option<ID2D1DeviceContext>>,
    swapchain: RefCell<Option<IDXGISwapChain1>>,
    dcomptarget: RefCell<Option<IDCompositionTarget>>,
    brush: RefCell<Option<ID2D1SolidColorBrush>>,
    nine_patch_bitmap: NinePatchBitmap,
    window: Window,
}

impl MessageWindow {
    pub(crate) fn new(parent: HWND, image_path: &Path) -> Result<Rc<MessageWindow>> {
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
                w!("zh-TW"),
            )?;

            let image_path = HSTRING::from(image_path.to_string_lossy().as_ref());
            let nine_patch_bitmap = NinePatchBitmap::new(&image_path)?;

            let message_window = Rc::new(MessageWindow {
                text: RefCell::new(h!("").to_owned()),
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
                message_window.hwnd(),
                Rc::clone(&message_window) as Rc<dyn WndProc>,
            );
            Ok(message_window)
        }
    }

    pub(crate) fn r#move(&self, x: c_int, y: c_int) {
        self.window.r#move(x, y);
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.window.hwnd()
    }

    pub(crate) fn show(&self) {
        self.window.show()
    }

    fn recalculate_size(&self) -> Result<()> {
        let text_layout = unsafe {
            self.dwrite_factory.CreateTextLayout(
                self.text.borrow().as_ref(),
                self.text_format.borrow().deref(),
                f32::MAX,
                f32::MAX,
            )?
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe { text_layout.GetMetrics(&mut metrics)? };

        let margin = self.nine_patch_bitmap.margin();
        let width = metrics.width + margin * 2.0;
        let height = metrics.height + margin * 2.0;

        // Convert to HW pixels
        let dpi = unsafe { GetDpiForWindow(self.window.hwnd()) } as f32;
        let width = width * dpi / USER_DEFAULT_SCREEN_DPI as f32;
        let height = height * dpi / USER_DEFAULT_SCREEN_DPI as f32;

        unsafe {
            let _ = SetWindowPos(
                self.window.hwnd(),
                Some(HWND_TOPMOST),
                0,
                0,
                width as i32,
                height as i32,
                SWP_NOACTIVATE | SWP_NOMOVE,
            );
        }
        self.resize_swap_chain(width as u32, height as u32)?;

        Ok(())
    }

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
                        DXGI_FORMAT_UNKNOWN,
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
            unsafe { GetClientRect(self.window.hwnd(), &mut rc)? };

            let swapchain = create_swapchain(
                &device,
                (rc.right - rc.left) as u32,
                (rc.bottom - rc.top) as u32,
            )?;
            let dpi = unsafe { GetDpiForWindow(self.window.hwnd()) } as f32;
            unsafe { target.SetDpi(dpi, dpi) };
            create_swapchain_bitmap(&swapchain, &target, dpi)?;
            let dcomptarget =
                setup_direct_composition(&device, self.window.hwnd(), &swapchain)?;

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
                GetClientRect(self.window.hwnd(), &mut rc)?;
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
                let margin = self.nine_patch_bitmap.margin();

                if let Some(brush) = self.brush.borrow().as_ref() {
                    target.DrawText(
                        self.text.borrow().as_ref(),
                        self.text_format.borrow().deref(),
                        &D2D_RECT_F {
                            left: margin,
                            top: margin,
                            right: f32::MAX,
                            bottom: f32::MAX,
                        },
                        brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
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
    pub(crate) fn set_font_size(&self, font_size: i32) {
        if let Ok(text_format) = unsafe {
            self.dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size as f32,
                w!("zh-TW"),
            )
        } {
            self.text_format.replace(text_format);
        }
    }
    pub(crate) fn set_text(&self, text: HSTRING) -> Result<()> {
        self.text.replace(text);
        self.recalculate_size()?;
        if self.window.is_visible() {
            self.window.refresh();
        }
        Ok(())
    }
}

impl WndProc for MessageWindow {
    fn wnd_proc(&self, msg: c_uint, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_PAINT => {
                    let mut ps = PAINTSTRUCT::default();
                    BeginPaint(self.hwnd(), &mut ps);
                    let _ = self.on_paint();
                    let _ = EndPaint(self.hwnd(), &ps);
                    LRESULT(0)
                }
                WM_TIMER => {
                    if wp.0 == ID_TIMEOUT {
                        self.window.hide();
                        let _ = KillTimer(Some(self.hwnd()), ID_TIMEOUT);
                    }
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
