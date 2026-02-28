// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Display;
use std::sync::LazyLock;

use exn::Exn;
use exn::ResultExt;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::UI::HiDpi::*;
use windows_core::Interface;

static DEVICE: LazyLock<Result<ID3D11Device, Exn<GfxError>>> =
    LazyLock::new(|| create_device_with_type(D3D_DRIVER_TYPE_WARP));

fn create_device_with_type(drive_type: D3D_DRIVER_TYPE) -> Result<ID3D11Device, Exn<GfxError>> {
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
        .or_raise(|| GfxError {
            msg: format!("failed to create D3D11 device with type {drive_type:?}"),
        })
    }
}

pub(crate) fn d3d11_device() -> Result<ID3D11Device, Exn<GfxError>> {
    DEVICE
        .as_ref()
        .map(|d| d.clone())
        .map_err(|e| (*e).clone().into())
}

pub(crate) fn create_render_target(
    factory: &ID2D1Factory1,
    device: &ID3D11Device,
) -> Result<ID2D1DeviceContext, Exn<GfxError>> {
    let err = || GfxError {
        msg: "failed to create render target".to_string(),
    };
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

pub(crate) fn get_dxgi_factory(device: &ID3D11Device) -> Result<IDXGIFactory2, Exn<GfxError>> {
    let err = || GfxError {
        msg: "failed to get DXGI factory".to_string(),
    };
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
    dpi: f32,
) -> Result<(), Exn<GfxError>> {
    let err = || GfxError {
        msg: format!("failed to create new swapchain bitmap with dpi {dpi}"),
    };
    let surface: IDXGISurface = unsafe { swapchain.GetBuffer(0).or_raise(err)? };

    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi,
        dpiY: dpi,
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
) -> Result<IDXGISwapChain1, Exn<GfxError>> {
    let err = || GfxError {
        msg: format!("failed to create new swapchain with size {width}x{height}"),
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
) -> Result<IDCompositionTarget, Exn<GfxError>> {
    let err = || GfxError {
        msg: "failed to setup direct composition".to_string(),
    };
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
        let dpi = GetDpiForWindow(hwnd);
        if dpi == 0 { 96.0 } else { dpi as f32 }
    }
}

pub(crate) fn get_scale_for_window(hwnd: HWND) -> f32 {
    get_dpi_for_window(hwnd) / 96.0
}

#[derive(Debug, Clone)]
pub(crate) struct GfxError {
    msg: String,
}

impl Display for GfxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for GfxError {}
