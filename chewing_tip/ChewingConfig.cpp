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

#include <Aclapi.h>
#include <Windows.h>
#include <VersionHelpers.h>
#include <debugapi.h>
#include <handleapi.h>
#include <minwindef.h>
#include <synchapi.h>
#include <winbase.h>
#include <winnt.h>
#include <winreg.h>
#include <cassert>
#include <cstddef>

#include "ChewingConfig.h"

namespace Chewing {

#define DEF_FONT_SIZE           16

const wchar_t* Config::selKeys[]={
	L"1234567890",
	L"asdfghjkl;",
	L"asdfzxcv89",
	L"asdfjkl789",
	L"aoeuhtn789",
    L"1234qweras",
	NULL
};

const wchar_t* Config::convEngines[]={
    L"簡單注音",
    L"智慧選詞",
    L"模糊智慧選詞",
    NULL
};

// http://msdn.microsoft.com/en-us/library/windows/desktop/hh448449(v=vs.85).aspx
// define new Win 8 app related constants for older versions of SDK and VC++
#ifndef SECURITY_APP_PACKAGE_AUTHORITY
#define SECURITY_APP_PACKAGE_AUTHORITY {0,0,0,0,0,15}
#define SECURITY_APP_PACKAGE_BASE_RID 0x00000002L
#define SECURITY_BUILTIN_APP_PACKAGE_RID_COUNT 2L
#define SECURITY_APP_PACKAGE_RID_COUNT 8L
#define SECURITY_CAPABILITY_BASE_RID 0x00000003L
#define SECURITY_BUILTIN_CAPABILITY_RID_COUNT 2L
#define SECURITY_CAPABILITY_RID_COUNT 5L
#define SECURITY_BUILTIN_PACKAGE_ANY_PACKAGE 0x00000001L
#endif

Config::Config():
	hChangeEvent(NULL),
	monitorHkey(NULL) {
	// Configuration
	keyboardLayout = 0;
	candPerRow = 3;
	defaultEnglish = false;
	defaultFullSpace = false;
	showCandWithSpaceKey = false;
	switchLangWithShift = true;
	outputSimpChinese = false;
	addPhraseForward = true;
	colorCandWnd = true;
	advanceAfterSelection = true;
	fontSize = DEF_FONT_SIZE;
	selKeyType = 0;
	convEngine = 1;
	candPerPage = 9;
	cursorCandList = 1;
	enableCapsLock = 1;
	fullShapeSymbols = 1;
	phraseMark = 1;
	escCleanAllBuf = 0;
	easySymbolsWithShift = 1;
	easySymbolsWithCtrl = 0;
	upperCaseWithShift = 0;
}

Config::~Config(void) {
	if (hChangeEvent) {
		CloseHandle(hChangeEvent);
	}
	if (monitorHkey) {
		RegCloseKey(monitorHkey);
	}
}

void Config::load() {
	// ensure that we always access 64 bit registry if running under WOW64
	HANDLE process = ::GetCurrentProcess();
	BOOL isWow64 = FALSE;
	::IsWow64Process(process, &isWow64);
	DWORD regFlags = isWow64 ? KEY_WOW64_64KEY : 0;
/*
	#define KB_TYPE_NUM 9
	#define KB_DEFAULT 0
	#define KB_HSU 1
	#define KB_IBM 2
	#define KB_GIN_YIEH 3
	#define KB_ET 4
	#define KB_ET26 5
	#define KB_DVORAK 6
	#define KB_DVORAK_HSU 7
	#define KB_HANYU_PINYING 8
*/
	HKEY hk = NULL;
	if(ERROR_SUCCESS == ::RegOpenKeyExW(HKEY_CURRENT_USER, L"Software\\ChewingTextService", 0, regFlags|KEY_READ, &hk)) {
		DWORD size = sizeof(DWORD);
		DWORD type = REG_DWORD;
		::RegQueryValueEx(hk, L"KeyboardLayout", 0, &type, (LPBYTE)&keyboardLayout, &size);		
		::RegQueryValueEx(hk, L"CandPerRow", 0, &type, (LPBYTE)&candPerRow, &size);
		::RegQueryValueEx(hk, L"DefaultEnglish", 0, &type, (LPBYTE)&defaultEnglish, &size);
		::RegQueryValueEx(hk, L"DefaultFullSpace", 0, &type, (LPBYTE)&defaultFullSpace, &size);
		::RegQueryValueEx(hk, L"ShowCandWithSpaceKey", 0, &type, (LPBYTE)&showCandWithSpaceKey, &size);
		::RegQueryValueEx(hk, L"SwitchLangWithShift", 0, &type, (LPBYTE)&switchLangWithShift, &size);
		::RegQueryValueEx(hk, L"OutputSimpChinese", 0, &type, (LPBYTE)&outputSimpChinese, &size);
		::RegQueryValueEx(hk, L"AddPhraseForward", 0, &type, (LPBYTE)&addPhraseForward, &size);
		::RegQueryValueEx(hk, L"ColorCandWnd", 0, &type, (LPBYTE)&colorCandWnd, &size);
		::RegQueryValueEx(hk, L"AdvanceAfterSelection", 0, &type, (LPBYTE)&advanceAfterSelection, &size);
        ::RegQueryValueEx(hk, L"DefFontSize", 0, &type, (LPBYTE)&fontSize, &size);
		::RegQueryValueEx(hk, L"SelKeyType", 0, &type, (LPBYTE)&selKeyType, &size);
		::RegQueryValueEx(hk, L"ConvEngine", 0, &type, (LPBYTE)&convEngine, &size);
		::RegQueryValueEx(hk, L"SelAreaLen", 0, &type, (LPBYTE)&candPerPage, &size);
		::RegQueryValueEx(hk, L"CursorCandList", 0, &type, (LPBYTE)&cursorCandList, &size);
		::RegQueryValueEx(hk, L"EnableCapsLock", 0, &type, (LPBYTE)&enableCapsLock, &size);
		::RegQueryValueEx(hk, L"FullShapeSymbols", 0, &type, (LPBYTE)&fullShapeSymbols, &size);
		::RegQueryValueEx(hk, L"PhraseMark", 0, &type, (LPBYTE)&phraseMark, &size);
		::RegQueryValueEx(hk, L"EscCleanAllBuf", 0, &type, (LPBYTE)&escCleanAllBuf, &size);
		::RegQueryValueEx(hk, L"EasySymbolsWithShift", 0, &type, (LPBYTE)&easySymbolsWithShift, &size);
		::RegQueryValueEx(hk, L"EasySymbolsWithCtrl", 0, &type, (LPBYTE)&easySymbolsWithCtrl, &size);
		::RegQueryValueEx(hk, L"UpperCaseWithShift", 0, &type, (LPBYTE)&upperCaseWithShift, &size);

		::RegCloseKey(hk);
	}

	if(selKeyType > ((sizeof(selKeys)/sizeof(char*))-1))
		selKeyType = 0;
}

void Config::save() {
	// ensure that we always access 64 bit registry if running under WOW64
	HANDLE process = ::GetCurrentProcess();
	BOOL isWow64 = FALSE;
	::IsWow64Process(process, &isWow64);
	DWORD regFlags = isWow64 ? KEY_WOW64_64KEY : 0;
	DWORD timestamp = GetTickCount();

	HKEY hk = NULL;
	LSTATUS ret = ::RegCreateKeyExW(HKEY_CURRENT_USER, L"Software\\ChewingTextService", 0, 
						NULL, 0, regFlags|KEY_READ|KEY_WRITE , NULL, &hk, NULL);
	if(ERROR_SUCCESS == ret) {
		::RegSetValueExW(hk, L"KeyboardLayout", 0, REG_DWORD, (LPBYTE)&keyboardLayout, sizeof(DWORD));
		::RegSetValueExW(hk, L"CandPerRow", 0, REG_DWORD, (LPBYTE)&candPerRow, sizeof(DWORD));
		::RegSetValueExW(hk, L"DefaultEnglish", 0, REG_DWORD, (LPBYTE)&defaultEnglish, sizeof(DWORD));
		::RegSetValueExW(hk, L"DefaultFullSpace", 0, REG_DWORD, (LPBYTE)&defaultFullSpace, sizeof(DWORD));
		::RegSetValueExW(hk, L"ShowCandWithSpaceKey", 0, REG_DWORD, (LPBYTE)&showCandWithSpaceKey, sizeof(DWORD));
		::RegSetValueExW(hk, L"SwitchLangWithShift", 0, REG_DWORD, (LPBYTE)&switchLangWithShift, sizeof(DWORD));
		::RegSetValueExW(hk, L"OutputSimpChinese", 0, REG_DWORD, (LPBYTE)&outputSimpChinese, sizeof(DWORD));
		::RegSetValueExW(hk, L"AddPhraseForward", 0, REG_DWORD, (LPBYTE)&addPhraseForward, sizeof(DWORD));
		::RegSetValueExW(hk, L"ColorCandWnd", 0, REG_DWORD, (LPBYTE)&colorCandWnd, sizeof(DWORD));
		::RegSetValueExW(hk, L"AdvanceAfterSelection", 0, REG_DWORD, (LPBYTE)&advanceAfterSelection, sizeof(DWORD));
		::RegSetValueExW(hk, L"DefFontSize", 0, REG_DWORD, (LPBYTE)&fontSize, sizeof(DWORD));
		::RegSetValueExW(hk, L"SelKeyType", 0, REG_DWORD, (LPBYTE)&selKeyType, sizeof(DWORD));
		::RegSetValueExW(hk, L"ConvEngine", 0, REG_DWORD, (LPBYTE)&convEngine, sizeof(DWORD));
		::RegSetValueExW(hk, L"SelAreaLen", 0, REG_DWORD, (LPBYTE)&candPerPage, sizeof(DWORD));
		::RegSetValueExW(hk, L"CursorCandList", 0, REG_DWORD, (LPBYTE)&cursorCandList, sizeof(DWORD));
		::RegSetValueExW(hk, L"EnableCapsLock", 0, REG_DWORD, (LPBYTE)&enableCapsLock, sizeof(DWORD));
		::RegSetValueExW(hk, L"FullShapeSymbols", 0, REG_DWORD, (LPBYTE)&fullShapeSymbols, sizeof(DWORD));
		::RegSetValueExW(hk, L"PhraseMark", 0, REG_DWORD, (LPBYTE)&phraseMark, sizeof(DWORD));
		::RegSetValueExW(hk, L"EscCleanAllBuf", 0, REG_DWORD, (LPBYTE)&escCleanAllBuf, sizeof(DWORD));
		::RegSetValueExW(hk, L"EasySymbolsWithShift", 0, REG_DWORD, (LPBYTE)&easySymbolsWithShift, sizeof(DWORD));
		::RegSetValueExW(hk, L"EasySymbolsWithCtrl", 0, REG_DWORD, (LPBYTE)&easySymbolsWithCtrl, sizeof(DWORD));
		::RegSetValueExW(hk, L"UpperCaseWithShift", 0, REG_DWORD, (LPBYTE)&upperCaseWithShift, sizeof(DWORD));
		::RegSetValueExW(hk, L"ModifiedTimestamp", 0, REG_DWORD, (LPBYTE)&timestamp, sizeof(DWORD));

		::RegCloseKey(hk);

		// grant access to app containers in Windows 8
		if(IsWindows8OrGreater())
			grantAppContainerAccess(L"CURRENT_USER\\Software\\ChewingTextService", SE_REGISTRY_KEY, KEY_READ);
	}
}

bool Config::reloadIfNeeded() {
	if (hChangeEvent) {
		DWORD result = WaitForSingleObject(hChangeEvent, 0);
		if (WAIT_OBJECT_0 == result) {
			OutputDebugStringW(L"[chewing] config change detected, reload.\n");
			load();
			watchChanges();
			return true;
		}
		if (WAIT_FAILED == result) {
			CloseHandle(hChangeEvent);
			hChangeEvent = NULL;
			watchChanges();
		}
	}
	return false;
}

void Config::watchChanges() {
	if (hChangeEvent == NULL) {
		hChangeEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
		if (hChangeEvent == NULL) {
			OutputDebugStringW(L"Unable to create change event handle\n");
			return;
		}
	} else {
		ResetEvent(hChangeEvent);
	}

	// ensure that we always access 64 bit registry if running under WOW64
	if (monitorHkey == NULL){
		HANDLE process = ::GetCurrentProcess();
		BOOL isWow64 = FALSE;
		::IsWow64Process(process, &isWow64);
		DWORD regFlags = isWow64 ? KEY_WOW64_64KEY : 0;
		if (ERROR_SUCCESS != ::RegOpenKeyExW(HKEY_CURRENT_USER, L"Software\\ChewingTextService", 0, regFlags|KEY_NOTIFY, &monitorHkey)) {
			OutputDebugStringW(L"Unable to open HKEY handle\n");
			return;
		}
	}
	DWORD  dwFilter = REG_NOTIFY_CHANGE_LAST_SET | REG_NOTIFY_THREAD_AGNOSTIC;
	if (ERROR_SUCCESS != RegNotifyChangeKeyValue(monitorHkey, TRUE, dwFilter, hChangeEvent, TRUE)) {
		OutputDebugStringW(L"Unable to register notify for registry change\n");
	}
}

// enable access to app containers
// Reference: http://msdn.microsoft.com/en-us/library/windows/desktop/aa379283(v=vs.85).aspx
//            http://msdn.microsoft.com/en-us/library/windows/desktop/hh448493(v=vs.85).aspx
//            http://www.codeproject.com/Articles/10042/The-Windows-Access-Control-Model-Part-1
//            http://www.codeproject.com/Articles/10200/The-Windows-Access-Control-Model-Part-2

// static
bool Config::grantAppContainerAccess(const wchar_t* object, SE_OBJECT_TYPE type, DWORD access) {
    bool ret = false;
    PACL oldAcl = NULL, newAcl = NULL;
    PSECURITY_DESCRIPTOR sd = NULL;
	// get old security descriptor
	if(::GetNamedSecurityInfo(object, type, DACL_SECURITY_INFORMATION,
			NULL, NULL, &oldAcl, NULL, &sd) == ERROR_SUCCESS) {
		// Create a well-known SID for the all appcontainers group.
		SID_IDENTIFIER_AUTHORITY ApplicationAuthority = SECURITY_APP_PACKAGE_AUTHORITY;
		PSID pAllAppsSID = NULL;
		if(::AllocateAndInitializeSid(&ApplicationAuthority, 
				SECURITY_BUILTIN_APP_PACKAGE_RID_COUNT,
				SECURITY_APP_PACKAGE_BASE_RID,
				SECURITY_BUILTIN_PACKAGE_ANY_PACKAGE,
				0, 0, 0, 0, 0, 0, &pAllAppsSID)) {
			EXPLICIT_ACCESS ea;
			memset(&ea, 0, sizeof(ea));
			ea.grfAccessPermissions = access;
			ea.grfAccessMode = SET_ACCESS;
			ea.grfInheritance= SUB_CONTAINERS_AND_OBJECTS_INHERIT;
			ea.Trustee.TrusteeForm = TRUSTEE_IS_SID;
			ea.Trustee.TrusteeType = TRUSTEE_IS_GROUP;
			ea.Trustee.ptstrName  = (LPTSTR) pAllAppsSID;
			// add the new entry to the existing DACL
			if(::SetEntriesInAcl(1, &ea, oldAcl, &newAcl) == ERROR_SUCCESS) {
				// set the new DACL back to the object
				if(::SetNamedSecurityInfo((LPWSTR)object, type,
					DACL_SECURITY_INFORMATION, NULL, NULL, newAcl, NULL) == ERROR_SUCCESS) {
						ret = true;
				}
			}
			::FreeSid(pAllAppsSID);
		}
	}
	if(sd != NULL) 
		::LocalFree((HLOCAL)sd); 
    if(newAcl != NULL) 
        ::LocalFree((HLOCAL)newAcl); 

	return ret;
}

} // namespace Chewing
