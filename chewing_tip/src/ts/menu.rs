use std::ops::Deref;

use windows::Win32::{Foundation::HINSTANCE, UI::WindowsAndMessaging::*};
use windows_core::PCWSTR;

#[derive(Default)]
pub(super) struct Menu {
    hmenu: HMENU,
}

#[derive(Default)]
pub(super) struct MenuRef {
    hmenu: HMENU,
}

impl Menu {
    pub(super) fn load(hinst: HINSTANCE, resource_id: u32) -> Menu {
        let hmenu =
            match unsafe { LoadMenuW(Some(hinst), PCWSTR::from_raw(resource_id as *const u16)) } {
                Ok(menu) => menu,
                Err(error) => {
                    log::error!("unable to load menu {resource_id}: {error}");
                    HMENU::default()
                }
            };
        Menu { hmenu }
    }
    pub(super) fn sub_menu(&self, npos: i32) -> MenuRef {
        if self.hmenu.is_invalid() {
            return MenuRef {
                hmenu: HMENU::default(),
            };
        }
        let hmenu = unsafe { GetSubMenu(self.hmenu, npos) };
        MenuRef { hmenu }
    }
}

impl Deref for MenuRef {
    type Target = HMENU;

    fn deref(&self) -> &Self::Target {
        &self.hmenu
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        if !self.hmenu.is_invalid() {
            unsafe {
                let _ = DestroyMenu(self.hmenu);
            }
        }
    }
}
