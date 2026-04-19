use std::{
    error::Error,
    fmt::Display,
    io::{BufRead, BufReader, Write},
};

use chewing_tip_core::ipc::{
    messages::{HideCandidateList, ShowCandidateList, ShowNotification},
    named_pipe::{NAMED_PIPE_PATH, create_pipe_listener},
    varlink::MethodCall,
};
use exn::{Result, ResultExt};
use interprocess::os::windows::named_pipe::{PipeStream, pipe_mode::Bytes};
use log::{debug, error, info, warn};

use crate::ui::event_loop::MainLoopHandle;

pub(crate) fn run_ipc_server(mh: MainLoopHandle) -> Result<(), IpcError> {
    let err = || IpcError("IPC server failed".to_string());

    info!("Server is listening at {}", NAMED_PIPE_PATH);
    let listener = create_pipe_listener().or_raise(err)?;
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
            Ok(call) => match call.method.as_str() {
                ShowNotification::METHOD
                | ShowCandidateList::METHOD
                | HideCandidateList::METHOD => {
                    let oneway = call.oneway.is_some_and(|v| v);
                    if let Err(error) = mh.send(call) {
                        error!("Failed to dispatch message: {error:?}");
                        continue;
                    }
                    if !oneway {
                        let reply = c"{}";
                        if let Err(error) = sender.write_all(reply.to_bytes_with_nul()) {
                            error!("Failed to reply varlink message: {error:?}");
                            continue;
                        }
                    }
                }
                _ => {
                    warn!("Unknown method: {call:?}");
                }
            },
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
