#include "CClassFactory.h"

#include <minwindef.h>
#include <msctf.h>
#include <unknwnbase.h>
#include <winbase.h>
#include <winerror.h>
#include <winnt.h>

#include "ChewingTextService.h"

namespace Chewing {

STDMETHODIMP CClassFactory::QueryInterface(REFIID riid, void **ppvObj) {
    if (ppvObj == nullptr) {
        return E_POINTER;
    }

    if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_IClassFactory)) {
        *ppvObj = this;
    } else {
        *ppvObj = nullptr;
    }

    if (*ppvObj) {
        AddRef();
        return S_OK;
    }
    return E_NOINTERFACE;
}

STDMETHODIMP_(ULONG) CClassFactory::AddRef(void) {
    return InterlockedIncrement(&refCount_);
}

STDMETHODIMP_(ULONG) CClassFactory::Release(void) {
    if (InterlockedExchangeSubtract(&refCount_, 1) == 1) {
        delete this;
    }
    return refCount_;
}

STDMETHODIMP CClassFactory::CreateInstance(IUnknown *pUnkOuter, REFIID riid,
                                           void **ppvObj) {
    *ppvObj = nullptr;

    OutputDebugStringW(L"CClassFactory::CreateInstance Called\n");

    LPOLESTR str;
    StringFromIID(riid, &str);
    OutputDebugStringW(str);
    OutputDebugStringW(L"\n");
    CoTaskMemFree(str);

    TextService *service = new TextService();
    service->QueryInterface(riid, ppvObj);
    service->Release();

    return *ppvObj == nullptr ? E_NOINTERFACE : S_OK;
}

STDMETHODIMP CClassFactory::LockServer(BOOL fLock) {
    if (fLock) {
        AddRef();
    } else {
        Release();
    }
    return S_OK;
}

}  // namespace Chewing