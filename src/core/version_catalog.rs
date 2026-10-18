use std::path::Path;

use serde_json::Value;

use super::error::{AppError, AppResult};
use super::input_validator::{normalize_directory, normalize_version_id, sanitize_file_name};
use super::manifest_cache::ManifestCache;
use super::models::{AppSettings, SourceProfile, VersionInfo, VersionJob};
use super::net::SafeHttp;
use super::url_guard::{is_public_url, validate_public_url};

const OFFICIAL_MANIFESTS: [&str; 2] = [
    "https://launchermeta.mojang.com/mc/game/version_manifest.json",
    "https://piston-meta.mojang.com/mc/game/version_manifest.json",
];
const BMCL_MANIFEST: &str = "https://bmclapi2.bangbang93.com/mc/game/version_manifest.json";
const BMCL_METADATA: &str = "https://bmclapi2.bangbang93.com/version/{id}/json";
const BMCL_SERVER: &str = "https://bmclapi2.bangbang93.com/version/{id}/server";

pub struct VersionCatalog<'a> {
    http: &'a SafeHttp,
    cache: &'a ManifestCache,
    download_directory: String,
}

impl<'a> VersionCatalog<'a> {
    pub fn new(
        http: &'a SafeHttp,
        cache: &'a ManifestCache,
        download_directory: &str,
    ) -> AppResult<Self> {
        Ok(Self {
            http,
            cache,
            download_directory: normalize_directory(download_directory)?,
        })
    }

    pub fn load(
        &self,
        settings: &AppSettings,
        sources: &[SourceProfile],
        cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> AppResult<Vec<VersionInfo>> {
        let urls = manifest_urls(settings, sources)?;
        let source_key = source_key(settings, &urls);
        let manifest = match self.cache.read(&source_key) {
            Some(cached) => cached,
            None => {
                let fetched = self.fetch_manifest(&urls, cancel)?;
                self.cache.write(&source_key, &fetched);
                fetched
            }
        };
        parse_manifest(&manifest, &self.download_directory)
    }

    pub fn resolve(
        &self,
        settings: &AppSettings,
        version: &VersionInfo,
        cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> AppResult<Option<VersionJob>> {
        let metadata_url = if settings.mirror == "bmclapi" {
            BMCL_METADATA.replace("{id}", &version.id)
        } else {
            version.url.clone()
        };
        let metadata = self.http.get_json(&metadata_url, cancel)?;
        let Some(server) = metadata
            .get("downloads")
            .and_then(|value| value.get("server"))
        else {
            return Ok(None);
        };
        let server_url = if settings.mirror == "bmclapi" {
            BMCL_SERVER.replace("{id}", &version.id)
        } else {
            server
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        if server_url.is_empty() {
            return Ok(None);
        }
        let validated = validate_public_url(&server_url)?;
        let size = server.get("size").and_then(Value::as_i64).unwrap_or(0);
        let sha1 = server
            .get("sha1")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        Ok(Some(VersionJob {
            version: version.clone(),
            server_url: validated.text,
            sha1,
            size,
        }))
    }

    fn fetch_manifest(
        &self,
        urls: &[String],
        cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> AppResult<Value> {
        let mut last: Option<AppError> = None;
        for url in urls {
            match self.http.get_json(url, cancel) {
                Ok(manifest) => return Ok(manifest),
                Err(error) => {
                    if error.message() == "operation.cancelled" {
                        return Err(error);
                    }
                    last = Some(error);
                }
            }
        }
        Err(match last {
            Some(error) => AppError::general(format!("manifest.failed: {}", error.message())),
            None => AppError::general("manifest.failed"),
        })
    }
}

pub fn parse_manifest(manifest: &Value, download_directory: &str) -> AppResult<Vec<VersionInfo>> {
    let Some(versions) = manifest.get("versions").and_then(Value::as_array) else {
        return Err(AppError::validation("manifest.invalid"));
    };
    let root = normalize_directory(download_directory)?;
    let mut result: Vec<VersionInfo> = Vec::new();
    for element in versions {
        let Some(kind) = element.get("type").and_then(Value::as_str) else {
            continue;
        };
        if !kind.eq_ignore_ascii_case("release") {
            continue;
        }
        let Some(raw_id) = element.get("id").and_then(Value::as_str) else {
            continue;
        };
        let url = element
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !is_public_url(url) {
            continue;
        }
        let Ok(id) = normalize_version_id(raw_id) else {
            continue;
        };
        let release_time = element
            .get("releaseTime")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let safe = sanitize_file_name(&id);
        let folder = folder_name(&id);
        let jar_name = format!("minecraft_server_{safe}.jar");
        let is_downloaded = Path::new(&root).join(&folder).join(&jar_name).is_file();
        result.push(VersionInfo {
            id,
            url: url.to_string(),
            jar_name,
            folder,
            release_time,
            is_downloaded,
        });
    }
    result.sort_by(|left, right| {
        right.release_time.cmp(&left.release_time).then_with(|| {
            left.id
                .to_ascii_lowercase()
                .cmp(&right.id.to_ascii_lowercase())
        })
    });
    Ok(result)
}

pub fn folder_name(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        format!("Minecraft {}.{} Server JAR files", parts[0], parts[1])
    } else {
        format!("Minecraft {version} Server JAR files")
    }
}

fn manifest_urls(settings: &AppSettings, sources: &[SourceProfile]) -> AppResult<Vec<String>> {
    if settings.mirror == "bmclapi" {
        return Ok(vec![BMCL_MANIFEST.to_string()]);
    }
    if settings.mirror == "custom" {
        let source = settings
            .custom_source_id
            .as_ref()
            .and_then(|id| sources.iter().find(|source| &source.id == id))
            .ok_or_else(|| AppError::validation("source.missing"))?;
        return Ok(vec![validate_public_url(&source.address)?.text]);
    }
    Ok(OFFICIAL_MANIFESTS
        .iter()
        .map(|url| url.to_string())
        .collect())
}

fn source_key(settings: &AppSettings, urls: &[String]) -> String {
    if settings.mirror == "custom" {
        format!("custom:{}", urls.join("|"))
    } else {
        settings.mirror.clone()
    }
}
