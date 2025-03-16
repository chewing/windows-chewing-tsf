use std::{fs, path::PathBuf, time::SystemTime};

use anyhow::{Result, bail};
use chewing::path::data_dir;
use slint::ComponentHandle;
use windows::{
    Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WOW64_64KEY, KEY_WRITE, REG_DWORD,
        REG_OPEN_CREATE_OPTIONS, RRF_RT_REG_DWORD, RegCloseKey, RegCreateKeyExW, RegGetValueW,
        RegOpenKeyExW, RegSetValueExW,
    },
    core::{PCWSTR, w},
};

use crate::ConfigWindow;

pub fn run() -> Result<()> {
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

    ui.run()?;
    Ok(())
}

fn reg_open_key_read(hk: &mut HKEY) -> Result<()> {
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\ChewingTextService"),
            None,
            KEY_WOW64_64KEY | KEY_READ,
            hk,
        )
        .ok()?;
    }
    Ok(())
}

fn reg_open_key_write(hk: &mut HKEY) -> Result<()> {
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\ChewingTextService"),
            None,
            None,
            REG_OPEN_CREATE_OPTIONS::default(),
            KEY_WOW64_64KEY | KEY_READ | KEY_WRITE,
            None,
            hk,
            None,
        )
        .ok()?;
    }
    Ok(())
}

fn reg_close_key(hk: HKEY) {
    unsafe {
        let _ = RegCloseKey(hk);
    }
}

fn reg_get_u32(hk: HKEY, value_name: PCWSTR) -> Result<u32> {
    let mut dword = 0;
    let mut size = size_of::<u32>() as u32;
    let pdword: *mut u32 = &mut dword;
    unsafe {
        RegGetValueW(
            hk,
            None,
            value_name,
            RRF_RT_REG_DWORD,
            None,
            Some(pdword.cast()),
            Some(&mut size),
        )
        .ok()?;
    }
    Ok(dword)
}

fn reg_set_u32(hk: HKEY, value_name: PCWSTR, value: u32) -> Result<()> {
    unsafe {
        RegSetValueExW(hk, value_name, None, REG_DWORD, Some(&value.to_ne_bytes())).ok()?;
    }
    Ok(())
}

fn reg_get_i32(hk: HKEY, value_name: PCWSTR) -> Result<i32> {
    Ok(reg_get_u32(hk, value_name)? as i32)
}

fn reg_set_i32(hk: HKEY, value_name: PCWSTR, value: i32) -> Result<()> {
    Ok(reg_set_u32(hk, value_name, value as u32)?)
}

fn reg_get_bool(hk: HKEY, value_name: PCWSTR) -> Result<bool> {
    Ok(reg_get_u32(hk, value_name)? > 0)
}

fn reg_set_bool(hk: HKEY, value_name: PCWSTR, value: bool) -> Result<()> {
    Ok(reg_set_u32(hk, value_name, value as u32)?)
}

fn default_user_symbols_dat_path() -> PathBuf {
    let user_data_dir = PathBuf::from(env!("USERPROFILE")).join("ChewingTextService");
    let user_symbols_dat = data_dir().unwrap_or(user_data_dir).join("symbols.dat");
    user_symbols_dat
}

// FIXME: provide path info from libchewing
fn user_symbols_dat_path() -> Result<PathBuf> {
    let user_data_dir = PathBuf::from(env!("USERPROFILE")).join("ChewingTextService");
    let user_symbols_dat = data_dir().unwrap_or(user_data_dir).join("symbols.dat");
    if user_symbols_dat.exists() {
        return Ok(user_symbols_dat);
    }
    bail!("使用者符號檔不存在")
}

// FIXME: provide path info from libchewing
fn system_symbols_dat_path() -> Result<PathBuf> {
    let prog_files_x86 = PathBuf::from(env!("ProgramFiles(x86)"))
        .join("ChewingTextService\\Dictionary\\symbols.dat");
    let prog_files =
        PathBuf::from(env!("ProgramFiles")).join("ChewingTextService\\Dictionary\\symbols.dat");
    if prog_files_x86.exists() {
        return Ok(prog_files_x86);
    }
    if prog_files.exists() {
        return Ok(prog_files);
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

    // Load custom value from the registry
    let mut hk = Default::default();
    if reg_open_key_read(&mut hk).is_ok() {
        if let Ok(value) = reg_get_i32(hk, w!("KeyboardLayout")) {
            ui.set_keyboard_layout(value);
        }
        if let Ok(value) = reg_get_i32(hk, w!("CandPerRow")) {
            ui.set_cand_per_row(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("DefaultEnglish")) {
            ui.set_default_english(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("DefaultFullSpace")) {
            ui.set_default_full_space(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("ShowCandWithSpaceKey")) {
            ui.set_show_cand_with_space_key(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("SwitchLangWithShift")) {
            ui.set_switch_lang_with_shift(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("OutputSimpChinese")) {
            ui.set_output_simp_chinese(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("AddPhraseForward")) {
            ui.set_add_phrase_forward(value);
        }
        // if let Ok(value) = reg_get_bool(hk, w!("ColorCandWnd")) {
        //     ui.set_color_cand_wnd(value);
        // }
        if let Ok(value) = reg_get_bool(hk, w!("AdvanceAfterSelection")) {
            ui.set_advance_after_selection(value);
        }
        if let Ok(value) = reg_get_i32(hk, w!("DefFontSize")) {
            ui.set_font_size(value);
        }
        if let Ok(value) = reg_get_i32(hk, w!("SelKeyType")) {
            ui.set_sel_key_type(value);
        }
        if let Ok(value) = reg_get_i32(hk, w!("ConvEngine")) {
            ui.set_conv_engine(value);
        }
        if let Ok(value) = reg_get_i32(hk, w!("SelAreaLen")) {
            ui.set_cand_per_page(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("CursorCandList")) {
            ui.set_cursor_cand_list(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("EnableCapsLock")) {
            ui.set_enable_caps_lock(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("FullShapeSymbols")) {
            ui.set_full_shape_symbols(value);
        }
        // if let Ok(value) = reg_get_bool(hk, w!("PhraseMark")) {
        //     ui.set_phrase_mark(value);
        // }
        if let Ok(value) = reg_get_bool(hk, w!("EscCleanAllBuf")) {
            ui.set_esc_clean_all_buf(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("EasySymbolsWithShift")) {
            ui.set_easy_symbols_with_shift(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("EasySymbolsWithCtrl")) {
            ui.set_easy_symbols_with_ctrl(value);
        }
        if let Ok(value) = reg_get_bool(hk, w!("UpperCaseWithShift")) {
            ui.set_upper_case_with_shift(value);
        }

        reg_close_key(hk);
    }

    Ok(())
}

fn save_config(ui: &ConfigWindow) -> Result<()> {
    let mut hk: HKEY = Default::default();

    if reg_open_key_write(&mut hk).is_ok() {
        let _ = reg_set_i32(hk, w!("KeyboardLayout"), ui.get_keyboard_layout());
        let _ = reg_set_i32(hk, w!("CandPerRow"), ui.get_cand_per_row());
        let _ = reg_set_bool(hk, w!("DefaultEnglish"), ui.get_default_english());
        let _ = reg_set_bool(hk, w!("DefaultFullSpace"), ui.get_default_full_space());
        let _ = reg_set_bool(
            hk,
            w!("ShowCandWithSpaceKey"),
            ui.get_show_cand_with_space_key(),
        );
        let _ = reg_set_bool(
            hk,
            w!("SwitchLangWithShift"),
            ui.get_switch_lang_with_shift(),
        );
        let _ = reg_set_bool(hk, w!("OutputSimpChinese"), ui.get_output_simp_chinese());
        let _ = reg_set_bool(hk, w!("AddPhraseForward"), ui.get_add_phrase_forward());
        // let _ = reg_set_i32(hk, w!("ColorCandWnd"), ui.get_color_cand_wnd());
        let _ = reg_set_bool(
            hk,
            w!("AdvanceAfterSelection"),
            ui.get_advance_after_selection(),
        );
        let _ = reg_set_i32(hk, w!("DefFontSize"), ui.get_font_size());
        let _ = reg_set_i32(hk, w!("SelKeyType"), ui.get_sel_key_type());
        let _ = reg_set_i32(hk, w!("ConvEngine"), ui.get_conv_engine());
        let _ = reg_set_i32(hk, w!("SelAreaLen"), ui.get_cand_per_page());
        let _ = reg_set_bool(hk, w!("CursorCandList"), ui.get_cursor_cand_list());
        let _ = reg_set_bool(hk, w!("EnableCapsLock"), ui.get_enable_caps_lock());
        let _ = reg_set_bool(hk, w!("FullShapeSymbols"), ui.get_full_shape_symbols());
        // let _ = reg_set_bool(hk, w!("PhraseMark"), ui.get_phrase_mark());
        let _ = reg_set_bool(hk, w!("EscCleanAllBuf"), ui.get_esc_clean_all_buf());
        let _ = reg_set_bool(
            hk,
            w!("EasySymbolsWithShift"),
            ui.get_easy_symbols_with_shift(),
        );
        let _ = reg_set_bool(
            hk,
            w!("EasySymbolsWithCtrl"),
            ui.get_easy_symbols_with_ctrl(),
        );
        let _ = reg_set_bool(hk, w!("UpperCaseWithShift"), ui.get_upper_case_with_shift());
        let _ = reg_set_u32(
            hk,
            w!("ModifiedTimestamp"),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as u32,
        );

        reg_close_key(hk);
    }

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
