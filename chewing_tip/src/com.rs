// SPDX-License-Identifier: GPL-3.0-or-later

use core::ffi::c_void;

use windows::Win32::System::Com::{CoLockObjectExternal, IClassFactory, IClassFactory_Impl};
use windows_core::{
    BOOL, ComObjectInner, ComObjectInterface, GUID, IUnknown, Interface, Ref, Result, implement,
};

use crate::ts::TextService;

#[implement(IClassFactory)]
pub(super) struct CClassFactory;

impl CClassFactory {
    pub(super) fn new() -> CClassFactory {
        CClassFactory
    }
}

impl IClassFactory_Impl for CClassFactory_Impl {
    fn CreateInstance(
        &self,
        _punkouter: Ref<'_, IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        let text_service: IUnknown = TextService::new().into_object().into_interface();
        unsafe {
            text_service.query(riid, ppvobject).ok()?;
        }
        Ok(())
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        unsafe { CoLockObjectExternal(self.as_interface_ref(), flock.as_bool(), true) }
    }
}
