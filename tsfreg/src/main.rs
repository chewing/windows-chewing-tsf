// SPDX-License-Identifier: GPL-3.0-or-later

use std::{env, process};

use windows::{
    Win32::{
        Globalization::*,
        System::Com::*,
        UI::{Input::KeyboardAndMouse::HKL, TextServices::*},
    },
    core::*,
};
use windows_registry::LOCAL_MACHINE;

const CHEWING_TSF_CLSID: GUID = GUID::from_u128(0x13F2EF08_575C_4D8C_88E0_F67BB8052B84);
const CHEWING_PROFILE_GUID: GUID = GUID::from_u128(0xCE45F71D_CE79_41D1_967D_640B65A380E3);

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
            true,
            0,
        )?;

        let category_manager: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;

        category_manager.RegisterCategory(
            &CHEWING_TSF_CLSID,
            &GUID_TFCAT_TIP_KEYBOARD,
            &CHEWING_TSF_CLSID,
        )?;
        category_manager.RegisterCategory(
            &CHEWING_TSF_CLSID,
            &GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
            &CHEWING_TSF_CLSID,
        )?;
        category_manager.RegisterCategory(
            &CHEWING_TSF_CLSID,
            &GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
            &CHEWING_TSF_CLSID,
        )?;
        category_manager.RegisterCategory(
            &CHEWING_TSF_CLSID,
            &GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            &CHEWING_TSF_CLSID,
        )?;

        category_manager.RegisterCategory(
            &CHEWING_TSF_CLSID,
            &GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
            &CHEWING_TSF_CLSID,
        )?;
        category_manager.RegisterCategory(
            &CHEWING_TSF_CLSID,
            &GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
            &CHEWING_TSF_CLSID,
        )?;
    }

    #[cfg(debug_assertions)]
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

fn main() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

        if env::args().len() == 1 {
            println!("Usage:");
            println!("  tsfreg -r <IconPath>    註冊輸入法");
            println!("  tsfreg -u                 取消註冊");
            process::exit(1);
        }

        if let Some("-r") = env::args().nth(1).as_ref().map(|s| s.as_str()) {
            let icon_path = env::args().nth(2).expect("缺少 IconPath");
            register(icon_path)?;
        } else {
            if let Err(err) = unregister() {
                println!("警告：無法解除輸入法註冊，反安裝可能無法正常完成。");
                println!("錯誤訊息：{:?}", err);
            }
        }
    }

    Ok(())
}
