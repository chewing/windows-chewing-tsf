// SPDX-License-Identifier: GPL-3.0-or-later

use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use chewing::path::data_dir;
use chewing_tip_config::Config;

fn default_user_path_for_file(file: &str) -> PathBuf {
    let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\unknown".into());
    let user_data_dir = PathBuf::from(user_profile).join("ChewingTextService");
    data_dir().unwrap_or(user_data_dir).join(file)
}

fn user_path_for_file(file: &str) -> Result<PathBuf> {
    let user_file = default_user_path_for_file(file);
    if user_file.exists() {
        return Ok(user_file);
    }
    bail!("使用者檔案 {file} 不存在")
}

// FIXME: provide path info from libchewing
fn system_path_for_file(file: &str) -> Result<PathBuf> {
    let progfiles_x86 =
        env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files(x86)".into());
    let progfiles = env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
    let path_x86 = PathBuf::from(progfiles_x86)
        .join("ChewingTextService\\Dictionary")
        .join(file);
    let path = PathBuf::from(progfiles)
        .join("ChewingTextService\\Dictionary")
        .join(file);
    if path_x86.exists() {
        return Ok(path_x86);
    }
    if path.exists() {
        return Ok(path);
    }
    bail!("系統詞庫 {file} 不存在")
}

#[tauri::command]
pub(crate) fn import_config(path: String) -> Result<Config, String> {
    fn inner(path: &str) -> Result<Config> {
        let content = fs::read_to_string(path).context("無法讀取檔案")?;
        let cfg: Config = toml::from_str(&content).context("檔案內容錯誤")?;
        Ok(cfg)
    }

    inner(&path).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) fn export_config(path: String, config: Config) -> Result<(), String> {
    fn inner(path: &str, config: &Config) -> Result<()> {
        let content = toml::to_string_pretty(config).context("無法匯出設定檔")?;
        fs::write(path, &content).context("無法寫入檔案")?;
        Ok(())
    }

    inner(&path, &config).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) fn load_config() -> Result<Config, String> {
    fn inner() -> Result<Config> {
        let mut cfg = Config::from_reg()?;

        if let Ok(path) = user_path_for_file("symbols.dat") {
            cfg.symbols_dat = fs::read_to_string(path)?.into();
        } else if let Ok(path) = system_path_for_file("symbols.dat") {
            cfg.symbols_dat = fs::read_to_string(path)?.into();
        }

        if let Ok(path) = user_path_for_file("swkb.dat") {
            cfg.swkb_dat = fs::read_to_string(path)?.into();
        } else if let Ok(path) = system_path_for_file("swkb.dat") {
            cfg.swkb_dat = fs::read_to_string(path)?.into();
        }

        Ok(cfg)
    }

    inner().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_config(config: Config) -> Result<(), String> {
    fn inner(config: &Config) -> Result<()> {
        config.save_reg();

        let sys_symbols_dat = system_path_for_file("symbols.dat")
            .and_then(|path| Ok(fs::read_to_string(path)?))
            .unwrap_or_default();
        if config.symbols_dat != sys_symbols_dat {
            let user_symbols_dat_path = default_user_path_for_file("symbols.dat");
            fs::create_dir_all(user_symbols_dat_path.parent().unwrap())?;
            fs::write(user_symbols_dat_path, &config.symbols_dat)?;
        }

        let sys_swkb_dat = system_path_for_file("swkb.dat")
            .and_then(|path| Ok(fs::read_to_string(path)?))
            .unwrap_or_default();
        if config.swkb_dat != sys_swkb_dat {
            let user_swkb_dat_path = default_user_path_for_file("swkb.dat");
            fs::create_dir_all(user_swkb_dat_path.parent().unwrap())?;
            fs::write(user_swkb_dat_path, &config.swkb_dat)?;
        }

        Ok(())
    }

    inner(&config).map_err(|e| e.to_string())
}
