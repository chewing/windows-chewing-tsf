// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use clap::Parser;
use windows::{
    Win32::{
        Globalization::*,
        System::Com::*,
        UI::{Input::KeyboardAndMouse::HKL, TextServices::*},
    },
    core::*,
};
use windows_registry::LOCAL_MACHINE;

// https://learn.microsoft.com/en-us/windows/win32/tsf/installlayoutortip
windows::core::link!("input.dll" "system" fn InstallLayoutOrTip(psz: *const u16, dwFlags: u32));
const ILOT_INSTALL: u32 = 0x00000000;
// const ILOT_UNINSTALL: u32 = 0x00000001;

const CHEWING_TSF_CLSID: GUID = GUID::from_u128(0x13F2EF08_575C_4D8C_88E0_F67BB8052B84);
const CHEWING_ZH_TW_PROFILE_GUID: GUID = GUID::from_u128(0xCE45F71D_CE79_41D1_967D_640B65A380E3);
const CHEWING_ZH_CN_PROFILE_GUID: GUID = GUID::from_u128(0x9063E7CC_225E_4CEA_B7A1_2B0FB12A75CF);
const CHEWING_TIP_DESC: PCWSTR =
    w!("0x0404:{13F2EF08-575C-4D8C-88E0-F67BB8052B84}{CE45F71D-CE79-41D1-967D-640B65A380E3}");

const CATEGORIES: [GUID; 7] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_COMLESS,
];

fn register(icon_path: &Path) -> Result<()> {
    println!("註冊新酷音輸入法");
    unsafe {
        let input_processor_profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        let pw_icon_path = icon_path.as_os_str().encode_wide().collect::<Vec<_>>();

        // Register for zh_TW
        let mut lcid = LocaleNameToLCID(w!("zh-TW"), 0);
        if matches!(lcid, 0 | 0x0C00 | 0x1000) {
            lcid = 0x404;
        }
        input_processor_profile_mgr.RegisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_ZH_TW_PROFILE_GUID,
            w!("新酷音輸入法").as_wide(),
            &pw_icon_path,
            0,
            HKL::default(),
            0,
            false,
            0,
        )?;
        // Register for zh_CN
        let mut lcid = LocaleNameToLCID(w!("zh-CN"), 0);
        if matches!(lcid, 0 | 0x0C00 | 0x1000) {
            lcid = 0x804;
        }
        input_processor_profile_mgr.RegisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_ZH_CN_PROFILE_GUID,
            w!("新酷音输入法").as_wide(),
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

    if option_env!("CHEWING_BUILD_TYPE").is_some_and(|t| t == "nightly") {
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
    println!("取消註冊新酷音輸入法");
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

        // Unregister zh_TW profile
        let mut lcid = LocaleNameToLCID(w!("zh-TW"), 0);
        if matches!(lcid, 0 | 0x0C00 | 0x1000) {
            lcid = 0x404;
        }
        input_processor_profile_mgr.UnregisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_ZH_TW_PROFILE_GUID,
            0,
        )?;
        // Unregister zh_CN profile
        let mut lcid = LocaleNameToLCID(w!("zh-CN"), 0);
        if matches!(lcid, 0 | 0x0C00 | 0x1000) {
            lcid = 0x804;
        }
        input_processor_profile_mgr.UnregisterProfile(
            &CHEWING_TSF_CLSID,
            lcid as u16,
            &CHEWING_ZH_CN_PROFILE_GUID,
            0,
        )?;
    }

    Ok(())
}

fn enable() {
    println!("啟用新酷音輸入法");
    unsafe {
        InstallLayoutOrTip(CHEWING_TIP_DESC.as_ptr(), ILOT_INSTALL);
    }
}

fn disable() {
    // Don't uninstall the layout for now. If the last layout of a language is
    // uninstalled then Windows changes the system locale to English or another
    // available language.
    //
    // Ref: https://codeberg.org/chewing/windows-chewing-tsf/issues/553
    //
    // unsafe {
    //     InstallLayoutOrTip(CHEWING_TIP_DESC.as_ptr(), ILOT_UNINSTALL);
    // }
}

/// TSF registration helper for the Chewing IME
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// 註冊輸入法
    #[arg(short = 'r')]
    icon_path: Option<PathBuf>,
    /// 立即啟用輸入法
    #[arg(short = 'i')]
    enable: bool,
    /// 立即停用輸入法
    #[arg(short = 'd')]
    disable: bool,
    /// 取消註冊
    #[arg(short = 'u')]
    unregister: bool,
}

fn main() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;

        let args = Args::parse();

        if let Some(icon_path) = args.icon_path {
            register(&icon_path)?;
        } else if args.enable {
            enable();
        } else if args.disable {
            disable();
        } else if args.unregister
            && let Err(err) = unregister()
        {
            println!("警告：無法解除輸入法註冊，反安裝可能無法正常完成。");
            println!("錯誤訊息：{:?}", err);
        }
    }

    Ok(())
}
