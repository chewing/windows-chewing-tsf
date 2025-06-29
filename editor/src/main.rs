// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod editor;

use std::env;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    editor::run()?;
    Ok(())
}
