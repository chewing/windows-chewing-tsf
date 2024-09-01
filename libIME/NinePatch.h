#pragma once

#include <Unknwn.h>
#include <d2d1.h>
#include <d2d1_1.h>
#include <minwindef.h>
#include <wincodec.h>
#include <winrt/base.h>

#include <string>

#include "rustlib_bridge/lib.h"

class NinePatch {
   public:
    NinePatch(std::wstring image_path);
    ~NinePatch();

    HRESULT DrawBitmap(ID2D1DeviceContext *dc, D2D1_RECT_F rect);
    FLOAT GetMargin();

   private:
    std::wstring image_path_;
    winrt::com_ptr<IWICBitmap> bitmap_;
    rust::Box<NinePatchDrawable> ninePatch_;
};