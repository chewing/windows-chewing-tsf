// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    embedinator::ResourceBuilder::from_env()
        .add_manifest(std::fs::read_to_string("app.manifest")?)
        .add_string(
            "LegalCopyright",
            "Copyright (C) 2013-2026 libchewing Core Team",
        )
        .finish();
    println!("cargo:rerun-if-changed=app.manifest");
    Ok(())
}
