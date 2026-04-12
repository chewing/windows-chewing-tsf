// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    cell::{Cell, RefCell},
    io::Write,
};

use anyhow::Result;
use chewing_tip_core::ipc::{messages::ShowNotification, varlink::MethodCall};
use interprocess::os::windows::named_pipe::{PipeStream, pipe_mode::Bytes};
use log::error;
use windows::Win32::{
    Foundation::TRUE,
    UI::TextServices::{ITfThreadMgr, ITfUIElement, ITfUIElement_Impl, ITfUIElementMgr},
};
use windows_core::{
    BOOL, BSTR, ComObject, ComObjectInner, GUID, Interface, Result as WindowsResult, implement,
};

#[implement(ITfUIElement)]
pub(crate) struct Notification {
    thread_mgr: ITfThreadMgr,
    element_id: Cell<u32>,
    pipe: RefCell<PipeStream<Bytes, Bytes>>,
    model: ShowNotification,
}

impl Notification {
    pub(crate) fn new(
        thread_mgr: ITfThreadMgr,
        pipe: PipeStream<Bytes, Bytes>,
        model: ShowNotification,
    ) -> Result<ComObject<Notification>> {
        let ui_manager: ITfUIElementMgr = thread_mgr.cast()?;
        let notification = Notification {
            thread_mgr,
            element_id: Cell::new(0),
            pipe: RefCell::new(pipe),
            model,
        }
        .into_object();
        let mut should_show = TRUE;
        let mut ui_element_id = 0;
        let ui_element: ITfUIElement = notification.cast()?;
        unsafe {
            ui_manager.BeginUIElement(&ui_element, &mut should_show, &mut ui_element_id)?;
            notification.set_element_id(ui_element_id);
            notification.Show(should_show)?;
        }
        Ok(notification)
    }
    pub(crate) fn end_ui_element(&self) {
        let Ok(ui_manager): Result<ITfUIElementMgr, windows_core::Error> = self.thread_mgr.cast()
        else {
            error!("unable to cast thread manager to ITfUIElementMgr");
            return;
        };
        unsafe {
            let _ = ui_manager.EndUIElement(self.element_id.get());
        }
    }
    fn set_element_id(&self, id: u32) {
        self.element_id.set(id);
    }
    fn show(&self) -> Result<()> {
        if let Ok(mut pipe) = self.pipe.try_borrow_mut() {
            let mut bytes = serde_json::to_vec(&MethodCall {
                method: ShowNotification::METHOD.to_string(),
                parameters: serde_json::to_value(&self.model)?,
                oneway: Some(true),
                more: None,
                upgrade: None,
            })?;
            bytes.push(0);
            pipe.write_all(&bytes)?;
        }
        Ok(())
    }
}

impl ITfUIElement_Impl for Notification_Impl {
    fn GetDescription(&self) -> WindowsResult<BSTR> {
        Ok(BSTR::from("Candidate List"))
    }

    fn GetGUID(&self) -> WindowsResult<GUID> {
        Ok(GUID::from_u128(0x80cd1c64_5c4a_4478_8690_20c489534629))
    }

    fn Show(&self, show: BOOL) -> WindowsResult<()> {
        if show.as_bool() {
            if let Err(error) = self.show() {
                error!("Failed to show notification window: {error:?}");
            }
        }
        Ok(())
    }

    fn IsShown(&self) -> WindowsResult<BOOL> {
        // TODO
        Ok(TRUE)
    }
}
