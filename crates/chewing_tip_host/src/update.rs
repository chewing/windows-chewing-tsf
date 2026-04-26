use std::{error::Error, fmt::Display};

mod config;
mod releases;
mod version;

pub(crate) fn check_for_update() {
    log::info!("Checking for update...");
    // Always clear update URL before a new check
    if let Err(error) = config::set_update_info_url("") {
        log::error!("Unable to set update info URL: {error:#}");
    }
    let cfg = match config::get_check_update_config() {
        Ok(cfg) => cfg,
        Err(error) => {
            log::error!("Unable to get CheckUpdateConfig: {error:#}");
            return;
        }
    };
    if !cfg.enabled {
        log::info!("Check for update was disabled");
        return;
    }
    let dll_version = version::chewing_dll_version();
    log::info!("Current version = {dll_version}");
    match releases::fetch_releases() {
        Ok(releases) => 'check: {
            for rel in releases {
                if rel.channel == cfg.channel && version::version_gt(&rel.version, &dll_version) {
                    log::info!("Updates available: version {}", rel.version);
                    if let Err(error) = config::set_update_info_url(&rel.url) {
                        log::error!("Unable to set update info URL: {error:#}");
                    }
                    break 'check;
                }
            }
            // no new releases were found, clear update url
            if let Err(error) = config::set_update_info_url("") {
                log::error!("Unable to set update info URL: {error:#}");
            }
        }
        Err(error) => {
            log::error!("Unable to fetch release metadata: {error:#}");
            if let Err(error) = config::set_update_info_url("") {
                log::error!("Unable to set update info URL: {error:#}");
            }
            return;
        }
    }
    if let Err(error) = config::set_last_update_check_time() {
        log::error!("Unable to set last update check time: {error:#}");
    }
}

#[derive(Debug)]
struct UpdateError(&'static str);
impl Error for UpdateError {}
impl Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UpdateError: {}", self.0)
    }
}
