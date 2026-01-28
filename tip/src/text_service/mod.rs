// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::{Cell, RefCell};

use log::{debug, error};
use windows::Win32::{
    Foundation::{E_UNEXPECTED, FALSE, LPARAM, WPARAM},
    UI::TextServices::*,
};
use windows_core::{
    BOOL, BSTR, ComObjectInterface, GUID, IUnknown, IUnknown_Vtbl, Interface, InterfaceRef, Ref,
    Result, implement, interface,
};

use crate::text_service::chewing::ReentrantOps;

use self::chewing::ChewingTextService;
use self::display_attribute::{EnumTfDisplayAttributeInfo, get_display_attribute_info};
use self::key_event::SystemKeyboardEvent;

mod chewing;
mod display_attribute;
mod edit_session;
mod key_event;
mod lang_bar;
mod menu;
mod resources;
mod theme;
mod ui_elements;

const CHEWING_TSF_CLSID: GUID = GUID::from_u128(0x13F2EF08_575C_4D8C_88E0_F67BB8052B84);
const GUID_INPUT_DISPLAY_ATTRIBUTE: GUID = GUID::from_u128(0xEEA32958_DC57_4542_9FC833C74F5CAAA9);

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub(super) enum CommandType {
    LeftClick,
    RightClick,
    Menu,
}

#[interface("f320f835-b95d-4d3f-89d5-fd4ab7b9d7bb")]
pub(super) unsafe trait IFnRunCommand: IUnknown {
    fn on_command(&self, id: u32, cmd_type: CommandType);
}

#[implement(
    IFnRunCommand,
    ITfCompartmentEventSink,
    ITfCompositionSink,
    ITfDisplayAttributeProvider,
    ITfFunctionProvider,
    ITfKeyEventSink,
    ITfTextInputProcessorEx,
    ITfThreadMgrEventSink,
    ITfThreadFocusSink
)]
pub(super) struct TextService {
    inner: RefCell<Option<ChewingTextService>>,
    tid: Cell<u32>,
    thread_cookies: RefCell<Vec<u32>>,
    keyboard_openclose_cookie: Cell<u32>,
    key_busy: Cell<bool>,
}

impl TextService {
    pub(super) fn new() -> TextService {
        TextService {
            inner: RefCell::default(),
            tid: Cell::default(),
            thread_cookies: RefCell::new(vec![]),
            keyboard_openclose_cookie: Cell::new(TF_INVALID_COOKIE),
            key_busy: Cell::new(false),
        }
    }
}

// XXX MS document says "The TSF manager obtains an instance of this
// interface by calling CoCreateInstance with the class identifier
// passed to ITfCategoryMgr::RegisterCategory with GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER
// and IID_ITfDisplayAttributeProvider. For more information, see
// Providing Display Attributes." However, in practice the DisplayAttributeMgr
// directly queries the text service object for the interface, so we need
// to handle the query interface here.
impl ITfDisplayAttributeProvider_Impl for TextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(EnumTfDisplayAttributeInfo::default().into())
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        get_display_attribute_info(guid)
    }
}

impl ITfFunctionProvider_Impl for TextService_Impl {
    fn GetType(&self) -> Result<GUID> {
        Ok(GUID::zeroed())
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("Chewing TSF Function Provider"))
    }

    fn GetFunction(&self, _rguid: *const GUID, riid: *const GUID) -> Result<IUnknown> {
        if !riid.is_null() && unsafe { *riid } == IFnRunCommand::IID {
            let punk: InterfaceRef<IUnknown> = self.as_interface_ref();
            return Ok(punk.to_owned());
        }
        Err(TS_E_NOINTERFACE.into())
    }
}

impl IFnRunCommand_Impl for TextService_Impl {
    unsafe fn on_command(&self, id: u32, cmd_type: CommandType) {
        let mut borrowed_ts = self.inner.borrow_mut();
        let Some(ts) = borrowed_ts.as_mut() else {
            return;
        };
        ts.on_command(id, cmd_type);
        let reentrant_ops = ReentrantOps::from_mut(&self.inner, borrowed_ts);
        if let Err(error) = reentrant_ops.sync_keyboard_openclose(false) {
            error!("Unable to sync lang mode: {error:#}");
        }
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        debug!(tid; "tip::activate");
        self.tid.set(tid);
        let mut ts = self.inner.borrow_mut();
        let mut thread_cookies = self.thread_cookies.borrow_mut();
        let thread_mgr = ptim.ok()?;
        let composition_sink: InterfaceRef<ITfCompositionSink> = self.as_interface_ref();
        let Ok(cts) =
            ChewingTextService::new(thread_mgr.clone(), tid, composition_sink.cast_object()?)
        else {
            return Err(E_UNEXPECTED.into());
        };
        ts.replace(cts);

        let punk: InterfaceRef<IUnknown> = self.as_interface_ref();
        // Set up event sinks
        unsafe {
            let source: ITfSource = thread_mgr.cast()?;
            thread_cookies
                .push(source.AdviseSink(&ITfThreadMgrEventSink::IID, self.as_interface_ref())?);
            thread_cookies
                .push(source.AdviseSink(&ITfThreadFocusSink::IID, self.as_interface_ref())?);
            let source_single: ITfSourceSingle = thread_mgr.cast()?;
            if let Err(error) = source_single.AdviseSingleSink(tid, &ITfFunctionProvider::IID, punk)
            {
                error!("Unable to register function provider: {error:#}");
            }
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            if let Err(error) = keystroke_mgr.AdviseKeyEventSink(tid, self.as_interface_ref(), true)
            {
                error!("Unable to register key event sink: {error:#}");
            }
            let compartment_mgr: ITfCompartmentMgr = thread_mgr.cast()?;
            let openclose_compartment =
                compartment_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)?;
            let source: ITfSource = openclose_compartment.cast()?;
            self.keyboard_openclose_cookie
                .set(source.AdviseSink(&ITfCompartmentEventSink::IID, self.as_interface_ref())?);
        }

        Ok(())
    }

    fn Deactivate(&self) -> Result<()> {
        debug!("tip::deactivate");
        let thread_cookies = self.thread_cookies.take();

        if let Some(ts) = self.inner.borrow_mut().take() {
            let thread_mgr = ts.deactivate();

            // Remove event sinks
            unsafe {
                let source: ITfSource = thread_mgr.cast()?;
                for cookie in thread_cookies {
                    source.UnadviseSink(cookie)?;
                }
                let source_single: ITfSourceSingle = thread_mgr.cast()?;
                source_single.UnadviseSingleSink(self.tid.get(), &ITfFunctionProvider::IID)?;
                let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
                keystroke_mgr.UnadviseKeyEventSink(self.tid.get())?;
                let compartment_mgr: ITfCompartmentMgr = thread_mgr.cast()?;
                let openclose_compartment =
                    compartment_mgr.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)?;
                let source: ITfSource = openclose_compartment.cast()?;
                source.UnadviseSink(self.keyboard_openclose_cookie.get())?;
            }
        }
        Ok(())
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    fn ActivateEx(&self, ptim: Ref<ITfThreadMgr>, tid: u32, _dwflags: u32) -> Result<()> {
        self.Activate(ptim, tid)
    }
}

impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnUninitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnSetFocus(
        &self,
        pdimfocus: Ref<ITfDocumentMgr>,
        pdimprevfocus: Ref<ITfDocumentMgr>,
    ) -> Result<()> {
        debug!(
            focus = !pdimfocus.is_null(),
            prevfocus = !pdimprevfocus.is_null(); "on_set_focus"
        );
        // Excel switches document upon first key down. Skip this superflos
        // focus change.
        if self.key_busy.get() {
            return Ok(());
        }
        let mut borrowed_ts = self.inner.borrow_mut();
        let Some(ts) = borrowed_ts.as_mut() else {
            return Ok(());
        };
        if pdimfocus.is_null() {
            if let Some(doc_mgr) = pdimprevfocus.as_ref() {
                // From MSTF doc: To simplify this process and prevent
                // multiple modal UIs from being displayed, there is a maximum
                // of two contexts allowed on the stack.
                //
                // XXX: We don't push contexts, so there should always only one
                // context. It doesn't matter we get the Top or the Base.
                let context = unsafe { doc_mgr.GetBase()? };
                if let Err(error) = ts.on_kill_focus(&context) {
                    error!("Unable to kill focus: {error:#}");
                    return Err(E_UNEXPECTED.into());
                }
            }
        } else if pdimfocus.is_some()
            && let Err(error) = ts.on_focus()
        {
            error!("Unable to handle focus: {error:#}");
            return Err(E_UNEXPECTED.into());
        }
        Ok(())
    }

    fn OnPushContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }

    fn OnPopContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }
}

impl ITfThreadFocusSink_Impl for TextService_Impl {
    fn OnSetThreadFocus(&self) -> Result<()> {
        debug!("on_set_thread_focus");
        let mut borrowed_ts = self.inner.borrow_mut();
        let Some(ts) = borrowed_ts.as_mut() else {
            return Ok(());
        };
        if let Err(error) = ts.on_focus() {
            error!("Unable to handle focus: {error:#}");
            return Err(E_UNEXPECTED.into());
        }
        let reentrant_ops = ReentrantOps::from_mut(&self.inner, borrowed_ts);
        if let Err(error) = reentrant_ops.sync_keyboard_openclose(false) {
            error!("Unable to sync lang mode: {error:#}");
            return Err(E_UNEXPECTED.into());
        }
        Ok(())
    }
    fn OnKillThreadFocus(&self) -> Result<()> {
        debug!("on_kill_thread_focus");
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        debug!(wparam:?, lparam:?; "on_test_keydown");
        let mut borrowed_ts = self.inner.borrow_mut();
        let Some(ts) = borrowed_ts.as_mut() else {
            return Ok(FALSE);
        };
        let ev = SystemKeyboardEvent::new(wparam.0 as u16, lparam.0);
        let should_handle = match ts.on_test_keydown(pic.ok()?, ev) {
            Ok(v) => v,
            Err(error) => {
                error!("Unable to handle OnTestKeyDown: {error:#}");
                return Err(E_UNEXPECTED.into());
            }
        };
        Ok(should_handle.into())
    }

    fn OnTestKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        debug!(wparam:?, lparam:?; "on_test_keyup");
        let mut borrowed_ts = self.inner.borrow_mut();
        let Some(ts) = borrowed_ts.as_mut() else {
            return Ok(FALSE);
        };
        let ev = SystemKeyboardEvent::new(wparam.0 as u16, lparam.0);
        let should_handle = match ts.on_test_keyup(pic.ok()?, ev) {
            Ok(v) => v,
            Err(error) => {
                error!("Unable to handle OnTestKeyUp: {error:#}");
                return Err(E_UNEXPECTED.into());
            }
        };
        let reentrant_ops = ReentrantOps::from_mut(&self.inner, borrowed_ts);
        if let Err(error) = reentrant_ops.sync_keyboard_openclose(false) {
            error!("Unable to sync lang mode: {error:#}");
            return Err(E_UNEXPECTED.into());
        }
        Ok(should_handle.into())
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        debug!(wparam:?, lparam:?; "on_keydown");
        self.key_busy.set(true);
        let mut borrowed_ts = self.inner.borrow_mut();
        let Some(ts) = borrowed_ts.as_mut() else {
            return Ok(FALSE);
        };
        let ev = SystemKeyboardEvent::new(wparam.0 as u16, lparam.0);
        let handled = match ts.on_keydown(pic.ok()?, ev) {
            Ok(v) => v,
            Err(error) => {
                error!("Unable to handle OnKeyDown: {error:#}");
                return Err(E_UNEXPECTED.into());
            }
        };
        Ok(handled.into())
    }

    fn OnKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        debug!(wparam:?, lparam:?; "on_keyup");
        self.key_busy.set(false);
        let mut borrowed_ts = self.inner.borrow_mut();
        let Some(ts) = borrowed_ts.as_mut() else {
            return Ok(FALSE);
        };
        let ev = SystemKeyboardEvent::new(wparam.0 as u16, lparam.0);
        let handled = match ts.on_keyup(pic.ok()?, ev) {
            Ok(v) => v,
            Err(error) => {
                error!("Unable to handle OnKeyUp: {error:#}");
                return Err(E_UNEXPECTED.into());
            }
        };
        let reentrant_ops = ReentrantOps::from_mut(&self.inner, borrowed_ts);
        if let Err(error) = reentrant_ops.sync_keyboard_openclose(false) {
            error!("Unable to sync lang mode: {error:#}");
            return Err(E_UNEXPECTED.into());
        }
        Ok(handled.into())
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(FALSE)
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    fn OnCompositionTerminated(
        &self,
        ecwrite: u32,
        pcomposition: Ref<ITfComposition>,
    ) -> Result<()> {
        debug!("on_composition_terminated");
        // This is called by TSF when our composition is terminated by others.
        // For example, when the user click on another text editor and the input focus is
        // grabbed by others, we're ``forced'' to terminate current composition.
        // If we end the composition by calling ITfComposition::EndComposition() ourselves,
        // this event is not triggered.
        let Ok(mut borrowed_ts) = self.inner.try_borrow_mut() else {
            // Some EditSession can trigger reentrant. It should be safe to ignore.
            // TODO: handle this without conflict
            debug!("on_composition_terminated reentrant detected - abort");
            return Ok(());
        };
        let Some(ts) = borrowed_ts.as_mut() else {
            return Ok(());
        };
        if let Some(composition) = pcomposition.as_ref()
            && let Err(error) = ts.on_composition_terminated(ecwrite, composition)
        {
            error!("failed to properly terminate composition: {error:#}");
        }

        Ok(())
    }
}

impl ITfCompartmentEventSink_Impl for TextService_Impl {
    fn OnChange(&self, rguid: *const GUID) -> Result<()> {
        if let Some(rguid) = unsafe { rguid.as_ref() } {
            debug!(rguid:?; "compartment::on_change");
            let borrowed_ts = self.inner.borrow();
            let Some(ts) = borrowed_ts.as_ref() else {
                error!("text_service is not initialized");
                return Ok(());
            };
            if let Err(error) = ts.on_compartment_change(rguid) {
                error!("Unable to handle compartment change: {error:#}");
                return Err(E_UNEXPECTED.into());
            }
            if rguid == &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE {
                let reentrant_ops = ReentrantOps::from_ref(&self.inner, borrowed_ts);
                if let Err(error) = reentrant_ops.sync_keyboard_openclose(true) {
                    error!("Unable to sync lang mode: {error:#}");
                    return Err(E_UNEXPECTED.into());
                }
            }
        }
        Ok(())
    }
}
