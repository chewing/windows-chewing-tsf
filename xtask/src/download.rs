use std::{ffi::OsString, io::Cursor, path::PathBuf};

use rpgpie_sop::{Certs, RPGSOP, Sigs};
use scoped_error::{Error, expect_error, expect_error_fn};
use sop::{Load, SOP};
use xshell::{Shell, cmd};

use crate::{flags::DownloadComponents, zip::unzip};

const MANIFEST: [(&str, &str, &str, &str); 3] = [
    (
        "https://codeberg.org/chewing/windows-chewing-preferences/releases/download/v26.6.0.1/windows-chewing-preferences-26.6.0.1-x86_64-pc-windows.zip",
        "https://codeberg.org/chewing/windows-chewing-preferences/releases/download/v26.6.0.1/windows-chewing-preferences-26.6.0.1-x86_64-pc-windows.zip.asc",
        "windows-chewing-preferences.zip",
        "build/installer",
    ),
    (
        "https://codeberg.org/chewing/windows-chewing-editor/releases/download/v26.4.1.0/windows-chewing-editor-26.4.1.0-x86_64-pc-windows.zip",
        "https://codeberg.org/chewing/windows-chewing-editor/releases/download/v26.4.1.0/windows-chewing-editor-26.4.1.0-x86_64-pc-windows.zip.asc",
        "windows-chewing-editor.zip",
        "build/installer",
    ),
    (
        "https://codeberg.org/chewing/libchewing-data/releases/download/v2026.3.22/libchewing-data-2026.3.22-Generic.zip",
        "https://codeberg.org/chewing/libchewing-data/releases/download/v2026.3.22/libchewing-data-2026.3.22-Generic.zip.asc",
        "libchewing-data.zip",
        "build/installer/Dictionary",
    ),
];

pub(crate) fn download_components(_flags: DownloadComponents) -> Result<(), Error> {
    expect_error("failed to download components", || {
        for component in MANIFEST {
            let (url, sig_url, output, dest) = component;
            sq_download(url, sig_url, "release.pgp", output, dest)?;
        }
        Ok(())
    })
}

fn sq_download(
    url: &str,
    sig_url: &str,
    cert_file: &str,
    src: &str,
    dest: &str,
) -> Result<(), Error> {
    let err = || {
        Error::new(format!(
            "failed to download file\n      url: {url}\nsignature: {sig_url}\n     cert: {cert_file}"
        ))
    };
    expect_error_fn(err, || {
        let sh = Shell::new()?;
        let temp_dir = sh.create_temp_dir()?;
        let src = temp_dir.path().join(src);
        let dest = PathBuf::from(dest);

        sh.create_dir(&dest)?;

        cmd!(sh, "curl -L -o {src} {url}").run()?;
        cmd!(sh, "curl -L -o {src}.asc {sig_url}").run()?;

        let sop = RPGSOP::default();
        let certs = Certs::from_file(&sop, "release.pgp")?;

        let mut sig_path = src.clone();
        let extension = sig_path.extension().map_or(OsString::from("asc"), |ext| {
            let mut ext = ext.to_os_string();
            ext.push(".asc");
            ext
        });
        sig_path.set_extension(extension);
        let sig = Sigs::from_file(&sop, sig_path)?;
        let data = std::fs::read(&src)?;
        let mut cursor = Cursor::new(data);
        let verifications = sop
            .verify()?
            .certs(&certs)?
            .signatures(&sig)?
            .data(&mut cursor)?;
        if verifications.is_empty() {
            Err("unable to verify signature")?;
        }

        cursor.set_position(0);
        unzip(&dest, cursor)?;
        Ok(())
    })
}
