use std::iter;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::RwLock;

use anyhow::{anyhow, Result};
use chewing::dictionary::{
    Dictionary, DictionaryBuilder, SystemDictionaryLoader, TrieBuilder, UserDictionaryLoader,
};
use chewing::dictionary::{DictionaryInfo, Phrase};
use chewing::zhuyin::Syllable;
use slint::{
    ComponentHandle, Model, ModelNotify, ModelRc, ModelTracker, StandardListViewItem, VecModel,
};

use crate::CallbackResult;
use crate::EditorWindow;
use crate::ErrorKind;

pub fn run() -> Result<()> {
    let ui = EditorWindow::new()?;

    ui.set_dictionaries(dict_list_model()?);

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
        ui.set_entries(dict_model);
    });

    let ui_handle = ui.as_weak();
    ui.on_edit_entry_done(move || -> CallbackResult {
        let ui = ui_handle.upgrade().unwrap();
        let out_phrase = ui.get_phrase();
        let out_bopomofo = ui.get_bopomofo();
        let out_freq = ui.get_freq();
        let index = ui.get_edit_dict_current_row() as usize;
        let entries_rc = ui.get_entries();
        let entry = entries_rc
            .as_any()
            .downcast_ref::<DictEditViewModel>()
            .expect("entries should be a DictEditViewModel");
        if let Ok(mut cache) = entry.cache.write() {
            let freq: u32 = match out_freq.parse() {
                Ok(v) => v,
                Err(_) => {
                    return CallbackResult {
                        error: ErrorKind::Other,
                        err_msg: slint::format!("無法辨認 {out_freq} 為數字"),
                    }
                }
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
                let ellipsis = if out_bopomofo.len() > 20 { 3 } else { 0 };
                let sample = out_bopomofo
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
            let syllables = syllables
                .into_iter()
                .map(|syl| syl.unwrap())
                .collect::<Vec<_>>();
            cache[index] = (syllables, phrase);
        }
        entry.tracker.row_changed(index);
        CallbackResult {
            error: ErrorKind::Ok,
            ..Default::default()
        }
    });

    let ui_handle = ui.as_weak();
    ui.on_edit_entry_new(move || {
        let ui = ui_handle.upgrade().unwrap();
        let entries_rc = ui.get_entries();
        let entry = entries_rc
            .as_any()
            .downcast_ref::<DictEditViewModel>()
            .expect("entries should be a DictEditViewModel");
        let _ = entry
            .cache
            .write()
            .map(|mut cache| {
                let phrase = Phrase::new("", 0);
                cache.push((vec![], phrase));
                cache.len() - 1
            })
            .map(|index| {
                entry.tracker.row_added(index, 1);
            });
    });

    let ui_handle = ui.as_weak();
    ui.on_edit_entry_delete(move || {
        let ui = ui_handle.upgrade().unwrap();
        let index = ui.get_edit_dict_current_row() as usize;
        let entries_rc = ui.get_entries();
        let entry = entries_rc
            .as_any()
            .downcast_ref::<DictEditViewModel>()
            .expect("entries should be a DictEditViewModel");
        let _ = entry
            .cache
            .write()
            .map(|mut cache| {
                cache.remove(index);
            })
            .map(|_| {
                entry.tracker.row_removed(index, 1);
            });
    });

    let ui_handle = ui.as_weak();
    ui.on_edit_dict_save(move || -> CallbackResult {
        let ui = ui_handle.upgrade().unwrap();
        let entries_rc = ui.get_entries();
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
    let sys_loader = SystemDictionaryLoader::new();
    let user_loader = UserDictionaryLoader::new();

    Ok(ModelRc::new(VecModel::from_iter(
        sys_loader
            .load()?
            .into_iter()
            .map(|dict| ModelRc::new(DictTableItemModel::new("系統", dict)))
            .chain(
                sys_loader
                    .load_extra()?
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
        2
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        match row {
            0 => Some(self.0.into()),
            1 => self
                .1
                .read()
                .ok()
                .map(|dict| dict.about().name.as_str().into()),
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
        DictEditViewModel {
            cache: value
                .1
                .read()
                .expect("should not have concurrent writer")
                .entries()
                .collect::<Vec<_>>()
                .into(),
            dict: value.1.clone(),
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

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.tracker
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
