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

#include "KeyboardPropertyPage.h"
#include "resource.h"
#include <WindowsX.h>

namespace Chewing {

KeyboardPropertyPage::KeyboardPropertyPage(Config* config):
	Ime::PropertyPage((LPCTSTR)IDD_KBLAYOUT),
	config_(config) {
}

KeyboardPropertyPage::~KeyboardPropertyPage(void) {
}


// virtual
bool KeyboardPropertyPage::onInitDialog() {
	CheckRadioButton(hwnd_, IDC_KB1, IDC_KB15, IDC_KB1 + config_->keyboardLayout);
	return PropertyPage::onInitDialog();
}

// virtual
void KeyboardPropertyPage::onOK() {
	for(UINT id = IDC_KB1; id <= IDC_KB15; ++id) {
		if(IsDlgButtonChecked(hwnd_, id)) {
			config_->keyboardLayout = (id - IDC_KB1);
			break;
		}
	}
	PropertyPage::onOK();
}


} // namespace Chewing
