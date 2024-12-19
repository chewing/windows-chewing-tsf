#include <combaseapi.h>
#include <debugapi.h>
#include <minwindef.h>
#include <objbase.h>
#include <unknwn.h>
#include <windows.h>
#include <winerror.h>
#include <winnt.h>

#include "CClassFactory.h"

// DLL module handle
HINSTANCE g_hInstance = nullptr;

BOOL APIENTRY DllMain(HMODULE hModule, DWORD ul_reason_for_call,
                      LPVOID lpReserved) {
    OutputDebugStringW(L"DllMain called\n");
    switch (ul_reason_for_call) {
        case DLL_PROCESS_ATTACH:
            g_hInstance = HINSTANCE(hModule);
            // disable DllMain calls due to new thread creation
            DisableThreadLibraryCalls(g_hInstance);
            OutputDebugStringW(L"DllMain attached to process\n");
            break;
    }
    return TRUE;
}

STDAPI DllGetClassObject(REFCLSID rclsid, REFIID riid, void** ppvObj) {
    OutputDebugStringW(L"DllGetClassObject Called\n");

    Chewing::CClassFactory* pFactory = new Chewing::CClassFactory();

    HRESULT hr = pFactory->QueryInterface(riid, ppvObj);
    pFactory->Release();

    return hr;
}