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

#include "LangBarButton.h"
#include "TextService.h"
#include <OleCtl.h>
#include <assert.h>
#include <minwindef.h>
#include <stdlib.h>

extern HINSTANCE g_hInstance;

namespace Ime {

LangBarButton::LangBarButton(TextService* service, const GUID& guid, UINT commandId, const wchar_t* text, DWORD style):
	textService_(service),
	tooltip_(),
	commandId_(commandId),
	menu_(NULL),
	icon_(NULL),
	status_(0),
	refCount_(1) {

	assert(service);

	textService_->AddRef();
	info_.clsidService = service->clsid();
	info_.guidItem = guid;
	info_.dwStyle = style;
	info_.ulSort = 0;
	setText(text);
}

LangBarButton::~LangBarButton(void) {
	if(textService_)
		textService_->Release();
	if(menu_)
		::DestroyMenu(menu_);
}

const wchar_t* LangBarButton::text() const {
	return info_.szDescription;
}

void LangBarButton::setText(const wchar_t* text) {
	if (text && text[0] != '\0') {
		wcsncpy(info_.szDescription, text, TF_LBI_DESC_MAXLEN - 1);
		info_.szDescription[TF_LBI_DESC_MAXLEN - 1] = 0;
	}
	else {
		// NOTE: The language button text should NOT be empty.
		// Otherwise, when the button status or icon is changed after its creation,
		// the button will disappear temporarily in Windows 10 for unknown reason.
		// This can be considered a bug of Windows 10 and there does not seem to be a way to fix it.
		// So we need to avoid empty button text otherwise the language button won't work properly.
		// Here we use a space character to make the text non-empty to workaround the problem.
		wcscpy(info_.szDescription, L" ");
	}
	update(TF_LBI_TEXT);
}

void LangBarButton::setText(UINT stringId) {
	const wchar_t* str;
	int len = ::LoadStringW(g_hInstance, stringId, (LPTSTR)&str, 0);
	if(str) {
		if(len > (TF_LBI_DESC_MAXLEN - 1))
			len = TF_LBI_DESC_MAXLEN - 1;
		wcsncpy(info_.szDescription, str, len);
		info_.szDescription[len] = 0;
		update(TF_LBI_TEXT);
	}
}

// public methods
const wchar_t* LangBarButton::tooltip() const {
	return tooltip_.c_str();
}

void LangBarButton::setTooltip(const wchar_t* tooltip) {
	tooltip_ = tooltip;
	update(TF_LBI_TOOLTIP);
}

void LangBarButton::setTooltip(UINT tooltipId) {
	const wchar_t* str;
	//  If this parameter is 0, then lpBuffer receives a read-only pointer to the resource itself.
	auto len = ::LoadStringW(g_hInstance, tooltipId, (LPTSTR)&str, 0);
	if(str) {
		tooltip_ = std::wstring(str, len);
		update(TF_LBI_TOOLTIP);
	}
}

HICON LangBarButton::icon() const {
	return icon_;
}

// The language button does not take owner ship of the icon
// That means, when the button is destroyed, it will not destroy
// the icon automatically.
void LangBarButton::setIcon(HICON icon) {
	icon_ = icon;
	update(TF_LBI_ICON);
}

void LangBarButton::setIcon(UINT iconId) {
	HICON icon = ::LoadIconW(g_hInstance, (LPCTSTR)iconId);
	if(icon)
		setIcon(icon);
}

UINT LangBarButton::commandId() const {
	return commandId_;
}

void LangBarButton::setCommandId(UINT id) {
	commandId_ = id;
}

HMENU LangBarButton::menu() const {
	return menu_;
}

void LangBarButton::setMenu(HMENU menu) {
	if(menu_) {
		::DestroyMenu(menu_);
	}
	menu_ = menu;
	// FIXME: how to handle toggle buttons?
	if(menu)
		info_.dwStyle = TF_LBI_STYLE_BTN_MENU;
	else
		info_.dwStyle = TF_LBI_STYLE_BTN_BUTTON;
}

bool LangBarButton::enabled() const {
	return !(status_ & TF_LBI_STATUS_DISABLED);
}

void LangBarButton::setEnabled(bool enable) {
	if(enabled() != enable) {
		if(enable)
			status_ &= ~TF_LBI_STATUS_DISABLED;
		else
			status_ |= TF_LBI_STATUS_DISABLED;
		update(TF_LBI_STATUS);
	}
}

// need to create the button with TF_LBI_STYLE_BTN_TOGGLE style
bool LangBarButton::toggled() const {
	return (status_ & TF_LBI_STATUS_BTN_TOGGLED) ? true : false;
}

void LangBarButton::setToggled(bool toggle) {
	if(toggled() != toggle) {
		if(toggle)
			status_ |= TF_LBI_STATUS_BTN_TOGGLED;
		else
			status_ &= ~TF_LBI_STATUS_BTN_TOGGLED;
		update(TF_LBI_STATUS);
	}
}


DWORD LangBarButton::style() const {
	return info_.dwStyle;
}

void LangBarButton::setStyle(DWORD style) {
	info_.dwStyle = style;
}


// COM stuff

// IUnknown
STDMETHODIMP LangBarButton::QueryInterface(REFIID riid, void **ppvObj) {
    if (ppvObj == NULL)
        return E_INVALIDARG;

	if(IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_ITfLangBarItem) || IsEqualIID(riid, IID_ITfLangBarItemButton))
		*ppvObj = (ITfLangBarItemButton*)this;
	else if(IsEqualIID(riid, IID_ITfSource))
		*ppvObj = (ITfSource*)this;
	else
		*ppvObj = NULL;

	if(*ppvObj) {
		AddRef();
		return S_OK;
	}
	return E_NOINTERFACE;
}

// IUnknown implementation
STDMETHODIMP_(ULONG) LangBarButton::AddRef(void) {
	return ++refCount_;
}

STDMETHODIMP_(ULONG) LangBarButton::Release(void) {
	assert(refCount_ > 0);
	const ULONG newCount = --refCount_;
	if (0 == refCount_)
		delete this;
	return newCount;
}

// ITfLangBarItem
STDMETHODIMP LangBarButton::GetInfo(TF_LANGBARITEMINFO *pInfo) {
	*pInfo = info_;
	return S_OK;
}

STDMETHODIMP LangBarButton::GetStatus(DWORD *pdwStatus) {
	*pdwStatus = status_;
	return S_OK;
}

STDMETHODIMP LangBarButton::Show(BOOL fShow) {
	return E_NOTIMPL;
}

STDMETHODIMP LangBarButton::GetTooltipString(BSTR *pbstrToolTip) {
	*pbstrToolTip = ::SysAllocString(tooltip_.c_str());
	return *pbstrToolTip ? S_OK : E_FAIL;
}

// ITfLangBarItemButton
STDMETHODIMP LangBarButton::OnClick(TfLBIClick click, POINT pt, const RECT *prcArea) {
	TextService::CommandType type;
	if(click == TF_LBI_CLK_RIGHT)
		type = TextService::COMMAND_RIGHT_CLICK;
	else
		type = TextService::COMMAND_LEFT_CLICK;
	textService_->onCommand(commandId_, type);
	return S_OK;
}

STDMETHODIMP LangBarButton::InitMenu(ITfMenu *pMenu) {
	if(!menu_)
		return E_FAIL;
	buildITfMenu(pMenu, menu_);
	return S_OK;
}

STDMETHODIMP LangBarButton::OnMenuSelect(UINT wID) {
	textService_->onCommand(wID, TextService::COMMAND_MENU);
	return S_OK;
}

STDMETHODIMP LangBarButton::GetIcon(HICON *phIcon) {
	// https://msdn.microsoft.com/zh-tw/library/windows/desktop/ms628718%28v=vs.85%29.aspx
	// The caller will delete the icon when it's no longer needed.
	// However, we might still need it. So let's return a copy here.
	*phIcon = (HICON)CopyImage(icon_, IMAGE_ICON, 0, 0, 0);
	return S_OK;
}

STDMETHODIMP LangBarButton::GetText(BSTR *pbstrText) {
	*pbstrText = ::SysAllocString(info_.szDescription);
	return *pbstrText ? S_OK : S_FALSE;
}

// ITfSource
STDMETHODIMP LangBarButton::AdviseSink(REFIID riid, IUnknown *punk, DWORD *pdwCookie) {
    if(IsEqualIID(riid, IID_ITfLangBarItemSink)) {
		ITfLangBarItemSink* langBarItemSink;
		if(punk->QueryInterface(IID_ITfLangBarItemSink, (void **)&langBarItemSink) == S_OK) {
		    *pdwCookie = (DWORD)rand();
			sinks_[*pdwCookie] = langBarItemSink;
			return S_OK;
		}
		else
			return E_NOINTERFACE;
	}
    return CONNECT_E_CANNOTCONNECT;
}

STDMETHODIMP LangBarButton::UnadviseSink(DWORD dwCookie) {
	std::map<DWORD, ITfLangBarItemSink*>::iterator it = sinks_.find(dwCookie);
	if(it != sinks_.end()) {
		ITfLangBarItemSink* langBarItemSink = (ITfLangBarItemSink*)it->second;
		langBarItemSink->Release();
		sinks_.erase(it);
		return S_OK;
	}
	return CONNECT_E_NOCONNECTION;
}


// build ITfMenu according to the content of HMENU
void LangBarButton::buildITfMenu(ITfMenu* menu, HMENU templ) {
	int n = ::GetMenuItemCount(templ);
	for(int i = 0; i < n; ++i) {
		MENUITEMINFO mi;
		wchar_t textBuffer[256];
		memset(&mi, 0, sizeof(mi));
		mi.cbSize = sizeof(mi);
		mi.dwTypeData = (LPTSTR)textBuffer;
		mi.cch = 255;
		mi.fMask = MIIM_FTYPE|MIIM_ID|MIIM_STATE|MIIM_STRING|MIIM_SUBMENU;
		if(::GetMenuItemInfoW(templ, i, TRUE, &mi)) {
			UINT flags = 0;
			wchar_t* text = nullptr;
			ULONG textLen = 0;
			ITfMenu* subMenu = NULL;
			ITfMenu** pSubMenu = NULL;
			if(mi.hSubMenu) { // has submenu
				pSubMenu = &subMenu;
				flags |= TF_LBMENUF_SUBMENU;
			}
			if(mi.fType == MFT_STRING) { // text item
				text = (wchar_t*)mi.dwTypeData;
				textLen = mi.cch;
			}
			else if(mi.fType == MFT_SEPARATOR) { // separator item
				flags |= TF_LBMENUF_SEPARATOR;
			}
			else // other types are not supported
				continue;

			if(mi.fState & MFS_CHECKED) // checked
				flags |= TF_LBMENUF_CHECKED;
			if(mi.fState & (MFS_GRAYED|MFS_DISABLED)) // disabled
				flags |= TF_LBMENUF_GRAYED;
			
			if(menu->AddMenuItem(mi.wID, flags, NULL, 0, text, textLen, pSubMenu) == S_OK) {
				if(subMenu) {
					buildITfMenu(subMenu, mi.hSubMenu);
					subMenu->Release();
				}
			}
		}
		else {
			DWORD error = ::GetLastError();
		}
	}
}

// call all sinks to generate update notifications
void LangBarButton::update(DWORD flags) {
	if(!sinks_.empty()) {
		std::map<DWORD, ITfLangBarItemSink*>::iterator it;
		for(it = sinks_.begin(); it != sinks_.end(); ++it) {
			ITfLangBarItemSink* sink = it->second;
			sink->OnUpdate(flags);
		}
	}
}

} // namespace Ime

