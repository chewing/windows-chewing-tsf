// SPDX-License-Identifier: GPL-3.0-or-later

use std::rc::Rc;
use std::{env, fs, path::PathBuf};

use anyhow::{Result, bail};
use chewing::path::data_dir;
use chewing_tip_config::Config;
use slint::ModelRc;
use slint::VecModel;
use slint::{ComponentHandle, SharedString};

use crate::AboutWindow;
use crate::ConfigWindow;

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
    let cfg = Config::from_reg()?;
    let chewing_tsf = &cfg.chewing_tsf;

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

    ui.set_keyboard_layout(chewing_tsf.keyboard_layout);
    ui.set_cand_per_row(chewing_tsf.cand_per_row);
    ui.set_default_english(chewing_tsf.default_english);
    ui.set_default_full_space(chewing_tsf.default_full_space);
    ui.set_show_cand_with_space_key(chewing_tsf.show_cand_with_space_key);
    ui.set_switch_lang_with_shift(chewing_tsf.switch_lang_with_shift);
    ui.set_enable_fullwidth_toggle_key(chewing_tsf.enable_fullwidth_toggle_key);
    ui.set_show_notification(chewing_tsf.show_notification);
    ui.set_output_simp_chinese(chewing_tsf.output_simp_chinese);
    ui.set_add_phrase_forward(chewing_tsf.add_phrase_forward);
    ui.set_phrase_choice_rearward(chewing_tsf.phrase_choice_rearward);
    ui.set_advance_after_selection(chewing_tsf.advance_after_selection);
    ui.set_font_size(chewing_tsf.font_size);
    ui.set_font_family(SharedString::from(&chewing_tsf.font_family));
    ui.set_font_fg_color(SharedString::from(&chewing_tsf.font_fg_color));
    ui.set_font_bg_color(SharedString::from(&chewing_tsf.font_bg_color));
    ui.set_font_highlight_fg_color(SharedString::from(&chewing_tsf.font_highlight_fg_color));
    ui.set_font_highlight_bg_color(SharedString::from(&chewing_tsf.font_highlight_bg_color));
    ui.set_font_number_fg_color(SharedString::from(&chewing_tsf.font_number_fg_color));
    ui.set_sel_key_type(chewing_tsf.sel_key_type);
    ui.set_conv_engine(chewing_tsf.conv_engine);
    ui.set_cand_per_page(chewing_tsf.cand_per_page);
    ui.set_cursor_cand_list(chewing_tsf.cursor_cand_list);
    ui.set_enable_caps_lock(chewing_tsf.enable_caps_lock);
    ui.set_full_shape_symbols(chewing_tsf.full_shape_symbols);
    ui.set_esc_clean_all_buf(chewing_tsf.esc_clean_all_buf);
    ui.set_easy_symbols_with_shift(chewing_tsf.easy_symbols_with_shift);
    ui.set_easy_symbols_with_shift_ctrl(chewing_tsf.easy_symbols_with_shift_ctrl);
    ui.set_upper_case_with_shift(chewing_tsf.upper_case_with_shift);
    ui.set_enable_auto_learn(chewing_tsf.enable_auto_learn);

    Ok(())
}

fn extract_config(ui: &ConfigWindow) -> Config {
    let mut cfg = Config::default();
    let chewing_tsf = &mut cfg.chewing_tsf;

    chewing_tsf.keyboard_layout = ui.get_keyboard_layout();
    chewing_tsf.cand_per_row = ui.get_cand_per_row();
    chewing_tsf.default_english = ui.get_default_english();
    chewing_tsf.default_full_space = ui.get_default_full_space();
    chewing_tsf.show_cand_with_space_key = ui.get_show_cand_with_space_key();
    chewing_tsf.switch_lang_with_shift = ui.get_switch_lang_with_shift();
    chewing_tsf.enable_fullwidth_toggle_key = ui.get_enable_fullwidth_toggle_key();
    chewing_tsf.show_notification = ui.get_show_notification();
    chewing_tsf.output_simp_chinese = ui.get_output_simp_chinese();
    chewing_tsf.add_phrase_forward = ui.get_add_phrase_forward();
    chewing_tsf.phrase_choice_rearward = ui.get_phrase_choice_rearward();
    chewing_tsf.advance_after_selection = ui.get_advance_after_selection();
    chewing_tsf.font_size = ui.get_font_size();
    chewing_tsf.font_family = ui.get_font_family().to_string();
    chewing_tsf.font_fg_color = ui.get_font_fg_color().to_string();
    chewing_tsf.font_bg_color = ui.get_font_bg_color().to_string();
    chewing_tsf.font_highlight_fg_color = ui.get_font_highlight_fg_color().to_string();
    chewing_tsf.font_highlight_bg_color = ui.get_font_highlight_bg_color().to_string();
    chewing_tsf.font_number_fg_color = ui.get_font_number_fg_color().to_string();
    chewing_tsf.sel_key_type = ui.get_sel_key_type();
    chewing_tsf.conv_engine = ui.get_conv_engine();
    chewing_tsf.cand_per_page = ui.get_cand_per_page();
    chewing_tsf.cursor_cand_list = ui.get_cursor_cand_list();
    chewing_tsf.enable_caps_lock = ui.get_enable_caps_lock();
    chewing_tsf.full_shape_symbols = ui.get_full_shape_symbols();
    chewing_tsf.esc_clean_all_buf = ui.get_esc_clean_all_buf();
    chewing_tsf.easy_symbols_with_shift = ui.get_easy_symbols_with_shift();
    chewing_tsf.easy_symbols_with_shift_ctrl = ui.get_easy_symbols_with_shift_ctrl();
    chewing_tsf.upper_case_with_shift = ui.get_upper_case_with_shift();
    chewing_tsf.enable_auto_learn = ui.get_enable_auto_learn();

    cfg
}

fn save_config(ui: &ConfigWindow) -> Result<()> {
    let cfg = extract_config(&ui);
    cfg.save_reg();

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

    Ok(())
}
