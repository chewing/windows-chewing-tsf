use std::time::{Duration, SystemTime, UNIX_EPOCH};

use exn::Result;
use exn::ResultExt;
use windows_registry::CURRENT_USER;

use super::UpdateError;
use super::version;

pub(crate) struct CheckUpdateConfig {
    pub(crate) enabled: bool,
    pub(crate) channel: String,
}

pub(crate) fn get_check_update_config() -> Result<CheckUpdateConfig, UpdateError> {
    let err = || UpdateError("Failed to get update config");
    let key = CURRENT_USER
        .create(r"Software\ChewingTextService")
        .or_raise(err)?;
    let channel = match key.get_string("AutoCheckUpdateChannel") {
        Ok(ch) => ch,
        Err(_) => {
            let dll_channel = version::chewing_dll_channel();
            let _ = key.set_string("AutoCheckUpdateChannel", &dll_channel);
            dll_channel
        }
    };
    let enabled = channel == "stable" || channel == "development";
    Ok(CheckUpdateConfig { enabled, channel })
}

pub(crate) fn set_update_info_url(url: &str) -> Result<(), UpdateError> {
    let err = || UpdateError("Failed to set update info");
    let key = CURRENT_USER
        .create(r"Software\ChewingTextService")
        .or_raise(err)?;
    if url.is_empty() {
        key.remove_value("UpdateInfoUrl").or_raise(err)?;
    } else {
        key.set_string("UpdateInfoUrl", &url).or_raise(err)?;
    }
    Ok(())
}

pub(crate) fn set_last_update_check_time() -> Result<(), UpdateError> {
    let err = || UpdateError("Failed to set last update timestamp");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .as_ref()
        .map(Duration::as_secs)
        .unwrap_or_default();
    let key = CURRENT_USER
        .create(r"Software\ChewingTextService")
        .or_raise(err)?;
    key.set_u64("LastUpdateCheckTime", now).or_raise(err)?;
    Ok(())
}
