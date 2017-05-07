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

#include "ImeModule.h"
#include <string>
#include <algorithm>
#include <memory>
#include <ObjBase.h>
#include <msctf.h>
#include <Shlwapi.h>
#include <ShlObj.h>
#include <assert.h>
#include "Window.h"
#include "TextService.h"
#include "DisplayAttributeProvider.h"

using namespace std;

namespace Ime {

// these values are not defined in older TSF SDK (windows xp)
#ifndef TF_IPP_CAPS_IMMERSIVESUPPORT
// for Windows 8
// GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT {13A016DF-560B-46CD-947A-4C3AF1E0E35D}
static const GUID GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT =
{ 0x13A016DF, 0x560B, 0x46CD, { 0x94, 0x7A, 0x4C, 0x3A, 0xF1, 0xE0, 0xE3, 0x5D } };
// GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT {25504FB4-7BAB-4BC1-9C69-CF81890F0EF5}
static const GUID GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT =
{ 0x25504FB4, 0x7BAB, 0x4BC1, { 0x9C, 0x69, 0xCF, 0x81, 0x89, 0x0F, 0x0E, 0xF5 } };
#endif

// display attribute GUIDs

// {05814A20-00B3-4B73-A3D0-2C521EFA8BE5}
static const GUID g_inputDisplayAttributeGuid = 
{ 0x5814a20, 0xb3, 0x4b73, { 0xa3, 0xd0, 0x2c, 0x52, 0x1e, 0xfa, 0x8b, 0xe5 } };

// {E1270AA5-A6B1-4112-9AC7-F5E476C3BD63}
// static const GUID g_convertedDisplayAttributeGuid = 
// { 0xe1270aa5, 0xa6b1, 0x4112, { 0x9a, 0xc7, 0xf5, 0xe4, 0x76, 0xc3, 0xbd, 0x63 } };


ImeModule::ImeModule(HMODULE module, const CLSID& textServiceClsid):
	hInstance_(HINSTANCE(module)),
	textServiceClsid_(textServiceClsid),
	refCount_(1) {

	Window::registerClass(hInstance_);

	// regiser default display attributes
	inputAttrib_ = new DisplayAttributeInfo(g_inputDisplayAttributeGuid);
	inputAttrib_->setTextColor(COLOR_WINDOWTEXT);
	inputAttrib_->setLineStyle(TF_LS_DOT);
	inputAttrib_->setLineColor(COLOR_WINDOWTEXT);
	inputAttrib_->setBackgroundColor(COLOR_WINDOW);
	displayAttrInfos_.push_back(inputAttrib_);
	// convertedAttrib_ = new DisplayAttributeInfo(g_convertedDisplayAttributeGuid);
	// displayAttrInfos_.push_back(convertedAttrib_);

	registerDisplayAttributeInfos();
}

ImeModule::~ImeModule(void) {

	// display attributes
	if(!displayAttrInfos_.empty()) {
		list<DisplayAttributeInfo*>::iterator it;
		for(it = displayAttrInfos_.begin(); it != displayAttrInfos_.end(); ++it) {
			DisplayAttributeInfo* info = *it;
			info->Release();
		}
	}
}

// Dll entry points implementations
HRESULT ImeModule::canUnloadNow() {
	// we own the last reference
	return refCount_ <= 1 ? S_OK : S_FALSE;
}

HRESULT ImeModule::getClassObject(REFCLSID rclsid, REFIID riid, void **ppvObj) {
    if (IsEqualIID(riid, IID_IClassFactory) || IsEqualIID(riid, IID_IUnknown)) {
		// increase reference count
		AddRef();
		*ppvObj = (IClassFactory*)this; // our own object implements IClassFactory
		return NOERROR;
    }
	else
	    *ppvObj = NULL;
    return CLASS_E_CLASSNOTAVAILABLE;
}

#ifndef _WIN64  // only do this for the 32-bit version dll
static void loadDefaultUserRegistry(const wchar_t* defaultUserRegKey) {
	// The registry settings of all newly created users are based on the content of 
	// "C:\Users\Default User\ntuser.dat", so we need to write our settings to this file so 
	// the HKEY_CURRENT_USER key of newly created users can also contain our settings.
	// In order to do this, we need to load the default "hive" to registry first.
	// Reference: https://msdn.microsoft.com/zh-tw/library/windows/desktop/ms724889(v=vs.85).aspx
	wchar_t *userProfilesDir = nullptr;
	if (SUCCEEDED(::SHGetKnownFolderPath(FOLDERID_UserProfiles, 0, NULL, &userProfilesDir))) {
		// get the path of the default ntuser.dat file
		std::wstring defaultRegFile = userProfilesDir;
		::CoTaskMemFree(userProfilesDir);
		defaultRegFile += L"\\Default User\\ntuser.dat";

		// loading registry file requires special privileges SE_RESTORE_NAME and SE_BACKUP_NAME.
		// So let's do privilege elevation for our process.
		HANDLE processToken = NULL;
		::OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES, &processToken);
		DWORD bufLen = sizeof(TOKEN_PRIVILEGES) + sizeof(LUID_AND_ATTRIBUTES);
		std::unique_ptr<char> buf(new char[bufLen]);
		TOKEN_PRIVILEGES* privileges = reinterpret_cast<TOKEN_PRIVILEGES*>(buf.get());
		privileges->PrivilegeCount = 2;
		::LookupPrivilegeValue(NULL, SE_RESTORE_NAME, &privileges->Privileges[0].Luid);
		privileges->Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;
		::LookupPrivilegeValue(NULL, SE_BACKUP_NAME, &privileges->Privileges[1].Luid);
		privileges->Privileges[1].Attributes = SE_PRIVILEGE_ENABLED;
		::AdjustTokenPrivileges(processToken, FALSE, privileges, bufLen, NULL, NULL);
		::CloseHandle(processToken);

		// load the default registry hive under the specified key name
		::RegLoadKeyW(HKEY_USERS, defaultUserRegKey, defaultRegFile.c_str());
	}
}
#endif  // #ifndef _WIN64

HRESULT ImeModule::registerLangProfiles(LangProfileInfo* langs, int langsCount) {
	// register the language profile
	ComPtr<ITfInputProcessorProfiles> inputProcessProfiles;
	if(CoCreateInstance(CLSID_TF_InputProcessorProfiles, NULL, CLSCTX_INPROC_SERVER, IID_ITfInputProcessorProfiles, (void**)&inputProcessProfiles) == S_OK) {
		for(int i = 0; i < langsCount; ++i) {
			LangProfileInfo& lang = langs[i];
			if(inputProcessProfiles->Register(textServiceClsid_) == S_OK) {
				LCID lcid = LocaleNameToLCID(lang.locale.c_str(), 0);
				if (lcid == 0 && !lang.fallbackLocale.empty()) { // the conversion fails
					// The new RFC4646 locale names are not well-supported in Windows 7/Vista, so
					// here we provide a fallback locale which uses the deprecated RFC 1766 format instead.
					lcid = LocaleNameToLCID(lang.fallbackLocale.c_str(), 0);
				}
				if (lcid != 0) {
					LANGID langId = LANGIDFROMLCID(lcid);
					if (inputProcessProfiles->AddLanguageProfile(textServiceClsid_, langId, lang.profileGuid,
						lang.name.c_str(), lang.name.length(), lang.iconFile.empty() ? NULL : lang.iconFile.c_str(),
						lang.iconFile.length(), lang.iconIndex) != S_OK) {
						return E_FAIL;
					}
				}
				else {
					return E_FAIL;
				}
			}
		}
	}

	// NOTE: For Windows newer than Windows 8, we have to manually write some settings
	//       to the registry so the input methods can appear in the Windows control panel.
	//
	//       Registry path: "HKEY_CURRENT_USER\Control Panel\International\User Profile\<locale_name>"
	//       Sub key: "<lang ID>:{text service GUID}{input module GUID}"
	//
	//       Unfortunately, this is not documented officially by Microsoft.
	//       We found the values with some registry monitor tools:
	//       These settings are user-specific so they should be written to HKEY_CURRENT_USER of all users.
	//       This might be achieved by Microsoft Acitve Setup, yet another undocumented feature.
	//       https://helgeklein.com/blog/2010/04/active-setup-explained/
	//
	//       However, there is no way to uninstall keys installed with Active Setup. So let's avoid it.
	//       References: https://support.microsoft.com/en-us/kb/284193
	//                   https://blogs.technet.microsoft.com/deploymentguys/2009/10/29/configuring-default-user-settings-full-update-for-windows-7-and-windows-server-2008-r2/
#ifndef _WIN64  // only do this for the 32-bit version dll
	// The keys under HKCU\Control Panel\ is shared between the x86 and x64 versions and 
	// are not affected by WOW64 redirection. So doing this inside the 32-bit version is enough.

	if (isWindows8Above()) {
		DWORD sidCount = 0;
		if (::RegQueryInfoKeyW(HKEY_USERS, NULL, NULL, NULL, &sidCount, NULL, NULL, NULL, NULL, NULL, NULL, NULL) != ERROR_SUCCESS)
			return E_FAIL;
		wchar_t* textServiceClsIdStr = nullptr;
		if (FAILED(::StringFromCLSID(textServiceClsid_, &textServiceClsIdStr)))
			return E_FAIL;

		const wchar_t* defaultUserRegKey = L"__PIME_Default_user__";
		loadDefaultUserRegistry(defaultUserRegKey);

		// write the language settings to user-specific registry.
		wchar_t sid[256];
		for (DWORD iSid = 0; iSid < sidCount; ++iSid) {
			DWORD sidLen = sizeof(sid) / sizeof(wchar_t);
			if (::RegEnumKeyExW(HKEY_USERS, iSid, sid, &sidLen, NULL, NULL, NULL, NULL) == ERROR_SUCCESS) {
				// write settings of each input module to the user's registry
				for (int i = 0; i < langsCount; ++i) {
					auto& lang = langs[i];
					std::wstring localeRegPath = sid;
					localeRegPath += L"\\Control Panel\\International\\User Profile\\";
					localeRegPath += lang.locale;
					HKEY localeRegKey = NULL;
					DWORD err = ::RegCreateKeyExW(HKEY_USERS, localeRegPath.c_str(), 0, NULL, REG_OPTION_NON_VOLATILE, KEY_ALL_ACCESS, NULL, &localeRegKey, NULL);
					if (err == ERROR_SUCCESS) {
						LCID lcid = LocaleNameToLCID(lang.locale.c_str(), 0);
						if (lcid == 0 && !lang.fallbackLocale.empty()) { // the conversion fails
							lcid = LocaleNameToLCID(lang.fallbackLocale.c_str(), 0);  // try the fallback locale name
						}
						wchar_t lcid_hex[16];
						wsprintf(lcid_hex, L"%04x", lcid);
						std::wstring valueName = lcid_hex;
						valueName += L":";
						valueName += textServiceClsIdStr;
						wchar_t* profileClsIdStr = nullptr;
						if (SUCCEEDED(::StringFromCLSID(lang.profileGuid, &profileClsIdStr))) {
							valueName += profileClsIdStr;
							::CoTaskMemFree(profileClsIdStr);
							DWORD profileCount = 1;
							if (::RegQueryInfoKeyW(localeRegKey, NULL, NULL, NULL, NULL, NULL, NULL, &profileCount, NULL, NULL, NULL, NULL) == ERROR_SUCCESS) {
								// ::MessageBoxW(0, std::to_wstring(profileCount).c_str(), 0, 0);
								++profileCount;
							}
							::RegSetKeyValueW(localeRegKey, NULL, valueName.c_str(), REG_DWORD, &profileCount, sizeof(DWORD));
						}
						::RegCloseKey(localeRegKey);
					}
				}
			}
		}
		::CoTaskMemFree(textServiceClsIdStr);

		// unload the default user registry hive
		::RegUnLoadKeyW(HKEY_USERS, defaultUserRegKey);
	}
#endif  // #ifndef _WIN64
	return S_OK;
}

HRESULT ImeModule::registerServer(wchar_t* imeName, LangProfileInfo* langs, int count) {
	// write info of our COM text service component to the registry
	// path: HKEY_CLASS_ROOT\\CLSID\\{xxxx-xxxx-xxxx-xx....}
	// This reguires Administrator permimssion to write to the registery
	// regsvr32 should be run with Administrator
	// For 64 bit dll, it seems that we need to write the key to
	// a different path to make it coexist with 32 bit version:
	// HKEY_LOCAL_MACHINE\SOFTWARE\Wow6432Node\Classes\CLSID\{xxx-xxx-...}
	// Reference: http://stackoverflow.com/questions/1105031/can-my-32-bit-and-64-bit-com-components-co-reside-on-the-same-machine

	HRESULT result = S_OK;

	// get path of our module
	wchar_t modulePath[MAX_PATH];
	DWORD modulePathLen = GetModuleFileNameW(hInstance_, modulePath, MAX_PATH);

	wstring regPath = L"CLSID\\";
	LPOLESTR clsidStr = NULL;
	if(StringFromCLSID(textServiceClsid_, &clsidStr) != ERROR_SUCCESS)
		return E_FAIL;
	regPath += clsidStr;
	CoTaskMemFree(clsidStr);

	HKEY hkey = NULL;
	if(::RegCreateKeyExW(HKEY_CLASSES_ROOT, regPath.c_str(), 0, NULL, REG_OPTION_NON_VOLATILE, KEY_WRITE, NULL, &hkey, NULL) == ERROR_SUCCESS) {
		// write name of our IME
		::RegSetValueExW(hkey, NULL, 0, REG_SZ, (BYTE*)imeName, sizeof(wchar_t) * (wcslen(imeName) + 1));

		HKEY inProcServer32Key;
		if(::RegCreateKeyExW(hkey, L"InprocServer32", 0, NULL, REG_OPTION_NON_VOLATILE, KEY_WRITE, NULL, &inProcServer32Key, NULL) == ERROR_SUCCESS) {
			// store the path of our dll module in the registry
			::RegSetValueExW(inProcServer32Key, NULL, 0, REG_SZ, (BYTE*)modulePath, (modulePathLen + 1) * sizeof(wchar_t));
			// write threading model
			wchar_t apartmentStr[] = L"Apartment";
            ::RegSetValueExW(inProcServer32Key, L"ThreadingModel", 0, REG_SZ, (BYTE*)apartmentStr, 10 * sizeof(wchar_t));
			::RegCloseKey(inProcServer32Key);
		}
		else
			result = E_FAIL;
		::RegCloseKey(hkey);
    }
	else
		result = E_FAIL;

	// register language profiles
	if(result == S_OK) {
		result = registerLangProfiles(langs, count);
	}

	// register category
	if(result == S_OK) {
		ITfCategoryMgr *categoryMgr = NULL;
		if(CoCreateInstance(CLSID_TF_CategoryMgr, NULL, CLSCTX_INPROC_SERVER, IID_ITfCategoryMgr, (void**)&categoryMgr) == S_OK) {
			if(categoryMgr->RegisterCategory(textServiceClsid_, GUID_TFCAT_TIP_KEYBOARD, textServiceClsid_) != S_OK) {
				result = E_FAIL;
			}

			// register ourself as a display attribute provider
			// so later we can set change the look and feels of composition string.
			if(categoryMgr->RegisterCategory(textServiceClsid_, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, textServiceClsid_) != S_OK) {
				result = E_FAIL;
			}

			// enable UI less mode
			if(categoryMgr->RegisterCategory(textServiceClsid_, GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT, textServiceClsid_) != S_OK ||
				categoryMgr->RegisterCategory(textServiceClsid_, GUID_TFCAT_TIPCAP_UIELEMENTENABLED, textServiceClsid_) != S_OK) {
				result  = E_FAIL;
			}

			if(isWindows8Above()) {
				// for Windows 8 store app support
				// TODO: according to a exhaustive Google search, I found that
				// TF_IPP_CAPS_IMMERSIVESUPPORT is required to make the IME work with Windows 8.
				// http://social.msdn.microsoft.com/Forums/windowsapps/en-US/4c422cf1-ceb4-413b-8a7c-6881946a4c63/how-to-set-a-flag-indicating-tsf-components-compatibility
				// Quote from the page: "To indicate that your IME is compatible with Windows Store apps, call RegisterCategory with GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT."

				// declare supporting immersive mode
				if(categoryMgr->RegisterCategory(textServiceClsid_, GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, textServiceClsid_) != S_OK) {
					result = E_FAIL;
				}

				// declare compatibility with Windows 8 system tray
				if(categoryMgr->RegisterCategory(textServiceClsid_, GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, textServiceClsid_) != S_OK) {
					result = E_FAIL;
				}
			}

			categoryMgr->Release();
		}
	}
	return result;
}

HRESULT ImeModule::unregisterServer() {
	// unregister the language profile
	ITfInputProcessorProfiles *inputProcessProfiles = NULL;
	if(CoCreateInstance(CLSID_TF_InputProcessorProfiles, NULL, CLSCTX_INPROC_SERVER, IID_ITfInputProcessorProfiles, (void**)&inputProcessProfiles) == S_OK) {
		inputProcessProfiles->Unregister(textServiceClsid_);
		inputProcessProfiles->Release();
	}

	// unregister categories
	ITfCategoryMgr *categoryMgr = NULL;
	if(CoCreateInstance(CLSID_TF_CategoryMgr, NULL, CLSCTX_INPROC_SERVER, IID_ITfCategoryMgr, (void**)&categoryMgr) == S_OK) {
		categoryMgr->UnregisterCategory(textServiceClsid_, GUID_TFCAT_TIP_KEYBOARD, textServiceClsid_);
		categoryMgr->UnregisterCategory(textServiceClsid_, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, textServiceClsid_);
		// UI less mode
		categoryMgr->UnregisterCategory(textServiceClsid_, GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT, textServiceClsid_);

		if(isWindows8Above()) {
			// Windows 8 support
			categoryMgr->UnregisterCategory(textServiceClsid_, GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, textServiceClsid_);
			categoryMgr->RegisterCategory(textServiceClsid_, GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, textServiceClsid_);
		}

		categoryMgr->Release();
	}

	// delete the registry key
	wstring regPath = L"CLSID\\";
	LPOLESTR clsidStr = NULL;
	if(StringFromCLSID(textServiceClsid_, &clsidStr) == ERROR_SUCCESS) {
		regPath += clsidStr;
		CoTaskMemFree(clsidStr);
		::SHDeleteKey(HKEY_CLASSES_ROOT, regPath.c_str());
	}

#ifndef _WIN64  // only do this for the 32-bit version dll
	// The keys under HKCU\Control Panel\ is shared between the x86 and x64 versions and 
	// are not affected by WOW64 redirection. So doing this inside the 32-bit version is enough.

	// delete settings under "HKEY_CURRENT_USER\Control Panel\International\User Profile\<locale_name>" for all users
	if (isWindows8Above()) {
		DWORD sidCount = 0;
		if (::RegQueryInfoKeyW(HKEY_USERS, NULL, NULL, NULL, &sidCount, NULL, NULL, NULL, NULL, NULL, NULL, NULL) != ERROR_SUCCESS)
			return E_FAIL;
		wchar_t* textServiceClsIdStr = nullptr;
		if (FAILED(::StringFromCLSID(textServiceClsid_, &textServiceClsIdStr)))
			return E_FAIL;

		const wchar_t* defaultUserRegKey = L"__PIME_Default_user__";
		loadDefaultUserRegistry(defaultUserRegKey);

		// delete the language settings from user-specific registry.
		wchar_t sid[256];
		for (DWORD iSid = 0; iSid < sidCount; ++iSid) {
			DWORD sidLen = sizeof(sid) / sizeof(wchar_t);
			if (::RegEnumKeyExW(HKEY_USERS, iSid, sid, &sidLen, NULL, NULL, NULL, NULL) == ERROR_SUCCESS) {
				// remove settings of each input module to the user's registry
				std::wstring userRegPath = sid;
				userRegPath += L"\\Control Panel\\International\\User Profile";
				HKEY userKey = NULL;
				if (::RegOpenKeyExW(HKEY_USERS, userRegPath.c_str(), 0, KEY_READ, &userKey) == ERROR_SUCCESS) {
					DWORD localeCount = 0;
					if (::RegQueryInfoKeyW(userKey, NULL, NULL, NULL, &localeCount, NULL, NULL, NULL, NULL, NULL, NULL, NULL) == ERROR_SUCCESS) {
						// list all locales under this user
						wchar_t locale[100];
						for (DWORD iLocale = 0; iLocale < localeCount; ++iLocale) {
							DWORD localeLen = sizeof(locale) / sizeof(wchar_t);
							if (::RegEnumKeyExW(userKey, iLocale, locale, &localeLen, NULL, NULL, NULL, NULL) == ERROR_SUCCESS) {
								HKEY localeKey = NULL;
								if (::RegOpenKeyExW(userKey, locale, 0, KEY_ALL_ACCESS | KEY_READ, &localeKey) == ERROR_SUCCESS) {
									DWORD profileCount = 0;
									::RegQueryInfoKeyW(localeKey, NULL, NULL, NULL, NULL, NULL, NULL, &profileCount, NULL, NULL, NULL, NULL);
									// list all language profiles under this locale
									std::vector<std::wstring> profiles;
									for (DWORD iProfile = 0; iProfile < profileCount; ++iProfile) {
										wchar_t profile[128];
										DWORD profileLen = sizeof(profile) / sizeof(wchar_t);
										if (::RegEnumValueW(localeKey, iProfile, profile, &profileLen, 0, NULL, NULL, NULL) == ERROR_SUCCESS) {
											if (wcsstr(profile, textServiceClsIdStr)) {  // this profile is registered by us
												profiles.push_back(profile);
											}
										}
									}
									// delete these language profiles beloning to us
									for (const auto& profile : profiles) {
										::RegDeleteValueW(localeKey, profile.c_str());
									}
									::RegCloseKey(localeKey);
								}
							}
						}
					}
					::RegCloseKey(userKey);
				}
			}
		}
		::CoTaskMemFree(textServiceClsIdStr);

		// unload the default user registry hive
		::RegUnLoadKeyW(HKEY_USERS, defaultUserRegKey);
	}
#endif // #ifndef _WIN64
	return S_OK;
}


// display attributes stuff
bool ImeModule::registerDisplayAttributeInfos() {

	// register display attributes
	ComPtr<ITfCategoryMgr> categoryMgr;
	if(::CoCreateInstance(CLSID_TF_CategoryMgr, NULL, CLSCTX_INPROC_SERVER, IID_ITfCategoryMgr, (void**)&categoryMgr) == S_OK) {
		TfGuidAtom atom;
		categoryMgr->RegisterGUID(g_inputDisplayAttributeGuid, &atom);
		inputAttrib_->setAtom(atom);
		// categoryMgr->RegisterGUID(g_convertedDisplayAttributeGuid, &atom);
		// convertedAttrib_->setAtom(atom);
		return true;
	}
	return false;
}

void ImeModule::removeTextService(TextService* service) {
	textServices_.remove(service);
}

// virtual
bool ImeModule::onConfigure(HWND hwndParent, LANGID langid, REFGUID rguidProfile) {
	return true;
}


// COM related stuff

// IUnknown
STDMETHODIMP ImeModule::QueryInterface(REFIID riid, void **ppvObj) {
    if (ppvObj == NULL)
        return E_INVALIDARG;

	if(IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_IClassFactory))
		*ppvObj = (IClassFactory*)this;
	else if(IsEqualIID(riid, IID_ITfFnConfigure))
	 	*ppvObj = (ITfFnConfigure*)this;
	else
		*ppvObj = NULL;

	if(*ppvObj) {
		AddRef();
		return S_OK;
	}
	return E_NOINTERFACE;
}

STDMETHODIMP_(ULONG) ImeModule::AddRef(void) {
	return ::InterlockedIncrement(&refCount_);
}

STDMETHODIMP_(ULONG) ImeModule::Release(void) {
	// NOTE: I think we do not need to use critical sections to
	// protect the operation as M$ did in their TSF samples.
	// Our ImeModule will be alive until dll unload so
	// it's not possible for an user application and TSF manager
	// to free our objecet since we always have refCount == 1.
	// The last reference is released in DllMain() when unloading.
	// Hence interlocked operations are enough here, I guess.
	assert(refCount_ > 0);
	if(::InterlockedExchangeSubtract(&refCount_, 1) == 1) {
		delete this;
		return 0;
	}
	return refCount_;
}

// IClassFactory
STDMETHODIMP ImeModule::CreateInstance(IUnknown *pUnkOuter, REFIID riid, void **ppvObj) {
	*ppvObj = NULL;
	if(::IsEqualIID(riid, IID_ITfDisplayAttributeProvider)) {
		DisplayAttributeProvider* provider = new DisplayAttributeProvider(this);
		if(provider) {
			provider->QueryInterface(riid, ppvObj);
			provider->Release();
		}
	}
	else if(::IsEqualIID(riid, IID_ITfFnConfigure)) {
		// ourselves implement this interface.
		this->QueryInterface(riid, ppvObj);
	}
	else {
		TextService* service = createTextService();
		if(service) {
			textServices_.push_back(service);
			service->QueryInterface(riid, ppvObj);
			service->Release();
		}
	}
	return *ppvObj ? S_OK : E_NOINTERFACE;
}

STDMETHODIMP ImeModule::LockServer(BOOL fLock) {
	if(fLock)
		AddRef();
	else
		Release();
	return S_OK;
}

// ITfFnConfigure
STDMETHODIMP ImeModule::Show(HWND hwndParent, LANGID langid, REFGUID rguidProfile) {
	return onConfigure(hwndParent, langid, rguidProfile) ? S_OK : E_FAIL;
}

// ITfFunction
STDMETHODIMP ImeModule::GetDisplayName(BSTR *pbstrName) {
	*pbstrName = ::SysAllocString(L"Configuration");
	return S_OK;
}

} // namespace Ime
