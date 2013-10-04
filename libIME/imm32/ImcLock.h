#pragma once

#define NOIME
#include <windows.h>
#include "imm.h"

namespace Imm {

class CompStr;
class CandList;

class ImcLock
{
public:
	ImcLock(HIMC hIMC);
	~ImcLock(void);
protected:
	HIMC himc;
	INPUTCONTEXT* ic;
	CompStr* compStr;
	CandList* candList;
public:
	CompStr* getCompStr(void);
	CandList* getCandList(void);
	INPUTCONTEXT* getIC(){	return ic;	}
	HIMC getHIMC(){	return himc;	}
	bool lock(void);
	void unlock(void);
	bool isChinese(void);
	bool isFullShape(void);
	bool isVerticalComp(void);
};

} // namespace Imm
