use std::cmp::Ordering;

use anyhow::Result;
use slint::SharedString;
use windows::{
    Win32::{Foundation::FALSE, Graphics::DirectWrite::*},
    core::w,
};

pub(super) fn enum_font_families() -> Result<Vec<SharedString>> {
    let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
    let mut collection = None;
    let mut res = vec![];
    unsafe {
        factory.GetSystemFontCollection(&mut collection, false)?;
        if let Some(collection) = collection {
            let family_count = collection.GetFontFamilyCount();
            for i in 0..family_count {
                let family = collection.GetFontFamily(i)?;
                let names = family.GetFamilyNames()?;
                let mut index = 0;
                let mut exists = FALSE;
                names.FindLocaleName(w!("zh-tw"), &mut index, &mut exists)?;
                if !exists.as_bool() {
                    index = 0;
                }
                let len = names.GetStringLength(index)?;
                let mut name = vec![0u16; (len + 1) as usize];
                names.GetString(index, &mut name)?;
                res.push(String::from_utf16_lossy(&name).into());
            }
        }
    }
    res.sort_by(|a: &SharedString, b| {
        let a_localized = a.chars().any(|ch| !ch.is_ascii());
        let b_localized = b.chars().any(|ch| !ch.is_ascii());
        match (a_localized, b_localized) {
            (true, true) => a.cmp(b),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => a.cmp(b),
        }
    });
    Ok(res)
}
