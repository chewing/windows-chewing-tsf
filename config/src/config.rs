// SPDX-License-Identifier: GPL-3.0-or-later

use std::ptr::null_mut;

use anyhow::{Result, bail};
use log::error;
use serde::{Deserialize, Serialize};
use windows::{
    Win32::{
        Foundation::{ERROR_SUCCESS, HLOCAL, LocalFree},
        Graphics::Direct2D::Common::D2D1_COLOR_F,
        Security::{
            AllocateAndInitializeSid,
            Authorization::{
                EXPLICIT_ACCESS_W, GetNamedSecurityInfoW, SE_OBJECT_TYPE, SE_REGISTRY_KEY,
                SET_ACCESS, SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_GROUP,
                TRUSTEE_IS_SID, TRUSTEE_W,
            },
            DACL_SECURITY_INFORMATION, FreeSid, PSECURITY_DESCRIPTOR, PSID,
            SECURITY_APP_PACKAGE_AUTHORITY, SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        },
        System::{
            Registry::{KEY_READ, KEY_WOW64_64KEY},
            SystemServices::{
                SECURITY_APP_PACKAGE_BASE_RID, SECURITY_BUILTIN_APP_PACKAGE_RID_COUNT,
                SECURITY_BUILTIN_PACKAGE_ANY_PACKAGE,
            },
        },
    },
    core::{PCWSTR, PWSTR, w},
};
use windows_registry::{CURRENT_USER, Key};

#[derive(Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Config {
    pub chewing_tsf: ChewingTsfConfig,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ChewingTsfConfig {
    pub switch_lang_with_shift: bool,
    pub enable_fullwidth_toggle_key: bool,
    pub enable_caps_lock: bool,
    pub show_notification: bool,
    pub enable_auto_learn: bool,
    pub esc_clean_all_buf: bool,
    pub full_shape_symbols: bool,
    pub upper_case_with_shift: bool,
    pub add_phrase_forward: bool,
    pub phrase_choice_rearward: bool,
    pub easy_symbols_with_shift: bool,
    pub easy_symbols_with_shift_ctrl: bool,
    pub cursor_cand_list: bool,
    pub show_cand_with_space_key: bool,
    pub advance_after_selection: bool,
    pub default_full_space: bool,
    pub default_english: bool,
    pub output_simp_chinese: bool,
    pub sel_key_type: i32,
    pub conv_engine: i32,
    pub cand_per_row: i32,
    pub cand_per_page: i32,
    pub font_size: i32,
    pub font_family: String,
    pub font_fg_color: String,
    pub font_bg_color: String,
    pub font_highlight_fg_color: String,
    pub font_highlight_bg_color: String,
    pub font_number_fg_color: String,
    pub keyboard_layout: i32,
}

impl Default for ChewingTsfConfig {
    fn default() -> Self {
        Self {
            switch_lang_with_shift: true,
            enable_fullwidth_toggle_key: false,
            enable_caps_lock: true,
            show_notification: true,
            enable_auto_learn: true,
            esc_clean_all_buf: false,
            full_shape_symbols: true,
            upper_case_with_shift: false,
            add_phrase_forward: true,
            phrase_choice_rearward: false,
            easy_symbols_with_shift: true,
            easy_symbols_with_shift_ctrl: false,
            cursor_cand_list: true,
            show_cand_with_space_key: false,
            advance_after_selection: true,
            default_full_space: false,
            default_english: false,
            output_simp_chinese: false,
            sel_key_type: 0,
            conv_engine: 1,
            cand_per_row: 3,
            cand_per_page: 9,
            font_size: 16,
            font_family: "Segoe UI".to_owned(),
            font_fg_color: "000000FF".to_owned(),
            font_bg_color: "FAFAFAFF".to_owned(),
            font_highlight_fg_color: "FFFFFFFF".to_owned(),
            font_highlight_bg_color: "000000FF".to_owned(),
            font_number_fg_color: "0000FFFF".to_owned(),
            keyboard_layout: 0,
        }
    }
}

impl Config {
    pub fn reload_if_needed(&mut self) -> Result<bool> {
        let cfg = Config::from_reg()?;
        if cfg == *self {
            Ok(false)
        } else {
            *self = cfg;
            Ok(true)
        }
    }
    pub fn from_reg() -> Result<Config> {
        let key = CURRENT_USER
            .options()
            .read()
            .access(KEY_WOW64_64KEY.0)
            .open("Software\\ChewingTextService")?;
        let mut cfg = ChewingTsfConfig::default();

        // if let Ok(path) = user_symbols_dat_path() {
        //     cfg.set_symbols_dat(fs::read_to_string(path)?.into());
        // } else {
        //     if let Ok(path) = system_symbols_dat_path() {
        //         cfg.set_symbols_dat(fs::read_to_string(path)?.into());
        //     }
        // }

        // Load custom value from the registry
        if let Ok(value) = reg_get_i32(&key, "KeyboardLayout") {
            cfg.keyboard_layout = value;
        }
        if let Ok(value) = reg_get_i32(&key, "CandPerRow") {
            cfg.cand_per_row = value;
        }
        if let Ok(value) = reg_get_bool(&key, "DefaultEnglish") {
            cfg.default_english = value;
        }
        if let Ok(value) = reg_get_bool(&key, "DefaultFullSpace") {
            cfg.default_full_space = value;
        }
        if let Ok(value) = reg_get_bool(&key, "ShowCandWithSpaceKey") {
            cfg.show_cand_with_space_key = value;
        }
        if let Ok(value) = reg_get_bool(&key, "SwitchLangWithShift") {
            cfg.switch_lang_with_shift = value;
        }
        if let Ok(value) = reg_get_bool(&key, "ShowNotification") {
            cfg.show_notification = value;
        }
        if let Ok(value) = reg_get_bool(&key, "OutputSimpChinese") {
            cfg.output_simp_chinese = value;
        }
        if let Ok(value) = reg_get_bool(&key, "AddPhraseForward") {
            cfg.add_phrase_forward = value;
        }
        if let Ok(value) = reg_get_bool(&key, "PhraseChoiceRearward") {
            cfg.phrase_choice_rearward = value;
        }
        if let Ok(value) = reg_get_bool(&key, "AdvanceAfterSelection") {
            cfg.advance_after_selection = value;
        }
        if let Ok(value) = reg_get_i32(&key, "DefFontSize") {
            cfg.font_size = value;
        }
        if let Ok(value) = key.get_string("DefFontFamily") {
            cfg.font_family = value;
        }
        if let Ok(value) = key.get_string("DefFontFgColor") {
            cfg.font_fg_color = value;
        }
        if let Ok(value) = key.get_string("DefFontBgColor") {
            cfg.font_bg_color = value;
        }
        if let Ok(value) = key.get_string("DefFontHighlightFgColor") {
            cfg.font_highlight_fg_color = value;
        }
        if let Ok(value) = key.get_string("DefFontHighlightBgColor") {
            cfg.font_highlight_bg_color = value;
        }
        if let Ok(value) = key.get_string("DefFontNumberFgColor") {
            cfg.font_number_fg_color = value;
        }
        if let Ok(value) = reg_get_i32(&key, "SelKeyType") {
            cfg.sel_key_type = value;
        }
        if let Ok(value) = reg_get_i32(&key, "ConvEngine") {
            cfg.conv_engine = value;
        }
        if let Ok(value) = reg_get_i32(&key, "SelAreaLen") {
            cfg.cand_per_page = value;
        }
        if let Ok(value) = reg_get_bool(&key, "CursorCandList") {
            cfg.cursor_cand_list = value;
        }
        if let Ok(value) = reg_get_bool(&key, "EnableCapsLock") {
            cfg.enable_caps_lock = value;
        }
        if let Ok(value) = reg_get_bool(&key, "EnableAutoLearn") {
            cfg.enable_auto_learn = value;
        }
        if let Ok(value) = reg_get_bool(&key, "FullShapeSymbols") {
            cfg.full_shape_symbols = value;
        }
        if let Ok(value) = reg_get_bool(&key, "EscCleanAllBuf") {
            cfg.esc_clean_all_buf = value;
        }
        if let Ok(value) = reg_get_bool(&key, "EasySymbolsWithShift") {
            cfg.easy_symbols_with_shift = value;
        }
        if let Ok(value) = reg_get_bool(&key, "EasySymbolsWithShiftCtrl") {
            cfg.easy_symbols_with_shift_ctrl = value;
        }
        if let Ok(value) = reg_get_bool(&key, "UpperCaseWithShift") {
            cfg.upper_case_with_shift = value;
        }
        if let Ok(value) = reg_get_bool(&key, "EnableFullwidthToggleKey") {
            cfg.enable_fullwidth_toggle_key = value;
        }

        Ok(Config { chewing_tsf: cfg })
    }
    pub fn save_reg(&self) {
        let chewing_tsf = &self.chewing_tsf;

        let Ok(key) = CURRENT_USER
            .options()
            .create()
            .access(KEY_WOW64_64KEY.0)
            .write()
            .open("Software\\ChewingTextService")
        else {
            error!("Unable to open registry for write");
            return;
        };

        let _ = reg_set_i32(&key, "KeyboardLayout", chewing_tsf.keyboard_layout);
        let _ = reg_set_i32(&key, "CandPerRow", chewing_tsf.cand_per_row);
        let _ = reg_set_bool(&key, "DefaultEnglish", chewing_tsf.default_english);
        let _ = reg_set_bool(&key, "DefaultFullSpace", chewing_tsf.default_full_space);
        let _ = reg_set_bool(
            &key,
            "ShowCandWithSpaceKey",
            chewing_tsf.show_cand_with_space_key,
        );
        let _ = reg_set_bool(
            &key,
            "SwitchLangWithShift",
            chewing_tsf.switch_lang_with_shift,
        );
        let _ = reg_set_bool(
            &key,
            "EnableFullwidthToggleKey",
            chewing_tsf.enable_fullwidth_toggle_key,
        );
        let _ = reg_set_bool(&key, "ShowNotification", chewing_tsf.show_notification);
        let _ = reg_set_bool(&key, "OutputSimpChinese", chewing_tsf.output_simp_chinese);
        let _ = reg_set_bool(&key, "AddPhraseForward", chewing_tsf.add_phrase_forward);
        let _ = reg_set_bool(
            &key,
            "PhraseChoiceRearward",
            chewing_tsf.phrase_choice_rearward,
        );
        let _ = reg_set_bool(
            &key,
            "AdvanceAfterSelection",
            chewing_tsf.advance_after_selection,
        );
        let _ = reg_set_i32(&key, "DefFontSize", chewing_tsf.font_size);
        let _ = key.set_string("DefFontFamily", &chewing_tsf.font_family);
        let _ = key.set_string("DefFontFgColor", &chewing_tsf.font_fg_color);
        let _ = key.set_string("DefFontBgColor", &chewing_tsf.font_bg_color);
        let _ = key.set_string(
            "DefFontHighlightFgColor",
            &chewing_tsf.font_highlight_fg_color,
        );
        let _ = key.set_string(
            "DefFontHighlightBgColor",
            &chewing_tsf.font_highlight_bg_color,
        );
        let _ = key.set_string("DefFontNumberFgColor", &chewing_tsf.font_number_fg_color);
        let _ = reg_set_i32(&key, "SelKeyType", chewing_tsf.sel_key_type);
        let _ = reg_set_i32(&key, "ConvEngine", chewing_tsf.conv_engine);
        let _ = reg_set_i32(&key, "SelAreaLen", chewing_tsf.cand_per_page);
        let _ = reg_set_bool(&key, "CursorCandList", chewing_tsf.cursor_cand_list);
        let _ = reg_set_bool(&key, "EnableCapsLock", chewing_tsf.enable_caps_lock);
        let _ = reg_set_bool(&key, "FullShapeSymbols", chewing_tsf.full_shape_symbols);
        let _ = reg_set_bool(&key, "EscCleanAllBuf", chewing_tsf.esc_clean_all_buf);
        let _ = reg_set_bool(
            &key,
            "EasySymbolsWithShift",
            chewing_tsf.easy_symbols_with_shift,
        );
        let _ = reg_set_bool(
            &key,
            "EasySymbolsWithShiftCtrl",
            chewing_tsf.easy_symbols_with_shift_ctrl,
        );
        let _ = reg_set_bool(
            &key,
            "UpperCaseWithShift",
            chewing_tsf.upper_case_with_shift,
        );
        let _ = reg_set_bool(&key, "EnableAutoLearn", chewing_tsf.enable_auto_learn);

        // AppContainer app, like the SearchHost.exe powering the start menu search bar
        // needs this to access the settings.
        if let Err(error) = grant_app_container_access(
            w!(r"CURRENT_USER\Software\ChewingTextService"),
            SE_REGISTRY_KEY,
            KEY_READ.0,
        ) {
            error!("Failed to grant app container access: {error:#}");
        }
    }
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

fn reg_get_i32(hk: &Key, value_name: &str) -> Result<i32> {
    Ok(hk.get_u32(value_name)? as i32)
}

fn reg_get_bool(hk: &Key, value_name: &str) -> Result<bool> {
    Ok(hk.get_u32(value_name)? > 0)
}

fn reg_set_i32(hk: &Key, value_name: &str, value: i32) -> Result<()> {
    Ok(hk.set_u32(value_name, value as u32)?)
}

fn reg_set_bool(hk: &Key, value_name: &str, value: bool) -> Result<()> {
    Ok(hk.set_u32(value_name, value as u32)?)
}

pub fn color_f(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

// XXX: Rust and LLVM assumes the floating point environment is in the default
// state and divide by zero does not trigger exception. However, chewing_tip is
// loaded to host program that may set the MXCSR register and trigger UB. Never
// inline this function to ensure the 4 values used by the SSE instruction DIVPS
// are all defined.
//
// Reference:
// * https://github.com/rust-lang/unsafe-code-guidelines/issues/471
// * https://github.com/chewing/windows-chewing-tsf/issues/412
#[inline(never)]
pub fn color_uf(r: u16, g: u16, b: u16, a: u16) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (r as f32) / 255.0,
        g: (g as f32) / 255.0,
        b: (b as f32) / 255.0,
        a: (a as f32) / 255.0,
    }
}

pub fn color_s(rgb: &str) -> D2D1_COLOR_F {
    let mut rgb_u32 = u32::from_str_radix(rgb, 16).unwrap_or(0);
    let a = if rgb.len() > 6 {
        let a = rgb_u32 & 0xFF;
        rgb_u32 >>= 8;
        a as u16
    } else {
        255
    };
    let r = ((rgb_u32 >> 16) & 0xFF) as u16;
    let g = ((rgb_u32 >> 8) & 0xFF) as u16;
    let b = (rgb_u32 & 0xFF) as u16;
    color_uf(r, g, b, a)
}

#[cfg(test)]
mod test {
    use super::{color_f, color_s};

    #[test]
    fn color_rgb() {
        assert_eq!(color_f(1.0, 0.0, 1.0, 1.0), color_s("FF00FF"));
    }
    #[test]
    fn color_rgba() {
        assert_eq!(color_f(1.0, 0.0, 1.0, 0.0), color_s("FF00FF00"));
    }
    #[test]
    fn color_alpha_only() {
        assert_eq!(color_f(0.0, 0.0, 1.0, 1.0), color_s("0000FFFF"));
        assert_eq!(color_f(0.0, 0.0, 0.0, 1.0), color_s("000000FF"));
    }
}
