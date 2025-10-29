// SPDX-License-Identifier: GPL-3.0-or-later
#![windows_subsystem = "windows"]

use std::{env, process};

use windows::{
    Win32::{
        Globalization::*,
        System::Com::*,
        UI::{Input::KeyboardAndMouse::HKL, TextServices::*},
    },
    core::*,
};
#[cfg(feature = "nightly")]
use windows_registry::LOCAL_MACHINE;

// https://learn.microsoft.com/en-us/windows/win32/tsf/installlayoutortip
windows_link::link!("input.dll" "system" fn InstallLayoutOrTip(psz: *const u16, dwFlags: u32));
const ILOT_INSTALL: u32 = 0x00000000;
const ILOT_UNINSTALL: u32 = 0x00000001;

const CHEWING_TSF_CLSID: GUID = GUID::from_u128(0x13F2EF08_575C_4D8C_88E0_F67BB8052B84);
const CHEWING_PROFILE_GUID: GUID = GUID::from_u128(0xCE45F71D_CE79_41D1_967D_640B65A380E3);
const CHEWING_TIP_DESC: PCWSTR =
    w!("0x0404:{13F2EF08-575C-4D8C-88E0-F67BB8052B84}{CE45F71D-CE79-41D1-967D-640B65A380E3}");

const CATEGORIES: [GUID; 6] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
];

fn register(icon_path: String) -> Result<()> {
    unsafe {
        let input_processor_profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        let mut lcid = LocaleNameToLCID(w!("zh-Hant-TW"), 0);
        if lcid == 0 {
            lcid = LocaleNameToLCID(w!("zh-TW"), 0);
        }

        let pw_icon_path = icon_path.encode_utf16().collect::<Vec<_>>();

        input_processor_profile_mgr.RegisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_PROFILE_GUID,
            w!("新酷音輸入法").as_wide(),
            &pw_icon_path,
            0,
            HKL::default(),
            0,
            false,
            0,
        )?;

        let category_manager: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for tfcat in &CATEGORIES {
            category_manager.RegisterCategory(&CHEWING_TSF_CLSID, tfcat, &CHEWING_TSF_CLSID)?;
        }
    }

    #[cfg(feature = "nightly")]
    {
        // Enable user-mode minidump for debug build
        if let Err(error) = LOCAL_MACHINE
            .create("SOFTWARE\\Microsoft\\Windows\\Windows Error Reporting\\LocalDumps")
        {
            println!("Error: unable to enable user-mode minidump: {error}");
        }
    }
    Ok(())
}

fn unregister() -> Result<()> {
    unsafe {
        let input_processor_profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        let category_manager: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for tfcat in &CATEGORIES {
            if let Err(error) =
                category_manager.UnregisterCategory(&CHEWING_TSF_CLSID, tfcat, &CHEWING_TSF_CLSID)
            {
                println!("Failed to unregister category {tfcat:?}: {error}");
            }
        }

        let mut lcid = LocaleNameToLCID(w!("zh-Hant-TW"), 0);
        if lcid == 0 {
            lcid = LocaleNameToLCID(w!("zh-TW"), 0);
        }

        input_processor_profile_mgr.UnregisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_PROFILE_GUID,
            0,
        )?;
    }

    Ok(())
}

fn enable() {
    unsafe {
        InstallLayoutOrTip(CHEWING_TIP_DESC.as_ptr(), ILOT_INSTALL);
    }
}

fn disable() {
    unsafe {
        InstallLayoutOrTip(CHEWING_TIP_DESC.as_ptr(), ILOT_UNINSTALL);
    }
}

fn main() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

        if env::args().len() == 1 {
            println!("Usage:");
            println!("  tsfreg -r <IconPath>    註冊輸入法");
            println!("  tsfreg -i           立即啟用輸入法");
            println!("  tsfreg -d           立即停用輸入法");
            println!("  tsfreg -u                 取消註冊");
            process::exit(1);
        }

        if let Some("-r") = env::args().nth(1).as_ref().map(|s| s.as_str()) {
            let icon_path = env::args().nth(2).expect("缺少 IconPath");
            register(icon_path)?;
        } else if let Some("-i") = env::args().nth(1).as_ref().map(|s| s.as_str()) {
            enable();
        } else if let Some("-d") = env::args().nth(1).as_ref().map(|s| s.as_str()) {
            disable();
        } else {
            if let Err(err) = unregister() {
                println!("警告：無法解除輸入法註冊，反安裝可能無法正常完成。");
                println!("錯誤訊息：{:?}", err);
            }
        }
    }

    Ok(())
}
