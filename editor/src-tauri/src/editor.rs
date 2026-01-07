use std::{path::PathBuf, str::FromStr};

use anyhow::bail;
use anyhow::Result;
use chewing::{
    dictionary::{Dictionary, DictionaryBuilder, DictionaryInfo, Phrase, TrieBuf, TrieBuilder},
    zhuyin::Syllable,
};
use chewing_tip::config::Config;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub(super) struct Entry {
    phrase: String,
    bopomofo: String,
    frequency: u32,
}

#[tauri::command]
pub(super) fn load(path: String) -> Result<Vec<Entry>, String> {
    fn inner(path: String) -> Result<Vec<Entry>> {
        let dict = TrieBuf::open(path)?;
        Ok(dict
            .entries()
            .map(|it| Entry {
                phrase: it.1.to_string(),
                bopomofo: it
                    .0
                    .iter()
                    .map(|syl| syl.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
                frequency: it.1.freq(),
            })
            .collect())
    }
    inner(path).map_err(|e| format!("{:#}", e))
}

#[tauri::command]
pub(super) fn save(path: String, entries: Vec<Entry>) -> Result<(), String> {
    fn inner(path: String, entries: Vec<Entry>) -> Result<()> {
        let mut builder = TrieBuilder::new();
        builder.set_info(DictionaryInfo {
            name: "我的詞庫".to_string(),
            copyright: "Unknown".to_string(),
            license: "Unknown".to_string(),
            version: "1.0.0".to_string(),
            software: "新酷音詞庫管理程式".to_string(),
        })?;
        for entry in entries {
            let mut syllables = vec![];
            for syl in entry
                .bopomofo
                .replace("␣", " ")
                // number one vs. bopomofo I
                .replace("一", "ㄧ")
                .trim()
                .split_whitespace()
                .map(|cluster| Syllable::from_str(&cluster))
            {
                syllables.push(syl?);
            }
            let phrase = Phrase::new(entry.phrase, entry.frequency);
            builder.insert(&syllables, phrase)?;
        }
        builder.build(&PathBuf::from(&path))?;
        // HACK trigger reload
        let cfg = Config::from_reg()?;
        cfg.save_reg();
        Ok(())
    }
    inner(path, entries).map_err(|e| format!("{:#}", e))
}

#[tauri::command]
pub(super) fn validate(bopomofo: String) -> Result<(), String> {
    fn inner(bopomofo: String) -> Result<()> {
        if bopomofo.is_empty() {
            bail!("注音不可為空白");
        }
        for syl in bopomofo
            .replace("␣", " ")
            // number one vs. bopomofo I
            .replace("一", "ㄧ")
            .trim()
            .split_whitespace()
            .map(|cluster| Syllable::from_str(&cluster))
        {
            syl?;
        }
        Ok(())
    }
    inner(bopomofo).map_err(|e| format!("不是正確的注音\n注意：字與字之間須有空格分開\n\n{:#}", e))
}
