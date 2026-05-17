use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Write},
    rc::Rc,
    time::Duration,
};

use error_plus::{expect_error, impl_context_error};
use interprocess::{
    TryClone,
    os::windows::named_pipe::{DuplexPipeStream, pipe_mode::Bytes},
};

use crate::ipc::{
    messages::{Ping, PingReply},
    named_pipe::{connect_and_attest, named_pipe_path},
    varlink::{MethodCall, MethodReply},
};

#[derive(Clone, Default)]
pub struct ChewingIpcClient {
    pipe: Rc<RefCell<Option<DuplexPipeStream<Bytes>>>>,
}

impl ChewingIpcClient {
    pub fn new() -> ChewingIpcClient {
        Self::default()
    }
    pub fn connect(&self) -> Result<(), IpcOpError> {
        expect_error("Unable to connect to chewing_tip_host", || {
            let pipe_path = named_pipe_path()?;
            let pipe = connect_and_attest(&pipe_path, Duration::from_millis(100))?;
            self.pipe.replace(Some(pipe));
            Ok(())
        })
    }
    pub fn send(&self, method_call: MethodCall) -> Result<MethodReply, IpcOpError> {
        expect_error("Failed to call IPC method", || {
            let mut bytes = serde_json::to_vec(&method_call)?;
            bytes.push(0);
            self.pipe
                .try_borrow_mut()?
                .as_mut()
                .map(|pipe| pipe.write_all(&bytes))
                .ok_or("IPC Client was not connected")??;
            if matches!(method_call.oneway, Some(true)) {
                return Ok(MethodReply {
                    parameters: serde_json::Value::Null,
                    continues: None,
                    error: None,
                });
            }
            let mut buffer = vec![];
            let mut reader = BufReader::new(
                self.pipe
                    .try_borrow()?
                    .as_ref()
                    .ok_or("Broken Pipe")?
                    .try_clone()?,
            );
            reader.read_until(0, &mut buffer)?;
            if buffer.last().is_none_or(|b| *b != 0) {
                log::debug!("EOF - server exited");
                return Err("EOF - server exited".into());
            }
            buffer.pop();
            Ok(serde_json::from_slice::<MethodReply>(&buffer)?)
        })
    }
    pub fn ping(&self) -> Result<String, IpcOpError> {
        expect_error("Cannot ping server", || {
            let reply = self.send(MethodCall {
                method: Ping::METHOD.to_string(),
                parameters: serde_json::to_value(Ping::new())?,
                oneway: Some(false),
                more: Some(false),
                upgrade: Some(false),
            })?;
            let params: PingReply = serde_json::from_value(reply.parameters)?;
            Ok(params.uuid)
        })
    }
}

impl Drop for ChewingIpcClient {
    fn drop(&mut self) {
        if let Some(pipe) = self.pipe.borrow_mut().as_mut() {
            let _ = pipe.flush();
        }
    }
}

impl_context_error!(pub IpcOpError);
