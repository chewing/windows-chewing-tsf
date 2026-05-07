use std::error::Error;
use std::fmt::Display;
use std::io::ErrorKind;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;

use exn::{OptionExt, Result, ResultExt};
use windows::Foundation::Uri;
use windows::System::Launcher;
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_FLAGS_AND_ATTRIBUTES, SetFileAttributesW,
};
use windows::core::BSTR;

pub fn user_dir() -> Result<PathBuf, ShellError> {
    let err = || ShellError("unable to determine user dir".to_string());
    let user_dir = chewing::path::data_dir().ok_or_raise(err)?;

    // NB: chewing might be loaded into a low mandatory integrity level process (SearchHost.exe).
    // In that case, it might not be able to check if a file exists using CreateFile
    // If the file exists, it will get the PermissionDenied error instead.
    let user_dir_exists = match std::fs::exists(&user_dir) {
        Ok(true) => true,
        Err(e) => matches!(e.kind(), ErrorKind::PermissionDenied),
        _ => false,
    };

    if !user_dir_exists {
        std::fs::create_dir(&user_dir).or_raise(err)?;
        let metadata = user_dir.metadata().or_raise(err)?;
        let attributes = metadata.file_attributes();
        let user_dir_w: Vec<u16> = user_dir.as_os_str().encode_wide().collect();
        unsafe {
            SetFileAttributesW(
                &BSTR::from_wide(&user_dir_w),
                FILE_FLAGS_AND_ATTRIBUTES(attributes | FILE_ATTRIBUTE_HIDDEN.0),
            )
            .or_raise(err)?;
        };
    }

    Ok(user_dir)
}

pub fn program_dir() -> Result<PathBuf, ShellError> {
    let err = || ShellError("failed to determine Program Files path".to_string());
    Ok(PathBuf::from(
        std::env::var("ProgramW6432")
            .or_else(|_| std::env::var("ProgramFiles"))
            .or_else(|_| std::env::var("ProgramFiles(x86)"))
            .or_raise(err)?,
    )
    .join("ChewingTextService"))
}

pub fn open_url(url: &str) {
    if let Ok(uri) = Uri::CreateUri(&url.into()) {
        let _ = Launcher::LaunchUriAsync(&uri);
    }
}

#[derive(Debug)]
pub struct ShellError(String);
impl Error for ShellError {}
impl Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ShellError: {}", self.0)
    }
}
