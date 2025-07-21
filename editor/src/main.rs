// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod editor;

use std::env;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    win_dbg_logger::init();
    slint::BackendSelector::new()
        .with_winit_window_attributes_hook(|attrs| attrs.with_transparent(false))
        .select()?;
    editor::run()?;
    Ok(())
}
