use std::io::Read;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::error::{AppError, AppResult};
use super::input_validator::{
    normalize_credential, normalize_integer, MAX_CONCURRENT_DOWNLOADS, MAX_RETRY_COUNT,
    MAX_TIMEOUT_SECONDS, MIN_TIMEOUT_SECONDS,
};
use super::models::AppSettings;
use super::url_guard::{resolve_public_addresses, validate_public_url, PublicUrl};

const RETRY_STATUSES: [u16; 6] = [408, 429, 500, 502, 503, 504];
const MAX_REDIRECTS: usize = 6;
const USER_AGENT: &str = "MinecraftServerDownloader/2.0";
const MAX_BACKOFF_MILLIS: u64 = 8_000;

#[derive(Debug, Clone)]
pub struct RequestProfile {
    pub timeout_seconds: i32,
    pub retry_count: i32,
    pub max_concurrent: i32,
    pub proxy: Option<PublicUrl>,
    pub credential_type: String,
    pub credential: String,
    pub user_id: Option<i64>,
}

impl RequestProfile {
    pub fn from_settings(
        settings: &AppSettings,
        credential_type: &str,
        credential: &str,
        user_id: Option<i64>,
    ) -> AppResult<Self> {
        let proxy = if settings.proxy.trim().is_empty() {
            None
        } else {
            Some(validate_public_url(&settings.proxy)?)
        };
        Ok(Self {
            timeout_seconds: normalize_integer(
                settings.request_timeout_seconds,
                MIN_TIMEOUT_SECONDS,
                MAX_TIMEOUT_SECONDS,
                30,
            ),
            retry_count: normalize_integer(settings.retry_count, 0, MAX_RETRY_COUNT, 2),
            max_concurrent: normalize_integer(
                settings.max_concurrent,
                1,
                MAX_CONCURRENT_DOWNLOADS,
                3,
            ),
            proxy,
            credential_type: credential_type.trim().to_ascii_lowercase(),
            credential: normalize_credential(credential)?,
            user_id,
        })
    }
}

pub struct SafeHttp {
    agent: ureq::Agent,
    profile: RequestProfile,
}

impl SafeHttp {
    pub fn new(profile: RequestProfile) -> AppResult<Self> {
        let mut builder = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(profile.timeout_seconds.max(1) as u64))
            .timeout_connect(Duration::from_secs(
                profile.timeout_seconds.clamp(1, 15) as u64
            ))
            .redirects(0)
            .user_agent(USER_AGENT)
            .resolver(guarded_resolve);
        if let Some(proxy) = profile.proxy.as_ref() {
            let proxy =
                ureq::Proxy::new(&proxy.text).map_err(|_| AppError::validation("url.blocked"))?;
            builder = builder.proxy(proxy);
        }
        Ok(Self {
            agent: builder.build(),
            profile,
        })
    }

    pub fn get_json(&self, url: &str, cancel: &Arc<AtomicBool>) -> AppResult<serde_json::Value> {
        let response = self.execute(url, "application/json", None, cancel, true)?;
        let value: serde_json::Value = response
            .into_json()
            .map_err(|error| AppError::general(format!("json.invalid: {error}")))?;
        Ok(value)
    }

    pub fn get_download(
        &self,
        url: &str,
        offset: i64,
        cancel: &Arc<AtomicBool>,
    ) -> AppResult<DownloadStream> {
        let response = self.execute(url, "application/octet-stream", Some(offset), cancel, true)?;
        let status = response.status();
        let partial = status == 206 && offset > 0;
        let total = expected_total(&response, offset);
        Ok(DownloadStream {
            partial,
            total,
            reader: Box::new(response.into_reader()),
        })
    }

    fn execute(
        &self,
        url: &str,
        accept: &str,
        offset: Option<i64>,
        cancel: &Arc<AtomicBool>,
        include_credentials: bool,
    ) -> AppResult<ureq::Response> {
        let mut current = validate_public_url(url)?;
        let initial_origin = current.origin.clone();
        let mut redirects = 0usize;
        let mut attempt = 0i32;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::general("operation.cancelled"));
            }
            let mut request = self
                .agent
                .get(&current.text)
                .set("Accept", accept)
                .set("Accept-Encoding", "identity");
            if let Some(position) = offset {
                if position > 0 {
                    request = request.set("Range", &format!("bytes={position}-"));
                }
            }
            if include_credentials && current.origin == initial_origin {
                if self.profile.credential_type == "bearer" && !self.profile.credential.is_empty() {
                    request = request.set(
                        "Authorization",
                        &format!("Bearer {}", self.profile.credential),
                    );
                }
                if let Some(user_id) = self.profile.user_id {
                    request = request.set("X-User-Id", &user_id.to_string());
                }
            }
            match request.call() {
                Ok(response) => return Ok(response),
                Err(ureq::Error::Status(status, response)) => {
                    if is_redirect(status) {
                        redirects += 1;
                        if redirects > MAX_REDIRECTS {
                            return Err(AppError::general("network.redirect"));
                        }
                        let location = response
                            .header("Location")
                            .ok_or_else(|| AppError::general("network.redirect"))?
                            .to_string();
                        current = resolve_location(&current, &location)?;
                        continue;
                    }
                    if RETRY_STATUSES.contains(&status) && attempt < self.profile.retry_count {
                        attempt += 1;
                        sleep_backoff(attempt, cancel)?;
                        continue;
                    }
                    return Err(AppError::general(format!("http.{status}")));
                }
                Err(ureq::Error::Transport(_)) => {
                    if attempt < self.profile.retry_count {
                        attempt += 1;
                        sleep_backoff(attempt, cancel)?;
                        continue;
                    }
                    return Err(AppError::general("network.failed"));
                }
            }
        }
    }
}

fn sleep_backoff(attempt: i32, cancel: &Arc<AtomicBool>) -> AppResult<()> {
    let millis = backoff_millis(attempt);
    let mut remaining = millis;
    while remaining > 0 {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::general("operation.cancelled"));
        }
        let step = remaining.min(50);
        std::thread::sleep(Duration::from_millis(step));
        remaining -= step;
    }
    Ok(())
}

fn guarded_resolve(netloc: &str) -> std::io::Result<Vec<SocketAddr>> {
    let (host, port) = netloc
        .rsplit_once(':')
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "url.invalid"))?;
    let port: u16 = port
        .parse()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "url.invalid"))?;
    resolve_public_addresses(host, port)
}

pub fn resolve_location(current: &PublicUrl, location: &str) -> AppResult<PublicUrl> {
    let trimmed = location.trim();
    if trimmed.is_empty() {
        return Err(AppError::general("network.redirect"));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return validate_public_url(trimmed);
    }
    if let Some(rest) = trimmed.strip_prefix("//") {
        return validate_public_url(&format!("{}://{rest}", current.scheme));
    }
    let path = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        let base = match current.path.rfind('/') {
            Some(index) => &current.path[..=index],
            None => "/",
        };
        format!("{base}{trimmed}")
    };
    validate_public_url(&format!("{}{path}", current.origin))
}

pub fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

pub fn should_retry_status(status: u16) -> bool {
    RETRY_STATUSES.contains(&status)
}

pub fn backoff_millis(attempt: i32) -> u64 {
    (1_000u64 << attempt.clamp(0, 3)).min(MAX_BACKOFF_MILLIS)
}

fn expected_total(response: &ureq::Response, offset: i64) -> i64 {
    if let Some(value) = response.header("Content-Range") {
        if let Some((_, total)) = value.rsplit_once('/') {
            if let Ok(parsed) = total.trim().parse::<i64>() {
                return parsed;
            }
        }
    }
    match response
        .header("Content-Length")
        .and_then(|value| value.parse::<i64>().ok())
    {
        Some(length) if length > 0 => offset + length,
        _ => 0,
    }
}

pub struct DownloadStream {
    pub partial: bool,
    pub total: i64,
    reader: Box<dyn Read + Send + Sync + 'static>,
}

impl DownloadStream {
    pub fn read_chunk(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buffer)
    }
}
