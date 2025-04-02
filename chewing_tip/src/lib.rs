use std::{ffi::{c_int, c_long, c_void}, sync::{atomic::{AtomicUsize, Ordering}, Mutex}};

mod gfx;
mod ts;
mod window;
mod lang_bar;

static G_HINSTANCE: AtomicUsize = AtomicUsize::new(0);

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
    G_HINSTANCE.store(hmodule as usize, Ordering::Relaxed);
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
