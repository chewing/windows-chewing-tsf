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

#ifndef IME_MESSAGE_WINDOW_H
#define IME_MESSAGE_WINDOW_H

#include <Unknwn.h>
#include <d2d1_1.h>
#include <d3d11_1.h>
#include <winrt/base.h>

#include <string>

#include "EditSession.h"
#include "ImeWindow.h"

namespace Ime {

class TextService;

class MessageWindow : public ImeWindow {
   public:
    MessageWindow(TextService* service, EditSession* session = NULL);
    virtual ~MessageWindow(void);

    std::wstring text() { return text_; }
    void setText(std::wstring text);

    TextService* textService() { return textService_; }

    virtual void recalculateSize();

   protected:
    LRESULT wndProc(UINT msg, WPARAM wp, LPARAM lp);
    void onPaint(WPARAM wp, LPARAM lp);
    void resizeSwapChain(int width, int height);

   private:
    winrt::com_ptr<ID2D1DeviceContext> target_;
    winrt::com_ptr<IDXGISwapChain1> swapChain_;
    winrt::com_ptr<ID2D1Factory1> factory_;

    std::wstring text_;
};

}  // namespace Ime

#endif
