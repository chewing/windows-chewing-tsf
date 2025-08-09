// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod about;
mod config;
mod fonts;

use std::env;

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    win_dbg_logger::init();
    let mut sel = slint::BackendSelector::new();
    if is_vm() {
        sel = sel.renderer_name("software".to_string());
    }
    sel.with_winit_window_attributes_hook(|attrs| attrs.with_transparent(false))
        .select()?;
    if env::args()
        .any(|arg| arg == "/about" || arg == "--about" || arg == "chewing-preferences://about/")
    {
        about::run()?;
    } else {
        config::run()?;
    }
    Ok(())
}

fn is_vm() -> bool {
    if let Some(mb) = sysinfo::Motherboard::new() {
        let name = mb.name().unwrap_or_default();
        return ["Virtual Machine"].contains(&name.as_str());
    }
    true
}
