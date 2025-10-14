// SPDX-License-Identifier: GPL-3.0-or-later

use std::{path::PathBuf, str::FromStr};

use anyhow::{Error, Result, bail};
use xshell::{Shell, cmd};

use crate::flags::{BuildInstaller, PackageInstaller};

#[derive(Debug)]
pub(super) enum Target {
    Gnu,
    Msvc,
}

impl FromStr for Target {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gnu" => Ok(Target::Gnu),
            "msvc" => Ok(Target::Msvc),
            _ => bail!("unknown target: {s}"),
        }
    }
}

pub(crate) fn build_installer(flags: BuildInstaller) -> Result<()> {
    let sh = Shell::new()?;

    let release = if flags.release {
        Some("--release")
    } else {
        None
    };
    let nightly = if flags.nightly {
        vec!["--features", "nightly"]
    } else {
        vec![]
    };

    let x86_64_target = match flags.target {
        None | Some(Target::Gnu) => "x86_64-pc-windows-gnu",
        Some(Target::Msvc) => "x86_64-pc-windows-msvc",
    };
    let i686_target = match flags.target {
        None | Some(Target::Gnu) => "i686-pc-windows-gnu",
        Some(Target::Msvc) => "i686-pc-windows-msvc",
    };

    let x86_64_target_dir = PathBuf::from("target").join(x86_64_target);
    let x86_64_target_dir = if flags.release {
        x86_64_target_dir.join("release")
    } else {
        x86_64_target_dir.join("debug")
    };
    let i686_target_dir = PathBuf::from("target").join(i686_target);
    let i686_target_dir = if flags.release {
        i686_target_dir.join("release")
    } else {
        i686_target_dir.join("debug")
    };

    sh.set_var("RUSTFLAGS", "-Ctarget-feature=+crt-static");

    {
        cmd!(
            sh,
            "cargo install --locked chewing-cli --git https://github.com/chewing/libchewing
                 --root build --target {x86_64_target} --features sqlite-bundled"
        )
        .run()?;
        cmd!(
            sh,
            "cargo build -p chewing_tip {release...} --target {x86_64_target}"
        )
        .run()?;
        {
            let _p = sh.push_dir("preferences");
            let debug = if !flags.release {
                Some("--debug")
            } else {
                None
            };
            // FIXME https://github.com/matklad/xshell/issues/82
            if sh.path_exists("/usr/bin/npm") {
                cmd!(
                    sh,
                    "npm run tauri -- build {debug...} --target {x86_64_target}"
                )
                .run()?;
            } else {
                cmd!(
                    sh,
                    "npm.cmd run tauri -- build {debug...} --target {x86_64_target}"
                )
                .run()?;
            }
        }
        cmd!(
            sh,
            "cargo build -p chewing-editor {release...} --target {x86_64_target}"
        )
        .run()?;
        cmd!(
            sh,
            "cargo build -p chewing-update-svc {release...} --target {x86_64_target}"
        )
        .run()?;
        cmd!(
            sh,
            "cargo build -p tsfreg {release...} {nightly...} --target {x86_64_target}"
        )
        .run()?;
    }
    {
        cmd!(
            sh,
            "cargo build -p chewing_tip {release...} --target {i686_target}"
        )
        .run()?;
    }

    sh.remove_path("build/installer")?;
    sh.create_dir("build/installer")?;
    {
        let _p = sh.push_dir("installer");
        for file in [
            "gpl-notice.rtf",
            "windows-chewing-tsf.wixproj",
            "windows-chewing-tsf.wxs",
            "windows-chewing-tsf.wxl",
            "version.wxi",
        ] {
            sh.copy_file(file, "../build/installer")?;
        }
    }
    sh.copy_file(
        "tip/rc/im.chewing.Chewing.ico",
        "build/installer/chewing.ico",
    )?;
    sh.copy_file("build/bin/chewing-cli.exe", "build/installer")?;

    sh.create_dir("build/installer/Dictionary")?;
    {
        let _p = sh.push_dir("data/misc");
        for file in ["swkb.dat", "symbols.dat"] {
            sh.copy_file(file, "../../build/installer/Dictionary")?;
        }
    }
    {
        let _dir = sh.push_dir("build/bin");
        cmd!(
            sh,
            "chewing-cli init --csv ../../data/dict/chewing/tsi.csv ../../build/installer/Dictionary/tsi.dat"
        )
        .run()?;
        cmd!(
            sh,
            "chewing-cli init --csv ../../data/dict/chewing/word.csv ../../build/installer/Dictionary/word.dat"
        )
        .run()?;
        cmd!(
            sh,
            "chewing-cli init --csv ../../data/dict/chewing/alt.csv ../../build/installer/Dictionary/alt.dat"
        )
        .run()?;
    }

    sh.create_dir("build/installer/x64")?;
    for file in ["chewing_tip.dll"] {
        sh.copy_file(
            format!("{}/{file}", x86_64_target_dir.display()),
            "build/installer/x64",
        )?;
    }
    for file in ["chewing_tip.pdb"] {
        let _ = sh.copy_file(
            format!("{}/{file}", x86_64_target_dir.display()),
            "build/installer/x64",
        );
    }
    sh.copy_file(
        format!(
            "preferences/src-tauri/{}/ChewingPreferences.exe",
            x86_64_target_dir.display()
        ),
        "build/installer",
    )?;
    // May not exist in cross-compile environment.
    let _ = sh.copy_file(
        format!(
            "preferences/src-tauri/{}/ChewingPreferences.pdb",
            x86_64_target_dir.display()
        ),
        "build/installer",
    );
    for file in ["chewing-editor.exe", "chewing-update-svc.exe", "tsfreg.exe"] {
        sh.copy_file(
            format!("{}/{file}", x86_64_target_dir.display()),
            "build/installer",
        )?;
    }
    for file in ["chewing-editor.pdb", "chewing-update-svc.pdb", "tsfreg.pdb"] {
        let _ = sh.copy_file(
            format!("{}/{file}", x86_64_target_dir.display()),
            "build/installer",
        );
    }
    sh.create_dir("build/installer/x86")?;
    for file in ["chewing_tip.dll"] {
        sh.copy_file(
            format!("{}/{file}", i686_target_dir.display()),
            "build/installer/x86",
        )?;
    }
    for file in ["chewing_tip.pdb"] {
        let _ = sh.copy_file(
            format!("{}/{file}", i686_target_dir.display()),
            "build/installer/x86",
        );
    }

    Ok(())
}

pub(crate) fn package_installer(_flags: PackageInstaller) -> Result<()> {
    let sh = Shell::new()?;

    sh.create_dir("dist")?;
    {
        let _p = sh.push_dir("build/installer");
        cmd!(
            sh,
            "msbuild -p:Configuration=Release -restore windows-chewing-tsf.wixproj"
        )
        .run()?;
    }
    sh.copy_file(
        "build/installer/bin/Release/zh-TW/windows-chewing-tsf.msi",
        "dist/windows-chewing-tsf-unsigned.msi",
    )?;

    Ok(())
}
