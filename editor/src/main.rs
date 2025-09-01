// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod editor;

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
    editor::run()?;
    Ok(())
}

fn is_vm() -> bool {
    use vm_detect::{Detection, vm_detect};

    let detection = vm_detect();
    detection.contains(Detection::HYPERVISOR_BIT)
        || detection.contains(Detection::HYPERVISOR_CPU_VENDOR)
        || detection.contains(Detection::UNEXPECTED_CPU_VENDOR)
}
