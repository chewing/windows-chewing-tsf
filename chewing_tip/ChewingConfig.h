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

#ifndef CHEWING_CONFIG_H
#define CHEWING_CONFIG_H

#include <Windows.h>
#include <AccCtrl.h>
#include <ocidl.h>

namespace Chewing {

class Config {
public:
	Config();
	~Config(void);

	void load();
	void save();

	// Returns true if config was reloaded
	bool reloadIfNeeded();
	void watchChanges();

	static bool grantAppContainerAccess(const wchar_t* object, SE_OBJECT_TYPE type, DWORD access);

private:
	static bool createSecurityDesc(SECURITY_DESCRIPTOR& sd);

public:
	// Configuration
	DWORD keyboardLayout; // keyboard type
	DWORD candPerRow; // candidate string per row (not supported yet)
	DWORD defaultEnglish; // English mode by default
	DWORD defaultFullSpace; // full space mode by default
	DWORD showCandWithSpaceKey; // Use space key to open candidate window.
	DWORD switchLangWithShift; // switch language mode by Shift key
	DWORD outputSimpChinese; // output simplified Chinese (not supported yet)
	DWORD addPhraseForward; // add user phrase before or after the cursor
	DWORD colorCandWnd; // use colorful candidate windows (not supported yet)
	DWORD advanceAfterSelection; // automatically shift cursor to the next char after choosing a candidate
	DWORD fontSize; // font size of candidate window and tip window (not supported yet)
	DWORD selKeyType; // keys use to select candidate strings (default: 123456789)
	DWORD convEngine; // conversion engine (default: CHEWING_CONVERSION_ENGINE)
	DWORD candPerPage; // number of candiate strings per page
	DWORD cursorCandList; // use cursor to select items in the candidate window (not supported yet)
	DWORD enableCapsLock; // use capslock to Change language mode
	DWORD phraseMark; // not supported yet
	DWORD escCleanAllBuf; // clean the composition buffer by Esc key
	DWORD fullShapeSymbols; // output fullshape symbols when Shift key is down
	DWORD upperCaseWithShift; // output upper case English characters when shift is pressed
	DWORD easySymbolsWithShift; // output easy symbols when Shift is pressed
	DWORD easySymbolsWithCtrl; // output easy symbols when Ctrl is pressed

	static const wchar_t* selKeys[]; // keys used to select candidate strings.
	static const wchar_t* convEngines[];

private:
	HANDLE hChangeEvent;
	HKEY monitorHkey;
};

}

#endif
