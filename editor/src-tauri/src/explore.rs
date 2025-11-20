use anyhow::Result;
use chewing::dictionary::Dictionary;
use chewing::dictionary::SingleDictionaryLoader;
use chewing::dictionary::TrieBuf;
use chewing::dictionary::UserDictionaryLoader;
use chewing::path::{find_files_by_ext, sys_path_from_env_var};
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct DictionaryItem {
    category: String,
    name: String,
    path: String,
}

#[derive(Serialize)]
pub struct DictionaryInfo {
    name: String,
    copyright: String,
    license: String,
    version: String,
    software: String,
}

impl DictionaryItem {
    fn new(category: &str, dict: &dyn Dictionary) -> DictionaryItem {
        DictionaryItem {
            category: category.to_string(),
            name: dict.about().name,
            path: dict
                .path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "不明路徑".to_string()),
        }
    }
}

// TODO call chewing-cli
#[tauri::command]
pub(super) fn explore() -> Result<Vec<DictionaryItem>, String> {
    fn inner() -> Result<Vec<DictionaryItem>> {
        let loader = SingleDictionaryLoader::new();
        let search_path = sys_path_from_env_var();
        let files = find_files_by_ext(&search_path, &["dat", "sqlite3"]);
        let dictionaries = files
            .iter()
            .filter(|file_name| !file_name.ends_with("chewing.dat"))
            .filter_map(|file_name| loader.guess_format_and_load(&file_name).ok())
            .map(|dict| DictionaryItem::new("系統", dict.as_ref()));
        let user_loader = UserDictionaryLoader::new();
        Ok(dictionaries
            .chain(
                user_loader
                    .load()
                    .into_iter()
                    .map(|dict| DictionaryItem::new("個人", dict.as_ref())),
            )
            .collect())
    }

    inner().map_err(|e| format!("{:#}", e))
}

#[tauri::command]
pub(super) fn info(path: String) -> Result<DictionaryInfo, String> {
    fn inner(path: String) -> Result<DictionaryInfo> {
        let dict = TrieBuf::open(path)?;
        let info = dict.about();
        Ok(DictionaryInfo {
            name: info.name,
            copyright: info.copyright,
            license: info.license,
            version: info.version,
            software: info.software,
        })
    }
    inner(path).map_err(|e| format!("{:#}", e))
}
