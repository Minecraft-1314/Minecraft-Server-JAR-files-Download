use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub download_directory: String,
    pub mirror: String,
    pub custom_source_id: Option<String>,
    pub proxy: String,
    pub max_concurrent: i32,
    pub request_timeout_seconds: i32,
    pub retry_count: i32,
    pub language: String,
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            download_directory: String::new(),
            mirror: "official".to_string(),
            custom_source_id: None,
            proxy: String::new(),
            max_concurrent: 3,
            request_timeout_seconds: 30,
            retry_count: 2,
            language: "auto".to_string(),
            theme: "system".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceProfile {
    pub id: String,
    pub name: String,
    pub address: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub credential_type: String,
    pub user_id: Option<i64>,
}

impl Default for SourceProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            address: String::new(),
            kind: "manifest".to_string(),
            credential_type: "none".to_string(),
            user_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfiguration {
    pub version: i32,
    pub settings: AppSettings,
    pub sources: Vec<SourceProfile>,
}

impl Default for AppConfiguration {
    fn default() -> Self {
        Self {
            version: 1,
            settings: AppSettings::default(),
            sources: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub id: String,
    pub url: String,
    pub jar_name: String,
    pub folder: String,
    pub release_time: String,
    pub is_downloaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionJob {
    pub version: VersionInfo,
    pub server_url: String,
    pub sha1: String,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub completed: i32,
    pub total: i32,
    pub done_bytes: i64,
    pub total_bytes: i64,
    pub bytes_per_second: f64,
    pub remaining_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DownloadOutcome {
    Completed,
    Stopped,
    DiskFull,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadSummary {
    pub outcome: DownloadOutcome,
    pub completed: i32,
    pub failed: i32,
    pub total_bytes: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DownloadTiming {
    pub bytes_per_second: f64,
    pub remaining_seconds: f64,
}
