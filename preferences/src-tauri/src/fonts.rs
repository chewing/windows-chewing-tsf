use std::cmp::Ordering;

use anyhow::Result;
use log::error;
use serde::{Deserialize, Serialize};
use windows::{
    Win32::{
        Foundation::FALSE,
        Graphics::DirectWrite::{
            DWRITE_FACTORY_TYPE_SHARED, DWRITE_INFORMATIONAL_STRING_FULL_NAME,
            DWRITE_INFORMATIONAL_STRING_ID, DWRITE_INFORMATIONAL_STRING_PREFERRED_FAMILY_NAMES,
            DWRITE_INFORMATIONAL_STRING_TYPOGRAPHIC_FAMILY_NAMES,
            DWRITE_INFORMATIONAL_STRING_WIN32_FAMILY_NAMES, DWriteCreateFactory, IDWriteFactory,
            IDWriteFontFamily, IDWriteLocalizedStrings,
        },
    },
    core::{PCWSTR, w},
};

#[tauri::command]
pub fn get_system_fonts() -> Result<Vec<FontFamilyName>, String> {
    enum_font_families_dwrite().map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FontFamilyName {
    name: String,
    display_name: String,
}

const LOCALE_LIST: [PCWSTR; 4] = [w!("zh-tw"), w!("zh-hk"), w!("ja-jp"), w!("zh-cn")];
const INFO_ID_LIST: [DWRITE_INFORMATIONAL_STRING_ID; 4] = [
    DWRITE_INFORMATIONAL_STRING_PREFERRED_FAMILY_NAMES,
    DWRITE_INFORMATIONAL_STRING_TYPOGRAPHIC_FAMILY_NAMES,
    DWRITE_INFORMATIONAL_STRING_FULL_NAME,
    DWRITE_INFORMATIONAL_STRING_WIN32_FAMILY_NAMES,
];

fn enum_font_families_dwrite() -> Result<Vec<FontFamilyName>> {
    let mut result = vec![];
    let dwrite_factory: IDWriteFactory =
        unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
    let mut system_font_collection = None;
    unsafe { dwrite_factory.GetSystemFontCollection(&mut system_font_collection, false)? };
    if let Some(font_collection) = system_font_collection {
        let family_count = unsafe { font_collection.GetFontFamilyCount() };
        for i in 0..family_count {
            let font_family = unsafe { font_collection.GetFontFamily(i)? };
            let family_names = unsafe { font_family.GetFamilyNames()? };

            let mut en_name_idx = 0;
            let mut exists = FALSE;
            unsafe { family_names.FindLocaleName(w!("en-us"), &mut en_name_idx, &mut exists)? };
            if !exists.as_bool() {
                en_name_idx = 0;
            }
            let len = unsafe { family_names.GetStringLength(en_name_idx)? } as usize;
            let mut name = vec![0u16; len + 1];
            unsafe { family_names.GetString(en_name_idx, name.as_mut_slice())? };

            let name = String::from_utf16_lossy(&name[..len]);
            let display_name = get_localized_name(&family_names)
                .or_else(|| get_localized_info_string(&font_family))
                .unwrap_or(name.clone());

            result.push(FontFamilyName { name, display_name })
        }
    }
    result.sort_by(|a, b| {
        let a_localized = a.display_name.chars().any(|ch| !ch.is_ascii());
        let b_localized = b.display_name.chars().any(|ch| !ch.is_ascii());
        match (a_localized, b_localized) {
            (true, true) => a.display_name.cmp(&b.display_name),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => a.display_name.cmp(&b.display_name),
        }
    });
    Ok(result)
}

fn get_localized_name(names: &IDWriteLocalizedStrings) -> Option<String> {
    for loc in LOCALE_LIST {
        let mut exists = FALSE;
        let mut index = 0;
        unsafe {
            if let Err(error) = names.FindLocaleName(loc, &mut index, &mut exists) {
                error!("failed to find locale index for {loc:?}: {error:?}");
                continue;
            }
        }
        if exists.as_bool() {
            let Ok(len) = (unsafe { names.GetStringLength(index) }) else {
                continue;
            };
            let len = len as usize;
            let mut name = vec![0u16; len + 1];
            let Ok(_) = (unsafe { names.GetString(index, name.as_mut_slice()) }) else {
                continue;
            };
            if let Ok(result) = String::from_utf16(&name[..len]) {
                return Some(result);
            }
        }
    }
    None
}

fn get_localized_info_string(font_family: &IDWriteFontFamily) -> Option<String> {
    let font = unsafe {
        let Ok(font) = font_family.GetFont(0) else {
            return None;
        };
        font
    };
    for id in INFO_ID_LIST {
        let mut names: Option<IDWriteLocalizedStrings> = None;
        let mut exists = FALSE;
        unsafe {
            let Ok(_) = font.GetInformationalStrings(id, &mut names, &mut exists) else {
                continue;
            };
        }
        if exists.as_bool() && names.is_some() {
            if let Some(name) = get_localized_name(names.as_ref().unwrap()) {
                return Some(name);
            }
        }
    }
    None
}
