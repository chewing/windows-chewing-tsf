use anyhow::Result;
use chewing::dictionary::Dictionary;
use chewing::dictionary::SystemDictionaryLoader;
use chewing::dictionary::TrieBuf;
use chewing::dictionary::UserDictionaryLoader;
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

#[tauri::command]
pub(super) fn explore() -> Result<Vec<DictionaryItem>, String> {
    fn inner() -> Result<Vec<DictionaryItem>> {
        let sys_loader = SystemDictionaryLoader::new();
        let user_loader = UserDictionaryLoader::new();
        Ok(sys_loader
            .load()?
            .into_iter()
            .map(|dict| DictionaryItem::new("系統", dict.as_ref()))
            .chain(
                sys_loader
                    .load_drop_in()?
                    .into_iter()
                    .map(|dict| DictionaryItem::new("擴充", dict.as_ref())),
            )
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
