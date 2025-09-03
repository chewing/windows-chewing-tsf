#![windows_subsystem = "windows"]

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use windows::{
    Win32::Foundation::*, Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*, core::*,
};

mod config;
mod releases;
mod version;

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
const TIMER_ID: usize = 1;
const HOUR_IN_MS: u32 = 3600000; // 1 hour in milliseconds

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_QUERYENDSESSION => {
            log::info!("Restart Manager requesting shutdown preparation");
            LRESULT(1)
        }
        WM_ENDSESSION => {
            if wparam.0 != 0 {
                log::info!("Shutdown confirmed - performing final cleanup");
                SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
            } else {
                log::info!("Shutdown cancelled");
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_ID {
                check_for_update();
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER_ID);
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn check_for_update() {
    log::info!("Checking for update...");
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
    match releases::fetch_releases() {
        Ok(releases) => {
            for rel in releases {
                if rel.channel == cfg.channel && version::version_gt(&rel.version, &dll_version) {
                    log::info!("Updates available: version {}", rel.version);
                    if let Err(error) = config::set_update_info_url(&rel.url) {
                        log::error!("Unable to set update info URL: {error:#}");
                    }
                    return;
                }
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
}

struct MainLoop {
    hwnd: HWND,
}

impl MainLoop {
    fn new() -> Result<Self> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            let class_name = w!("ChewingUpdateSvc");

            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: hinstance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };

            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!("ChewingUpdateSvc"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                None,
                None,
                Some(hinstance.into()),
                None,
            )?;

            // Set up the timer for periodic work
            if SetTimer(Some(hwnd), TIMER_ID, HOUR_IN_MS, None) == 0 {
                return Err(Error::from_win32()).context("SetTimer failed");
            }

            Ok(MainLoop { hwnd })
        }
    }

    fn run_message_loop(&self) -> Result<()> {
        unsafe {
            let mut msg = MSG::default();

            // Perform initial work
            check_for_update();

            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);

                if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
                    break;
                }
            }
        }

        Ok(())
    }
}

impl Drop for MainLoop {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_invalid() {
                let _ = KillTimer(Some(self.hwnd), TIMER_ID);
                let _ = DestroyWindow(self.hwnd);
            }
        }
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
    let main_loop = match MainLoop::new() {
        Ok(ml) => ml,
        Err(error) => {
            log::error!("Unable to create message window and main loop: {error:#}");
            return;
        }
    };
    if let Err(error) = main_loop.run_message_loop() {
        log::error!("Error: {error:#}");
    }
}
