// SPDX-License-Identifier: GPL-3.0-or-later

use std::ptr::null_mut;
use std::rc::Rc;
use std::{env, fs, path::PathBuf};

use anyhow::{Result, bail};
use chewing::path::data_dir;
use log::error;
use slint::ComponentHandle;
use slint::ModelRc;
use slint::VecModel;
use windows::Win32::Foundation::{ERROR_SUCCESS, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    EXPLICIT_ACCESS_W, GetNamedSecurityInfoW, SE_OBJECT_TYPE, SE_REGISTRY_KEY, SET_ACCESS,
    SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_GROUP, TRUSTEE_IS_SID, TRUSTEE_W,
};
use windows::Win32::Security::{
    AllocateAndInitializeSid, DACL_SECURITY_INFORMATION, FreeSid, PSECURITY_DESCRIPTOR, PSID,
    SECURITY_APP_PACKAGE_AUTHORITY, SUB_CONTAINERS_AND_OBJECTS_INHERIT,
};
use windows::Win32::System::Registry::KEY_READ;
use windows::Win32::System::SystemServices::{
    SECURITY_APP_PACKAGE_BASE_RID, SECURITY_BUILTIN_APP_PACKAGE_RID_COUNT,
    SECURITY_BUILTIN_PACKAGE_ANY_PACKAGE,
};
use windows::core::{PCWSTR, PWSTR, w};
use windows_registry::{CURRENT_USER, Key};

use crate::AboutWindow;
use crate::ConfigWindow;

const KEY_WOW64_64KEY: u32 = 0x0100;

pub fn run() -> Result<()> {
    let about = AboutWindow::new()?;
    let ui = ConfigWindow::new()?;
    let families = crate::fonts::enum_font_families()?;
    let model = Rc::new(VecModel::from(families));
    ui.set_font_families(ModelRc::from(model));
    load_config(&ui)?;

    ui.on_cancel(move || {
        slint::quit_event_loop().unwrap();
    });
    let ui_handle = ui.as_weak();
    ui.on_apply(move || {
        let ui = ui_handle.upgrade().unwrap();
        save_config(&ui).unwrap();
    });
    let ui_handle = ui.as_weak();
    ui.on_apply_and_quit(move || {
        let ui = ui_handle.upgrade().unwrap();
        save_config(&ui).unwrap();
        slint::quit_event_loop().unwrap();
    });
    let about_handle = about.as_weak();
    about.on_done(move || {
        let about = about_handle.upgrade().unwrap();
        about.hide().unwrap();
    });
    ui.on_about(move || {
        about.show().unwrap();
    });

    ui.run()?;
    Ok(())
}

fn reg_get_i32(hk: &Key, value_name: &str) -> Result<i32> {
    Ok(hk.get_u32(value_name)? as i32)
}

fn reg_set_i32(hk: &Key, value_name: &str, value: i32) -> Result<()> {
    Ok(hk.set_u32(value_name, value as u32)?)
}

fn reg_get_bool(hk: &Key, value_name: &str) -> Result<bool> {
    Ok(hk.get_u32(value_name)? > 0)
}

fn reg_set_bool(hk: &Key, value_name: &str, value: bool) -> Result<()> {
    Ok(hk.set_u32(value_name, value as u32)?)
}

fn default_user_path_for_file(file: &str) -> PathBuf {
    let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\unknown".into());
    let user_data_dir = PathBuf::from(user_profile).join("ChewingTextService");
    data_dir().unwrap_or(user_data_dir).join(file)
}

fn user_path_for_file(file: &str) -> Result<PathBuf> {
    let user_file = default_user_path_for_file(file);
    if user_file.exists() {
        return Ok(user_file);
    }
    bail!("使用者檔案 {file} 不存在")
}

// FIXME: provide path info from libchewing
fn system_path_for_file(file: &str) -> Result<PathBuf> {
    let progfiles_x86 =
        env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files(x86)".into());
    let progfiles = env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
    let path_x86 = PathBuf::from(progfiles_x86)
        .join("ChewingTextService\\Dictionary")
        .join(file);
    let path = PathBuf::from(progfiles)
        .join("ChewingTextService\\Dictionary")
        .join(file);
    if path_x86.exists() {
        return Ok(path_x86);
    }
    if path.exists() {
        return Ok(path);
    }
    bail!("系統詞庫 {file} 不存在")
}

fn load_config(ui: &ConfigWindow) -> Result<()> {
    // Init settings to default value
    ui.set_cand_per_row(3);
    ui.set_switch_lang_with_shift(true);
    ui.set_enable_fullwidth_toggle_key(false);
    ui.set_show_notification(true);
    ui.set_add_phrase_forward(true);
    ui.set_advance_after_selection(true);
    ui.set_conv_engine(1);
    ui.set_cand_per_page(9);
    ui.set_cursor_cand_list(true);
    ui.set_enable_caps_lock(true);
    ui.set_full_shape_symbols(true);
    ui.set_easy_symbols_with_shift(true);
    ui.set_enable_auto_learn(true);
    ui.set_font_size(16);
    ui.set_font_family("Segoe UI".into());
    ui.set_font_fg_color("000000".into());
    ui.set_font_bg_color("FAFAFA".into());
    ui.set_font_highlight_fg_color("FFFFFF".into());
    ui.set_font_highlight_bg_color("000000".into());
    ui.set_font_number_fg_color("0000FF".into());

    if let Ok(path) = user_path_for_file("symbols.dat") {
        ui.set_symbols_dat(fs::read_to_string(path)?.into());
    } else if let Ok(path) = system_path_for_file("symbols.dat") {
        ui.set_symbols_dat(fs::read_to_string(path)?.into());
    }

    if let Ok(path) = user_path_for_file("swkb.dat") {
        ui.set_swkb_dat(fs::read_to_string(path)?.into());
    } else if let Ok(path) = system_path_for_file("swkb.dat") {
        ui.set_swkb_dat(fs::read_to_string(path)?.into());
    }

    let key = CURRENT_USER
        .options()
        .create()
        .read()
        .access(KEY_WOW64_64KEY)
        .open("Software\\ChewingTextService")?;
    // Load custom value from the registry
    if let Ok(value) = reg_get_i32(&key, "KeyboardLayout") {
        ui.set_keyboard_layout(value);
    }
    if let Ok(value) = reg_get_i32(&key, "CandPerRow") {
        ui.set_cand_per_row(value);
    }
    if let Ok(value) = reg_get_bool(&key, "DefaultEnglish") {
        ui.set_default_english(value);
    }
    if let Ok(value) = reg_get_bool(&key, "DefaultFullSpace") {
        ui.set_default_full_space(value);
    }
    if let Ok(value) = reg_get_bool(&key, "ShowCandWithSpaceKey") {
        ui.set_show_cand_with_space_key(value);
    }
    if let Ok(value) = reg_get_bool(&key, "SwitchLangWithShift") {
        ui.set_switch_lang_with_shift(value);
    }
    if let Ok(value) = reg_get_bool(&key, "EnableFullwidthToggleKey") {
        ui.set_enable_fullwidth_toggle_key(value);
    }
    if let Ok(value) = reg_get_bool(&key, "ShowNotification") {
        ui.set_show_notification(value);
    }
    if let Ok(value) = reg_get_bool(&key, "OutputSimpChinese") {
        ui.set_output_simp_chinese(value);
    }
    if let Ok(value) = reg_get_bool(&key, "AddPhraseForward") {
        ui.set_add_phrase_forward(value);
    }
    if let Ok(value) = reg_get_bool(&key, "PhraseChoiceRearward") {
        ui.set_phrase_choice_rearward(value);
    }
    // if let Ok(value) = reg_get_bool(&key, "ColorCandWnd") {
    //     ui.set_color_cand_wnd(value);
    // }
    if let Ok(value) = reg_get_bool(&key, "AdvanceAfterSelection") {
        ui.set_advance_after_selection(value);
    }
    if let Ok(value) = reg_get_i32(&key, "DefFontSize") {
        ui.set_font_size(value);
    }
    if let Ok(value) = key.get_string("DefFontFamily") {
        ui.set_font_family(value.into());
    }
    if let Ok(value) = key.get_string("DefFontFgColor") {
        ui.set_font_fg_color(value.into());
    }
    if let Ok(value) = key.get_string("DefFontBgColor") {
        ui.set_font_bg_color(value.into());
    }
    if let Ok(value) = key.get_string("DefFontHighlightFgColor") {
        ui.set_font_highlight_fg_color(value.into());
    }
    if let Ok(value) = key.get_string("DefFontHighlightBgColor") {
        ui.set_font_highlight_bg_color(value.into());
    }
    if let Ok(value) = key.get_string("DefFontNumberFgColor") {
        ui.set_font_number_fg_color(value.into());
    }
    if let Ok(value) = reg_get_i32(&key, "SelKeyType") {
        ui.set_sel_key_type(value);
    }
    if let Ok(value) = reg_get_i32(&key, "ConvEngine") {
        ui.set_conv_engine(value);
    }
    if let Ok(value) = reg_get_i32(&key, "SelAreaLen") {
        ui.set_cand_per_page(value);
    }
    if let Ok(value) = reg_get_bool(&key, "CursorCandList") {
        ui.set_cursor_cand_list(value);
    }
    if let Ok(value) = reg_get_bool(&key, "EnableCapsLock") {
        ui.set_enable_caps_lock(value);
    }
    if let Ok(value) = reg_get_bool(&key, "FullShapeSymbols") {
        ui.set_full_shape_symbols(value);
    }
    // if let Ok(value) = reg_get_bool(&key, "PhraseMark") {
    //     ui.set_phrase_mark(value);
    // }
    if let Ok(value) = reg_get_bool(&key, "EscCleanAllBuf") {
        ui.set_esc_clean_all_buf(value);
    }
    if let Ok(value) = reg_get_bool(&key, "EasySymbolsWithShift") {
        ui.set_easy_symbols_with_shift(value);
    }
    if let Ok(value) = reg_get_bool(&key, "EasySymbolsWithShiftCtrl") {
        ui.set_easy_symbols_with_shift_ctrl(value);
    }
    if let Ok(value) = reg_get_bool(&key, "UpperCaseWithShift") {
        ui.set_upper_case_with_shift(value);
    }
    if let Ok(value) = reg_get_bool(&key, "EnableAutoLearn") {
        ui.set_enable_auto_learn(value);
    }

    Ok(())
}

fn save_config(ui: &ConfigWindow) -> Result<()> {
    let key = CURRENT_USER
        .options()
        .create()
        .access(KEY_WOW64_64KEY)
        .write()
        .open("Software\\ChewingTextService")?;

    let _ = reg_set_i32(&key, "KeyboardLayout", ui.get_keyboard_layout());
    let _ = reg_set_i32(&key, "CandPerRow", ui.get_cand_per_row());
    let _ = reg_set_bool(&key, "DefaultEnglish", ui.get_default_english());
    let _ = reg_set_bool(&key, "DefaultFullSpace", ui.get_default_full_space());
    let _ = reg_set_bool(
        &key,
        "ShowCandWithSpaceKey",
        ui.get_show_cand_with_space_key(),
    );
    let _ = reg_set_bool(&key, "SwitchLangWithShift", ui.get_switch_lang_with_shift());
    let _ = reg_set_bool(
        &key,
        "EnableFullwidthToggleKey",
        ui.get_enable_fullwidth_toggle_key(),
    );
    let _ = reg_set_bool(&key, "ShowNotification", ui.get_show_notification());
    let _ = reg_set_bool(&key, "OutputSimpChinese", ui.get_output_simp_chinese());
    let _ = reg_set_bool(&key, "AddPhraseForward", ui.get_add_phrase_forward());
    let _ = reg_set_bool(
        &key,
        "PhraseChoiceRearward",
        ui.get_phrase_choice_rearward(),
    );
    // let _ = reg_set_i32(&key, "ColorCandWnd", ui.get_color_cand_wnd());
    let _ = reg_set_bool(
        &key,
        "AdvanceAfterSelection",
        ui.get_advance_after_selection(),
    );
    let _ = reg_set_i32(&key, "DefFontSize", ui.get_font_size());
    let _ = key.set_string("DefFontFamily", ui.get_font_family());
    let _ = key.set_string("DefFontFgColor", ui.get_font_fg_color());
    let _ = key.set_string("DefFontBgColor", ui.get_font_bg_color());
    let _ = key.set_string("DefFontHighlightFgColor", ui.get_font_highlight_fg_color());
    let _ = key.set_string("DefFontHighlightBgColor", ui.get_font_highlight_bg_color());
    let _ = key.set_string("DefFontNumberFgColor", ui.get_font_number_fg_color());
    let _ = reg_set_i32(&key, "SelKeyType", ui.get_sel_key_type());
    let _ = reg_set_i32(&key, "ConvEngine", ui.get_conv_engine());
    let _ = reg_set_i32(&key, "SelAreaLen", ui.get_cand_per_page());
    let _ = reg_set_bool(&key, "CursorCandList", ui.get_cursor_cand_list());
    let _ = reg_set_bool(&key, "EnableCapsLock", ui.get_enable_caps_lock());
    let _ = reg_set_bool(&key, "FullShapeSymbols", ui.get_full_shape_symbols());
    // let _ = reg_set_bool(&key, "PhraseMark", ui.get_phrase_mark());
    let _ = reg_set_bool(&key, "EscCleanAllBuf", ui.get_esc_clean_all_buf());
    let _ = reg_set_bool(
        &key,
        "EasySymbolsWithShift",
        ui.get_easy_symbols_with_shift(),
    );
    let _ = reg_set_bool(
        &key,
        "EasySymbolsWithShiftCtrl",
        ui.get_easy_symbols_with_shift_ctrl(),
    );
    let _ = reg_set_bool(&key, "UpperCaseWithShift", ui.get_upper_case_with_shift());
    let _ = reg_set_bool(&key, "EnableAutoLearn", ui.get_enable_auto_learn());

    let sys_symbols_dat = system_path_for_file("symbols.dat")
        .and_then(|path| Ok(fs::read_to_string(path)?))
        .unwrap_or_default();
    if ui.get_symbols_dat() != sys_symbols_dat {
        let user_symbols_dat_path = default_user_path_for_file("symbols.dat");
        fs::create_dir_all(user_symbols_dat_path.parent().unwrap())?;
        fs::write(user_symbols_dat_path, ui.get_symbols_dat())?;
    }

    let sys_swkb_dat = system_path_for_file("swkb.dat")
        .and_then(|path| Ok(fs::read_to_string(path)?))
        .unwrap_or_default();
    if ui.get_swkb_dat() != sys_swkb_dat {
        let user_swkb_dat_path = default_user_path_for_file("swkb.dat");
        fs::create_dir_all(user_swkb_dat_path.parent().unwrap())?;
        fs::write(user_swkb_dat_path, ui.get_swkb_dat())?;
    }

    // AppContainer app, like the SearchHost.exe powering the start menu search bar
    // needs this to access the settings.
    if let Err(error) = grant_app_container_access(
        w!(r"CURRENT_USER\Software\ChewingTextService"),
        SE_REGISTRY_KEY,
        KEY_READ.0,
    ) {
        error!("Failed to grant app container access: {error:#}");
    }

    Ok(())
}

fn grant_app_container_access(object: PCWSTR, typ: SE_OBJECT_TYPE, access: u32) -> Result<()> {
    let mut success = false;
    let mut old_acl_mut_ptr = null_mut();
    let mut new_acl_mut_ptr = null_mut();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // Get old security descriptor
    unsafe {
        if GetNamedSecurityInfoW(
            object,
            typ,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut old_acl_mut_ptr),
            None,
            &mut sd,
        ) == ERROR_SUCCESS
        {
            // Create a well-known SID for the all appcontainers group.
            let mut psid = PSID::default();
            if AllocateAndInitializeSid(
                &SECURITY_APP_PACKAGE_AUTHORITY,
                SECURITY_BUILTIN_APP_PACKAGE_RID_COUNT as u8,
                SECURITY_APP_PACKAGE_BASE_RID as u32,
                SECURITY_BUILTIN_PACKAGE_ANY_PACKAGE as u32,
                0,
                0,
                0,
                0,
                0,
                0,
                &mut psid,
            )
            .is_ok()
            {
                let ea = EXPLICIT_ACCESS_W {
                    grfAccessPermissions: access,
                    grfAccessMode: SET_ACCESS,
                    grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
                    Trustee: TRUSTEE_W {
                        TrusteeForm: TRUSTEE_IS_SID,
                        TrusteeType: TRUSTEE_IS_GROUP,
                        ptstrName: PWSTR::from_raw(psid.0.cast()),
                        ..Default::default()
                    },
                };
                // Add the new entry to the existing DACL
                if SetEntriesInAclW(Some(&[ea]), Some(old_acl_mut_ptr), &mut new_acl_mut_ptr)
                    == ERROR_SUCCESS
                {
                    // Set the new DACL back to the object
                    if SetNamedSecurityInfoW(
                        object,
                        typ,
                        DACL_SECURITY_INFORMATION,
                        None,
                        None,
                        Some(new_acl_mut_ptr),
                        None,
                    ) == ERROR_SUCCESS
                    {
                        success = true;
                    }
                }
                FreeSid(psid);
            }
        }
        if !sd.is_invalid() {
            LocalFree(Some(HLOCAL(sd.0)));
        }
        if !new_acl_mut_ptr.is_null() {
            LocalFree(Some(HLOCAL(new_acl_mut_ptr.cast())));
        }
    }
    if success {
        Ok(())
    } else {
        bail!("Unable to update security descriptor");
    }
}
