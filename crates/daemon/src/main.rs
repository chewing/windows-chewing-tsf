use std::{error::Error, thread};

use log::info;
use logforth::record::LevelFilter;

use crate::{ipc::run_ipc_server, ui::event_loop::MainLoop};

mod ipc;
mod ui;
mod ui_elements;

fn main() -> Result<(), Box<dyn Error>> {
    logforth::starter_log::stdout()
        .filter(LevelFilter::All)
        .apply();

    info!("Setup main loop");
    let mut main_loop = MainLoop::new();
    let mh = main_loop.get_handle();

    info!("Spawn IPC thread");
    thread::spawn(move || run_ipc_server(mh));

    info!("Starting main loop");
    main_loop.run();

    Ok(())
}
