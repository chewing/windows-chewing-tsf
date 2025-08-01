use anyhow::Result;
use log::{error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use windows_registry::LOCAL_MACHINE;
use xshell::{Shell, cmd};

#[derive(Debug)]
struct MsiProduct {
    name: String,
    version: Option<String>,
    uninstall_string: Option<String>,
    product_code: Option<String>,
    registry_path: String,
}

struct CleanupTool {
    shell: Shell,
    install_paths: Vec<PathBuf>,
}

impl CleanupTool {
    fn new() -> Self {
        let install_paths = vec![
            PathBuf::from(std::env::var("ProgramFiles").unwrap_or_default())
                .join("ChewingTextService"),
            PathBuf::from(std::env::var("ProgramFiles(x86)").unwrap_or_default())
                .join("ChewingTextService"),
        ];

        Self {
            shell: Shell::new().unwrap(),
            install_paths,
        }
    }

    fn find_msi_products(&self) -> Result<Vec<MsiProduct>> {
        info!("搜尋已安裝的新酷音TSF...");
        let mut products = Vec::new();

        // Search in both 32-bit and 64-bit registry locations
        let registry_paths = vec![
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ];

        for reg_path in registry_paths {
            if let Ok(uninstall_key) = LOCAL_MACHINE.open(reg_path) {
                for subkey_name in uninstall_key.keys()? {
                    if let Ok(product_key) = uninstall_key.open(&subkey_name) {
                        if let Ok(display_name) = product_key.get_string("DisplayName") {
                            if display_name.contains("新酷音") {
                                let version = product_key.get_string("DisplayVersion").ok();

                                let uninstall_string =
                                    product_key.get_string("UninstallString").ok();

                                products.push(MsiProduct {
                                    name: display_name.to_string(),
                                    version,
                                    uninstall_string,
                                    product_code: Some(subkey_name.clone()),
                                    registry_path: format!("{}\\{}", reg_path, subkey_name),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(products)
    }

    fn check_installation_paths(&self) -> HashMap<PathBuf, Vec<PathBuf>> {
        info!("檢查安裝目錄...");
        let mut found_paths = HashMap::new();

        for path in &self.install_paths {
            if path.exists() {
                match self.get_directory_contents(path) {
                    Ok(files) => {
                        warn!("找到: {} ({} 個檔案)", path.display(), files.len());
                        for file in &files {
                            warn!("  - {}", file.display());
                        }
                        found_paths.insert(path.clone(), files);
                    }
                    Err(e) => {
                        error!("無法讀取 {}: {}", path.display(), e);
                    }
                }
            } else {
                info!("查無檔案: {}", path.display());
            }
        }

        found_paths
    }

    fn get_directory_contents(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                files.push(path.clone());

                if path.is_dir() {
                    collect_files(&path, files)?;
                }
            }
            Ok(())
        }

        collect_files(path, &mut files)?;
        Ok(files)
    }

    fn attempt_standard_uninstall(&self, products: &[MsiProduct]) -> bool {
        info!("嘗試一般反安裝程序...");

        for product in products {
            if let Some(uninstall_string) = &product.uninstall_string {
                info!("嘗試反安裝: {}", product.name);

                // Parse msiexec command
                if uninstall_string.to_lowercase().starts_with("msiexec") {
                    let cmd_parts: Vec<&str> =
                        uninstall_string.split_whitespace().skip(1).collect();
                    match cmd!(self.shell, "msiexec {cmd_parts...} /quiet /norestart").run() {
                        Ok(_) => {
                            info!("一般反安裝程序成功完成。");
                            return true;
                        }
                        Err(e) => {
                            error!("反安裝程序失敗: {}", e);
                        }
                    }
                }
            }
        }

        // Try product code uninstall
        for product in products {
            if let Some(product_code) = &product.product_code {
                if product_code.starts_with('{') && product_code.ends_with('}') {
                    info!("嘗試使用 Product Code 反安裝: {}", product_code);

                    match cmd!(self.shell, "msiexec /x {product_code} /quiet /norestart").run() {
                        Ok(_) => {
                            info!("Product Code 反安裝程序成功完成。");
                            return true;
                        }
                        Err(e) => {
                            error!("Product Code 反安裝程序失敗: {}", e);
                        }
                    }
                }
            }
        }

        false
    }

    fn force_cleanup(&self, products: &[MsiProduct]) -> Result<()> {
        warn!("強制執行清理程序...");

        // Remove files
        self.remove_installation_files()?;

        // Clean registry
        self.clean_registry_entries(products)?;

        // Clean MSI cache
        // self.clean_msi_cache()?;

        Ok(())
    }

    fn remove_installation_files(&self) -> Result<()> {
        for path in &self.install_paths {
            if path.exists() {
                info!("移除目錄: {}", path.display());

                match fs::remove_dir_all(path) {
                    Ok(_) => {
                        info!("目錄已成功移除！");
                    }
                    Err(e) => {
                        error!("無法移除目錄: {e}");
                        error!("嘗試強制移除...");

                        // Try force removal with takeown and icacls
                        let path_str = path.to_string_lossy().into_owned();
                        let _ = cmd!(self.shell, "takeown /f {path_str} /r /d y").run();
                        let _ =
                            cmd!(self.shell, "icacls {path_str} /grant administrators:F /t").run();
                        let _ = cmd!(self.shell, "rmdir /s /q {path_str}").run();
                    }
                }
            }
        }
        Ok(())
    }

    fn clean_registry_entries(&self, products: &[MsiProduct]) -> Result<()> {
        info!("清理登錄檔...");

        for product in products {
            let reg_path_parts: Vec<&str> = product.registry_path.split('\\').collect();
            if reg_path_parts.len() >= 2 {
                let key_path = reg_path_parts[0..reg_path_parts.len() - 1].join("\\");
                let subkey_name = reg_path_parts[reg_path_parts.len() - 1];

                if let Ok(parent_key) = LOCAL_MACHINE.open(&key_path) {
                    match parent_key.remove_tree(subkey_name) {
                        Ok(_) => {
                            info!("成功移除登錄檔: LOCAL_MACHINE\\{}", product.registry_path);
                        }
                        Err(e) => {
                            error!("無法移除登錄檔: {e}");
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // fn clean_msi_cache(&self) -> Result<()> {
    //     info!("清理殘留 MSI 快取...");

    //     let windows_dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    //     let installer_cache = PathBuf::from(windows_dir).join("Installer");

    //     if installer_cache.exists() {
    //         if let Ok(entries) = fs::read_dir(&installer_cache) {
    //             for entry in entries.flatten() {
    //                 if let Some(ext) = entry.path().extension() {
    //                     if ext == "msi" {
    //                         // This is a simplified check - in practice you'd want to
    //                         // examine MSI properties to confirm it's related to ChewingTextService
    //                         // if let Some(name) = entry.file_name().to_str() {
    //                         //     if name.to_lowercase().contains("chewing") {
    //                         //         self.log("INFO", &format!("Removing cached MSI: {}", name));
    //                         //         let _ = fs::remove_file(entry.path());
    //                         //     }
    //                         // }
    //                     }
    //                 }
    //             }
    //         }
    //     }

    //     Ok(())
    // }

    fn run_cleanup(&self) -> Result<()> {
        info!("=== 新酷音TSF疑難排解程序 ===");

        let products = self.find_msi_products()?;

        if products.is_empty() {
            info!("登錄檔中沒有找到已安裝的新酷音TSF");
        } else {
            info!("找到相關的新酷音TSF安裝資訊:");
            for product in &products {
                println!("  名稱: {}", product.name);
                if let Some(version) = &product.version {
                    println!("  版本: {}", version);
                }
                if let Some(code) = &product.product_code {
                    println!("  序號: {}", code);
                }
                println!("  登錄檔: LOCAL_MACHINE\\{}", product.registry_path);
                println!();
            }
        }

        let found_paths = self.check_installation_paths();

        if found_paths.is_empty() {
            info!("沒有找到相關的資料檔案");
        }

        info!("分析完畢");
        info!("=== 新酷音TSF清理程序 ===");

        let mut standard_success = false;

        if !products.is_empty() {
            if let Ok(true) = confirm("要執行反安裝程式嗎？") {
                standard_success = self.attempt_standard_uninstall(&products);
            }
        }

        if !standard_success {
            if let Ok(true) = confirm("要執行強制清理程序嗎？") {
                self.force_cleanup(&products)?;
            } else {
                info!("中斷清理程序...");
                return Ok(());
            }
        }

        // Final verification
        info!("=== 確認結果 ===");
        let final_products = self.find_msi_products()?;
        let final_paths = self.check_installation_paths();

        if final_products.is_empty() && final_paths.is_empty() {
            info!("成功清理完成");
        } else {
            warn!("部份資料檔案仍然無法移除，須手動清理：");
            for product in final_products {
                info!("登錄檔: LOCAL_MACHINE\\{}", product.registry_path);
            }
            for path in final_paths {
                info!("目錄: {}", path.0.display());
            }
        }

        Ok(())
    }
}

fn confirm(plan: &str) -> Result<bool> {
    print!("{plan} (y/N): ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
        return Ok(true);
    }
    Ok(false)
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Check for admin privileges
    if !matches!(is_elevated(), Ok(true)) {
        error!("此程式需要以管理員權限執行");
        process::exit(1);
    }

    let tool = CleanupTool::new();
    if let Err(e) = tool.run_cleanup() {
        error!("失敗: {e}");
        process::exit(1);
    }
}

fn is_elevated() -> Result<bool> {
    use std::mem;

    unsafe {
        let mut token_handle = windows::Win32::Foundation::HANDLE::default();
        let current_process = windows::Win32::System::Threading::GetCurrentProcess();

        if windows::Win32::System::Threading::OpenProcessToken(
            current_process,
            windows::Win32::Security::TOKEN_QUERY,
            &mut token_handle,
        )
        .is_err()
        {
            return Ok(false);
        }

        let mut elevation = windows::Win32::Security::TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = 0u32;

        let result = windows::Win32::Security::GetTokenInformation(
            token_handle,
            windows::Win32::Security::TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            mem::size_of::<windows::Win32::Security::TOKEN_ELEVATION>() as u32,
            &mut size,
        );

        let _ = windows::Win32::Foundation::CloseHandle(token_handle);

        Ok(result.is_ok() && elevation.TokenIsElevated != 0)
    }
}
