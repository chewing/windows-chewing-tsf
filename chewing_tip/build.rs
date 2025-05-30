// SPDX-License-Identifier: GPL-3.0-or-later

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    embed_resource::compile("rc/ChewingTextService.rc", embed_resource::NONE)
        .manifest_optional()?;
    Ok(())
}
