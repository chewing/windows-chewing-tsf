use std::ffi::{c_int, c_long, c_void};

mod display_attribute;
mod gfx;
mod lang_bar;
mod window;

// Force linking chewing_capi
#[allow(unused_imports)]
use chewing_capi::setup::chewing_new;

#[unsafe(no_mangle)]
unsafe extern "C" fn LibIME2Init() {
    win_dbg_logger::rust_win_dbg_logger_init_info();
    log::debug!("libIME2 initialized");
}

unsafe extern "C" {
    unsafe fn DllMain_cpp(
        hmodule: *const c_void,
        ul_reason_for_call: u32,
        reserved: *const c_void,
    ) -> c_int;
    unsafe fn DllGetClassObject_cpp(
        rclsid: *const c_void,
        riid: *const c_void,
        ppv_obj: *mut *mut c_void,
    ) -> c_long;
}

#[unsafe(no_mangle)]
pub unsafe extern "stdcall" fn DllMain(
    hmodule: *const c_void,
    ul_reason_for_call: u32,
    reserved: *const c_void,
) -> c_int {
    unsafe { DllMain_cpp(hmodule, ul_reason_for_call, reserved) }
}

#[unsafe(no_mangle)]
pub unsafe extern "stdcall" fn DllGetClassObject(
    rclsid: *const c_void,
    riid: *const c_void,
    ppv_obj: *mut *mut c_void,
) -> c_long {
    unsafe { DllGetClassObject_cpp(rclsid, riid, ppv_obj) }
}
