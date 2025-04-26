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

    sh.set_var("SQLITE3_STATIC", "1");
    sh.set_var("RUSTFLAGS", "-Ctarget-feature=+crt-static");

    // Ensure chewing-cli is installed
    cmd!(sh, "chewing-cli -V").run()?;

    {
        let _env = if matches!(flags.target, Some(Target::Msvc)) {
            None
        } else {
            Some(sh.push_env(
                "SQLITE3_LIB_DIR",
                "/usr/x86_64-w64-mingw32/sys-root/mingw/lib/",
            ))
        };
        cmd!(
            sh,
            "cargo install --locked chewing-cli --root build --target {x86_64_target}"
        )
        .run()?;
        cmd!(
            sh,
            "cargo build -p chewing_tip {release...} --target {x86_64_target}"
        )
        .run()?;
        cmd!(
            sh,
            "cargo build -p chewing-preferences {release...} --target {x86_64_target}"
        )
        .run()?;
        cmd!(
            sh,
            "cargo build -p tsfreg {release...} --target {x86_64_target}"
        )
        .run()?;
    }
    {
        let _env = if matches!(flags.target, Some(Target::Msvc)) {
            None
        } else {
            Some(sh.push_env(
                "SQLITE3_LIB_DIR",
                "/usr/i686-w64-mingw32/sys-root/mingw/lib/",
            ))
        };
        cmd!(
            sh,
            "cargo build -p chewing_tip {release...} --target {i686_target}"
        )
        .run()?;
    }

    sh.remove_path("build/installer")?;
    sh.create_dir("build/installer/assets")?;
    {
        let _p = sh.push_dir("assets");
        for file in ["bubble.9.png", "msg.9.png"] {
            sh.copy_file(file, "../build/installer/assets")?;
        }
    }
    {
        let _p = sh.push_dir("installer");
        for file in [
            "lgpl-2.1.rtf",
            "windows-chewing-tsf.wixproj",
            "windows-chewing-tsf.wxs",
            "windows-chewing-tsf.wxl",
            "version.wxi",
        ] {
            sh.copy_file(file, "../build/installer")?;
        }
    }
    sh.copy_file("chewing_tip/rc/im.chewing.Chewing.ico", "build/installer/chewing.ico")?;
    sh.copy_file("build/bin/chewing-cli.exe", "build/installer")?;

    sh.create_dir("build/installer/Dictionary")?;
    {
        let _p = sh.push_dir("libchewing/data");
        for file in ["swkb.dat", "symbols.dat"] {
            sh.copy_file(file, "../../build/installer/Dictionary")?;
        }
    }
    let copyright = "Copyright (c) 2025 libchewing Core Team";
    let license = "LGPL-2.1-or-later";
    let revision = "2025.04.11";
    cmd!(
        sh,
        "chewing-cli init-database -c {copyright} -l {license} -r {revision} -t trie -n 內建詞庫
                  libchewing/data/tsi.src
                  build/installer/Dictionary/tsi.dat"
    )
    .run()?;
    cmd!(
        sh,
        "chewing-cli init-database -c {copyright} -l {license} -r {revision} -t trie -n 內建字庫
                  libchewing/data/word.src
                  build/installer/Dictionary/word.dat"
    )
    .run()?;

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
    for file in ["ChewingPreferences.exe", "tsfreg.exe"] {
        sh.copy_file(
            format!("{}/{file}", x86_64_target_dir.display()),
            "build/installer",
        )?;
    }
    for file in ["ChewingPreferences.pdb", "tsfreg.pdb"] {
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
