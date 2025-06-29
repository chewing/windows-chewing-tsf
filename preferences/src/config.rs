// SPDX-License-Identifier: GPL-3.0-or-later

use std::{env, fs, path::PathBuf};

use anyhow::{Result, bail};
use chewing::path::data_dir;
use slint::ComponentHandle;
use windows::Win32::System::Registry::KEY_WOW64_64KEY;
use windows_registry::{CURRENT_USER, Key};

use crate::ConfigWindow;
use crate::AboutWindow;

pub fn run() -> Result<()> {
    let about = AboutWindow::new()?;
    let ui = ConfigWindow::new()?;
    load_config(&ui)?;

    ui.on_cancel(move || {
        slint::quit_event_loop().unwrap();
    });
    let ui_handle = ui.as_weak();
    ui.on_apply(move || {
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

fn default_user_symbols_dat_path() -> PathBuf {
    let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\unknown".into());
    let user_data_dir = PathBuf::from(user_profile).join("ChewingTextService");
    let user_symbols_dat = data_dir().unwrap_or(user_data_dir).join("symbols.dat");
    user_symbols_dat
}

// FIXME: provide path info from libchewing
fn user_symbols_dat_path() -> Result<PathBuf> {
    let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\unknown".into());
    let user_data_dir = PathBuf::from(user_profile).join("ChewingTextService");
    let user_symbols_dat = data_dir().unwrap_or(user_data_dir).join("symbols.dat");
    if user_symbols_dat.exists() {
        return Ok(user_symbols_dat);
    }
    bail!("使用者符號檔不存在")
}

// FIXME: provide path info from libchewing
fn system_symbols_dat_path() -> Result<PathBuf> {
    let progfiles_x86 =
        env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files(x86)".into());
    let progfiles = env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
    let symbols_x86 =
        PathBuf::from(progfiles_x86).join("ChewingTextService\\Dictionary\\symbols.dat");
    let symbols = PathBuf::from(progfiles).join("ChewingTextService\\Dictionary\\symbols.dat");
    if symbols_x86.exists() {
        return Ok(symbols_x86);
    }
    if symbols.exists() {
        return Ok(symbols);
    }
    bail!("系統詞庫不存在")
}

fn load_config(ui: &ConfigWindow) -> Result<()> {
    // Init settings to default value
    ui.set_cand_per_row(3);
    ui.set_switch_lang_with_shift(true);
    ui.set_add_phrase_forward(true);
    ui.set_advance_after_selection(true);
    ui.set_font_size(16);
    ui.set_conv_engine(1);
    ui.set_cand_per_page(9);
    ui.set_cursor_cand_list(true);
    ui.set_enable_caps_lock(true);
    ui.set_full_shape_symbols(true);
    ui.set_easy_symbols_with_shift(true);

    if let Ok(path) = user_symbols_dat_path() {
        ui.set_symbols_dat(fs::read_to_string(path)?.into());
    } else {
        if let Ok(path) = system_symbols_dat_path() {
            ui.set_symbols_dat(fs::read_to_string(path)?.into());
        }
    }

    let key = CURRENT_USER
        .options()
        .create()
        .read()
        .access(KEY_WOW64_64KEY.0)
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
    if let Ok(value) = reg_get_bool(&key, "OutputSimpChinese") {
        ui.set_output_simp_chinese(value);
    }
    if let Ok(value) = reg_get_bool(&key, "AddPhraseForward") {
        ui.set_add_phrase_forward(value);
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
    if let Ok(value) = reg_get_bool(&key, "EasySymbolsWithCtrl") {
        ui.set_easy_symbols_with_ctrl(value);
    }
    if let Ok(value) = reg_get_bool(&key, "UpperCaseWithShift") {
        ui.set_upper_case_with_shift(value);
    }

    Ok(())
}

fn save_config(ui: &ConfigWindow) -> Result<()> {
    let key = CURRENT_USER
        .options()
        .create()
        .access(KEY_WOW64_64KEY.0)
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
    let _ = reg_set_bool(&key, "OutputSimpChinese", ui.get_output_simp_chinese());
    let _ = reg_set_bool(&key, "AddPhraseForward", ui.get_add_phrase_forward());
    // let _ = reg_set_i32(&key, "ColorCandWnd", ui.get_color_cand_wnd());
    let _ = reg_set_bool(
        &key,
        "AdvanceAfterSelection",
        ui.get_advance_after_selection(),
    );
    let _ = reg_set_i32(&key, "DefFontSize", ui.get_font_size());
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
    let _ = reg_set_bool(&key, "EasySymbolsWithCtrl", ui.get_easy_symbols_with_ctrl());
    let _ = reg_set_bool(&key, "UpperCaseWithShift", ui.get_upper_case_with_shift());

    let sys_symbols_dat = system_symbols_dat_path()
        .and_then(|path| Ok(fs::read_to_string(path)?))
        .unwrap_or_default();
    if ui.get_symbols_dat() != sys_symbols_dat {
        let user_symbols_dat_path =
            user_symbols_dat_path().unwrap_or_else(|_| default_user_symbols_dat_path());
        fs::create_dir_all(user_symbols_dat_path.parent().unwrap())?;
        fs::write(user_symbols_dat_path, ui.get_symbols_dat())?;
    }

    Ok(())
}
