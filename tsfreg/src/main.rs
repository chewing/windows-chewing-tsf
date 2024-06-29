use std::{env, process};

use windows::{
    core::*,
    Win32::{Globalization::*, System::Com::*, UI::TextServices::*},
};

const CHEWING_TSF_CLSID: GUID = GUID::from_u128(0x13F2EF08_575C_4D8C_88E0_F67BB8052B84);
const CHEWING_PROFILE_GUID: GUID = GUID::from_u128(0xCE45F71D_CE79_41D1_967D_640B65A380E3);

fn register(dllpath: String) -> Result<()> {
    unsafe {
        let input_processor_profiles: ITfInputProcessorProfiles =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        input_processor_profiles.Register(&CHEWING_TSF_CLSID)?;

        let mut lcid = LocaleNameToLCID(w!("zh-Hant-TW"), 0);
        if lcid == 0 {
            lcid = LocaleNameToLCID(w!("zh-TW"), 0);
        }
        let pw_icon_path = dllpath.encode_utf16().collect::<Vec<_>>();
        input_processor_profiles.AddLanguageProfile(
            &CHEWING_TSF_CLSID,
            lcid.try_into().unwrap(),
            &CHEWING_PROFILE_GUID,
            w!("新酷音輸入法").as_wide(),
            &pw_icon_path,
            1,
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
    Ok(())
}

fn unregister() -> Result<()> {
    unsafe {
        let input_processor_profiles: ITfInputProcessorProfiles =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        input_processor_profiles.Unregister(&CHEWING_TSF_CLSID)?;
    }
    Ok(())
}

fn main() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

        if env::args().len() == 1 {
            println!("Usage:");
            println!("  tsfreg -r <DllPath>    註冊輸入法");
            println!("  tsfreg -u                 取消註冊");
            process::exit(1);
        }

        if let Some("-r") = env::args().nth(1).as_ref().map(|s| s.as_str()) {
            let dllpath = env::args().nth(2).expect("缺少 DllPath");
            register(dllpath)?;
        } else {
            unregister()?;
        }
    }

    Ok(())
}
