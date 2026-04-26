use std::{
    ffi::OsStr,
    hash::Hasher,
    iter::once,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    time::Duration,
};

use exn::{Exn, Result, ResultExt, bail};
use fnv::FnvHasher;
use interprocess::os::windows::{
    named_pipe::{DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode::Bytes},
    security_descriptor::SecurityDescriptor,
};
use log::{debug, error, info};
use widestring::U16CString;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, INVALID_HANDLE_VALUE, MAX_PATH, S_OK},
        Security::WinTrust::{
            WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
            WTD_CHOICE_FILE, WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT, WTD_REVOKE_WHOLECHAIN,
            WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE, WinVerifyTrust,
        },
        System::{
            Pipes::WaitNamedPipeW,
            Threading::{
                OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
    },
    core::{HSTRING, PCWSTR, PWSTR},
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
    // Owner SID
    security_descriptor.push_str(&format!("O:{}", user_cred.token_user_sid));
    // DACL - ACE Strings
    security_descriptor.push_str("D:");
    // Remove default owner rights
    security_descriptor.push_str("(A;;;;;OW)");
    // Allow local system
    security_descriptor.push_str("(A;;GA;;;SY)");
    // Allow administrator
    security_descriptor.push_str("(A;;GA;;;BA)");
    // Allow all read/write from app containers
    security_descriptor.push_str("(A;;GA;;;AC)");
    // Allow all read/write from user
    security_descriptor.push_str(&format!("(A;;GA;;;{})", user_cred.token_user_sid));
    // SACL - mandatory label - no execute up - low integrity level
    security_descriptor.push_str("S:(ML;;NX;;;LW)");

    debug!("SDDL for NamedPipe: {security_descriptor}");

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
pub fn connect_and_attest(
    pipe_path: &str,
    timeout: Duration,
) -> Result<DuplexPipeStream<Bytes>, IpcError> {
    let err = || IpcError(format!("failed to connect to named pipe {pipe_path}"));

    debug!("trying to connect to named pipe {pipe_path}");
    unsafe {
        let _ = WaitNamedPipeW(&HSTRING::from(pipe_path), timeout.as_millis() as u32);
    }
    let pipe = DuplexPipeStream::connect_by_path(pipe_path).or_raise(err)?;

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

    if !verify_trust(&exe_path) {
        bail!(IpcError(format!(
            "unable to verify the signature of {}",
            exe_path.display()
        )));
    }

    Ok(())
}

fn os_to_wstring(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(once(0)).collect()
}

pub fn verify_trust(path: &Path) -> bool {
    // Convert path to wide string
    let wide_path: Vec<u16> = os_to_wstring(path.as_os_str());

    let mut file_info = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide_path.as_ptr()),
        hFile: Default::default(),
        pgKnownSubject: std::ptr::null_mut(),
    };

    let mut trust_data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_WHOLECHAIN,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &mut file_info as *mut _,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT,
        ..Default::default()
    };

    let mut pg_action_id = WINTRUST_ACTION_GENERIC_VERIFY_V2;

    // First verify trust
    let status = unsafe {
        WinVerifyTrust(
            HWND(INVALID_HANDLE_VALUE.0),
            &mut pg_action_id,
            &trust_data as *const _ as *mut _,
        )
    };

    unsafe {
        trust_data.dwStateAction = WTD_STATEACTION_CLOSE;
        WinVerifyTrust(
            HWND(INVALID_HANDLE_VALUE.0),
            &mut pg_action_id,
            &trust_data as *const _ as *mut _,
        );
    }

    status == S_OK.0
}
