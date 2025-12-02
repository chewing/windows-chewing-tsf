// SPDX-License-Identifier: GPL-3.0-or-later

use core::slice;
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_int, c_void};
use std::io::ErrorKind;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use chewing::input::KeyState;
use chewing::input::keysym::{Keysym, SYM_CAPSLOCK, SYM_LEFTSHIFT, SYM_RIGHTSHIFT, SYM_SPACE};
use chewing_capi::candidates::{
    chewing_cand_ChoicePerPage, chewing_cand_CurrentPage, chewing_cand_Enumerate,
    chewing_cand_String, chewing_cand_TotalChoice, chewing_cand_TotalPage,
    chewing_cand_choose_by_index, chewing_cand_close, chewing_cand_hasNext, chewing_get_selKey,
    chewing_set_candPerPage,
};
use chewing_capi::globals::{
    AUTOLEARN_DISABLED, AUTOLEARN_ENABLED, chewing_config_set_int, chewing_config_set_str,
    chewing_set_addPhraseDirection, chewing_set_autoShiftCur, chewing_set_easySymbolInput,
    chewing_set_escCleanAllBuf, chewing_set_maxChiSymbolLen, chewing_set_phraseChoiceRearward,
    chewing_set_spaceAsSelection,
};
use chewing_capi::input::{chewing_handle_KeyboardEvent, chewing_handle_ShiftSpace};
use chewing_capi::layout::{KB, chewing_get_KBType, chewing_set_KBType};
use chewing_capi::modes::{
    CHINESE_MODE, FULLSHAPE_MODE, HALFSHAPE_MODE, SYMBOL_MODE, chewing_get_ChiEngMode,
    chewing_get_ShapeMode, chewing_set_ChiEngMode, chewing_set_ShapeMode,
};
use chewing_capi::output::{
    chewing_ack, chewing_aux_Check, chewing_aux_String, chewing_bopomofo_Check,
    chewing_bopomofo_String_static, chewing_buffer_Check, chewing_buffer_String,
    chewing_clean_bopomofo_buf, chewing_clean_preedit_buf, chewing_commit_Check,
    chewing_commit_String, chewing_cursor_Current, chewing_keystroke_CheckIgnore,
};
use chewing_capi::setup::{ChewingContext, chewing_delete, chewing_free, chewing_new};
use tracing::{debug, error, info};
use windows::Foundation::Uri;
use windows::System::Launcher;
use windows::Win32::Foundation::{GetLastError, HINSTANCE, POINT, RECT};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_FLAGS_AND_ATTRIBUTES, SetFileAttributesW,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_EMPTYCONTEXT, GUID_COMPARTMENT_KEYBOARD_DISABLED,
    GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, GUID_LBI_INPUTMODE, ITfCompartmentMgr, ITfCompositionSink,
    ITfContext, TF_ATTR_INPUT, TF_DISPLAYATTRIBUTE, TF_ES_ASYNCDONTCARE, TF_ES_READ,
    TF_ES_READWRITE, TF_ES_SYNC, TF_LBI_STYLE_BTN_BUTTON, TF_LBI_STYLE_BTN_MENU, TF_LS_DOT,
    TF_SD_READONLY,
};
use windows::Win32::UI::TextServices::{
    ITfComposition, ITfLangBarItemButton, ITfLangBarItemMgr, ITfThreadMgr, TF_LANGBARITEMINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CheckMenuItem, EnableMenuItem, GetCursorPos, HMENU, LoadIconW, LoadStringW, MF_CHECKED,
    MF_ENABLED, MF_GRAYED, MF_UNCHECKED, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_LEFTBUTTON,
    TPM_NONOTIFY, TPM_RETURNCMD, TrackPopupMenu,
};
use windows_core::{BSTR, ComObject, ComObjectInner, GUID, HSTRING, Interface, PCWSTR, PWSTR};
use zhconv::{Variant, zhconv};

use crate::com::G_HINSTANCE;
use crate::config::{Config, color_s};
use crate::keybind::Keybinding;
use crate::text_service::TextService;
use crate::ui::window::window_register_class;

use super::CommandType;
use super::GUID_INPUT_DISPLAY_ATTRIBUTE;
use super::display_attribute::register_display_attribute;
use super::edit_session::InsertText;
use super::edit_session::{EndComposition, SelectionRect, SetCompositionString};
use super::key_event::KeyEvent;
use super::lang_bar::LangBarButton;
use super::menu::Menu;
use super::resources::*;
use super::theme::{ThemeDetector, WindowsTheme};
use super::ui_elements::{CandidateList, FilterKeyResult, Model, Notification, NotificationModel};

const GUID_MODE_BUTTON: GUID = GUID::from_u128(0xB59D51B9_B832_40D2_9A8D_56959372DDC7);
const GUID_SHAPE_TYPE_BUTTON: GUID = GUID::from_u128(0x5325DBF5_5FBE_467B_ADF0_2395BE9DD2BB);
const GUID_SETTINGS_BUTTON: GUID = GUID::from_u128(0x4FAFA520_2104_407E_A532_9F1AAB7751CD);
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

#[derive(Debug)]
enum ShiftKeyState {
    Down(Instant),
    Consumed,
    Up,
}

impl ShiftKeyState {
    fn release(&mut self) -> Duration {
        let duration = match self {
            ShiftKeyState::Down(instant) => instant.elapsed(),
            ShiftKeyState::Consumed | ShiftKeyState::Up => Duration::MAX,
        };
        *self = ShiftKeyState::Up;
        duration
    }
}

pub(super) struct CommitString {
    pub(super) text: HSTRING,
    pub(super) cursor: i32,
}

pub(super) struct ChewingTextService {
    // === readonly ===
    thread_mgr: ITfThreadMgr,
    tid: u32,
    input_da_atom: VARIANT,
    _menu: Menu,
    popup_menu: HMENU,
    lang_bar_buttons: Vec<ITfLangBarItemButton>,

    // === mutable ===
    lang_mode: Cell<i32>,
    output_simp_chinese: Cell<bool>,
    open: Cell<bool>,
    shift_key_state: RefCell<ShiftKeyState>,
    cfg: RefCell<Config>,
    keybindings: RefCell<Vec<Keybinding>>,
    chewing_context: *mut ChewingContext,
    switch_lang_button: ComObject<LangBarButton>,
    switch_shape_button: ComObject<LangBarButton>,
    ime_mode_button: ComObject<LangBarButton>,
    notification: Cell<Option<ComObject<Notification>>>,
    candidate_list: RefCell<Option<ComObject<CandidateList>>>,
    composition: Rc<RefCell<Option<ITfComposition>>>,
    composition_sink: ITfCompositionSink,
    pending_edit: RefCell<Weak<RefCell<CommitString>>>,
}

impl ChewingTextService {
    pub(super) fn new(
        thread_mgr: ITfThreadMgr,
        tid: u32,
        ts: ComObject<TextService>,
    ) -> Result<ChewingTextService> {
        let da = TF_DISPLAYATTRIBUTE {
            lsStyle: TF_LS_DOT,
            bAttr: TF_ATTR_INPUT,
            ..Default::default()
        };
        let input_da_atom = register_display_attribute(&GUID_INPUT_DISPLAY_ATTRIBUTE, da)?;

        let g_hinstance = HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut c_void);
        let menu = Menu::load(g_hinstance, IDR_MENU);

        window_register_class();

        let lang_bar_item_mgr: ITfLangBarItemMgr = thread_mgr.cast()?;
        info!("Detected theme info: {:?}", ThemeDetector::get_theme_info());
        info!("Add language bar buttons to switch Chinese/English modes");
        let switch_lang_button = unsafe {
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
            lang_bar_item_mgr.AddItem(button.as_interface())?;
            button
        };

        info!("Add language bar buttons to toggle full shape/half shape modes");
        let switch_shape_button = unsafe {
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
            lang_bar_item_mgr.AddItem(button.as_interface())?;
            button
        };

        info!("Add button for settings and others, may open a popup menu");
        let (settings_button, popup_menu) = unsafe {
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
            let popup_menu = menu.sub_menu(0);
            let button = LangBarButton::new(
                info,
                BSTR::from_wide(tooltip.as_wide()),
                LoadIconW(
                    Some(g_hinstance),
                    PCWSTR::from_raw(IDI_CONFIG as *const u16),
                )?,
                popup_menu,
                0,
                thread_mgr.clone(),
            )
            .into_object();
            lang_bar_item_mgr.AddItem(button.as_interface())?;
            (button, popup_menu)
        };

        // Windows 8 systray IME mode icon
        info!("Add systray IME mode icon to switch Chinese/English modes");
        let ime_mode_button = unsafe {
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
            let button = LangBarButton::new(
                info,
                BSTR::from_wide(tooltip.as_wide()),
                LoadIconW(Some(g_hinstance), PCWSTR::from_raw(IDI_CHI as *const u16))?,
                HMENU::default(),
                ID_MODE_ICON,
                thread_mgr.clone(),
            )
            .into_object();
            lang_bar_item_mgr.AddItem(button.as_interface())?;
            button
        };
        let lang_bar_buttons = vec![
            switch_lang_button.cast()?,
            switch_shape_button.cast()?,
            settings_button.cast()?,
            ime_mode_button.cast()?,
        ];

        let cfg = Config::from_reg().unwrap_or_else(|error| {
            error!("unable to load config: {error}");
            Config::default()
        });

        let mut cts = ChewingTextService {
            thread_mgr,
            tid,
            composition_sink: ts.cast()?,
            input_da_atom,
            _menu: menu,
            popup_menu,
            lang_mode: Default::default(),
            open: Cell::new(true),
            output_simp_chinese: Default::default(),
            shift_key_state: RefCell::new(ShiftKeyState::Up),
            cfg: RefCell::new(cfg),
            keybindings: RefCell::new(vec![]),
            chewing_context: Default::default(),
            lang_bar_buttons,
            switch_lang_button,
            switch_shape_button,
            ime_mode_button,
            notification: Default::default(),
            candidate_list: Default::default(),
            composition: Default::default(),
            pending_edit: RefCell::new(Weak::new()),
        };

        // FIXME error handling
        if let Err(error) = cts.init_chewing_context() {
            error!("unable to initialize chewing: {error}");
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .as_ref()
            .map(Duration::as_secs)
            .unwrap_or_default();
        if cts.cfg.borrow().chewing_tsf.auto_check_update_channel != "none"
            && now.abs_diff(cts.cfg.borrow().chewing_tsf.last_update_check_time) > 3600
        {
            open_url("chewing-update-svc://check-now");
        }
        Ok(cts)
    }

    pub(super) fn deactivate(mut self) -> ITfThreadMgr {
        unsafe {
            chewing_delete(self.chewing_context);
        }
        if let Err(error) = self.remove_buttons() {
            error!("failed to remove buttons: {error:#}");
        }
        // TSF doc: The corresponding ITfTextInputProcessor::Deactivate
        // method that shuts down the text service must release all references
        // to the ptim parameter.
        self.thread_mgr
    }

    pub(super) fn on_kill_focus(&self, context: &ITfContext) -> Result<()> {
        if self.is_composing() {
            self.end_composition(context)?;
        }
        self.hide_candidates();
        self.hide_message();
        Ok(())
    }

    pub(super) fn on_focus(&self) -> Result<()> {
        self.apply_config_if_changed()?;
        self.sync_lang_mode()?;
        Ok(())
    }

    pub(super) fn on_test_keydown(&self, context: &ITfContext, ev: KeyEvent) -> Result<bool> {
        let evt = ev.to_keyboard_event(self.cfg.borrow().chewing_tsf.simulate_english_layout);
        let simulate_english_layout = self.cfg.borrow().chewing_tsf.simulate_english_layout != 0;
        // Determine shift key state here, this might be our last chance seeing this key.
        if evt.ksym != SYM_LEFTSHIFT
            && evt.ksym != SYM_RIGHTSHIFT
            && evt.is_state_on(KeyState::Shift)
        {
            self.shift_key_state.replace(ShiftKeyState::Consumed);
        }
        debug!(?evt, shift_key_state = ?self.shift_key_state.borrow(), "on_test_keydown");
        //
        // Step 1. apply any config changes
        //
        if let Err(error) = self.apply_config_if_changed() {
            error!("unable to load config: {error}");
        }
        //
        // Step 2. handle any mode change related keydown
        //
        //
        // Step 2.1 handle switch lang with Shift
        //
        if (evt.ksym == SYM_LEFTSHIFT || evt.ksym == SYM_RIGHTSHIFT)
            && self.cfg.borrow().chewing_tsf.switch_lang_with_shift
        {
            return Ok(true);
        }
        //
        // Step 2.2 handle switch lang with CapsLock
        //
        if self.cfg.borrow().chewing_tsf.enable_caps_lock && !self.open.get() {
            // Disable all processing when disabled
            return Ok(false);
        }
        //
        // Step 2.3 handle any keybindings
        //
        if self.keybindings.borrow().iter().any(|kb| kb.matches(&evt)) {
            return Ok(true);
        }
        //
        // Step 2.4 ignore CapsLock if disabled
        if evt.ksym == SYM_CAPSLOCK && !self.cfg.borrow().chewing_tsf.enable_caps_lock {
            return Ok(false);
        }
        //
        // Step 3. ignore key events if the document is readonly or inactive
        //
        let status = unsafe { context.GetStatus()? };
        if status.dwDynamicFlags & TF_SD_READONLY != 0 {
            debug!("key not handled - readonly document");
            return Ok(false);
        }
        let compartment_mgr: ITfCompartmentMgr = context.cast()?;
        unsafe {
            if let Ok(empty_context) =
                compartment_mgr.GetCompartment(&GUID_COMPARTMENT_EMPTYCONTEXT)
            {
                let value = i32::try_from(&empty_context.GetValue()?)?;
                if value == 1 {
                    debug!("key not handled - empty context");
                    return Ok(false);
                }
            }
            if let Ok(disabled) =
                compartment_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_DISABLED)
            {
                let value = i32::try_from(&disabled.GetValue()?)?;
                if value == 1 {
                    debug!("key not handled - keyboard disabled");
                    return Ok(false);
                }
            }
        }
        //
        // Step 4. ignore key events if they might be shortcut keys
        //
        if evt.is_state_on(KeyState::Alt) {
            // bypass IME. This might be a shortcut key used in the application
            debug!("key not handled - Alt modifier key was down");
            return Ok(false);
        }
        if evt.is_state_on(KeyState::Control) {
            // bypass IME. This might be a shortcut key used in the application
            if self.is_composing() && evt.ksym.is_digit() {
                // need to handle userphrase
                return Ok(true);
            } else if evt.is_state_on(KeyState::Shift)
                && self.cfg.borrow().chewing_tsf.easy_symbols_with_shift_ctrl
            {
                // need to handle easy symbol input
                return Ok(true);
            } else {
                debug!("key not handled - Ctrl modifier key was down");
                return Ok(false);
            }
        }
        if !self.is_composing() {
            let shape_mode = unsafe { chewing_get_ShapeMode(self.chewing_context) };
            // don't do further handling in pure English + half shape mode
            if self.lang_mode.get() == SYMBOL_MODE
                && shape_mode == HALFSHAPE_MODE
                && !simulate_english_layout
            {
                if evt.ksym == SYM_SPACE
                    && evt.is_state_on(KeyState::Shift)
                    && self.cfg.borrow().chewing_tsf.enable_fullwidth_toggle_key
                {
                    // need to handle fullwidth mode switch
                    return Ok(true);
                } else if evt.is_state_on(KeyState::CapsLock)
                    && evt.ksym.is_unicode()
                    && self.cfg.borrow().chewing_tsf.enable_caps_lock
                {
                    // need to invert case
                    return Ok(true);
                } else {
                    debug!("key not handled - in English mode");
                    return Ok(false);
                }
            }
            // No need to handle VK_SPACE when not composing and not fullshape mode
            // This make the space key available for other shortcuts
            if evt.ksym == SYM_SPACE && !evt.is_state_on(KeyState::Shift) {
                return Ok(false);
            }
            if !evt.ksym.is_unicode() {
                debug!("key not handled - key is not printable");
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn on_keydown(&self, context: &ITfContext, ev: KeyEvent) -> Result<bool> {
        if !self.on_test_keydown(context, ev)? {
            return Ok(false);
        }
        let mut evt = ev.to_keyboard_event(self.cfg.borrow().chewing_tsf.simulate_english_layout);
        debug!(?evt, "on_keydown");

        if (evt.ksym == SYM_LEFTSHIFT || evt.ksym == SYM_RIGHTSHIFT)
            && self.cfg.borrow().chewing_tsf.switch_lang_with_shift
            && matches!(*self.shift_key_state.borrow(), ShiftKeyState::Up)
        {
            debug!("shift_key_state = Down");
            self.shift_key_state
                .replace(ShiftKeyState::Down(Instant::now()));
        }

        // Handle keybindings
        if let Some(keybinding) = self.keybindings.borrow().iter().find(|kb| kb.matches(&evt)) {
            match keybinding.action.as_str() {
                "toggle_simplified_chinese" => {
                    self.toggle_simp_chinese()?;
                }
                "toggle_hsu_keyboard" => {
                    self.toggle_hsu_keyboard(context)?;
                }
                act => {
                    error!("Unsupported keybinding action: {act}");
                }
            }
            return Ok(true);
        }

        let ctx = self.chewing_context;
        if evt.ksym.is_unicode() {
            let mut momentary_english_mode = false;
            let mut invert_case = false;
            // Invert case if the SYMBOL_MODE is forced by CapsLock
            if self.lang_mode.get() == SYMBOL_MODE
                && evt.is_state_on(KeyState::CapsLock)
                && self.cfg.borrow().chewing_tsf.enable_caps_lock
            {
                invert_case = true;
            }
            // HACK: invert case if in selection mode
            if evt.is_state_on(KeyState::CapsLock) && self.candidate_list.borrow().is_some() {
                invert_case = true;
            }
            // If shift is pressed, but we don't want to enter full shape symbols, or easy_symbol_input is not enabled
            if evt.is_state_on(KeyState::Shift)
                && (!self.cfg.borrow().chewing_tsf.full_shape_symbols || evt.ksym.is_atoz())
                && !self.cfg.borrow().chewing_tsf.easy_symbols_with_shift
                && !(evt.is_state_on(KeyState::Control)
                    && self.cfg.borrow().chewing_tsf.easy_symbols_with_shift_ctrl)
            {
                momentary_english_mode = true;
                if !self.cfg.borrow().chewing_tsf.upper_case_with_shift {
                    invert_case = true;
                }
            }
            evt.ksym = if invert_case && evt.ksym.is_ascii() {
                let code = evt.ksym.to_unicode();
                if code.is_ascii_uppercase() {
                    Keysym::from(code.to_ascii_lowercase())
                } else {
                    Keysym::from(code.to_ascii_uppercase())
                }
            } else {
                evt.ksym
            };
            if evt.ksym == SYM_SPACE && evt.is_state_on(KeyState::Shift) {
                unsafe {
                    chewing_handle_ShiftSpace(ctx);
                }
            } else if self.lang_mode.get() == SYMBOL_MODE || momentary_english_mode {
                let old_lang_mode = unsafe { chewing_get_ChiEngMode(ctx) };
                unsafe {
                    chewing_set_ChiEngMode(ctx, SYMBOL_MODE);
                }
                unsafe {
                    chewing_handle_KeyboardEvent(ctx, evt.code.0, evt.ksym.0, evt.state);
                    chewing_set_ChiEngMode(ctx, old_lang_mode);
                }
            } else {
                unsafe {
                    chewing_handle_KeyboardEvent(ctx, evt.code.0, evt.ksym.0, evt.state);
                }
            }
        } else {
            let mut key_handled = false;
            if self.cfg.borrow().chewing_tsf.cursor_cand_list
                && let Some(candidate_list) = &*self.candidate_list.borrow()
            {
                match candidate_list.filter_key_event(evt.ksym) {
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

            if !key_handled {
                unsafe {
                    chewing_handle_KeyboardEvent(ctx, evt.code.0, evt.ksym.0, evt.state);
                }
            }
        }

        if unsafe { chewing_keystroke_CheckIgnore(ctx) } == 1 {
            debug!("early return - chewing ignored key");
            return Ok(false);
        }

        // Not composing so we can commit the text immediately
        if !self.is_composing() && unsafe { chewing_commit_Check(ctx) } == 1 {
            let ptr = unsafe { chewing_commit_String(ctx) };
            let cstr = unsafe { CStr::from_ptr(ptr) };
            let text = cstr.to_string_lossy().into_owned();
            unsafe {
                chewing_free(ptr.cast());
                chewing_ack(ctx);
            }
            debug!(%text, "commit string");
            self.insert_text(context, &text)?;
            debug!("commit string ok");
            return Ok(true);
        }

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
            debug!(%text, "commit string");
            self.set_composition_string(context, &text, 0)?;
            self.end_composition(context)?;
            debug!("commit string ok");
        }

        self.update_preedit(context, ctx)?;

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

    pub(super) fn on_test_keyup(&self, context: &ITfContext, ev: KeyEvent) -> Result<bool> {
        self.on_keyup(context, ev)
    }

    pub(super) fn on_keyup(&self, context: &ITfContext, ev: KeyEvent) -> Result<bool> {
        let ctx = self.chewing_context;
        let evt = ev.to_keyboard_event(self.cfg.borrow().chewing_tsf.simulate_english_layout);
        let last_is_shift = evt.ksym == SYM_LEFTSHIFT || evt.ksym == SYM_RIGHTSHIFT;
        let last_is_capslock = evt.ksym == SYM_CAPSLOCK;

        debug!(last_is_shift, last_is_capslock);

        if last_is_shift
            && self.shift_key_state.borrow_mut().release()
                < Duration::from_millis(self.cfg.borrow().chewing_tsf.shift_key_sensitivity as u64)
            && self.cfg.borrow().chewing_tsf.switch_lang_with_shift
        {
            // TODO: simplify this
            if self.cfg.borrow().chewing_tsf.enable_caps_lock && evt.is_state_on(KeyState::CapsLock)
            {
                // Locked by CapsLock
                let msg = match self.lang_mode.get() {
                    SYMBOL_MODE => HSTRING::from("英數模式 (CapsLock)"),
                    CHINESE_MODE => HSTRING::from("中文模式"),
                    _ => unreachable!(),
                };
                if self.cfg.borrow().chewing_tsf.show_notification {
                    self.show_message(context, &msg, Duration::from_millis(500))?;
                }
            } else {
                self.toggle_lang_mode()?;
                let msg = match self.lang_mode.get() {
                    SYMBOL_MODE => HSTRING::from("英數模式"),
                    CHINESE_MODE
                        if self.cfg.borrow().chewing_tsf.enable_caps_lock
                            && evt.is_state_on(KeyState::CapsLock) =>
                    {
                        HSTRING::from("英數模式 (CapsLock)")
                    }
                    CHINESE_MODE => HSTRING::from("中文模式"),
                    _ => unreachable!(),
                };
                if self.cfg.borrow().chewing_tsf.show_notification {
                    self.show_message(context, &msg, Duration::from_millis(500))?;
                }
            }
        }

        if self.cfg.borrow().chewing_tsf.enable_caps_lock && last_is_capslock {
            self.sync_lang_mode()?;
            let msg = match unsafe { chewing_get_ChiEngMode(ctx) } {
                SYMBOL_MODE => HSTRING::from("英數模式 (CapsLock)"),
                CHINESE_MODE => HSTRING::from("中文模式"),
                _ => unreachable!(),
            };
            if self.cfg.borrow().chewing_tsf.show_notification {
                self.show_message(context, &msg, Duration::from_millis(500))?;
            }
        }

        // It is usually harmless to bubble up the keyup event but can be problematic if
        // keyup of a corresponding keydown doesn't match. Shortcut might be stuck, and
        // key repeat might not stop. So we always return `false` and handle keyup in
        // `on_test_keyup`.
        Ok(false)
    }

    pub(super) fn on_preserved_key(&self, guid: &GUID) -> bool {
        if guid == &GUID_CONTROL_F12 && self.toggle_simp_chinese().is_ok() {
            return true;
        }
        false
    }

    pub(super) fn on_composition_terminated(
        &self,
        ecwrite: u32,
        composition: &ITfComposition,
    ) -> Result<()> {
        if self.candidate_list.borrow().is_some() {
            self.hide_candidates();
        }
        let ctx = self.chewing_context;
        // commit current preedit
        {
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
            unsafe {
                let range = composition.GetRange()?;
                if let Err(error) = range.SetText(ecwrite, 0, &HSTRING::from(composition_buf)) {
                    error!("set composition string failed: {error}");
                }
            }
        }
        unsafe {
            chewing_cand_close(ctx);
            if chewing_bopomofo_Check(ctx) == 1 {
                chewing_clean_bopomofo_buf(ctx);
            }
            if chewing_buffer_Check(ctx) == 1 {
                chewing_clean_preedit_buf(ctx);
            }
        }
        self.pending_edit.replace(Weak::new());
        self.composition.replace(None);
        Ok(())
    }

    pub(super) fn on_compartment_change(&self, guid: &GUID) -> Result<()> {
        if guid == &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE {
            let compartment_mgr: ITfCompartmentMgr = self.thread_mgr.cast()?;
            unsafe {
                let thread_compartment =
                    compartment_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)?;
                let value = thread_compartment.GetValue()?;
                let openclose: i32 = (&value).try_into().unwrap_or_default();
                self.on_keyboard_status_changed(openclose != 0)?;
            }
        }
        Ok(())
    }

    fn on_keyboard_status_changed(&self, opened: bool) -> Result<()> {
        self.open.set(opened);
        if opened {
            self.lang_mode.set(CHINESE_MODE);
        } else {
            self.lang_mode.set(SYMBOL_MODE);
        }
        self.sync_lang_mode()?;
        Ok(())
    }

    pub(super) fn on_command(&self, id: u32, cmd_type: CommandType) {
        if matches!(cmd_type, CommandType::RightClick) {
            if id == ID_MODE_ICON {
                let mut pos = POINT::default();
                let ret = unsafe {
                    let _ = GetCursorPos(&mut pos);
                    TrackPopupMenu(
                        self.popup_menu,
                        TPM_NONOTIFY
                            | TPM_RETURNCMD
                            | TPM_LEFTALIGN
                            | TPM_BOTTOMALIGN
                            | TPM_LEFTBUTTON,
                        pos.x,
                        pos.y,
                        None,
                        GetFocus(),
                        None,
                    )
                };
                if ret.as_bool() {
                    self.on_command(ret.0 as u32, CommandType::Menu);
                } else {
                    let last_error = unsafe { GetLastError() };
                    let hresult = last_error.to_hresult();
                    error!("unable to open popup menu: {}", hresult.message());
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
                ID_HASHED => open_url("chewing-editor://open"),
                ID_CONFIG => open_url("chewing-preferences://config"),
                ID_OUTPUT_SIMP_CHINESE => {
                    if let Err(error) = self.toggle_simp_chinese() {
                        error!("unable to toggle simplified chinese: {error}");
                    }
                }
                ID_CHECK_NEW_VER => open_url(&self.cfg.borrow().chewing_tsf.update_info_url),
                ID_ABOUT => open_url("chewing-preferences://about"),
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

    fn update_preedit(&self, context: &ITfContext, ctx: *mut ChewingContext) -> Result<()> {
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
            if self.candidate_list.borrow().is_none() {
                self.end_composition(context)?;
            }
        }
        Ok(())
    }

    fn insert_text(&self, context: &ITfContext, text: &str) -> Result<()> {
        debug!(%text, "going to request immediate text insertion");
        let htext = text.into();
        let session = InsertText::new(context.clone(), htext).into_object();
        unsafe {
            match context.RequestEditSession(
                self.tid,
                session.as_interface(),
                TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
            ) {
                Err(error) => error!("failed to request edit session: {error}"),
                Ok(res) => {
                    if let Err(error) = res.ok() {
                        error!("failed to insert text: {error}")
                    }
                }
            }
        }
        Ok(())
    }

    fn end_composition(&self, context: &ITfContext) -> Result<()> {
        let Some(composition) = self.composition.take() else {
            return Ok(());
        };
        self.pending_edit.take();
        {
            let session = EndComposition::new(context.clone(), composition).into_object();
            debug!("end composition start");
            unsafe {
                context
                    .RequestEditSession(
                        self.tid,
                        session.as_interface(),
                        TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
                    )?
                    .ok()?;
            }
        }
        Ok(())
    }

    fn set_composition_string(&self, context: &ITfContext, text: &str, cursor: i32) -> Result<()> {
        debug!(%text, "set composition string");
        let htext = if self.output_simp_chinese.get() {
            zhconv(text, Variant::ZhHans).into()
        } else {
            text.into()
        };
        if let Some(cell) = self.pending_edit.borrow().upgrade() {
            debug!(cursor, %htext, "Reuse existing edit session");
            let mut pending = cell.borrow_mut();
            pending.text = htext;
            pending.cursor = cursor;
        } else {
            let pending = Rc::new(RefCell::new(CommitString {
                text: htext,
                cursor,
            }));
            let session = SetCompositionString::new(
                context.clone(),
                self.composition.clone(),
                self.composition_sink.clone(),
                self.input_da_atom.clone(),
                pending.clone(),
            )
            .into_object();
            self.pending_edit.replace(Rc::downgrade(&pending));
            unsafe {
                match context.RequestEditSession(
                    self.tid,
                    session.as_interface(),
                    TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
                ) {
                    Err(error) => error!("failed to request edit session: {error}"),
                    Ok(res) => {
                        if let Err(error) = res.ok() {
                            error!("failed to set composition: {error}")
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn get_selection_rect(&self, context: &ITfContext) -> Result<RECT> {
        let session = SelectionRect::new(context.clone()).into_object();
        unsafe {
            context
                .RequestEditSession(self.tid, session.as_interface(), TF_ES_SYNC | TF_ES_READ)?
                .ok()?;
        }
        Ok(session.rect())
    }

    fn show_message(&self, context: &ITfContext, text: &HSTRING, dur: Duration) -> Result<()> {
        let hwnd = unsafe {
            let view = context.GetActiveView()?;
            // UILess console may not have valid HWND
            view.GetWnd().unwrap_or_default()
        };
        let notification = Notification::new(hwnd, self.thread_mgr.clone())?;
        notification.set_model(NotificationModel {
            text: text.clone(),
            font_family: HSTRING::from(&self.cfg.borrow().chewing_tsf.font_family),
            font_size: self.cfg.borrow().chewing_tsf.font_size as f32,
        });
        if let Ok(rect) = self.get_selection_rect(context) {
            notification.set_position(rect.left + 50, rect.bottom + 50);
            // HACK set position again to use correct DPI setting
            notification.set_position(rect.left + 50, rect.bottom + 50);
        }
        notification.show();
        notification.set_timer(dur);
        self.notification.replace(Some(notification));
        Ok(())
    }

    fn hide_message(&self) {
        if let Some(notification) = self.notification.take() {
            notification.set_timer(Duration::ZERO);
            notification.end_ui_element();
        }
    }

    fn update_candidates(&self, context: &ITfContext) -> Result<()> {
        let ctx = self.chewing_context;
        if unsafe { chewing_cand_TotalChoice(ctx) } == 0 {
            self.hide_candidates();
            return Ok(());
        }
        if self.candidate_list.borrow().is_none() {
            let view = unsafe { context.GetActiveView()? };
            // UILess console may not have valid HWND
            let hwnd = unsafe { view.GetWnd().unwrap_or_default() };
            let candidate_list = CandidateList::new(hwnd, self.thread_mgr.clone())?;
            self.candidate_list.replace(Some(candidate_list));
        }

        if let Some(candidate_list) = &*self.candidate_list.borrow() {
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
                let total_page = chewing_cand_TotalPage(ctx) as u32;
                let current_page = chewing_cand_CurrentPage(ctx) as u32 + 1;
                candidate_list.set_model(Model {
                    items,
                    selkeys: sel_keys.iter().take(n).map(|&k| k as u16).collect(),
                    cand_per_row: self.cfg.borrow().chewing_tsf.cand_per_row as u32,
                    total_page,
                    current_page,
                    font_family: HSTRING::from(&self.cfg.borrow().chewing_tsf.font_family),
                    font_size: self.cfg.borrow().chewing_tsf.font_size as f32,
                    fg_color: color_s(&self.cfg.borrow().chewing_tsf.font_fg_color),
                    bg_color: color_s(&self.cfg.borrow().chewing_tsf.font_bg_color),
                    highlight_fg_color: color_s(
                        &self.cfg.borrow().chewing_tsf.font_highlight_fg_color,
                    ),
                    highlight_bg_color: color_s(
                        &self.cfg.borrow().chewing_tsf.font_highlight_bg_color,
                    ),
                    selkey_color: color_s(&self.cfg.borrow().chewing_tsf.font_number_fg_color),
                    use_cursor: self.cfg.borrow().chewing_tsf.cursor_cand_list,
                    current_sel: 0,
                });
            }

            candidate_list.show();

            if let Ok(rect) = self.get_selection_rect(context) {
                candidate_list.set_position(rect.left, rect.bottom);
                // HACK set position again to use correct DPI setting
                candidate_list.set_position(rect.left, rect.bottom);
            }
        }

        Ok(())
    }

    fn hide_candidates(&self) {
        if let Some(candidate_list) = self.candidate_list.take() {
            candidate_list.end_ui_element();
        }
    }

    fn toggle_simp_chinese(&self) -> Result<()> {
        self.output_simp_chinese.update(|v| !v);
        debug!(
            "toggle output simplified chinese: {}",
            self.output_simp_chinese.get()
        );
        let check_flag = if self.output_simp_chinese.get() {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        unsafe {
            CheckMenuItem(self.popup_menu, ID_OUTPUT_SIMP_CHINESE, check_flag.0);
        }
        self.update_lang_buttons()?;
        Ok(())
    }

    fn toggle_shape_mode(&self) -> Result<()> {
        let ctx = self.chewing_context;
        unsafe {
            chewing_set_ShapeMode(
                ctx,
                match chewing_get_ShapeMode(ctx) {
                    0 => 1,
                    _ => 0,
                },
            );
        }
        let check_flag = if unsafe { chewing_get_ShapeMode(ctx) == 1 } {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        unsafe {
            CheckMenuItem(self.popup_menu, ID_SWITCH_SHAPE, check_flag.0);
        }
        self.update_lang_buttons()?;

        Ok(())
    }

    fn toggle_hsu_keyboard(&self, context: &ITfContext) -> Result<()> {
        let ctx = self.chewing_context;
        unsafe {
            let kbtype = chewing_get_KBType(ctx);
            if kbtype == KB::Hsu as i32 {
                chewing_set_KBType(ctx, KB::Default as i32);
                self.show_message(
                    context,
                    &HSTRING::from("標準鍵盤"),
                    Duration::from_millis(500),
                )?;
            } else {
                chewing_set_KBType(ctx, KB::Hsu as i32);
                self.show_message(
                    context,
                    &HSTRING::from("許氏鍵盤"),
                    Duration::from_millis(500),
                )?;
            }
        }
        Ok(())
    }

    fn sync_lang_mode(&self) -> Result<()> {
        let ctx = self.chewing_context;
        let evt = KeyEvent::default()
            .to_keyboard_event(self.cfg.borrow().chewing_tsf.simulate_english_layout);
        if self.cfg.borrow().chewing_tsf.enable_caps_lock {
            if evt.is_state_on(KeyState::CapsLock) {
                self.lang_mode.set(SYMBOL_MODE);
            } else {
                self.lang_mode.set(CHINESE_MODE);
            }
        }
        unsafe {
            chewing_set_ChiEngMode(ctx, self.lang_mode.get());
        }
        self.update_lang_buttons()?;

        // The OpenClose compartment is not synced when CapsLock English mode is enabled
        if !self.cfg.borrow().chewing_tsf.enable_caps_lock {
            let compartment_mgr: ITfCompartmentMgr = self.thread_mgr.cast()?;
            unsafe {
                let compartment =
                    compartment_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)?;
                let openclose: i32 = match self.lang_mode.get() {
                    CHINESE_MODE => 1,
                    SYMBOL_MODE => 0,
                    _ => unreachable!(),
                };
                let old_openclose = i32::try_from(&compartment.GetValue()?)?;
                if openclose != old_openclose {
                    let _ = compartment.SetValue(self.tid, &openclose.into());
                }
            }
        }
        Ok(())
    }

    fn toggle_lang_mode(&self) -> Result<()> {
        self.lang_mode.update(|v| match v {
            SYMBOL_MODE => CHINESE_MODE,
            CHINESE_MODE => SYMBOL_MODE,
            _ => unreachable!(),
        });
        self.sync_lang_mode()?;

        Ok(())
    }

    fn get_lang_icon_id(&self) -> u32 {
        let mut icon_id = match (ThemeDetector::detect_theme(), self.lang_mode.get()) {
            (WindowsTheme::Light, CHINESE_MODE) => IDI_CHI,
            (WindowsTheme::Light, SYMBOL_MODE) => IDI_ENG,
            (WindowsTheme::Dark, CHINESE_MODE) => IDI_CHI_DARK,
            (WindowsTheme::Dark, SYMBOL_MODE) => IDI_ENG_DARK,
            _ => IDI_CHI,
        };
        if self.output_simp_chinese.get() {
            icon_id = match icon_id {
                IDI_CHI => IDI_SIMP,
                IDI_CHI_DARK => IDI_SIMP_DARK,
                _ => icon_id,
            }
        }
        let show_dot = !self.cfg.borrow().chewing_tsf.update_info_url.is_empty();
        match (icon_id, show_dot) {
            (IDI_CHI, true) => IDI_CHI_DOT,
            (IDI_CHI_DARK, true) => IDI_CHI_DARK_DOT,
            (IDI_ENG, true) => IDI_ENG_DOT,
            (IDI_ENG_DARK, true) => IDI_ENG_DARK_DOT,
            (IDI_SIMP, true) => IDI_SIMP_DOT,
            (IDI_SIMP_DARK, true) => IDI_SIMP_DARK_DOT,
            _ => icon_id,
        }
    }

    fn is_composing(&self) -> bool {
        // when candidate window is shown we are composing even without a composition
        self.composition.borrow().is_some() || self.candidate_list.borrow().is_some()
    }

    fn init_chewing_context(&mut self) -> anyhow::Result<()> {
        debug_assert!(self.chewing_context.is_null());
        if let Err(error) = init_chewing_env() {
            error!("unable to init chewing env, init may fail: {error}");
        }
        let ctx = chewing_new();
        if ctx.is_null() {
            bail!("chewing context is null");
        }
        self.chewing_context = ctx;

        self.apply_config();

        unsafe {
            chewing_set_maxChiSymbolLen(ctx, 50);
        }
        self.lang_mode
            .set(if self.cfg.borrow().chewing_tsf.default_english {
                SYMBOL_MODE
            } else {
                CHINESE_MODE
            });
        self.sync_lang_mode()?;
        if self.cfg.borrow().chewing_tsf.default_full_space {
            unsafe {
                chewing_set_ShapeMode(ctx, FULLSHAPE_MODE);
            }
        }
        Ok(())
    }

    fn apply_config_if_changed(&self) -> anyhow::Result<()> {
        if self.cfg.borrow_mut().reload_if_needed()? {
            self.apply_config();
        }
        Ok(())
    }

    fn apply_config(&self) {
        let cfg = &self.cfg.borrow().chewing_tsf;
        let ctx = self.chewing_context;
        unsafe {
            if cfg.easy_symbols_with_shift || cfg.easy_symbols_with_shift_ctrl {
                chewing_set_easySymbolInput(ctx, 1);
            } else {
                chewing_set_easySymbolInput(ctx, 0);
            }
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
            chewing_config_set_int(
                ctx,
                c"chewing.enable_fullwidth_toggle_key".as_ptr(),
                cfg.enable_fullwidth_toggle_key as i32,
            );
            chewing_config_set_int(
                ctx,
                c"chewing.sort_candidates_by_frequency".as_ptr(),
                cfg.sort_candidates_by_frequency as i32,
            );
        }
        self.output_simp_chinese.set(cfg.output_simp_chinese);
        let check_flag = if self.output_simp_chinese.get() {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        unsafe {
            CheckMenuItem(self.popup_menu, ID_OUTPUT_SIMP_CHINESE, check_flag.0);
        }
        let _ = self.update_lang_buttons();
        let keybindings = cfg
            .keybind
            .iter()
            .filter_map(|kb| Keybinding::try_from(kb).ok())
            .collect();
        self.keybindings.replace(keybindings);
    }

    fn update_lang_buttons(&self) -> Result<()> {
        let ctx = self.chewing_context;
        let g_hinstance = HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut c_void);
        let icon_id = self.get_lang_icon_id();
        unsafe {
            self.switch_lang_button.set_icon(LoadIconW(
                Some(g_hinstance),
                PCWSTR::from_raw(icon_id as *const u16),
            )?)?;
            self.ime_mode_button.set_icon(LoadIconW(
                Some(g_hinstance),
                PCWSTR::from_raw(icon_id as *const u16),
            )?)?;
            if self.cfg.borrow().chewing_tsf.enable_caps_lock {
                let _ = self.ime_mode_button.set_enabled(self.open.get());
            }
        }
        // TODO extract shape mode change to dedicated method
        let shape_mode = unsafe { chewing_get_ShapeMode(ctx) };
        unsafe {
            self.switch_shape_button.set_icon(LoadIconW(
                Some(g_hinstance),
                PCWSTR::from_raw(if shape_mode == FULLSHAPE_MODE {
                    IDI_FULL_SHAPE as *const u16
                } else {
                    IDI_HALF_SHAPE as *const u16
                }),
            )?)?;
        }
        unsafe {
            CheckMenuItem(
                self.popup_menu,
                ID_SWITCH_SHAPE,
                if shape_mode == FULLSHAPE_MODE {
                    MF_CHECKED.0
                } else {
                    MF_UNCHECKED.0
                },
            );
        }
        unsafe {
            let _ = EnableMenuItem(
                self.popup_menu,
                ID_CHECK_NEW_VER,
                match self.cfg.borrow().chewing_tsf.update_info_url.as_str() {
                    "" => MF_GRAYED,
                    _ => MF_ENABLED,
                },
            );
        }
        Ok(())
    }

    fn remove_buttons(&mut self) -> Result<()> {
        let lang_bar_item_mgr: ITfLangBarItemMgr = self.thread_mgr.cast()?;
        for button in self.lang_bar_buttons.drain(0..) {
            if let Err(error) = unsafe { lang_bar_item_mgr.RemoveItem(&button) } {
                error!("unable to remove lang bar item: {error}");
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
    if let Ok(uri) = Uri::CreateUri(&url.into()) {
        let _ = Launcher::LaunchUriAsync(&uri);
    }
}
