// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // embed_resource::compile("rc/chewing_tip_host.rc", embed_resource::NONE).manifest_required()?;
    embedinator::ResourceBuilder::from_env()
        .add_manifest(std::fs::read_to_string("rc/chewing_tip_host.manifest").unwrap())
        .add_string(
            "LegalCopyright",
            "Copyright (C) 2013-2026 libchewing Core Team",
        )
        .finish();
    println!("cargo:rerun-if-changed=app.manifest");
    Ok(())
}
