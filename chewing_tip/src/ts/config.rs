use std::{fs, path::PathBuf, time::SystemTime};

use anyhow::Result;
use chewing::path::data_dir;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WOW64_64KEY, KEY_WRITE, REG_DWORD,
    REG_OPEN_CREATE_OPTIONS, RRF_RT_REG_DWORD, RegCloseKey, RegCreateKeyExW, RegGetValueW,
    RegOpenKeyExW, RegSetValueExW,
};
use windows_core::{PCWSTR, w};

// TODO use this config module in preferences

#[derive(Debug, Default)]
pub(super) struct Config {
    pub(super) switch_lang_with_shift: bool,
    pub(super) enable_caps_lock: bool,
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
    pub(super) fn load(&mut self) -> Result<()> {
        load_config(self)
    }
    pub(super) fn save(&self) -> Result<()> {
        save_config(self)
    }
    pub(super) fn reload_if_needed(&mut self) -> Result<()> {
        // FIXME
        self.load()
    }
    pub(super) fn watch_changes(&mut self) {
        // FIXME
    }
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

// fn default_user_symbols_dat_path() -> PathBuf {
//     let user_data_dir = PathBuf::from(env!("USERPROFILE")).join("ChewingTextService");
//     let user_symbols_dat = data_dir().unwrap_or(user_data_dir).join("symbols.dat");
//     user_symbols_dat
// }

// // FIXME: provide path info from libchewing
// fn user_symbols_dat_path() -> Result<PathBuf> {
//     let user_data_dir = PathBuf::from(env!("USERPROFILE")).join("ChewingTextService");
//     let user_symbols_dat = data_dir().unwrap_or(user_data_dir).join("symbols.dat");
//     if user_symbols_dat.exists() {
//         return Ok(user_symbols_dat);
//     }
//     bail!("使用者符號檔不存在")
// }

// // FIXME: provide path info from libchewing
// fn system_symbols_dat_path() -> Result<PathBuf> {
//     let prog_files_x86 = PathBuf::from(env!("ProgramFiles(x86)"))
//         .join("ChewingTextService\\Dictionary\\symbols.dat");
//     let prog_files =
//         PathBuf::from(env!("ProgramFiles")).join("ChewingTextService\\Dictionary\\symbols.dat");
//     if prog_files_x86.exists() {
//         return Ok(prog_files_x86);
//     }
//     if prog_files.exists() {
//         return Ok(prog_files);
//     }
//     bail!("系統詞庫不存在")
// }

fn load_config(cfg: &mut Config) -> Result<()> {
    // Init settings to default value
    cfg.cand_per_row = 3;
    cfg.switch_lang_with_shift = true;
    cfg.add_phrase_forward = true;
    cfg.advance_after_selection = true;
    cfg.font_size = 16;
    cfg.conv_engine = 1;
    cfg.cand_per_page = 9;
    cfg.cursor_cand_list = true;
    cfg.enable_caps_lock = true;
    cfg.full_shape_symbols = true;
    cfg.easy_symbols_with_shift = true;

    // if let Ok(path) = user_symbols_dat_path() {
    //     cfg.set_symbols_dat(fs::read_to_string(path)?.into());
    // } else {
    //     if let Ok(path) = system_symbols_dat_path() {
    //         cfg.set_symbols_dat(fs::read_to_string(path)?.into());
    //     }
    // }

    // Load custom value from the registry
    let mut hk = Default::default();
    if reg_open_key_read(&mut hk).is_ok() {
        if let Ok(value) = reg_get_i32(hk, w!("KeyboardLayout")) {
            cfg.keyboard_layout = value;
        }
        if let Ok(value) = reg_get_i32(hk, w!("CandPerRow")) {
            cfg.cand_per_row = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("DefaultEnglish")) {
            cfg.default_english = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("DefaultFullSpace")) {
            cfg.default_full_space = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("ShowCandWithSpaceKey")) {
            cfg.show_cand_with_space_key = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("SwitchLangWithShift")) {
            cfg.switch_lang_with_shift = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("OutputSimpChinese")) {
            cfg.output_simp_chinese = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("AddPhraseForward")) {
            cfg.add_phrase_forward = value;
        }
        // if let Ok(value) = reg_get_bool(hk, w!("ColorCandWnd")) {
        //     ui.color_cand_wnd = value;
        // }
        if let Ok(value) = reg_get_bool(hk, w!("AdvanceAfterSelection")) {
            cfg.advance_after_selection = value;
        }
        if let Ok(value) = reg_get_i32(hk, w!("DefFontSize")) {
            cfg.font_size = value;
        }
        if let Ok(value) = reg_get_i32(hk, w!("SelKeyType")) {
            cfg.sel_key_type = value;
        }
        if let Ok(value) = reg_get_i32(hk, w!("ConvEngine")) {
            cfg.conv_engine = value;
        }
        if let Ok(value) = reg_get_i32(hk, w!("SelAreaLen")) {
            cfg.cand_per_page = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("CursorCandList")) {
            cfg.cursor_cand_list = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("EnableCapsLock")) {
            cfg.enable_caps_lock = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("FullShapeSymbols")) {
            cfg.full_shape_symbols = value;
        }
        // if let Ok(value) = reg_get_bool(hk, w!("PhraseMark")) {
        //     ui.phrase_mark = value;
        // }
        if let Ok(value) = reg_get_bool(hk, w!("EscCleanAllBuf")) {
            cfg.esc_clean_all_buf = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("EasySymbolsWithShift")) {
            cfg.easy_symbols_with_shift = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("EasySymbolsWithCtrl")) {
            cfg.easy_symbols_with_ctrl = value;
        }
        if let Ok(value) = reg_get_bool(hk, w!("UpperCaseWithShift")) {
            cfg.upper_case_with_shift = value;
        }

        reg_close_key(hk);
    }

    Ok(())
}

fn save_config(cfg: &Config) -> Result<()> {
    let mut hk: HKEY = Default::default();

    if reg_open_key_write(&mut hk).is_ok() {
        let _ = reg_set_i32(hk, w!("KeyboardLayout"), cfg.keyboard_layout);
        let _ = reg_set_i32(hk, w!("CandPerRow"), cfg.cand_per_row);
        let _ = reg_set_bool(hk, w!("DefaultEnglish"), cfg.default_english);
        let _ = reg_set_bool(hk, w!("DefaultFullSpace"), cfg.default_full_space);
        let _ = reg_set_bool(hk, w!("ShowCandWithSpaceKey"), cfg.show_cand_with_space_key);
        let _ = reg_set_bool(hk, w!("SwitchLangWithShift"), cfg.switch_lang_with_shift);
        let _ = reg_set_bool(hk, w!("OutputSimpChinese"), cfg.output_simp_chinese);
        let _ = reg_set_bool(hk, w!("AddPhraseForward"), cfg.add_phrase_forward);
        // let _ = reg_set_i32(hk, w!("ColorCandWnd"), ui.color_cand_wnd);
        let _ = reg_set_bool(hk, w!("AdvanceAfterSelection"), cfg.advance_after_selection);
        let _ = reg_set_i32(hk, w!("DefFontSize"), cfg.font_size);
        let _ = reg_set_i32(hk, w!("SelKeyType"), cfg.sel_key_type);
        let _ = reg_set_i32(hk, w!("ConvEngine"), cfg.conv_engine);
        let _ = reg_set_i32(hk, w!("SelAreaLen"), cfg.cand_per_page);
        let _ = reg_set_bool(hk, w!("CursorCandList"), cfg.cursor_cand_list);
        let _ = reg_set_bool(hk, w!("EnableCapsLock"), cfg.enable_caps_lock);
        let _ = reg_set_bool(hk, w!("FullShapeSymbols"), cfg.full_shape_symbols);
        // let _ = reg_set_bool(hk, w!("PhraseMark"), ui.phrase_mark);
        let _ = reg_set_bool(hk, w!("EscCleanAllBuf"), cfg.esc_clean_all_buf);
        let _ = reg_set_bool(hk, w!("EasySymbolsWithShift"), cfg.easy_symbols_with_shift);
        let _ = reg_set_bool(hk, w!("EasySymbolsWithCtrl"), cfg.easy_symbols_with_ctrl);
        let _ = reg_set_bool(hk, w!("UpperCaseWithShift"), cfg.upper_case_with_shift);
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

    // let sys_symbols_dat = system_symbols_dat_path()
    //     .and_then(|path| Ok(fs::read_to_string(path)?))
    //     .unwrap_or_default();
    // if cfg.get_symbols_dat() != sys_symbols_dat {
    //     let user_symbols_dat_path =
    //         user_symbols_dat_path().unwrap_or_else(|_| default_user_symbols_dat_path());
    //     fs::create_dir_all(user_symbols_dat_path.parent().unwrap())?;
    //     fs::write(user_symbols_dat_path, cfg.get_symbols_dat())?;
    // }

    Ok(())
}
