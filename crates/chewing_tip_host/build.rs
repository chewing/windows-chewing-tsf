// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    embed_resource::compile("rc/chewing_tip_host.rc", embed_resource::NONE).manifest_required()?;
    Ok(())
}
