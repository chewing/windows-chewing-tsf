// SPDX-License-Identifier: GPL-3.0-or-later

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardState, ToAscii, VIRTUAL_KEY, VK_A, VK_CONTROL, VK_DIVIDE, VK_NUMPAD0, VK_Z,
};

pub(super) struct KeyEvent {
    pub(super) vk: u16,
    pub(super) code: u8,
    key_state: [u8; 256],
}

impl KeyEvent {
    pub fn new(vk: u16, lparam: isize) -> KeyEvent {
        let scan_code = lparam & 0xff0000;
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
            code,
            key_state,
        }
    }
    pub(super) fn is_alphabet(&self) -> bool {
        self.vk >= VK_A.0 && self.vk <= VK_Z.0
    }
    pub(super) fn is_num_pad(&self) -> bool {
        self.vk >= VK_NUMPAD0.0 && self.vk <= VK_DIVIDE.0
    }
    pub(super) fn is_printable(&self) -> bool {
        !self.code.is_ascii_control()
    }
    pub(super) fn is_key(&self, vk: VIRTUAL_KEY) -> bool {
        self.vk == vk.0
    }
    pub(super) fn is_key_down(&self, vk: VIRTUAL_KEY) -> bool {
        self.key_state[vk.0 as usize] & (1 << 7) != 0
    }
    pub(super) fn is_key_toggled(&self, vk: VIRTUAL_KEY) -> bool {
        self.key_state[vk.0 as usize] & 1 != 0
    }
}
