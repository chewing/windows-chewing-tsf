// SPDX-License-Identifier: GPL-3.0-or-later

use std::{path::PathBuf, str::FromStr};

use scoped_error::{Error, expect_error};
use xshell::{Shell, cmd};

use crate::flags::{BuildInstaller, PackageInstaller};

#[derive(Debug)]
pub(super) enum Target {
    Gnu,
    GnuLlvm,
    Msvc,
}

impl FromStr for Target {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        expect_error("Failed to parse target", || match s {
            "gnu" => Ok(Target::Gnu),
            "gnullvm" => Ok(Target::GnuLlvm),
            "msvc" => Ok(Target::Msvc),
            _ => Err(format!("unknown target: {s}"))?,
        })
    }
}

pub(crate) fn build_installer(flags: BuildInstaller) -> Result<(), Error> {
    expect_error("Failed to build installer", || {
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
            Some(Target::Gnu) => "x86_64-pc-windows-gnu",
            Some(Target::GnuLlvm) => "x86_64-pc-windows-gnullvm",
            None | Some(Target::Msvc) => "x86_64-pc-windows-msvc",
        };
        let aarch64_target = match flags.target {
            Some(Target::Gnu) => "aarch64-pc-windows-gnu",
            Some(Target::GnuLlvm) => "aarch64-pc-windows-gnullvm",
            None | Some(Target::Msvc) => "aarch64-pc-windows-msvc",
        };
        let i686_target = match flags.target {
            Some(Target::Gnu) => "i686-pc-windows-gnu",
            Some(Target::GnuLlvm) => "i686-pc-windows-gnullvm",
            None | Some(Target::Msvc) => "i686-pc-windows-msvc",
        };

        let x86_64_target_dir = PathBuf::from("target").join(x86_64_target);
        let x86_64_target_dir = if flags.release {
            x86_64_target_dir.join("release")
        } else {
            x86_64_target_dir.join("debug")
        };
        let aarch64_target_dir = PathBuf::from("target").join(aarch64_target);
        let aarch64_target_dir = if flags.release {
            aarch64_target_dir.join("release")
        } else {
            aarch64_target_dir.join("debug")
        };
        let i686_target_dir = PathBuf::from("target").join(i686_target);
        let i686_target_dir = if flags.release {
            i686_target_dir.join("release")
        } else {
            i686_target_dir.join("debug")
        };

        sh.set_var("RUSTFLAGS", "-Ctarget-feature=+crt-static");
        if matches!(flags.target, Some(Target::GnuLlvm)) {
            sh.set_var("RC", "llvm-rc");
        }

        {
            cmd!(
                sh,
                "cargo install --locked chewing-cli --version 0.13.0
                 --root build --target {x86_64_target} --features sqlite-bundled"
            )
            .run()?;
            cmd!(
                sh,
                "cargo build -p chewing_tip {release...} --target {x86_64_target}"
            )
            .run()?;
            cmd!(
                sh,
                "cargo build -p chewing_tip_host {release...} --target {x86_64_target}"
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
                "cargo build -p chewing_tip {release...} --target {aarch64_target}"
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

        sh.create_dir("build/installer")?;
        {
            let _p = sh.push_dir("installer");
            for file in [
                "gpl-notice.rtf",
                "windows-chewing-tsf.wixproj",
                "windows-chewing-tsf.wxs",
                "windows-chewing-tsf.wxl",
                "version.wxi",
                "version.json",
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

        sh.create_dir("build/installer/x64")?;
        sh.copy_file(
            format!("{}/chewing_tip.dll", x86_64_target_dir.display()),
            "build/installer/x64/chewing_tip_x64.dll",
        )?;
        let _ = sh.copy_file(
            format!("{}/chewing_tip.pdb", x86_64_target_dir.display()),
            "build/installer/x64/chewing_tip_x64.pdb",
        );
        sh.copy_file(
            format!("{}/chewing_tip.dll", aarch64_target_dir.display()),
            "build/installer/x64/chewing_tip_arm64.dll",
        )?;
        let _ = sh.copy_file(
            format!("{}/chewing_tip.dll", aarch64_target_dir.display()),
            "build/installer/x64/chewing_tip_arm64.pdb",
        );
        sh.copy_file(
            "platform/arm64x/chewing_tip.dll",
            "build/installer/x64/chewing_tip.dll",
        )?;
        for file in ["chewing_tip_host.exe", "tsfreg.exe"] {
            sh.copy_file(
                format!("{}/{file}", x86_64_target_dir.display()),
                "build/installer",
            )?;
        }
        for file in ["chewing_tip_host.pdb", "tsfreg.pdb"] {
            let _ = sh.copy_file(
                format!("{}/{file}", x86_64_target_dir.display()),
                "build/installer",
            );
        }
        sh.create_dir("build/installer/x86")?;
        sh.copy_file(
            format!("{}/chewing_tip.dll", i686_target_dir.display()),
            "build/installer/x86",
        )?;
        let _ = sh.copy_file(
            format!("{}/chewing_tip.pdb", i686_target_dir.display()),
            "build/installer/x86",
        );

        Ok(())
    })
}

pub(crate) fn package_installer(_flags: PackageInstaller) -> Result<(), Error> {
    expect_error("Failed to package installer", || {
        let sh = Shell::new()?;

        sh.create_dir("dist")?;
        {
            let _p = sh.push_dir("build/installer");
            cmd!(
                sh,
                "wix build -acceptEula wix7 -arch x64 -culture zh-TW -ext WixToolset.UI.wixext
                    -o ../../dist/windows-chewing-tsf-unsigned.msi -pdbtype none
                    windows-chewing-tsf.wxs"
            )
            .run()?;
        }

        Ok(())
    })
}
