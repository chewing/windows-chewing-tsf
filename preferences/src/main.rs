// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod about;
mod config;
mod fonts;

use std::env;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    win_dbg_logger::init();
    if env::args().any(|arg| arg == "/about" || arg == "--about") {
        about::run()?;
    } else {
        config::run()?;
    }
    Ok(())
}
