// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

use windows::Win32::{
    Foundation::{E_FAIL, E_INVALIDARG, POINT, RECT},
    Graphics::Gdi::HBITMAP,
    UI::{
        TextServices::{
            ITfLangBarItem, ITfLangBarItem_Impl, ITfLangBarItemButton, ITfLangBarItemButton_Impl,
            ITfLangBarItemSink, ITfMenu, ITfSource, ITfSource_Impl, ITfThreadMgr,
            TF_LANGBARITEMINFO, TF_LBI_CLK_RIGHT, TF_LBI_ICON, TF_LBI_STATUS,
            TF_LBI_STATUS_DISABLED, TF_LBI_STATUS_HIDDEN, TF_LBMENUF_CHECKED, TF_LBMENUF_GRAYED,
            TF_LBMENUF_SEPARATOR, TF_LBMENUF_SUBMENU, TfLBIClick,
        },
        WindowsAndMessaging::{
            CopyIcon, DestroyIcon, GetMenuItemCount, GetMenuItemInfoW, HICON, HMENU,
            MENU_ITEM_STATE, MENUITEMINFOW, MFS_CHECKED, MFS_DISABLED, MFS_GRAYED, MFT_SEPARATOR,
            MFT_STRING, MIIM_FTYPE, MIIM_ID, MIIM_STATE, MIIM_STRING, MIIM_SUBMENU,
        },
    },
};
use windows_core::{BOOL, BSTR, GUID, IUnknown, Interface, PWSTR, Ref, Result, implement};

use super::CHEWING_TSF_CLSID;
use super::{CommandType, IFnRunCommand};

#[implement(ITfLangBarItem, ITfLangBarItemButton, ITfSource)]
pub(super) struct LangBarButton {
    info: TF_LANGBARITEMINFO,
    status: Cell<u32>,
    tooltip: BSTR,
    icon: Cell<HICON>,
    menu: HMENU,
    command_id: u32,
    thread_mgr: ITfThreadMgr,
    sinks: RefCell<BTreeMap<u32, ITfLangBarItemSink>>,
}

impl Drop for LangBarButton {
    fn drop(&mut self) {
        if !self.icon.get().is_invalid() {
            let _ = unsafe { DestroyIcon(self.icon.get()) };
        }
    }
}

impl LangBarButton {
    pub(super) fn new(
        info: TF_LANGBARITEMINFO,
        tooltip: BSTR,
        icon: HICON,
        menu: HMENU,
        command_id: u32,
        thread_mgr: ITfThreadMgr,
    ) -> LangBarButton {
        LangBarButton {
            info,
            status: Cell::new(0),
            tooltip,
            icon: Cell::new(icon),
            menu,
            command_id,
            thread_mgr,
            sinks: RefCell::new(BTreeMap::new()),
        }
    }
    pub(super) fn set_icon(&self, icon: HICON) -> Result<()> {
        if !self.icon.get().is_invalid() {
            unsafe { DestroyIcon(self.icon.get()) }?;
        }
        self.icon.set(icon);
        self.update_sinks(TF_LBI_ICON)?;
        Ok(())
    }
    pub(super) fn set_enabled(&self, enabled: bool) -> Result<()> {
        if enabled {
            self.status.update(|s| s & !TF_LBI_STATUS_DISABLED);
        } else {
            self.status.update(|s| s | TF_LBI_STATUS_DISABLED);
        }
        self.update_sinks(TF_LBI_STATUS)?;
        Ok(())
    }
    fn update_sinks(&self, dwflags: u32) -> Result<()> {
        if let Ok(sinks) = self.sinks.try_borrow() {
            for sink in sinks.values() {
                unsafe { sink.OnUpdate(dwflags)? };
            }
        }
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
        unsafe {
            if let Ok(fn_provider) = self.thread_mgr.GetFunctionProvider(&CHEWING_TSF_CLSID)
                && let Ok(punk) = fn_provider.GetFunction(&GUID::zeroed(), &IFnRunCommand::IID)
            {
                let cb: IFnRunCommand = punk.cast()?;
                cb.on_command(self.command_id, cmd_type);
            }
        }
        Ok(())
    }

    fn InitMenu(&self, pmenu: Ref<ITfMenu>) -> Result<()> {
        if self.menu.is_invalid() {
            return Err(E_FAIL.into());
        }
        if let Some(pmenu) = pmenu.as_ref() {
            return build_menu(pmenu, self.menu);
        }
        Err(E_FAIL.into())
    }

    fn OnMenuSelect(&self, wid: u32) -> Result<()> {
        unsafe {
            if let Ok(fn_provider) = self.thread_mgr.GetFunctionProvider(&CHEWING_TSF_CLSID)
                && let Ok(punk) = fn_provider.GetFunction(&GUID::zeroed(), &IFnRunCommand::IID)
            {
                let cb: IFnRunCommand = punk.cast()?;
                cb.on_command(wid, CommandType::Menu);
            }
        }
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        unsafe { CopyIcon(self.icon.get()) }
    }

    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::from_wide(&self.info.szDescription))
    }
}

impl ITfSource_Impl for LangBarButton_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>) -> Result<u32> {
        if riid.is_null() || punk.is_none() {
            return Err(E_INVALIDARG.into());
        }
        if unsafe { *riid == ITfLangBarItemSink::IID } {
            let Ok(cookie) = getrandom::u32() else {
                return Err(E_FAIL.into());
            };
            let sink: ITfLangBarItemSink = punk.unwrap().cast()?;
            if let Ok(mut sinks) = self.sinks.try_borrow_mut() {
                sinks.insert(cookie, sink);
                return Ok(cookie);
            }
        }
        Err(E_FAIL.into())
    }

    fn UnadviseSink(&self, dwcookie: u32) -> Result<()> {
        if let Ok(mut sinks) = self.sinks.try_borrow_mut() {
            sinks.remove(&dwcookie);
            return Ok(());
        }
        Err(E_FAIL.into())
    }
}

fn build_menu(menu: &ITfMenu, hmenu: HMENU) -> Result<()> {
    for i in 0..unsafe { GetMenuItemCount(Some(hmenu)) } {
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
            if let Ok(()) = unsafe {
                menu.AddMenuItem(
                    mi.wID,
                    flags,
                    HBITMAP::default(),
                    HBITMAP::default(),
                    &text_buffer,
                    &mut sub_menu,
                )
            } && let Some(sub_menu) = sub_menu
            {
                build_menu(&sub_menu, mi.hSubMenu)?;
            }
        }
    }
    Ok(())
}
