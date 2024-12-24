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

#include "TypingPropertyPage.h"
#include "UiPropertyPage.h"
#include "KeyboardPropertyPage.h"
#include "SymbolsPropertyPage.h"
#include "PropertyDialog.h"
#include "AboutDialog.h"
#include "resource.h"
#include <CommCtrl.h>
#include <Msctf.h>

#include <Unknwn.h>
#include <winrt/base.h>

namespace Chewing {

static void initControls() {
	INITCOMMONCONTROLSEX ic;
	ic.dwSize = sizeof(ic);
	ic.dwICC = ICC_UPDOWN_CLASS;
	::InitCommonControlsEx(&ic);

	// Init RichEdit 2.0
	// HMODULE riched20;
	// riched20 = LoadLibraryA("RICHED20.DLL");
}

static void configDialog(HINSTANCE hInstance) {
	initControls();

	Config config;
	config.load();

	Ime::PropertyDialog dlg;
	TypingPropertyPage* typingPage = new TypingPropertyPage(&config);
	UiPropertyPage* uiPage = new UiPropertyPage(&config);
	KeyboardPropertyPage* keyboardPage = new KeyboardPropertyPage(&config);
	SymbolsPropertyPage* symbolsPage = new SymbolsPropertyPage(&config);
	dlg.addPage(typingPage);
	dlg.addPage(uiPage);
	dlg.addPage(keyboardPage);
	dlg.addPage(symbolsPage);
	INT_PTR ret = dlg.showModal(hInstance, (LPCTSTR)IDS_CONFIG_TITLE, 0, HWND_DESKTOP);
	if(ret) { // the user clicks OK button
		config.save();
	}
}

static void aboutDialog(HINSTANCE hInstance) {
	AboutDialog dlg;
	dlg.showModal(hInstance, IDD_ABOUT);
}

}

int WINAPI wWinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, PWSTR cmdLine, int nShow) {
	if(cmdLine && wcscmp(cmdLine, L"/about") == 0) // show about
		Chewing::aboutDialog(hInstance);
	else // show configuration dialog
		Chewing::configDialog(hInstance);
	return 0;
}
