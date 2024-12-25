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

#ifndef CHEWING_SYMBOLS_PROPERTY_PAGE
#define CHEWING_SYMBOLS_PROPERTY_PAGE
#pragma once

#include "PropertyPage.h"
#include "ChewingConfig.h"
#include <string>

namespace Chewing {

class SymbolsPropertyPage : public Ime::PropertyPage {
public:
	SymbolsPropertyPage(Config* config);
	virtual ~SymbolsPropertyPage(void);

protected:
	virtual bool onInitDialog();
	virtual void onOK();

private:
	Config* config_;
	std::wstring userDir_;
};

} // namespace Chewing

#endif

