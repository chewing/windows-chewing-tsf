#![windows_subsystem = "windows"]

mod config;
mod releases;
mod version;

fn check_for_update() {
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

fn main() {
    win_dbg_logger::init();
    log::set_max_level(log::LevelFilter::Info);
    log::info!("chewing-update-svc started");
    let lock_path = std::env::temp_dir().join("chewing_update_svc.lock");
    let lock_file = match std::fs::File::create(lock_path) {
        Ok(file) => file,
        Err(error) => {
            log::error!("Unable to open the lock file: {error:#}");
            return;
        }
    };
    if lock_file.try_lock().is_err() {
        log::info!("Another chewing-update-svc.exe is already running");
        return;
    }
    check_for_update();
}
