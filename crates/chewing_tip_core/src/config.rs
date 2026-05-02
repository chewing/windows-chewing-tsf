// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{fmt::Display, ptr::null_mut, str::FromStr, time::SystemTime};

use log::error;
use serde::{Deserialize, Serialize};
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::{
            ACL, AllocateAndInitializeSid,
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

use crate::{impl_context_error, result::expect_error};

#[derive(Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Config {
    pub chewing_tsf: ChewingTsfConfig,
    pub symbols_dat: String,
    pub swkb_dat: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ChewingTsfConfig {
    pub switch_lang_with_shift: bool,
    pub shift_key_sensitivity: i32,
    pub enable_fullwidth_toggle_key: bool,
    pub enable_caps_lock: bool,
    pub lock_chinese_on_caps_lock: bool,
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
    pub sort_candidates_by_frequency: bool,
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
    pub cand_list_border_color: String,
    pub notify_fg_color: String,
    pub notify_bg_color: String,
    pub notify_border_color: String,
    pub keyboard_layout: i32,
    pub simulate_english_layout: i32,
    pub sync_lang_mode_openclose: bool,
    pub keybind: Vec<KeybindValue>,
    pub auto_check_update_channel: String,
    pub update_info_url: String,
    pub last_update_check_time: u64,
    pub modified_timestamp: u64,
}

impl Default for ChewingTsfConfig {
    fn default() -> Self {
        Self {
            switch_lang_with_shift: true,
            shift_key_sensitivity: 200,
            enable_fullwidth_toggle_key: false,
            enable_caps_lock: false,
            lock_chinese_on_caps_lock: true,
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
            sort_candidates_by_frequency: false,
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
            cand_list_border_color: "D6D9DBFF".to_owned(),
            notify_fg_color: "000000FF".to_owned(),
            notify_bg_color: "FCFBDAFF".to_owned(),
            notify_border_color: "D6D9DBFF".to_owned(),
            keyboard_layout: 0,
            simulate_english_layout: 0,
            sync_lang_mode_openclose: false,
            keybind: vec![
                KeybindValue {
                    key: "Ctrl+F12".to_string(),
                    action: "toggle_simplified_chinese".to_string(),
                    param: "".to_string(),
                },
                KeybindValue {
                    key: "Ctrl+Delete".to_string(),
                    action: "selecting_unlearn_phrase".to_string(),
                    param: "".to_string(),
                },
            ],
            auto_check_update_channel: "stable".to_string(),
            update_info_url: "".to_string(),
            last_update_check_time: 0,
            modified_timestamp: 0,
        }
    }
}

impl Config {
    pub fn reload_if_needed(&mut self) -> Result<bool, ConfigError> {
        let cfg = Config::from_reg()?;
        if cfg == *self {
            Ok(false)
        } else {
            *self = cfg;
            Ok(true)
        }
    }
    pub fn from_reg() -> Result<Config, ConfigError> {
        expect_error("Failed to load config from registry", || {
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
            if let Ok(value) = reg_get_i32(&key, "SimulateEnglishLayout") {
                cfg.simulate_english_layout = value;
            }
            if let Ok(value) = reg_get_bool(&key, "SyncLangModeOpenclose") {
                cfg.sync_lang_mode_openclose = value;
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
            if let Ok(value) = reg_get_i32(&key, "ShiftKeySensitivity") {
                cfg.shift_key_sensitivity = value;
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
            if let Ok(value) = key.get_string("DefCandListBorderColor") {
                cfg.cand_list_border_color = value;
            }
            if let Ok(value) = key.get_string("DefNotifyFgColor") {
                cfg.notify_fg_color = value;
            }
            if let Ok(value) = key.get_string("DefNotifyBgColor") {
                cfg.notify_bg_color = value;
            }
            if let Ok(value) = key.get_string("DefNotifyBorderColor") {
                cfg.notify_border_color = value;
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
            if let Ok(value) = reg_get_bool(&key, "SortCandidatesByFrequency") {
                cfg.sort_candidates_by_frequency = value;
            }
            if let Ok(value) = reg_get_bool(&key, "EnableCapsLock") {
                cfg.enable_caps_lock = value;
            }
            if let Ok(value) = reg_get_bool(&key, "LockChineseOnCapsLock") {
                cfg.lock_chinese_on_caps_lock = value;
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
            if let Ok(value) = key.get_string("AutoCheckUpdateChannel") {
                cfg.auto_check_update_channel = value;
            }
            if let Ok(value) = key.get_string("UpdateInfoUrl") {
                cfg.update_info_url = value;
            }
            if let Ok(value) = key.get_u64("LastUpdateCheckTime") {
                cfg.last_update_check_time = value;
            }
            if let Ok(value) = key.get_u64("ModifiedTimestamp") {
                cfg.modified_timestamp = value;
            }
            if let Ok(values) = key.get_multi_string("Keybind") {
                cfg.keybind = values
                    .into_iter()
                    .flat_map(|value| KeybindValue::from_str(&value))
                    .collect();
            }

            Ok(Config {
                chewing_tsf: cfg,
                symbols_dat: String::new(),
                swkb_dat: String::new(),
            })
        })
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
        let _ = reg_set_i32(
            &key,
            "SimulateEnglishLayout",
            chewing_tsf.simulate_english_layout,
        );
        let _ = reg_set_bool(
            &key,
            "SyncLangModeOpenclose",
            chewing_tsf.sync_lang_mode_openclose,
        );
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
        let _ = reg_set_i32(
            &key,
            "ShiftKeySensitivity",
            chewing_tsf.shift_key_sensitivity,
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
        let _ = key.set_string(
            "DefCandListBorderColor",
            &chewing_tsf.cand_list_border_color,
        );
        let _ = key.set_string("DefNotifyFgColor", &chewing_tsf.notify_fg_color);
        let _ = key.set_string("DefNotifyBgColor", &chewing_tsf.notify_bg_color);
        let _ = key.set_string("DefNotifyBorderColor", &chewing_tsf.notify_border_color);
        let _ = reg_set_i32(&key, "SelKeyType", chewing_tsf.sel_key_type);
        let _ = reg_set_i32(&key, "ConvEngine", chewing_tsf.conv_engine);
        let _ = reg_set_i32(&key, "SelAreaLen", chewing_tsf.cand_per_page);
        let _ = reg_set_bool(&key, "CursorCandList", chewing_tsf.cursor_cand_list);
        let _ = reg_set_bool(
            &key,
            "SortCandidatesByFrequency",
            chewing_tsf.sort_candidates_by_frequency,
        );
        let _ = reg_set_bool(&key, "EnableCapsLock", chewing_tsf.enable_caps_lock);
        let _ = reg_set_bool(
            &key,
            "LockChineseOnCapsLock",
            chewing_tsf.lock_chinese_on_caps_lock,
        );
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
        let _ = key.set_string(
            "AutoCheckUpdateChannel",
            &chewing_tsf.auto_check_update_channel,
        );
        let _ = key.set_multi_string(
            "Keybind".to_string(),
            chewing_tsf
                .keybind
                .iter()
                .map(|kb| kb.to_string())
                .collect::<Vec<String>>()
                .as_slice(),
        );
        let _ = key.set_u64(
            "ModifiedTimestamp",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

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

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct KeybindValue {
    pub key: String,
    pub action: String,
    pub param: String,
}

impl FromStr for KeybindValue {
    type Err = ConfigError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        expect_error("Failed to parse keybinding", || {
            let (key, action) = s.rsplit_once('=').ok_or("missing seperator =")?;
            let (action, param) = if action.contains(':') {
                action.rsplit_once(':').ok_or("missing seperator :")?
            } else {
                (action, "")
            };
            Ok(KeybindValue {
                key: key.to_string(),
                action: action.to_string(),
                param: param.to_string(),
            })
        })
    }
}

impl Display for KeybindValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key.trim(), self.action.trim())?;
        if !self.param.is_empty() {
            write!(f, ":{}", self.param)?;
        }
        Ok(())
    }
}

fn grant_app_container_access(
    object: PCWSTR,
    typ: SE_OBJECT_TYPE,
    access: u32,
) -> Result<(), ConfigError> {
    #[derive(Default)]
    struct AclSdGuard {
        new_acl_mut_ptr: *mut ACL,
        sd: PSECURITY_DESCRIPTOR,
    }
    impl Drop for AclSdGuard {
        fn drop(&mut self) {
            unsafe {
                if !self.sd.is_invalid() {
                    LocalFree(Some(HLOCAL(self.sd.0)));
                }
                if !self.new_acl_mut_ptr.is_null() {
                    LocalFree(Some(HLOCAL(self.new_acl_mut_ptr.cast())));
                }
            }
        }
    }
    expect_error("Failed to grant AppContainer access to object", || {
        let mut old_acl_mut_ptr = null_mut();
        let mut result = AclSdGuard::default();
        // Get old security descriptor
        unsafe {
            GetNamedSecurityInfoW(
                object,
                typ,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(&mut old_acl_mut_ptr),
                None,
                &mut result.sd,
            )
            .ok()?;

            // Create a well-known SID for the all appcontainers group.
            let mut psid = PSID::default();
            AllocateAndInitializeSid(
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
            )?;

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
            SetEntriesInAclW(
                Some(&[ea]),
                Some(old_acl_mut_ptr),
                &mut result.new_acl_mut_ptr,
            )
            .ok()?;
            // Set the new DACL back to the object
            SetNamedSecurityInfoW(
                object,
                typ,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(result.new_acl_mut_ptr),
                None,
            )
            .ok()?;

            FreeSid(psid);
        }
        Ok(())
    })
}

fn reg_get_i32(hk: &Key, value_name: &str) -> Result<i32, ConfigError> {
    hk.get_u32(value_name)
        .map(|v| v as i32)
        .map_err(|e| ConfigError {
            msg: "Failed to read config as i32",
            source: e.into(),
        })
}

fn reg_get_bool(hk: &Key, value_name: &str) -> Result<bool, ConfigError> {
    hk.get_u32(value_name)
        .map(|v| v > 0)
        .map_err(|e| ConfigError {
            msg: "Failed to read config as bool",
            source: e.into(),
        })
}

fn reg_set_i32(hk: &Key, value_name: &str, value: i32) -> Result<(), ConfigError> {
    hk.set_u32(value_name, value as u32)
        .map_err(|e| ConfigError {
            msg: "Failed to set config as i32",
            source: e.into(),
        })
}

fn reg_set_bool(hk: &Key, value_name: &str, value: bool) -> Result<(), ConfigError> {
    hk.set_u32(value_name, value as u32)
        .map_err(|e| ConfigError {
            msg: "Failed to set config as bool",
            source: e.into(),
        })
}

impl_context_error!(pub ConfigError);

#[cfg(test)]
mod test {
    use crate::config::KeybindValue;

    #[test]
    fn parse_keybind_action() {
        let keybind = "ctrl+c=text";
        let value: KeybindValue = keybind.parse().unwrap();
        assert_eq!(value.key, "ctrl+c");
        assert_eq!(value.action, "text");
        assert_eq!(value.param, "");
    }
    #[test]
    fn parse_keybind_param() {
        let keybind = "ctrl+c=text:酷";
        let value: KeybindValue = keybind.parse().unwrap();
        assert_eq!(value.key, "ctrl+c");
        assert_eq!(value.action, "text");
        assert_eq!(value.param, "酷");
    }
    #[test]
    fn serialize_keybind() {
        let keybind = "ctrl+c=text:酷";
        let value: KeybindValue = keybind.parse().unwrap();
        assert_eq!(keybind, value.to_string());
    }
}
