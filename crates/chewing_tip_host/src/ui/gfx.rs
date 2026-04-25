// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::error::Error;
use std::fmt::Display;
use std::sync::LazyLock;

use exn::{Result, ResultExt};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::GetMonitorInfoW;
use windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST;
use windows::Win32::Graphics::Gdi::MONITORINFO;
use windows::Win32::Graphics::Gdi::MonitorFromPoint;
use windows::Win32::Graphics::Gdi::MonitorFromWindow;
use windows::Win32::UI::HiDpi::*;
use windows::core::Interface;

static DEVICE: LazyLock<Result<ID3D11Device, GfxError>> =
    LazyLock::new(|| create_device_with_type(D3D_DRIVER_TYPE_WARP));

fn create_device_with_type(drive_type: D3D_DRIVER_TYPE) -> Result<ID3D11Device, GfxError> {
    let mut device = None;
    unsafe {
        D3D11CreateDevice(
            None,
            drive_type,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
        .map(|()| device.unwrap())
        .or_raise(|| {
            GfxError(format!(
                "failed to create D3D11 device with type {drive_type:?}"
            ))
        })
    }
}

pub(crate) fn d3d11_device() -> Result<ID3D11Device, GfxError> {
    DEVICE
        .as_ref()
        .map(|d| d.clone())
        .map_err(|e| (*e).clone().into())
}

pub(crate) fn create_render_target(
    factory: &ID2D1Factory1,
    device: &ID3D11Device,
) -> Result<ID2D1DeviceContext, GfxError> {
    let err = || GfxError("failed to create render target".to_string());

    unsafe {
        let d2device = factory
            .CreateDevice(&device.cast::<IDXGIDevice>().or_raise(err)?)
            .or_raise(err)?;
        let target = d2device
            .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
            .or_raise(err)?;
        Ok(target)
    }
}

pub(crate) fn get_dxgi_factory(device: &ID3D11Device) -> Result<IDXGIFactory2, GfxError> {
    let err = || GfxError("failed to get DXGI factory".to_string());
    let dxdevice = device.cast::<IDXGIDevice>().or_raise(err)?;
    unsafe {
        dxdevice
            .GetAdapter()
            .or_raise(err)?
            .GetParent()
            .or_raise(err)
    }
}

pub(crate) fn create_swapchain_bitmap(
    swapchain: &IDXGISwapChain1,
    target: &ID2D1DeviceContext,
) -> Result<(), GfxError> {
    let err = || GfxError(format!("failed to create new swapchain bitmap with dpi"));
    let surface: IDXGISurface = unsafe { swapchain.GetBuffer(0).or_raise(err)? };

    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };

    unsafe {
        let bitmap = target
            .CreateBitmapFromDxgiSurface(&surface, Some(&props))
            .or_raise(err)?;
        target.SetTarget(&bitmap);
    };

    Ok(())
}

pub(crate) fn create_swapchain(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1, GfxError> {
    let err = || {
        GfxError(format!(
            "failed to create new swapchain with size {width}x{height}"
        ))
    };
    let factory = get_dxgi_factory(device).or_raise(err)?;

    let props = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        ..Default::default()
    };

    unsafe {
        factory
            .CreateSwapChainForComposition(device, &props, None)
            .or_raise(err)
    }
}

pub(crate) fn setup_direct_composition(
    device: &ID3D11Device,
    window: HWND,
    swapchain: &IDXGISwapChain,
) -> Result<IDCompositionTarget, GfxError> {
    let err = || GfxError("failed to setup direct composition".to_string());
    let dxgidevice = device.cast::<IDXGIDevice>().or_raise(err)?;
    unsafe {
        let dcompdevice: IDCompositionDevice =
            DCompositionCreateDevice(&dxgidevice).or_raise(err)?;
        let dcomptarget: IDCompositionTarget = dcompdevice
            .CreateTargetForHwnd(window, true)
            .or_raise(err)?;
        let visual: IDCompositionVisual = dcompdevice.CreateVisual().or_raise(err)?;
        visual.SetContent(swapchain).or_raise(err)?;
        dcomptarget.SetRoot(&visual).or_raise(err)?;
        dcompdevice.Commit().or_raise(err)?;
        Ok(dcomptarget)
    }
}

pub(crate) fn get_dpi_for_window(hwnd: HWND) -> f32 {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut dpi_x = 96;
        let mut dpi_y = 96;
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        dpi_x as f32
    }
}

pub(crate) fn get_dpi_for_point(point: POINT) -> f32 {
    unsafe {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut dpi_x = 96;
        let mut dpi_y = 96;
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        dpi_x as f32
    }
}

pub(crate) fn clamp_point_to_monitor(mut x: i32, mut y: i32, w: i32, h: i32) -> (i32, i32) {
    // Ensure that the window does not fall outside of the screen.
    let monitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    let mut mi = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // Reinitialize rectangle if we can get the monitor bound
    unsafe {
        if GetMonitorInfoW(monitor, &mut mi).as_bool() {
            let rc = mi.rcWork;
            x = x.clamp(rc.left, rc.right - w);
            if x + w > rc.right {
                x = x.saturating_sub(w);
            }
            if y + h > rc.bottom {
                y = y.saturating_sub(h + 10);
            }
        }
    }

    (x, y)
}

pub fn color_s(rgb: &str) -> D2D1_COLOR_F {
    let mut rgb_u32 = u32::from_str_radix(rgb, 16).unwrap_or(0);
    let a = if rgb.len() > 6 {
        let a = rgb_u32 & 0xFF;
        rgb_u32 >>= 8;
        a as u16
    } else {
        255
    };
    let r = ((rgb_u32 >> 16) & 0xFF) as u16;
    let g = ((rgb_u32 >> 8) & 0xFF) as u16;
    let b = (rgb_u32 & 0xFF) as u16;
    D2D1_COLOR_F {
        r: (r as f32) / 255.0,
        g: (g as f32) / 255.0,
        b: (b as f32) / 255.0,
        a: (a as f32) / 255.0,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GfxError(String);
impl Error for GfxError {}
impl Display for GfxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GfxError: {}", self.0)
    }
}

#[cfg(test)]
mod test {
    use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

    use super::color_s;

    pub fn color_f(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
        D2D1_COLOR_F { r, g, b, a }
    }

    #[test]
    fn color_rgb() {
        assert_eq!(color_f(1.0, 0.0, 1.0, 1.0), color_s("FF00FF"));
    }
    #[test]
    fn color_rgba() {
        assert_eq!(color_f(1.0, 0.0, 1.0, 0.0), color_s("FF00FF00"));
    }
    #[test]
    fn color_alpha_only() {
        assert_eq!(color_f(0.0, 0.0, 1.0, 1.0), color_s("0000FFFF"));
        assert_eq!(color_f(0.0, 0.0, 0.0, 1.0), color_s("000000FF"));
    }
}
