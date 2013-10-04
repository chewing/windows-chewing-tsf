#include "compstr.h"

// Disable warning on converting pointer to DWORD in VC++
#pragma warning( disable: 4244 )

namespace Imm {

CompStr::CompStr(void) {
	cs.dwSize = sizeof(CompStr);
	cs.dwDeltaStart = 0;
	cs.dwCursorPos = 0;

	cs.dwCompReadStrOffset = (BYTE*)&readStr[0] - (BYTE*)this;
	cs.dwCompReadStrLen = 0;
	memset( readStr, 0, sizeof(readStr) );

	cs.dwCompReadAttrOffset = (BYTE*)&readAttr[0] - (BYTE*)this;
	cs.dwCompReadAttrLen = 0;
	memset( readAttr, 0, sizeof(readAttr) );

	cs.dwCompReadClauseOffset = DWORD( (BYTE*)&readClause[0] - (BYTE*)this);
	cs.dwCompReadClauseLen = 0;
	memset( readClause, 0, sizeof(readClause) );

	cs.dwCompStrOffset = (BYTE*)&compStr[0] - (BYTE*)this;
	cs.dwCompStrLen = 0;
	memset( compStr, 0, sizeof(compStr) );

	cs.dwCompAttrOffset = (BYTE*)&compAttr[0] - (BYTE*)(this);
	cs.dwCompAttrLen = 0;
	memset( compAttr, 0, sizeof(compAttr) );

	cs.dwCompClauseOffset = DWORD( (BYTE*) &compClause[0] - (BYTE*)this);
	cs.dwCompClauseLen = 0;
	memset( compClause, 0, sizeof(compClause) );

	cs.dwResultReadStrOffset = (BYTE*)&resultReadStr[0] - (BYTE*)this;
	cs.dwResultReadStrLen = 0;
	memset( resultReadStr, 0, sizeof(resultReadStr) );

	cs.dwResultReadClauseOffset = DWORD( (BYTE*)&resultReadClause[0] - (BYTE*)this);
	cs.dwResultReadClauseLen = 0;
	memset( resultReadClause, 0, sizeof(resultReadClause) );

	cs.dwResultStrOffset = (BYTE*)&resultStr[0] - (BYTE*)this;
	cs.dwResultStrLen = 0;
	memset( resultStr, 0, sizeof(resultStr) );

	cs.dwResultClauseOffset = DWORD( (BYTE*)&resultClause[0] - (BYTE*)this);
	cs.dwResultClauseLen = 0;
	memset( resultClause, 0, sizeof(resultClause) );

	cs.dwPrivateOffset = DWORD( (BYTE*)&showMsg[0] - (BYTE*)this);
	cs.dwPrivateSize = sizeof(showMsg);
	memset( showMsg, 0, sizeof(showMsg) );
}

CompStr::~CompStr(void)
{
}

void CompStr::setCompStr(LPCWSTR comp_str)
{
	wcscpy( compStr, comp_str );
	cs.dwCompStrLen = wcslen( compStr );
	cs.dwCompAttrLen = cs.dwCompStrLen;
	memset( (char*)compAttr, ATTR_CONVERTED, cs.dwCompAttrLen );
}

void CompStr::setShowMsg(LPCWSTR show_msg)
{
	wcscpy( showMsg, show_msg );
}

void CompStr::setResultStr(LPCWSTR result_str)
{
	wcscpy( resultStr, result_str );
	cs.dwResultStrLen = wcslen( resultStr );
	cs.dwResultClauseLen = sizeof(resultClause);
	resultClause[0] = 0;
	resultClause[1] = cs.dwResultStrLen;
	cs.dwResultReadStrLen = 0;
}

void CompStr::setCursorPos(int pos)
{
	// ATTR_INPUT 	Character currently being entered.
	// ATTR_TARGET_CONVERTED 	Character currently being converted (already converted).
	// ATTR_CONVERTED 	Character given from the conversion.
	// ATTR_TARGET_NOTCONVERTED 	Character currently being converted (yet to be converted).

	cs.dwCursorPos = pos;
}

void CompStr::setZuin(LPCWSTR zuin)
{
	wcscpy( readStr, zuin );
	cs.dwCompReadStrLen = wcslen(readStr);

	cs.dwCompReadAttrLen = cs.dwCompReadStrLen;
	if(cs.dwCompReadStrLen)
		memset( (char*)readAttr, ATTR_TARGET_NOTCONVERTED, cs.dwCompReadStrLen );
}

void CompStr::beforeGenerateMsg(void)
{
	wchar_t* sinsert = compStr + cs.dwCursorPos;

	memmove( sinsert + cs.dwCompReadStrLen, 
		sinsert, sizeof(wchar_t) * (cs.dwCompStrLen - cs.dwCursorPos) );
	wcsncpy( sinsert, readStr, cs.dwCompReadStrLen );
	cs.dwCompStrLen += cs.dwCompReadStrLen;
	compStr[cs.dwCompStrLen] = '\0';

	if (cs.dwCompReadAttrLen == 0 && cs.dwCompAttrLen != 0) {
		for (int i = 0; i+1 < cs.dwCompClauseLen; i++)
			if (compClause[i] <= cs.dwCursorPos && cs.dwCursorPos < compClause[i+1]) {
				for(int j=compClause[i]; j<compClause[i+1]; j++)
					compAttr[j]=ATTR_TARGET_CONVERTED;
			}

	} else {
		BYTE* ainsert = compAttr + cs.dwCursorPos;
		memmove( ainsert + cs.dwCompReadAttrLen, 
				ainsert, cs.dwCompAttrLen - cs.dwCursorPos);
		memcpy( ainsert, readAttr, cs.dwCompReadAttrLen );
		cs.dwCompAttrLen += cs.dwCompReadAttrLen;
	}

	bool g_useUnicode = true;
	if( g_useUnicode )	{
		if( compStr[0] == 0 ) {	// If quick commit
			cs.dwCompClauseLen = 0;	// No clause info
			compClause[0] = 0;
			compClause[1] = cs.dwCompStrLen;
		}
		else	{	// This composition string contains Chinese characters

			if( cs.dwCompReadStrLen ) {
				if( 0 == cs.dwCompClauseLen ) {
					for (int i = 0; i <= cs.dwCompStrLen; i++)
						compClause[cs.dwCompClauseLen++] = i;
				}
				int newCompClauseLen = 0;
				DWORD newCompClause[257];
				int i;
				for (i = 0; i < cs.dwCompClauseLen && compClause[i] < cs.dwCursorPos; i++)
					newCompClause[newCompClauseLen++] = compClause[i];
				if (compClause[i] == cs.dwCursorPos)
					i++;
				for (int j = 0; j <= cs.dwCompReadStrLen; j++)
					newCompClause[newCompClauseLen++] = cs.dwCursorPos + j;
				for (; i < cs.dwCompClauseLen; i++)
					newCompClause[newCompClauseLen++] = compClause[i] + cs.dwCompReadStrLen;

				memcpy(compClause, newCompClause, sizeof(compClause));
				cs.dwCompClauseLen = newCompClauseLen;
			}
			cs.dwCompClauseLen *= sizeof(DWORD);
		}

		if( resultStr[0] == 0 )	// If no result string
			cs.dwResultClauseLen = 0;	// No clause info
		else	{	// This result string contains Chinese characters
			for(int i = 0; i <= (int) cs.dwResultStrLen; ++i ) {
				resultClause[ i ] = i;
			}
			cs.dwResultClauseLen = (cs.dwResultStrLen+1) * sizeof(DWORD);
		}
	}

	cs.dwCompReadStrLen = cs.dwCompReadAttrLen = 0;

	resultClause[0] = 0;
	resultClause[1] = cs.dwResultStrLen;
	cs.dwResultClauseLen = 0;//sizeof(resultClause);

	readClause[0] = 0;
	readClause[1] = cs.dwCompReadStrLen;
	cs.dwCompReadClauseLen = 0;//sizeof(readClause);

	resultReadClause[0] = 0;
	resultReadClause[1] = cs.dwResultReadClauseLen;
	cs.dwResultReadClauseLen = 0;//sizeof(resultReadClause);


}

void CompStr::setIntervalArray( unsigned char* interval, int count )
{
	cs.dwCompClauseLen = 0;
	if ( count<=0 ) {
		return;
	}

	for( DWORD i = 0; i < cs.dwCompStrLen; ) {
		if( interval == NULL || i < interval[0] || count <= 0 ) {
			compClause[ cs.dwCompClauseLen++ ] = i++;
			continue;
		}
		compClause[ cs.dwCompClauseLen++ ] = interval[0];
		i = interval[ 1 ];
		interval += 2;
		count -= 2;
	}
	compClause[ cs.dwCompClauseLen++ ] = cs.dwCompStrLen;
}

// for IE workaround
void CompStr::backupCompLen(void)
{
	bakCompStrLen = cs.dwCompStrLen;
	bakCompClauseLen = cs.dwCompClauseLen;
	bakCompAttrLen = cs.dwCompAttrLen;
	bakCompReadStrLen = cs.dwCompReadStrLen;
	bakCompReadClauseLen = cs.dwCompReadClauseLen;
	bakCompReadAttrLen = cs.dwCompReadAttrLen;
	bakCursorPos = cs.dwCursorPos;
}

void CompStr::resetCompLen(void)
{
	cs.dwCompStrLen = 0;
	cs.dwCompClauseLen = 0;
	cs.dwCompAttrLen = 0;
	cs.dwCompReadStrLen = 0;
	cs.dwCompReadClauseLen = 0;
	cs.dwCompReadAttrLen = 0;
	cs.dwCursorPos = 0;
}

void CompStr::restoreCompLen(void)
{
	cs.dwCompStrLen = bakCompStrLen;
	cs.dwCompClauseLen = bakCompClauseLen;
	cs.dwCompAttrLen = bakCompAttrLen;
	cs.dwCompReadStrLen = bakCompReadStrLen;
	cs.dwCompReadClauseLen = bakCompReadClauseLen;
	cs.dwCompReadAttrLen = bakCompReadAttrLen;
	cs.dwCursorPos = bakCursorPos;
}

} // namespace Imm
