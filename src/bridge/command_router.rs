use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::core::config_store;
use crate::core::download_engine::{source_profile_credentials, DownloadEngine, ProgressSink};
use crate::core::error::{AppError, AppResult};
use crate::core::input_validator::{
    normalize_directory, normalize_integer, normalize_name, MAX_CONCURRENT_DOWNLOADS,
    MAX_RETRY_COUNT, MAX_TIMEOUT_SECONDS, MIN_TIMEOUT_SECONDS,
};
use crate::core::manifest_cache::ManifestCache;
use crate::core::models::{
    AppConfiguration, AppSettings, DownloadProgress, DownloadSummary, SourceProfile, VersionInfo,
};
use crate::core::net::{RequestProfile, SafeHttp};
use crate::core::url_guard::validate_public_url;
use crate::core::version_catalog::VersionCatalog;

use super::external_launcher;
use super::json as bridge_json;
use super::operation_gate::OperationGate;
use super::state_view;

const CACHE_MINUTES: u64 = 30;

pub trait EventSink: Send + Sync {
    fn post(&self, payload: Value);
}

pub struct CommandRouter {
    events: Arc<dyn EventSink>,
    operations: OperationGate,
    versions: Mutex<Vec<VersionInfo>>,
    engine: Mutex<Option<Arc<DownloadEngine>>>,
}

impl CommandRouter {
    pub fn new(events: Arc<dyn EventSink>) -> Self {
        Self {
            events,
            operations: OperationGate::new(),
            versions: Mutex::new(Vec::new()),
            engine: Mutex::new(None),
        }
    }

    pub fn execute(&self, action: &str, payload: &Value) -> AppResult<Value> {
        match action {
            "getState" => Ok(self.build_state()),
            "refreshVersions" => self.refresh_versions(payload),
            "saveSettings" => self.save_settings(payload),
            "selectDirectory" => Ok(json!(select_directory())),
            "importSource" => self.import_source(payload),
            "removeSource" => self.remove_source(payload),
            "selectSource" => self.select_source(payload),
            "startDownload" => self.start_download(payload),
            "pauseDownload" => self.set_paused(true),
            "resumeDownload" => self.set_paused(false),
            "stopDownload" => self.stop_download(),
            "cancel" => self.cancel(),
            "openDataDirectory" => self.open_data_directory(),
            _ => Err(AppError::validation("bridge.unknownAction")),
        }
    }

    pub fn shutdown(&self) {
        if let Some(engine) = self
            .engine
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
        {
            engine.stop();
        }
        self.operations.cancel_current();
    }

    fn build_state(&self) -> Value {
        let versions = self
            .versions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state_view::build(
            &config_store::settings(),
            &config_store::sources(),
            &versions,
            &config_store::credentials(),
        )
    }

    fn refresh_versions(&self, payload: &Value) -> AppResult<Value> {
        let settings = self.read_settings(payload)?;
        let sources = config_store::sources();
        let mut lease = self.operations.create()?;
        self.post(json!({ "type": "status", "key": "status.refreshing" }));
        let result = (|| -> AppResult<Vec<VersionInfo>> {
            let http = create_http(&settings, &sources)?;
            let cache = ManifestCache::new(
                config_store::data_directory().join("manifest-cache.json"),
                Some(Duration::from_secs(CACHE_MINUTES * 60)),
            );
            let catalog = VersionCatalog::new(&http, &cache, &settings.download_directory)?;
            catalog.load(&settings, &sources, &lease.token())
        })();
        lease.release();
        self.post(json!({ "type": "status", "key": "status.ready" }));
        let versions = result?;
        *self
            .versions
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = versions;
        Ok(self.build_state())
    }

    fn save_settings(&self, payload: &Value) -> AppResult<Value> {
        let settings = self.read_settings(payload)?;
        config_store::save_settings(&settings)?;
        Ok(self.build_state())
    }

    fn import_source(&self, payload: &Value) -> AppResult<Value> {
        let address = validate_public_url(&bridge_json::text(payload, "address", ""))?;
        let host = address.host.clone();
        let source = SourceProfile {
            id: config_store::new_identifier(),
            name: normalize_name(&bridge_json::text(payload, "name", ""), &host)?,
            address: address.text,
            kind: "manifest".to_string(),
            credential_type: "none".to_string(),
            user_id: None,
        };
        let mut sources = config_store::sources();
        if sources
            .iter()
            .any(|item| item.address.eq_ignore_ascii_case(&source.address))
        {
            return Err(AppError::validation("source.duplicate"));
        }
        sources.push(source);
        let configuration = AppConfiguration {
            version: 1,
            settings: config_store::settings(),
            sources,
        };
        config_store::save(&configuration, &config_store::credentials())?;
        Ok(self.build_state())
    }

    fn remove_source(&self, payload: &Value) -> AppResult<Value> {
        let id = bridge_json::text(payload, "sourceId", "");
        let mut sources = config_store::sources();
        sources.retain(|source| source.id != id);
        let mut credentials = config_store::credentials();
        credentials.remove(&id);
        let mut settings = config_store::settings();
        if settings.custom_source_id.as_deref() == Some(id.as_str()) {
            settings.custom_source_id = None;
            settings.mirror = "official".to_string();
        }
        let configuration = AppConfiguration {
            version: 1,
            settings,
            sources,
        };
        config_store::save(&configuration, &credentials)?;
        Ok(self.build_state())
    }

    fn select_source(&self, payload: &Value) -> AppResult<Value> {
        let id = bridge_json::text(payload, "sourceId", "");
        let mut settings = config_store::settings();
        let exists = config_store::sources().iter().any(|source| source.id == id);
        if exists {
            settings.mirror = "custom".to_string();
            settings.custom_source_id = Some(id);
        } else {
            settings.mirror = "official".to_string();
            settings.custom_source_id = None;
        }
        let versions = self
            .versions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        Ok(state_view::build(
            &settings,
            &config_store::sources(),
            &versions,
            &config_store::credentials(),
        ))
    }

    fn start_download(&self, payload: &Value) -> AppResult<Value> {
        let settings = self.read_settings(payload)?;
        let selected: Vec<String> = payload
            .get("selectedIds")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string())
                    .collect()
            })
            .unwrap_or_default();
        let versions: Vec<VersionInfo> = self
            .versions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .filter(|item| selected.iter().any(|id| id == &item.id))
            .cloned()
            .collect();
        if versions.is_empty() {
            return Err(AppError::validation("download.noSelection"));
        }
        normalize_directory(&settings.download_directory)?;
        let sources = config_store::sources();
        let mut lease = self.operations.create()?;
        let http = create_http(&settings, &sources)?;
        let cache = ManifestCache::new(
            config_store::data_directory().join("manifest-cache.json"),
            Some(Duration::from_secs(CACHE_MINUTES * 60)),
        );
        let catalog = VersionCatalog::new(&http, &cache, &settings.download_directory)?;
        let engine = Arc::new(DownloadEngine::new());
        *self
            .engine
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(engine.clone());
        let sink = BridgeSink {
            events: self.events.clone(),
        };
        let summary = engine.run(&http, &catalog, &versions, &settings, &sink);
        *self
            .engine
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        lease.release();
        {
            let mut guard = self
                .versions
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for item in guard.iter_mut() {
                if versions.iter().any(|version| version.id == item.id) {
                    item.is_downloaded =
                        summary.outcome == crate::core::models::DownloadOutcome::Completed;
                }
            }
        }
        Ok(summary_payload(summary))
    }

    fn set_paused(&self, paused: bool) -> AppResult<Value> {
        let engine = self
            .engine
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .ok_or_else(|| AppError::validation("download.notRunning"))?;
        if paused {
            engine.pause();
        } else {
            engine.resume();
        }
        Ok(json!(true))
    }

    fn stop_download(&self) -> AppResult<Value> {
        let engine = self
            .engine
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .ok_or_else(|| AppError::validation("download.notRunning"))?;
        engine.stop();
        Ok(json!(true))
    }

    fn cancel(&self) -> AppResult<Value> {
        if let Some(engine) = self
            .engine
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
        {
            engine.stop();
        }
        self.operations.cancel_current();
        Ok(json!(true))
    }

    fn open_data_directory(&self) -> AppResult<Value> {
        external_launcher::open_data_directory()?;
        Ok(json!(true))
    }

    fn read_settings(&self, payload: &Value) -> AppResult<AppSettings> {
        let node = match bridge_json::object(payload, "settings") {
            Value::Object(_) => bridge_json::object(payload, "settings"),
            _ => payload.clone(),
        };
        let current = config_store::settings();
        let custom_source_id = {
            let raw = bridge_json::text(&node, "customSourceId", "");
            if raw.is_empty() {
                None
            } else {
                Some(raw)
            }
        };
        Ok(AppSettings {
            download_directory: bridge_json::text(
                &node,
                "downloadDirectory",
                &current.download_directory,
            ),
            mirror: bridge_json::text(&node, "mirror", &current.mirror),
            custom_source_id,
            proxy: bridge_json::text(&node, "proxy", &current.proxy),
            max_concurrent: normalize_integer(
                bridge_json::integer(&node, "maxConcurrent", current.max_concurrent),
                1,
                MAX_CONCURRENT_DOWNLOADS,
                3,
            ),
            request_timeout_seconds: normalize_integer(
                bridge_json::integer(
                    &node,
                    "requestTimeoutSeconds",
                    current.request_timeout_seconds,
                ),
                MIN_TIMEOUT_SECONDS,
                MAX_TIMEOUT_SECONDS,
                30,
            ),
            retry_count: normalize_integer(
                bridge_json::integer(&node, "retryCount", current.retry_count),
                0,
                MAX_RETRY_COUNT,
                2,
            ),
            language: bridge_json::text(&node, "language", &current.language),
            theme: bridge_json::text(&node, "theme", &current.theme),
        })
    }

    fn post(&self, payload: Value) {
        self.events.post(payload);
    }
}

struct BridgeSink {
    events: Arc<dyn EventSink>,
}

impl ProgressSink for BridgeSink {
    fn report(&self, progress: DownloadProgress) {
        self.events
            .post(json!({ "type": "progress", "value": progress }));
    }

    fn log(&self, key: &str, is_error: bool) {
        self.events
            .post(json!({ "type": "log", "key": key, "error": is_error }));
    }
}

fn summary_payload(summary: DownloadSummary) -> Value {
    json!({
        "outcome": match summary.outcome {
            crate::core::models::DownloadOutcome::Completed => "Completed",
            crate::core::models::DownloadOutcome::Stopped => "Stopped",
            crate::core::models::DownloadOutcome::DiskFull => "DiskFull",
            crate::core::models::DownloadOutcome::Failed => "Failed",
        },
        "completed": summary.completed,
        "failed": summary.failed,
        "totalBytes": summary.total_bytes,
    })
}

fn create_http(settings: &AppSettings, sources: &[SourceProfile]) -> AppResult<SafeHttp> {
    let credentials: BTreeMap<String, String> = config_store::credentials();
    let (credential_type, credential, user_id) =
        source_profile_credentials(settings, sources, &credentials);
    let profile = RequestProfile::from_settings(settings, &credential_type, &credential, user_id)?;
    SafeHttp::new(profile)
}

fn select_directory() -> String {
    let mut dialog = rfd::FileDialog::new().set_title("选择下载目录");
    if let Ok(home) = std::env::var("USERPROFILE") {
        dialog = dialog.set_directory(PathBuf::from(home));
    }
    dialog
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default()
}
