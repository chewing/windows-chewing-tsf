// SPDX-License-Identifier: GPL-3.0-or-later

use chewing::input::keycode::Keycode;
use chewing::input::keymap::{
    INVERTED_COLEMAK_DH_ANSI_MAP, INVERTED_COLEMAK_DH_ORTH_MAP, INVERTED_COLEMAK_MAP,
    INVERTED_DVORAK_MAP, INVERTED_QGMLWY_MAP, INVERTED_WORKMAN_MAP, map_keycode,
};
use chewing::input::{KeyboardEvent, keysym::*};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardState, ToAscii, VIRTUAL_KEY, VK_CAPITAL, VK_CONTROL, VK_LWIN, VK_MENU, VK_NUMLOCK,
    VK_SHIFT,
};

pub(super) struct KeyEvent {
    vk: u16,
    scan_code: u16,
    ascii_code: u8,
    key_state: [u8; 256],
}

impl Default for KeyEvent {
    fn default() -> Self {
        KeyEvent::new(0, 0)
    }
}

impl KeyEvent {
    pub fn new(vk: u16, lparam: isize) -> KeyEvent {
        let scan_code = ((lparam & 0xff0000) >> 16) as u16;
        let mut key_state = [0u8; 256];
        let mut code = 0;
        unsafe {
            if GetKeyboardState(&mut key_state).is_err() {
                key_state.fill(0);
            }
            // try to convert the key event to an ASCII character
            // ToAscii API tries to convert Ctrl + printable characters to
            // ASCII 0x00 - 0x31 non-printable escape characters, which we don't want
            // So here is a hack: pretend that Ctrl key is not pressed
            let mut ks = key_state;
            ks[VK_CONTROL.0 as usize] = 0;
            let mut result = 0u16;
            if ToAscii(vk as u32, scan_code as u32, Some(&ks), &mut result, 0) == 1 {
                code = result as u8;
            }
        }
        KeyEvent {
            vk,
            scan_code,
            ascii_code: code,
            key_state,
        }
    }
    fn is_key_down(&self, vk: VIRTUAL_KEY) -> bool {
        self.key_state[vk.0 as usize] & (1 << 7) != 0
    }
    fn is_key_toggled(&self, vk: VIRTUAL_KEY) -> bool {
        self.key_state[vk.0 as usize] & 1 != 0
    }
    pub(super) fn to_keyboard_event(&self, kbtype: i32) -> KeyboardEvent {
        let keycode = SCANCODE_MAP
            .binary_search_by_key(&self.scan_code, |&(w, _)| w)
            .ok()
            .map(|idx| Keycode(SCANCODE_MAP[idx].1))
            .unwrap_or_default();
        let keymap = KB_KEYMAP_MAP
            .iter()
            .find(|it| it.0 == kbtype)
            .map(|it| it.1);
        let keysym = VKEY_MAP
            .binary_search_by_key(&self.vk, |&(k, _)| k)
            .ok()
            .map(|idx| VKEY_MAP[idx].1)
            .unwrap_or_else(|| {
                if let Some(keymap) = keymap {
                    map_keycode(&keymap, keycode, self.is_key_down(VK_SHIFT)).ksym
                } else {
                    Keysym(self.ascii_code as u32)
                }
            });
        KeyboardEvent::builder()
            .code(keycode)
            .ksym(keysym)
            .shift_if(self.is_key_down(VK_SHIFT))
            .control_if(self.is_key_down(VK_CONTROL))
            .alt_if(self.is_key_down(VK_MENU))
            .caps_lock_if(self.is_key_toggled(VK_CAPITAL))
            .num_lock_if(self.is_key_toggled(VK_NUMLOCK))
            .super_if(self.is_key_down(VK_LWIN))
            .build()
    }
}

const KB_KEYMAP_MAP: &[(i32, &[(u8, KeyboardEvent)])] = &[
    (1, &INVERTED_DVORAK_MAP),
    (2, &INVERTED_QGMLWY_MAP),
    (3, &INVERTED_COLEMAK_MAP),
    (4, &INVERTED_COLEMAK_DH_ANSI_MAP),
    (5, &INVERTED_COLEMAK_DH_ORTH_MAP),
    (6, &INVERTED_WORKMAN_MAP),
];

// Windows Set 1 scancode to X11 keycode mapping
const SCANCODE_MAP: &[(u16, u8)] = &[
    (0x01, 9),  // Esc
    (0x02, 10), // 1
    (0x03, 11), // 2
    (0x04, 12), // 3
    (0x05, 13), // 4
    (0x06, 14), // 5
    (0x07, 15), // 6
    (0x08, 16), // 7
    (0x09, 17), // 8
    (0x0A, 18), // 9
    (0x0B, 19), // 0
    (0x0C, 20), // -
    (0x0D, 21), // =
    (0x0E, 22), // Backspace
    (0x0F, 23), // Tab
    (0x10, 24), // Q
    (0x11, 25), // W
    (0x12, 26), // E
    (0x13, 27), // R
    (0x14, 28), // T
    (0x15, 29), // Y
    (0x16, 30), // U
    (0x17, 31), // I
    (0x18, 32), // O
    (0x19, 33), // P
    (0x1A, 34), // [
    (0x1B, 35), // ]
    (0x1C, 36), // Enter
    (0x1D, 37), // Left Ctrl
    (0x1E, 38), // A
    (0x1F, 39), // S
    (0x20, 40), // D
    (0x21, 41), // F
    (0x22, 42), // G
    (0x23, 43), // H
    (0x24, 44), // J
    (0x25, 45), // K
    (0x26, 46), // L
    (0x27, 47), // ;
    (0x28, 48), // '
    (0x29, 49), // `
    (0x2A, 50), // Left Shift
    (0x2B, 51), // \
    (0x2C, 52), // Z
    (0x2D, 53), // X
    (0x2E, 54), // C
    (0x2F, 55), // V
    (0x30, 56), // B
    (0x31, 57), // N
    (0x32, 58), // M
    (0x33, 59), // ,
    (0x34, 60), // .
    (0x35, 61), // /
    (0x36, 62), // Right Shift
    (0x37, 63), // Numpad *
    (0x38, 64), // Left Alt
    (0x39, 65), // Space
    (0x3A, 66), // Caps Lock
    (0x3B, 67), // F1
    (0x3C, 68), // F2
    (0x3D, 69), // F3
    (0x3E, 70), // F4
    (0x3F, 71), // F5
    (0x40, 72), // F6
    (0x41, 73), // F7
    (0x42, 74), // F8
    (0x43, 75), // F9
    (0x44, 76), // F10
    (0x45, 77), // Num Lock
    (0x46, 78), // Scroll Lock
    (0x47, 79), // Numpad 7
    (0x48, 80), // Numpad 8
    (0x49, 81), // Numpad 9
    (0x4A, 82), // Numpad -
    (0x4B, 83), // Numpad 4
    (0x4C, 84), // Numpad 5
    (0x4D, 85), // Numpad 6
    (0x4E, 86), // Numpad +
    (0x4F, 87), // Numpad 1
    (0x50, 88), // Numpad 2
    (0x51, 89), // Numpad 3
    (0x52, 90), // Numpad 0
    (0x53, 91), // Numpad .
    (0x57, 95), // F11
    (0x58, 96), // F12
    // Extended scancodes (0xE0xx)
    (0xE01C, 104), // Numpad Enter
    (0xE01D, 105), // Right Ctrl
    (0xE035, 106), // Numpad /
    (0xE038, 108), // Right Alt
    (0xE047, 110), // Home
    (0xE048, 111), // Up
    (0xE049, 112), // Page Up
    (0xE04B, 113), // Left
    (0xE04D, 114), // Right
    (0xE04F, 115), // End
    (0xE050, 116), // Down
    (0xE051, 117), // Page Down
    (0xE052, 118), // Insert
    (0xE053, 119), // Delete
    (0xE05B, 133), // Left Win
    (0xE05C, 134), // Right Win
    (0xE05D, 135), // Menu
];

const VKEY_MAP: &[(u16, Keysym)] = &[
    (0x08, SYM_BACKSPACE),
    (0x09, SYM_TAB),
    (0x0D, SYM_RETURN),
    (0x10, SYM_LEFTSHIFT),
    (0x11, SYM_LEFTCTRL),
    (0x12, SYM_LEFTALT),
    (0x14, SYM_CAPSLOCK),
    (0x1B, SYM_ESC),
    (0x20, SYM_SPACE),
    (0x21, SYM_PAGEUP),
    (0x22, SYM_PAGEDOWN),
    (0x23, SYM_END),
    (0x24, SYM_HOME),
    (0x25, SYM_LEFT),
    (0x26, SYM_UP),
    (0x27, SYM_RIGHT),
    (0x28, SYM_DOWN),
    (0x2E, SYM_DELETE),
    (0x5B, SYM_LEFTMETA),
    (0x5C, SYM_RIGHTMETA),
    (0x60, SYM_KP0),
    (0x61, SYM_KP1),
    (0x62, SYM_KP2),
    (0x63, SYM_KP3),
    (0x64, SYM_KP4),
    (0x65, SYM_KP5),
    (0x66, SYM_KP6),
    (0x67, SYM_KP7),
    (0x68, SYM_KP8),
    (0x69, SYM_KP9),
    (0x6A, SYM_KPMULTIPLY),
    (0x6B, SYM_KPADD),
    (0x6D, SYM_KPSUBTRACT),
    (0x6E, SYM_KPDECIMAL),
    (0x6F, SYM_KPDIVIDE),
    (0x70, SYM_F1),
    (0x71, SYM_F2),
    (0x72, SYM_F3),
    (0x73, SYM_F4),
    (0x74, SYM_F5),
    (0x75, SYM_F6),
    (0x76, SYM_F7),
    (0x77, SYM_F8),
    (0x78, SYM_F9),
    (0x79, SYM_F10),
    (0x7A, SYM_F11),
    (0x7B, SYM_F12),
    (0x90, SYM_NUMLOCK),
    (0xA0, SYM_LEFTSHIFT),
    (0xA1, SYM_RIGHTSHIFT),
    (0xA2, SYM_LEFTCTRL),
    (0xA3, SYM_RIGHTCTRL),
    (0xA4, SYM_LEFTALT),
    (0xA5, SYM_RIGHTALT),
];
