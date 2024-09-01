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

#include "CandidateWindow.h"

#include <Unknwn.h>
#include <d2d1_1.h>
#include <d2d1_1helper.h>
#include <d3d11_1.h>
#include <dcomp.h>
#include <dwrite_1.h>
#include <dxgi1_2.h>
#include <tchar.h>
#include <unknwnbase.h>
#include <windows.h>
#include <winrt/base.h>

#include <cassert>
#include <string>

#include "DrawUtils.h"
#include "EditSession.h"
#include "NinePatch.h"
#include "TextService.h"
#include "rustlib_bridge/lib.h"

using namespace std;
using winrt::check_hresult;
using winrt::com_ptr;

namespace Ime {

CandidateWindow::CandidateWindow(TextService *service, EditSession *session,
                                 wstring bitmap_path)
    : ImeWindow(service),
      refCount_(1),
      shown_(false),
      candPerRow_(1),
      textWidth_(0),
      itemHeight_(0),
      currentSel_(0),
      hasResult_(false),
      useCursor_(true),
      selKeyWidth_(0),
      ninePatch_(bitmap_path) {
    if (service->isImmersive()) {  // windows 8 app mode
        margin_ = 10;
        rowSpacing_ = 8;
        colSpacing_ = 12;
    } else {  // desktop mode
        margin_ = ninePatch_.GetMargin();
        rowSpacing_ = 4;
        colSpacing_ = 8;
    }

    HWND parent = service->compositionWindow(session);
    create(parent, WS_POPUP | WS_CLIPCHILDREN,
           WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_TOPMOST);

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
    swapChainDesc.Width = 100;
    swapChainDesc.Height = 100;
    swapChainDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    swapChainDesc.SampleDesc.Count = 1;  // don't use multi-sampling
    swapChainDesc.SampleDesc.Quality = 0;
    swapChainDesc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    swapChainDesc.BufferCount = 2;  // use double buffering to enable flip
    swapChainDesc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
    swapChainDesc.AlphaMode = DXGI_ALPHA_MODE_PREMULTIPLIED;

    check_hresult(dxdevice->GetAdapter(adapter.put()));
    check_hresult(adapter->GetParent(__uuidof(factory), factory.put_void()));

    check_hresult(factory->CreateSwapChainForComposition(
        d3device.get(), &swapChainDesc, nullptr, swapChain_.put()));
    check_hresult(
        swapChain_->GetBuffer(0, __uuidof(surface), surface.put_void()));
    auto bitmap_props = D2D1::BitmapProperties1(
        D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM,
                          D2D1_ALPHA_MODE_PREMULTIPLIED));
    check_hresult(target_->CreateBitmapFromDxgiSurface(
        surface.get(), &bitmap_props, bitmap.put()));
    target_->SetTarget(bitmap.get());

    // Setup Direct Composition
    check_hresult(DCompositionCreateDevice(
        dxdevice.get(), __uuidof(dcompDevice_), dcompDevice_.put_void()));

    check_hresult(
        dcompDevice_->CreateTargetForHwnd(hwnd_, true, dcompTarget.put()));

    check_hresult(dcompDevice_->CreateVisual(dcompVisual.put()));
    check_hresult(dcompVisual->SetContent(swapChain_.get()));
    check_hresult(dcompTarget->SetRoot(dcompVisual.get()));
    check_hresult(dcompDevice_->Commit());
}

CandidateWindow::~CandidateWindow(void) {}

// IUnknown
STDMETHODIMP
CandidateWindow::QueryInterface(REFIID riid, void **ppvObj) {
    if (!ppvObj) return E_INVALIDARG;

    if (IsEqualIID(riid, IID_IUnknown) ||
        IsEqualIID(riid, IID_ITfCandidateListUIElement)) {
        *ppvObj = (ITfCandidateListUIElement *)this;
    } else {
        *ppvObj = NULL;
    }

    if (!*ppvObj) {
        return E_NOINTERFACE;
    }

    AddRef();
    return S_OK;
}

STDMETHODIMP_(ULONG) CandidateWindow::AddRef(void) { return ++refCount_; }

STDMETHODIMP_(ULONG) CandidateWindow::Release(void) {
    assert(refCount_ > 0);
    const ULONG newCount = --refCount_;
    if (refCount_ == 0) delete this;
    return newCount;
}

// ITfUIElement
STDMETHODIMP
CandidateWindow::GetDescription(BSTR *pbstrDescription) {
    if (!pbstrDescription) return E_INVALIDARG;
    *pbstrDescription = SysAllocString(L"Candidate window~");
    return S_OK;
}

// {BD7CCC94-57CD-41D3-A789-AF47890CEB29}
STDMETHODIMP
CandidateWindow::GetGUID(GUID *pguid) {
    if (!pguid) return E_INVALIDARG;
    *pguid = {0xbd7ccc94,
              0x57cd,
              0x41d3,
              {0xa7, 0x89, 0xaf, 0x47, 0x89, 0xc, 0xeb, 0x29}};
    return S_OK;
}

STDMETHODIMP
CandidateWindow::Show(BOOL bShow) {
    shown_ = bShow;
    if (shown_)
        show();
    else
        hide();
    return S_OK;
}

STDMETHODIMP
CandidateWindow::IsShown(BOOL *pbShow) {
    if (!pbShow) return E_INVALIDARG;
    *pbShow = shown_;
    return S_OK;
}

// ITfCandidateListUIElement
STDMETHODIMP
CandidateWindow::GetUpdatedFlags(DWORD *pdwFlags) {
    if (!pdwFlags) return E_INVALIDARG;
    /// XXX update all!!!
    *pdwFlags = TF_CLUIE_DOCUMENTMGR | TF_CLUIE_COUNT | TF_CLUIE_SELECTION |
                TF_CLUIE_STRING | TF_CLUIE_PAGEINDEX | TF_CLUIE_CURRENTPAGE;
    return S_OK;
}

STDMETHODIMP
CandidateWindow::GetDocumentMgr(ITfDocumentMgr **ppdim) {
    if (!textService_) return E_FAIL;
    return textService_->currentContext()->GetDocumentMgr(ppdim);
}

STDMETHODIMP
CandidateWindow::GetCount(UINT *puCount) {
    if (!puCount) return E_INVALIDARG;
    *puCount = std::min<UINT>(10, items_.size());
    return S_OK;
}

STDMETHODIMP
CandidateWindow::GetSelection(UINT *puIndex) {
    assert(currentSel_ >= 0);
    if (!puIndex) return E_INVALIDARG;
    *puIndex = static_cast<UINT>(currentSel_);
    return S_OK;
}

STDMETHODIMP
CandidateWindow::GetString(UINT uIndex, BSTR *pbstr) {
    if (!pbstr) return E_INVALIDARG;
    if (uIndex >= items_.size()) return E_INVALIDARG;
    *pbstr = SysAllocString(items_[uIndex].c_str());
    return S_OK;
}

STDMETHODIMP
CandidateWindow::GetPageIndex(UINT *puIndex, UINT uSize, UINT *puPageCnt) {
    /// XXX Always return the same single page index.
    if (!puPageCnt) return E_INVALIDARG;
    *puPageCnt = 1;
    if (puIndex) {
        if (uSize < *puPageCnt) {
            return E_INVALIDARG;
        }
        puIndex[0] = 0;
    }
    return S_OK;
}

STDMETHODIMP
CandidateWindow::SetPageIndex(UINT *puIndex, UINT uPageCnt) {
    /// XXX Do not let app set page indices.
    if (!puIndex) return E_INVALIDARG;
    return S_OK;
}

STDMETHODIMP
CandidateWindow::GetCurrentPage(UINT *puPage) {
    if (!puPage) return E_INVALIDARG;
    *puPage = 0;
    return S_OK;
}

LRESULT
CandidateWindow::wndProc(UINT msg, WPARAM wp, LPARAM lp) {
    switch (msg) {
        case WM_PAINT:
            onPaint(wp, lp);
            break;
        case WM_ERASEBKGND:
            return TRUE;
            break;
        case WM_LBUTTONDOWN:
            onLButtonDown(wp, lp);
            break;
        case WM_MOUSEMOVE:
            onMouseMove(wp, lp);
            break;
        case WM_LBUTTONUP:
            onLButtonUp(wp, lp);
            break;
        case WM_MOUSEACTIVATE:
            return MA_NOACTIVATE;
        default:
            return Window::wndProc(msg, wp, lp);
    }
    return 0;
}

void CandidateWindow::onPaint(WPARAM wp, LPARAM lp) {
    RECT rc;
    GetClientRect(hwnd_, &rc);

    target_->BeginDraw();
    // paint window background and border
    // draw a flat black border in Windows 8 app immersive mode
    // draw a 3d border in desktop mode
    if (isImmersive()) {
        com_ptr<ID2D1SolidColorBrush> pBrush;
        check_hresult(target_->CreateSolidColorBrush(
            D2D1::ColorF(D2D1::ColorF::Black), pBrush.put()));
        target_->Clear(D2D1::ColorF(D2D1::ColorF::White));
        target_->DrawRectangle(
            D2D1::RectF(rc.left, rc.top, rc.right, rc.bottom), pBrush.get(),
            3.0f);
    } else {
        // ::FillSolidRectD2D(target_.get(), rc.left, rc.top, rc.right -
        // rc.left,
        //                    rc.bottom - rc.top, GetSysColor(COLOR_WINDOW));
        // ::Draw3DBorderD2D(target_.get(), &rc, GetSysColor(COLOR_3DFACE), 0,
        // 1);
        ninePatch_.DrawBitmap(
            target_.get(), D2D1::RectF(rc.left, rc.top, rc.right, rc.bottom));
    }

    // paint items
    int col = 0;
    int x = margin_, y = margin_;
    for (int i = 0, n = items_.size(); i < n; ++i) {
        paintItemD2D(target_.get(), i, x, y);
        ++col;  // go to next column
        if (col >= candPerRow_) {
            col = 0;
            x = margin_;
            y += itemHeight_ + rowSpacing_;
        } else {
            x += colSpacing_ + selKeyWidth_ + textWidth_;
        }
    }
    check_hresult(target_->EndDraw());
    check_hresult(swapChain_->Present(1, 0));
    ValidateRect(hwnd_, nullptr);
}

void CandidateWindow::recalculateSize() {
    if (items_.empty()) {
        resize(margin_ * 2, margin_ * 2);
        resizeSwapChain(margin_ * 2, margin_ * 2);
    }

    RECT rc;
    GetClientRect(hwnd_, &rc);

    com_ptr<ID2D1Factory> pD2DFactory;
    check_hresult(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED,
                                    pD2DFactory.put()));

    com_ptr<IDWriteFactory1> pDwriteFactory;
    com_ptr<IDWriteTextFormat> pTextFormat;
    check_hresult(DWriteCreateFactory(
        DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory1),
        reinterpret_cast<IUnknown **>(pDwriteFactory.put())));
    check_hresult(pDwriteFactory->CreateTextFormat(
        L"Segoe UI", nullptr, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, fontSize_, L"",
        pTextFormat.put()));

    int height = 0;
    int width = 0;
    selKeyWidth_ = 0;
    textWidth_ = 0;
    itemHeight_ = 0;
    com_ptr<IDWriteTextLayout> pTextLayout;
    DWRITE_TEXT_METRICS selKeyMetrics;
    DWRITE_TEXT_METRICS itemMetrics;

    vector<wstring>::const_iterator it;
    for (int i = 0, n = items_.size(); i < n; ++i) {
        int lineHeight = 0;
        // the selection key string
        wchar_t selKey[] = L"?. ";
        selKey[0] = selKeys_[i];
        pTextLayout = nullptr;
        pDwriteFactory->CreateTextLayout(selKey, 3, pTextFormat.get(),
                                         D2D1::FloatMax(), D2D1::FloatMax(),
                                         pTextLayout.put());
        pTextLayout->GetMetrics(&selKeyMetrics);
        if (selKeyMetrics.widthIncludingTrailingWhitespace > selKeyWidth_) {
            selKeyWidth_ = selKeyMetrics.widthIncludingTrailingWhitespace;
        }

        // the candidate string
        wstring &item = items_.at(i);
        pTextLayout = nullptr;
        pDwriteFactory->CreateTextLayout(item.c_str(), item.length(),
                                         pTextFormat.get(), D2D1::FloatMax(),
                                         D2D1::FloatMax(), pTextLayout.put());
        pTextLayout->GetMetrics(&itemMetrics);
        if (itemMetrics.widthIncludingTrailingWhitespace > textWidth_)
            textWidth_ = itemMetrics.widthIncludingTrailingWhitespace;
        int itemHeight = max(itemMetrics.height, selKeyMetrics.height);
        if (itemHeight > itemHeight_) itemHeight_ = itemHeight;
    }

    if (items_.size() <= candPerRow_) {
        width = items_.size() * (selKeyWidth_ + textWidth_);
        width += colSpacing_ * (items_.size() - 1);
        width += margin_ * 2;
        height = itemHeight_ + margin_ * 2;
    } else {
        width = candPerRow_ * (selKeyWidth_ + textWidth_);
        width += colSpacing_ * (candPerRow_ - 1);
        width += margin_ * 2;
        int rowCount = items_.size() / candPerRow_;
        if (items_.size() % candPerRow_) ++rowCount;
        height =
            itemHeight_ * rowCount + rowSpacing_ * (rowCount - 1) + margin_ * 2;
    }
    resize(width, height);
    resizeSwapChain(width, height);
}

void CandidateWindow::resizeSwapChain(int width, int height) {
    com_ptr<IDXGISurface> surface;
    com_ptr<ID2D1Bitmap1> bitmap;

    target_->SetTarget(nullptr);
    swapChain_->ResizeBuffers(0, width, height, DXGI_FORMAT_B8G8R8A8_UNORM, 0);
    check_hresult(
        swapChain_->GetBuffer(0, __uuidof(surface), surface.put_void()));
    auto bitmap_props = D2D1::BitmapProperties1(
        D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM,
                          D2D1_ALPHA_MODE_PREMULTIPLIED));
    check_hresult(target_->CreateBitmapFromDxgiSurface(
        surface.get(), &bitmap_props, bitmap.put()));
    target_->SetTarget(bitmap.get());
}

void CandidateWindow::setCandPerRow(int n) {
    if (n != candPerRow_) {
        candPerRow_ = n;
        recalculateSize();
    }
}

bool CandidateWindow::filterKeyEvent(KeyEvent &keyEvent) {
    // select item with arrow keys
    int oldSel = currentSel_;
    switch (keyEvent.keyCode()) {
        case VK_UP:
            if (currentSel_ - candPerRow_ >= 0) currentSel_ -= candPerRow_;
            break;
        case VK_DOWN:
            if (currentSel_ + candPerRow_ < items_.size())
                currentSel_ += candPerRow_;
            break;
        case VK_LEFT:
            if (currentSel_ - 1 >= 0) --currentSel_;
            break;
        case VK_RIGHT:
            if (currentSel_ + 1 < items_.size()) ++currentSel_;
            break;
        case VK_RETURN:
            hasResult_ = true;
            return true;
        default:
            return false;
    }
    // if currently selected item is changed, redraw
    if (currentSel_ != oldSel) {
        // repaint the old and new items
        RECT rect;
        itemRect(oldSel, rect);
        ::InvalidateRect(hwnd_, &rect, TRUE);
        itemRect(currentSel_, rect);
        ::InvalidateRect(hwnd_, &rect, TRUE);
        return true;
    }
    return false;
}

void CandidateWindow::setCurrentSel(int sel) {
    if (sel >= items_.size()) sel = 0;
    if (currentSel_ != sel) {
        currentSel_ = sel;
        if (isVisible()) ::InvalidateRect(hwnd_, NULL, TRUE);
    }
}

void CandidateWindow::clear() {
    items_.clear();
    selKeys_.clear();
    currentSel_ = 0;
    hasResult_ = false;
}

void CandidateWindow::setUseCursor(bool use) {
    useCursor_ = use;
    if (isVisible()) ::InvalidateRect(hwnd_, NULL, TRUE);
}

void CandidateWindow::paintItemD2D(ID2D1RenderTarget *pRenderTarget, int i,
                                   int x, int y) {
    com_ptr<IDWriteFactory1> pDwriteFactory;
    com_ptr<IDWriteTextFormat> pTextFormat;
    com_ptr<ID2D1SolidColorBrush> pSelKeyBrush;
    com_ptr<ID2D1SolidColorBrush> pTextBrush;
    com_ptr<ID2D1SolidColorBrush> pSelectedTextBrush;
    DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory1),
                        reinterpret_cast<IUnknown **>(pDwriteFactory.put()));
    pDwriteFactory->CreateTextFormat(
        L"Segoe UI", nullptr, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, fontSize_, L"",
        pTextFormat.put());

    RECT textRect = {x, y, 0, y + itemHeight_};
    wchar_t selKey[] = L"?. ";
    selKey[0] = selKeys_[i];
    textRect.right = textRect.left + selKeyWidth_;

    // FIXME: make the color of strings configurable.
    COLORREF selKeyColor = RGB(255, 0, 0);
    COLORREF textColor = GetSysColor(COLOR_WINDOWTEXT);
    pRenderTarget->CreateSolidColorBrush(D2D1::ColorF(selKeyColor),
                                         pSelKeyBrush.put());
    pRenderTarget->CreateSolidColorBrush(D2D1::ColorF(textColor),
                                         pTextBrush.put());
    pRenderTarget->CreateSolidColorBrush(D2D1::ColorF(D2D1::ColorF::White),
                                         pSelectedTextBrush.put());

    pRenderTarget->DrawText(selKey, 3, pTextFormat.get(),
                            D2D1::RectF(textRect.left, textRect.top,
                                        textRect.right, textRect.bottom),
                            pSelKeyBrush.get());

    wstring &item = items_.at(i);
    textRect.left += selKeyWidth_;
    textRect.right = textRect.left + textWidth_;

    // invert the selected item
    if (useCursor_ && i == currentSel_) {
        pRenderTarget->FillRectangle(
            D2D1::RectF(textRect.left, textRect.top, textRect.right,
                        textRect.bottom),
            pTextBrush.get());
        pRenderTarget->DrawText(item.c_str(), item.length(), pTextFormat.get(),
                                D2D1::RectF(textRect.left, textRect.top,
                                            textRect.right, textRect.bottom),
                                pSelectedTextBrush.get());
    } else {
        pRenderTarget->DrawText(item.c_str(), item.length(), pTextFormat.get(),
                                D2D1::RectF(textRect.left, textRect.top,
                                            textRect.right, textRect.bottom),
                                pTextBrush.get());
    }
}

void CandidateWindow::itemRect(int i, RECT &rect) {
    int row, col;
    row = i / candPerRow_;
    col = i % candPerRow_;
    rect.left = margin_ + col * (selKeyWidth_ + textWidth_ + colSpacing_);
    rect.top = margin_ + row * (itemHeight_ + rowSpacing_);
    rect.right = rect.left + (selKeyWidth_ + textWidth_);
    rect.bottom = rect.top + itemHeight_;
}

}  // namespace Ime
