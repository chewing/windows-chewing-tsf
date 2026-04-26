use std::{
    error::Error,
    fmt::Display,
    io::{BufRead, BufReader, Write},
};

use chewing_tip_core::ipc::{
    messages::{CheckUpdate, HideCandidateList, ShowCandidateList, ShowNotification, Stop},
    varlink::MethodCall,
};
use exn::{Result, ResultExt};
use interprocess::os::windows::named_pipe::{PipeListener, PipeStream, pipe_mode::Bytes};
use log::{debug, error, warn};

use crate::{ui::event_loop::MainLoopHandle, update::check_for_update};

pub(crate) fn run_ipc_server(
    listener: PipeListener<Bytes, Bytes>,
    mh: MainLoopHandle,
) -> Result<(), IpcError> {
    let err = || IpcError("IPC server failed".to_string());

    for pipe in listener.incoming() {
        let mh = mh.clone();
        let pipe = pipe.or_raise(err)?;
        std::thread::spawn(move || {
            run_ipc_loop(pipe, mh);
        });
    }
    Ok(())
}

fn run_ipc_loop(pipe: PipeStream<Bytes, Bytes>, mh: MainLoopHandle) {
    let (receiver_inner, mut sender) = pipe.split();
    let mut receiver = BufReader::new(receiver_inner);
    loop {
        let mut buffer = vec![];
        if let Err(error) = receiver.read_until(0, &mut buffer) {
            error!("Failed to read from pipe: {error:?}");
            continue;
        }
        if buffer.last().is_none_or(|b| *b != 0) {
            debug!("EOF - exit IPC loop");
            break;
        }

        buffer.pop();
        match serde_json::from_slice::<MethodCall>(&buffer) {
            Ok(call) => {
                let oneway = call.oneway.is_some_and(|v| v);

                match call.method.as_str() {
                    ShowNotification::METHOD
                    | ShowCandidateList::METHOD
                    | HideCandidateList::METHOD
                    | Stop::METHOD => {
                        if let Err(error) = mh.send(call) {
                            error!("Failed to dispatch message: {error:?}");
                            continue;
                        }
                    }
                    CheckUpdate::METHOD => {
                        check_for_update();
                    }
                    _ => {
                        warn!("Unknown method: {call:?}");
                    }
                }

                if !oneway {
                    let reply = c"{}";
                    if let Err(error) = sender.write_all(reply.to_bytes_with_nul()) {
                        error!("Failed to reply varlink message: {error:?}");
                        continue;
                    }
                }
            }
            Err(error) => {
                error!("Failed to parse varlink message: {error:?}");
                continue;
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct IpcError(String);
impl Error for IpcError {}
impl Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IpcError: {}", self.0)
    }
}
