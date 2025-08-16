// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::RwLock;
use std::{env, iter};

use anyhow::{Result, anyhow};
use chewing::dictionary::{
    Dictionary, DictionaryBuilder, SystemDictionaryLoader, TrieBuilder, UserDictionaryLoader,
};
use chewing::dictionary::{DictionaryInfo, Phrase};
use chewing::zhuyin::Syllable;
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
use slint::{
    ComponentHandle, Model, ModelExt, ModelNotify, ModelRc, ModelTracker, SharedString,
    StandardListViewItem, VecModel,
};

use crate::AboutWindow;
use crate::CallbackResult;
use crate::DictEntriesAdapter;
use crate::EditorWindow;
use crate::ErrorKind;

pub fn run() -> Result<()> {
    let ui = EditorWindow::new()?;
    let about = AboutWindow::new()?;

    ui.set_dictionaries(dict_list_model()?);

    let about_handle = about.as_weak();
    about.on_done(move || {
        let about = about_handle.upgrade().unwrap();
        about.hide().unwrap();
    });
    ui.on_quit(move || {
        slint::quit_event_loop().unwrap();
    });
    ui.on_about(move || {
        about.show().unwrap();
    });

    let ui_handle = ui.as_weak();
    ui.on_reload_dict_info(move || {
        let ui = ui_handle.upgrade().unwrap();
        // FIXME panic on error
        ui.set_dictionaries(dict_list_model().expect("unable to load dict info"));
    });

    let ui_handle = ui.as_weak();
    ui.on_info_clicked(move |row: ModelRc<StandardListViewItem>| {
        let dict_item = row
            .as_any()
            .downcast_ref::<DictTableItemModel>()
            .expect("row item should be a DictTableItemModel");
        let dict_model = ModelRc::new(DictInfoViewModel::from(dict_item));

        let ui = ui_handle.upgrade().unwrap();
        ui.set_dictionary_info(dict_model.clone());
    });

    let ui_handle = ui.as_weak();
    ui.on_edit_dict_clicked(move |row: ModelRc<StandardListViewItem>| {
        let dict_item = row
            .as_any()
            .downcast_ref::<DictTableItemModel>()
            .expect("row item should be a DictTableItemModel");
        let dict_model = ModelRc::new(DictEditViewModel::from(dict_item));

        let ui = ui_handle.upgrade().unwrap();
        ui.global::<DictEntriesAdapter>().set_entries(dict_model);
    });
    let ui_handle = ui.as_weak();
    ui.on_edit_dict_import(move |row: ModelRc<StandardListViewItem>| {
        let dict_item = row
            .as_any()
            .downcast_ref::<DictTableItemModel>()
            .expect("row item should be a DictTableItemModel");
        if let Ok(dict) = dict_item.1.read()
            && let Some(dict_path) = dict.path()
        {
            let dict_path = dict_path.to_owned();
            let ui_handle = ui_handle.clone();
            // XXX spawn MessageDialog in a different thread to avoid lock-up
            // slint's UI thread. In theory this is not necessary because
            // MessageBox spins the event loop. But in practice this is required
            // for current slint (1.12.1).
            std::thread::spawn(move || {
                let ok_cancel = MessageDialog::new()
                    .set_level(MessageLevel::Warning)
                    .set_title("警告")
                    .set_description("匯入字典檔會覆蓋現有字典資料")
                    .set_buttons(MessageButtons::OkCancel)
                    .show();
                if ok_cancel == MessageDialogResult::Cancel {
                    return;
                }
                if let Some(src_path) = FileDialog::new()
                    .set_directory(default_desktop_path())
                    .add_filter("CSV", &["csv"])
                    .pick_file()
                {
                    let chewing_cli = chewing_cli_path();
                    if let Ok(output) = Command::new(chewing_cli)
                        .arg("init")
                        .arg("-k")
                        .arg("--fix")
                        .arg("--csv")
                        .arg(src_path)
                        .arg(dict_path)
                        .output()
                    {
                        if !output.status.success() {
                            let error = String::from_utf8_lossy(&output.stderr);
                            MessageDialog::new()
                                .set_level(MessageLevel::Error)
                                .set_title("錯誤")
                                .set_description(format!("無法匯入字典檔\n\n{error}"))
                                .set_buttons(MessageButtons::Ok)
                                .show();
                            return;
                        }
                    }
                }
                let _ = ui_handle.upgrade_in_event_loop(|ui| {
                    ui.set_dictionaries(dict_list_model().expect("unable to load dict info"));
                });
            });
        }
    });
    ui.on_edit_dict_export(move |row: ModelRc<StandardListViewItem>| {
        let dict_item = row
            .as_any()
            .downcast_ref::<DictTableItemModel>()
            .expect("row item should be a DictTableItemModel");
        if let Ok(dict) = dict_item.1.read()
            && let Some(dict_path) = dict.path()
        {
            let dict_path = dict_path.to_owned();
            // XXX spawn MessageDialog in a different thread to avoid lock-up
            // slint's UI thread. In theory this is not necessary because
            // MessageBox spins the event loop. But in practice this is required
            // for current slint (1.12.1).
            std::thread::spawn(move || {
                if let Some(csv_path) = FileDialog::new()
                    .set_directory(default_desktop_path())
                    .add_filter("CSV", &["csv"])
                    .save_file()
                {
                    let chewing_cli = chewing_cli_path();
                    if let Ok(output) = Command::new(chewing_cli)
                        .arg("dump")
                        .arg("--csv")
                        .arg(dict_path)
                        .arg(csv_path)
                        .output()
                    {
                        if !output.status.success() {
                            let error = String::from_utf8_lossy(&output.stderr);
                            MessageDialog::new()
                                .set_level(MessageLevel::Error)
                                .set_title("錯誤")
                                .set_description(format!("無法匯出字典檔\n\n{error}"))
                                .set_buttons(MessageButtons::Ok)
                                .show();
                            return;
                        }
                    }
                }
            });
        }
    });

    ui.global::<DictEntriesAdapter>()
        .on_filter_sort_model(filter_sort_model);

    let ui_handle = ui.as_weak();
    ui.global::<DictEntriesAdapter>().on_update_entry(
        move |search_text, sort_index, sort_ascending, current_row, data| {
            let ui = ui_handle.upgrade().unwrap();
            // verification
            let bopomofo = data.row_data(1).unwrap_or_default().text;
            let freq = data.row_data(2).unwrap_or_default().text;
            match freq.parse::<u32>() {
                Ok(v) => v,
                Err(_) => {
                    return CallbackResult {
                        error: ErrorKind::Other,
                        err_msg: slint::format!("無法辨認 {freq} 為數字"),
                    };
                }
            };
            let syllables = bopomofo
                .replace("␣", " ")
                // number one vs. bopomofo I
                .replace("一", "ㄧ")
                .trim()
                .split_whitespace()
                .map(|cluster| Syllable::from_str(&cluster))
                .collect::<Vec<_>>();
            for err in syllables.iter() {
                if err.is_err() {
                    log::error!("{err:?}");
                }
            }
            if syllables.iter().any(|syl| syl.is_err()) {
                let ellipsis = if bopomofo.len() > 20 { 3 } else { 0 };
                let sample = bopomofo
                    .chars()
                    .take(20)
                    .chain(iter::repeat_n('.', ellipsis))
                    .collect::<String>();
                return CallbackResult {
                    error: ErrorKind::Other,
                    err_msg: slint::format!(
                        "{sample} 不是正確的注音\n注意：字與字之間須有 ␣ 或是空格分開"
                    ),
                };
            }
            // save data
            let entries_rc = ui.global::<DictEntriesAdapter>().get_entries();
            let wrapped_entries =
                filter_sort_model(entries_rc, search_text, sort_index, sort_ascending);
            // HACK: initialize the mapping
            let _ = wrapped_entries.row_data(0);
            wrapped_entries.set_row_data(current_row as usize, data);
            CallbackResult {
                error: ErrorKind::Ok,
                ..Default::default()
            }
        },
    );

    let ui_handle = ui.as_weak();
    ui.on_edit_entry_new(move || {
        let ui = ui_handle.upgrade().unwrap();
        let entries_rc = ui.global::<DictEntriesAdapter>().get_entries();
        let entry = entries_rc
            .as_any()
            .downcast_ref::<DictEditViewModel>()
            .expect("entries should be a DictEditViewModel");
        let _ = entry
            .cache
            .write()
            .map(|mut cache| {
                let phrase = Phrase::new("", 0);
                cache.insert(0, (vec![], phrase));
                0
            })
            .map(|index| {
                entry.tracker.row_added(index, 1);
            });
    });

    let ui_handle = ui.as_weak();
    ui.on_edit_entry_delete(move || {
        let ui = ui_handle.upgrade().unwrap();
        let index = ui.get_edit_dict_current_row() as usize;
        let entries_rc = ui.global::<DictEntriesAdapter>().get_entries();
        let entry = entries_rc
            .as_any()
            .downcast_ref::<DictEditViewModel>()
            .expect("entries should be a DictEditViewModel");
        if let Ok(mut cache) = entry.cache.write() {
            if index < cache.len() {
                cache.remove(index);
                entry.tracker.row_removed(index, 1);
            }
        }
    });

    let ui_handle = ui.as_weak();
    ui.on_edit_dict_save(move || -> CallbackResult {
        let ui = ui_handle.upgrade().unwrap();
        let entries_rc = ui.global::<DictEntriesAdapter>().get_entries();
        let entry = entries_rc
            .as_any()
            .downcast_ref::<DictEditViewModel>()
            .expect("entries should be a DictEditViewModel");
        entry
            .cache
            .read()
            .map_err(|_| anyhow!("should be able to read cache"))
            .and_then(|cache| -> Result<()> {
                entry
                    .dict
                    .read()
                    .map_err(|_| anyhow!("should be able to read dict"))
                    .and_then(|dict| -> Result<()> {
                        // FIXME detect original dict format
                        let path = dict.path().expect("dict should have file path");
                        let mut builder = TrieBuilder::new();
                        builder.set_info(DictionaryInfo {
                            software: format!(
                                "{} {}",
                                env!("CARGO_PKG_NAME"),
                                env!("CARGO_PKG_VERSION")
                            ),
                            ..dict.about()
                        })?;
                        for (syls, phrase) in cache.iter() {
                            builder.insert(&syls, phrase.clone())?;
                        }
                        builder.build(path)?;
                        Ok(())
                    })
            })
            .map(|_| CallbackResult {
                error: ErrorKind::Ok,
                ..Default::default()
            })
            .unwrap_or_else(|_e| CallbackResult {
                error: ErrorKind::Other,
                err_msg: "無法寫入檔案".into(),
            })
    });

    ui.run()?;

    Ok(())
}

fn dict_list_model() -> Result<ModelRc<ModelRc<StandardListViewItem>>> {
    // FIXME: stop relying on global CHEWING_PATH environment variable
    let sys_loader = SystemDictionaryLoader::new();
    let user_loader = UserDictionaryLoader::new();

    Ok(ModelRc::new(VecModel::from_iter(
        sys_loader
            .load()?
            .into_iter()
            .map(|dict| ModelRc::new(DictTableItemModel::new("系統", dict)))
            .chain(
                sys_loader
                    .load_drop_in()?
                    .into_iter()
                    .map(|dict| ModelRc::new(DictTableItemModel::new("擴充", dict))),
            )
            .chain(
                user_loader
                    .load()
                    .into_iter()
                    .map(|dict| ModelRc::new(DictTableItemModel::new("個人", dict))),
            ),
    )))
}

struct DictTableItemModel(&'static str, Rc<RwLock<Box<dyn Dictionary>>>);

impl DictTableItemModel {
    fn new(typ: &'static str, dict: Box<dyn Dictionary>) -> DictTableItemModel {
        DictTableItemModel(typ, Rc::new(RwLock::new(dict)))
    }
}

impl Model for DictTableItemModel {
    type Data = StandardListViewItem;

    fn row_count(&self) -> usize {
        3
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        match row {
            0 => Some(self.0.into()),
            1 => self
                .1
                .read()
                .ok()
                .map(|dict| dict.about().name.as_str().into()),
            2 => self
                .1
                .read()
                .ok()
                .map(|dict| dict.path().map(|p| p.to_string_lossy().as_ref().into()))
                .flatten(),
            _ => None,
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

struct DictInfoViewModel(&'static str, DictionaryInfo);

impl From<&DictTableItemModel> for DictInfoViewModel {
    fn from(value: &DictTableItemModel) -> Self {
        DictInfoViewModel(
            value.0,
            value
                .1
                .read()
                .expect("should have no concurrent write")
                .about(),
        )
    }
}

impl Model for DictInfoViewModel {
    type Data = ModelRc<StandardListViewItem>;

    fn row_count(&self) -> usize {
        6
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        match row {
            0 => Some(ModelRc::from(["來源".into(), self.0.into()])),
            1 => Some(ModelRc::from(["名稱".into(), self.1.name.as_str().into()])),
            2 => Some(ModelRc::from([
                "版本".into(),
                self.1.version.as_str().into(),
            ])),
            3 => Some(ModelRc::from([
                "著作權".into(),
                self.1.copyright.as_str().into(),
            ])),
            4 => Some(ModelRc::from([
                "授權方式".into(),
                self.1.license.as_str().into(),
            ])),
            5 => Some(ModelRc::from([
                "製作軟體".into(),
                self.1.software.as_str().into(),
            ])),
            _ => None,
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &()
    }
}

struct DictEditViewModel {
    cache: RwLock<Vec<(Vec<Syllable>, Phrase)>>,
    dict: Rc<RwLock<Box<dyn Dictionary>>>,
    tracker: ModelNotify,
}

impl From<&DictTableItemModel> for DictEditViewModel {
    fn from(value: &DictTableItemModel) -> Self {
        let dict = value.1.clone();
        if let Ok(mut dict) = dict.write() {
            if let Some(dict_mut) = dict.as_dict_mut() {
                let _ = dict_mut.reopen();
            }
        }
        DictEditViewModel {
            cache: value
                .1
                .read()
                .expect("should not have concurrent writer")
                .entries()
                .collect::<Vec<_>>()
                .into(),
            dict,
            tracker: ModelNotify::default(),
        }
    }
}

impl Model for DictEditViewModel {
    type Data = ModelRc<StandardListViewItem>;

    fn row_count(&self) -> usize {
        self.cache.read().expect("no concurrent writer").len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.cache
            .read()
            .expect("no concurrent writer")
            .get(row)
            .map(|entry| {
                ModelRc::from([
                    entry.1.as_str().into(),
                    // FIXME
                    entry
                        .0
                        .iter()
                        .map(|syl| syl.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                        .as_str()
                        .into(),
                    entry.1.freq().to_string().as_str().into(),
                ])
            })
    }

    fn set_row_data(&self, row: usize, data: Self::Data) {
        let out_phrase = data.row_data(0).unwrap_or_default().text;
        let out_bopomofo = data.row_data(1).unwrap_or_default().text;
        let out_freq = data.row_data(2).unwrap_or_default().text;
        if let Ok(mut cache) = self.cache.write() {
            let freq: u32 = match out_freq.parse() {
                Ok(v) => v,
                Err(_) => return,
            };
            let phrase = Phrase::new(out_phrase.as_str(), freq);
            let syllables = out_bopomofo
                .replace("␣", " ")
                // number one vs. bopomofo I
                .replace("一", "ㄧ")
                .trim()
                .split_whitespace()
                .map(|cluster| Syllable::from_str(&cluster))
                .collect::<Vec<_>>();
            for err in syllables.iter() {
                if err.is_err() {
                    dbg!(err);
                }
            }
            if syllables.iter().any(|syl| syl.is_err()) {
                return;
            }
            let syllables = syllables
                .into_iter()
                .map(|syl| syl.unwrap())
                .collect::<Vec<_>>();
            cache[row] = (syllables, phrase);
        }
        self.tracker.row_changed(row);
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.tracker
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

fn filter_sort_model(
    source_model: ModelRc<ModelRc<StandardListViewItem>>,
    filter: SharedString,
    sort_index: i32,
    sort_ascending: bool,
) -> ModelRc<ModelRc<StandardListViewItem>> {
    let mut model = source_model.clone();

    if !filter.is_empty() {
        let filter = filter.to_lowercase();

        // filter by first row
        model = Rc::new(source_model.clone().filter(move |e| {
            e.row_data(0)
                .unwrap()
                .text
                .to_lowercase()
                .contains(filter.as_str())
        }))
        .into();
    }

    if sort_index >= 0 {
        model = Rc::new(model.clone().sort_by(move |r_a, r_b| {
            let c_a = r_a.row_data(sort_index as usize).unwrap();
            let c_b = r_b.row_data(sort_index as usize).unwrap();

            if sort_index == 2 {
                let a_num: u32 = c_a.text.parse().unwrap_or_default();
                let b_num: u32 = c_b.text.parse().unwrap_or_default();
                if sort_ascending {
                    a_num.cmp(&b_num)
                } else {
                    b_num.cmp(&a_num)
                }
            } else {
                if sort_ascending {
                    c_a.text.cmp(&c_b.text)
                } else {
                    c_b.text.cmp(&c_a.text)
                }
            }
        }))
        .into();
    }

    model
}

fn default_desktop_path() -> PathBuf {
    let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\unknown".into());
    PathBuf::from(user_profile).join("Desktop")
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
