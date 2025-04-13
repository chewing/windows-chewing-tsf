mod chewing;
mod config;
mod display_attribute;
mod edit_session;
mod key_event;
mod lang_bar;
mod menu;
mod resources;

use std::{
    cell::Cell,
    sync::{RwLock, RwLockWriteGuard},
};

use display_attribute::{EnumTfDisplayAttributeInfo, get_display_attribute_info};
use log::info;
use windows::Win32::{
    Foundation::{FALSE, LPARAM, WPARAM},
    UI::TextServices::*,
};
use windows_core::{
    BOOL, BSTR, ComObjectInterface, GUID, IUnknown, IUnknown_Vtbl, Interface, InterfaceRef, Ref,
    Result, implement, interface,
};

use self::chewing::ChewingTextService;
use self::key_event::KeyEvent;

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
    ITfCompositionSink,
    ITfKeyEventSink,
    ITfTextEditSink,
    ITfTextInputProcessorEx,
    ITfThreadMgrEventSink,
    ITfDisplayAttributeProvider,
    ITfFunctionProvider,
    IFnRunCommand
)]
pub(super) struct TextService {
    inner: RwLock<ChewingTextService>,
    tid: Cell<u32>,
    thread_mgr_sink_cookie: Cell<u32>,
}

impl TextService {
    pub(super) fn new() -> TextService {
        TextService {
            inner: RwLock::new(ChewingTextService::new()),
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
        let mut ts = self.lock();
        ts.on_command(id, cmd_type);
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        info!("Activate chewing_tip");
        self.tid.set(tid);
        let mut ts = self.lock();
        let thread_mgr = ptim.ok()?;
        let composition_sink = self.as_interface_ref();
        ts.activate(thread_mgr, tid, composition_sink)?;

        let punk: InterfaceRef<IUnknown> = self.as_interface_ref();
        // Set up event sinks
        unsafe {
            let source: ITfSource = thread_mgr.cast()?;
            self.thread_mgr_sink_cookie
                .set(source.AdviseSink(&ITfThreadMgrEventSink::IID, self.as_interface_ref())?);
            let source_single: ITfSourceSingle = thread_mgr.cast()?;
            source_single.AdviseSingleSink(tid, &ITfFunctionProvider::IID, punk)?;
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            keystroke_mgr.AdviseKeyEventSink(tid, self.as_interface_ref(), true)?;
        }

        Ok(())
    }

    fn Deactivate(&self) -> Result<()> {
        info!("Deactivate chewing_tip");
        let mut ts = self.lock();
        let thread_mgr = ts.deactivate()?;

        // Remove event sinks
        unsafe {
            let source: ITfSource = thread_mgr.cast()?;
            source.UnadviseSink(self.thread_mgr_sink_cookie.replace(TF_INVALID_COOKIE))?;
            let source_single: ITfSourceSingle = thread_mgr.cast()?;
            source_single.UnadviseSingleSink(self.tid.get(), &ITfFunctionProvider::IID)?;
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
        _pic: Ref<ITfContext>,
        _ecreadonly: u32,
        _peditrecord: Ref<ITfEditRecord>,
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
        let ev = KeyEvent::new(wparam.0 as u16, lparam.0);
        let should_handle = ts.on_keydown(pic.ok()?, ev, true)?;
        Ok(should_handle.into())
    }

    fn OnTestKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let mut ts = self.lock();
        let ev = KeyEvent::new(wparam.0 as u16, lparam.0);
        let should_handle = ts.on_keyup(pic.ok()?, ev, true)?;
        Ok(should_handle.into())
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let mut ts = self.lock();
        let ev = KeyEvent::new(wparam.0 as u16, lparam.0);
        let handled = ts.on_keydown(pic.ok()?, ev, false)?;
        Ok(handled.into())
    }

    fn OnKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let mut ts = self.lock();
        let ev = KeyEvent::new(wparam.0 as u16, lparam.0);
        let handled = ts.on_keyup(pic.ok()?, ev, false)?;
        Ok(handled.into())
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, rguid: *const GUID) -> Result<BOOL> {
        if let Some(rguid) = unsafe { rguid.as_ref() } {
            let mut ts = self.lock();
            let handled = ts.on_preserved_key(rguid);
            Ok(handled.into())
        } else {
            Ok(FALSE)
        }
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
        let mut ts = self.lock();
        ts.on_composition_terminated();
        Ok(())
    }
}
