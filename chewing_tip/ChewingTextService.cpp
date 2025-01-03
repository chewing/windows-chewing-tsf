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

#include "ChewingTextService.h"

#include <Windows.h>
#include <VersionHelpers.h>

#include <Shellapi.h>
#include <assert.h>
#include <corecrt_wstring.h>
#include <ctfutb.h>
#include <debugapi.h>
#include <libloaderapi.h>
#include <minwindef.h>
#include <oleauto.h>
#include <shlobj_core.h>
#include <sys/stat.h>
#include <winerror.h>
#include <winnt.h>
#include <winreg.h>
#include <winrt/base.h>
#include <winuser.h>

#include <cstddef>
#include <string>

#include "TextService.h"
#include "Utils.h"
#include "resource.h"
#include "libime2.h"


using namespace std;

extern HINSTANCE g_hInstance;

namespace Chewing {

// {B59D51B9-B832-40D2-9A8D-56959372DDC7}
static const GUID g_modeButtonGuid = // English/Chinses mode switch
{ 0xb59d51b9, 0xb832, 0x40d2, { 0x9a, 0x8d, 0x56, 0x95, 0x93, 0x72, 0xdd, 0xc7 } };

// {5325DBF5-5FBE-467B-ADF0-2395BE9DD2BB}
static const GUID g_shapeTypeButtonGuid = // half shape/full shape switch
{ 0x5325dbf5, 0x5fbe, 0x467b, { 0xad, 0xf0, 0x23, 0x95, 0xbe, 0x9d, 0xd2, 0xbb } };

// {4FAFA520-2104-407E-A532-9F1AAB7751CD}
static const GUID g_settingsButtonGuid = // settings button/menu
{ 0x4fafa520, 0x2104, 0x407e, { 0xa5, 0x32, 0x9f, 0x1a, 0xab, 0x77, 0x51, 0xcd } };

// {C77A44F5-DB21-474E-A2A2-A17242217AB3}
static const GUID g_shiftSpaceGuid = // shift + space
{ 0xc77a44f5, 0xdb21, 0x474e, { 0xa2, 0xa2, 0xa1, 0x72, 0x42, 0x21, 0x7a, 0xb3 } };

// this is the GUID of the IME mode icon in Windows 8
// the value is not available in older SDK versions, so let's define it ourselves.
static const GUID _GUID_LBI_INPUTMODE =
{ 0x2C77A81E, 0x41CC, 0x4178, { 0xA3, 0xA7, 0x5F, 0x8A, 0x98, 0x75, 0x68, 0xE6 } };

// CLSID of our Text service
// {13F2EF08-575C-4D8C-88E0-F67BB8052B84}
const CLSID g_textServiceClsid =
{ 0x13f2ef08, 0x575c, 0x4d8c, { 0x88, 0xe0, 0xf6, 0x7b, 0xb8, 0x5, 0x2b, 0x84 } };

TextService::TextService():
	Ime::TextService(),
	showingCandidates_(false),
	langMode_(-1),
	shapeMode_(-1),
	outputSimpChinese_(false),
	lastKeyDownCode_(0),
	messageTimerId_(0),
	symbolsFileTime_(0),
	chewingContext_(NULL) {

	OutputDebugStringW(L"[chewing] Load config and start watching changes\n");
	config_.load();
	config_.watchChanges();

	// add preserved keys
	addPreservedKey(VK_SPACE, TF_MOD_SHIFT, g_shiftSpaceGuid); // shift + space
}

TextService::~TextService(void) {
	if(popupMenu_)
		::DestroyMenu(popupMenu_);

	if(messageWindow_)
		hideMessage();

	freeChewingContext();
	OutputDebugStringW(L"[chewing] Unloaded\n");
}

// virtual
void TextService::onActivate() {
	// add language bar buttons
	// siwtch Chinese/English modes
	TF_LANGBARITEMINFO info = {
		g_textServiceClsid,
		g_modeButtonGuid,
		TF_LBI_STYLE_BTN_BUTTON,
		0,
		{}
	};
	LPCWSTR tooltip;
	int len;
	len = LoadStringW(g_hInstance, IDS_SWITCH_LANG, (LPWSTR)&tooltip, 0);
	wcsncpy_s(info.szDescription, sizeof(info.szDescription), tooltip, len);
	CreateLangBarButton(
		info,
		SysAllocStringLen(tooltip, len),
		LoadIconW(g_hInstance, MAKEINTRESOURCEW(IDI_CHI)),
		NULL,
		ID_SWITCH_LANG,
		this,
		switchLangButton_.put_void()
	);
	addButton(switchLangButton_.get());

	// toggle full shape/half shape
	len = LoadStringW(g_hInstance, IDS_SWITCH_SHAPE, (LPWSTR)&tooltip, 0);
	info.guidItem = g_shapeTypeButtonGuid;
	wcsncpy_s(info.szDescription, sizeof(info.szDescription), tooltip, len);
	CreateLangBarButton(
		info,
		SysAllocStringLen(tooltip, len),
		LoadIconW(g_hInstance, MAKEINTRESOURCEW(IDI_HALF_SHAPE)),
		NULL,
		ID_SWITCH_SHAPE,
		this,
		switchShapeButton_.put_void()
	);
	addButton(switchShapeButton_.get());

	// settings and others, may open a popup menu
	len = LoadStringW(g_hInstance, IDS_SETTINGS, (LPWSTR)&tooltip, 0);
	info.guidItem = g_settingsButtonGuid;
	info.dwStyle = TF_LBI_STYLE_BTN_MENU;
	wcsncpy_s(info.szDescription, sizeof(info.szDescription), tooltip, len);
	HMENU menu = ::LoadMenuW(g_hInstance, MAKEINTRESOURCEW(IDR_MENU));
	popupMenu_ = ::GetSubMenu(menu, 0);
	CreateLangBarButton(
		info,
		SysAllocStringLen(tooltip, len),
		LoadIconW(g_hInstance, MAKEINTRESOURCEW(IDI_CONFIG)),
		popupMenu_,
		0,
		this,
		settingsMenuButton_.put_void()
	);
	addButton(settingsMenuButton_.get());

	// Windows 8 systray IME mode icon
	if(IsWindows8OrGreater()) {
		len = LoadStringW(g_hInstance, IDS_SWITCH_SHAPE, (LPWSTR)&tooltip, 0);
		info.guidItem = _GUID_LBI_INPUTMODE;
		info.dwStyle = TF_LBI_STYLE_BTN_BUTTON;
		wcsncpy_s(info.szDescription, sizeof(info.szDescription), tooltip, len);
		CreateLangBarButton(
			info,
			SysAllocStringLen(tooltip, len),
			LoadIconW(g_hInstance, MAKEINTRESOURCEW(isLightTheme() ? IDI_ENG : IDI_ENG_DARK)),
			NULL,
			ID_MODE_ICON,
			this,
			imeModeIcon_.put_void()
		);
		addButton(imeModeIcon_.get());
	}

	config().reloadIfNeeded();
	initChewingContext();
	updateLangButtons();

	if(imeModeIcon_) // windows 8 IME mode icon
		imeModeIcon_->setEnabled(isKeyboardOpened());
}

// virtual
void TextService::onDeactivate() {
	// Remove all buttons to avoid reference cycles
	switchLangButton_ = nullptr;
	switchShapeButton_ = nullptr;
	settingsMenuButton_ = nullptr;
	imeModeIcon_ = nullptr;

	lastKeyDownCode_ = 0;
	freeChewingContext();

	hideMessage();
	hideCandidates();
}

// virtual
void TextService::onKillFocus() {
	if (isComposing()) {
		// end current composition if needed
		ITfContext* context = currentContext();
		if (context) {
			endComposition(context);
			context->Release();
		}
	}
	hideCandidates();
	hideMessage();
}

// virtual
bool TextService::filterKeyDown(Ime::KeyEvent& keyEvent) {
	Config& cfg = config();
	if (cfg.reloadIfNeeded()) {
		// check if chewing context needs to be reloaded
		bool chewingNeedsReload = false;
		// check if symbols.dat file is changed
		// get last mtime of symbols.dat file
		std::wstring file = userDir() + L"\\symbols.dat";
		struct _stat64 stbuf;
		if(_wstat64(file.c_str(), &stbuf) == 0 && symbolsFileTime_ != stbuf.st_mtime) {
			symbolsFileTime_ = stbuf.st_mtime;
			chewingNeedsReload = true;
		}
		// re-create a new chewing context if needed
		if(chewingNeedsReload && chewingContext_) {
			freeChewingContext();
			initChewingContext();
		}
		else {
			applyConfig(); // apply the latest config
		}
	}
	lastKeyDownCode_ = keyEvent.keyCode();
	// return false if we don't need this key
	assert(chewingContext_);
	if(!isComposing()) { // we're not composing now
		// don't do further handling in English + half shape mode
		if(langMode_ != CHINESE_MODE && shapeMode_ != FULLSHAPE_MODE)
			return false;

		// if Ctrl or Alt key is down
		if(keyEvent.isKeyDown(VK_CONTROL) || keyEvent.isKeyDown(VK_MENU)) {
			// bypass IME. This might be a shortcut key used in the application
			// FIXME: we only need Ctrl in composition mode for adding user phrases.
			// However, if we turn on easy symbol input with Ctrl support later,
			// we'll need th Ctrl key then.
			return false;
		}

		// we always need further processing in full shape mode since all English chars,
		// numbers, and symbols need to be converted to full shape Chinese chars.
		if(shapeMode_ != FULLSHAPE_MODE) {
			// Caps lock is on => English mode
			if(cfg.enableCapsLock && keyEvent.isKeyToggled(VK_CAPITAL)) {
				// We need to handle this key because in onKeyDown(),
				// the upper case chars need to be converted to lower case
				// before doing output to the applications.
				if(keyEvent.isChar() && isalpha(keyEvent.charCode()))
					return true; // this is an English alphabet
				else
					return false;
			}

			if(keyEvent.isKeyToggled(VK_NUMLOCK)) { // NumLock is on
				// if this key is Num pad 0-9, +, -, *, /, pass it back to the system
				if(keyEvent.keyCode() >= VK_NUMPAD0 && keyEvent.keyCode() <= VK_DIVIDE)
					return false; // bypass IME
			}
		}
		else { // full shape mode
			if(keyEvent.keyCode() == VK_SPACE) // we need to convert space to fullshape.
				return true;
		}

		// when not composing, we only cares about Bopomofo
		// FIXME: we should check if the key is mapped to a phonetic symbol instead
		if(keyEvent.isChar() && isgraph(keyEvent.charCode())) {
			// this is a key mapped to a printable char. we want it!
			return true;
		}
		return false;
	}
	return true;
}

// virtual
bool TextService::onKeyDown(Ime::KeyEvent& keyEvent, Ime::EditSession* session) {
	assert(chewingContext_);
	Config& cfg = config();
#if 0 // What's easy symbol input??
	// set this to true or false according to the status of Shift key
	// alternatively, should we set this when onKeyDown and onKeyUp receive VK_SHIFT or VK_CONTROL?
	bool easySymbols = false;
	if(cfg.easySymbolsWithShift)
		easySymbols = keyEvent.isKeyDown(VK_SHIFT);
	if(!easySymbols && cfg.easySymbolsWithCtrl)
		easySymbols = keyEvent.isKeyDown(VK_CONTROL);
	::chewing_set_easySymbolInput(chewingContext_, easySymbols);
#endif

	UINT charCode = keyEvent.charCode();
	if(charCode && isprint(charCode)) { // printable characters (exclude extended keys?)
		int oldLangMode = ::chewing_get_ChiEngMode(chewingContext_);
		bool temporaryEnglishMode = false;
		bool invertCase = false;
		// If Caps lock is on, temporarily change to English mode
		if(cfg.enableCapsLock && keyEvent.isKeyToggled(VK_CAPITAL)) {
			temporaryEnglishMode = true;
			invertCase = true; // need to convert upper case to lower, and vice versa.
		}
		// If Shift is pressed, but we don't want to enter full shape symbols
		if(keyEvent.isKeyDown(VK_SHIFT) && (!cfg.fullShapeSymbols || isalpha(charCode))) {
			temporaryEnglishMode = true;
			if(!cfg.upperCaseWithShift)
				invertCase = true; // need to convert upper case to lower, and vice versa.
		}

		if(langMode_ == SYMBOL_MODE) { // English mode
			::chewing_handle_Default(chewingContext_, charCode);
		}
		else if(temporaryEnglishMode) { // temporary English mode
			::chewing_set_ChiEngMode(chewingContext_, SYMBOL_MODE); // change to English mode temporarily
			if(invertCase) { // need to invert upper case and lower case
				// we're NOT in real English mode, but Capslock is on, so we treat it as English mode
				// reverse upper and lower case
				charCode = isupper(charCode) ? tolower(charCode) : toupper(charCode);
			}
			::chewing_handle_Default(chewingContext_, charCode);
			::chewing_set_ChiEngMode(chewingContext_, oldLangMode); // restore previous mode
		}
		else { // Chinese mode
			if(isalpha(charCode)) // alphabets: A-Z
				::chewing_handle_Default(chewingContext_, tolower(charCode));
			else if(keyEvent.keyCode() == VK_SPACE) // space key
				::chewing_handle_Space(chewingContext_);
			else if(keyEvent.isKeyDown(VK_CONTROL) && isdigit(charCode)) // Ctrl + number (0~9)
				::chewing_handle_CtrlNum(chewingContext_, charCode);
			else if(keyEvent.isKeyToggled(VK_NUMLOCK) && keyEvent.keyCode() >= VK_NUMPAD0 && keyEvent.keyCode() <= VK_DIVIDE)
				// numlock is on, handle numpad keys
				::chewing_handle_Numlock(chewingContext_, charCode);
			else { // other keys, no special handling is needed
				::chewing_handle_Default(chewingContext_, charCode);
			}
		}
	} else { // non-printable keys
		bool keyHandled = false;
		// if we want to use the arrow keys to select candidate strings
		if(config().cursorCandList && showingCandidates() && candidateWindow_) {
			// if the candidate window is open, let it handle the key first
			if(candidateWindow_->filterKeyEvent(keyEvent.keyCode())) {
				// the user selected a string from the candidate list already
				if(candidateWindow_->hasResult()) {
					wchar_t selKey = candidateWindow_->currentSelKey();
					// pass the selKey to libchewing.
					::chewing_handle_Default(chewingContext_, selKey);
					keyHandled = true;
				}
				else // no candidate has been choosen yet
					return true; // eat the key and don't pass it to libchewing at all
			}
		}

		if(!keyHandled) {
			// the candidate window does not need the key. pass it to libchewing.
			switch(keyEvent.keyCode()) {
			case VK_ESCAPE:
				::chewing_handle_Esc(chewingContext_);
				break;
			case VK_RETURN:
				::chewing_handle_Enter(chewingContext_);
				break;
			case VK_TAB:
				::chewing_handle_Tab(chewingContext_);
				break;
			case VK_DELETE:
				::chewing_handle_Del(chewingContext_);
				break;
			case VK_BACK:
				::chewing_handle_Backspace(chewingContext_);
				break;
			case VK_UP:
				::chewing_handle_Up(chewingContext_);
				break;
			case VK_DOWN:
				::chewing_handle_Down(chewingContext_);
				break;
			case VK_LEFT:
				::chewing_handle_Left(chewingContext_);
				break;
			case VK_RIGHT:
				::chewing_handle_Right(chewingContext_);
				break;
			case VK_HOME:
				::chewing_handle_Home(chewingContext_);
				break;
			case VK_END:
				::chewing_handle_End(chewingContext_);
				break;
			case VK_PRIOR:
				::chewing_handle_PageUp(chewingContext_);
				break;
			case VK_NEXT:
				::chewing_handle_PageDown(chewingContext_);
				break;
			default: // we don't know this key. ignore it!
				return false;
			}
		}
	}

	updateLangButtons();

	if(::chewing_keystroke_CheckIgnore(chewingContext_))
		return false;

	if(!isComposing()) // start the composition
		startComposition(session->context());

	// handle candidates
	if(hasCandidates()) {
		if(!showingCandidates())
			showCandidates(session);
		else
			updateCandidates(session);
	}
	else {
		if(showingCandidates())
			hideCandidates();
	}

	// has something to commit
	if(::chewing_commit_Check(chewingContext_)) {
		char* buf = ::chewing_commit_String(chewingContext_);
		std::wstring wbuf = utf8ToUtf16(buf);
		::chewing_free(buf);
		::chewing_ack(chewingContext_);

		// FIXME: this should be per-instance rather than a global setting.
		if(outputSimpChinese_) // convert output to simplified Chinese
			wbuf = tradToSimpChinese(wbuf);

		// commit the text, replace currently selected text with our commit string
		setCompositionString(session, wbuf.c_str(), wbuf.length());

		if(isComposing())
			endComposition(session->context());
	}

	wstring compositionBuf;
	if(::chewing_buffer_Check(chewingContext_)) {
		char* buf = ::chewing_buffer_String(chewingContext_);
		if(buf) {
			std::wstring wbuf = ::utf8ToUtf16(buf);
			::chewing_free(buf);
			compositionBuf += wbuf;
		}
	}

	if(::chewing_bopomofo_Check(chewingContext_)) {
		std::wstring wbuf = ::utf8ToUtf16(::chewing_bopomofo_String_static(chewingContext_));
		// put bopomofo symbols at insertion point
		// FIXME: alternatively, should we show it in an additional floating window?
		int pos = ::chewing_cursor_Current(chewingContext_);
		compositionBuf.insert(pos, wbuf);
	}

	// has something in composition buffer
	if(!compositionBuf.empty()) {
		// FIXME there's no need to start a new edit session
		if(!isComposing()) { // start the composition
			startComposition(session->context());
		}
		setCompositionString(session, compositionBuf.c_str(), compositionBuf.length());
	}
	else { // nothing left in composition buffer, terminate composition status
		if(isComposing()) {
			// clean composition before end it
			setCompositionString(session, compositionBuf.c_str(), compositionBuf.length());

			// We also need to make sure that the candidate window is not currently shown.
			// When typing symbols with ` key, it's possible that the composition string empty,
			// while the candidate window is shown. We should not terminate the composition in this case.
			if(!showingCandidates())
				endComposition(session->context());
		}
	}

	// update cursor pos
	if(isComposing()) {
		setCompositionCursor(session, ::chewing_cursor_Current(chewingContext_));
	}

	// show aux info
	if(::chewing_aux_Check(chewingContext_)) {
		char* str = ::chewing_aux_String(chewingContext_);
		std::wstring wstr = utf8ToUtf16(str);
		::chewing_free(str);
		// show the message to the user
		// FIXME: sometimes libchewing shows the same aux info
		// for subsequent key events... I think this is a bug.
		showMessage(session, wstr, 2);
	}
	return true;
}

// virtual
bool TextService::filterKeyUp(Ime::KeyEvent& keyEvent) {
	if (config().switchLangWithShift) {
		if (lastKeyDownCode_ == VK_SHIFT && keyEvent.keyCode() == VK_SHIFT) {
			// last key down event is also shift key
			// a <Shift> key down + key up pair was detected
			// switch language
			return true;
		}
	}
	if (config().enableCapsLock) {
		if (lastKeyDownCode_ == VK_CAPITAL && keyEvent.keyCode() == VK_CAPITAL && langMode_ == CHINESE_MODE) {
			return true;
		}
	}
	lastKeyDownCode_ = 0;
	return false;
}

// virtual
bool TextService::onKeyUp(Ime::KeyEvent& keyEvent, Ime::EditSession* session) {
	if(config().switchLangWithShift) {
		if (lastKeyDownCode_ == VK_SHIFT && keyEvent.keyCode() == VK_SHIFT) {
			toggleLanguageMode(session);
			std::wstring msg;
			if (chewing_get_ChiEngMode(chewingContext_) == SYMBOL_MODE) {
				msg += L"英數模式";
			} else {
				msg += L"中文模式";
				if (config().enableCapsLock && keyEvent.isKeyToggled(VK_CAPITAL)) {
					msg = L"英數模式 (CapsLock)";
				}
			}
			showMessage(session, msg, 2);
		}
	}
	if (config().enableCapsLock) {
		if (lastKeyDownCode_ == VK_CAPITAL && keyEvent.keyCode() == VK_CAPITAL && langMode_ == CHINESE_MODE) {
			std::wstring msg;
			if (keyEvent.isKeyToggled(VK_CAPITAL)) {
				msg += L"英數模式 (CapsLock)";
			} else {
				msg += L"中文模式";
			}
			showMessage(session, msg, 2);
		}
	}
	lastKeyDownCode_ = 0;
	return true;
}

// virtual
bool TextService::onPreservedKey(const GUID& guid) {
	lastKeyDownCode_ = 0;
	// some preserved keys registered in ctor are pressed
	if(::IsEqualIID(guid, g_shiftSpaceGuid)) { // shift + space is pressed
		toggleShapeMode();
		// std::wstring msg;
		// if (chewing_get_ShapeMode(chewingContext_) == FULLSHAPE_MODE) {
		// 	msg += L"全形模式";
		// } else {
		// 	msg += L"半形模式";
		// }
		// showMessage(session, msg, 2);
		return true;
	}
	return false;
}


// virtual
STDMETHODIMP TextService::onCommand(UINT id, CommandType type) {
	assert(chewingContext_);
	if(type == COMMAND_RIGHT_CLICK) {
		if(id == ID_MODE_ICON) { // Windows 8 IME mode icon
			// TrackPopupMenu requires a window to work, so let's build a transient one.
			winrt::com_ptr<IWindow> window;
			CreateImeWindow(window.put_void());
			window->create(HWND_DESKTOP, 0);
			POINT pos = {0};
			::GetCursorPos(&pos);
			UINT ret = ::TrackPopupMenu(
				popupMenu_,
				TPM_NONOTIFY|TPM_RETURNCMD|TPM_LEFTALIGN|TPM_BOTTOMALIGN,
				pos.x,
				pos.y,
				0,
				window->hwnd(),
				NULL
			);
			if(ret > 0)
				onCommand(ret, COMMAND_MENU);
		}
		else {
			// we only handle right click in Windows 8 for the IME mode icon
			return S_FALSE;
		}
	}
	else {
		switch(id) {
		case ID_SWITCH_LANG:
			toggleLanguageMode(nullptr);
			break;
		case ID_SWITCH_SHAPE:
			toggleShapeMode();
			break;
		case ID_MODE_ICON: // Windows 8 IME mode icon
			toggleLanguageMode(nullptr);
			break;
		case ID_HASHED: // show config dialog
			if(!isImmersive()) { // only do this in desktop app mode
				std::wstring path = programDir();
    			path += L"\\ChewingPreferences.exe";
    			::ShellExecuteW(HWND_DESKTOP, L"open", path.c_str(), L"--edit", NULL, SW_SHOWNORMAL);
			}
			break;
		case ID_CONFIG: // show config dialog
			if(!isImmersive()) { // only do this in desktop app mode
				std::wstring path = programDir();
    			path += L"\\ChewingPreferences.exe";
    			::ShellExecuteW(HWND_DESKTOP, L"open", path.c_str(), NULL, NULL, SW_SHOWNORMAL);
			}
			break;
		case ID_OUTPUT_SIMP_CHINESE: // toggle output traditional or simplified Chinese
			toggleSimplifiedChinese();
			break;
		case ID_ABOUT: // show about dialog
			if(!isImmersive()) { // only do this in desktop app mode
				// show about dialog
				std::wstring path = programDir();
				path += L"\\ChewingPreferences.exe";
				::ShellExecuteW(NULL, L"open", path.c_str(), L"--about", NULL, SW_SHOWNORMAL);
			}
			break;
		case ID_WEBSITE: // visit chewing website
			::ShellExecuteW(NULL, NULL, L"https://chewing.im/", NULL, NULL, SW_SHOWNORMAL);
			break;
		case ID_GROUP: // visit chewing google groups website
			::ShellExecuteW(NULL, NULL, L"https://groups.google.com/group/chewing-devel", NULL, NULL, SW_SHOWNORMAL);
			break;
		case ID_BUGREPORT: // visit bug tracker page
			::ShellExecuteW(NULL, NULL, L"https://github.com/chewing/windows-chewing-tsf/issues?state=open", NULL, NULL, SW_SHOWNORMAL);
			break;
		case ID_DICT_BUGREPORT:
			::ShellExecuteW(NULL, NULL, L"https://github.com/chewing/libchewing-data/issues", NULL, NULL, SW_SHOWNORMAL);
			break;
		case ID_MOEDICT: // a very awesome online Chinese dictionary
			::ShellExecuteW(NULL, NULL, L"https://www.moedict.tw/", NULL, NULL, SW_SHOWNORMAL);
			break;
		case ID_DICT: // online Chinese dictonary
			::ShellExecuteW(NULL, NULL, L"https://dict.revised.moe.edu.tw/", NULL, NULL, SW_SHOWNORMAL);
			break;
		case ID_SIMPDICT: // a simplified version of the online dictonary
			::ShellExecuteW(NULL, NULL, L"https://dict.concised.moe.edu.tw/", NULL, NULL, SW_SHOWNORMAL);
			break;
		case ID_LITTLEDICT: // a simplified dictionary for little children
			::ShellExecuteW(NULL, NULL, L"https://dict.mini.moe.edu.tw/", NULL, NULL, SW_SHOWNORMAL);
			break;
		case ID_PROVERBDICT: // a dictionary for proverbs
			::ShellExecuteW(NULL, NULL, L"https://dict.idioms.moe.edu.tw/", NULL, NULL, SW_SHOWNORMAL);
			break;
		case ID_CHEWING_HELP:
			// TODO: open help file here
			// Need to update the old ChewingIME docs
			break;
		default:
			return S_FALSE;
		}
	}
	return S_OK;
}

STDMETHODIMP TextService::QueryInterface(REFIID riid, void **ppvObj) {
	if (IsEqualIID(riid, IID_IRunCommand)) {
		*ppvObj = (IRunCommand*)this;
		AddRef();
		return S_OK;
	}
	return	Ime::TextService::QueryInterface(riid, ppvObj);
}

STDMETHODIMP_(ULONG) TextService::AddRef() {
	return Ime::TextService::AddRef();
}

STDMETHODIMP_(ULONG) TextService::Release() {
	return Ime::TextService::Release();
}

// called when the keyboard is opened or closed
// virtual
void TextService::onKeyboardStatusChanged(bool opened) {
	Ime::TextService::onKeyboardStatusChanged(opened);
	if(opened) { // keyboard is opened
		initChewingContext();
	}
	else { // keyboard is closed
		if(isComposing()) {
			// end current composition if needed
			ITfContext* context = currentContext();
			if(context) {
				endComposition(context);
				context->Release();
			}
		}
		hideCandidates();
		hideMessage(); // hide message window, if there's any
		freeChewingContext(); // IME is closed, chewingContext is not needed
	}

	if(imeModeIcon_)
		imeModeIcon_->setEnabled(opened);
	// FIXME: should we also disable other language bar buttons as well?
}

// called just before current composition is terminated for doing cleanup.
// if forced is true, the composition is terminated by others, such as
// the input focus is grabbed by another application.
// if forced is false, the composition is terminated gracefully by endComposition().
// virtual
void TextService::onCompositionTerminated(bool forced) {
	// we do special handling here for forced composition termination.
	if(forced) {
		// we're still editing our composition and have something in the preedit buffer.
		// however, some other applications grabs the focus and force us to terminate
		// our composition.
		if(chewingContext_) {
			if(showingCandidates()) {
				chewing_cand_close(chewingContext_);
			}
			if(chewing_bopomofo_Check(chewingContext_)) {
				chewing_clean_bopomofo_buf(chewingContext_);
			}
			if(chewing_buffer_Check(chewingContext_)) {
				chewing_commit_preedit_buf(chewingContext_);
			}
		}
	}
}

void TextService::initChewingContext() {
	if (!chewingContext_) {
		initChewingEnv();
		chewingContext_ = ::chewing_new();
		::chewing_set_maxChiSymbolLen(chewingContext_, 50);
		Config& cfg = config();
		if(cfg.defaultEnglish)
			::chewing_set_ChiEngMode(chewingContext_, SYMBOL_MODE);
		if(cfg.defaultFullSpace)
			::chewing_set_ShapeMode(chewingContext_, FULLSHAPE_MODE);

		// get last mtime of symbols.dat file
		std::wstring file = userDir() + L"\\symbols.dat";
		struct _stat64 stbuf;
		if(_wstat64(file.c_str(), &stbuf) == 0)
			symbolsFileTime_ = stbuf.st_mtime;
	}

	applyConfig();
}

void TextService::freeChewingContext() {
	if(chewingContext_) {
		::chewing_delete(chewingContext_);
		chewingContext_ = NULL;
	}
}

void TextService::applyConfig() {
	Config& cfg = config();

	// apply the new configurations
	if(chewingContext_) {
		// Configuration

		// add user phrase before or after the cursor
		::chewing_set_addPhraseDirection(chewingContext_, cfg.addPhraseForward);

		// automatically shift cursor to the next char after choosing a candidate
		::chewing_set_autoShiftCur(chewingContext_, cfg.advanceAfterSelection);

		// candiate strings per page
		::chewing_set_candPerPage(chewingContext_, cfg.candPerPage);

		// clean the composition buffer by Esc key
		::chewing_set_escCleanAllBuf(chewingContext_, cfg.escCleanAllBuf);

		// keyboard type
		::chewing_set_KBType(chewingContext_, cfg.keyboardLayout);

		// Use space key to open candidate window.
		::chewing_set_spaceAsSelection(chewingContext_, cfg.showCandWithSpaceKey);

		// FIXME: what's this?
		// ::chewing_set_phraseChoiceRearward(chewingContext_, true);

		// keys use to select candidate strings (default: 123456789)
		int selKeys[10];
		for(int i = 0; i < 10; ++i)
			selKeys[i] = (int)Config::selKeys[cfg.selKeyType][i];
		::chewing_set_selKey(chewingContext_, selKeys, 10);

		chewing_config_set_int(chewingContext_, "chewing.conversion_engine", cfg.convEngine);
	}

	outputSimpChinese_ = cfg.outputSimpChinese;
	// update popup menu to check/uncheck the simplified Chinese item
	DWORD checkFlags = outputSimpChinese_ ?  MF_CHECKED : MF_UNCHECKED;
	::CheckMenuItem(popupMenu_, ID_OUTPUT_SIMP_CHINESE, MF_BYCOMMAND|checkFlags);

	if(messageWindow_) {
		messageWindow_->setFontSize(cfg.fontSize);
	}
	if(candidateWindow_) {
		candidateWindow_->setFontSize(cfg.fontSize);
	}
}

// toggle between English and Chinese
void TextService::toggleLanguageMode(Ime::EditSession* session) {
	// switch between Chinses and English modes
	if(chewingContext_) {
		// NB: must clear the bopomofo buffer before we can switch mode
		if(::chewing_bopomofo_Check(chewingContext_)) {
			chewing_clean_bopomofo_buf(chewingContext_);
			if (session) {
				// HACK reset the composition and remove bopomofo
				wstring compositionBuf;
				if(::chewing_buffer_Check(chewingContext_)) {
					char* buf = ::chewing_buffer_String(chewingContext_);
					if(buf) {
						std::wstring wbuf = ::utf8ToUtf16(buf);
						::chewing_free(buf);
						compositionBuf += wbuf;
						setCompositionString(session, compositionBuf.c_str(), compositionBuf.length());
					}
				}
			}
		}
		// HACK: send capslock to switch mode
		chewing_handle_Capslock(chewingContext_);
		updateLangButtons();
	}
}

// toggle between full shape and half shape
void TextService::toggleShapeMode() {
	// switch between half shape and full shape modes
	if(chewingContext_) {
		::chewing_set_ShapeMode(chewingContext_, !::chewing_get_ShapeMode(chewingContext_));
		updateLangButtons();
	}
}

// toggle output traditional or simplified Chinese
void TextService::toggleSimplifiedChinese() {
	outputSimpChinese_ = !outputSimpChinese_;
	// update popup menu to check/uncheck the simplified Chinese item
	DWORD checkFlags = outputSimpChinese_ ?  MF_CHECKED : MF_UNCHECKED;
	::CheckMenuItem(popupMenu_, ID_OUTPUT_SIMP_CHINESE, MF_BYCOMMAND|checkFlags);
}

void TextService::updateCandidates(Ime::EditSession* session) {
	assert(candidateWindow_);
	candidateWindow_->clear();
	candidateWindow_->setUseCursor(config().cursorCandList);
	candidateWindow_->setCandPerRow(config().candPerRow);
	candidateWindow_->setFontSize(config().fontSize);

	::chewing_cand_Enumerate(chewingContext_);
	int* selKeys = ::chewing_get_selKey(chewingContext_); // keys used to select candidates
	int n = ::chewing_cand_ChoicePerPage(chewingContext_); // candidate string shown per page
	int i;
	for(i = 0; i < n && ::chewing_cand_hasNext(chewingContext_); ++i) {
		char* str = ::chewing_cand_String(chewingContext_);
		std::wstring wstr = utf8ToUtf16(str);
		::chewing_free(str);
		candidateWindow_->add(wstr.c_str(), (wchar_t)selKeys[i]);
	}
	::chewing_free(selKeys);
	candidateWindow_->recalculateSize();
	candidateWindow_->refresh();

	RECT textRect;
	// get the position of composition area from TSF
	if(selectionRect(session, &textRect)) {
		// FIXME: where should we put the candidate window?
		candidateWindow_->move(textRect.left, textRect.bottom);
	}
}

// show candidate list window
void TextService::showCandidates(Ime::EditSession* session) {
	// TODO: implement ITfCandidateListUIElement interface to support UI less mode
	// Great reference: http://msdn.microsoft.com/en-us/library/windows/desktop/aa966970(v=vs.85).aspx

	// NOTE: in Windows 8 store apps, candidate window should be owned by
	// composition window, which can be returned by TextService::compositionWindow().
	// Otherwise, the candidate window cannot be shown.
	// Ime::CandidateWindow handles this internally. If you create your own
	// candidate window, you need to call TextService::isImmersive() to check
	// if we're in a Windows store app. If isImmersive() returns true,
	// The candidate window created should be a child window of the composition window.
	// Please see Ime::CandidateWindow::CandidateWindow() for an example.
	if(!candidateWindow_) {
		std::wstring bitmap_path = programDir();
		bitmap_path += L"\\Assets\\bubble.9.png";
		HWND parent = this->compositionWindow(session);
		candidateWindow_ = nullptr;
		CreateCandidateWindow(parent, bitmap_path.c_str(), candidateWindow_.put_void());
		candidateWindow_->setFontSize(config().fontSize);
	}
	updateCandidates(session);
	candidateWindow_->show();
	showingCandidates_ = true;
}

// hide candidate list window
void TextService::hideCandidates() {
	if(candidateWindow_) {
		candidateWindow_->hide();
		candidateWindow_ = nullptr;
	}
	showingCandidates_ = false;
}

// message window
void TextService::showMessage(Ime::EditSession* session, std::wstring message, int duration) {
	// remove previous message if there's any
	hideMessage();
	// FIXME: reuse the window whenever possible
	HWND parent = this->compositionWindow(session);
	messageWindow_ = nullptr;
	std::wstring bitmap_path = programDir();
	bitmap_path += L"\\Assets\\msg.9.png";
	CreateMessageWindow(parent, bitmap_path.c_str(), messageWindow_.put_void());
	messageWindow_->setFontSize(config().fontSize);
	messageWindow_->setText(message.c_str());
	
	int x = 0, y = 0;
	RECT rc;
	if(selectionRect(session, &rc)) {
		x = rc.left;
		y = rc.bottom;
	}

	messageWindow_->move(x, y);
	messageWindow_->show();

	messageTimerId_ = ::SetTimer(messageWindow_->hwnd(), 1, duration * 1000, nullptr);
}

void TextService::hideMessage() {
	if(messageTimerId_) {
		::KillTimer(messageWindow_->hwnd(), messageTimerId_);
		messageTimerId_ = 0;
	}
	if(messageWindow_) {
		messageWindow_->destroy();
		messageWindow_ = nullptr;
	}
}

bool TextService::isLightTheme() {
    DWORD value = 1;
    DWORD dataSize = sizeof(value);

    LSTATUS result = RegGetValueW(
        HKEY_CURRENT_USER,
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
        L"AppsUseLightTheme",
        RRF_RT_DWORD,
        nullptr,
        &value,
        &dataSize
    );

    if (result != ERROR_SUCCESS) {
        OutputDebugStringW(L"Determine isLightTheme failed, fallback to light theme");
        return true;
    }

	// 0 = dark theme, 1 = light theme
    return value > 0;
}

void TextService::updateLangButtons() {
	if(!chewingContext_)
		return;

	int langMode = ::chewing_get_ChiEngMode(chewingContext_);
	if(langMode != langMode_) {
		langMode_ = langMode;
		UINT iconId = isLightTheme() ? langMode == CHINESE_MODE ? IDI_CHI : IDI_ENG
		                             : langMode == CHINESE_MODE ? IDI_CHI_DARK : IDI_ENG_DARK;
		switchLangButton_->setIcon(LoadIconW(g_hInstance, MAKEINTRESOURCEW(iconId)));
		if(imeModeIcon_) {
			imeModeIcon_->setIcon(LoadIconW(g_hInstance, MAKEINTRESOURCEW(iconId)));
		}
	}

	int shapeMode = ::chewing_get_ShapeMode(chewingContext_);
	if(shapeMode != shapeMode_) {
		shapeMode_ = shapeMode;
		UINT iconId = shapeMode == FULLSHAPE_MODE ? IDI_FULL_SHAPE : IDI_HALF_SHAPE;
		switchShapeButton_->setIcon(LoadIconW(g_hInstance, MAKEINTRESOURCEW(iconId)));
	}
}

std::wstring TextService::userDir() {
    wchar_t path[MAX_PATH];
    HRESULT result;
    std::wstring userDir;

	// SHGetFolderPathA might fail in impersonation security context.
	// Use %USERPROFILE% to retrieve the user home directory.
    if (::GetEnvironmentVariableW(L"USERPROFILE", path, MAX_PATH)) {
        userDir = path;
        userDir += L"\\ChewingTextService";

        // create the user directory if not exists
        // NOTE: this call will fail in Windows 8 store apps
        // We need a way to create the dir in desktop mode and
        // set proper ACL, so later we can access it inside apps.
        DWORD attributes = ::GetFileAttributesW(userDir.c_str());
        if (attributes == INVALID_FILE_ATTRIBUTES) {
            // create the directory if it does not exist
            if (::GetLastError() == ERROR_FILE_NOT_FOUND) {
                ::CreateDirectoryW(userDir.c_str(), NULL);
                attributes = ::GetFileAttributesW(userDir.c_str());
            }
            // make the directory hidden
            if (attributes != INVALID_FILE_ATTRIBUTES &&
                (attributes & FILE_ATTRIBUTE_HIDDEN) == 0)
                ::SetFileAttributesW(userDir.c_str(),
                                     attributes | FILE_ATTRIBUTE_HIDDEN);
        }
    }
    return userDir;
}

std::wstring TextService::programDir() {
    wchar_t path[MAX_PATH];
    HRESULT result;
    // get the program data directory
    // try C:\program files (x86) first
    result = ::SHGetFolderPathW(NULL, CSIDL_PROGRAM_FILESX86, NULL, 0, path);
    if (result != S_OK)  // failed, fall back to C:\program files
        result = ::SHGetFolderPathW(NULL, CSIDL_PROGRAM_FILES, NULL, 0, path);
    if (result == S_OK) {  // program files folder is found
        std::wstring programDir = path;
        programDir += L"\\ChewingTextService";
        return programDir;
    }
    return std::wstring();
}

void TextService::initChewingEnv() {
	std::wstring env;
	std::wstring userPath = userDir();
	std::wstring chewingPath = programDir();

	env = L"CHEWING_USER_PATH=";
	env += userPath;
	_wputenv(env.c_str());

	env = L"CHEWING_PATH=";
	// prepend user dir path to program path, so user-specific files, if they exist,
	// can take precedence over built-in ones. (for ex: symbols.dat)
	env += userPath;
	// add ; to separate two dir paths
	env += ';';
	// add program dir after user profile dir
	env += chewingPath;
	env += L"\\Dictionary";
	_wputenv(env.c_str());
}

} // namespace Chewing
