#pragma once
#define NOIME
#include <Windows.h>
#include "imm.h"
#include <string>

namespace Ime {

class ImmSupport {
public:
	ImmSupport(HIMC hImc);
	virtual ~ImmSupport(void);

	void activate();
	void deactivate();

	bool isComposing() const;
	void startComposition();
	void endComposition();
	bool compositionRect(RECT* rect);
	bool selectionRect(RECT* rect);
	HWND compositionWindow();
	void setCompositionString(const wchar_t* str, int len);
	void setCompositionCursor(int pos);

private:
	bool generateMessage(UINT msg, WPARAM wp = 0, LPARAM lp = 0);

private:
	HIMC hImc_;
	bool isComposing_;
	std::wstring compositionStr_;
	int compositionCursor_;
};

}
