#include "NinePatch.h"

#include <Unknwn.h>
#include <combaseapi.h>
#include <d2d1.h>
#include <d2d1_1.h>
#include <d2d1helper.h>
#include <intsafe.h>
#include <minwindef.h>
#include <wincodec.h>
#include <winnt.h>
#include <winrt/base.h>
#include <wtypesbase.h>

#include <string>

#include "rustlib_bridge/lib.h"

using winrt::check_hresult;
using winrt::com_ptr;

NinePatch::NinePatch(std::wstring image_path)
    : image_path_(image_path), ninePatch_(nine_patch_uninit()) {
    com_ptr<IWICImagingFactory> imagingFactory;
    com_ptr<IWICBitmapDecoder> decoder;
    com_ptr<IWICBitmapFrameDecode> decoderFrame;
    com_ptr<IWICFormatConverter> converter;
    com_ptr<IWICBitmapLock> lock;
    check_hresult(
        CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                         __uuidof(imagingFactory), imagingFactory.put_void()));
    check_hresult(imagingFactory->CreateDecoderFromFilename(
        image_path_.c_str(), nullptr, GENERIC_READ,
        WICDecodeMetadataCacheOnDemand, decoder.put()));
    check_hresult(decoder->GetFrame(0, decoderFrame.put()));
    check_hresult(imagingFactory->CreateFormatConverter(converter.put()));
    check_hresult(converter->Initialize(
        decoderFrame.get(), GUID_WICPixelFormat32bppPRGBA,
        WICBitmapDitherTypeNone, nullptr, 0.f, WICBitmapPaletteTypeCustom));
    check_hresult(imagingFactory->CreateBitmapFromSource(
        converter.get(), WICBitmapCacheOnDemand, bitmap_.put()));

    UINT width;
    UINT height;
    bitmap_->GetSize(&width, &height);

    WICRect lockRect = {0, 0, static_cast<INT>(width),
                        static_cast<INT>(height)};
    check_hresult(bitmap_->Lock(&lockRect, WICBitmapLockRead, lock.put()));

    UINT stride;
    check_hresult(lock->GetStride(&stride));

    UINT len;
    unsigned char *data;
    check_hresult(lock->GetDataPointer(&len, &data));

    rust::Slice<const unsigned char> bitmap_slice =
        rust::Slice(const_cast<const unsigned char *>(data), len);
    ninePatch_ = make_nine_patch(bitmap_slice, stride, width, height);

    lock = nullptr;
}

NinePatch::~NinePatch() {}

HRESULT
NinePatch::DrawBitmap(ID2D1DeviceContext *dc, D2D1_RECT_F rect) {
    com_ptr<ID2D1Bitmap1> bitmap;
    check_hresult(dc->CreateBitmapFromWicBitmap(bitmap_.get(), bitmap.put()));

    auto patches = nine_patch_scale_to(ninePatch_, rect.right - rect.left,
                                       rect.bottom - rect.top);
    for (auto patch : patches) {
        auto source = D2D1::RectF(patch.source.left, patch.source.top,
                                  patch.source.right, patch.source.bottom);
        auto target = D2D1::RectF(
            rect.left + patch.target.left, rect.top + patch.target.top,
            rect.top + patch.target.right, rect.top + patch.target.bottom);
        dc->DrawBitmap(bitmap.get(), &target, 1.0f,
                       D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, &source);
    }
    return S_OK;
}

FLOAT
NinePatch::GetMargin() { return nine_patch_margin(ninePatch_); }