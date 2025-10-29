use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use windows_registry::CURRENT_USER;

use crate::version;

pub(crate) struct CheckUpdateConfig {
    pub(crate) enabled: bool,
    pub(crate) channel: String,
}

pub(crate) fn get_check_update_config() -> Result<CheckUpdateConfig> {
    let key = CURRENT_USER.create(r"Software\ChewingTextService")?;
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

pub(crate) fn set_update_info_url(url: &str) -> Result<()> {
    let key = CURRENT_USER.create(r"Software\ChewingTextService")?;
    if url.is_empty() {
        key.remove_value("UpdateInfoUrl")?;
    } else {
        key.set_string("UpdateInfoUrl", &url)?;
    }
    Ok(())
}

pub(crate) fn set_last_update_check_time() -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .as_ref()
        .map(Duration::as_secs)
        .unwrap_or_default();
    let key = CURRENT_USER.create(r"Software\ChewingTextService")?;
    key.set_u64("LastUpdateCheckTime", now)?;
    Ok(())
}
