use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Write},
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

pub struct ChewingIpcClient {
    pipe: RefCell<DuplexPipeStream<Bytes>>,
}

impl ChewingIpcClient {
    pub fn connect() -> Result<ChewingIpcClient, IpcError> {
        let err = || IpcError(format!("unable to connect to chewing_tip_host"));
        let pipe_path = named_pipe_path().or_raise(err)?;

        let res = connect_and_attest(&pipe_path, Duration::from_millis(100));
        if let Ok(pipe) = res {
            return Ok(ChewingIpcClient {
                pipe: RefCell::new(pipe),
            });
        }
        let error = res.unwrap_err();
        log::error!("Failed to connect to chewing_tip_host...");
        log::error!("{error:?}");
        log::error!("Trying to launch chewing_tip_host and retry...");
        open_url("chewing-tip-host://init");
        thread::sleep(Duration::from_millis(500));
        let res = connect_and_attest(&pipe_path, Duration::from_millis(100));
        if let Ok(pipe) = res {
            return Ok(ChewingIpcClient {
                pipe: RefCell::new(pipe),
            });
        }
        log::error!("Failed to connect to chewing_tip_host...");
        log::error!("{error:?}");
        log::error!("Trying to launch chewing_tip_host and retry...");
        open_url("chewing-tip-host://init");
        thread::sleep(Duration::from_millis(1000));
        // FIXME
        let pipe = connect_and_attest(&pipe_path, Duration::from_millis(100)).or_raise(err)?;
        Ok(ChewingIpcClient {
            pipe: RefCell::new(pipe),
        })
    }
    fn reconnect(&self) -> Result<(), IpcError> {
        let err = || IpcError(format!("failed reconnecting to chewing_tip_host"));
        let new_client = Self::connect().or_raise(err)?;
        let mut pipe = self.pipe.try_borrow_mut().or_raise(err)?;
        *pipe = new_client.pipe.into_inner();
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
        if let Err(error) = self.pipe.try_borrow_mut().or_raise(err)?.write_all(&bytes) {
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

impl TryClone for ChewingIpcClient {
    fn try_clone(&self) -> std::io::Result<Self> {
        let Ok(pipe) = self.pipe.try_borrow() else {
            return Err(std::io::Error::other(
                "internal pipe is already mutably borrowed elsewhere",
            ));
        };
        Ok(ChewingIpcClient {
            pipe: RefCell::new(pipe.try_clone()?),
        })
    }
}
