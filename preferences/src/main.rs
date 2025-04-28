// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod about;
mod config;
mod editor;

use std::env;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    if env::args().any(|arg| arg == "/edit" || arg == "--edit") {
        editor::run()?;
    } else if env::args().any(|arg| arg == "/about" || arg == "--about") {
        about::run()?;
    } else {
        config::run()?;
    }
    Ok(())
}
