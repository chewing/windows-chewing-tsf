// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    ffi::{c_int, c_void},
    sync::atomic::{AtomicUsize, Ordering},
};

use com::CClassFactory;
use windows::Win32::{Foundation::TRUE, System::SystemServices::DLL_PROCESS_ATTACH};
use windows_core::{ComObjectInner, GUID, HRESULT, IUnknown, Interface};

mod com;
mod gfx;
mod ts;
mod window;

static G_HINSTANCE: AtomicUsize = AtomicUsize::new(0);

#[unsafe(no_mangle)]
extern "stdcall" fn DllMain(
    hmodule: *mut c_void,
    ul_reason_for_call: u32,
    _reserved: *const c_void,
) -> c_int {
    if let DLL_PROCESS_ATTACH = ul_reason_for_call {
        let g_hinstance = G_HINSTANCE.load(Ordering::Relaxed);
        if g_hinstance == 0 {
            G_HINSTANCE.store(hmodule as usize, Ordering::Relaxed);
            win_dbg_logger::rust_win_dbg_logger_init_info();
            log::info!("chewing_tip initialized");
        }
    }
    TRUE.0
}

#[unsafe(no_mangle)]
extern "stdcall" fn DllGetClassObject(
    _rclsid: *const c_void,
    riid: *const GUID,
    ppv_obj: *mut *mut c_void,
) -> HRESULT {
    let factory: IUnknown = CClassFactory::new().into_object().into_interface();
    unsafe { factory.query(riid, ppv_obj) }
}
