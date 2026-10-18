use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use regex::Regex;

const MAX_LOG_BYTES: u64 = 1_048_576;
const MAX_MESSAGE_CHARACTERS: usize = 12_000;

fn writer() -> &'static Mutex<Option<std::fs::File>> {
    static WRITER: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();
    WRITER.get_or_init(|| Mutex::new(None))
}

pub fn log_directory() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base)
        .join("MinecraftServerDownloader")
        .join("logs")
}

pub fn bind() {
    let mut guard = writer().lock().unwrap_or_else(|error| error.into_inner());
    if guard.is_some() {
        return;
    }
    let directory = log_directory();
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    rotate_if_needed(&directory);
    let path = directory.join("app.log");
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(path) {
        *guard = Some(file);
    }
}

pub fn info(message: &str) {
    write("INFO", message);
}

pub fn warn(message: &str) {
    write("WARN", message);
}

pub fn handle_unhandled(message: &str) {
    write("ERROR", message);
}

fn write(level: &str, value: &str) {
    let masked = mask(value);
    let mut guard = writer().lock().unwrap_or_else(|error| error.into_inner());
    if let Some(file) = guard.as_mut() {
        let timestamp = timestamp();
        let _ = writeln!(file, "{timestamp} [{level}] {masked}");
    }
}

fn timestamp() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => format_iso8601(duration.as_secs()),
        Err(_) => "1970-01-01T00:00:00Z".to_string(),
    }
}

fn format_iso8601(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let remainder = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.000Z",
        remainder / 3600,
        (remainder % 3600) / 60,
        remainder % 60
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn mask(value: &str) -> String {
    static HEADER: OnceLock<Regex> = OnceLock::new();
    static QUERY: OnceLock<Regex> = OnceLock::new();
    let header = HEADER.get_or_init(|| {
        Regex::new(r"(?i)(authorization|token|password|secret|credential)\s*[:=]\s*\S+")
            .expect("valid mask pattern")
    });
    let query = QUERY.get_or_init(|| {
        Regex::new(r"(?i)([?&](?:key|token|password|secret)=)[^&\s]+").expect("valid mask pattern")
    });
    let masked = header.replace_all(value, "$1=***").to_string();
    let masked = query.replace_all(&masked, "$1***").to_string();
    if masked.chars().count() > MAX_MESSAGE_CHARACTERS {
        let head: String = masked.chars().take(MAX_MESSAGE_CHARACTERS).collect();
        format!("{head}...")
    } else {
        masked
    }
}

fn rotate_if_needed(directory: &std::path::Path) {
    let current = directory.join("app.log");
    let Ok(metadata) = fs::metadata(&current) else {
        return;
    };
    if metadata.len() <= MAX_LOG_BYTES {
        return;
    }
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let target = directory.join(format!("app.log.{suffix}"));
    let _ = fs::rename(&current, target);
}
