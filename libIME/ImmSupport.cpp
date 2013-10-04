#include "ImmSupport.h"

// This should be provided by MS, but they did not do it.

typedef struct _tagTRANSMSG {
	UINT message;
	WPARAM wParam;
	LPARAM lParam;
} TRANSMSG, *LPTRANSMSG;


namespace Ime {

ImmSupport::ImmSupport(HIMC hImc):
	isComposing_(false),
	compositionCursor_(0),
	hImc_(hImc) {
}

ImmSupport::~ImmSupport(void) {
}

void ImmSupport::startComposition() {
	generateMessage(WM_IME_STARTCOMPOSITION);
}

void ImmSupport::endComposition() {
	generateMessage(WM_IME_ENDCOMPOSITION);
}

bool ImmSupport::compositionRect(RECT* rect) {
	return false;
}

bool ImmSupport::selectionRect(RECT* rect) {
	return false;
}

HWND ImmSupport::compositionWindow() {
	return HWND_DESKTOP;
}

bool ImmSupport::isComposing() const {
	return isComposing_;
}

void ImmSupport::setCompositionString(const wchar_t* str, int len) {
	compositionStr_ = str;
}

void ImmSupport::setCompositionCursor(int pos) {
	compositionCursor_ = pos;
}

bool ImmSupport::generateMessage(UINT msg, WPARAM wp, LPARAM lp) {
	if(!hImc_)
		return false;

	bool success = false;
	INPUTCONTEXT* ic = ImmLockIMC(hImc_);
	if(!ic)
		return false;

	HIMCC hbuf = ImmReSizeIMCC( ic->hMsgBuf, sizeof(TRANSMSG) * (ic->dwNumMsgBuf + 1) );
	if(hbuf) {
		ic->hMsgBuf = hbuf;
		TRANSMSG* pbuf = (TRANSMSG*)ImmLockIMCC( hbuf );
		if(pbuf) {
			pbuf[ic->dwNumMsgBuf].message = msg;
			pbuf[ic->dwNumMsgBuf].wParam = wp;
			pbuf[ic->dwNumMsgBuf].lParam = lp;
			++ic->dwNumMsgBuf;
			success = true;
			ImmUnlockIMCC(hbuf);
		}
	}
	ImmUnlockIMC(hImc_);

	if( success )
		success = ImmGenerateMessage(hImc_);
	return success;
}


}
