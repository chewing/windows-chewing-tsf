use std::cell::Cell;
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::RwLock;

use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_NOTIMPL, S_FALSE, S_OK};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl,
    ITfCategoryMgr, ITfDisplayAttributeInfo, ITfDisplayAttributeInfo_Impl,
    ITfDisplayAttributeProvider, ITfDisplayAttributeProvider_Impl, TF_DISPLAYATTRIBUTE,
};
use windows::core::{BSTR, GUID, HRESULT, Result, implement};
use windows_core::{ComObjectInner, Interface, OutRef};

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

fn register_display_attribute(
    guid: *const GUID,
    da: TF_DISPLAYATTRIBUTE,
    atom_out: *mut u32,
) -> Result<()> {
    if guid.is_null() || atom_out.is_null() {
        return Err(E_INVALIDARG.into());
    }
    unsafe {
        let category_manager: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        let atom = category_manager.RegisterGUID(guid)?;
        if let Ok(mut attrs) = ATTRS.write() {
            attrs.insert((*guid).to_u128(), da);
        } else {
            return Err(E_FAIL.into());
        }
        atom_out.write(atom);
    }
    Ok(())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn RegisterDisplayAttribute(
    guid: *const GUID,
    da: TF_DISPLAYATTRIBUTE,
    atom_out: *mut u32,
) -> HRESULT {
    match register_display_attribute(guid, da, atom_out) {
        Ok(_) => S_OK,
        Err(e) => e.into(),
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn CreateDisplayAttributeProvider(ret: *mut *mut c_void) {
    unsafe {
        ret.write(
            DisplayAttributeProvider
                .into_object()
                .into_interface::<ITfDisplayAttributeProvider>()
                .into_raw(),
        )
    }
}

#[derive(Debug, Default)]
#[implement(IEnumTfDisplayAttributeInfo)]
struct EnumTfDisplayAttributeInfo {
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
        rginfo: OutRef<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> Result<()> {
        let mut count = 0;
        // FIXME - a bug introduced in windows-rs 0.60.0 broke this interface signature
        // Should be fixed in windows-rs 0.63.0 or after.
        // https://github.com/microsoft/windows-rs/pull/3517
        let mut rginfo_ptr: *mut Option<ITfDisplayAttributeInfo> =
            unsafe { std::mem::transmute(rginfo) };
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
