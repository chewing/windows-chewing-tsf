// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    embedinator::ResourceBuilder::from_env()
        .add_manifest(if cfg!(debug_assertions) {
            std::fs::read_to_string("rc/debug.manifest")?
        } else {
            std::fs::read_to_string("rc/release.manifest")?
        })
        .add_string(
            "LegalCopyright",
            "Copyright (C) 2013-2026 libchewing Core Team",
        )
        .finish();
    println!("cargo:rerun-if-changed=rc/debug.manifest");
    println!("cargo:rerun-if-changed=rc/release.manifest");
    Ok(())
}
