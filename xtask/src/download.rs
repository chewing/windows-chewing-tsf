use std::{ffi::OsString, io::Cursor};

use anyhow::{Context, Result, bail};
use rpgpie_sop::{Certs, RPGSOP, Sigs};
use sop::{Load, SOP};
use xshell::{Shell, cmd};

use crate::flags::DownloadComponents;

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

pub(crate) fn download_components(_flags: DownloadComponents) -> Result<()> {
    let err = || format!("failed to download components");
    for component in MANIFEST {
        let (url, sig_url, output, dest) = component;
        sq_download(url, sig_url, "release.pgp", output, dest).with_context(err)?;
    }
    Ok(())
}

fn sq_download(url: &str, sig_url: &str, cert_file: &str, output: &str, dest: &str) -> Result<()> {
    let err = || {
        format!(
            "failed to download file\n      url: {url}\nsignature: {sig_url}\n     cert: {cert_file}"
        )
    };
    let sh = Shell::new().with_context(err)?;
    let temp_dir = sh.create_temp_dir().with_context(err)?;
    let temp_path = temp_dir.path().as_os_str();

    cmd!(sh, "curl -L -o {temp_path}/{output} {url}")
        .run()
        .with_context(err)?;
    cmd!(sh, "curl -L -o {temp_path}/{output}.asc {sig_url}")
        .run()
        .with_context(err)?;

    let sop = RPGSOP::default();
    let certs = Certs::from_file(&sop, "release.pgp")?;

    let mut sig_path = temp_dir.path().join(output);
    let extension = sig_path.extension().map_or(OsString::from("asc"), |ext| {
        let mut ext = ext.to_os_string();
        ext.push(".asc");
        ext
    });
    sig_path.set_extension(extension);
    let sig = Sigs::from_file(&sop, sig_path)?;
    let data = std::fs::read(temp_dir.path().join(output))?;
    let mut cursor = Cursor::new(data);
    let verifications = sop
        .verify()?
        .certs(&certs)?
        .signatures(&sig)?
        .data(&mut cursor)?;
    if verifications.is_empty() {
        bail!("unable to verify");
    }
    cmd!(sh, "unzip -uoj {temp_path}/{output} -d {dest}")
        .run()
        .with_context(err)?;
    Ok(())
}
