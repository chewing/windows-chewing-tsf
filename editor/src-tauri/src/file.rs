use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Context, Result};
use chewing::dictionary::UserDictionaryLoader;
use tempfile::NamedTempFile;

#[tauri::command]
pub(super) fn import_file(path: String) -> Result<(), String> {
    fn inner(path: String) -> Result<()> {
        let user_loader = UserDictionaryLoader::new();
        let user_dict = user_loader.load()?;
        let dict_path = user_dict.path().context("無法開啟使用者詞庫")?;
        if path.to_lowercase().ends_with("sqlite3") {
            let temp_csv = NamedTempFile::with_suffix(".csv")?;
            let temp_csv_path = temp_csv.path().to_string_lossy().into_owned();
            let chewing_cli = chewing_cli_path();
            if let Ok(output) = Command::new(chewing_cli)
                .arg("dump")
                .arg("--csv")
                .arg(path)
                .arg(&temp_csv_path)
                .output()
            {
                if !output.status.success() {
                    let error = String::from_utf8_lossy(&output.stderr);
                    bail!("無法匯入字典檔\n\n{error}");
                }
            }
            import_from_csv(temp_csv_path, dict_path)?;
        } else {
            import_from_csv(path, dict_path)?;
        };

        Ok(())
    }
    inner(path).map_err(|e| format!("{:#}", e))
}

fn import_from_csv(src_path: String, dict_path: &Path) -> Result<()> {
    let chewing_cli = chewing_cli_path();
    if let Ok(output) = Command::new(chewing_cli)
        .arg("init")
        .arg("--fix")
        .arg("--csv")
        .arg(src_path)
        .arg(dict_path)
        .output()
    {
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            bail!("無法匯入字典檔\n\n{error}");
        }
    }
    Ok(())
}

#[tauri::command]
pub(super) fn export_file(path: String) -> Result<(), String> {
    fn inner(path: String) -> Result<()> {
        let user_loader = UserDictionaryLoader::new();
        let user_dict = user_loader.load()?;
        let dict_path = user_dict.path().context("無法開啟使用者詞庫")?;
        let chewing_cli = chewing_cli_path();
        if let Ok(output) = Command::new(chewing_cli)
            .arg("dump")
            .arg("--csv")
            .arg(dict_path)
            .arg(path)
            .output()
        {
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                bail!("無法匯出字典檔\n\n{error}");
            }
        }

        Ok(())
    }
    inner(path).map_err(|e| format!("{:#}", e))
}

fn program_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(
        env::var("ProgramW6432")
            .or_else(|_| env::var("ProgramFiles"))
            .or_else(|_| env::var("FrogramFiles(x86)"))?,
    )
    .join("ChewingTextService"))
}

fn chewing_cli_path() -> PathBuf {
    program_dir()
        .map(|prog| prog.join("chewing-cli.exe"))
        .unwrap_or_else(|_| PathBuf::from("chewing-cli.exe"))
}
