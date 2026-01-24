// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::io::ErrorKind;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use chewing::conversion::{ChewingEngine, FuzzyChewingEngine, SimpleEngine};
use chewing::dictionary::{DEFAULT_DICT_NAMES, LookupStrategy};
use chewing::editor::zhuyin_layout::{self, KeyboardLayoutCompat, SyllableEditor};
use chewing::editor::{
    BasicEditor, CharacterForm, ConversionEngineKind, Editor, EditorKeyBehavior, LanguageMode,
    UserPhraseAddDirection,
};
use chewing::input::keycode::Keycode;
use chewing::input::keysym::{Keysym, SYM_CAPSLOCK, SYM_LEFTSHIFT, SYM_RIGHTSHIFT, SYM_SPACE};
use chewing::input::{KeyState, KeyboardEvent, keycode, keysym};
use log::{debug, error, info};
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

const SEL_KEYS: [&'static str; 6] = [
    "1234567890",
    "asdfghjkl;",
    "asdfzxcv89",
    "asdfjkl789",
    "aoeuhtn789",
    "1234qweras",
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
    pub(super) cursor: usize,
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
    lang_mode: Cell<LanguageMode>,
    output_simp_chinese: Cell<bool>,
    open: Cell<bool>,
    shift_key_state: RefCell<ShiftKeyState>,
    cfg: RefCell<Config>,
    kbtype: Cell<KeyboardLayoutCompat>,
    keybindings: RefCell<Vec<Keybinding>>,
    chewing_editor: RefCell<Editor>,
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

        // Initialize a temp editor, this will be replaced in init_chewing_context.
        let editor = Editor::chewing(None, None, DEFAULT_DICT_NAMES);

        let mut cts = ChewingTextService {
            thread_mgr,
            tid,
            composition_sink: ts.cast()?,
            input_da_atom,
            _menu: menu,
            popup_menu,
            lang_mode: Cell::new(LanguageMode::English),
            open: Cell::new(true),
            output_simp_chinese: Default::default(),
            shift_key_state: RefCell::new(ShiftKeyState::Up),
            cfg: RefCell::new(cfg),
            kbtype: Cell::new(KeyboardLayoutCompat::Default),
            keybindings: RefCell::new(vec![]),
            chewing_editor: RefCell::new(editor),
            lang_bar_buttons,
            switch_lang_button,
            switch_shape_button,
            ime_mode_button,
            notification: Default::default(),
            candidate_list: Default::default(),
            composition: Default::default(),
            pending_edit: RefCell::new(Weak::new()),
        };

        if let Err(error) = cts.init_chewing_context() {
            error!("unable to initialize chewing: {error:#}");
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
        debug!(evt:?, shift_key_state:? = self.shift_key_state.borrow(); "on_test_keydown");
        //
        // Step 1. apply any config changes
        //
        if let Err(error) = self.apply_config_if_changed() {
            error!("unable to load config: {error:#}");
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
            let shape_mode = self.chewing_editor.borrow().editor_options().character_form;
            // don't do further handling in pure English + half shape mode
            if self.lang_mode.get() == LanguageMode::English
                && shape_mode == CharacterForm::Halfwidth
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
            if evt.ksym == SYM_SPACE
                && shape_mode != CharacterForm::Fullwidth
                && !evt.is_state_on(KeyState::Shift)
            {
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
        debug!(evt:?; "on_keydown");

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

        if evt.ksym.is_unicode() {
            let mut momentary_english_mode = false;
            let mut upper_case = false;
            if evt.is_state_on(KeyState::Shift) {
                upper_case = true;
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
                    upper_case = false;
                }
            }
            evt.ksym = if evt.ksym.is_ascii() {
                let code = evt.ksym.to_unicode();
                if upper_case {
                    Keysym::from(code.to_ascii_uppercase())
                } else {
                    Keysym::from(code.to_ascii_lowercase())
                }
            } else {
                evt.ksym
            };
            // HACK: convert sel_keys key to number key
            if self.chewing_editor.borrow().is_selecting() {
                evt = self.map_sel_key(evt);
            }
            if evt.ksym == SYM_SPACE && evt.is_state_on(KeyState::Shift) {
                // TODO: maybe this can be merged back to the default branch?
                self.chewing_editor.borrow_mut().process_keyevent(evt);
            } else if self.lang_mode.get() == LanguageMode::English || momentary_english_mode {
                let old_lang_mode = self.chewing_editor.borrow().editor_options().language_mode;
                self.chewing_editor
                    .borrow_mut()
                    .set_editor_options(|opt| opt.language_mode = LanguageMode::English);
                self.chewing_editor.borrow_mut().process_keyevent(evt);
                self.chewing_editor
                    .borrow_mut()
                    .set_editor_options(|opt| opt.language_mode = old_lang_mode);
            } else {
                self.chewing_editor.borrow_mut().process_keyevent(evt);
            }
        } else {
            let mut key_handled = false;
            if self.cfg.borrow().chewing_tsf.cursor_cand_list
                && let Some(candidate_list) = &*self.candidate_list.borrow()
            {
                match candidate_list.filter_key_event(evt.ksym) {
                    FilterKeyResult::HandledCommit => {
                        let sel_key = candidate_list.current_sel();
                        self.chewing_editor.borrow_mut().select(sel_key)?;
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
                self.chewing_editor.borrow_mut().process_keyevent(evt);
            }
        }

        let last_behavior = self.chewing_editor.borrow().last_key_behavior();

        if last_behavior == EditorKeyBehavior::Ignore {
            debug!("early return - chewing ignored key");
            return Ok(false);
        }

        // Not composing so we can commit the text immediately
        if !self.is_composing() && last_behavior == EditorKeyBehavior::Commit {
            let text = self.chewing_editor.borrow().display_commit().to_owned();
            self.chewing_editor.borrow_mut().ack();
            debug!(text; "commit string");
            self.insert_text(context, &text)?;
            debug!("commit string ok");
            return Ok(true);
        }

        self.update_candidates(context)?;

        debug!("updated candidates");

        if last_behavior == EditorKeyBehavior::Commit {
            let text = self.chewing_editor.borrow().display_commit().to_owned();
            self.chewing_editor.borrow_mut().ack();
            debug!(text; "commit string");
            self.set_composition_string(context, &text, 0)?;
            self.end_composition(context)?;
            debug!("commit string ok");
        }

        self.update_preedit(context)?;

        if !self.chewing_editor.borrow().notification().is_empty() {
            self.show_message(
                context,
                &HSTRING::from(self.chewing_editor.borrow().notification()),
                Duration::from_millis(500),
            )?;
        }

        Ok(true)
    }

    pub(super) fn on_test_keyup(&self, context: &ITfContext, ev: KeyEvent) -> Result<bool> {
        self.on_keyup(context, ev)
    }

    pub(super) fn on_keyup(&self, context: &ITfContext, ev: KeyEvent) -> Result<bool> {
        let evt = ev.to_keyboard_event(self.cfg.borrow().chewing_tsf.simulate_english_layout);
        let last_is_shift = evt.ksym == SYM_LEFTSHIFT || evt.ksym == SYM_RIGHTSHIFT;
        let last_is_capslock = evt.ksym == SYM_CAPSLOCK;
        let capslock_enabled_and_keyboard_closed =
            self.cfg.borrow().chewing_tsf.enable_caps_lock && !self.open.get();

        debug!(last_is_shift, last_is_capslock; "");

        if last_is_shift
            && self.shift_key_state.borrow_mut().release()
                < Duration::from_millis(self.cfg.borrow().chewing_tsf.shift_key_sensitivity as u64)
            && self.cfg.borrow().chewing_tsf.switch_lang_with_shift
            && !capslock_enabled_and_keyboard_closed
        {
            // TODO: simplify this
            if self.cfg.borrow().chewing_tsf.enable_caps_lock && evt.is_state_on(KeyState::CapsLock)
            {
                // Locked by CapsLock
                let msg = match self.lang_mode.get() {
                    LanguageMode::English => HSTRING::from("英數模式 (CapsLock)"),
                    LanguageMode::Chinese => HSTRING::from("中文模式"),
                };
                if self.cfg.borrow().chewing_tsf.show_notification {
                    self.show_message(context, &msg, Duration::from_millis(500))?;
                }
            } else {
                self.toggle_lang_mode()?;
                let msg = match self.lang_mode.get() {
                    LanguageMode::English => HSTRING::from("英數模式"),
                    LanguageMode::Chinese
                        if self.cfg.borrow().chewing_tsf.enable_caps_lock
                            && evt.is_state_on(KeyState::CapsLock) =>
                    {
                        HSTRING::from("英數模式 (CapsLock)")
                    }
                    LanguageMode::Chinese => HSTRING::from("中文模式"),
                };
                if self.cfg.borrow().chewing_tsf.show_notification {
                    self.show_message(context, &msg, Duration::from_millis(500))?;
                }
            }
        }

        if self.cfg.borrow().chewing_tsf.enable_caps_lock
            && last_is_capslock
            && !capslock_enabled_and_keyboard_closed
        {
            self.sync_lang_mode()?;
            let msg = match self.chewing_editor.borrow().editor_options().language_mode {
                LanguageMode::English => HSTRING::from("英數模式 (CapsLock)"),
                LanguageMode::Chinese => HSTRING::from("中文模式"),
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

        // commit current preedit
        unsafe {
            let doc_mgr = self
                .thread_mgr
                .GetFocus()
                .context("failed to get current ITfDocumentMgr")?;
            let context = doc_mgr
                .GetTop()
                .context("failed to get current ITfContext")?;
            EndComposition::will_end_composition(&context, composition, ecwrite)?;
        }
        let mut editor = self.chewing_editor.borrow_mut();
        if editor.is_selecting() {
            let _ = editor.cancel_selecting();
        }
        editor.clear_syllable_editor();
        editor.clear_composition_editor();
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
            self.lang_mode.set(LanguageMode::Chinese);
        } else {
            self.lang_mode.set(LanguageMode::English);
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

    fn update_preedit(&self, context: &ITfContext) -> Result<()> {
        let mut composition_buf = String::new();
        composition_buf.push_str(&self.chewing_editor.borrow().display());

        let cursor = self.chewing_editor.borrow().cursor();
        let bopomofo = self.chewing_editor.borrow().syllable_buffer_display();
        if !bopomofo.is_empty() {
            let idx = composition_buf
                .char_indices()
                .nth(cursor)
                .map(|pair| pair.0)
                .unwrap_or(composition_buf.len());
            composition_buf.insert_str(idx, &bopomofo);
        }

        // has something in composition buffer
        if !composition_buf.is_empty() {
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
        debug!(text; "going to request immediate text insertion");
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

    fn set_composition_string(
        &self,
        context: &ITfContext,
        text: &str,
        cursor: usize,
    ) -> Result<()> {
        debug!(text; "set composition string");
        let htext = if self.output_simp_chinese.get() {
            zhconv(text, Variant::ZhHans).into()
        } else {
            text.into()
        };
        if let Some(cell) = self.pending_edit.borrow().upgrade() {
            debug!(cursor, htext:%; "Reuse existing edit session");
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
        let editor = self.chewing_editor.borrow();
        if !editor.is_selecting() {
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
            let cfg = &self.cfg.borrow().chewing_tsf;
            let sel_keys = SEL_KEYS[cfg.sel_key_type as usize];
            let n = editor.editor_options().candidates_per_page;
            let total_page = editor.total_page()? as u32;
            let current_page = editor.current_page_no()? as u32 + 1;
            let mut items = editor.paginated_candidates()?;
            items.truncate(n);
            candidate_list.set_model(Model {
                items,
                selkeys: sel_keys.chars().take(n).map(|k| k as u16).collect(),
                cand_per_row: cfg.cand_per_row as u32,
                total_page,
                current_page,
                font_family: HSTRING::from(&cfg.font_family),
                font_size: cfg.font_size as f32,
                fg_color: color_s(&cfg.font_fg_color),
                bg_color: color_s(&cfg.font_bg_color),
                highlight_fg_color: color_s(&cfg.font_highlight_fg_color),
                highlight_bg_color: color_s(&cfg.font_highlight_bg_color),
                selkey_color: color_s(&cfg.font_number_fg_color),
                use_cursor: cfg.cursor_cand_list,
                current_sel: 0,
            });

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
        self.chewing_editor.borrow_mut().set_editor_options(|opt| {
            opt.character_form = match opt.character_form {
                CharacterForm::Fullwidth => CharacterForm::Halfwidth,
                CharacterForm::Halfwidth => CharacterForm::Fullwidth,
            }
        });
        let check_flag = match self.chewing_editor.borrow().editor_options().character_form {
            CharacterForm::Fullwidth => MF_CHECKED,
            CharacterForm::Halfwidth => MF_UNCHECKED,
        };
        unsafe {
            CheckMenuItem(self.popup_menu, ID_SWITCH_SHAPE, check_flag.0);
        }
        self.update_lang_buttons()?;

        Ok(())
    }

    fn toggle_hsu_keyboard(&self, context: &ITfContext) -> Result<()> {
        if self.kbtype.get() == KeyboardLayoutCompat::Hsu {
            self.kbtype.set(KeyboardLayoutCompat::Default);
            self.chewing_editor
                .borrow_mut()
                .set_syllable_editor(syl_editor_from_kbtype(KeyboardLayoutCompat::Default));
            self.show_message(
                context,
                &HSTRING::from("標準鍵盤"),
                Duration::from_millis(500),
            )?;
        } else {
            self.kbtype.set(KeyboardLayoutCompat::Hsu);
            self.chewing_editor
                .borrow_mut()
                .set_syllable_editor(syl_editor_from_kbtype(KeyboardLayoutCompat::Hsu));
            self.show_message(
                context,
                &HSTRING::from("許氏鍵盤"),
                Duration::from_millis(500),
            )?;
        }
        Ok(())
    }

    fn sync_lang_mode(&self) -> Result<()> {
        let evt = KeyEvent::default()
            .to_keyboard_event(self.cfg.borrow().chewing_tsf.simulate_english_layout);
        if self.cfg.borrow().chewing_tsf.enable_caps_lock {
            if evt.is_state_on(KeyState::CapsLock) {
                self.lang_mode.set(LanguageMode::English);
            } else {
                self.lang_mode.set(LanguageMode::Chinese);
            }
        }

        self.chewing_editor
            .borrow_mut()
            .set_editor_options(|opt| opt.language_mode = self.lang_mode.get());

        self.update_lang_buttons()?;

        // The OpenClose compartment is not synced when CapsLock English mode is enabled
        if !self.cfg.borrow().chewing_tsf.enable_caps_lock {
            let compartment_mgr: ITfCompartmentMgr = self.thread_mgr.cast()?;
            unsafe {
                let compartment =
                    compartment_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)?;
                let openclose: i32 = match self.lang_mode.get() {
                    LanguageMode::Chinese => 1,
                    LanguageMode::English => 0,
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
            LanguageMode::English => LanguageMode::Chinese,
            LanguageMode::Chinese => LanguageMode::English,
        });
        self.sync_lang_mode()?;

        Ok(())
    }

    fn get_lang_icon_id(&self) -> u32 {
        let mut icon_id = match (ThemeDetector::detect_theme(), self.lang_mode.get()) {
            (WindowsTheme::Light, LanguageMode::Chinese) => IDI_CHI,
            (WindowsTheme::Light, LanguageMode::English) => IDI_ENG,
            (WindowsTheme::Dark, LanguageMode::Chinese) => IDI_CHI_DARK,
            (WindowsTheme::Dark, LanguageMode::English) => IDI_ENG_DARK,
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

    fn init_chewing_context(&mut self) -> Result<()> {
        self.apply_config()?;

        self.chewing_editor.get_mut().set_editor_options(|opt| {
            if self.cfg.borrow().chewing_tsf.default_full_space {
                opt.character_form = CharacterForm::Fullwidth;
            }
            opt.auto_commit_threshold = 50;
        });

        self.lang_mode
            .set(if self.cfg.borrow().chewing_tsf.default_english {
                LanguageMode::English
            } else {
                LanguageMode::Chinese
            });

        self.sync_lang_mode()?;

        Ok(())
    }

    fn apply_config_if_changed(&self) -> Result<()> {
        if self.cfg.borrow_mut().reload_if_needed()? {
            self.apply_config()?;
        }
        Ok(())
    }

    fn apply_config(&self) -> Result<()> {
        let cfg = &self.cfg.borrow().chewing_tsf;
        {
            let user_path = user_dir()?;
            let chewing_path = format!(
                "{};{}",
                user_path.display(),
                program_dir()?.join("Dictionary").display()
            );
            let user_dict_path = user_path.join("chewing.dat");
            // Recreate editor to load latest user files
            self.chewing_editor.replace(Editor::chewing(
                Some(chewing_path),
                // NB: the current API requires a *file* path
                Some(user_dict_path.to_string_lossy().into_owned()),
                &DEFAULT_DICT_NAMES,
            ));
            let mut editor = self.chewing_editor.borrow_mut();
            editor.set_editor_options(|opt| {
                opt.easy_symbol_input =
                    cfg.easy_symbols_with_shift || cfg.easy_symbols_with_shift_ctrl;
                // NB: Historically the config was inverted
                opt.user_phrase_add_dir = if cfg.add_phrase_forward {
                    UserPhraseAddDirection::Backward
                } else {
                    UserPhraseAddDirection::Forward
                };
                opt.phrase_choice_rearward = cfg.phrase_choice_rearward;
                opt.auto_shift_cursor = cfg.advance_after_selection;
                opt.candidates_per_page = cfg.cand_per_page as usize;
                opt.esc_clear_all_buffer = cfg.esc_clean_all_buf;
                opt.space_is_select_key = cfg.show_cand_with_space_key;
                opt.disable_auto_learn_phrase = !cfg.enable_auto_learn;
                opt.enable_fullwidth_toggle_key = cfg.enable_fullwidth_toggle_key;
                opt.sort_candidates_by_frequency = cfg.sort_candidates_by_frequency;
                // FIXME
                opt.conversion_engine = match cfg.conv_engine {
                    0 => ConversionEngineKind::SimpleEngine,
                    2 => ConversionEngineKind::FuzzyChewingEngine,
                    1 | _ => ConversionEngineKind::ChewingEngine,
                };
                // FIXME
                opt.lookup_strategy = match cfg.conv_engine {
                    0 => LookupStrategy::Standard,
                    2 => LookupStrategy::FuzzyPartialPrefix,
                    1 | _ => LookupStrategy::Standard,
                };
            });
            self.kbtype.set(
                KeyboardLayoutCompat::try_from(cfg.keyboard_layout as u8)
                    .unwrap_or(KeyboardLayoutCompat::Default),
            );
            editor.set_syllable_editor(syl_editor_from_kbtype(self.kbtype.get()));
            // FIXME
            match editor.editor_options().conversion_engine {
                ConversionEngineKind::SimpleEngine => {
                    editor.set_conversion_engine(Box::new(SimpleEngine::new()));
                }
                ConversionEngineKind::ChewingEngine => {
                    editor.set_conversion_engine(Box::new(ChewingEngine::new()));
                }
                ConversionEngineKind::FuzzyChewingEngine => {
                    editor.set_conversion_engine(Box::new(FuzzyChewingEngine::new()));
                }
            }
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
        Ok(())
    }

    fn update_lang_buttons(&self) -> Result<()> {
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
        let shape_mode = self.chewing_editor.borrow().editor_options().character_form;
        unsafe {
            self.switch_shape_button.set_icon(LoadIconW(
                Some(g_hinstance),
                PCWSTR::from_raw(if shape_mode == CharacterForm::Fullwidth {
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
                if shape_mode == CharacterForm::Fullwidth {
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

    fn map_sel_key(&self, mut evt: KeyboardEvent) -> KeyboardEvent {
        if let Some(idx) = SEL_KEYS[self.cfg.borrow().chewing_tsf.sel_key_type as usize]
            .chars()
            .position(|it| it == evt.ksym.to_unicode())
        {
            match idx {
                0..9 => {
                    evt.code = Keycode(keycode::KEY_1.0 + idx as u8);
                    evt.ksym = Keysym(keysym::SYM_1.0 + idx as u32);
                }
                9 | _ => {
                    evt.code = keycode::KEY_0;
                    evt.ksym = keysym::SYM_0;
                }
            }
        };
        evt
    }
}

fn syl_editor_from_kbtype(kbtype: KeyboardLayoutCompat) -> Box<dyn SyllableEditor> {
    use zhuyin_layout::*;
    match kbtype {
        KeyboardLayoutCompat::Default => Box::new(Standard::new()),
        KeyboardLayoutCompat::Hsu => Box::new(Hsu::new()),
        KeyboardLayoutCompat::Ibm => Box::new(Ibm::new()),
        KeyboardLayoutCompat::GinYieh => Box::new(GinYieh::new()),
        KeyboardLayoutCompat::Et => Box::new(Et::new()),
        KeyboardLayoutCompat::Et26 => Box::new(Et26::new()),
        KeyboardLayoutCompat::Dvorak => Box::new(Standard::new()),
        KeyboardLayoutCompat::DvorakHsu => Box::new(Hsu::new()),
        KeyboardLayoutCompat::DachenCp26 => Box::new(DaiChien26::new()),
        KeyboardLayoutCompat::HanyuPinyin => Box::new(Pinyin::hanyu()),
        KeyboardLayoutCompat::ThlPinyin => Box::new(Pinyin::thl()),
        KeyboardLayoutCompat::Mps2Pinyin => Box::new(Pinyin::mps2()),
        KeyboardLayoutCompat::Carpalx
        | KeyboardLayoutCompat::ColemakDhAnsi
        | KeyboardLayoutCompat::ColemakDhOrth
        | KeyboardLayoutCompat::Workman
        | KeyboardLayoutCompat::Colemak => Box::new(Standard::new()),
    }
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
            .or_else(|_| std::env::var("ProgramFiles(x86)"))?,
    )
    .join("ChewingTextService"))
}

fn open_url(url: &str) {
    if let Ok(uri) = Uri::CreateUri(&url.into()) {
        let _ = Launcher::LaunchUriAsync(&uri);
    }
}
