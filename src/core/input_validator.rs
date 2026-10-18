use std::path::Path;

use regex::Regex;
use std::sync::OnceLock;

use super::error::{AppError, AppResult};

pub const MAX_NAME_LENGTH: usize = 80;
pub const MAX_ADDRESS_LENGTH: usize = 2048;
pub const MAX_CREDENTIAL_LENGTH: usize = 4096;
pub const MAX_CONCURRENT_DOWNLOADS: i32 = 8;
pub const MIN_TIMEOUT_SECONDS: i32 = 5;
pub const MAX_TIMEOUT_SECONDS: i32 = 300;
pub const MAX_RETRY_COUNT: i32 = 5;

pub fn clean(value: &str) -> String {
    value.trim().to_string()
}

fn version_id_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[A-Za-z0-9._-]{1,64}$").expect("valid version id pattern"))
}

pub fn normalize_name(value: &str, fallback: &str) -> AppResult<String> {
    let mut text = clean(value);
    if text.is_empty() {
        text = clean(fallback);
    }
    if text.is_empty() {
        return Err(AppError::validation("name.required"));
    }
    if text.chars().count() > MAX_NAME_LENGTH {
        return Err(AppError::validation("name.tooLong"));
    }
    Ok(text)
}

pub fn normalize_version_id(value: &str) -> AppResult<String> {
    let text = clean(value);
    if !version_id_pattern().is_match(&text) {
        return Err(AppError::validation("version.invalid"));
    }
    Ok(text)
}

pub fn normalize_integer(value: i32, minimum: i32, maximum: i32, fallback: i32) -> i32 {
    if value < minimum || value > maximum {
        fallback
    } else {
        value
    }
}

pub fn normalize_user_id(value: Option<i64>) -> Option<i64> {
    match value {
        Some(number) if (1..=999_999_999_999).contains(&number) => Some(number),
        _ => None,
    }
}

pub fn normalize_user_id_text(value: &str) -> Option<i64> {
    let text = clean(value);
    if text.is_empty() {
        return None;
    }
    normalize_user_id(text.parse::<i64>().ok())
}

pub fn normalize_credential(value: &str) -> AppResult<String> {
    let text = clean(value);
    if text.chars().count() > MAX_CREDENTIAL_LENGTH {
        return Err(AppError::validation("credential.tooLong"));
    }
    Ok(text)
}

pub fn normalize_directory(value: &str) -> AppResult<String> {
    let text = clean(value);
    if text.is_empty() {
        return Err(AppError::validation("directory.required"));
    }
    let path = Path::new(&text);
    let full = std::path::absolute(path).map_err(|_| AppError::validation("directory.invalid"))?;
    if has_invalid_path_characters(&full) {
        return Err(AppError::validation("directory.invalid"));
    }
    Ok(trim_trailing_separator(&full))
}

pub fn sanitize_file_name(value: &str) -> String {
    let cleaned = clean(value);
    cleaned
        .chars()
        .filter(|character| !is_invalid_file_name_character(*character) && *character != ' ')
        .collect()
}

fn is_invalid_file_name_character(character: char) -> bool {
    matches!(
        character,
        '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
    ) || (character as u32) < 32
}

fn has_invalid_path_characters(path: &Path) -> bool {
    path.to_string_lossy()
        .chars()
        .any(|character| matches!(character, '"' | '<' | '>' | '|') || (character as u32) < 32)
}

fn trim_trailing_separator(path: &Path) -> String {
    let text = path.to_string_lossy().to_string();
    let trimmed = text.trim_end_matches(['\\', '/']);
    if trimmed.len() >= 3 && trimmed.as_bytes()[1] == b':' {
        trimmed.to_string()
    } else if trimmed.is_empty() {
        text
    } else {
        trimmed.to_string()
    }
}
