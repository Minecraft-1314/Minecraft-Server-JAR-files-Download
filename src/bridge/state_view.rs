use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::core::config_store;
use crate::core::models::{AppSettings, SourceProfile, VersionInfo};

pub fn build(
    settings: &AppSettings,
    sources: &[SourceProfile],
    versions: &[VersionInfo],
    credentials: &BTreeMap<String, String>,
) -> Value {
    let sources: Vec<Value> = sources
        .iter()
        .map(|source| {
            json!({
                "id": source.id,
                "name": source.name,
                "address": source.address,
                "type": source.kind,
                "credentialType": source.credential_type,
                "userId": source.user_id,
                "hasCredential": credentials.contains_key(&source.id),
            })
        })
        .collect();
    json!({
        "settings": settings,
        "sources": sources,
        "versions": versions,
        "configExists": config_store::config_exists(),
        "lastError": config_store::last_error(),
        "dataDirectory": config_store::data_directory().to_string_lossy(),
        "settingsPath": config_store::data_directory().join("config.json").to_string_lossy(),
        "version": env!("CARGO_PKG_VERSION"),
        "runtimeVersion": env!("APP_RUST_VERSION"),
        "installPath": install_path(),
    })
}

fn install_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_default()
}
