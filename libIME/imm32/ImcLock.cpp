#include "ImcLock.h"

namespace Imm {

ImcLock::ImcLock(HIMC hIMC)
 : himc(hIMC), ic(NULL), compStr(NULL), candList(NULL)
{
	lock();
}

ImcLock::~ImcLock(void)
{
	unlock();
}

CompStr* ImcLock::getCompStr(void)
{
	if( compStr )
		return compStr;
	if( ic )
		return (CompStr*)ImmLockIMCC(ic->hCompStr);
	return NULL;
}

CandList* ImcLock::getCandList(void)
{
	if( candList )
		return candList;
	if( ic )
		return (CandList*)ImmLockIMCC(ic->hCandInfo);
	return NULL;
}


bool ImcLock::lock(void)
{
	if( !ic )
		ic = himc ? ImmLockIMC(himc) : NULL;
	return !!ic;
}

void ImcLock::unlock(void)
{
	if( ic )
	{
		if( compStr )
		{
			ImmUnlockIMCC(ic->hCompStr);
			compStr = NULL;
		}
		if( candList )
		{
			ImmUnlockIMCC(ic->hCandInfo);
			candList = NULL;
		}
		ImmUnlockIMC(himc);
		ic = NULL;
	}
}

bool ImcLock::isChinese(void)
{
	INPUTCONTEXT* ic = getIC();
	if(ic)
		return !!(ic->fdwConversion & IME_CMODE_CHINESE );
	return false;
}

bool ImcLock::isFullShape(void)
{
	INPUTCONTEXT* ic = getIC();
	if(ic)
		return !!(ic->fdwConversion & IME_CMODE_FULLSHAPE );

	return false;
}


bool ImcLock::isVerticalComp(void)
{
	return ( getIC() && ((LOGFONT&)ic->lfFont).lfEscapement == 2700 );
}

} // namespace Imm
