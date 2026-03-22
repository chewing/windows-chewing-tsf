//! Safe wrapper for MSCTF.DLL

use windows::Win32::UI::TextServices::ITfCategoryMgr;
use windows_core::{HRESULT, Result};

// The TF_CreateCategoryMgr function creates a category manager object without
// having to initialize COM. Usage must be done carefully because the calling
// thread must maintain the reference count on an object that is owned by
// MSCTF.DLL.
windows_core::link!("msctf.dll" "system" fn TF_CreateCategoryMgr(ppcat: *mut Option<ITfCategoryMgr>) -> HRESULT);

pub(crate) fn tf_create_category_mgr() -> Result<ITfCategoryMgr> {
    let mut ret: Option<ITfCategoryMgr> = None;
    unsafe { TF_CreateCategoryMgr(&raw mut ret) }.ok()?;
    return Ok(ret.expect("unexpected TF_CreateCategoryMgr failure"));
}
