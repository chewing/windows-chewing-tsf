use core::f32;
use std::{
    cell::RefCell,
    ffi::{c_int, c_uint, c_void},
    ops::Deref,
};

use windows::{
    core::{h, implement, interface, w, ComObject, ComObjectInner, Interface, PCWSTR},
    Foundation::Numerics::Matrix3x2,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::{
            Direct2D::{
                Common::{D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F},
                D2D1CreateFactory, ID2D1DeviceContext, ID2D1Factory1, ID2D1SolidColorBrush,
                D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
                D2D1_BITMAP_PROPERTIES1, D2D1_BRUSH_PROPERTIES, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
                D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_OPTIONS,
                D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_UNIT_MODE_DIPS,
            },
            Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
            Direct3D11::{
                D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                D3D11_SDK_VERSION,
            },
            DirectWrite::{
                DWriteCreateFactory, IDWriteFactory1, IDWriteTextFormat,
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_METRICS,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
                IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1, DXGI_ERROR_UNSUPPORTED,
                DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
                DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
            Gdi::{
                BeginPaint, EndPaint, GetSysColor, COLOR_INFOBK, COLOR_INFOTEXT, PAINTSTRUCT,
                SYS_COLOR_INDEX,
            },
        },
        UI::WindowsAndMessaging::{
            DefWindowProcW, GetClientRect, KillTimer, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE,
            SWP_NOMOVE, WM_NCDESTROY, WM_PAINT, WM_TIMER, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW,
            WS_EX_TOPMOST, WS_POPUP,
        },
    },
};
use windows_core::{Result, HSTRING};

use super::{IWindow, IWindow_Impl, IWindow_Vtbl, Window};

const ID_TIMEOUT: usize = 1;

#[interface("7375ef7b-4564-46eb-b8d1-e27228428623")]
unsafe trait IMessageWindow: IWindow {
    fn set_font_size(&self, font_size: u32);
    fn set_text(&self, text: PCWSTR);
}

#[derive(Debug)]
#[implement(IMessageWindow, IWindow)]
struct MessageWindow {
    text: RefCell<HSTRING>,
    factory: ID2D1Factory1,
    dwrite_factory: IDWriteFactory1,
    text_format: RefCell<IDWriteTextFormat>,
    target: RefCell<Option<ID2D1DeviceContext>>,
    swapchain: RefCell<Option<IDXGISwapChain1>>,
    brush: RefCell<Option<ID2D1SolidColorBrush>>,
    dpi: f32,
    window: ComObject<Window>,
}

impl MessageWindow {
    fn recalculate_size(&self) -> Result<()> {
        let text_layout = unsafe {
            self.dwrite_factory.CreateTextLayout(
                self.text.borrow().as_wide(),
                self.text_format.borrow().deref(),
                f32::MAX,
                f32::MAX,
            )?
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe { text_layout.GetMetrics(&mut metrics).unwrap() };

        // FIXME
        let margin = 5.0;
        let width = metrics.width + margin * 2.0;
        let height = metrics.height + margin * 2.0;
        unsafe {
            SetWindowPos(
                self.window.hwnd.get(),
                HWND_TOPMOST,
                0,
                0,
                width as i32,
                height as i32,
                SWP_NOACTIVATE | SWP_NOMOVE,
            )?
        };
        self.resize_swap_chain(width as u32, height as u32)?;

        Ok(())
    }

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
                        DXGI_FORMAT_UNKNOWN,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )
                    .is_ok()
            } {
                create_swapchain_bitmap(swapchain, target)?;
            } else {
                self.target.take();
                self.swapchain.take();
                self.brush.take();
            }

            self.on_paint()?;
        }

        Ok(())
    }

    fn on_paint(&self) -> Result<()> {
        let create_target = self.target.borrow().is_none();
        if create_target {
            let device = create_device()?;
            let target = create_render_target(&self.factory, &device)?;
            unsafe { target.SetDpi(self.dpi, self.dpi) };

            let swapchain = create_swapchain(&device, self.window.hwnd.get())?;
            create_swapchain_bitmap(&swapchain, &target)?;

            self.brush
                .replace(create_brush(&target, create_color(COLOR_INFOTEXT)).ok());
            self.target.replace(Some(target));
            self.swapchain.replace(Some(swapchain));
        }
        let target = self.target.borrow();
        let swapchain = self.swapchain.borrow();
        let target = target.as_ref().unwrap();
        unsafe {
            let mut rc = RECT::default();
            GetClientRect(self.window.hwnd.get(), &mut rc)?;

            target.BeginDraw();
            target.Clear(Some(&create_color(COLOR_INFOBK)));

            target.DrawText(
                self.text.borrow().as_wide(),
                self.text_format.borrow().deref(),
                &D2D_RECT_F {
                    left: 5.0,
                    top: 5.0,
                    right: f32::MAX,
                    bottom: f32::MAX,
                },
                self.brush.borrow().as_ref().unwrap(),
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
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
}

#[no_mangle]
unsafe extern "C" fn CreateMessageWindow(parent: HWND, ret: *mut *mut c_void) {
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

    let message_window = MessageWindow {
        text: RefCell::new(h!("").to_owned()),
        factory,
        dwrite_factory,
        text_format: RefCell::new(text_format),
        target: None.into(),
        swapchain: None.into(),
        brush: None.into(),
        dpi,
        window,
    }
    .into_object();
    Window::register_hwnd(message_window.hwnd(), message_window.to_interface());
    ret.write(message_window.into_interface::<IMessageWindow>().into_raw())
}

impl IWindow_Impl for MessageWindow_Impl {
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
        self.window.r#move(x, y);
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
            WM_TIMER => {
                if wp.0 == ID_TIMEOUT {
                    self.hide();
                    KillTimer(self.hwnd(), ID_TIMEOUT).expect("failed to kill timer");
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                self.target.take();
                self.swapchain.take();
                self.brush.take();
                LRESULT(0)
            }
            _ => DefWindowProcW(self.hwnd(), msg, wp, lp),
        }
    }
}

impl IMessageWindow_Impl for MessageWindow_Impl {
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
                    self.text.borrow().deref(),
                )
                .unwrap(),
        );
    }
    unsafe fn set_text(&self, text: PCWSTR) {
        self.text.replace(text.to_hstring().unwrap());
        self.recalculate_size().unwrap();
        if self.is_visible() {
            self.refresh();
        }
    }
}

fn create_color(gdi_color_index: SYS_COLOR_INDEX) -> D2D1_COLOR_F {
    let color = unsafe { GetSysColor(gdi_color_index) };
    D2D1_COLOR_F {
        r: (color & 0xFF) as f32 / 255.0,
        g: ((color >> 8) & 0xFF) as f32 / 255.0,
        b: ((color >> 16) & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

fn create_brush(target: &ID2D1DeviceContext, color: D2D1_COLOR_F) -> Result<ID2D1SolidColorBrush> {
    let properties = D2D1_BRUSH_PROPERTIES {
        opacity: 0.8,
        transform: Matrix3x2::identity(),
    };

    unsafe { target.CreateSolidColorBrush(&color, Some(&properties)) }
}

fn create_device_with_type(drive_type: D3D_DRIVER_TYPE) -> Result<ID3D11Device> {
    let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;

    let mut device = None;

    unsafe {
        D3D11CreateDevice(
            None,
            drive_type,
            None,
            flags,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
        .map(|()| device.unwrap())
    }
}

fn create_device() -> Result<ID3D11Device> {
    let mut result = create_device_with_type(D3D_DRIVER_TYPE_HARDWARE);

    if let Err(err) = &result {
        if err.code() == DXGI_ERROR_UNSUPPORTED {
            result = create_device_with_type(D3D_DRIVER_TYPE_WARP);
        }
    }

    result
}

fn create_render_target(
    factory: &ID2D1Factory1,
    device: &ID3D11Device,
) -> Result<ID2D1DeviceContext> {
    unsafe {
        let d2device = factory.CreateDevice(&device.cast::<IDXGIDevice>()?)?;

        let target = d2device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;

        target.SetUnitMode(D2D1_UNIT_MODE_DIPS);

        Ok(target)
    }
}

fn get_dxgi_factory(device: &ID3D11Device) -> Result<IDXGIFactory2> {
    let dxdevice = device.cast::<IDXGIDevice>()?;
    unsafe { dxdevice.GetAdapter()?.GetParent() }
}

fn create_swapchain_bitmap(swapchain: &IDXGISwapChain1, target: &ID2D1DeviceContext) -> Result<()> {
    let surface: IDXGISurface = unsafe { swapchain.GetBuffer(0)? };

    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_IGNORE,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };

    unsafe {
        let bitmap = target.CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
        target.SetTarget(&bitmap);
    };

    Ok(())
}

fn create_swapchain(device: &ID3D11Device, window: HWND) -> Result<IDXGISwapChain1> {
    let factory = get_dxgi_factory(device)?;

    let props = DXGI_SWAP_CHAIN_DESC1 {
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        ..Default::default()
    };

    unsafe { factory.CreateSwapChainForHwnd(device, window, &props, None, None) }
}
