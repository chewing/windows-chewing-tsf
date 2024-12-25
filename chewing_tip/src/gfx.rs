use core::slice;
use std::ptr::null_mut;

use nine_patch_drawable::NinePatchDrawable;
use nine_patch_drawable::PatchKind;
use nine_patch_drawable::Section;
use windows::core::*;
use windows::Foundation::Numerics::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;

#[derive(Debug)]
pub(crate) struct NinePatchBitmap {
    bitmap: IWICBitmap,
    nine_patch: NinePatchDrawable,
}

impl NinePatchBitmap {
    pub(crate) fn new(image_path: PCWSTR) -> Result<NinePatchBitmap> {
        unsafe {
            let wicfactory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let decoder = wicfactory.CreateDecoderFromFilename(
                image_path,
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )?;
            let frame = decoder.GetFrame(0)?;
            let converter = wicfactory.CreateFormatConverter()?;
            converter.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
            let bitmap = wicfactory.CreateBitmapFromSource(&converter, WICBitmapCacheOnDemand)?;
            let mut width = 0;
            let mut height = 0;
            bitmap.GetSize(&mut width, &mut height)?;
            let lock = bitmap.Lock(
                &WICRect {
                    X: 0,
                    Y: 0,
                    Width: width as i32,
                    Height: height as i32,
                },
                1,
            )?;
            let stride = lock.GetStride()?;

            let mut len = 0;
            let mut data = null_mut();
            lock.GetDataPointer(&mut len, &mut data)?;
            let data_slice = slice::from_raw_parts(data, len as usize);

            let nine_patch = NinePatchDrawable::new(
                data_slice,
                stride as usize,
                width as usize,
                height as usize,
            )
            .unwrap_or_else(|_| NinePatchDrawable {
                width: width as usize,
                height: height as usize,
                h_sections: vec![Section {
                    start: 1.0,
                    len: width as f32 - 1.0,
                    kind: PatchKind::Stretching,
                }],
                v_sections: vec![Section {
                    start: 1.0,
                    len: width as f32 - 1.0,
                    kind: PatchKind::Stretching,
                }],
                margin_left: 0.0,
                margin_top: 0.0,
                margin_right: 0.0,
                margin_bottom: 0.0,
            });
            Ok(NinePatchBitmap { bitmap, nine_patch })
        }
    }
    pub(crate) fn draw_bitmap(&self, dc: &ID2D1DeviceContext, rect: D2D_RECT_F) -> Result<()> {
        unsafe {
            let bitmap = dc.CreateBitmapFromWicBitmap(&self.bitmap, None)?;
            let patches = self.nine_patch.scale_to(
                (rect.right - rect.left) as usize,
                (rect.bottom - rect.top) as usize,
            );
            for patch in patches {
                let source = D2D_RECT_F {
                    left: patch.source.left,
                    top: patch.source.top,
                    right: patch.source.right,
                    bottom: patch.source.bottom,
                };
                let target = D2D_RECT_F {
                    left: patch.target.left,
                    top: patch.target.top,
                    right: patch.target.right,
                    bottom: patch.target.bottom,
                };
                dc.DrawBitmap(
                    &bitmap,
                    Some(&target),
                    1.0,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    Some(&source),
                    None,
                );
            }
            Ok(())
        }
    }
    pub(crate) fn margin(&self) -> f32 {
        self.nine_patch.margin_top
    }
}

pub(crate) fn create_color(gdi_color_index: SYS_COLOR_INDEX) -> D2D1_COLOR_F {
    let color = unsafe { GetSysColor(gdi_color_index) };
    D2D1_COLOR_F {
        r: (color & 0xFF) as f32 / 255.0,
        g: ((color >> 8) & 0xFF) as f32 / 255.0,
        b: ((color >> 16) & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

pub(crate) fn create_brush(
    target: &ID2D1DeviceContext,
    color: D2D1_COLOR_F,
) -> Result<ID2D1SolidColorBrush> {
    let properties = D2D1_BRUSH_PROPERTIES {
        opacity: 0.8,
        transform: Matrix3x2::identity(),
    };

    unsafe { target.CreateSolidColorBrush(&color, Some(&properties)) }
}

pub(crate) fn create_device_with_type(drive_type: D3D_DRIVER_TYPE) -> Result<ID3D11Device> {
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

pub(crate) fn create_device() -> Result<ID3D11Device> {
    let mut result = create_device_with_type(D3D_DRIVER_TYPE_HARDWARE);

    if let Err(err) = &result {
        if err.code() == DXGI_ERROR_UNSUPPORTED {
            result = create_device_with_type(D3D_DRIVER_TYPE_WARP);
        }
    }

    result
}

pub(crate) fn create_render_target(
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
