use std::{error::Error, fmt::Display, mem::transmute};

use exn::{Exn, Result, ResultExt, bail};
use windows::{
    Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_TOKEN, HANDLE},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_ACCESS_MASK,
            TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        System::Threading::{
            GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThreadToken,
        },
    },
    core::PWSTR,
};

/// Represents the user credential including SID
pub struct UserCred {
    pub token_user_sid: String,
}

pub fn open_effective_token(desired_access: TOKEN_ACCESS_MASK) -> Result<HANDLE, SandboxError> {
    let err = || SandboxError(format!("unable to get effective token"));
    unsafe {
        let mut token = HANDLE::default();
        if let Err(error) = OpenThreadToken(GetCurrentThread(), desired_access, true, &mut token) {
            if error.code() != ERROR_NO_TOKEN.into() {
                bail!(Exn::new(error).raise(err()));
            }
            if let Err(error) = OpenProcessToken(GetCurrentProcess(), desired_access, &mut token) {
                bail!(Exn::new(error).raise(err()));
            }
        }
        Ok(token)
    }
}

pub fn get_token_user_sid_string(token: HANDLE) -> Result<String, SandboxError> {
    let err = || SandboxError(format!("failed to extract user SID from token"));

    let mut buffer_size = 0;
    unsafe {
        if let Err(error) = GetTokenInformation(token, TokenUser, None, 0, &mut buffer_size) {
            if error != ERROR_INSUFFICIENT_BUFFER.into() {
                bail!(Exn::new(error).raise(err()));
            }
        }
    }
    let mut _return_size = 0;
    let mut buffer = vec![0; buffer_size as usize];
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            buffer_size,
            &mut _return_size,
        )
        .or_raise(err)?;
    }
    unsafe {
        let token_user: *const TOKEN_USER = transmute(buffer.as_ptr());
        let sid = token_user.as_ref().unwrap().User.Sid;
        let mut sid_string = PWSTR::null();
        ConvertSidToStringSidW(sid, &mut sid_string).or_raise(err)?;
        Ok(sid_string.to_string().or_raise(err)?)
    }
}

pub fn get_user_cred() -> Result<UserCred, SandboxError> {
    let err = || SandboxError(format!("failed to get user credential"));

    let token = open_effective_token(TOKEN_QUERY).or_raise(err)?;

    let token_user_sid = get_token_user_sid_string(token).or_raise(err)?;

    Ok(UserCred { token_user_sid })
}

#[derive(Debug)]
pub struct SandboxError(String);
impl Error for SandboxError {}
impl Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SandboxError: {}", self.0)
    }
}
