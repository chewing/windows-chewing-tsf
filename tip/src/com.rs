// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    ffi::{c_int, c_void},
    sync::atomic::{AtomicUsize, Ordering},
};

use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use windows::Win32::System::Com::{CoLockObjectExternal, IClassFactory, IClassFactory_Impl};
use windows::Win32::{Foundation::TRUE, System::SystemServices::DLL_PROCESS_ATTACH};
use windows::core::{
    BOOL, ComObjectInner, ComObjectInterface, GUID, HRESULT, IUnknown, Interface, Ref, Result,
    implement,
};

use crate::{
    logging::{WinDbgWriter, output_debug_string},
    text_service::TextService,
};

pub(crate) static G_HINSTANCE: AtomicUsize = AtomicUsize::new(0);

#[unsafe(no_mangle)]
extern "system" fn DllMain(
    hmodule: *mut c_void,
    ul_reason_for_call: u32,
    _reserved: *const c_void,
) -> c_int {
    if let DLL_PROCESS_ATTACH = ul_reason_for_call {
        let g_hinstance = G_HINSTANCE.load(Ordering::Relaxed);
        if g_hinstance == 0 {
            G_HINSTANCE.store(hmodule as usize, Ordering::Relaxed);
            if let Err(error) = tracing_subscriber::fmt()
                .with_writer(WinDbgWriter::default)
                .with_span_events(FmtSpan::ENTER)
                .with_max_level(if cfg!(debug_assertions) {
                    Level::DEBUG
                } else {
                    Level::INFO
                })
                .try_init()
            {
                output_debug_string(&format!(
                    "chewing_tip: failed to init tracing_subscriber: {error:?}"
                ));
            }
            tracing::info!("chewing_tip.dll loaded");
        }
    }
    TRUE.0
}

#[unsafe(no_mangle)]
extern "system" fn DllGetClassObject(
    _rclsid: *const c_void,
    riid: *const GUID,
    ppv_obj: *mut *mut c_void,
) -> HRESULT {
    let factory: IUnknown = CClassFactory::new().into_object().into_interface();
    unsafe { factory.query(riid, ppv_obj) }
}

#[implement(IClassFactory)]
struct CClassFactory;

impl CClassFactory {
    fn new() -> CClassFactory {
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
