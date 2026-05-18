#![windows_subsystem = "windows"]

use std::{path::PathBuf, process::Command, thread};

use chewing_tip_core::ipc::named_pipe::{create_pipe_listener, named_pipe_path};
use error_plus::{ErrorExt, expect_error};
use log::{error, info};
use logforth::record::{Level, LevelFilter};
use windows::Win32::System::{
    Console::{ATTACH_PARENT_PROCESS, AttachConsole},
    Recovery::{REGISTER_APPLICATION_RESTART_FLAGS, RegisterApplicationRestart},
};

use crate::{ipc::run_ipc_listener, ui::event_loop::MainLoop};

mod ipc;
mod text_service;
mod ui;
mod ui_elements;
mod update;

fn main() -> Result<(), error_plus::Error> {
    expect_error("Running chewing_tip_host failed", || {
        if std::env::args().any(|arg| arg == "-d") {
            // daemonize
            let self_exe = std::env::current_exe()?;
            let _ = Command::new(self_exe).spawn()?;
            return Ok(());
        }
        if std::env::args().any(|arg| arg == "-a") {
            unsafe {
                let _ = AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
        logforth::starter_log::stdout()
            .filter(LevelFilter::MoreSevereEqual(Level::Debug))
            .apply();

        info!("Register application for automatic restart");
        unsafe {
            if let Err(error) =
                RegisterApplicationRestart(None, REGISTER_APPLICATION_RESTART_FLAGS(0))
            {
                error!(
                    "Failed to register application for restart: {}",
                    error.error_report()
                );
            }
        }
        info!("Clear update info URL on restart");
        if let Err(error) = update::config::set_update_info_url("") {
            log::error!("{}", error.error_report());
        }

        info!("Create NamedPipe listener");
        let listener = create_pipe_listener().inspect_err(|_| {
            if let Ok(path) = named_pipe_path() {
                let pipe_path = PathBuf::from(path);
                if pipe_path.exists() {
                    info!("Another chewing_tip_host is already running.");
                }
            }
        })?;

        info!("Setup main loop");
        let mut main_loop = MainLoop::new();
        let mh = main_loop.get_handle();

        info!("Spawn IPC thread");
        thread::spawn(move || run_ipc_listener(listener, mh));

        info!("Starting main loop");
        main_loop.run();

        Ok(())
    })
}
