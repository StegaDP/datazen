use std::collections::HashMap;

use backend::UiLanguage;
use once_cell::sync::Lazy;
use serde_yaml::Value;

#[derive(Clone, Copy)]
pub struct I18n {
    language: UiLanguage,
}

impl I18n {
    pub fn new(language: UiLanguage) -> Self {
        Self { language }
    }

    pub fn language(self) -> UiLanguage {
        self.language
    }

    pub fn set_language(&mut self, language: UiLanguage) {
        self.language = language;
    }

    pub fn tr(self, key: &'static str) -> &'static str {
        bundle(self.language)
            .messages
            .get(key)
            .copied()
            .or_else(|| ENGLISH.messages.get(key).copied())
            .unwrap_or("")
    }

    pub fn language_label(self, language: UiLanguage) -> &'static str {
        bundle(language).autonym
    }
}

pub const ALL_LANGUAGES: [UiLanguage; 11] = [
    UiLanguage::English,
    UiLanguage::Spanish,
    UiLanguage::Portuguese,
    UiLanguage::French,
    UiLanguage::German,
    UiLanguage::Russian,
    UiLanguage::Ukrainian,
    UiLanguage::Mandarin,
    UiLanguage::Hindi,
    UiLanguage::Japanese,
    UiLanguage::Korean,
];

struct LocaleBundle {
    autonym: &'static str,
    messages: HashMap<&'static str, &'static str>,
}

static ENGLISH: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/en.yaml")));
static SPANISH: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/es.yaml")));
static PORTUGUESE: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/pt-PT.yaml")));
static FRENCH: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/fr.yaml")));
static GERMAN: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/de.yaml")));
static RUSSIAN: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/ru.yaml")));
static UKRAINIAN: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/uk.yaml")));
static MANDARIN: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/zh-Hans.yaml")));
static HINDI: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/hi.yaml")));
static JAPANESE: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/ja.yaml")));
static KOREAN: Lazy<LocaleBundle> =
    Lazy::new(|| load_locale(include_str!("../locales/ko.yaml")));

fn bundle(language: UiLanguage) -> &'static LocaleBundle {
    match language {
        UiLanguage::English => &ENGLISH,
        UiLanguage::Spanish => &SPANISH,
        UiLanguage::Portuguese => &PORTUGUESE,
        UiLanguage::French => &FRENCH,
        UiLanguage::German => &GERMAN,
        UiLanguage::Russian => &RUSSIAN,
        UiLanguage::Ukrainian => &UKRAINIAN,
        UiLanguage::Mandarin => &MANDARIN,
        UiLanguage::Hindi => &HINDI,
        UiLanguage::Japanese => &JAPANESE,
        UiLanguage::Korean => &KOREAN,
    }
}

fn load_locale(source: &'static str) -> LocaleBundle {
    let value: Value = serde_yaml::from_str(source).expect("locale yaml must be valid");
    let mut flat = HashMap::new();
    flatten("", &value, &mut flat);

    let autonym = leak(
        flat.remove("meta.autonym")
            .unwrap_or_else(|| "English".to_string()),
    );

    let messages = flat
        .into_iter()
        .map(|(key, value)| (leak(key), leak(value)))
        .collect();

    LocaleBundle { autonym, messages }
}

fn flatten(prefix: &str, value: &Value, out: &mut HashMap<String, String>) {
    match value {
        Value::Mapping(mapping) => {
            for (key, nested) in mapping {
                let Value::String(key) = key else {
                    continue;
                };
                let next = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&next, nested, out);
            }
        }
        Value::String(text) => {
            out.insert(prefix.to_string(), text.clone());
        }
        Value::Bool(flag) => {
            out.insert(prefix.to_string(), flag.to_string());
        }
        Value::Number(number) => {
            out.insert(prefix.to_string(), number.to_string());
        }
        _ => {}
    }
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}
