use std::fs::File;
use std::io::Write;

use jiff::Zoned;

mod flags {
    xflags::xflags! {
        src "src/main.rs"

        /// cargo-xtask helper
        cmd xtask {
            /// Update the version.rc file.
            cmd update-version {
                /// The major version of the release. (u32)
                required --major MAJOR: u32
                /// The minor version of the release. (u32)
                required --minor MINOR: u32
                /// The patch version of the release. (u32)
                required --patch PATCH: u32
                /// Optional build number (u32)
                optional -b, --build BUILD_NUMBER: u32
            }
        }
    }
    // generated start
    // The following code is generated by `xflags` macro.
    // Run `env UPDATE_XFLAGS=1 cargo build` to regenerate.
    #[derive(Debug)]
    pub struct Xtask {
        pub subcommand: XtaskCmd,
    }

    #[derive(Debug)]
    pub enum XtaskCmd {
        UpdateVersion(UpdateVersion),
    }

    #[derive(Debug)]
    pub struct UpdateVersion {
        pub major: u32,
        pub minor: u32,
        pub patch: u32,
        pub build: Option<u32>,
    }

    impl Xtask {
        #[allow(dead_code)]
        pub fn from_env_or_exit() -> Self {
            Self::from_env_or_exit_()
        }

        #[allow(dead_code)]
        pub fn from_env() -> xflags::Result<Self> {
            Self::from_env_()
        }

        #[allow(dead_code)]
        pub fn from_vec(args: Vec<std::ffi::OsString>) -> xflags::Result<Self> {
            Self::from_vec_(args)
        }
    }
    // generated end
}

fn main() -> anyhow::Result<()> {
    let flags = flags::Xtask::from_env()?;

    match flags.subcommand {
        flags::XtaskCmd::UpdateVersion(update_version) => {
            let now = Zoned::now();
            let year = now.year();
            let month = now.month();
            let day = now.day();
            let yy = update_version.major;
            let mm = update_version.minor;
            let rv = update_version.patch;
            let bn = update_version.build.unwrap_or_default();
            let mut version_rc = File::create("version.rc")?;
            indoc::writedoc!(
                version_rc,
                r#"
                    #define VER_FILEVERSION             {yy},{mm},{rv},{bn}
                    #define VER_FILEVERSION_STR         "{yy}.{mm}.{rv}.{bn}\0"
                    #define VER_PRODUCTVERSION          {yy},{mm},{rv},{bn}
                    #define VER_PRODUCTVERSION_STR      "{yy}.{mm}.{rv}.{bn}\0"
                    #define ABOUT_CAPTION_WITH_VER      "關於新酷音輸入法 ({yy}.{mm}.{rv}.{bn})\0"
                    #define ABOUT_VERSION_STR           "版本：{yy}.{mm}.{rv}.{bn}\0"
                    #define ABOUT_RELEASE_DATE_STR      "發行日期：{year} 年 {month:02} 月 {day:02} 日\0"
                    #define PREFS_TITLE_WITH_VER        "設定新酷音輸入法 ({yy}.{mm}.{rv}.{bn})\0"
                "#
            )?;

            let mut version_slint = File::create("preferences/ui/version.slint")?;
            indoc::writedoc!(
                version_slint,
                r#"
                    export global Version {{
                        out property <string> product-version: "{yy}.{mm}.{rv}.{bn}";
                        out property <string> build-date: "{year} 年 {month:02} 月 {day:02} 日";
                    }}
                "#
            )?;

            let mut version_wxi = File::create("installer/version.wxi")?;
            indoc::writedoc!(
                version_wxi,
                r#"
                    <?xml version="1.0" encoding="UTF-8"?>
                    <Include>
                        <?define Version = "{yy}.{mm}.{rv}.{bn}"?>
                    </Include>
                "#
            )?;
        }
    }

    Ok(())
}
