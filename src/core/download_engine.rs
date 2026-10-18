use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use sha1::{Digest, Sha1};

use super::app_log;
use super::disk_space::available_bytes;
use super::error::{AppError, AppResult};
use super::input_validator::normalize_directory;
use super::models::{
    AppSettings, DownloadOutcome, DownloadProgress, DownloadSummary, DownloadTiming, SourceProfile,
    VersionInfo, VersionJob,
};
use super::net::SafeHttp;
use super::version_catalog::VersionCatalog;

const BUFFER_SIZE: usize = 131_072;
const ESTIMATE_PER_FILE: i64 = 41_943_040;
const MAX_FILE_ATTEMPTS: i32 = 3;

pub trait ProgressSink: Send + Sync {
    fn report(&self, progress: DownloadProgress);
    fn log(&self, key: &str, is_error: bool);
}

pub struct DownloadEngine {
    pause: Arc<PauseController>,
    cancel: Arc<AtomicBool>,
}

impl DownloadEngine {
    pub fn new() -> Self {
        Self {
            pause: Arc::new(PauseController::new()),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn pause(&self) {
        self.pause.pause();
    }

    pub fn resume(&self) {
        self.pause.resume();
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        self.pause.resume();
    }

    pub fn run(
        &self,
        http: &SafeHttp,
        catalog: &VersionCatalog<'_>,
        versions: &[VersionInfo],
        settings: &AppSettings,
        sink: &dyn ProgressSink,
    ) -> DownloadSummary {
        self.cancel.store(false, Ordering::SeqCst);
        self.pause.resume();
        let started = Instant::now();
        let done_bytes = AtomicI64::new(0);
        let total_bytes = AtomicI64::new(0);
        let completed = AtomicI64::new(0);
        let failed = AtomicI64::new(0);

        let jobs = self.resolve_jobs(catalog, versions, settings, sink);
        if self.cancel.load(Ordering::SeqCst) {
            return DownloadSummary {
                outcome: DownloadOutcome::Stopped,
                completed: 0,
                failed: 0,
                total_bytes: 0,
            };
        }
        for job in jobs.iter().filter(|job| !job.version.is_downloaded) {
            total_bytes.fetch_add(
                if job.size > 0 {
                    job.size
                } else {
                    ESTIMATE_PER_FILE
                },
                Ordering::SeqCst,
            );
        }
        if jobs.is_empty() || jobs.iter().all(|job| job.version.is_downloaded) {
            sink.log("download.allDone", false);
            return DownloadSummary {
                outcome: DownloadOutcome::Completed,
                completed: versions.len() as i32,
                failed: 0,
                total_bytes: 0,
            };
        }
        let total = total_bytes.load(Ordering::SeqCst);
        if available_bytes(&settings.download_directory) < total {
            sink.log("download.diskFull", true);
            return DownloadSummary {
                outcome: DownloadOutcome::DiskFull,
                completed: 0,
                failed: 0,
                total_bytes: total,
            };
        }

        let root = match normalize_directory(&settings.download_directory) {
            Ok(value) => value,
            Err(error) => {
                app_log::handle_unhandled(error.message());
                String::new()
            }
        };
        let workers = settings.max_concurrent.clamp(1, 8) as usize;
        let cursor = Mutex::new(0usize);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    if self.cancel.load(Ordering::SeqCst) {
                        break;
                    }
                    let index = {
                        let mut guard = cursor.lock().unwrap_or_else(|error| error.into_inner());
                        let index = *guard;
                        *guard += 1;
                        index
                    };
                    let Some(job) = jobs.get(index) else {
                        break;
                    };
                    if job.version.is_downloaded {
                        completed.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                    match self.download_one(http, job, &root, &done_bytes, sink) {
                        Ok(_) => {
                            completed.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(error) if error.message() == "operation.cancelled" => {}
                        Err(error) => {
                            app_log::warn(&format!(
                                "{}: {}",
                                job.version.jar_name,
                                error.message()
                            ));
                            failed.fetch_add(1, Ordering::SeqCst);
                            sink.log(&format!("download.failed:{}", job.version.jar_name), true);
                        }
                    }
                    let timing = timing_snapshot(&done_bytes, &total_bytes, started);
                    sink.report(DownloadProgress {
                        completed: completed.load(Ordering::SeqCst) as i32,
                        total: versions.len() as i32,
                        done_bytes: done_bytes.load(Ordering::SeqCst),
                        total_bytes: total_bytes.load(Ordering::SeqCst),
                        bytes_per_second: timing.bytes_per_second,
                        remaining_seconds: timing.remaining_seconds,
                    });
                });
            }
        });

        let outcome = if self.cancel.load(Ordering::SeqCst) {
            DownloadOutcome::Stopped
        } else if failed.load(Ordering::SeqCst) > 0 {
            DownloadOutcome::Failed
        } else {
            DownloadOutcome::Completed
        };
        let summary = DownloadSummary {
            outcome,
            completed: completed.load(Ordering::SeqCst) as i32,
            failed: failed.load(Ordering::SeqCst) as i32,
            total_bytes: total_bytes.load(Ordering::SeqCst),
        };
        sink.log(
            match outcome {
                DownloadOutcome::Stopped => "download.stopped",
                DownloadOutcome::Failed => "download.finishedWithErrors",
                _ => "download.allDone",
            },
            outcome == DownloadOutcome::Failed,
        );
        summary
    }

    fn resolve_jobs(
        &self,
        catalog: &VersionCatalog<'_>,
        versions: &[VersionInfo],
        settings: &AppSettings,
        sink: &dyn ProgressSink,
    ) -> Vec<VersionJob> {
        let collected = Mutex::new(Vec::new());
        let cursor = Mutex::new(0usize);
        let workers = settings.max_concurrent.clamp(1, 8) as usize;
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    if self.cancel.load(Ordering::SeqCst) {
                        break;
                    }
                    let index = {
                        let mut guard = cursor.lock().unwrap_or_else(|error| error.into_inner());
                        let index = *guard;
                        *guard += 1;
                        index
                    };
                    let Some(version) = versions.get(index) else {
                        break;
                    };
                    match catalog.resolve(settings, version, &self.cancel) {
                        Ok(Some(job)) => collected
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .push(job),
                        Ok(None) => sink.log(&format!("download.noUrl:{}", version.id), true),
                        Err(error) if error.message() == "operation.cancelled" => break,
                        Err(error) => {
                            app_log::warn(error.message());
                            sink.log(&format!("download.metadataFailed:{}", version.id), true);
                        }
                    }
                });
            }
        });
        let mut jobs = collected
            .into_inner()
            .unwrap_or_else(|error| error.into_inner());
        jobs.sort_by(|left, right| left.version.id.cmp(&right.version.id));
        jobs
    }

    fn download_one(
        &self,
        http: &SafeHttp,
        job: &VersionJob,
        root: &str,
        done_bytes: &AtomicI64,
        sink: &dyn ProgressSink,
    ) -> AppResult<i64> {
        let directory = Path::new(root).join(&job.version.folder);
        fs::create_dir_all(&directory)?;
        let final_path = directory.join(&job.version.jar_name);
        let part_path = final_path.with_extension("jar.part");
        sink.log(
            &format!("download.starting:{}", job.version.jar_name),
            false,
        );
        let mut attempts = 0;
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(AppError::general("operation.cancelled"));
            }
            self.pause.wait(&self.cancel)?;
            match self.download_attempt(http, job, &part_path, &final_path, done_bytes) {
                Ok(written) => {
                    sink.log(
                        &format!("download.completed:{}", job.version.jar_name),
                        false,
                    );
                    return Ok(written);
                }
                Err(error) if error.message() == "operation.cancelled" => return Err(error),
                Err(error) if attempts < MAX_FILE_ATTEMPTS => {
                    attempts += 1;
                    app_log::warn(&format!("retry {}: {}", attempts, error.message()));
                    std::thread::sleep(std::time::Duration::from_millis(750 * attempts as u64));
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn download_attempt(
        &self,
        http: &SafeHttp,
        job: &VersionJob,
        part_path: &Path,
        final_path: &Path,
        done_bytes: &AtomicI64,
    ) -> AppResult<i64> {
        let mut offset = part_path
            .metadata()
            .map(|meta| meta.len() as i64)
            .unwrap_or(0);
        if job.size > 0 && offset > job.size {
            let _ = fs::remove_file(part_path);
            offset = 0;
        }
        let mut stream = http.get_download(&job.server_url, offset, &self.cancel)?;
        if offset > 0 && !stream.partial {
            offset = 0;
        }
        let expected = if stream.total > 0 {
            stream.total
        } else if job.size > 0 {
            job.size
        } else {
            0
        };
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(offset > 0)
            .truncate(offset == 0)
            .open(part_path)?;
        if offset > 0 {
            file.seek(SeekFrom::End(0))?;
        }
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut written = offset;
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(AppError::general("operation.cancelled"));
            }
            self.pause.wait(&self.cancel)?;
            let read = stream.read_chunk(&mut buffer)?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])?;
            written += read as i64;
            done_bytes.fetch_add(read as i64, Ordering::SeqCst);
        }
        file.flush()?;
        drop(file);
        if expected > 0 && written != expected {
            return Err(AppError::general("download.incomplete"));
        }
        if !job.sha1.is_empty() && !verify_sha1(part_path, &job.sha1)? {
            let _ = fs::remove_file(part_path);
            return Err(AppError::general("download.checksum"));
        }
        fs::rename(part_path, final_path)?;
        Ok(written)
    }
}

impl Default for DownloadEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn verify_sha1(path: &Path, expected: &str) -> AppResult<bool> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; BUFFER_SIZE];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let actual: String = digest.iter().map(|byte| format!("{byte:02X}")).collect();
    Ok(actual.eq_ignore_ascii_case(expected))
}

fn timing_snapshot(done: &AtomicI64, total: &AtomicI64, started: Instant) -> DownloadTiming {
    let seconds = started.elapsed().as_secs_f64();
    if seconds <= 0.0 {
        return DownloadTiming {
            bytes_per_second: 0.0,
            remaining_seconds: 0.0,
        };
    }
    let done_value = done.load(Ordering::SeqCst) as f64;
    let total_value = total.load(Ordering::SeqCst) as f64;
    let rate = done_value / seconds;
    DownloadTiming {
        bytes_per_second: rate,
        remaining_seconds: if rate > 0.0 {
            ((total_value - done_value).max(0.0)) / rate
        } else {
            0.0
        },
    }
}

struct PauseController {
    lock: Mutex<bool>,
    signal: Condvar,
}

impl PauseController {
    fn new() -> Self {
        Self {
            lock: Mutex::new(false),
            signal: Condvar::new(),
        }
    }

    fn pause(&self) {
        let mut guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        *guard = true;
    }

    fn resume(&self) {
        let mut guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        *guard = false;
        self.signal.notify_all();
    }

    fn wait(&self, cancel: &Arc<AtomicBool>) -> AppResult<()> {
        let mut guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        while *guard {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::general("operation.cancelled"));
            }
            let (next, timeout) = self
                .signal
                .wait_timeout(guard, std::time::Duration::from_millis(120))
                .unwrap_or_else(|error| error.into_inner());
            guard = next;
            let _ = timeout;
        }
        Ok(())
    }
}

pub fn source_profile_credentials(
    settings: &AppSettings,
    sources: &[SourceProfile],
    credentials: &std::collections::BTreeMap<String, String>,
) -> (String, String, Option<i64>) {
    if settings.mirror != "custom" {
        return ("none".to_string(), String::new(), None);
    }
    let Some(source) = settings
        .custom_source_id
        .as_ref()
        .and_then(|id| sources.iter().find(|source| &source.id == id))
    else {
        return ("none".to_string(), String::new(), None);
    };
    let credential = credentials.get(&source.id).cloned().unwrap_or_default();
    (source.credential_type.clone(), credential, source.user_id)
}
