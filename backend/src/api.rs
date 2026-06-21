use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiLanguage {
    English,
    Spanish,
    Portuguese,
    French,
    German,
    Russian,
    Ukrainian,
    Mandarin,
    Hindi,
    Japanese,
    Korean,
}

impl UiLanguage {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
            Self::Portuguese => "pt-PT",
            Self::French => "fr",
            Self::German => "de",
            Self::Russian => "ru",
            Self::Ukrainian => "uk",
            Self::Mandarin => "zh-Hans",
            Self::Hindi => "hi",
            Self::Japanese => "ja",
            Self::Korean => "ko",
        }
    }

    pub fn from_code(value: &str) -> Self {
        match value {
            "es" => Self::Spanish,
            "pt" | "pt-PT" => Self::Portuguese,
            "fr" => Self::French,
            "de" => Self::German,
            "ru" => Self::Russian,
            "uk" => Self::Ukrainian,
            "zh" | "zh-Hans" => Self::Mandarin,
            "hi" => Self::Hindi,
            "ja" => Self::Japanese,
            "ko" => Self::Korean,
            _ => Self::English,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    Graphite,
    System,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Graphite => "graphite",
            Self::System => "system",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "system" => Self::System,
            _ => Self::Graphite,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseKind {
    PostgreSql,
    MySql,
    MariaDb,
    Sqlite,
    Redis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsSnapshot {
    pub language: UiLanguage,
    pub theme: ThemeMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDraft {
    pub id: Option<String>,
    pub name: String,
    pub database_kind: DatabaseKind,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub group: Option<String>,
    pub color_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSummary {
    pub id: String,
    pub name: String,
    pub database_kind: DatabaseKind,
    pub host: Option<String>,
    pub database: Option<String>,
    pub group: Option<String>,
    pub color_tag: Option<String>,
    pub last_connected_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapData {
    pub settings: AppSettingsSnapshot,
    pub connections: Vec<ConnectionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSession {
    pub session_id: String,
    pub server_type: String,
    pub server_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSummary {
    pub name: String,
    pub table_type: String,
    pub row_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuerySnapshot {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub rows_affected: Option<u64>,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: Option<i64>,
    pub page: u32,
    pub page_size: u32,
}
