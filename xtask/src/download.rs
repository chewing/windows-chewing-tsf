use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::flags::DownloadComponents;

const MANIFEST: [(&str, &str, &str, &str); 3] = [
    (
        "https://codeberg.org/chewing/windows-chewing-preferences/releases/download/v26.4.1.0/windows-chewing-preferences-26.4.1.0-x86_64-pc-windows.zip",
        "https://codeberg.org/chewing/windows-chewing-preferences/releases/download/v26.4.1.0/windows-chewing-preferences-26.4.1.0-x86_64-pc-windows.zip.asc",
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
    // sq-download cannot verify with signer-file yet: https://gitlab.com/sequoia-pgp/sequoia-sq/-/work_items/637
    // cmd!(sh, "sq --overwrite download --url {url} --signature-url {sig_url} --signer-file {cert_file} --output {output}")
    //     .run()
    //     .with_context(err)?;
    cmd!(
        sh,
        "sqv --keyring release.pgp --signature-file {temp_path}/{output}.asc {temp_path}/{output}"
    )
    .run()
    .with_context(err)?;
    cmd!(sh, "unzip -uoj {temp_path}/{output} -d {dest}")
        .run()
        .with_context(err)?;
    Ok(())
}
