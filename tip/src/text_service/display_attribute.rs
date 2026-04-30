// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::cell::Cell;
use std::sync::RwLock;

use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_NOTIMPL, S_FALSE};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::TextServices::{
    IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl, ITfCategoryMgr,
    ITfDisplayAttributeInfo, ITfDisplayAttributeInfo_Impl, TF_DISPLAYATTRIBUTE,
};
use windows_core::{BSTR, ComObjectInner, GUID, Result, implement};

use crate::msctf::tf_create_category_mgr;

static ATTRS: RwLock<Vec<DisplayAttributeInfo>> = RwLock::new(Vec::new());

pub(super) fn register_display_attribute(guid: &GUID, da: TF_DISPLAYATTRIBUTE) -> Result<VARIANT> {
    unsafe {
        let category_manager: ITfCategoryMgr = tf_create_category_mgr()?;
        // XXX Although RegisterGUID returns a DWORD (u32), all display
        // attributes are TfGuidAtoms and TfGuidAtoms are VT_I4.
        let atom = VARIANT::from(category_manager.RegisterGUID(guid)? as i32);
        if let Ok(mut attrs) = ATTRS.write() {
            let attr_info = DisplayAttributeInfo::new(*guid, da);
            if let Some(pos) = attrs.iter().position(|attr| &attr.guid == guid) {
                attrs[pos] = attr_info;
            } else {
                attrs.push(attr_info);
            }
        } else {
            return Err(E_FAIL.into());
        }
        Ok(atom)
    }
}

pub(super) fn get_display_attribute_info(guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
    let Some(guid) = (unsafe { guid.as_ref() }) else {
        return Err(E_INVALIDARG.into());
    };
    if let Ok(attrs) = ATTRS.read()
        && let Some(pos) = attrs.iter().position(|attr| &attr.guid == guid)
    {
        return Ok(attrs[pos].clone().into_object().into_interface());
    }
    Err(E_FAIL.into())
}

#[derive(Debug, Default)]
#[implement(IEnumTfDisplayAttributeInfo)]
pub(super) struct EnumTfDisplayAttributeInfo {
    cursor: Cell<usize>,
}

impl IEnumTfDisplayAttributeInfo_Impl for EnumTfDisplayAttributeInfo_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(EnumTfDisplayAttributeInfo {
            cursor: self.cursor.clone(),
        }
        .into())
    }

    // Fun fact: Eclipse SWT always iterate through the list of
    // display attributes but stop immediately if the bAttr type
    // matches. So in practice it only shows one kind of display
    // attribute unless we use different bAttr for differnt segment.
    // We can perhaps have a quirk mode for eclipse.
    fn Next(
        &self,
        ulcount: u32,
        mut rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> Result<()> {
        let mut count = 0;
        if let Ok(attrs) = ATTRS.read() {
            for attr in attrs.iter().skip(self.cursor.get()).take(ulcount as usize) {
                self.cursor.update(|x| x + 1);
                let attr_info: ITfDisplayAttributeInfo =
                    attr.clone().into_object().into_interface();
                unsafe {
                    rginfo.write(Some(attr_info));
                    rginfo = rginfo.add(1);
                };
                count += 1;
            }
        }
        if !pcfetched.is_null() {
            unsafe { pcfetched.write(count) };
        }
        if count == ulcount {
            Ok(())
        } else {
            // XXX: S_FALSE is HRESULT(1), a value considered non-error
            // when it is converted to Result<()> the value is lost.
            // This is a windows-rs binding error.
            Err(S_FALSE.into())
        }
    }

    fn Reset(&self) -> Result<()> {
        self.cursor.set(0);
        Ok(())
    }

    fn Skip(&self, ulcount: u32) -> Result<()> {
        let mut count = 0;
        if let Ok(attrs) = ATTRS.read() {
            for _ in attrs.iter().skip(self.cursor.get()).take(ulcount as usize) {
                self.cursor.update(|x| x + 1);
                count += 1;
            }
        }
        if count == ulcount {
            Ok(())
        } else {
            Err(S_FALSE.into())
        }
    }
}

#[derive(Clone)]
#[implement(ITfDisplayAttributeInfo)]
struct DisplayAttributeInfo {
    guid: GUID,
    da: TF_DISPLAYATTRIBUTE,
}

impl DisplayAttributeInfo {
    fn new(guid: GUID, da: TF_DISPLAYATTRIBUTE) -> DisplayAttributeInfo {
        DisplayAttributeInfo { guid, da }
    }
}

impl ITfDisplayAttributeInfo_Impl for DisplayAttributeInfo_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(self.guid)
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok("Display Attribute".into())
    }

    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        if pda.is_null() {
            return Err(E_INVALIDARG.into());
        }
        unsafe { pda.write(self.da) }
        Ok(())
    }

    fn SetAttributeInfo(&self, pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        if pda.is_null() {
            return Err(E_INVALIDARG.into());
        }
        Err(E_NOTIMPL.into())
    }

    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod test {

    use windows::Win32::UI::TextServices::{IEnumTfDisplayAttributeInfo, TF_DISPLAYATTRIBUTE};
    use windows_core::{ComObjectInner, GUID};

    use super::{EnumTfDisplayAttributeInfo, register_display_attribute};

    #[test]
    fn enum_tf_display_attribute_info() {
        let guid1 = GUID::new().expect("new GUID 1");
        let guid2 = GUID::new().expect("new GUID 2");
        register_display_attribute(&guid1, TF_DISPLAYATTRIBUTE::default())
            .expect("register GUID 1");
        register_display_attribute(&guid2, TF_DISPLAYATTRIBUTE::default())
            .expect("register GUID 2");

        let enum_info: IEnumTfDisplayAttributeInfo = EnumTfDisplayAttributeInfo::default()
            .into_object()
            .into_interface();
        let mut rginfo = [None; 1];
        let mut pcfetched = 0;
        let mut count = 0;
        unsafe {
            loop {
                let hr = enum_info.Next(&mut rginfo[..], &mut pcfetched);
                // Checking hr is not reliable
                if hr.is_err() || pcfetched == 0 {
                    break;
                }
                count += 1;
            }
        }
        assert_eq!(2, count);
    }
}
