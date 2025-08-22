// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::OnceCell;
use std::ops::Deref;
use std::ptr;

use log::{debug, error};
use windows::Win32::Foundation::RECT;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::TextServices::{
    GUID_PROP_ATTRIBUTE, ITfComposition, ITfCompositionSink, ITfContext, ITfContextComposition,
    ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection, TF_ANCHOR_END, TF_ANCHOR_START,
    TF_DEFAULT_SELECTION, TF_IAS_QUERYONLY, TF_SELECTION,
};
use windows_core::{BOOL, HSTRING, Interface, Result, implement};

#[implement(ITfEditSession)]
pub(super) struct StartComposition {
    context: ITfContext,
    composition_sink: ITfCompositionSink,
    composition: OnceCell<ITfComposition>,
}

impl StartComposition {
    pub(super) fn new(
        context: ITfContext,
        composition_sink: ITfCompositionSink,
    ) -> StartComposition {
        Self {
            context,
            composition_sink,
            composition: OnceCell::new(),
        }
    }
    pub(super) fn composition(&self) -> Option<&ITfComposition> {
        self.composition.get()
    }
}

impl ITfEditSession_Impl for StartComposition_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        let context_composition: ITfContextComposition = self.context.cast()?;
        let range = unsafe {
            let insert_at_selection: ITfInsertAtSelection = self.context.cast()?;
            insert_at_selection.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?
        };

        debug!("range = {range:?}");

        unsafe {
            // XXX even though MS document says pSink is optional,
            // StartComposition fails if NULL is passed.
            let composition =
                context_composition.StartComposition(ec, &range, &self.composition_sink);
            if let Err(error) = &composition {
                error!("unable to start composition: {error}");
            }
            // TODO test if we need to reset the selection as the remark in the original C++ code
            if let Err(error) = self.composition.set(composition?) {
                error!("unable to set composition: {error:?}");
            }
        }
        Ok(())
    }
}

#[implement(ITfEditSession)]
pub(super) struct EndComposition<'a> {
    context: &'a ITfContext,
    composition: &'a ITfComposition,
}

impl<'a> EndComposition<'a> {
    pub(super) fn new(
        context: &'a ITfContext,
        composition: &'a ITfComposition,
    ) -> EndComposition<'a> {
        Self {
            context,
            composition,
        }
    }
}

impl ITfEditSession_Impl for EndComposition_Impl<'_> {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let range = self.composition.GetRange()?;
            let disp_attr_prop = self.context.GetProperty(&GUID_PROP_ATTRIBUTE)?;
            disp_attr_prop.Clear(ec, &range)?;

            let mut selection = [TF_SELECTION::default(); 1];
            let mut selection_len = 0;
            self.context.GetSelection(
                ec,
                TF_DEFAULT_SELECTION,
                &mut selection,
                &mut selection_len,
            )?;
            if let Some(sel_range) = &selection[0].range.deref() {
                sel_range.ShiftEndToRange(ec, &range, TF_ANCHOR_END)?;
                sel_range.Collapse(ec, TF_ANCHOR_END)?;
                self.context.SetSelection(ec, &selection)?;
            }
            self.composition.EndComposition(ec)?;
        }
        Ok(())
    }
}

#[implement(ITfEditSession)]
pub(super) struct SetCompositionString<'a> {
    context: &'a ITfContext,
    composition: &'a ITfComposition,
    da_atom: VARIANT,
    text: &'a HSTRING,
    cursor: i32,
}

impl<'a> SetCompositionString<'a> {
    pub(super) fn new(
        context: &'a ITfContext,
        composition: &'a ITfComposition,
        da_atom: VARIANT,
        text: &'a HSTRING,
        cursor: i32,
    ) -> SetCompositionString<'a> {
        Self {
            context,
            composition,
            da_atom,
            text,
            cursor,
        }
    }
}

impl ITfEditSession_Impl for SetCompositionString_Impl<'_> {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let range = self.composition.GetRange()?;
            debug!("range {:?}", &range);
            if let Err(error) = range.SetText(ec, 0, self.text) {
                error!("set composition string failed: {error}");
            }
            let disp_attr_prop = self.context.GetProperty(&GUID_PROP_ATTRIBUTE)?;
            if let Err(error) = disp_attr_prop.SetValue(ec, &range, &self.da_atom) {
                error!("set display attribute failed: {error}");
            }

            let mut selection = [TF_SELECTION::default(); 1];
            let mut selection_len = 0;
            self.context.GetSelection(
                ec,
                TF_DEFAULT_SELECTION,
                &mut selection,
                &mut selection_len,
            )?;
            if let Some(sel_range) = &selection[0].range.deref() {
                sel_range.ShiftStartToRange(ec, &range, TF_ANCHOR_START)?;
                sel_range.Collapse(ec, TF_ANCHOR_START)?;
                let mut moved = 0;
                sel_range.ShiftStart(ec, self.cursor, &mut moved, ptr::null())?;
                sel_range.Collapse(ec, TF_ANCHOR_START)?;
                self.context.SetSelection(ec, &selection)?;
            }
        }
        Ok(())
    }
}

#[implement(ITfEditSession)]
pub(super) struct SelectionRect<'a> {
    context: &'a ITfContext,
    rect: OnceCell<RECT>,
}

impl<'a> SelectionRect<'a> {
    pub(super) fn new(context: &'a ITfContext) -> SelectionRect<'a> {
        Self {
            context,
            rect: OnceCell::new(),
        }
    }
    pub(super) fn rect(&self) -> Option<&RECT> {
        self.rect.get()
    }
}

impl ITfEditSession_Impl for SelectionRect_Impl<'_> {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        let mut selection = [TF_SELECTION::default(); 1];
        let mut selection_len = 0;
        unsafe {
            self.context.GetSelection(
                ec,
                TF_DEFAULT_SELECTION,
                &mut selection,
                &mut selection_len,
            )?;
            if let Some(sel_range) = &selection[0].range.deref() {
                let view = self.context.GetActiveView()?;
                let mut rc = RECT::default();
                let mut clipped = BOOL::default();
                view.GetTextExt(ec, sel_range, &mut rc, &mut clipped)?;
                if let Err(error) = self.rect.set(rc) {
                    error!("unable to set rect: {error:?}");
                }
            }
        }
        Ok(())
    }
}
