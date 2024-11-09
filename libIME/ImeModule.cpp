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

#include <Windows.h>
#include <VersionHelpers.h>

#include "ImeModule.h"
#include <ObjBase.h>
#include <msctf.h>
#include <Shlwapi.h>
#include <ShlObj.h>
#include <assert.h>
#include "TextService.h"
#include "DisplayAttributeProvider.h"

#include "libime2.h"

using namespace std;

namespace Ime {

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

	LibIME2Init();
	ImeWindowRegisterClass(hInstance_);

	// regiser default display attributes
	inputAttrib_ = new DisplayAttributeInfo(g_inputDisplayAttributeGuid);
	inputAttrib_->setTextColor(COLOR_WINDOWTEXT);
	inputAttrib_->setBackgroundColor(COLOR_WINDOW);
	inputAttrib_->setLineStyle(TF_LS_DOT);
	inputAttrib_->setLineColor(COLOR_WINDOWTEXT);
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

// display attributes stuff
bool ImeModule::registerDisplayAttributeInfos() {

	// register display attributes
	winrt::com_ptr<ITfCategoryMgr> categoryMgr;
	if(::CoCreateInstance(CLSID_TF_CategoryMgr, NULL, CLSCTX_INPROC_SERVER, IID_ITfCategoryMgr, categoryMgr.put_void()) == S_OK) {
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
