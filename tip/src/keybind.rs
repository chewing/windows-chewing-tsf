use anyhow::{Context, Error};
use chewing::input::{KeyState, KeyboardEvent, keysym::*};

use crate::config::KeybindValue;

#[derive(Debug)]
pub(crate) struct Keybinding {
    pub(crate) key: KeyboardEvent,
    pub(crate) action: String,
}

impl TryFrom<&KeybindValue> for Keybinding {
    type Error = Error;

    fn try_from(value: &KeybindValue) -> Result<Self, Self::Error> {
        let key = key_from_str(&value.key).context("unable to parse key")?;
        Ok(Keybinding {
            key,
            action: value.action.clone(),
        })
    }
}

impl Keybinding {
    pub(crate) fn matches(&self, evt: &KeyboardEvent) -> bool {
        self.key.ksym == evt.ksym
            && [
                KeyState::Alt,
                KeyState::Control,
                KeyState::Shift,
                KeyState::Super,
            ]
            .iter()
            .all(|&state| self.key.is_state_on(state) == evt.is_state_on(state))
    }
}

fn key_from_str(s: &str) -> Option<KeyboardEvent> {
    let mut parts = s.split('+').rev();
    let mut builder = KeyboardEvent::builder();
    if let Some(key) = parts.next() {
        // Last part should be the Keysym
        builder.ksym(keysym_from_str(key)?);
    }
    // Other parts should be modifiers
    for modifier in parts {
        match modifier.trim().to_lowercase().as_str() {
            "ctrl" | "control" => {
                builder.control();
            }
            "shift" => {
                builder.shift();
            }
            "alt" | "opt" | "option" => {
                builder.alt_if(true);
            }
            "super" | "cmd" | "command" => {
                builder.super_if(true);
            }
            _ => (),
        }
    }
    Some(builder.build())
}

fn keysym_from_str(s: &str) -> Option<Keysym> {
    let s = s.trim();
    let ksym = match s {
        "Esc" => SYM_ESC,
        "F1" => SYM_F1,
        "F2" => SYM_F2,
        "F3" => SYM_F3,
        "F4" => SYM_F4,
        "F5" => SYM_F5,
        "F6" => SYM_F6,
        "F7" => SYM_F7,
        "F8" => SYM_F8,
        "F9" => SYM_F9,
        "F10" => SYM_F10,
        "F11" => SYM_F11,
        "F12" => SYM_F12,
        "Home" => SYM_HOME,
        "End" => SYM_END,
        "Delete" => SYM_DELETE,
        "Tab" => SYM_TAB,
        "Backspace" => SYM_BACKSPACE,
        "CapsLock" => SYM_CAPSLOCK,
        "Enter" | "Return" => SYM_RETURN,
        _ if s.chars().count() == 1 => Keysym::from_char(s.chars().next().unwrap()),
        _ => return None,
    };
    Some(ksym)
}

#[cfg(test)]
mod tests {
    use chewing::input::{
        KeyboardEvent,
        keysym::{Keysym, SYM_F12},
    };

    use super::key_from_str;

    #[test]
    fn parse_ctrl_f12() {
        let target = Some(KeyboardEvent::builder().ksym(SYM_F12).control().build());
        assert_eq!(target, key_from_str("Ctrl+F12"));
        assert_eq!(target, key_from_str("ctrl+F12"));
        assert_eq!(target, key_from_str("control+F12"));
    }
    #[test]
    fn parse_ctrl_shift_a() {
        let target = Some(
            KeyboardEvent::builder()
                .ksym(Keysym::from_char('A'))
                .control()
                .shift()
                .build(),
        );
        assert_eq!(target, key_from_str("Ctrl+Shift+A"));
        assert_eq!(target, key_from_str("ctrl+shift+A"));
        assert_eq!(target, key_from_str("control+shift+A"));
    }
}
