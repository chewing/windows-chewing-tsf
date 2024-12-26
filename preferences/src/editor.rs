use std::rc::Rc;
use std::sync::RwLock;

use anyhow::Result;
use chewing::dictionary::{Dictionary, SystemDictionaryLoader, UserDictionaryLoader};
use chewing::dictionary::{DictionaryInfo, Phrase};
use chewing::zhuyin::Syllable;
use slint::{ComponentHandle, Model, ModelRc, ModelTracker, StandardListViewItem, VecModel};

use crate::DictionaryInfoDialog;
use crate::EditorWindow;
use crate::EditEntryDialog;

pub fn run() -> Result<()> {
    let ui = EditorWindow::new()?;

    ui.set_dictionaries(dict_list_model()?);

    ui.on_info_clicked(move |row: ModelRc<StandardListViewItem>| {
        let info_dialog = DictionaryInfoDialog::new().unwrap();
        let dict_item = row
            .as_any()
            .downcast_ref::<DictTableItemModel>()
            .expect("row item should be a DictTableItemModel");
        let dict_model = ModelRc::new(DictInfoViewModel::from(dict_item));

        let dialog_handle = info_dialog.as_weak();
        info_dialog.set_dictionary_info(dict_model);
        info_dialog.on_ok_clicked(move || {
            let dialog = dialog_handle.upgrade().unwrap();
            dialog.window().hide().unwrap();
        });
        info_dialog.show().unwrap();
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

    ui.on_edit_entry_clicked(|phrase, bopomofo, freq| {
        let edit_dialog = EditEntryDialog::new().unwrap();
        edit_dialog.set_phrase(phrase);
        edit_dialog.set_bopomofo(bopomofo);
        edit_dialog.set_freq(freq);
        edit_dialog.show().unwrap();
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
                    .map(|dict| ModelRc::new(DictTableItemModel::new("附加", dict))),
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
    cache: Vec<(Vec<Syllable>, Phrase)>,
    dict: Rc<RwLock<Box<dyn Dictionary>>>,
}

impl From<&DictTableItemModel> for DictEditViewModel {
    fn from(value: &DictTableItemModel) -> Self {
        DictEditViewModel {
            cache: value
                .1
                .read()
                .expect("should not have concurrent writer")
                .entries()
                .collect(),
            dict: value.1.clone(),
        }
    }
}

impl Model for DictEditViewModel {
    type Data = ModelRc<StandardListViewItem>;

    fn row_count(&self) -> usize {
        self.cache.len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        self.cache.get(row).map(|entry| {
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
        // FIXME
        &()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
