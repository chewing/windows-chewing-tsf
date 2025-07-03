// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use windows::Win32::System::Registry::KEY_WOW64_64KEY;
use windows_registry::{CURRENT_USER, Key};

// TODO use this config module in preferences

#[derive(Debug, Default, PartialEq)]
pub(super) struct Config {
    pub(super) switch_lang_with_shift: bool,
    pub(super) enable_caps_lock: bool,
    pub(super) enable_auto_learn: bool,
    pub(super) esc_clean_all_buf: bool,
    pub(super) full_shape_symbols: bool,
    pub(super) upper_case_with_shift: bool,
    pub(super) add_phrase_forward: bool,
    pub(super) easy_symbols_with_shift: bool,
    pub(super) easy_symbols_with_ctrl: bool,
    pub(super) cursor_cand_list: bool,
    pub(super) show_cand_with_space_key: bool,
    pub(super) advance_after_selection: bool,
    pub(super) default_full_space: bool,
    pub(super) default_english: bool,
    pub(super) output_simp_chinese: bool,
    pub(super) sel_key_type: i32,
    pub(super) conv_engine: i32,
    pub(super) cand_per_row: i32,
    pub(super) cand_per_page: i32,
    pub(super) font_size: i32,
    pub(super) keyboard_layout: i32,
}

impl Config {
    pub(super) fn reload_if_needed(&mut self) -> Result<bool> {
        let cfg = load_config()?;
        if cfg == *self {
            Ok(false)
        } else {
            *self = cfg;
            Ok(true)
        }
    }
}

fn reg_get_i32(hk: &Key, value_name: &str) -> Result<i32> {
    Ok(hk.get_u32(value_name)? as i32)
}

fn reg_get_bool(hk: &Key, value_name: &str) -> Result<bool> {
    Ok(hk.get_u32(value_name)? > 0)
}

fn load_config() -> Result<Config> {
    let key = CURRENT_USER
        .options()
        .create()
        .read()
        .access(KEY_WOW64_64KEY.0)
        .open("Software\\ChewingTextService")?;
    let mut cfg = Config {
        cand_per_row: 3,
        switch_lang_with_shift: true,
        add_phrase_forward: true,
        advance_after_selection: true,
        font_size: 16,
        conv_engine: 1,
        cand_per_page: 9,
        cursor_cand_list: true,
        enable_caps_lock: true,
        enable_auto_learn: true,
        full_shape_symbols: true,
        easy_symbols_with_shift: true,
        ..Config::default()
    };

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
    if let Ok(value) = reg_get_bool(&key, "OutputSimpChinese") {
        cfg.output_simp_chinese = value;
    }
    if let Ok(value) = reg_get_bool(&key, "AddPhraseForward") {
        cfg.add_phrase_forward = value;
    }
    // if let Ok(value) = reg_get_bool(&key, "ColorCandWnd") {
    //     ui.color_cand_wnd = value;
    // }
    if let Ok(value) = reg_get_bool(&key, "AdvanceAfterSelection") {
        cfg.advance_after_selection = value;
    }
    if let Ok(value) = reg_get_i32(&key, "DefFontSize") {
        cfg.font_size = value;
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
    // if let Ok(value) = reg_get_bool(&key, "PhraseMark") {
    //     ui.phrase_mark = value;
    // }
    if let Ok(value) = reg_get_bool(&key, "EscCleanAllBuf") {
        cfg.esc_clean_all_buf = value;
    }
    if let Ok(value) = reg_get_bool(&key, "EasySymbolsWithShift") {
        cfg.easy_symbols_with_shift = value;
    }
    if let Ok(value) = reg_get_bool(&key, "EasySymbolsWithCtrl") {
        cfg.easy_symbols_with_ctrl = value;
    }
    if let Ok(value) = reg_get_bool(&key, "UpperCaseWithShift") {
        cfg.upper_case_with_shift = value;
    }

    Ok(cfg)
}
