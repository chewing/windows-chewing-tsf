// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::LazyLock;

use anyhow::bail;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::DirectWrite::IDWriteGdiInterop;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::*;
use windows::core::*;

static DEVICE: LazyLock<Result<ID3D11Device>> =
    LazyLock::new(|| create_device_with_type(D3D_DRIVER_TYPE_WARP));

fn create_device_with_type(drive_type: D3D_DRIVER_TYPE) -> Result<ID3D11Device> {
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
    }
}

pub(crate) fn d3d11_device() -> Result<ID3D11Device> {
    LazyLock::force(&DEVICE).to_owned()
}

pub(crate) fn create_render_target(
    factory: &ID2D1Factory1,
    device: &ID3D11Device,
) -> Result<ID2D1DeviceContext> {
    unsafe {
        let d2device = factory.CreateDevice(&device.cast::<IDXGIDevice>()?)?;
        let target = d2device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
        Ok(target)
    }
}

pub(crate) fn get_dxgi_factory(device: &ID3D11Device) -> Result<IDXGIFactory2> {
    let dxdevice = device.cast::<IDXGIDevice>()?;
    unsafe { dxdevice.GetAdapter()?.GetParent() }
}

pub(crate) fn create_swapchain_bitmap(
    swapchain: &IDXGISwapChain1,
    target: &ID2D1DeviceContext,
    dpi: f32,
) -> Result<()> {
    let surface: IDXGISurface = unsafe { swapchain.GetBuffer(0)? };

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
        let bitmap = target.CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
        target.SetTarget(&bitmap);
    };

    Ok(())
}

pub(crate) fn create_swapchain(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    let factory = get_dxgi_factory(device)?;

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

    unsafe { factory.CreateSwapChainForComposition(device, &props, None) }
}

pub(crate) fn setup_direct_composition(
    device: &ID3D11Device,
    window: HWND,
    swapchain: &IDXGISwapChain,
) -> Result<IDCompositionTarget> {
    let dxgidevice = device.cast::<IDXGIDevice>()?;
    unsafe {
        let dcompdevice: IDCompositionDevice = DCompositionCreateDevice(&dxgidevice)?;
        let dcomptarget: IDCompositionTarget = dcompdevice.CreateTargetForHwnd(window, true)?;
        let visual: IDCompositionVisual = dcompdevice.CreateVisual()?;
        visual.SetContent(swapchain)?;
        dcomptarget.SetRoot(&visual)?;
        dcompdevice.Commit()?;
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

pub(crate) fn dwrite_family_from_gdi_name(
    interop: &IDWriteGdiInterop,
    family_name: &HSTRING,
) -> anyhow::Result<HSTRING> {
    let mut logfont = LOGFONTW {
        lfCharSet: DEFAULT_CHARSET,
        lfOutPrecision: OUT_DEFAULT_PRECIS,
        lfClipPrecision: CLIP_DEFAULT_PRECIS,
        lfQuality: DEFAULT_QUALITY,
        ..Default::default()
    };
    unsafe {
        let fa: &[u16] = family_name;
        if fa.len() > logfont.lfFaceName.len() {
            bail!(
                "Unable to convert GDI font name longer than {}",
                logfont.lfFaceName.len()
            );
        }
        logfont.lfFaceName[..fa.len()].copy_from_slice(fa);
        let dwfont = interop.CreateFontFromLOGFONT(&logfont)?;
        let family = dwfont.GetFontFamily()?;
        let names = family.GetFamilyNames()?;
        let name_len = names.GetStringLength(0)? as usize;
        let mut name = vec![0; name_len];
        names.GetString(0, &mut name)?;
        Ok(HSTRING::from_wide(&name))
    }
}
