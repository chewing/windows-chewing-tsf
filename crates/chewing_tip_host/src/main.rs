#![windows_subsystem = "windows"]

use std::{error::Error, path::PathBuf, thread};

use chewing_tip_core::ipc::named_pipe::named_pipe_path;
use log::info;
use logforth::record::LevelFilter;
use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

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

    let pipe_path = PathBuf::from(named_pipe_path()?);
    if pipe_path.exists() {
        info!("Another chewing_tip_host is already running.");
        return Ok(());
    }

    info!("Setup main loop");
    let mut main_loop = MainLoop::new();
    let mh = main_loop.get_handle();

    info!("Spawn IPC thread");
    thread::spawn(move || run_ipc_server(mh));

    info!("Starting main loop");
    main_loop.run();

    Ok(())
}
