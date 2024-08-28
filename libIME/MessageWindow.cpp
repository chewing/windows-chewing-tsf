//
//	Copyright (C) 2013 Hong Jen Yee (PCMan) <pcman.tw@gmail.com>
//
//	This library is free software; you can redistribute it and/or
//	modify it under the terms of the GNU Library General Public
//	License as published by the Free Software Foundation; either
//	version 2 of the License, or (at your option) any later version.
//
//	This library is distributed in the hope that it will be useful,
//	but WITHOUT ANY WARRANTY; without even the implied warranty of
//	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
//	Library General Public License for more details.
//
//	You should have received a copy of the GNU Library General Public
//	License along with this library; if not, write to the
//	Free Software Foundation, Inc., 51 Franklin St, Fifth Floor,
//	Boston, MA  02110-1301, USA.
//

#include "MessageWindow.h"

#include <Unknwn.h>
#include <d2d1.h>
#include <d2d1_1.h>
#include <d2d1_1helper.h>
#include <d3d11_1.h>
#include <dwrite_1.h>
#include <winrt/base.h>

#include "DrawUtils.h"
#include "TextService.h"

using winrt::check_hresult;
using winrt::com_ptr;

namespace Ime {

MessageWindow::MessageWindow(TextService* service, EditSession* session)
    : ImeWindow(service) {
    HWND parent = service->compositionWindow(session);
    create(parent, WS_POPUP | WS_CLIPCHILDREN,
           WS_EX_TOOLWINDOW | WS_EX_TOPMOST);

    check_hresult(
        D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, factory_.put()));

    com_ptr<ID3D11Device> d3device;
    com_ptr<IDXGIDevice> dxdevice;
    com_ptr<IDXGIAdapter> adapter;
    com_ptr<IDXGIFactory2> factory;
    com_ptr<ID2D1Device> device;
    com_ptr<IDXGISurface> surface;
    com_ptr<ID2D1Bitmap1> bitmap;
    check_hresult(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_WARP, nullptr,
                                    D3D11_CREATE_DEVICE_BGRA_SUPPORT, nullptr,
                                    0, D3D11_SDK_VERSION, d3device.put(),
                                    nullptr, nullptr));
    dxdevice = d3device.as<IDXGIDevice>();
    check_hresult(factory_->CreateDevice(dxdevice.get(), device.put()));
    check_hresult(device->CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
                                              target_.put()));

    DXGI_SWAP_CHAIN_DESC1 swapChainDesc = {};
    swapChainDesc.Width = 0;
    swapChainDesc.Height = 0;
    swapChainDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    swapChainDesc.Stereo = false;
    swapChainDesc.SampleDesc.Count = 1;  // don't use multi-sampling
    swapChainDesc.SampleDesc.Quality = 0;
    swapChainDesc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    swapChainDesc.BufferCount = 2;  // use double buffering to enable flip
    swapChainDesc.Scaling = DXGI_SCALING_STRETCH;
    swapChainDesc.SwapEffect = DXGI_SWAP_EFFECT_DISCARD;
    swapChainDesc.Flags = 0;

    check_hresult(dxdevice->GetAdapter(adapter.put()));
    check_hresult(adapter->GetParent(__uuidof(factory), factory.put_void()));

    check_hresult(factory->CreateSwapChainForHwnd(d3device.get(), hwnd_,
                                                  &swapChainDesc, nullptr,
                                                  nullptr, swapChain_.put()));
    check_hresult(
        swapChain_->GetBuffer(0, __uuidof(surface), surface.put_void()));
    auto bitmap_props = D2D1::BitmapProperties1(
        D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE));
    check_hresult(target_->CreateBitmapFromDxgiSurface(
        surface.get(), &bitmap_props, bitmap.put()));
    target_->SetTarget(bitmap.get());
}

MessageWindow::~MessageWindow(void) {}

// virtual
void MessageWindow::recalculateSize() {
    com_ptr<IDWriteFactory1> pDwriteFactory;
    com_ptr<IDWriteTextFormat> pTextFormat;
    DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory1),
                        reinterpret_cast<IUnknown**>(pDwriteFactory.put()));
    pDwriteFactory->CreateTextFormat(
        L"Segoe UI", nullptr, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, fontSize_, L"",
        pTextFormat.put());
    com_ptr<IDWriteTextLayout> pTextLayout;
    DWRITE_TEXT_METRICS metrics;
    D2D1_POINT_2F origin;
    pDwriteFactory->CreateTextLayout(text_.c_str(), text_.length(),
                                     pTextFormat.get(), D2D1::FloatMax(),
                                     D2D1::FloatMax(), pTextLayout.put());
    pTextLayout->GetMetrics(&metrics);

    auto width = metrics.width + margin_ * 2;
    auto height = metrics.height + margin_ * 2;
    SetWindowPos(hwnd_, HWND_TOPMOST, 0, 0, width, height,
                 SWP_NOACTIVATE | SWP_NOMOVE);

    resizeSwapChain(width, height);
}

void MessageWindow::resizeSwapChain(int width, int height) {
    com_ptr<IDXGISurface> surface;
    com_ptr<ID2D1Bitmap1> bitmap;

    target_->SetTarget(nullptr);
    swapChain_->ResizeBuffers(0, width, height, DXGI_FORMAT_B8G8R8A8_UNORM, 0);
    check_hresult(
        swapChain_->GetBuffer(0, __uuidof(surface), surface.put_void()));
    auto bitmap_props = D2D1::BitmapProperties1(
        D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE));
    check_hresult(target_->CreateBitmapFromDxgiSurface(
        surface.get(), &bitmap_props, bitmap.put()));
    target_->SetTarget(bitmap.get());
}

void MessageWindow::setText(std::wstring text) {
    // FIXMEl: use different appearance under immersive mode
    text_ = text;
    recalculateSize();
    if (IsWindowVisible(hwnd_)) {
        InvalidateRect(hwnd_, NULL, TRUE);
    }
}

LRESULT MessageWindow::wndProc(UINT msg, WPARAM wp, LPARAM lp) {
    switch (msg) {
        case WM_PAINT:
            onPaint(wp, lp);
            break;
        case WM_MOUSEACTIVATE:
            return MA_NOACTIVATE;
        default:
            return ImeWindow::wndProc(msg, wp, lp);
    }
    return 0;
}

void MessageWindow::onPaint(WPARAM wp, LPARAM lp) {
    RECT rc, textrc = {0};
    GetClientRect(hwnd_, &rc);

    target_->BeginDraw();

    com_ptr<ID2D1SolidColorBrush> pTextBrush;

    if (isImmersive()) {
        // draw a flat black border in Windows 8 app immersive mode
        com_ptr<ID2D1SolidColorBrush> pBrush;
        check_hresult(target_->CreateSolidColorBrush(
            D2D1::ColorF(D2D1::ColorF::Black), pBrush.put()));
        check_hresult(target_->CreateSolidColorBrush(
            D2D1::ColorF(GetSysColor(COLOR_WINDOWTEXT)), pTextBrush.put()));
        target_->Clear(D2D1::ColorF(GetSysColor(COLOR_WINDOW)));
        target_->DrawRectangle(
            D2D1::RectF(rc.left, rc.top, rc.right, rc.bottom), pBrush.get(),
            3.0f);
    } else {
        check_hresult(target_->CreateSolidColorBrush(
            D2D1::ColorF(GetSysColor(COLOR_INFOTEXT)), pTextBrush.put()));
        target_->Clear(D2D1::ColorF(GetSysColor(COLOR_INFOBK)));
        ::FillSolidRectD2D(target_.get(), rc.left, rc.top, rc.right, rc.bottom,
                           GetSysColor(COLOR_INFOBK));
        ::Draw3DBorderD2D(target_.get(), &rc, GetSysColor(COLOR_3DFACE), 0, 1);
    }

    com_ptr<IDWriteFactory1> pDwriteFactory;
    com_ptr<IDWriteTextFormat> pTextFormat;
    DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory1),
                        reinterpret_cast<IUnknown**>(pDwriteFactory.put()));
    pDwriteFactory->CreateTextFormat(
        L"Segoe UI", nullptr, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, fontSize_, L"",
        pTextFormat.put());
    target_->DrawText(
        text_.c_str(), text_.length(), pTextFormat.get(),
        D2D1::RectF(margin_, margin_, D2D1::FloatMax(), D2D1::FloatMax()),
        pTextBrush.get());

    check_hresult(target_->EndDraw());
    check_hresult(swapChain_->Present(1, 0));
    ValidateRect(hwnd_, nullptr);
}

}  // namespace Ime
