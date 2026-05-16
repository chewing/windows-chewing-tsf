use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Write},
    rc::Rc,
    thread,
    time::Duration,
};

use error_plus::{ErrorExt, expect_error, impl_context_error};
use interprocess::{
    TryClone,
    os::windows::named_pipe::{DuplexPipeStream, pipe_mode::Bytes},
};

use crate::{
    ipc::{
        named_pipe::{connect_and_attest, named_pipe_path},
        varlink::{MethodCall, MethodReply},
    },
    shell::launch_tip_host,
};

#[derive(Clone)]
pub struct ChewingIpcClient {
    pipe: Rc<RefCell<Option<DuplexPipeStream<Bytes>>>>,
}

impl ChewingIpcClient {
    pub fn connect_with_retry() -> ChewingIpcClient {
        let pipe = ChewingIpcClient::connect_pipe()
            .inspect_err(|error| {
                log::error!("{}", error.error_report());
            })
            .ok();
        ChewingIpcClient {
            pipe: Rc::new(RefCell::new(pipe)),
        }
    }
    pub fn connect() -> Result<ChewingIpcClient, IpcOpError> {
        expect_error("Unable to connect to chewing_tip_host", || {
            let pipe_path = named_pipe_path()?;
            let pipe = connect_and_attest(&pipe_path, Duration::from_millis(100))
                .inspect_err(|error| {
                    log::error!("{}", error.error_report());
                })
                .ok();
            Ok(ChewingIpcClient {
                pipe: Rc::new(RefCell::new(pipe)),
            })
        })
    }
    fn connect_pipe() -> Result<DuplexPipeStream<Bytes>, IpcOpError> {
        expect_error("Unable to connect to chewing_tip_host", || {
            let pipe_path = named_pipe_path()?;

            let res = connect_and_attest(&pipe_path, Duration::from_millis(100));
            if let Ok(pipe) = res {
                return Ok(pipe);
            }
            let error = res.unwrap_err();
            log::error!("Failed to connect to chewing_tip_host...");
            log::error!("{error:?}");
            log::error!("Trying to launch chewing_tip_host and retry...");
            launch_tip_host()?;
            for _ in 0..5 {
                thread::sleep(Duration::from_millis(100));
                let res = connect_and_attest(&pipe_path, Duration::from_millis(100));
                if let Ok(pipe) = res {
                    return Ok(pipe);
                }
            }
            log::error!("Failed to connect to chewing_tip_host...");
            log::error!("{error:?}");
            log::error!("Trying to launch chewing_tip_host and retry...");
            launch_tip_host()?;
            for _ in 0..10 {
                thread::sleep(Duration::from_millis(100));
                let res = connect_and_attest(&pipe_path, Duration::from_millis(100));
                if let Ok(pipe) = res {
                    return Ok(pipe);
                }
            }
            // FIXME
            Ok(connect_and_attest(&pipe_path, Duration::from_millis(100))?)
        })
    }
    fn reconnect(&self) -> Result<(), IpcOpError> {
        expect_error("Failed to reconnect to chewing_tip_host", || {
            let pipe = Self::connect_pipe()?;
            if let Some(pipe) = self.pipe.borrow().as_ref() {
                pipe.assume_flushed();
            }
            self.pipe.replace(Some(pipe));
            Ok(())
        })
    }
    pub fn send(&self, method_call: MethodCall) -> Result<MethodReply, IpcOpError> {
        // FIXME move retry logic out
        expect_error("Failed to call IPC method", || {
            let mut bytes = serde_json::to_vec(&method_call)?;
            bytes.push(0);
            let write_result = self
                .pipe
                .try_borrow_mut()?
                .as_mut()
                .map(|pipe| pipe.write_all(&bytes));
            match write_result {
                None => {
                    log::error!("Retrying...");
                    self.reconnect()?;
                    self.pipe
                        .try_borrow_mut()?
                        .as_mut()
                        .ok_or("Unable to connect")?
                        .write_all(&bytes)?;
                }
                Some(Err(error)) => {
                    log::error!("Error calling ipc method: {}", &method_call.method);
                    log::error!("{error:?}");
                    log::error!("Retrying...");
                    self.reconnect()?;
                    self.pipe
                        .try_borrow_mut()?
                        .as_mut()
                        .ok_or("Unable to connect")?
                        .write_all(&bytes)?;
                }
                Some(Ok(_)) => {}
            }
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
            if let Err(error) = reader.read_until(0, &mut buffer) {
                log::error!("Failed to read from pipe: {error:?}");
                log::error!("Retrying...");
                self.reconnect()?;
                reader = BufReader::new(
                    self.pipe
                        .try_borrow()?
                        .as_ref()
                        .ok_or("Broken Pipe")?
                        .try_clone()?,
                );
                reader.read_until(0, &mut buffer)?;
            }
            if buffer.last().is_none_or(|b| *b != 0) {
                log::debug!("EOF - server exited");
                return Err("EOF - server exited".into());
            }
            buffer.pop();
            Ok(serde_json::from_slice::<MethodReply>(&buffer)?)
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
