use std::ffi::{CStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::simd::i32x8;
use std::sync::atomic::Ordering;
use std::time::UNIX_EPOCH;
use std::{collections::BTreeMap, path::PathBuf};

use chewing_capi::candidates::{chewing_set_candPerPage, chewing_set_selKey};
use chewing_capi::globals::{
    chewing_config_set_int, chewing_config_set_str, chewing_set_addPhraseDirection,
    chewing_set_autoShiftCur, chewing_set_escCleanAllBuf, chewing_set_maxChiSymbolLen,
    chewing_set_spaceAsSelection,
};
use chewing_capi::input::{
    chewing_handle_Backspace, chewing_handle_CtrlNum, chewing_handle_Default, chewing_handle_Del, chewing_handle_Down, chewing_handle_End, chewing_handle_Enter, chewing_handle_Esc, chewing_handle_Home, chewing_handle_Left, chewing_handle_Numlock, chewing_handle_PageDown, chewing_handle_PageUp, chewing_handle_Right, chewing_handle_Space, chewing_handle_Tab, chewing_handle_Up
};
use chewing_capi::layout::chewing_set_KBType;
use chewing_capi::modes::{
    CHINESE_MODE, FULLSHAPE_MODE, HALFSHAPE_MODE, SYMBOL_MODE, chewing_get_ChiEngMode,
    chewing_get_ShapeMode, chewing_set_ChiEngMode, chewing_set_ShapeMode,
};
use chewing_capi::output::chewing_keystroke_CheckIgnore;
use chewing_capi::setup::{ChewingContext, chewing_new};
use log::{error, info};
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_FLAGS_AND_ATTRIBUTES, SetFileAttributesW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_MENU, VK_NEXT, VK_NUMLOCK, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_TAB, VK_UP
};
use windows::Win32::UI::TextServices::{
    GUID_LBI_INPUTMODE, TF_LBI_STYLE_BTN_BUTTON, TF_LBI_STYLE_BTN_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSubMenu, HMENU, LoadIconW, LoadMenuW, LoadStringW,
};
use windows::Win32::UI::{
    Input::KeyboardAndMouse::VK_SPACE,
    TextServices::{
        ITfComposition, ITfKeystrokeMgr, ITfLangBarItemButton, ITfLangBarItemMgr, ITfThreadMgr,
        TF_LANGBARITEMINFO, TF_MOD_SHIFT, TF_PRESERVEDKEY,
    },
};
use windows_core::{BSTR, ComObject, ComObjectInner, GUID, Interface, PCWSTR, PWSTR, Result};
use windows_version::OsVersion;

use crate::G_HINSTANCE;

use super::config::Config;
use super::key_event::KeyEvent;
use super::lang_bar::LangBarButton;
use super::resources::{
    ID_MODE_ICON, ID_SWITCH_LANG, ID_SWITCH_SHAPE, IDI_CHI, IDI_CHI_DARK, IDI_CONFIG, IDI_ENG,
    IDI_ENG_DARK, IDI_FULL_SHAPE, IDI_HALF_SHAPE, IDR_MENU, IDS_SETTINGS, IDS_SWITCH_LANG,
    IDS_SWITCH_SHAPE,
};

const GUID_MODE_BUTTON: GUID = GUID::from_u128(0xB59D51B9_B832_40D2_9A8D_56959372DDC7);
const GUID_SHAPE_TYPE_BUTTON: GUID = GUID::from_u128(0x5325DBF5_5FBE_467B_ADF0_2395BE9DD2BB);
const GUID_SETTINGS_BUTTON: GUID = GUID::from_u128(0x4FAFA520_2104_407E_A532_9F1AAB7751CD);
const GUID_SHIFT_SPACE: GUID = GUID::from_u128(0xC77A44F5_DB21_474E_A2A2_A17242217AB3);

const CLSID_TEXT_SERVICE: GUID = GUID::from_u128(0x13F2EF08_575C_4D8C_88E0_F67BB8052B84);

const SEL_KEYS: [&CStr; 6] = [
    c"1234567890",
    c"asdfghjkl;",
    c"asdfzxcv89",
    c"asdfjkl789",
    c"aoeuhtn789",
    c"1234qweras",
];

#[derive(Default)]
pub(super) struct ChewingTextService {
    is_showing_candidates: bool,
    lang_mode: i32,
    shape_mode: i32,
    output_simp_chinese: bool,
    last_keydown_code: i32,
    message_timer_id: i32,
    symbols_file_mtime: u64,
    cfg: Config,
    chewing_context: Option<*mut ChewingContext>,

    preserved_keys: BTreeMap<u128, TF_PRESERVEDKEY>,
    lang_bar_buttons: Vec<ITfLangBarItemButton>,
    switch_lang_button: Option<ComObject<LangBarButton>>,
    switch_shape_button: Option<ComObject<LangBarButton>>,
    ime_mode_button: Option<ComObject<LangBarButton>>,
    thread_mgr: Option<ITfThreadMgr>,
    composition: Option<ITfComposition>,
    tid: u32,
}

impl ChewingTextService {
    pub(super) fn new() -> ChewingTextService {
        Default::default()
    }

    pub(super) fn activate(&mut self, thread_mgr: &ITfThreadMgr, tid: u32) -> Result<()> {
        self.thread_mgr = Some(thread_mgr.clone());
        self.tid = tid;
        self.add_preserved_key(VK_SPACE.0 as u32, TF_MOD_SHIFT, GUID_SHIFT_SPACE)?;

        info!("Load config and start watching changes");
        self.cfg.load().unwrap();
        // TODO watch change

        let g_hinstance = HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut c_void);

        info!("Add language bar buttons to switch Chinese/English modes");
        unsafe {
            let mut info = TF_LANGBARITEMINFO {
                clsidService: CLSID_TEXT_SERVICE,
                guidItem: GUID_MODE_BUTTON,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON,
                ..Default::default()
            };
            let tooltip = PWSTR::null();
            LoadStringW(Some(g_hinstance), IDS_SWITCH_LANG, tooltip, 0);
            info.szDescription.copy_from_slice(tooltip.as_wide());
            let button = LangBarButton::new(
                info,
                BSTR::from_wide(tooltip.as_wide()),
                LoadIconW(Some(g_hinstance), PCWSTR::from_raw(IDI_CHI as *const u16))?,
                HMENU::default(),
                ID_SWITCH_LANG,
                Box::new(|id, cmd| {}),
            )
            .into_object();
            self.switch_lang_button = Some(button.clone());
            self.add_button(button.to_interface())?;
        }

        info!("Add language bar buttons to toggle full shape/half shape modes");
        unsafe {
            let mut info = TF_LANGBARITEMINFO {
                clsidService: CLSID_TEXT_SERVICE,
                guidItem: GUID_SHAPE_TYPE_BUTTON,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON,
                ..Default::default()
            };
            let tooltip = PWSTR::null();
            LoadStringW(Some(g_hinstance), IDS_SWITCH_SHAPE, tooltip, 0);
            info.szDescription.copy_from_slice(tooltip.as_wide());
            let button = LangBarButton::new(
                info,
                BSTR::from_wide(tooltip.as_wide()),
                LoadIconW(
                    Some(g_hinstance),
                    PCWSTR::from_raw(IDI_HALF_SHAPE as *const u16),
                )?,
                HMENU::default(),
                ID_SWITCH_SHAPE,
                Box::new(|id, cmd| {}),
            )
            .into_object();
            self.switch_shape_button = Some(button.clone());
            self.add_button(button.to_interface())?;
        }

        info!("Add button for settings and others, may open a popup menu");
        unsafe {
            let mut info = TF_LANGBARITEMINFO {
                clsidService: CLSID_TEXT_SERVICE,
                guidItem: GUID_SETTINGS_BUTTON,
                dwStyle: TF_LBI_STYLE_BTN_MENU,
                ..Default::default()
            };
            let tooltip = PWSTR::null();
            LoadStringW(Some(g_hinstance), IDS_SETTINGS, tooltip, 0);
            info.szDescription.copy_from_slice(tooltip.as_wide());
            let menu = LoadMenuW(Some(g_hinstance), PCWSTR::from_raw(IDR_MENU as *const u16))?;
            let popup_menu = GetSubMenu(menu, 0);
            let button = LangBarButton::new(
                info,
                BSTR::from_wide(tooltip.as_wide()),
                LoadIconW(
                    Some(g_hinstance),
                    PCWSTR::from_raw(IDI_CONFIG as *const u16),
                )?,
                popup_menu,
                0,
                Box::new(|id, cmd| {}),
            )
            .into_object();
            self.add_button(button.to_interface())?;
        }

        // Windows 8 systray IME mode icon
        if OsVersion::current() >= OsVersion::new(6, 2, 9200, 0) {
            info!("Add systray IME mode icon to switch Chinese/English modes");
            unsafe {
                let mut info = TF_LANGBARITEMINFO {
                    clsidService: CLSID_TEXT_SERVICE,
                    guidItem: GUID_LBI_INPUTMODE,
                    dwStyle: TF_LBI_STYLE_BTN_BUTTON,
                    ..Default::default()
                };
                let tooltip = PWSTR::null();
                LoadStringW(Some(g_hinstance), IDS_SWITCH_LANG, tooltip, 0);
                info.szDescription.copy_from_slice(tooltip.as_wide());
                let icon_id = if is_light_theme() {
                    IDI_ENG
                } else {
                    IDI_ENG_DARK
                };
                let button = LangBarButton::new(
                    info,
                    BSTR::from_wide(tooltip.as_wide()),
                    LoadIconW(Some(g_hinstance), PCWSTR::from_raw(icon_id as *const u16))?,
                    HMENU::default(),
                    ID_MODE_ICON,
                    Box::new(|id, cmd| {}),
                )
                .into_object();
                button.set_enabled(true)?;
                self.ime_mode_button = Some(button.clone());
                self.add_button(button.to_interface())?;
            }
        }

        // FIXME error handling
        self.init_chewing_context().unwrap();

        Ok(())
    }

    pub(super) fn deactivate(&mut self) -> Result<ITfThreadMgr> {
        self.tid = 0;
        self.free_chewing_context();
        self.switch_lang_button = None;
        self.switch_shape_button = None;
        self.ime_mode_button = None;
        // TODO hide message window
        // TODO hide candidate window

        // TSF Remarks: The corresponding ITfTextInputProcessor::Deactivate
        // method that shuts down the text service must release all references
        // to the ptim parameter.
        Ok(self
            .thread_mgr
            .take()
            .expect("chewing_tip must have an active thread_mgr"))
    }

    pub(super) fn on_kill_focus(&mut self) -> Result<()> {
        if self.is_composing() {
            // TODO end composition
        }
        self.hide_candidates()?;
        self.hide_message()?;
        Ok(())
    }

    pub(super) fn on_keydown(&mut self, ev: KeyEvent, dry_run: bool) -> bool {
        // TODO detect changes
        if let Err(error) = self.apply_config() {
            error!("unable to apply config {error}");
        }
        if self.chewing_context.is_none() {
            error!("on_keydown but chewing context is null");
            return false;
        }
        let last_keydown_code = ev.vk;
        if !self.is_composing() {
            // don't do further handling in English + half shape mode
            if self.lang_mode == SYMBOL_MODE && self.shape_mode == HALFSHAPE_MODE {
                return false;
            }

            if ev.is_key_down(VK_CONTROL) || ev.is_key_down(VK_MENU) {
                // bypass IME. This might be a shortcut key used in the application
                // FIXME: we only need Ctrl in composition mode for adding user phrases.
                // However, if we turn on easy symbol input with Ctrl support later,
                // we'll need th Ctrl key then.
                return false;
            }

            // we always need further processing in full shape mode since all English chars,
            // numbers, and symbols need to be converted to full shape Chinese chars.
            if self.shape_mode != FULLSHAPE_MODE {
                // Caps lock is on => English mode
                if self.cfg.enable_caps_lock && ev.is_key_toggled(VK_CAPITAL) {
                    // We only need to handle printable keys because we need to
                    // convert them to upper case.
                    if !ev.is_a2z() {
                        return false;
                    }
                }
                // NumLock is on
                if ev.is_key_toggled(VK_NUMLOCK) && ev.is_num_pad() {
                    return false;
                }
            }
            if !ev.is_printable() {
                return false;
            }
        }

        if dry_run {
            return true;
        }

        let ctx = self.chewing_context.unwrap();
        if ev.is_printable() {
            let old_lang_mode = unsafe { chewing_get_ChiEngMode(ctx) };
            let mut momentary_english_mode = false;
            let mut invert_case = false;
            // If caps lock is on, temprarily change to English mode
            if self.cfg.enable_caps_lock && ev.is_key_toggled(VK_CAPITAL) {
                momentary_english_mode = true;
                invert_case = true;
            }
            // If shift is pressed, but we don't want to enter full shape symbols
            if ev.is_key_down(VK_SHIFT) && (!self.cfg.full_shape_symbols || ev.is_a2z()) {
                momentary_english_mode = true;
                if !self.cfg.upper_case_with_shift {
                    invert_case = true;
                }
            }
            if self.lang_mode == SYMBOL_MODE {
                unsafe {
                    chewing_handle_Default(ctx, ev.code as i32);
                }
            } else if momentary_english_mode {
                unsafe {
                    chewing_set_ChiEngMode(ctx, SYMBOL_MODE);
                }
                let code = if invert_case {
                    if ev.code.is_ascii_uppercase() {
                        ev.code.to_ascii_lowercase()
                    } else {
                        ev.code.to_ascii_uppercase()
                    }
                } else {
                    ev.code
                };
                unsafe {
                    chewing_handle_Default(ctx, code as i32);
                    chewing_set_ChiEngMode(ctx, old_lang_mode);
                }
            } else {
                if ev.is_a2z() {
                    unsafe {
                        chewing_handle_Default(ctx, ev.code.to_ascii_lowercase() as i32);
                    }
                } else if ev.vk == VK_SPACE.0 {
                    unsafe {
                        chewing_handle_Space(ctx);
                    }
                } else if ev.is_key_down(VK_CONTROL) && ev.code.is_ascii_digit() {
                    unsafe {
                        chewing_handle_CtrlNum(ctx, ev.code as i32);
                    }
                } else if ev.is_key_toggled(VK_NUMLOCK) && ev.is_num_pad() {
                    unsafe {
                        chewing_handle_Numlock(ctx, ev.code as i32);
                    }
                } else {
                    unsafe {
                        chewing_handle_Default(ctx, ev.code as i32);
                    }
                }
            }
        } else {
            let mut key_handled = false;
            if self.cfg.cursor_cand_list && self.is_showing_candidates {
                // TODO
            }

            if !key_handled {
                match VIRTUAL_KEY(ev.vk) {
                    VK_ESCAPE => unsafe {
                        chewing_handle_Esc(ctx);
                    },
                    VK_RETURN => unsafe {
                        chewing_handle_Enter(ctx);
                    },
                    VK_TAB => unsafe {
                        chewing_handle_Tab(ctx);
                    },
                    VK_DELETE => unsafe {
                        chewing_handle_Del(ctx);
                    },
                    VK_BACK => unsafe {
                        chewing_handle_Backspace(ctx);
                    },
                    VK_UP => unsafe {
                        chewing_handle_Up(ctx);
                    },
                    VK_DOWN => unsafe {
                        chewing_handle_Down(ctx);
                    },
                    VK_LEFT => unsafe {
                        chewing_handle_Left(ctx);
                    },
                    VK_RIGHT => unsafe {
                        chewing_handle_Right(ctx);
                    },
                    VK_HOME => unsafe {
                        chewing_handle_Home(ctx);
                    },
                    VK_END => unsafe {
                        chewing_handle_End(ctx);
                    },
                    VK_PRIOR => unsafe {
                        chewing_handle_PageUp(ctx);
                    },
                    VK_NEXT => unsafe {
                        chewing_handle_PageDown(ctx);
                    }
                    _ => return false,
                }
            }
        }

        if let Err(error) = self.update_lang_buttons() {
            error!("unable to update lang bar button: {error}")
        }

        if unsafe { chewing_keystroke_CheckIgnore(ctx) } == 1 {
            return false;
        }

        true
    }

    pub(super) fn on_keyup(&mut self, ev: KeyEvent, try_run: bool) -> bool {
        true
    }

    pub(super) fn on_preserved_key(&mut self, guid: &GUID) -> bool {
        if guid == &GUID_SHIFT_SPACE {
            if self.toggle_shape_mode().is_ok() {
                return true;
            }
        }
        false
    }

    fn toggle_shape_mode(&mut self) -> Result<()> {
        if let Some(ctx) = self.chewing_context {
            unsafe {
                chewing_set_ShapeMode(ctx, !chewing_get_ShapeMode(ctx));
            }
            self.update_lang_buttons()?;
        }
        Ok(())
    }

    fn is_composing(&self) -> bool {
        todo!()
    }

    fn init_chewing_context(&mut self) -> anyhow::Result<()> {
        // FIXME assert ctx should be none
        if self.chewing_context.is_none() {
            init_chewing_env();
            let ctx = chewing_new();
            unsafe {
                chewing_set_maxChiSymbolLen(ctx, 50);
                // if cfg.default_english
                chewing_set_ChiEngMode(ctx, SYMBOL_MODE);
                // if cfg.default_full_space
                chewing_set_ShapeMode(ctx, FULLSHAPE_MODE);
            }

            // Get last mtime of the symbols.dat file
            let symbols_dat = user_dir()?.join("symbols.dat");
            let metadata = std::fs::metadata(&symbols_dat)?;
            self.symbols_file_mtime = metadata
                .modified()?
                .duration_since(UNIX_EPOCH)
                .expect("mtime should be positive")
                .as_secs();
        }

        self.apply_config()?;
        Ok(())
    }

    fn free_chewing_context(&mut self) {}

    fn apply_config(&mut self) -> anyhow::Result<()> {
        self.cfg.reload_if_needed()?;
        let cfg = &self.cfg;

        if let Some(ctx) = &self.chewing_context {
            unsafe {
                chewing_set_addPhraseDirection(*ctx, cfg.add_phrase_forward as i32);
                chewing_set_autoShiftCur(*ctx, cfg.advance_after_selection as i32);
                chewing_set_candPerPage(*ctx, cfg.cand_per_page);
                chewing_set_escCleanAllBuf(*ctx, cfg.esc_clean_all_buf as i32);
                chewing_set_KBType(*ctx, cfg.keyboard_layout);
                chewing_set_spaceAsSelection(*ctx, cfg.show_cand_with_space_key as i32);
                chewing_config_set_str(
                    *ctx,
                    c"chewing.selection_keys".as_ptr(),
                    SEL_KEYS[cfg.sel_key_type as usize].as_ptr(),
                );
                chewing_config_set_int(
                    *ctx,
                    c"chewing.conversion_engine".as_ptr(),
                    cfg.conv_engine,
                );
            }
        }

        // TODO update popup menu to check/uncheck the simplified Chinese item
        // TODO update message window font size
        // TODO update candidate window font size

        Ok(())
    }

    fn update_lang_buttons(&mut self) -> Result<()> {
        if self.chewing_context.is_none() {
            error!("update_lang_buttons called with null chewing context");
            return Ok(());
        }
        let ctx = self.chewing_context.unwrap();

        let g_hinstance = HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut c_void);
        let lang_mode = unsafe { chewing_get_ChiEngMode(ctx) };
        if lang_mode != self.lang_mode {
            self.lang_mode = lang_mode;
            let icon_id = match (is_light_theme(), lang_mode) {
                (true, CHINESE_MODE) => IDI_CHI,
                (true, SYMBOL_MODE) => IDI_ENG,
                (false, CHINESE_MODE) => IDI_CHI_DARK,
                (false, SYMBOL_MODE) => IDI_ENG_DARK,
                _ => unreachable!(),
            };
            if let Some(button) = &self.switch_lang_button {
                unsafe {
                    button.set_icon(LoadIconW(
                        Some(g_hinstance),
                        PCWSTR::from_raw(icon_id as *const u16),
                    )?);
                }
            }
            if let Some(button) = &self.ime_mode_button {
                unsafe {
                    button.set_icon(LoadIconW(
                        Some(g_hinstance),
                        PCWSTR::from_raw(icon_id as *const u16),
                    )?);
                }
            }
        }
        let shape_mode = unsafe { chewing_get_ShapeMode(ctx) };
        if shape_mode != self.shape_mode {
            self.shape_mode = shape_mode;
            let icon_id = if shape_mode == FULLSHAPE_MODE {
                IDI_FULL_SHAPE
            } else {
                IDI_HALF_SHAPE
            };
            if let Some(button) = &self.switch_shape_button {
                unsafe {
                    button.set_icon(LoadIconW(
                        Some(g_hinstance),
                        PCWSTR::from_raw(icon_id as *const u16),
                    )?);
                }
            }
        }
        Ok(())
    }

    fn add_preserved_key(&mut self, keycode: u32, modifiers: u32, guid: GUID) -> Result<()> {
        let preserved_key = TF_PRESERVEDKEY {
            uVKey: keycode,
            uModifiers: modifiers,
        };
        self.preserved_keys.insert(guid.to_u128(), preserved_key);
        if let Some(thread_mgr) = &self.thread_mgr {
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            unsafe { keystroke_mgr.PreserveKey(self.tid, &guid, &preserved_key, &[])? };
        }
        Ok(())
    }

    fn add_button(&mut self, button: ITfLangBarItemButton) -> Result<()> {
        self.lang_bar_buttons.push(button.clone());
        if let Some(thread_mgr) = &self.thread_mgr {
            let lang_bar_item_mgr: ITfLangBarItemMgr = thread_mgr.cast()?;
            unsafe { lang_bar_item_mgr.AddItem(&button)? };
        }
        Ok(())
    }

    fn hide_candidates(&mut self) -> Result<()> {
        Ok(())
    }

    fn hide_message(&mut self) -> Result<()> {
        Ok(())
    }
}

fn init_chewing_env() {
    // FIXME don't use env to control chewing path
    let user_path = user_dir();
    let chewing_path = program_dir();

    // std::env::set_var("CHEWING_USER_PATH", user_path);
    // std::env::set_var("CHEWING_PATH", user_path);
}

fn user_dir() -> Result<PathBuf> {
    // FIXME use chewing::path instead.
    //
    // SHGetFolderPath might fail in impersonation security context.
    // Use %USERPROFILE% to retrieve the user home directory.
    let user_profile =
        PathBuf::from(std::env::var("USERPROFILE").unwrap()).join("ChewingTextService");

    if !user_profile.exists() {
        std::fs::create_dir(&user_profile)?;
        let metadata = user_profile.metadata()?;
        let attributes = metadata.file_attributes();
        let user_profile_w: Vec<u16> = user_profile.as_os_str().encode_wide().collect();
        unsafe {
            SetFileAttributesW(
                &BSTR::from_wide(&user_profile_w),
                FILE_FLAGS_AND_ATTRIBUTES(attributes | FILE_ATTRIBUTE_HIDDEN.0),
            )
        };
    }

    Ok(user_profile)
}

fn program_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var("programfiles(x86)")
            .or_else(|_| std::env::var("programfiles"))
            .unwrap(),
    )
    .join("ChewingTextService"))
}

fn is_light_theme() -> bool {
    true
}
