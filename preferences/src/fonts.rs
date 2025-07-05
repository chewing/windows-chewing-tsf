use anyhow::Result;
use slint::SharedString;
use windows::Win32::Graphics::DirectWrite::*;

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
                let len = names.GetStringLength(0)?;
                let mut name = vec![0u16; (len + 1) as usize];
                names.GetString(0, &mut name)?;
                res.push(String::from_utf16_lossy(&name).into());
            }
        }
    }
    Ok(res)
}
