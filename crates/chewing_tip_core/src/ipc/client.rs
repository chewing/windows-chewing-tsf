use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Write},
    rc::Rc,
    thread,
    time::Duration,
};

use interprocess::{
    TryClone,
    os::windows::named_pipe::{DuplexPipeStream, pipe_mode::Bytes},
};

use crate::{
    impl_context_error,
    ipc::{
        named_pipe::{connect_and_attest, named_pipe_path},
        varlink::{MethodCall, MethodReply},
    },
    result::expect_error,
    shell::{execute, program_dir},
};

#[derive(Clone)]
pub struct ChewingIpcClient {
    pipe: Rc<RefCell<DuplexPipeStream<Bytes>>>,
}

impl ChewingIpcClient {
    pub fn connect_with_retry() -> Result<ChewingIpcClient, IpcOpError> {
        let pipe = ChewingIpcClient::connect_pipe()?;
        Ok(ChewingIpcClient {
            pipe: Rc::new(RefCell::new(pipe)),
        })
    }
    pub fn connect() -> Result<ChewingIpcClient, IpcOpError> {
        expect_error("Unable to connect to chewing_tip_host", || {
            let pipe_path = named_pipe_path()?;
            let pipe = connect_and_attest(&pipe_path, Duration::from_millis(100))?;
            Ok(ChewingIpcClient {
                pipe: Rc::new(RefCell::new(pipe)),
            })
        })
    }
    fn connect_pipe() -> Result<DuplexPipeStream<Bytes>, IpcOpError> {
        expect_error("Unable to connect to chewing_tip_host", || {
            let tip_host_path = program_dir()?.join("chewing_tip_host.exe");
            let pipe_path = named_pipe_path()?;

            let res = connect_and_attest(&pipe_path, Duration::from_millis(100));
            if let Ok(pipe) = res {
                return Ok(pipe);
            }
            let error = res.unwrap_err();
            log::error!("Failed to connect to chewing_tip_host...");
            log::error!("{error:?}");
            log::error!("Trying to launch chewing_tip_host and retry...");
            execute(&tip_host_path)?;
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
            execute(&tip_host_path)?;
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
            self.pipe.borrow().assume_flushed();
            self.pipe.replace(pipe);
            Ok(())
        })
    }
    pub fn send(&self, method_call: MethodCall) -> Result<MethodReply, IpcOpError> {
        expect_error("Failed to call IPC method", || {
            let mut bytes = serde_json::to_vec(&method_call)?;
            bytes.push(0);
            let write_result = self.pipe.try_borrow_mut()?.write_all(&bytes);
            if let Err(error) = write_result {
                log::error!("Error calling ipc method: {}", &method_call.method);
                log::error!("{error:?}");
                log::error!("Retrying...");
                self.reconnect()?;
                self.pipe.try_borrow_mut()?.write_all(&bytes)?;
            }
            if matches!(method_call.oneway, Some(true)) {
                return Ok(MethodReply {
                    parameters: serde_json::Value::Null,
                    continues: None,
                    error: None,
                });
            }
            let mut buffer = vec![];
            let mut reader = BufReader::new(self.pipe.try_borrow()?.try_clone()?);
            if let Err(error) = reader.read_until(0, &mut buffer) {
                log::error!("Failed to read from pipe: {error:?}");
                log::error!("Retrying...");
                self.reconnect()?;
                reader = BufReader::new(self.pipe.try_borrow()?.try_clone()?);
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
        let _ = self.pipe.borrow_mut().flush();
    }
}

impl_context_error!(pub IpcOpError);
