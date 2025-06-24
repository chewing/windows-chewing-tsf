// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    ffi::{c_int, c_void},
    sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use com::CClassFactory;
use flexi_logger::LoggerHandle;
use windows::Win32::{
    Foundation::TRUE,
    System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
};
use windows_core::{ComObjectInner, GUID, HRESULT, IUnknown, Interface};

mod com;
mod gfx;
mod logging;
mod ts;
mod window;

static G_HINSTANCE: AtomicUsize = AtomicUsize::new(0);
static LOGGER: LazyLock<Option<LoggerHandle>> = LazyLock::new(crate::logging::init_logger);

#[unsafe(no_mangle)]
extern "system" fn DllMain(
    hmodule: *mut c_void,
    ul_reason_for_call: u32,
    _reserved: *const c_void,
) -> c_int {
    if DLL_PROCESS_ATTACH == ul_reason_for_call {
        let g_hinstance = G_HINSTANCE.load(Ordering::Relaxed);
        if g_hinstance == 0 {
            G_HINSTANCE.store(hmodule as usize, Ordering::Relaxed);
        }
    }
    if DLL_PROCESS_DETACH == ul_reason_for_call {
        if let Some(handle) = &*LOGGER {
            handle.shutdown();
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
