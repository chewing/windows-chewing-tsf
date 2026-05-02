use chewing::{
    conversion::{ChewingEngine, FuzzyChewingEngine, SimpleEngine},
    dictionary::{DEFAULT_DICT_NAMES, LookupStrategy},
    editor::{
        CharacterForm, ConversionEngineKind, Editor, LanguageMode, UserPhraseAddDirection,
        zhuyin_layout::{self, KeyboardLayoutCompat, SyllableEditor},
    },
    input::{
        KeyState,
        keysym::{SYM_CAPSLOCK, SYM_LEFTSHIFT, SYM_RIGHTSHIFT, SYM_SPACE},
    },
};
use chewing_tip_core::{
    config::{ChewingTsfConfig, Config},
    impl_context_error,
    ipc::values::IpcShiftKeyState,
    result::{Report, expect_error},
    shell::{program_dir, user_dir},
};

use crate::text_service::{keybind::Keybinding, keyevent::SystemKeyboardEvent};

#[derive(Debug)]
pub(crate) struct TipSession {
    // FIXME: use global override
    output_simp_chinese: bool,
    // FIXME: use global cfg
    cfg: Config,
    lang_mode: TsfLangMode,
    kbtype: KeyboardLayoutCompat,
    keybindings: Vec<Keybinding>,
    chewing_editor: Editor,
}

impl TipSession {
    pub(crate) fn new() -> TipSession {
        let cfg = Config::from_reg().unwrap_or_else(|error| {
            log::error!("Failed to load config from registry: {}", Report(&error));
            log::error!("Fallback to default config");
            Config::default()
        });
        let editor = Editor::chewing(None, None, DEFAULT_DICT_NAMES);
        TipSession {
            output_simp_chinese: cfg.chewing_tsf.output_simp_chinese,
            cfg,
            lang_mode: TsfLangMode::English,
            kbtype: KeyboardLayoutCompat::Default,
            keybindings: vec![],
            chewing_editor: editor,
        }
    }
    pub(crate) fn init_chewing_context(&mut self) {
        // self.apply_config()?;
        self.chewing_editor.set_editor_options(|opt| {
            if self.cfg.chewing_tsf.default_full_space {
                opt.character_form = CharacterForm::Fullwidth;
            }
            opt.auto_commit_threshold = 50;
        });
        self.lang_mode = if self.cfg.chewing_tsf.default_english {
            TsfLangMode::English
        } else {
            TsfLangMode::Chinese
        };
    }
    /// Initialize the config to the user default
    fn apply_init_config(&mut self) -> Result<(), TipError> {
        expect_error("Failed to apply initial config", || {
            let cfg = &self.cfg.chewing_tsf;
            self.output_simp_chinese = cfg.output_simp_chinese;
            self.chewing_editor.set_editor_options(|opt| {
                if self.cfg.chewing_tsf.default_full_space {
                    opt.character_form = CharacterForm::Fullwidth;
                }
                opt.auto_commit_threshold = 50;
            });

            self.lang_mode = if self.cfg.chewing_tsf.default_english {
                TsfLangMode::English
            } else {
                TsfLangMode::Chinese
            };
            self.apply_runtime_config()?;
            Ok(())
        })
    }
    /// Applys config if value was changed at runtime
    fn apply_config_if_changed(&mut self) -> Result<(), TipError> {
        expect_error("Failed to reapply config", || {
            if self.cfg.reload_if_needed()? {
                self.apply_runtime_config()?;
            }
            Ok(())
        })
    }
    /// Applys config changes that should be effective at runtime
    fn apply_runtime_config(&mut self) -> Result<(), TipError> {
        expect_error("Failed to apply runtime config", || {
            let cfg = &self.cfg.chewing_tsf;
            self.kbtype = KeyboardLayoutCompat::try_from(cfg.keyboard_layout as u8)
                .unwrap_or(KeyboardLayoutCompat::Default);
            self.chewing_editor = build_editor_from_cfg(cfg)?;
            let keybindings = cfg
                .keybind
                .iter()
                .filter_map(|kb| Keybinding::try_from(kb).ok())
                .collect();
            self.keybindings = keybindings;
            Ok(())
        })
    }
}

impl TipSession {
    pub(crate) fn on_focus(&mut self) -> Result<(), TipError> {
        Ok(())
    }
    pub(crate) fn on_blur(&mut self) -> Result<(), TipError> {
        Ok(())
    }
    pub(crate) fn on_test_keydown(
        &mut self,
        is_context_mutable: bool,
        is_composing: bool,
        shift_key_state: IpcShiftKeyState,
        ev: SystemKeyboardEvent,
    ) -> Result<bool, TipError> {
        expect_error("Failed to handle OnTestKeyDown", || {
            // NB: self.lang_mode might have changed earlier
            self.chewing_editor
                .set_editor_options(|opt| opt.language_mode = self.lang_mode.into());

            let evt = ev.to_keyboard_event(self.cfg.chewing_tsf.simulate_english_layout);
            let simulate_english_layout = self.cfg.chewing_tsf.simulate_english_layout != 0;
            //
            // Step 1. apply any config changes
            //
            if let Err(error) = self.apply_config_if_changed() {
                log::error!("{}", Report(&error));
            }
            //
            // Step 2. handle any mode change related keydown
            //
            // Ignore all keys if keyboard is closed
            if self.lang_mode.is_disabled() {
                return Ok(false);
            }
            //
            // Step 2.1 handle switch lang with Shift
            //
            if (evt.ksym == SYM_LEFTSHIFT || evt.ksym == SYM_RIGHTSHIFT)
                && self.cfg.chewing_tsf.switch_lang_with_shift
                && matches!(shift_key_state, IpcShiftKeyState::Up)
            {
                return Ok(false);
            }
            //
            // Step 2.2 handle any keybindings
            //
            if self.keybindings.iter().any(|kb| kb.matches(&evt)) {
                return Ok(true);
            }
            //
            // Step 2.3 ignore CapsLock if disabled
            if evt.ksym == SYM_CAPSLOCK && !self.cfg.chewing_tsf.enable_caps_lock {
                return Ok(false);
            }
            //
            // Step 3. ignore key events if the document is readonly or inactive
            //
            if !is_context_mutable {
                return Ok(false);
            }
            // Step 4. ignore key events if they might be shortcut keys
            //
            if evt.is_state_on(KeyState::Alt) {
                // bypass IME. This might be a shortcut key used in the application
                log::debug!("key not handled - Alt modifier key was down");
                return Ok(false);
            }
            if evt.is_state_on(KeyState::Control) {
                // bypass IME. This might be a shortcut key used in the application
                if is_composing && evt.ksym.is_digit() {
                    // need to handle userphrase
                    return Ok(true);
                } else if evt.is_state_on(KeyState::Shift)
                    && self.cfg.chewing_tsf.easy_symbols_with_shift_ctrl
                {
                    // need to handle easy symbol input
                    return Ok(true);
                } else {
                    log::debug!("key not handled - Ctrl modifier key was down");
                    return Ok(false);
                }
            }
            if self.cfg.chewing_tsf.enable_caps_lock
                && !self.cfg.chewing_tsf.lock_chinese_on_caps_lock
                && evt.ksym.is_unicode()
            {
                // need to handle case conversion
                return Ok(true);
            }
            if !is_composing {
                let shape_mode = self.chewing_editor.editor_options().character_form;
                // don't do further handling in pure English + half shape mode
                if self.lang_mode == LanguageMode::English
                    && shape_mode == CharacterForm::Halfwidth
                    && !simulate_english_layout
                {
                    if evt.ksym == SYM_SPACE
                        && evt.is_state_on(KeyState::Shift)
                        && self.cfg.chewing_tsf.enable_fullwidth_toggle_key
                    {
                        // need to handle fullwidth mode switch
                        return Ok(true);
                    } else {
                        log::debug!("key not handled - in English mode");
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
                    log::debug!("key not handled - key is not printable");
                    return Ok(false);
                }
            }
            Ok(true)
        })
    }
    pub(crate) fn on_keydown(&mut self, ev: SystemKeyboardEvent) -> Result<bool, TipError> {
        Ok(true)
    }
    pub(crate) fn on_test_keyup(&mut self, ev: SystemKeyboardEvent) -> Result<bool, TipError> {
        Ok(true)
    }
    pub(crate) fn on_keyup(&mut self, ev: SystemKeyboardEvent) -> Result<bool, TipError> {
        Ok(true)
    }
}

fn build_editor_from_cfg(cfg: &ChewingTsfConfig) -> Result<Editor, TipError> {
    expect_error("Failed to build chewing editor from config", || {
        let user_path = user_dir()?;
        let chewing_path = format!(
            "{};{}",
            user_path.display(),
            program_dir()?.join("Dictionary").display()
        );
        let user_dict_path = user_path.join("chewing.dat");
        // Recreate editor to load latest user files
        let mut editor = Editor::chewing(
            Some(chewing_path),
            // NB: the current API requires a *file* path
            Some(user_dict_path.to_string_lossy().into_owned()),
            &["word.dat", "tsi.dat", "chewing.dat", "chewing-deleted.dat"],
        );
        editor.set_editor_options(|opt| {
            opt.easy_symbol_input = cfg.easy_symbols_with_shift || cfg.easy_symbols_with_shift_ctrl;
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
                _ => ConversionEngineKind::ChewingEngine,
            };
            // FIXME
            opt.lookup_strategy = match cfg.conv_engine {
                0 => LookupStrategy::Standard,
                2 => LookupStrategy::FuzzyPartialPrefix,
                _ => LookupStrategy::Standard,
            };
            // TODO experimental
            opt.auto_snapshot_selections = true;
        });
        let kbtype = KeyboardLayoutCompat::try_from(cfg.keyboard_layout as u8)
            .unwrap_or(KeyboardLayoutCompat::Default);
        editor.set_syllable_editor(syl_editor_from_kbtype(kbtype));
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
        Ok(editor)
    })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TsfLangMode {
    Chinese,
    English,
    DisabledChinese,
    DisabledEnglish,
}

impl TsfLangMode {
    fn is_disabled(&self) -> bool {
        matches!(
            self,
            TsfLangMode::DisabledChinese | TsfLangMode::DisabledEnglish
        )
    }
}

impl From<TsfLangMode> for LanguageMode {
    fn from(value: TsfLangMode) -> Self {
        match value {
            TsfLangMode::Chinese => LanguageMode::Chinese,
            TsfLangMode::English => LanguageMode::English,
            TsfLangMode::DisabledChinese => LanguageMode::Chinese,
            TsfLangMode::DisabledEnglish => LanguageMode::English,
        }
    }
}

impl PartialEq<LanguageMode> for TsfLangMode {
    fn eq(&self, other: &LanguageMode) -> bool {
        matches!(
            (self, other),
            (TsfLangMode::Chinese, LanguageMode::Chinese)
                | (TsfLangMode::English, LanguageMode::English)
        )
    }
}

impl_context_error!(TipError);
