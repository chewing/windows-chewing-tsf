use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::RwLock;

use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_NOTIMPL, S_FALSE};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl,
    ITfCategoryMgr, ITfDisplayAttributeInfo, ITfDisplayAttributeInfo_Impl,
    ITfDisplayAttributeProvider, ITfDisplayAttributeProvider_Impl, TF_DISPLAYATTRIBUTE,
};
use windows_core::{BSTR, GUID, Result, implement};

static ATTRS: RwLock<BTreeMap<u128, TF_DISPLAYATTRIBUTE>> = RwLock::new(BTreeMap::new());

#[derive(Debug)]
#[implement(ITfDisplayAttributeProvider)]
struct DisplayAttributeProvider;

impl ITfDisplayAttributeProvider_Impl for DisplayAttributeProvider_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(EnumTfDisplayAttributeInfo::default().into())
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        if guid.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let guid = unsafe { *guid };
        let guid_u128 = guid.to_u128();
        if let Ok(attrs) = ATTRS.read() {
            if let Some(da) = attrs.get(&guid_u128) {
                return Ok(DisplayAttributeInfo::new(guid, *da).into());
            }
        }
        Err(E_FAIL.into())
    }
}

pub(super) fn register_display_attribute(guid: &GUID, da: TF_DISPLAYATTRIBUTE) -> Result<VARIANT> {
    unsafe {
        let category_manager: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        // XXX Although RegisterGUID returns a DWORD (u32), all display
        // attributes are TfGuidAtoms and TfGuidAtoms are VT_I4.
        let atom = VARIANT::from(category_manager.RegisterGUID(guid)? as i32);
        if let Ok(mut attrs) = ATTRS.write() {
            attrs.insert((*guid).to_u128(), da);
        } else {
            return Err(E_FAIL.into());
        }
        Ok(atom)
    }
}

pub(super) fn get_display_attribute_info(guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
    if guid.is_null() {
        return Err(E_INVALIDARG.into());
    }
    let guid = unsafe { guid.as_ref().unwrap() };
    let guid_u128 = guid.to_u128();
    if let Ok(attrs) = ATTRS.read() {
        if let Some(da) = attrs.get(&guid_u128) {
            return Ok(DisplayAttributeInfo::new(*guid, *da).into());
        }
    }
    Err(E_FAIL.into())
}

#[derive(Debug, Default)]
#[implement(IEnumTfDisplayAttributeInfo)]
pub(super) struct EnumTfDisplayAttributeInfo {
    cursor: Cell<u128>,
}

impl IEnumTfDisplayAttributeInfo_Impl for EnumTfDisplayAttributeInfo_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(EnumTfDisplayAttributeInfo {
            cursor: self.cursor.clone(),
        }
        .into())
    }

    fn Next(
        &self,
        ulcount: u32,
        rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> Result<()> {
        let mut count = 0;
        let mut rginfo_ptr = rginfo;
        if let Ok(attrs) = ATTRS.read() {
            for (&guid, &da) in attrs.range(self.cursor.get()..) {
                if count > ulcount {
                    self.cursor.set(guid);
                    break;
                }
                let info = DisplayAttributeInfo::new(GUID::from_u128(guid), da);
                unsafe {
                    rginfo_ptr.write(Some(info.into()));
                    rginfo_ptr = rginfo_ptr.add(1);
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
            for (&guid, _) in attrs.range(self.cursor.get()..) {
                if count > ulcount {
                    self.cursor.set(guid);
                    break;
                }
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
