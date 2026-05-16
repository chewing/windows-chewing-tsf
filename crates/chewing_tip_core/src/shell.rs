use std::io::ErrorKind;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use windows::Foundation::Uri;
use windows::System::Launcher;
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_FLAGS_AND_ATTRIBUTES, SetFileAttributesW,
};
use windows::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CREATE_DEFAULT_ERROR_MODE, CREATE_NEW_PROCESS_GROUP,
};
use windows::core::BSTR;

use error_plus::expect_error;
use error_plus::impl_context_error;

pub fn user_dir() -> Result<PathBuf, ShellError> {
    expect_error("Unable to determine user dir", || {
        let user_dir = chewing::path::data_dir().ok_or("Unsupported operating system")?;

        // NB: chewing might be loaded into a low mandatory integrity level process (SearchHost.exe).
        // In that case, it might not be able to check if a file exists using CreateFile
        // If the file exists, it will get the PermissionDenied error instead.
        let user_dir_exists = match std::fs::exists(&user_dir) {
            Ok(true) => true,
            Err(e) => matches!(e.kind(), ErrorKind::PermissionDenied),
            _ => false,
        };

        if !user_dir_exists {
            std::fs::create_dir(&user_dir)?;
            let metadata = user_dir.metadata()?;
            let attributes = metadata.file_attributes();
            let user_dir_w: Vec<u16> = user_dir.as_os_str().encode_wide().collect();
            unsafe {
                SetFileAttributesW(
                    &BSTR::from_wide(&user_dir_w),
                    FILE_FLAGS_AND_ATTRIBUTES(attributes | FILE_ATTRIBUTE_HIDDEN.0),
                )?;
            };
        }

        Ok(user_dir)
    })
}

pub fn program_dir() -> Result<PathBuf, ShellError> {
    expect_error("Failed to determine Program Files path", || {
        Ok(PathBuf::from(
            std::env::var("ProgramW6432")
                .or_else(|_| std::env::var("ProgramFiles"))
                .or_else(|_| std::env::var("ProgramFiles(x86)"))?,
        )
        .join("ChewingTextService"))
    })
}

pub fn open_url(url: &str) {
    if let Ok(uri) = Uri::CreateUri(&url.into()) {
        let _ = Launcher::LaunchUriAsync(&uri);
    }
}

pub fn launch_tip_host() -> Result<(), ShellError> {
    expect_error("Unable to launch chewing_tip_host.exe", || {
        let path = program_dir()?.join("chewing_tip_host.exe");
        let _ = Command::new(path)
            .env_clear()
            .creation_flags(
                CREATE_BREAKAWAY_FROM_JOB.0
                    | CREATE_DEFAULT_ERROR_MODE.0
                    | CREATE_NEW_PROCESS_GROUP.0,
            )
            .arg("-d")
            .spawn()?;
        Ok(())
    })
}

impl_context_error!(pub ShellError);
