use std::time::{Duration, SystemTime, UNIX_EPOCH};

use scoped_error::ErrorExt;

pub(crate) mod config;
mod releases;
mod version;

pub(crate) fn check_for_update() {
    log::info!("Checking for update...");
    let cfg = match config::get_check_update_config() {
        Ok(cfg) => cfg,
        Err(error) => {
            log::error!("{}", error.report());
            return;
        }
    };

    if cfg.current_update_info_url.is_empty() {
        // If we don't know the current update status, then we skip the checks if
        // we just checked before.
        if !cfg.enabled {
            log::info!("Check for update was disabled; abort");
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .as_ref()
            .map(Duration::as_secs)
            .unwrap_or_default();
        if now.abs_diff(cfg.last_update_check_time) < 3600 {
            log::info!(
                "Current ts: {now}, last_update_check_time: {}",
                cfg.last_update_check_time
            );
            log::info!("Already checked updates in last one hour; abort");
            return;
        }
    }

    // Always clear update URL before a new check
    if let Err(error) = config::set_update_info_url("") {
        log::error!("{}", error.report());
    }

    if !cfg.enabled {
        log::info!("Check for update was disabled; abort");
        return;
    }

    let dll_version = version::chewing_product_version();
    log::info!("Current version = {dll_version}");
    match releases::fetch_releases() {
        Ok(releases) => 'check: {
            for rel in releases {
                if rel.channel == cfg.channel && version::version_gt(&rel.version, &dll_version) {
                    log::info!("Updates available: version {}", rel.version);
                    if let Err(error) = config::set_update_info_url(&rel.url) {
                        log::error!("{}", error.report());
                    }
                    break 'check;
                }
            }
            // no new releases were found, clear update url
            if let Err(error) = config::set_update_info_url("") {
                log::error!("{}", error.report());
            }
        }
        Err(error) => {
            log::error!("{}", error.report());
            if let Err(error) = config::set_update_info_url("") {
                log::error!("{}", error.report());
            }
            return;
        }
    }
    if let Err(error) = config::set_last_update_check_time() {
        log::error!("{}", error.report());
    }
}
