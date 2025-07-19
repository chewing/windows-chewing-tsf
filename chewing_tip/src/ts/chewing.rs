// SPDX-License-Identifier: GPL-3.0-or-later

use core::slice;
use std::ffi::{CStr, c_int, c_void};
use std::io::ErrorKind;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{Context, Result, bail};
use chewing_capi::candidates::{
    chewing_cand_ChoicePerPage, chewing_cand_Enumerate, chewing_cand_String,
    chewing_cand_TotalChoice, chewing_cand_choose_by_index, chewing_cand_close,
    chewing_cand_hasNext, chewing_get_selKey, chewing_set_candPerPage,
};
use chewing_capi::globals::{
    AUTOLEARN_DISABLED, AUTOLEARN_ENABLED, chewing_config_set_int, chewing_config_set_str,
    chewing_set_addPhraseDirection, chewing_set_autoShiftCur, chewing_set_easySymbolInput,
    chewing_set_escCleanAllBuf, chewing_set_maxChiSymbolLen, chewing_set_phraseChoiceRearward,
    chewing_set_spaceAsSelection,
};
use chewing_capi::input::{
    chewing_handle_Backspace, chewing_handle_Capslock, chewing_handle_CtrlNum,
    chewing_handle_Default, chewing_handle_Del, chewing_handle_Down, chewing_handle_End,
    chewing_handle_Enter, chewing_handle_Esc, chewing_handle_Home, chewing_handle_Left,
    chewing_handle_Numlock, chewing_handle_PageDown, chewing_handle_PageUp, chewing_handle_Right,
    chewing_handle_Space, chewing_handle_Tab, chewing_handle_Up,
};
use chewing_capi::layout::chewing_set_KBType;
use chewing_capi::modes::{
    CHINESE_MODE, FULLSHAPE_MODE, HALFSHAPE_MODE, SYMBOL_MODE, chewing_get_ChiEngMode,
    chewing_get_ShapeMode, chewing_set_ChiEngMode, chewing_set_ShapeMode,
};
use chewing_capi::output::{
    chewing_ack, chewing_aux_Check, chewing_aux_String, chewing_bopomofo_Check,
    chewing_bopomofo_String_static, chewing_buffer_Check, chewing_buffer_String,
    chewing_clean_bopomofo_buf, chewing_commit_Check, chewing_commit_String,
    chewing_commit_preedit_buf, chewing_cursor_Current, chewing_keystroke_CheckIgnore,
};
use chewing_capi::setup::{ChewingContext, chewing_delete, chewing_free, chewing_new};
use log::{debug, error, info};
use windows::Win32::Foundation::{HINSTANCE, POINT, RECT};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_FLAGS_AND_ATTRIBUTES, SetFileAttributesW,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F12,
    VK_HOME, VK_LEFT, VK_MENU, VK_NEXT, VK_NUMLOCK, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT,
    VK_TAB, VK_UP,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, GUID_LBI_INPUTMODE, ITfCompartmentMgr, ITfCompositionSink,
    ITfContext, TF_ATTR_INPUT, TF_DISPLAYATTRIBUTE, TF_ES_READ, TF_ES_READWRITE, TF_ES_SYNC,
    TF_LBI_STYLE_BTN_BUTTON, TF_LBI_STYLE_BTN_MENU, TF_LS_DOT, TF_MOD_CONTROL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CheckMenuItem, GetCursorPos, HMENU, HWND_DESKTOP, LoadIconW, LoadStringW, MF_CHECKED,
    MF_UNCHECKED, SW_SHOWNORMAL, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
    TrackPopupMenu, WINDOW_EX_STYLE, WINDOW_STYLE,
};
use windows::Win32::UI::{
    Input::KeyboardAndMouse::VK_SPACE,
    TextServices::{
        ITfComposition, ITfKeystrokeMgr, ITfLangBarItemButton, ITfLangBarItemMgr, ITfThreadMgr,
        TF_LANGBARITEMINFO, TF_MOD_SHIFT, TF_PRESERVEDKEY,
    },
};
use windows_core::{
    BSTR, ComObject, ComObjectInner, GUID, HSTRING, Interface, InterfaceRef, PCWSTR, PWSTR, w,
};
use zhconv::{Variant, zhconv};

use crate::G_HINSTANCE;
use crate::ts::GUID_INPUT_DISPLAY_ATTRIBUTE;
use crate::ts::display_attribute::register_display_attribute;
use crate::ts::menu::Menu;
use crate::ts::theme::{ThemeDetector, WindowsTheme};
use crate::window::{Window, window_register_class};

use super::CommandType;
use super::config::Config;
use super::edit_session::{EndComposition, SelectionRect, SetCompositionString, StartComposition};
use super::key_event::KeyEvent;
use super::lang_bar::LangBarButton;
use super::resources::*;
use super::ui_elements::{CandidateList, FilterKeyResult, Model, Notification, NotificationModel};

const GUID_MODE_BUTTON: GUID = GUID::from_u128(0xB59D51B9_B832_40D2_9A8D_56959372DDC7);
const GUID_SHAPE_TYPE_BUTTON: GUID = GUID::from_u128(0x5325DBF5_5FBE_467B_ADF0_2395BE9DD2BB);
const GUID_SETTINGS_BUTTON: GUID = GUID::from_u128(0x4FAFA520_2104_407E_A532_9F1AAB7751CD);
const GUID_SHIFT_SPACE: GUID = GUID::from_u128(0xC77A44F5_DB21_474E_A2A2_A17242217AB3);
const GUID_CONTROL_F12: GUID = GUID::from_u128(0x1797B43A_2332_40B4_8007_B2F98F19C047);

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
    lang_mode: i32,
    shape_mode: i32,
    output_simp_chinese: bool,
    last_keydown_code: u16,
    last_keydown_time: Option<Instant>,
    cfg: Config,
    chewing_context: Option<*mut ChewingContext>,

    preserved_keys: BTreeMap<u128, TF_PRESERVEDKEY>,
    lang_bar_buttons: Vec<ITfLangBarItemButton>,
    switch_lang_button: Option<ComObject<LangBarButton>>,
    switch_shape_button: Option<ComObject<LangBarButton>>,
    ime_mode_button: Option<ComObject<LangBarButton>>,
    notification: Option<ComObject<Notification>>,
    candidate_list: Option<ComObject<CandidateList>>,
    thread_mgr: Option<ITfThreadMgr>,
    composition: Option<ITfComposition>,
    composition_sink: Option<ITfCompositionSink>,
    input_da_atom: VARIANT,
    menu: Menu,
    popup_menu: HMENU,
    tid: u32,
}

impl ChewingTextService {
    pub(super) fn new() -> ChewingTextService {
        Default::default()
    }

    pub(super) fn activate(
        &mut self,
        thread_mgr: &ITfThreadMgr,
        tid: u32,
        composition_sink: InterfaceRef<ITfCompositionSink>,
    ) -> Result<()> {
        self.thread_mgr = Some(thread_mgr.clone());
        self.tid = tid;
        self.composition_sink = Some(composition_sink.to_owned());
        self.add_preserved_key(VK_SPACE.0 as u32, TF_MOD_SHIFT, GUID_SHIFT_SPACE)?;
        self.add_preserved_key(VK_F12.0 as u32, TF_MOD_CONTROL, GUID_CONTROL_F12)?;
        let da = TF_DISPLAYATTRIBUTE {
            lsStyle: TF_LS_DOT,
            bAttr: TF_ATTR_INPUT,
            ..Default::default()
        };
        self.input_da_atom = register_display_attribute(&GUID_INPUT_DISPLAY_ATTRIBUTE, da)?;

        let g_hinstance = HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut c_void);

        window_register_class();

        info!("Detected theme info: {:?}", ThemeDetector::get_theme_info());
        info!("Add language bar buttons to switch Chinese/English modes");
        unsafe {
            let mut info = TF_LANGBARITEMINFO {
                clsidService: CLSID_TEXT_SERVICE,
                guidItem: GUID_MODE_BUTTON,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON,
                ..Default::default()
            };
            let tooltip = PWSTR::from_raw(info.szDescription.as_mut_ptr());
            LoadStringW(
                Some(g_hinstance),
                IDS_SWITCH_LANG,
                tooltip,
                info.szDescription.len() as i32,
            );
            let button = LangBarButton::new(
                info,
                BSTR::from_wide(tooltip.as_wide()),
                LoadIconW(Some(g_hinstance), PCWSTR::from_raw(IDI_CHI as *const u16))?,
                HMENU::default(),
                ID_SWITCH_LANG,
                thread_mgr.clone(),
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
            let tooltip = PWSTR::from_raw(info.szDescription.as_mut_ptr());
            LoadStringW(
                Some(g_hinstance),
                IDS_SWITCH_SHAPE,
                tooltip,
                info.szDescription.len() as i32,
            );
            let button = LangBarButton::new(
                info,
                BSTR::from_wide(tooltip.as_wide()),
                LoadIconW(
                    Some(g_hinstance),
                    PCWSTR::from_raw(IDI_HALF_SHAPE as *const u16),
                )?,
                HMENU::default(),
                ID_SWITCH_SHAPE,
                thread_mgr.clone(),
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
            let tooltip = PWSTR::from_raw(info.szDescription.as_mut_ptr());
            LoadStringW(
                Some(g_hinstance),
                IDS_SETTINGS,
                tooltip,
                info.szDescription.len() as i32,
            );
            // TODO we can define the menu in code
            self.menu = Menu::load(g_hinstance, IDR_MENU);
            self.popup_menu = self.menu.sub_menu(0);
            let button = LangBarButton::new(
                info,
                BSTR::from_wide(tooltip.as_wide()),
                LoadIconW(
                    Some(g_hinstance),
                    PCWSTR::from_raw(IDI_CONFIG as *const u16),
                )?,
                self.popup_menu,
                0,
                thread_mgr.clone(),
            )
            .into_object();
            self.add_button(button.to_interface())?;
        }

        // Windows 8 systray IME mode icon
        info!("Add systray IME mode icon to switch Chinese/English modes");
        unsafe {
            let mut info = TF_LANGBARITEMINFO {
                clsidService: CLSID_TEXT_SERVICE,
                guidItem: GUID_LBI_INPUTMODE,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON,
                ..Default::default()
            };
            let tooltip = PWSTR::from_raw(info.szDescription.as_mut_ptr());
            LoadStringW(
                Some(g_hinstance),
                IDS_SWITCH_LANG,
                tooltip,
                info.szDescription.len() as i32,
            );
            let mut icon_id = match (ThemeDetector::detect_theme(), self.lang_mode) {
                (WindowsTheme::Light, CHINESE_MODE) => IDI_CHI,
                (WindowsTheme::Light, SYMBOL_MODE) => IDI_ENG,
                (WindowsTheme::Dark, CHINESE_MODE) => IDI_CHI_DARK,
                (WindowsTheme::Dark, SYMBOL_MODE) => IDI_ENG_DARK,
                _ => IDI_CHI,
            };
            if self.output_simp_chinese {
                icon_id = match icon_id {
                    IDI_CHI => IDI_SIMP,
                    IDI_CHI_DARK => IDI_SIMP_DARK,
                    _ => icon_id,
                }
            }
            let button = LangBarButton::new(
                info,
                BSTR::from_wide(tooltip.as_wide()),
                LoadIconW(Some(g_hinstance), PCWSTR::from_raw(icon_id as *const u16))?,
                HMENU::default(),
                ID_MODE_ICON,
                thread_mgr.clone(),
            )
            .into_object();
            button.set_enabled(true)?;
            self.ime_mode_button = Some(button.clone());
            self.add_button(button.to_interface())?;
        }

        if let Err(error) = self.cfg.reload_if_needed() {
            error!("unable to load config: {error}");
        }

        // FIXME error handling
        if let Err(error) = self.init_chewing_context() {
            error!("unable to initialize chewing: {error}");
        }

        if let Err(error) = self.update_lang_buttons(false) {
            error!("unable to update lang buttons: {error}");
        }

        Ok(())
    }

    pub(super) fn deactivate(&mut self) -> Result<ITfThreadMgr> {
        self.tid = 0;
        self.free_chewing_context();
        self.switch_lang_button = None;
        self.switch_shape_button = None;
        self.ime_mode_button = None;
        self.remove_buttons()?;
        self.remove_preserved_key(VK_SPACE.0 as u32, TF_MOD_SHIFT, GUID_SHIFT_SPACE)?;
        self.remove_preserved_key(VK_F12.0 as u32, TF_MOD_CONTROL, GUID_CONTROL_F12)?;
        self.composition = None;
        self.hide_candidates();
        self.hide_message();

        // TSF doc: The corresponding ITfTextInputProcessor::Deactivate
        // method that shuts down the text service must release all references
        // to the ptim parameter.
        self.thread_mgr.take().context("there is no thread manager")
    }

    pub(super) fn on_kill_focus(&mut self, context: &ITfContext) -> Result<()> {
        if self.is_composing() {
            self.end_composition(context)?;
        }
        self.hide_candidates();
        self.hide_message();
        Ok(())
    }

    pub(super) fn on_focus(&mut self, _context: &ITfContext) -> Result<()> {
        self.apply_config_if_changed()
    }

    pub(super) fn on_keydown(
        &mut self,
        context: &ITfContext,
        ev: KeyEvent,
        dry_run: bool,
    ) -> Result<bool> {
        if let Err(error) = self.apply_config_if_changed() {
            error!("unable to load config: {error}");
        }
        if self.chewing_context.is_none() {
            error!("on_keydown but chewing context is null");
            return Ok(false);
        }
        self.last_keydown_code = ev.vk;
        if self.last_keydown_time.is_none() {
            self.last_keydown_time = Some(Instant::now());
        }
        let enable_caps_lock = self.cfg.enable_caps_lock && ev.is_key_toggled(VK_CAPITAL);
        if !self.is_composing() {
            // don't do further handling in English + half shape mode
            if self.lang_mode == SYMBOL_MODE
                && self.shape_mode == HALFSHAPE_MODE
                && !enable_caps_lock
            {
                debug!("key not handled - in English mode");
                return Ok(false);
            }

            if ev.is_key_down(VK_CONTROL) || ev.is_key_down(VK_MENU) {
                // bypass IME. This might be a shortcut key used in the application
                // FIXME: we only need Ctrl in composition mode for adding user phrases.
                // However, if we turn on easy symbol input with Ctrl support later,
                // we'll need th Ctrl key then.
                debug!("key not handled - Ctrl or Alt modifier key was down");
                return Ok(false);
            }

            // we always need further processing in full shape mode since all English chars,
            // numbers, and symbols need to be converted to full shape Chinese chars.
            if self.shape_mode != FULLSHAPE_MODE {
                // Caps lock is on => English mode
                if enable_caps_lock {
                    // We only need to handle printable keys because we need to
                    // convert them to upper case.
                    if !ev.is_alphabet() {
                        debug!("key not handled - Capslock key toggled");
                        return Ok(false);
                    }
                }
                // NumLock is on
                if ev.is_key_toggled(VK_NUMLOCK) && ev.is_num_pad() {
                    debug!("key not handled - Numlock toggled and key is a numpad key");
                    return Ok(false);
                }
            }
            if !ev.is_printable() {
                debug!("key not handled - key is not printable");
                return Ok(false);
            }
        }

        if dry_run {
            debug!("early return in dry_run mode - key should be handled");
            return Ok(true);
        }

        let Some(ctx) = self.chewing_context else {
            error!("chewing context is null");
            return Ok(false);
        };
        if ev.is_printable() {
            let old_lang_mode = unsafe { chewing_get_ChiEngMode(ctx) };
            let mut momentary_english_mode = false;
            let mut invert_case = false;
            // If caps lock is on, temprarily change to English mode
            if enable_caps_lock {
                invert_case = true;
            }
            // If shift is pressed, but we don't want to enter full shape symbols, or easy_symbol_input is not enabled
            if ev.is_key_down(VK_SHIFT)
                && (!self.cfg.full_shape_symbols || ev.is_alphabet())
                && !self.cfg.easy_symbols_with_shift
            {
                momentary_english_mode = true;
                if !self.cfg.upper_case_with_shift {
                    invert_case = true;
                }
            }
            if self.lang_mode == SYMBOL_MODE || momentary_english_mode {
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
            } else if ev.is_alphabet() {
                unsafe {
                    let mut code = ev.code.to_ascii_lowercase();
                    if ev.is_key_down(VK_SHIFT) && self.cfg.easy_symbols_with_shift {
                        code = ev.code.to_ascii_uppercase();
                    }
                    chewing_handle_Default(ctx, code as i32);
                }
            } else if ev.is_key(VK_SPACE) {
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
        } else {
            let mut key_handled = false;
            if self.cfg.cursor_cand_list {
                if let Some(candidate_list) = &self.candidate_list {
                    match candidate_list.filter_key_event(ev.vk) {
                        FilterKeyResult::HandledCommit => {
                            let sel_key = candidate_list.current_sel();
                            unsafe {
                                chewing_cand_choose_by_index(ctx, sel_key as i32);
                            }
                            key_handled = true;
                        }
                        FilterKeyResult::Handled => {
                            candidate_list.show();
                            return Ok(true);
                        }
                        FilterKeyResult::NotHandled => {
                            // do nothing
                        }
                    }
                }
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
                    },
                    _ => return Ok(false),
                }
            }
        }

        if let Err(error) = self.update_lang_buttons(false) {
            error!("unable to update lang bar button: {error}")
        }

        if unsafe { chewing_keystroke_CheckIgnore(ctx) } == 1 {
            debug!("early return - chewing ignored key");
            return Ok(false);
        }

        if !self.is_composing() {
            self.start_composition(context)?;
        }

        debug!("started composition");

        self.update_candidates(context)?;

        debug!("updated candidates");

        if unsafe { chewing_commit_Check(ctx) } == 1 {
            let ptr = unsafe { chewing_commit_String(ctx) };
            let cstr = unsafe { CStr::from_ptr(ptr) };
            let text = cstr.to_string_lossy().into_owned();
            unsafe {
                chewing_free(ptr.cast());
                chewing_ack(ctx);
            }
            debug!("commit string {}", &text);
            self.set_composition_string(context, &text, 0)?;
            self.end_composition(context)?;
            debug!("commit string ok");
        }

        let mut composition_buf = String::new();
        if unsafe { chewing_buffer_Check(ctx) } == 1 {
            let ptr = unsafe { chewing_buffer_String(ctx) };
            let cstr = unsafe { CStr::from_ptr(ptr) };
            let text = cstr.to_string_lossy().into_owned();
            unsafe {
                chewing_free(ptr.cast());
            }
            composition_buf.push_str(&text);
        }
        if unsafe { chewing_bopomofo_Check(ctx) } == 1 {
            let ptr = unsafe { chewing_bopomofo_String_static(ctx) };
            let cstr = unsafe { CStr::from_ptr(ptr) };
            let pos = unsafe { chewing_cursor_Current(ctx) } as usize;
            let idx = composition_buf
                .char_indices()
                .nth(pos)
                .map(|pair| pair.0)
                .unwrap_or(composition_buf.len());
            composition_buf.insert_str(idx, &cstr.to_string_lossy());
        }

        // has something in composition buffer
        if !composition_buf.is_empty() {
            if !self.is_composing() {
                self.start_composition(context)?;
            }
            let cursor = unsafe { chewing_cursor_Current(ctx) };
            self.set_composition_string(context, &composition_buf, cursor)?;
        } else {
            // nothing left in composition buffer, terminate composition status
            if self.is_composing() {
                self.set_composition_string(context, "", 0)?;
            }
            // We also need to make sure that the candidate window is not
            // currently shown. When typing symbols with ` key, it's possible
            // that the composition string empty, while the candidate window is
            // shown. We should not terminate the composition in this case.
            if self.candidate_list.is_none() {
                self.end_composition(context)?;
            }
        }

        if unsafe { chewing_aux_Check(ctx) } == 1 {
            let ptr = unsafe { chewing_aux_String(ctx) };
            let cstr = unsafe { CStr::from_ptr(ptr) };
            let text = HSTRING::from(cstr.to_string_lossy().as_ref());
            unsafe {
                chewing_free(ptr.cast());
            }
            self.show_message(context, &text, Duration::from_millis(500))?;
        }

        Ok(true)
    }

    pub(super) fn on_keyup(
        &mut self,
        context: &ITfContext,
        ev: KeyEvent,
        dry_run: bool,
    ) -> Result<bool> {
        let Some(ctx) = self.chewing_context else {
            return Ok(false);
        };
        let last_is_shift = self.last_keydown_code == VK_SHIFT.0 && ev.vk == VK_SHIFT.0;
        let last_is_caps_lock = self.last_keydown_code == VK_CAPITAL.0 && ev.vk == VK_CAPITAL.0;
        let hold_duration = self
            .last_keydown_time
            .map(|t| t.elapsed())
            .unwrap_or(Duration::from_secs(1));

        if self.cfg.switch_lang_with_shift
            && hold_duration < Duration::from_secs(1)
            && last_is_shift
        {
            if dry_run {
                return Ok(true);
            }
            if self.cfg.enable_caps_lock && ev.is_key_toggled(VK_CAPITAL) {
                // Locked by CapsLock
                let msg = match unsafe { chewing_get_ChiEngMode(ctx) } {
                    SYMBOL_MODE => HSTRING::from("英數模式 (CapsLock)"),
                    CHINESE_MODE => HSTRING::from("中文模式"),
                    _ => unreachable!(),
                };
                self.show_message(context, &msg, Duration::from_millis(500))?;
                self.last_keydown_time = None;
                self.last_keydown_code = 0;
                return Ok(true);
            } else {
                self.toggle_lang_mode()?;
                let msg = match unsafe { chewing_get_ChiEngMode(ctx) } {
                    SYMBOL_MODE => HSTRING::from("英數模式"),
                    CHINESE_MODE if self.cfg.enable_caps_lock && ev.is_key_toggled(VK_CAPITAL) => {
                        HSTRING::from("英數模式 (CapsLock)")
                    }
                    CHINESE_MODE => HSTRING::from("中文模式"),
                    _ => unreachable!(),
                };
                self.show_message(context, &msg, Duration::from_millis(500))?;
                self.last_keydown_time = None;
                self.last_keydown_code = 0;
                return Ok(true);
            }
        }

        if self.cfg.enable_caps_lock && last_is_caps_lock {
            if dry_run {
                return Ok(true);
            }
            self.toggle_lang_mode()?;
            let msg = match unsafe { chewing_get_ChiEngMode(ctx) } {
                SYMBOL_MODE => HSTRING::from("英數模式 (CapsLock)"),
                CHINESE_MODE => HSTRING::from("中文模式"),
                _ => unreachable!(),
            };
            self.show_message(context, &msg, Duration::from_millis(500))?;
            self.last_keydown_time = None;
            self.last_keydown_code = 0;
            return Ok(true);
        }

        self.last_keydown_time = None;
        self.last_keydown_code = 0;
        Ok(false)
    }

    pub(super) fn on_composition_terminated(&mut self) {
        if let Some(ctx) = self.chewing_context {
            if self.candidate_list.is_some() {
                self.hide_candidates();
                unsafe {
                    chewing_cand_close(ctx);
                }
            }
            if unsafe { chewing_bopomofo_Check(ctx) } == 1 {
                unsafe {
                    chewing_clean_bopomofo_buf(ctx);
                }
            }
            if unsafe { chewing_buffer_Check(ctx) } == 1 {
                unsafe {
                    chewing_commit_preedit_buf(ctx);
                }
            }
        }
        self.composition = None;
    }

    pub(super) fn on_preserved_key(&mut self, guid: &GUID) -> bool {
        if guid == &GUID_SHIFT_SPACE && self.toggle_shape_mode().is_ok() {
            return true;
        }
        if guid == &GUID_CONTROL_F12 && self.toggle_simp_chinese().is_ok() {
            return true;
        }
        false
    }

    pub(super) fn on_compartment_change(&mut self, guid: &GUID) -> Result<()> {
        if let Some(thread_mgr) = &self.thread_mgr {
            if guid == &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE {
                let compartment_mgr: ITfCompartmentMgr = thread_mgr.cast()?;
                unsafe {
                    let thread_compartment =
                        compartment_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)?;
                    let value = thread_compartment.GetValue()?;
                    let openclose: i32 = (&value).try_into().unwrap_or_default();
                    self.on_keyboard_status_changed(openclose != 0)?;
                }
            }
        }
        Ok(())
    }

    fn on_keyboard_status_changed(&mut self, opened: bool) -> Result<()> {
        if opened {
            self.init_chewing_context()?;
        } else {
            let context = self
                .current_context()
                .context("unable to get current ITfContext")?;
            self.on_kill_focus(&context)?;
            self.free_chewing_context();
        }
        if let Some(ime_icon) = &self.ime_mode_button {
            ime_icon.set_enabled(opened)?;
        }
        Ok(())
    }

    pub(super) fn on_command(&mut self, id: u32, cmd_type: CommandType) {
        if matches!(cmd_type, CommandType::RightClick) {
            if id == ID_MODE_ICON {
                // TrackPopupMenu requires a window to work, so let's build a transient one.
                let window = Window::new();
                window.create(
                    HWND_DESKTOP,
                    WINDOW_STYLE::default(),
                    WINDOW_EX_STYLE::default(),
                );
                let mut pos = POINT::default();
                unsafe {
                    let _ = GetCursorPos(&mut pos);
                }
                let ret = unsafe {
                    TrackPopupMenu(
                        self.popup_menu,
                        TPM_NONOTIFY | TPM_RETURNCMD | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                        pos.x,
                        pos.y,
                        None,
                        window.hwnd(),
                        None,
                    )
                };
                if ret.as_bool() {
                    self.on_command(ret.0 as u32, CommandType::Menu);
                }
            }
        } else {
            match id {
                ID_SWITCH_LANG => {
                    if let Err(error) = self.toggle_lang_mode() {
                        error!("unable to toggle lang mode: {error}");
                    }
                }
                ID_SWITCH_SHAPE => {
                    if let Err(error) = self.toggle_shape_mode() {
                        error!("unable to toggle shape mode: {error}");
                    }
                }
                ID_MODE_ICON => {
                    if let Err(error) = self.toggle_lang_mode() {
                        error!("unable to toggle lang mode: {error}");
                    }
                }
                ID_HASHED => {
                    if let Ok(prog_dir) = program_dir() {
                        let exe = prog_dir
                            .join("chewing-editor.exe")
                            .to_string_lossy()
                            .into_owned();
                        unsafe {
                            ShellExecuteW(
                                Some(HWND_DESKTOP),
                                w!("open"),
                                &HSTRING::from(&exe),
                                None,
                                None,
                                SW_SHOWNORMAL,
                            );
                        }
                    }
                }
                ID_CONFIG => {
                    if let Ok(prog_dir) = program_dir() {
                        let exe = prog_dir
                            .join("ChewingPreferences.exe")
                            .to_string_lossy()
                            .into_owned();
                        unsafe {
                            ShellExecuteW(
                                Some(HWND_DESKTOP),
                                w!("open"),
                                &HSTRING::from(&exe),
                                w!("--config"),
                                None,
                                SW_SHOWNORMAL,
                            );
                        }
                    }
                }
                ID_OUTPUT_SIMP_CHINESE => {
                    if let Err(error) = self.toggle_simp_chinese() {
                        error!("unable to toggle simplified chinese: {error}");
                    }
                }
                ID_ABOUT => {
                    if let Ok(prog_dir) = program_dir() {
                        let exe = prog_dir
                            .join("ChewingPreferences.exe")
                            .to_string_lossy()
                            .into_owned();
                        unsafe {
                            ShellExecuteW(
                                Some(HWND_DESKTOP),
                                w!("open"),
                                &HSTRING::from(&exe),
                                w!("--about"),
                                None,
                                SW_SHOWNORMAL,
                            );
                        }
                    }
                }
                ID_WEBSITE => open_url("https://chewing.im/"),
                ID_GROUP => open_url("https://groups.google.com/group/chewing-devel"),
                ID_BUGREPORT => {
                    open_url("https://github.com/chewing/windows-chewing-tsf/issues?state=open")
                }
                ID_DICT_BUGREPORT => open_url("https://github.com/chewing/libchewing-data/issues"),
                ID_MOEDICT => open_url("https://www.moedict.tw/"),
                ID_DICT => open_url("https://dict.revised.moe.edu.tw/"),
                ID_SIMPDICT => open_url("https://dict.concised.moe.edu.tw/"),
                ID_LITTLEDICT => open_url("https://dict.mini.moe.edu.tw/"),
                ID_PROVERBDICT => open_url("https://dict.idioms.moe.edu.tw/"),
                ID_CHEWING_HELP => open_url("https://chewing.im/features.html"),
                _ => {}
            }
        }
    }

    fn start_composition(&mut self, context: &ITfContext) -> Result<()> {
        debug!("going to request start composition");
        if let Some(sink) = &self.composition_sink {
            let session = StartComposition::new(context.clone(), sink.clone()).into_object();
            unsafe {
                context
                    .RequestEditSession(
                        self.tid,
                        session.as_interface(),
                        TF_ES_SYNC | TF_ES_READWRITE,
                    )?
                    .ok()?;
                debug!("requested start composition");
                self.composition = session.composition().cloned();
            }
        }
        Ok(())
    }

    fn end_composition(&mut self, context: &ITfContext) -> Result<()> {
        let Some(composition) = &self.composition else {
            return Ok(());
        };
        let session = EndComposition::new(context, composition).into_object();
        debug!("end composition start");
        unsafe {
            context
                .RequestEditSession(
                    self.tid,
                    session.as_interface(),
                    TF_ES_SYNC | TF_ES_READWRITE,
                )?
                .ok()?;
        }
        debug!("end composition");
        drop(session);
        self.composition = None;
        Ok(())
    }

    fn set_composition_string(
        &mut self,
        context: &ITfContext,
        text: &str,
        cursor: i32,
    ) -> Result<()> {
        let Some(composition) = &self.composition else {
            return Ok(());
        };
        debug!("set composition string to {text}");
        let htext = if self.output_simp_chinese {
            zhconv(text, Variant::ZhHans).into()
        } else {
            text.into()
        };
        let session = SetCompositionString::new(
            context,
            composition,
            self.input_da_atom.clone(),
            &htext,
            cursor,
        )
        .into_object();
        unsafe {
            match context.RequestEditSession(
                self.tid,
                session.as_interface(),
                TF_ES_SYNC | TF_ES_READWRITE,
            ) {
                Err(error) => error!("failed to request edit session: {error}"),
                Ok(res) => {
                    if let Err(error) = res.ok() {
                        error!("failed to set composition: {error}")
                    }
                }
            }
        }
        debug!("done compose {text}");
        Ok(())
    }

    fn show_message(&mut self, context: &ITfContext, text: &HSTRING, dur: Duration) -> Result<()> {
        if !self.cfg.show_notification {
            return Ok(());
        }
        unsafe {
            let view = context.GetActiveView()?;
            // UILess console may not have valid HWND
            let hwnd = view.GetWnd().unwrap_or_default();
            if let Some(thread_mgr) = &self.thread_mgr {
                let notification = Notification::new(hwnd, thread_mgr.clone())?;
                notification.set_model(NotificationModel {
                    text: text.clone(),
                    font_family: self.cfg.font_family.clone(),
                    font_size: self.cfg.font_size as f32,
                });
                let mut rect = self.get_selection_rect(context)?;
                rect.bottom += 50;
                rect.left += 50;
                notification.set_position(rect.left, rect.bottom);
                notification.show();
                notification.set_timer(dur);
                self.notification = Some(notification);
            }
        }
        Ok(())
    }

    fn hide_message(&mut self) {
        if let Some(notification) = self.notification.take() {
            notification.set_timer(Duration::ZERO);
            notification.end_ui_element();
        }
    }

    fn get_selection_rect(&self, context: &ITfContext) -> Result<RECT> {
        let session = SelectionRect::new(context).into_object();
        unsafe {
            context
                .RequestEditSession(self.tid, session.as_interface(), TF_ES_SYNC | TF_ES_READ)?
                .ok()?;
        }
        session.rect().cloned().context("there is no selection")
    }

    fn update_candidates(&mut self, context: &ITfContext) -> Result<()> {
        let Some(ctx) = self.chewing_context else {
            error!("chewing context was null");
            return Ok(());
        };
        if unsafe { chewing_cand_TotalChoice(ctx) } == 0 {
            self.hide_candidates();
            return Ok(());
        }
        if self.candidate_list.is_none() {
            let view = unsafe { context.GetActiveView()? };
            // UILess console may not have valid HWND
            let hwnd = unsafe { view.GetWnd().unwrap_or_default() };
            if let Some(thread_mgr) = &self.thread_mgr {
                let candidate_list = CandidateList::new(hwnd, thread_mgr.clone())?;
                self.candidate_list = Some(candidate_list);
            }
        }

        if let Some(candidate_list) = &self.candidate_list {
            unsafe {
                let sel_keys = slice::from_raw_parts(chewing_get_selKey(ctx), 10);
                let n = chewing_cand_ChoicePerPage(ctx) as usize;
                let mut items = vec![];

                chewing_cand_Enumerate(ctx);
                for _ in 0..n {
                    if chewing_cand_hasNext(ctx) != 1 {
                        break;
                    }
                    let ptr = chewing_cand_String(ctx);
                    items.push(CStr::from_ptr(ptr).to_string_lossy().into_owned());
                    chewing_free(ptr.cast());
                }
                candidate_list.set_model(Model {
                    items,
                    selkeys: sel_keys.iter().take(n).map(|&k| k as u16).collect(),
                    cand_per_row: self.cfg.cand_per_row as u32,
                    font_family: self.cfg.font_family.clone(),
                    font_size: self.cfg.font_size as f32,
                    fg_color: self.cfg.font_fg_color,
                    bg_color: self.cfg.font_bg_color,
                    highlight_fg_color: self.cfg.font_highlight_fg_color,
                    highlight_bg_color: self.cfg.font_highlight_bg_color,
                    selkey_color: self.cfg.font_number_fg_color,
                    use_cursor: self.cfg.cursor_cand_list,
                    current_sel: 0,
                });
            }

            candidate_list.show();

            if let Ok(rect) = self.get_selection_rect(context) {
                candidate_list.set_position(rect.left, rect.bottom);
            }
        }

        Ok(())
    }

    fn hide_candidates(&mut self) {
        if let Some(candidate_list) = self.candidate_list.take() {
            candidate_list.end_ui_element();
        }
    }

    fn toggle_simp_chinese(&mut self) -> Result<()> {
        self.output_simp_chinese = !self.output_simp_chinese;
        debug!(
            "toggle output simplified chinese: {}",
            self.output_simp_chinese
        );
        let check_flag = if self.output_simp_chinese {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        unsafe {
            CheckMenuItem(self.popup_menu, ID_OUTPUT_SIMP_CHINESE, check_flag.0);
        }
        self.update_lang_buttons(true)?;
        Ok(())
    }

    fn toggle_shape_mode(&mut self) -> Result<()> {
        if let Some(ctx) = self.chewing_context {
            unsafe {
                chewing_set_ShapeMode(
                    ctx,
                    match chewing_get_ShapeMode(ctx) {
                        0 => 1,
                        _ => 0,
                    },
                );
            }
            self.update_lang_buttons(false)?;
        }
        Ok(())
    }

    fn toggle_lang_mode(&mut self) -> Result<()> {
        if let Some(ctx) = self.chewing_context {
            unsafe {
                // HACK: send capslock to switch mode
                chewing_handle_Capslock(ctx);
            }
            self.update_lang_buttons(false)?;
        }
        Ok(())
    }

    fn current_context(&self) -> Option<ITfContext> {
        let Some(thread_mgr) = &self.thread_mgr else {
            return None;
        };
        unsafe {
            let doc_mgr = thread_mgr.GetFocus().ok()?;
            doc_mgr.GetTop().ok()
        }
    }

    fn is_composing(&self) -> bool {
        self.composition.is_some()
    }

    fn init_chewing_context(&mut self) -> anyhow::Result<()> {
        // FIXME assert ctx should be none
        if self.chewing_context.is_none() {
            if let Err(error) = init_chewing_env() {
                error!("unable to init chewing env, init may fail: {error}");
            }
            let ctx = chewing_new();
            if ctx.is_null() {
                bail!("chewing context is null");
            }
            log::set_max_level(log::LevelFilter::Info);
            self.chewing_context = Some(ctx);
        }

        self.apply_config();

        let ev = KeyEvent::default();
        // XXX assumes there is only one keyboard
        let capslock = ev.is_key_toggled(VK_CAPITAL);
        if let Some(ctx) = self.chewing_context {
            unsafe {
                chewing_set_maxChiSymbolLen(ctx, 50);
                if self.cfg.default_english || capslock {
                    chewing_set_ChiEngMode(ctx, SYMBOL_MODE);
                } else {
                    chewing_set_ChiEngMode(ctx, CHINESE_MODE);
                }
                if self.cfg.default_full_space {
                    chewing_set_ShapeMode(ctx, FULLSHAPE_MODE);
                }
            }
        }
        Ok(())
    }

    fn free_chewing_context(&mut self) {
        if let Some(ctx) = self.chewing_context.take() {
            unsafe {
                chewing_delete(ctx);
            }
        }
    }

    fn apply_config_if_changed(&mut self) -> anyhow::Result<()> {
        if self.cfg.reload_if_needed()? {
            self.apply_config();
        }
        Ok(())
    }

    fn apply_config(&mut self) {
        let cfg = &self.cfg;
        if let Some(ctx) = self.chewing_context {
            unsafe {
                chewing_set_easySymbolInput(ctx, cfg.easy_symbols_with_shift as i32);
                chewing_set_addPhraseDirection(ctx, cfg.add_phrase_forward as i32);
                chewing_set_phraseChoiceRearward(ctx, cfg.phrase_choice_rearward as i32);
                chewing_set_autoShiftCur(ctx, cfg.advance_after_selection as i32);
                chewing_set_candPerPage(ctx, cfg.cand_per_page);
                chewing_set_escCleanAllBuf(ctx, cfg.esc_clean_all_buf as i32);
                chewing_set_KBType(ctx, cfg.keyboard_layout);
                chewing_set_spaceAsSelection(ctx, cfg.show_cand_with_space_key as i32);
                chewing_config_set_str(
                    ctx,
                    c"chewing.selection_keys".as_ptr(),
                    SEL_KEYS[cfg.sel_key_type as usize].as_ptr(),
                );
                chewing_config_set_int(ctx, c"chewing.conversion_engine".as_ptr(), cfg.conv_engine);
                chewing_config_set_int(
                    ctx,
                    c"chewing.disable_auto_learn_phrase".as_ptr(),
                    if cfg.enable_auto_learn {
                        AUTOLEARN_ENABLED
                    } else {
                        AUTOLEARN_DISABLED
                    } as c_int,
                );
            }
        }
        self.output_simp_chinese = cfg.output_simp_chinese;
        let check_flag = if self.output_simp_chinese {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        unsafe {
            CheckMenuItem(self.popup_menu, ID_OUTPUT_SIMP_CHINESE, check_flag.0);
        }
        let _ = self.update_lang_buttons(true);
    }

    fn update_lang_buttons(&mut self, check_simp_mode: bool) -> Result<()> {
        let Some(ctx) = self.chewing_context else {
            error!("update_lang_buttons called with null chewing context");
            return Ok(());
        };

        let g_hinstance = HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut c_void);
        let lang_mode = unsafe { chewing_get_ChiEngMode(ctx) };
        if lang_mode != self.lang_mode || check_simp_mode {
            self.lang_mode = lang_mode;
            let mut icon_id = match (ThemeDetector::detect_theme(), self.lang_mode) {
                (WindowsTheme::Light, CHINESE_MODE) => IDI_CHI,
                (WindowsTheme::Light, SYMBOL_MODE) => IDI_ENG,
                (WindowsTheme::Dark, CHINESE_MODE) => IDI_CHI_DARK,
                (WindowsTheme::Dark, SYMBOL_MODE) => IDI_ENG_DARK,
                _ => IDI_CHI,
            };
            if self.output_simp_chinese {
                icon_id = match icon_id {
                    IDI_CHI => IDI_SIMP,
                    IDI_CHI_DARK => IDI_SIMP_DARK,
                    _ => icon_id,
                }
            }
            if let Some(button) = &self.switch_lang_button {
                unsafe {
                    button.set_icon(LoadIconW(
                        Some(g_hinstance),
                        PCWSTR::from_raw(icon_id as *const u16),
                    )?)?;
                }
            }
            if let Some(button) = &self.ime_mode_button {
                unsafe {
                    button.set_icon(LoadIconW(
                        Some(g_hinstance),
                        PCWSTR::from_raw(icon_id as *const u16),
                    )?)?;
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
                    )?)?;
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
            if let Err(error) =
                unsafe { keystroke_mgr.PreserveKey(self.tid, &guid, &preserved_key, &[]) }
            {
                error!("unable to add preserved key: {error}");
            }
        }
        Ok(())
    }

    fn remove_preserved_key(&mut self, keycode: u32, modifiers: u32, guid: GUID) -> Result<()> {
        let preserved_key = TF_PRESERVEDKEY {
            uVKey: keycode,
            uModifiers: modifiers,
        };
        self.preserved_keys.remove(&guid.to_u128());
        if let Some(thread_mgr) = &self.thread_mgr {
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            if let Err(error) = unsafe { keystroke_mgr.UnpreserveKey(&guid, &preserved_key) } {
                error!("unable to remove preserved key: {error}");
            }
        }
        Ok(())
    }

    fn add_button(&mut self, button: ITfLangBarItemButton) -> Result<()> {
        self.lang_bar_buttons.push(button.clone());
        if let Some(thread_mgr) = &self.thread_mgr {
            let lang_bar_item_mgr: ITfLangBarItemMgr = thread_mgr.cast()?;
            if let Err(error) = unsafe { lang_bar_item_mgr.AddItem(&button) } {
                error!("unable to add lang bar item: {error}");
            }
        }
        Ok(())
    }

    fn remove_buttons(&mut self) -> Result<()> {
        if let Some(thread_mgr) = &self.thread_mgr {
            let lang_bar_item_mgr: ITfLangBarItemMgr = thread_mgr.cast()?;
            for button in self.lang_bar_buttons.drain(0..) {
                if let Err(error) = unsafe { lang_bar_item_mgr.RemoveItem(&button) } {
                    error!("unable to remove lang bar item: {error}");
                }
            }
        }
        Ok(())
    }
}

fn init_chewing_env() -> Result<()> {
    // FIXME don't use env to control chewing path
    let user_path = user_dir()?;
    let chewing_path = format!(
        "{};{}",
        user_path.display(),
        program_dir()?.join("Dictionary").display()
    );

    unsafe {
        std::env::set_var("CHEWING_PATH", &chewing_path);
    }
    Ok(())
}

fn user_dir() -> Result<PathBuf> {
    let user_dir = chewing::path::data_dir().context("unable to determine user_dir")?;

    // NB: chewing might be loaded into a low mandatory integrity level process (SearchHost.exe).
    // In that case, it might not be able to check if a file exists using CreateFile
    // If the file exists, it will get the PermissionDenied error instead.
    let user_dir_exists = match std::fs::exists(&user_dir) {
        Ok(true) => true,
        Err(e) => matches!(e.kind(), ErrorKind::PermissionDenied),
        _ => false,
    };

    if !user_dir_exists {
        std::fs::create_dir(&user_dir)?;
        let metadata = user_dir.metadata()?;
        let attributes = metadata.file_attributes();
        let user_dir_w: Vec<u16> = user_dir.as_os_str().encode_wide().collect();
        unsafe {
            SetFileAttributesW(
                &BSTR::from_wide(&user_dir_w),
                FILE_FLAGS_AND_ATTRIBUTES(attributes | FILE_ATTRIBUTE_HIDDEN.0),
            )?;
        };
    }

    Ok(user_dir)
}

fn program_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var("ProgramW6432")
            .or_else(|_| std::env::var("ProgramFiles"))
            .or_else(|_| std::env::var("FrogramFiles(x86)"))?,
    )
    .join("ChewingTextService"))
}

fn open_url(url: &str) {
    unsafe {
        ShellExecuteW(None, None, &HSTRING::from(url), None, None, SW_SHOWNORMAL);
    }
}
