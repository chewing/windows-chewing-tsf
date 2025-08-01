// SPDX-License-Identifier: GPL-3.0-or-later

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    dbg!(std::env::var("MINGW_CHOST"));
    println!("cargo::rerun-if-env-changed=MINGW_CHOST");
    embed_resource::compile("rc/ChewingTextService.rc", embed_resource::NONE)
        .manifest_optional()?;
    Ok(())
}
