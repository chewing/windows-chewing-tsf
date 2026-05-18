use error_plus::ErrorExt;

pub(crate) mod config;
mod releases;
mod version;

pub(crate) fn check_for_update() {
    log::info!("Checking for update...");
    // Always clear update URL before a new check
    if let Err(error) = config::set_update_info_url("") {
        log::error!("{}", error.error_report());
    }
    let cfg = match config::get_check_update_config() {
        Ok(cfg) => cfg,
        Err(error) => {
            log::error!("{}", error.error_report());
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
                        log::error!("{}", error.error_report());
                    }
                    break 'check;
                }
            }
            // no new releases were found, clear update url
            if let Err(error) = config::set_update_info_url("") {
                log::error!("{}", error.error_report());
            }
        }
        Err(error) => {
            log::error!("{}", error.error_report());
            if let Err(error) = config::set_update_info_url("") {
                log::error!("{}", error.error_report());
            }
            return;
        }
    }
    if let Err(error) = config::set_last_update_check_time() {
        log::error!("{}", error.error_report());
    }
}
