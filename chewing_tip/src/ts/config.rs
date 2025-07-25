// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use windows::Win32::{Graphics::Direct2D::Common::D2D1_COLOR_F, System::Registry::KEY_WOW64_64KEY};
use windows_core::{HSTRING, h};
use windows_registry::{CURRENT_USER, Key};

// TODO use this config module in preferences

#[derive(Debug, PartialEq)]
pub(super) struct Config {
    pub(super) switch_lang_with_shift: bool,
    pub(super) enable_fullwidth_toggle_key: bool,
    pub(super) enable_caps_lock: bool,
    pub(super) show_notification: bool,
    pub(super) enable_auto_learn: bool,
    pub(super) esc_clean_all_buf: bool,
    pub(super) full_shape_symbols: bool,
    pub(super) upper_case_with_shift: bool,
    pub(super) add_phrase_forward: bool,
    pub(super) phrase_choice_rearward: bool,
    pub(super) easy_symbols_with_shift: bool,
    pub(super) easy_symbols_with_shift_ctrl: bool,
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
    pub(super) font_family: HSTRING,
    pub(super) font_fg_color: D2D1_COLOR_F,
    pub(super) font_bg_color: D2D1_COLOR_F,
    pub(super) font_highlight_fg_color: D2D1_COLOR_F,
    pub(super) font_highlight_bg_color: D2D1_COLOR_F,
    pub(super) font_number_fg_color: D2D1_COLOR_F,
    pub(super) keyboard_layout: i32,
}

impl Default for Config {
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
            font_family: h!("Segoe UI").to_owned(),
            font_fg_color: color_f(0.0, 0.0, 0.0),
            font_bg_color: color_f(0.98, 0.98, 0.98),
            font_highlight_fg_color: color_f(1.0, 1.0, 1.0),
            font_highlight_bg_color: color_f(0.0, 0.0, 0.0),
            font_number_fg_color: color_f(0.0, 0.0, 1.0),
            keyboard_layout: 0,
        }
    }
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

fn color_f(r: f32, g: f32, b: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a: 1.0 }
}

fn color_s(rgb: &str) -> D2D1_COLOR_F {
    let rgb = u32::from_str_radix(rgb, 16).unwrap_or(0);
    let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let b = (rgb & 0xFF) as f32 / 255.0;
    color_f(r, g, b)
}

fn load_config() -> Result<Config> {
    let key = CURRENT_USER
        .options()
        .read()
        .access(KEY_WOW64_64KEY.0)
        .open("Software\\ChewingTextService")?;
    let mut cfg = Config::default();

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
    // if let Ok(value) = reg_get_bool(&key, "ColorCandWnd") {
    //     ui.color_cand_wnd = value;
    // }
    if let Ok(value) = reg_get_bool(&key, "AdvanceAfterSelection") {
        cfg.advance_after_selection = value;
    }
    if let Ok(value) = reg_get_i32(&key, "DefFontSize") {
        cfg.font_size = value;
    }
    if let Ok(value) = key.get_hstring("DefFontFamily") {
        cfg.font_family = value;
    }
    if let Ok(value) = key.get_string("DefFontFgColor") {
        cfg.font_fg_color = color_s(&value);
    }
    if let Ok(value) = key.get_string("DefFontBgColor") {
        cfg.font_bg_color = color_s(&value);
    }
    if let Ok(value) = key.get_string("DefFontHighlightFgColor") {
        cfg.font_highlight_fg_color = color_s(&value);
    }
    if let Ok(value) = key.get_string("DefFontHighlightBgColor") {
        cfg.font_highlight_bg_color = color_s(&value);
    }
    if let Ok(value) = key.get_string("DefFontNumberFgColor") {
        cfg.font_number_fg_color = color_s(&value);
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
    if let Ok(value) = reg_get_bool(&key, "EasySymbolsWithShiftCtrl") {
        cfg.easy_symbols_with_shift_ctrl = value;
    }
    if let Ok(value) = reg_get_bool(&key, "UpperCaseWithShift") {
        cfg.upper_case_with_shift = value;
    }
    if let Ok(value) = reg_get_bool(&key, "EnableFullwidthToggleKey") {
        cfg.enable_fullwidth_toggle_key = value;
    }

    Ok(cfg)
}
