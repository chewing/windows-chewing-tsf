// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::OnceCell;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ptr;

use log::{debug, error};
use windows::Win32::Foundation::{FALSE, RECT};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::TextServices::{
    GUID_PROP_ATTRIBUTE, INSERT_TEXT_AT_SELECTION_FLAGS, ITfComposition, ITfCompositionSink,
    ITfContext, ITfContextComposition, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection,
    ITfRange, TF_AE_END, TF_ANCHOR_END, TF_ANCHOR_START, TF_DEFAULT_SELECTION, TF_IAS_QUERYONLY,
    TF_SELECTION, TfActiveSelEnd,
};
use windows_core::{BOOL, HSTRING, Interface, Result, implement};

fn set_selection(
    context: &ITfContext,
    ec: u32,
    range: ITfRange,
    active_sel_end: TfActiveSelEnd,
) -> Result<()> {
    let mut selections = [TF_SELECTION::default(); 1];
    selections[0].range = ManuallyDrop::new(Some(range));
    selections[0].style.ase = active_sel_end;
    selections[0].style.fInterimChar = FALSE;
    let result = unsafe { context.SetSelection(ec, &selections) };
    let [TF_SELECTION { range, .. }] = selections;
    ManuallyDrop::into_inner(range);
    result
}

#[implement(ITfEditSession)]
pub(super) struct InsertText {
    context: ITfContext,
    text: HSTRING,
}

impl InsertText {
    pub(super) fn new(context: ITfContext, text: HSTRING) -> InsertText {
        Self { context, text }
    }
}

impl ITfEditSession_Impl for InsertText_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let insert_at_selection: ITfInsertAtSelection = self.context.cast()?;
            let range = insert_at_selection.InsertTextAtSelection(
                ec,
                INSERT_TEXT_AT_SELECTION_FLAGS(0),
                &self.text,
            )?;
            range.Collapse(ec, TF_ANCHOR_END)?;
            set_selection(&self.context, ec, range, TF_AE_END)?;
        }
        Ok(())
    }
}

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

            let new_composition_start = range.Clone()?;
            new_composition_start.Collapse(ec, TF_ANCHOR_END)?;
            self.composition.ShiftStart(ec, &new_composition_start)?;
            set_selection(&self.context, ec, new_composition_start, TF_AE_END)?;
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

            let cursor_range = range.Clone()?;
            let mut moved = 0;
            cursor_range.Collapse(ec, TF_ANCHOR_START)?;
            cursor_range.ShiftEnd(ec, self.cursor, &mut moved, ptr::null())?;
            cursor_range.ShiftStart(ec, self.cursor, &mut moved, ptr::null())?;
            set_selection(&self.context, ec, cursor_range, TF_AE_END)?;
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
