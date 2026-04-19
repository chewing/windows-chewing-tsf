use std::{fs::File, hash::Hasher, io::Read, path::PathBuf};

use exn::{Exn, Result, ResultExt, bail};
use fnv::FnvHasher;
use interprocess::os::windows::{
    named_pipe::{DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode::Bytes},
    security_descriptor::SecurityDescriptor,
};
use log::{debug, error, info};
use minisign_verify::{PublicKey, Signature};
use widestring::U16CString;
use windows::{
    Win32::{
        Foundation::{CloseHandle, MAX_PATH},
        System::Threading::{
            OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    },
    core::PWSTR,
};

use crate::ipc::IpcError;
use crate::sandbox::get_user_cred;

pub const NAMED_PIPE_PATH_BASE: &str = r"\\.\pipe\chewing.";
pub const TRUSTED_MINISIGN_KEY: &str = "RWTgGhLoHRMztdiikZxoXuU4C3tabjFLP5PjdH934zCOxmhZa6ktuGbX";

pub fn named_pipe_path() -> Result<String, IpcError> {
    let err = || IpcError(format!("failed to create unique user local NamedPipe path"));
    let user_cred = get_user_cred().or_raise(err)?;
    let mut hasher = FnvHasher::default();
    hasher.write(user_cred.token_user_sid.as_bytes());
    Ok(format!("{NAMED_PIPE_PATH_BASE}{:x}.pipe", hasher.finish()))
}

pub fn create_pipe_listener() -> Result<PipeListener<Bytes, Bytes>, IpcError> {
    let err = || IpcError(format!("failed to create named pipe listener"));
    let user_cred = get_user_cred().or_raise(err)?;
    let mut security_descriptor = String::new();
    security_descriptor.push_str(&format!("O:{}", user_cred.token_user_sid));
    security_descriptor.push_str(&format!("G:{}", user_cred.token_primary_group_sid));
    security_descriptor.push_str("D:");
    security_descriptor.push_str("(A;;;;;OW)");
    security_descriptor.push_str("(A;;GA;;;SY)");
    security_descriptor.push_str("(A;;GA;;;BA)");
    security_descriptor.push_str("(A;;GA;;;AC)");
    security_descriptor.push_str(&format!("(A;;GA;;;{})", user_cred.token_user_sid));
    security_descriptor.push_str("S:(ML;;NX;;;LW)");

    let sd = SecurityDescriptor::deserialize(
        U16CString::from_str(&security_descriptor)
            .or_raise(err)?
            .as_ucstr(),
    )
    .or_raise(|| IpcError(format!("failed to parse SDDL: {security_descriptor}")))
    .or_raise(err)?;

    let pipe_path = named_pipe_path().or_raise(err)?;

    info!("Creating named pipe at {pipe_path}");
    Ok(PipeListenerOptions::new()
        .path(pipe_path)
        .mode(PipeMode::Bytes)
        .security_descriptor(Some(sd))
        .create_duplex::<Bytes>()
        .or_raise(err)?)
}

/// Connects to the well-known windows-chewing-tsf named pipe and validate the
/// server executable is signed with a trusted minisign key.
pub fn connect_and_attest() -> Result<DuplexPipeStream<Bytes>, IpcError> {
    let pipe_path =
        named_pipe_path().or_raise(|| IpcError(format!("failed to connect to named pipe")))?;
    let err = || IpcError(format!("failed to connect to named pipe {pipe_path}"));

    debug!("trying to connect to named pipe {pipe_path}");
    let pipe = DuplexPipeStream::connect_by_path(pipe_path.clone()).or_raise(err)?;

    let peer_pid = pipe.server_process_id().or_raise(err)?;
    if let Err(error) = attest_server(peer_pid) {
        if cfg!(debug_assertions) {
            error!("failed to validate signature: {error:?}");
        } else {
            bail!(error.raise(err()));
        }
    }

    Ok(pipe)
}

fn attest_server(pid: u32) -> Result<(), IpcError> {
    let err = || IpcError(format!("failed to attest server executible"));
    let public_key = PublicKey::from_base64(TRUSTED_MINISIGN_KEY).or_raise(err)?;

    let exe_path = unsafe {
        let mut buffer = [0u16; MAX_PATH as usize];
        let mut size = MAX_PATH;
        let pwpath = PWSTR::from_raw(buffer.as_mut_ptr());
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).or_raise(err)?;
        if let Err(error) =
            QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), pwpath, &mut size)
        {
            if let Err(e) = CloseHandle(handle) {
                bail!(Exn::raise_all(err(), [error, e]));
            }
            bail!(err());
        }
        PathBuf::from(pwpath.to_string().or_raise(err)?)
    };
    let sig_path = exe_path.with_added_extension("minisig");

    let sig = Signature::from_file(sig_path).or_raise(err)?;
    let mut verifier = public_key.verify_stream(&sig).or_raise(err)?;
    let mut exe_file = File::open(exe_path).or_raise(err)?;
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = exe_file.read(&mut buffer).or_raise(err)?;
        if bytes_read == 0 {
            break; // End of file
        }
        verifier.update(&buffer[..bytes_read]);
    }
    verifier.finalize().or_raise(err)?;

    Ok(())
}
