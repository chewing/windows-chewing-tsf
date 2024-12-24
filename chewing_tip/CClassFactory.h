#pragma once

#include <unknwnbase.h>
#include <winnt.h>

namespace Chewing {

class CClassFactory : public IClassFactory {
   public:
    CClassFactory() : refCount_(1) {}

    STDMETHODIMP QueryInterface(REFIID riid, void **ppvObj);
    STDMETHODIMP_(ULONG) AddRef(void);
    STDMETHODIMP_(ULONG) Release(void);

   protected:
    STDMETHODIMP CreateInstance(IUnknown *pUnkOuter, REFIID riid,
                                void **ppvObj);
    STDMETHODIMP LockServer(BOOL fLock);

   private:
    ~CClassFactory() {}

    unsigned long refCount_;
};

}  // namespace Chewing