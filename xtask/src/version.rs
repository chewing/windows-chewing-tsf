// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::Write;

use anyhow::Result;
use jiff::Zoned;

use super::flags::UpdateVersion;

pub(super) fn update_version(flags: UpdateVersion) -> Result<()> {
    let now = Zoned::now();
    let year = now.year();
    let month = now.month();
    let day = now.day();
    let yy = flags.major;
    let mm = flags.minor;
    let rv = flags.patch;
    let bn = flags.build.unwrap_or_default();
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

    let mut pref_version_slint = File::create("preferences/src/version.tsx")?;
    indoc::writedoc!(
        pref_version_slint,
        r#"
            const version = {{
                productVersion: "{yy}.{mm}.{rv}.{bn}",
                buildDate: "{year} 年 {month:02} 月 {day:02} 日",
            }};
            export default version;
        "#
    )?;

    let mut editor_version_slint = File::create("editor/ui/version.slint")?;
    indoc::writedoc!(
        editor_version_slint,
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
    Ok(())
}
