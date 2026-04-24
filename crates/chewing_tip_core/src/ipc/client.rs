use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Write},
    rc::Rc,
    thread,
    time::Duration,
};

use exn::{Exn, Result, ResultExt, bail};
use interprocess::{
    TryClone,
    os::windows::named_pipe::{DuplexPipeStream, pipe_mode::Bytes},
};

use crate::{
    ipc::{
        IpcError,
        named_pipe::{connect_and_attest, named_pipe_path},
        varlink::{MethodCall, MethodReply},
    },
    shell::open_url,
};

#[derive(Clone)]
pub struct ChewingIpcClient {
    pipe: Rc<RefCell<DuplexPipeStream<Bytes>>>,
}

impl ChewingIpcClient {
    pub fn connect() -> Result<ChewingIpcClient, IpcError> {
        let pipe = ChewingIpcClient::connect_pipe()?;
        Ok(ChewingIpcClient {
            pipe: Rc::new(RefCell::new(pipe)),
        })
    }
    fn connect_pipe() -> Result<DuplexPipeStream<Bytes>, IpcError> {
        let err = || IpcError(format!("unable to connect to chewing_tip_host"));
        let pipe_path = named_pipe_path().or_raise(err)?;

        let res = connect_and_attest(&pipe_path, Duration::from_millis(100));
        if let Ok(pipe) = res {
            return Ok(pipe);
        }
        let error = res.unwrap_err();
        log::error!("Failed to connect to chewing_tip_host...");
        log::error!("{error:?}");
        log::error!("Trying to launch chewing_tip_host and retry...");
        open_url("chewing-tip-host://init");
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
        open_url("chewing-tip-host://init");
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            let res = connect_and_attest(&pipe_path, Duration::from_millis(100));
            if let Ok(pipe) = res {
                return Ok(pipe);
            }
        }
        // FIXME
        Ok(connect_and_attest(&pipe_path, Duration::from_millis(100)).or_raise(err)?)
    }
    fn reconnect(&self) -> Result<(), IpcError> {
        let err = || IpcError(format!("failed reconnecting to chewing_tip_host"));
        let pipe = Self::connect_pipe().or_raise(err)?;
        self.pipe.borrow().assume_flushed();
        self.pipe.replace(pipe);
        Ok(())
    }
    pub fn send(&self, method_call: MethodCall) -> Result<MethodReply, IpcError> {
        let err = || {
            IpcError(format!(
                "failed to call ipc method: {}",
                &method_call.method
            ))
        };
        let mut bytes = serde_json::to_vec(&method_call).or_raise(err)?;
        bytes.push(0);
        let write_result = self.pipe.try_borrow_mut().or_raise(err)?.write_all(&bytes);
        if let Err(error) = write_result {
            log::error!("Error calling ipc method: {}", &method_call.method);
            log::error!("{error:?}");
            log::error!("Retrying...");
            self.reconnect().or_raise(err)?;
            self.pipe
                .try_borrow_mut()
                .or_raise(err)?
                .write_all(&bytes)
                .or_raise(err)?;
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
                .try_borrow()
                .or_raise(err)?
                .try_clone()
                .or_raise(err)?,
        );
        if let Err(error) = reader.read_until(0, &mut buffer) {
            log::error!("Failed to read from pipe: {error:?}");
            log::error!("Retrying...");
            self.reconnect().or_raise(err)?;
            reader = BufReader::new(
                self.pipe
                    .try_borrow()
                    .or_raise(err)?
                    .try_clone()
                    .or_raise(err)?,
            );
            reader.read_until(0, &mut buffer).or_raise(err)?;
        }
        if buffer.last().is_none_or(|b| *b != 0) {
            log::debug!("EOF - server exited");
            bail!(Exn::new(err()));
        }
        buffer.pop();
        Ok(serde_json::from_slice::<MethodReply>(&buffer).or_raise(err)?)
    }
}

impl Drop for ChewingIpcClient {
    fn drop(&mut self) {
        let _ = self.pipe.borrow_mut().flush();
    }
}
