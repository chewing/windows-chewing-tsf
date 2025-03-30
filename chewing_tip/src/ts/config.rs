use anyhow::Result;
use log::{error, info};
use windows::Win32::{
    Foundation::{CloseHandle, FALSE, HANDLE, WAIT_FAILED, WAIT_OBJECT_0},
    System::{
        Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, KEY_READ, KEY_WOW64_64KEY,
            REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_THREAD_AGNOSTIC, RRF_RT_REG_DWORD, RegCloseKey,
            RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
        },
        Threading::{
            CreateEventW, GetCurrentProcess, IsWow64Process, ResetEvent, WaitForSingleObject,
        },
    },
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
    change_event: HANDLE,
    monitor_hkey: HKEY,
}

impl Config {
    pub(super) fn load(&mut self) -> Result<()> {
        load_config(self)
    }
    pub(super) fn reload_if_needed(&mut self) -> Result<()> {
        if !self.change_event.is_invalid() {
            unsafe {
                match WaitForSingleObject(self.change_event, 0) {
                    WAIT_OBJECT_0 => {
                        info!("config change detected, reload config");
                        self.load()?;
                        self.watch_changes();
                        return Ok(());
                    }
                    WAIT_FAILED => {
                        let _ = CloseHandle(self.change_event);
                        self.change_event = HANDLE::default();
                        self.watch_changes();
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
    pub(super) fn watch_changes(&mut self) {
        if self.change_event.is_invalid() {
            match unsafe { CreateEventW(None, true, false, None) } {
                Ok(change_event) => self.change_event = change_event,
                Err(error) => {
                    error!("unable to create change event handle: {error}");
                    return;
                }
            }
        } else {
            unsafe {
                let _ = ResetEvent(self.change_event);
            }
        }

        if self.monitor_hkey.is_invalid() {
            unsafe {
                let process = GetCurrentProcess();
                let mut is_wow64 = FALSE;
                let _ = IsWow64Process(process, &mut is_wow64);
                let reg_flags = if is_wow64.as_bool() {
                    KEY_WOW64_64KEY | KEY_NOTIFY
                } else {
                    KEY_NOTIFY
                };
                if let Err(error) = RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    w!("Software\\ChewingTextService"),
                    None,
                    reg_flags,
                    &mut self.monitor_hkey,
                )
                .ok()
                {
                    error!("unable to open HKEY handle: {error}");
                    return;
                }
            }
        }
        let filter = REG_NOTIFY_CHANGE_LAST_SET | REG_NOTIFY_THREAD_AGNOSTIC;
        unsafe {
            if let Err(error) = RegNotifyChangeKeyValue(
                self.monitor_hkey,
                true,
                filter,
                Some(self.change_event),
                true,
            )
            .ok()
            {
                error!("unable to register notify for registry change: {error}");
            }
        }
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

fn reg_get_i32(hk: HKEY, value_name: PCWSTR) -> Result<i32> {
    Ok(reg_get_u32(hk, value_name)? as i32)
}

fn reg_get_bool(hk: HKEY, value_name: PCWSTR) -> Result<bool> {
    Ok(reg_get_u32(hk, value_name)? > 0)
}

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
