// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use slint::ComponentHandle;

use crate::AboutWindow;

pub fn run() -> Result<()> {
    let ui = AboutWindow::new()?;

    ui.on_done(move || {
        slint::quit_event_loop().unwrap();
    });

    ui.run()?;
    Ok(())
}
