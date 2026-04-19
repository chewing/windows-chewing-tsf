// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::{
    cell::{Cell, RefCell},
    io::Write,
    ops::Not,
};

use anyhow::Result;
use chewing::input::keysym::{Keysym, SYM_DOWN, SYM_LEFT, SYM_RETURN, SYM_RIGHT, SYM_UP};
use chewing_tip_core::ipc::{
    messages::{HideCandidateList, ShowCandidateList},
    varlink::MethodCall,
};
use interprocess::os::windows::named_pipe::{PipeStream, pipe_mode::Bytes};
use log::error;
use windows::Win32::{
    Foundation::{E_INVALIDARG, TRUE},
    UI::TextServices::{
        ITfCandidateListUIElement, ITfCandidateListUIElement_Impl, ITfDocumentMgr, ITfThreadMgr,
        ITfUIElement, ITfUIElement_Impl, ITfUIElementMgr, TF_CLUIE_COUNT, TF_CLUIE_CURRENTPAGE,
        TF_CLUIE_DOCUMENTMGR, TF_CLUIE_PAGEINDEX, TF_CLUIE_SELECTION, TF_CLUIE_STRING,
    },
};
use windows_core::{
    BOOL, BSTR, ComObject, ComObjectInner, GUID, Interface, Result as WindowsResult, implement,
};

#[implement(ITfUIElement, ITfCandidateListUIElement)]
pub(crate) struct CandidateList {
    thread_mgr: ITfThreadMgr,
    element_id: Cell<u32>,
    uiless: Cell<bool>,
    pipe: RefCell<PipeStream<Bytes, Bytes>>,
    model: RefCell<ShowCandidateList>,
}

pub(crate) enum FilterKeyResult {
    Handled,
    HandledCommit,
    NotHandled,
}

impl CandidateList {
    pub(crate) fn new(
        thread_mgr: ITfThreadMgr,
        pipe: PipeStream<Bytes, Bytes>,
        model: ShowCandidateList,
    ) -> Result<ComObject<CandidateList>> {
        let ui_manager: ITfUIElementMgr = thread_mgr.cast()?;
        let candidate_list = CandidateList {
            thread_mgr,
            element_id: Cell::new(0),
            uiless: Cell::new(false),
            pipe: RefCell::new(pipe),
            model: RefCell::new(model),
        }
        .into_object();
        let mut should_show = TRUE;
        let mut ui_element_id = 0;
        let ui_element: ITfUIElement = candidate_list.cast()?;
        unsafe {
            ui_manager.BeginUIElement(&ui_element, &mut should_show, &mut ui_element_id)?;
            candidate_list.set_element_id(ui_element_id);
            candidate_list.Show(should_show)?;
        }
        Ok(candidate_list)
    }
    pub(crate) fn end_ui_element(&self) {
        self.hide();
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
    fn update_ui_element(&self) -> Result<()> {
        let ui_manager: ITfUIElementMgr = self.thread_mgr.cast()?;
        unsafe {
            ui_manager.UpdateUIElement(self.element_id.get())?;
        }
        Ok(())
    }
    pub(crate) fn set_model(&self, model: ShowCandidateList) {
        *self.model.borrow_mut() = model;
        if let Err(error) = self.update_ui_element() {
            error!("Failed to update UI element: {error}");
        }
    }
    pub(crate) fn filter_key_event(&self, ksym: Keysym) -> FilterKeyResult {
        let mut res = FilterKeyResult::NotHandled;
        {
            let mut model = self.model.borrow_mut();
            let old_sel = model.current_sel;
            if self.uiless.get() {
                match ksym {
                    SYM_DOWN | SYM_RIGHT => {
                        model.current_sel = model
                            .current_sel
                            .saturating_add(1)
                            .clamp(0, model.items.len() - 1);
                    }
                    SYM_UP | SYM_LEFT => {
                        model.current_sel = model.current_sel.saturating_sub(1);
                    }
                    SYM_RETURN => {
                        res = FilterKeyResult::HandledCommit;
                    }
                    _ => res = FilterKeyResult::NotHandled,
                }
            } else {
                let cand_per_row = model.cand_per_row as usize;
                match ksym {
                    SYM_UP => {
                        if model.current_sel >= cand_per_row {
                            model.current_sel -= cand_per_row;
                        }
                    }
                    SYM_DOWN => {
                        if model.current_sel + cand_per_row < model.items.len() {
                            model.current_sel += cand_per_row;
                        }
                    }
                    SYM_LEFT => {
                        if cand_per_row > 1 {
                            model.current_sel = model.current_sel.saturating_sub(1);
                        }
                    }
                    SYM_RIGHT => {
                        if cand_per_row > 1 {
                            model.current_sel = model
                                .current_sel
                                .saturating_add(1)
                                .clamp(0, model.items.len() - 1);
                        }
                    }
                    SYM_RETURN => {
                        res = FilterKeyResult::HandledCommit;
                    }
                    _ => res = FilterKeyResult::NotHandled,
                }
            }

            if model.current_sel != old_sel {
                res = FilterKeyResult::Handled;
            }
        }
        if let Err(error) = self.update_ui_element() {
            error!("Failed to update UI element: {error}");
        }
        res
    }
    pub(crate) fn current_sel(&self) -> usize {
        self.model.borrow().current_sel
    }
    pub(crate) fn current_phrase(&self) -> String {
        let sel = self.current_sel();
        self.model.borrow().items[sel].clone()
    }
    pub(crate) fn show(&self) -> Result<()> {
        if let Ok(mut pipe) = self.pipe.try_borrow_mut() {
            let mut bytes = serde_json::to_vec(&MethodCall {
                method: ShowCandidateList::METHOD.to_string(),
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
    pub(crate) fn hide(&self) -> Result<()> {
        if let Ok(mut pipe) = self.pipe.try_borrow_mut() {
            let mut bytes = serde_json::to_vec(&MethodCall {
                method: HideCandidateList::METHOD.to_string(),
                parameters: serde_json::to_value(HideCandidateList)?,
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

impl ITfUIElement_Impl for CandidateList_Impl {
    fn GetDescription(&self) -> WindowsResult<BSTR> {
        Ok(BSTR::from("Candidate List"))
    }

    fn GetGUID(&self) -> WindowsResult<GUID> {
        Ok(GUID::from_u128(0x4b7f55c3_2ae5_4077_a1c0_d17c5cb3c88a))
    }

    fn Show(&self, show: BOOL) -> WindowsResult<()> {
        self.uiless.set(!show.as_bool());
        if show.as_bool() {
            self.show();
        }
        Ok(())
    }

    fn IsShown(&self) -> WindowsResult<BOOL> {
        // TODO
        Ok(self.uiless.get().not().into())
    }
}

impl ITfCandidateListUIElement_Impl for CandidateList_Impl {
    fn GetUpdatedFlags(&self) -> WindowsResult<u32> {
        Ok(TF_CLUIE_DOCUMENTMGR
            | TF_CLUIE_COUNT
            | TF_CLUIE_SELECTION
            | TF_CLUIE_STRING
            | TF_CLUIE_PAGEINDEX
            | TF_CLUIE_CURRENTPAGE)
    }

    fn GetDocumentMgr(&self) -> WindowsResult<ITfDocumentMgr> {
        unsafe { self.thread_mgr.GetFocus() }
    }

    fn GetCount(&self) -> WindowsResult<u32> {
        let model = self.model.borrow();
        Ok(model.items.len() as u32)
    }

    fn GetSelection(&self) -> WindowsResult<u32> {
        let model = self.model.borrow();
        Ok(model.current_sel as u32)
    }

    fn GetString(&self, uindex: u32) -> WindowsResult<BSTR> {
        let model = self.model.borrow();
        if uindex as usize >= model.items.len() {
            return Err(E_INVALIDARG.into());
        }
        Ok(BSTR::from(model.items[uindex as usize].clone()))
    }

    fn GetPageIndex(
        &self,
        _pindex: *mut u32,
        _usize: u32,
        pupagecnt: *mut u32,
    ) -> WindowsResult<()> {
        unsafe {
            *pupagecnt = 1; // Assuming single page for simplicity
        }
        Ok(())
    }

    fn SetPageIndex(&self, _pindex: *const u32, _upagecnt: u32) -> WindowsResult<()> {
        Ok(())
    }

    fn GetCurrentPage(&self) -> WindowsResult<u32> {
        Ok(0) // Assuming single page for simplicity
    }
}
