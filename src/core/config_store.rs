use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use super::error::AppResult;
use super::input_validator::{
    normalize_credential, normalize_directory, normalize_integer, normalize_name,
    normalize_user_id, sanitize_file_name, MAX_CONCURRENT_DOWNLOADS, MAX_RETRY_COUNT,
    MAX_TIMEOUT_SECONDS, MIN_TIMEOUT_SECONDS,
};
use super::models::{AppConfiguration, AppSettings, SourceProfile};
use super::url_guard::validate_public_url;

struct State {
    configuration: AppConfiguration,
    last_error: Option<String>,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(State {
            configuration: AppConfiguration::default(),
            last_error: None,
        })
    })
}

pub fn data_directory() -> PathBuf {
    match std::env::var("MSD_DATA_DIR") {
        Ok(value) if !value.is_empty() => PathBuf::from(value),
        _ => {
            let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(base).join("MinecraftServerDownloader")
        }
    }
}

pub fn config_path() -> PathBuf {
    data_directory().join("config.json")
}

pub fn credential_path() -> PathBuf {
    data_directory().join("credentials.bin")
}

pub fn config_exists() -> bool {
    config_path().is_file()
}

pub fn last_error() -> Option<String> {
    state()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .last_error
        .clone()
}

pub fn load() -> AppConfiguration {
    let mut guard = state().lock().unwrap_or_else(|error| error.into_inner());
    guard.last_error = None;
    let path = config_path();
    if !path.is_file() {
        guard.configuration = normalize(AppConfiguration::default());
        return guard.configuration.clone();
    }
    match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<AppConfiguration>(&text) {
            Ok(loaded) => guard.configuration = normalize(loaded),
            Err(_) => {
                guard.configuration = normalize(AppConfiguration::default());
                guard.last_error = Some("config.ignored".to_string());
            }
        },
        Err(_) => {
            guard.configuration = normalize(AppConfiguration::default());
            guard.last_error = Some("config.ignored".to_string());
        }
    }
    guard.configuration.clone()
}

pub fn save(
    configuration: &AppConfiguration,
    credentials: &BTreeMap<String, String>,
) -> AppResult<()> {
    let normalized = normalize(configuration.clone());
    let mut clean: BTreeMap<String, String> = BTreeMap::new();
    for (key, value) in credentials {
        if value.trim().is_empty() {
            continue;
        }
        clean.insert(key.clone(), normalize_credential(value)?);
    }
    let directory = data_directory();
    fs::create_dir_all(&directory)?;
    write_json(&config_path(), &normalized)?;
    write_protected(&credential_path(), &clean)?;
    let mut guard = state().lock().unwrap_or_else(|error| error.into_inner());
    guard.configuration = normalized;
    guard.last_error = None;
    Ok(())
}

pub fn save_settings(settings: &AppSettings) -> AppResult<AppConfiguration> {
    let mut guard = state().lock().unwrap_or_else(|error| error.into_inner());
    guard.configuration.settings = normalize_settings(settings.clone());
    let directory = data_directory();
    fs::create_dir_all(&directory)?;
    let snapshot = guard.configuration.clone();
    write_json(&config_path(), &snapshot)?;
    Ok(snapshot)
}

pub fn settings() -> AppSettings {
    state()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .configuration
        .settings
        .clone()
}

pub fn sources() -> Vec<SourceProfile> {
    state()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .configuration
        .sources
        .clone()
}

pub fn credentials() -> BTreeMap<String, String> {
    let path = credential_path();
    if !path.is_file() {
        return BTreeMap::new();
    }
    match read_protected(&path) {
        Ok(map) => map,
        Err(_) => {
            let mut guard = state().lock().unwrap_or_else(|error| error.into_inner());
            guard.last_error = Some("credentials.ignored".to_string());
            BTreeMap::new()
        }
    }
}

pub fn reset_for_tests() {
    let mut guard = state().lock().unwrap_or_else(|error| error.into_inner());
    guard.configuration = AppConfiguration::default();
    guard.last_error = None;
    let directory = data_directory();
    if directory.is_dir() {
        let _ = fs::remove_dir_all(&directory);
    }
}

pub fn normalize(mut configuration: AppConfiguration) -> AppConfiguration {
    configuration.version = 1;
    configuration.settings = normalize_settings(configuration.settings);
    let mut seen: Vec<String> = Vec::new();
    let mut sources: Vec<SourceProfile> = Vec::new();
    for mut source in configuration.sources {
        if source.address.trim().is_empty() {
            continue;
        }
        source.id = sanitize_file_name(&source.id);
        if source.id.is_empty() {
            source.id = new_identifier();
        }
        if seen.iter().any(|value| value == &source.id) {
            continue;
        }
        seen.push(source.id.clone());
        source.name = match normalize_name(&source.name, &source.id.clone()) {
            Ok(value) => value,
            Err(_) => source.id.clone(),
        };
        let Ok(address) = validate_public_url(&source.address) else {
            continue;
        };
        source.address = address.text;
        source.kind = if source.kind.eq_ignore_ascii_case("bmclapi") {
            "bmclapi".to_string()
        } else {
            "manifest".to_string()
        };
        source.credential_type = if source.credential_type.eq_ignore_ascii_case("bearer") {
            "bearer".to_string()
        } else {
            "none".to_string()
        };
        source.user_id = normalize_user_id(source.user_id);
        sources.push(source);
    }
    configuration.sources = sources;
    if let Some(id) = configuration.settings.custom_source_id.clone() {
        if !configuration.sources.iter().any(|source| source.id == id) {
            configuration.settings.custom_source_id = None;
        }
    }
    configuration
}

pub fn normalize_settings(mut settings: AppSettings) -> AppSettings {
    settings.mirror = match settings.mirror.to_ascii_lowercase().as_str() {
        "bmclapi" => "bmclapi".to_string(),
        "custom" => "custom".to_string(),
        _ => "official".to_string(),
    };
    settings.max_concurrent =
        normalize_integer(settings.max_concurrent, 1, MAX_CONCURRENT_DOWNLOADS, 3);
    settings.request_timeout_seconds = normalize_integer(
        settings.request_timeout_seconds,
        MIN_TIMEOUT_SECONDS,
        MAX_TIMEOUT_SECONDS,
        30,
    );
    settings.retry_count = normalize_integer(settings.retry_count, 0, MAX_RETRY_COUNT, 2);
    let language = settings.language.to_ascii_lowercase();
    settings.language = if language.starts_with("zh") {
        "zh-cn".to_string()
    } else if language.starts_with("en") {
        "en-us".to_string()
    } else {
        "auto".to_string()
    };
    settings.theme = match settings.theme.to_ascii_lowercase().as_str() {
        "light" => "light".to_string(),
        "dark" => "dark".to_string(),
        _ => "system".to_string(),
    };
    settings.proxy = settings.proxy.trim().to_string();
    if !settings.proxy.is_empty() {
        if let Ok(address) = validate_public_url(&settings.proxy) {
            settings.proxy = address.text;
        } else {
            settings.proxy.clear();
        }
    }
    settings.download_directory = if settings.download_directory.trim().is_empty() {
        std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string())
    } else {
        match normalize_directory(&settings.download_directory) {
            Ok(value) => value,
            Err(_) => std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string()),
        }
    };
    settings
}

pub fn new_identifier() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    };
    format!("{nanos:x}{counter:x}")
}

fn write_json(path: &PathBuf, configuration: &AppConfiguration) -> AppResult<()> {
    let text = serde_json::to_string_pretty(configuration)?;
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn write_protected(path: &PathBuf, credentials: &BTreeMap<String, String>) -> AppResult<()> {
    let json = serde_json::to_string(credentials)?;
    let blob = protect(json.as_bytes());
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    fs::write(path, blob)?;
    Ok(())
}

fn read_protected(path: &PathBuf) -> AppResult<BTreeMap<String, String>> {
    let bytes = fs::read(path)?;
    let plain = unprotect(&bytes)
        .ok_or_else(|| super::error::AppError::general("credentials.undecryptable"))?;
    let map: BTreeMap<String, String> = serde_json::from_slice(&plain)?;
    Ok(map)
}

fn protect(data: &[u8]) -> Vec<u8> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 {
            return Vec::new();
        }
        let slice = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as *mut core::ffi::c_void);
        slice
    }
}

fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 {
            return None;
        }
        let slice = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as *mut core::ffi::c_void);
        Some(slice)
    }
}
