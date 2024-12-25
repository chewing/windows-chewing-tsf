use std::{cell::Cell, collections::BTreeMap, ffi::c_void, sync::RwLock};

use windows::Win32::{
    Foundation::{BOOL, E_FAIL, E_INVALIDARG, POINT, RECT},
    UI::{
        TextServices::{
            ITfLangBarItem, ITfLangBarItemButton, ITfLangBarItemButton_Impl,
            ITfLangBarItemButton_Vtbl, ITfLangBarItemSink, ITfLangBarItem_Impl, ITfMenu, ITfSource,
            ITfSource_Impl, TfLBIClick, TF_LANGBARITEMINFO, TF_LBI_CLK_RIGHT, TF_LBI_ICON,
            TF_LBI_STATUS, TF_LBI_STATUS_DISABLED, TF_LBI_STATUS_HIDDEN, TF_LBMENUF_CHECKED,
            TF_LBMENUF_GRAYED, TF_LBMENUF_SEPARATOR, TF_LBMENUF_SUBMENU,
        },
        WindowsAndMessaging::{
            CopyIcon, DestroyIcon, GetMenuItemCount, GetMenuItemInfoW, HICON, HMENU, MENUITEMINFOW,
            MENU_ITEM_STATE, MFS_CHECKED, MFS_DISABLED, MFS_GRAYED, MFT_SEPARATOR, MFT_STRING,
            MIIM_FTYPE, MIIM_ID, MIIM_STATE, MIIM_STRING, MIIM_SUBMENU,
        },
    },
};
use windows_core::{
    implement, interface, ComObjectInner, IUnknown, IUnknown_Vtbl, Interface, Result, BSTR, GUID,
    PWSTR,
};

#[repr(C)]
enum CommandType {
    LeftClick,
    RightClick,
    Menu,
}

#[interface("f320f835-b95d-4d3f-89d5-fd4ab7b9d7bb")]
pub(crate) unsafe trait IRunCommand: IUnknown {
    fn on_command(&self, id: u32, cmd_type: CommandType);
}

#[interface("4db963b1-ced3-42b7-8f87-937534740e7a")]
pub(crate) unsafe trait ILangBarButton: ITfLangBarItemButton {
    fn set_icon(&self, icon: HICON) -> Result<()>;
    fn set_enabled(&self, enabled: bool) -> Result<()>;
}

#[implement(ITfLangBarItem, ITfLangBarItemButton, ITfSource, ILangBarButton)]
struct LangBarButton {
    info: TF_LANGBARITEMINFO,
    status: Cell<u32>,
    tooltip: BSTR,
    icon: Cell<HICON>,
    /// borrowed - we don't own this menu
    menu: HMENU,
    command_id: u32,
    run_command: IRunCommand,
    sinks: RwLock<BTreeMap<u32, ITfLangBarItemSink>>,
}

impl Drop for LangBarButton {
    fn drop(&mut self) {
        if !self.icon.get().is_invalid() {
            let _ = unsafe { DestroyIcon(self.icon.get()) };
        }
    }
}

#[no_mangle]
unsafe extern "C" fn CreateLangBarButton(
    info: TF_LANGBARITEMINFO,
    tooltip: BSTR,
    icon: HICON,
    menu: HMENU,
    command_id: u32,
    run_command: *mut IRunCommand,
    ret: *mut *mut c_void,
) {
    let binding = run_command.cast();
    let run_command_ref =
        IRunCommand::from_raw_borrowed(&binding).expect("invalid IRunCommand pointer");
    let run_command: IRunCommand = run_command_ref.cast().expect("invalid IRunCommand pointer");
    let lang_bar_btn = LangBarButton {
        info,
        status: Cell::new(0),
        tooltip,
        icon: Cell::new(icon),
        menu,
        command_id,
        run_command,
        sinks: RwLock::new(BTreeMap::new()),
    }
    .into_object();
    ret.write(lang_bar_btn.into_interface::<ILangBarButton>().into_raw());
}

impl ILangBarButton_Impl for LangBarButton_Impl {
    unsafe fn set_icon(&self, icon: HICON) -> Result<()> {
        if !self.icon.get().is_invalid() {
            DestroyIcon(self.icon.get())?;
        }
        self.icon.set(icon);
        self.update_sinks(TF_LBI_ICON)?;
        Ok(())
    }

    unsafe fn set_enabled(&self, enabled: bool) -> Result<()> {
        if enabled {
            self.status.set(self.status.get() & !TF_LBI_STATUS_DISABLED);
        } else {
            self.status.set(self.status.get() | TF_LBI_STATUS_DISABLED);
        }
        self.update_sinks(TF_LBI_STATUS)?;
        Ok(())
    }
}

impl ITfLangBarItem_Impl for LangBarButton_Impl {
    fn GetInfo(&self, pinfo: *mut TF_LANGBARITEMINFO) -> Result<()> {
        if pinfo.is_null() {
            return Err(E_INVALIDARG.into());
        }
        unsafe {
            pinfo.write(self.info);
        }
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        Ok(self.status.get())
    }

    fn Show(&self, fshow: BOOL) -> Result<()> {
        if fshow.as_bool() {
            self.status.set(self.status.get() & !TF_LBI_STATUS_HIDDEN);
        } else {
            self.status.set(self.status.get() | TF_LBI_STATUS_HIDDEN);
        }
        self.update_sinks(TF_LBI_STATUS)?;
        Ok(())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(self.tooltip.clone())
    }
}

impl ITfLangBarItemButton_Impl for LangBarButton_Impl {
    fn OnClick(&self, click: TfLBIClick, _pt: &POINT, _prcareaa: *const RECT) -> Result<()> {
        let cmd_type = if click == TF_LBI_CLK_RIGHT {
            CommandType::RightClick
        } else {
            CommandType::LeftClick
        };
        unsafe { self.run_command.on_command(self.command_id, cmd_type) };
        Ok(())
    }

    fn InitMenu(&self, pmenu: Option<&ITfMenu>) -> Result<()> {
        if self.menu.is_invalid() {
            return Err(E_FAIL.into());
        }
        if let Some(menu) = pmenu {
            build_menu(menu, self.menu)
        } else {
            Err(E_FAIL.into())
        }
    }

    fn OnMenuSelect(&self, wid: u32) -> Result<()> {
        unsafe { self.run_command.on_command(wid, CommandType::Menu) };
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        unsafe { CopyIcon(self.icon.get()) }
    }

    fn GetText(&self) -> Result<BSTR> {
        BSTR::from_wide(&self.info.szDescription)
    }
}

impl ITfSource_Impl for LangBarButton_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Option<&IUnknown>) -> Result<u32> {
        if riid.is_null() || punk.is_none() {
            return Err(E_INVALIDARG.into());
        }
        if unsafe { *riid == ITfLangBarItemSink::IID } {
            let mut cookie = [0; 4];
            if let Err(_) = getrandom::getrandom(&mut cookie) {
                return Err(E_FAIL.into());
            }
            let cookie = u32::from_ne_bytes(cookie);
            let sink: ITfLangBarItemSink = punk.unwrap().cast()?;
            if let Ok(mut sinks) = self.sinks.write() {
                sinks.insert(cookie, sink);
                return Ok(cookie);
            }
        }
        Err(E_FAIL.into())
    }

    fn UnadviseSink(&self, dwcookie: u32) -> Result<()> {
        if let Ok(mut sinks) = self.sinks.write() {
            sinks.remove(&dwcookie);
            return Ok(());
        }
        Err(E_FAIL.into())
    }
}

impl LangBarButton {
    fn update_sinks(&self, dwflags: u32) -> Result<()> {
        if let Ok(sinks) = self.sinks.read() {
            for sink in sinks.values() {
                unsafe { sink.OnUpdate(dwflags)? };
            }
        }
        Ok(())
    }
}

fn build_menu(menu: &ITfMenu, hmenu: HMENU) -> Result<()> {
    for i in 0..unsafe { GetMenuItemCount(hmenu) } {
        let mut text_buffer = [0_u16; 256];
        let mut mi = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_FTYPE | MIIM_ID | MIIM_STATE | MIIM_STRING | MIIM_SUBMENU,
            dwTypeData: PWSTR::from_raw(text_buffer.as_mut_ptr()),
            cch: 255,
            ..Default::default()
        };
        if let Ok(()) = unsafe { GetMenuItemInfoW(hmenu, i as u32, true, &mut mi) } {
            let mut flags = 0;
            let mut sub_menu = None;
            if !mi.hSubMenu.is_invalid() {
                flags |= TF_LBMENUF_SUBMENU;
            }
            if mi.fType == MFT_SEPARATOR {
                flags |= TF_LBMENUF_SEPARATOR;
            } else if mi.fType != MFT_STRING {
                // other types of menu are not supported
                continue;
            }
            if mi.fState.contains(MFS_CHECKED) {
                flags |= TF_LBMENUF_CHECKED;
            }
            // Despite the document says MFS_GRAYED and MFS_DISABLED has the same value 0x3
            // Inactive menu item actually has value 0x2, same as MF_DISABLED
            if mi.fState.contains(MFS_GRAYED)
                || mi.fState.contains(MFS_DISABLED)
                || mi.fState.contains(MENU_ITEM_STATE(0x2))
            {
                flags |= TF_LBMENUF_GRAYED;
            }
            if let Ok(()) =
                unsafe { menu.AddMenuItem(mi.wID, flags, None, None, &text_buffer, &mut sub_menu) }
            {
                if let Some(sub_menu) = sub_menu {
                    build_menu(&sub_menu, mi.hSubMenu)?;
                }
            }
        }
    }
    Ok(())
}
