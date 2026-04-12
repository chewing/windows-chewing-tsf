use std::{
    rc::Rc,
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    time::Duration,
};

use chewing_tip_core::ipc::{
    messages::{ShowCandidateList, ShowNotification},
    varlink::MethodCall,
};
use exn::{Result, ResultExt};
use log::{debug, error, warn};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
        TranslateMessage, WM_APP,
    },
};
use windows_core::HSTRING;

use crate::{
    ui::{UiError, gfx::color_s, window::window_register_class},
    ui_elements::{
        candidate_list::{CandidateList, CandidateListModel},
        notification::{Notification, NotificationModel},
    },
};

const PM_APP_COMMAND: u32 = WM_APP + 1;

/// The main UI event loop
#[derive(Debug)]
pub(crate) struct MainLoop {
    receiver: Receiver<MethodCall>,
    sender: SyncSender<MethodCall>,

    candidate_list: Rc<CandidateList>,
    notification: Rc<Notification>,
}

impl MainLoop {
    pub(crate) fn new() -> MainLoop {
        let mut msg = MSG::default();
        unsafe {
            // Initialize the message queue
            let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        }
        // FIXME
        let hinst = unsafe { GetModuleHandleW(None).unwrap_or_default() };
        let _ = Notification::window_register_class(hinst.into());
        let _ = CandidateList::window_register_class(hinst.into());
        let _ = window_register_class(hinst.into());

        let notification = Notification::new().expect("failed to create notification window");
        let candidate_list = CandidateList::new().expect("failed to create candidate list window");

        let (sender, receiver) = sync_channel(130);
        MainLoop {
            receiver,
            sender,
            notification,
            candidate_list,
        }
    }
    pub(crate) fn get_handle(&self) -> MainLoopHandle {
        let main_thread_id = unsafe { GetCurrentThreadId() };
        let sender = self.sender.clone();
        MainLoopHandle {
            main_thread_id,
            sender,
        }
    }
    pub(crate) fn run(&mut self) {
        unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if msg.hwnd.is_invalid() && msg.message == PM_APP_COMMAND {
                    self.command_loop();
                    continue;
                }
                let _ = TranslateMessage(&msg);
                let _ = DispatchMessageW(&msg);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MainLoopHandle {
    main_thread_id: u32,
    sender: SyncSender<MethodCall>,
}

impl MainLoopHandle {
    pub(crate) fn send(&self, msg: MethodCall) -> Result<(), UiError> {
        let err = || UiError("failed to send message to main loop".to_string());
        unsafe {
            PostThreadMessageW(self.main_thread_id, PM_APP_COMMAND, WPARAM(0), LPARAM(0))
                .or_raise(err)?;
        }
        self.sender.send(msg).or_raise(err)?;
        Ok(())
    }
}

impl MainLoop {
    fn command_loop(&mut self) {
        for cmd in self.receiver.try_iter() {
            // TODO skip duplicate command types
            debug!("handle an IPC command {cmd:?} in the main loop");
            if let Err(error) = self.process(cmd) {
                error!("{error:?}");
            }
        }
    }
    fn process(&self, cmd: MethodCall) -> Result<(), UiError> {
        let err = || UiError(format!("failed to handle IPC command"));
        match cmd.method.as_str() {
            ShowNotification::METHOD => {
                let params: ShowNotification =
                    serde_json::from_value(cmd.parameters).or_raise(err)?;
                self.notification.set_model(NotificationModel {
                    text: HSTRING::from(params.text),
                    font_family: HSTRING::from(params.font_family),
                    font_size: params.font_size,
                    fg_color: color_s(&params.fg_color),
                    bg_color: color_s(&params.bg_color),
                    border_color: color_s(&params.border_color),
                });
                self.notification
                    .set_position(params.position.x, params.position.y);
                self.notification.show();
                self.notification.set_timer(Duration::from_millis(500));
            }
            ShowCandidateList::METHOD => {
                self.candidate_list.show();
            }
            _ => {
                warn!("Unknown method: {cmd:?}");
            }
        }
        Ok(())
    }
}
