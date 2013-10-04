#include "ImmSupport.h"
#include "ImcLock.h"
#include "CompStr.h"
#include <new>

// This should be provided by MS, but they did not do it.

typedef struct _tagTRANSMSG {
	UINT message;
	WPARAM wParam;
	LPARAM lParam;
} TRANSMSG, *LPTRANSMSG;

namespace Ime {

using namespace Imm;

ImmSupport::ImmSupport(HIMC hImc):
	isComposing_(false),
	compositionCursor_(0),
	hImc_(hImc) {
}

ImmSupport::~ImmSupport(void) {
}

void ImmSupport::activate() {
	ImcLock lock(hImc_);
	INPUTCONTEXT* inputContext = lock.getIC();
	if(inputContext) {
		inputContext->fOpen = TRUE;
		ImmReSizeIMCC( inputContext->hCompStr, sizeof(CompStr) );
		CompStr* cs = lock.getCompStr();
		if(!cs)
			return;
		cs = new (cs) CompStr;	// placement new
#if 0
		ImmReSizeIMCC( inputContext->hCandInfo, sizeof(CandList) );
		CandList* cl = lock.getCandList();
		if(!cl)
			return FALSE;
		cl = new (cl) CandList;	// placement new
#endif
		if( !(inputContext->fdwInit & INIT_CONVERSION) )		// Initialize
		{
			inputContext->fdwConversion = IME_CMODE_CHINESE;
			inputContext->fdwConversion &=  ~IME_CMODE_FULLSHAPE;
			inputContext->fdwInit |= INIT_CONVERSION;
		}
#if 0
		if( !(inputContext->fdwInit & INIT_STATUSWNDPOS) )
		{
			RECT rc;
			IMEUI::getWorkingArea( &rc, inputContext->hWnd );
			inputContext->ptStatusWndPos.x = rc.right - (9+20*3+4) - 150;
			inputContext->ptStatusWndPos.y = rc.bottom - 26;
			inputContext->fdwInit |= INIT_STATUSWNDPOS;
		}
		if( !(inputContext->fdwInit & INIT_LOGFONT) )
		{
			// TODO: initialize font here
			inputContext->lfFont;
		}
#endif
	}
}

void ImmSupport::deactivate() {
	ImcLock lock(hImc_);
	INPUTCONTEXT* inputContext = lock.getIC();
	if(inputContext) {
		CompStr* cs = lock.getCompStr();
		cs->~CompStr();	// delete cs;
#if 0
		CandList* cl = lock.getCandList();
		cl->~CandList();	// delete cl;
#endif
	}
}

void ImmSupport::startComposition() {
	isComposing_ = true;
	generateMessage(WM_IME_STARTCOMPOSITION);
}

void ImmSupport::endComposition() {
	isComposing_ = false;
	if(!compositionStr_.empty()) {
		ImcLock lock(hImc_);
		CompStr* cs = lock.getCompStr();
		cs->setCompStr(L"");
		cs->setResultStr(compositionStr_.c_str());
		// commit the result only
		generateMessage(WM_IME_COMPOSITION, 0,
				GCS_CURSORPOS|GCS_RESULTCLAUSE|GCS_RESULTSTR|GCS_RESULTREADSTR|GCS_RESULTREADCLAUSE);
	}
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
	ImcLock lock(hImc_);
	INPUTCONTEXT* inputContext = lock.getIC();
	if(!inputContext)
		return false;

	HIMCC hbuf = ImmReSizeIMCC( inputContext->hMsgBuf, sizeof(TRANSMSG) * (inputContext->dwNumMsgBuf + 1) );
	if(hbuf) {
		inputContext->hMsgBuf = hbuf;
		TRANSMSG* pbuf = (TRANSMSG*)ImmLockIMCC( hbuf );
		if(pbuf) {
			pbuf[inputContext->dwNumMsgBuf].message = msg;
			pbuf[inputContext->dwNumMsgBuf].wParam = wp;
			pbuf[inputContext->dwNumMsgBuf].lParam = lp;
			++inputContext->dwNumMsgBuf;
			success = true;
			ImmUnlockIMCC(hbuf);
		}
	}

	if( success )
		success = ImmGenerateMessage(hImc_);
	return success;
}

}
