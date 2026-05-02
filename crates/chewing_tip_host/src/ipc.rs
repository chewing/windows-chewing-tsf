use std::{
    io::{BufRead, BufReader, Write},
    ops::ControlFlow,
};

use chewing_tip_core::{
    impl_context_error,
    ipc::{
        messages::{
            CheckUpdate, HideCandidateList, OnKeyDownReply, OnTestKeyDown, OnTestKeyDownReply,
            ShowCandidateList, ShowNotification, Stop,
        },
        varlink::{MethodCall, MethodReply},
    },
    result::{Report, expect_error},
};
use interprocess::os::windows::named_pipe::{PipeListener, PipeStream, pipe_mode::Bytes};
use log::{debug, error, warn};

use crate::{
    text_service::chewing::TipSession, ui::event_loop::MainLoopHandle, update::check_for_update,
};

pub(crate) fn run_ipc_listener(
    listener: PipeListener<Bytes, Bytes>,
    mh: MainLoopHandle,
) -> Result<(), HandleIpcError> {
    expect_error("IPC listener failed", || {
        for pipe in listener.incoming() {
            let mh = mh.clone();
            let pipe = pipe?;
            std::thread::spawn(move || {
                ipc_loop(pipe, mh);
            });
        }
        Ok(())
    })
}

fn ipc_loop(pipe: PipeStream<Bytes, Bytes>, mh: MainLoopHandle) {
    let (receiver_inner, mut sender) = pipe.split();
    let mut receiver = BufReader::new(receiver_inner);
    let mut tip_session = TipSession::new();
    loop {
        match ipc_loop_once(&mut receiver, &mut sender, &mh, &mut tip_session) {
            Ok(ControlFlow::Continue(_)) => continue,
            Ok(ControlFlow::Break(_)) => break,
            Err(error) => {
                error!("{}", Report(&error))
                // FIXME reply with errors if not oneway
            }
        }
    }
}

fn ipc_loop_once(
    mut receiver: impl BufRead,
    mut sender: impl Write,
    mh: &MainLoopHandle,
    tip_session: &mut TipSession,
) -> Result<ControlFlow<()>, HandleIpcError> {
    expect_error("Failed to handle one IPC message", || {
        let mut buffer = vec![];
        receiver.read_until(0, &mut buffer)?;
        if buffer.pop().is_none_or(|b| b != 0) {
            debug!("EOF - exit IPC loop");
            return Ok(ControlFlow::Break(()));
        }
        let call = serde_json::from_slice::<MethodCall>(&buffer)?;
        let oneway = call.oneway.is_some_and(|v| v);

        match call.method.as_str() {
            ShowNotification::METHOD
            | ShowCandidateList::METHOD
            | HideCandidateList::METHOD
            | Stop::METHOD => {
                mh.send(call)?;
                if !oneway {
                    sender.write_all(c"{}".to_bytes_with_nul())?;
                }
            }
            CheckUpdate::METHOD => {
                check_for_update();
                if !oneway {
                    sender.write_all(c"{}".to_bytes_with_nul())?;
                }
            }
            OnTestKeyDown::METHOD => {
                let params: OnTestKeyDown = serde_json::from_value(call.parameters)?;
                let handled = tip_session.on_test_keydown(
                    params.is_context_mutable,
                    params.is_composing,
                    params.shift_key_state,
                    params.event.try_into()?,
                )?;
                let reply = MethodReply {
                    parameters: serde_json::to_value(OnTestKeyDownReply { handled })?,
                    continues: None,
                    error: None,
                };
                if !oneway {
                    sender.write_all(&reply.to_bytes()?)?;
                }
            }
            _ => {
                warn!("Unknown method: {call:?}");
            }
        }
        Ok(ControlFlow::Continue(()))
    })
}

impl_context_error!(HandleIpcError);
