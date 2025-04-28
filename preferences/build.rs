// SPDX-License-Identifier: GPL-3.0-or-later

fn main() {
    slint_build::compile("ui/index.slint").expect("Slint build failed");
    embed_resource::compile("preferences.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
