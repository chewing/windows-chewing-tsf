mod chewing;
mod config;
mod display_attribute;
mod key_event;
mod lang_bar;
mod resources;

use std::{cell::Cell, sync::{RwLock, RwLockWriteGuard}};

use display_attribute::{
    EnumTfDisplayAttributeInfo, get_display_attribute_info, register_display_attribute,
};
use windows::Win32::{
    Foundation::{FALSE, LPARAM, WPARAM},
    UI::TextServices::*,
};
use windows_core::{implement, ComObjectInterface, Interface, Ref, Result, BOOL, GUID};

use self::chewing::ChewingTextService;
use self::key_event::KeyEvent;

const GUID_INPUT_DISPLAY_ATTRIBUTE: GUID = GUID::from_u128(0xEEA32958_DC57_4542_9FC833C74F5CAAA9);

#[implement(
    ITfCompositionSink,
    ITfKeyEventSink,
    ITfTextEditSink,
    ITfTextInputProcessorEx,
    ITfThreadMgrEventSink,
    ITfDisplayAttributeProvider
)]
struct TextService {
    inner: RwLock<ChewingTextService>,
    input_da_atom: u32,
    tid: Cell<u32>,
    thread_mgr_sink_cookie: Cell<u32>,
}

impl TextService {
    fn new() -> TextService {
        let da = TF_DISPLAYATTRIBUTE {
            lsStyle: TF_LS_DOT,
            bAttr: TF_ATTR_INPUT,
            ..Default::default()
        };
        let input_da_atom = register_display_attribute(&GUID_INPUT_DISPLAY_ATTRIBUTE, da)
            .expect("unable to register display attribute");
        TextService {
            inner: RwLock::new(ChewingTextService::new()),
            input_da_atom,
            tid: Cell::default(),
            thread_mgr_sink_cookie: Cell::new(TF_INVALID_COOKIE),
        }
    }
    #[track_caller]
    fn lock(&self) -> RwLockWriteGuard<'_, ChewingTextService> {
        self.inner
            .write()
            .expect("failed to acquire lock on chewing_tip")
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

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        self.tid.set(tid);
        let mut ts = self.lock();
        let thread_mgr = ptim.ok()?;
        ts.activate(thread_mgr, tid)?;

        // Set up event sinks

        unsafe {
            let source: ITfSource = thread_mgr.cast()?;
            self.thread_mgr_sink_cookie.set(source.AdviseSink(&ITfThreadMgrEventSink::IID, self.as_interface_ref())?);
        }

        unsafe {
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            keystroke_mgr.AdviseKeyEventSink(tid, self.as_interface_ref(), true)?;
        }

        Ok(())
    }

    fn Deactivate(&self) -> Result<()> {
        let mut ts = self.lock();
        let thread_mgr = ts.deactivate()?;

        // Remove event sinks

        unsafe {
            let source: ITfSource = thread_mgr.cast()?;
            source.UnadviseSink(self.thread_mgr_sink_cookie.replace(TF_INVALID_COOKIE))?;
        }

        unsafe {
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            keystroke_mgr.UnadviseKeyEventSink(self.tid.get())?;
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
        _pdimprevfocus: Ref<ITfDocumentMgr>,
    ) -> Result<()> {
        let mut ts = self.lock();
        if pdimfocus.is_null() {
            ts.on_kill_focus()?;
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

impl ITfTextEditSink_Impl for TextService_Impl {
    fn OnEndEdit(
        &self,
        pic: Ref<ITfContext>,
        ecreadonly: u32,
        peditrecord: Ref<ITfEditRecord>,
    ) -> Result<()> {
        // TODO
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let mut ts = self.lock();
        let ev = KeyEvent::down(wparam.0, lparam.0);
        let should_handle = ts.on_keydown(ev, true);
        Ok(should_handle.into())
    }

    fn OnTestKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let mut ts = self.lock();
        let ev = KeyEvent::up(wparam.0, lparam.0);
        let should_handle = ts.on_keyup(ev, true);
        Ok(should_handle.into())
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let mut ts = self.lock();
        let ev = KeyEvent::down(wparam.0, lparam.0);
        let handled = ts.on_keydown(ev, false);
        Ok(handled.into())
    }

    fn OnKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let mut ts = self.lock();
        let ev = KeyEvent::up(wparam.0, lparam.0);
        let handled = ts.on_keyup(ev, false);
        Ok(handled.into())
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, rguid: *const GUID) -> Result<BOOL> {
        if rguid.is_null() {
            return Ok(FALSE);
        }
        let mut ts = self.lock();
        let handled = ts.on_preserved_key(unsafe { rguid.as_ref() }.unwrap());
        Ok(handled.into())
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Ref<ITfComposition>,
    ) -> Result<()> {
        // This is called by TSF when our composition is terminated by others.
        // For example, when the user click on another text editor and the input focus is
        // grabbed by others, we're ``forced'' to terminate current composition.
        // If we end the composition by calling ITfComposition::EndComposition() ourselves,
        // this event is not triggered.
        //
        // TODO
        Ok(())
    }
}
