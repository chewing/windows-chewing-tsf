#![windows_subsystem = "windows"]

use std::{error::Error, path::PathBuf, thread};

use chewing_tip_core::ipc::named_pipe::{create_pipe_listener, named_pipe_path};
use log::{error, info};
use logforth::record::LevelFilter;
use windows::Win32::System::{
    Console::{ATTACH_PARENT_PROCESS, AttachConsole},
    Recovery::{REGISTER_APPLICATION_RESTART_FLAGS, RegisterApplicationRestart},
};
use windows_core::w;

use crate::{ipc::run_ipc_server, ui::event_loop::MainLoop};

mod ipc;
mod ui;
mod ui_elements;

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::args().any(|arg| arg == "-a") {
        unsafe {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
    logforth::starter_log::stdout()
        .filter(LevelFilter::All)
        .apply();

    info!("Register application for automatic restart");
    unsafe {
        if let Err(error) = RegisterApplicationRestart(None, REGISTER_APPLICATION_RESTART_FLAGS(0))
        {
            error!("Failed to register application for restart: {error:?}");
        }
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
    thread::spawn(move || run_ipc_server(listener, mh));

    info!("Starting main loop");
    main_loop.run();

    Ok(())
}
