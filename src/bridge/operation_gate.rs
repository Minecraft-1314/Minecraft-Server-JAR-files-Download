use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::error::{AppError, AppResult};

#[derive(Clone)]
pub struct OperationGate {
    inner: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}

impl OperationGate {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn create(&self) -> AppResult<OperationLease> {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(current) = guard.as_ref() {
            if !current.load(Ordering::SeqCst) {
                return Err(AppError::general("operation.running"));
            }
        }
        let flag = Arc::new(AtomicBool::new(false));
        *guard = Some(flag.clone());
        Ok(OperationLease {
            gate: self.clone(),
            flag,
            released: false,
        })
    }

    pub fn cancel_current(&self) {
        let guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(current) = guard.as_ref() {
            current.store(true, Ordering::SeqCst);
        }
    }

    fn release(&self, flag: &Arc<AtomicBool>) {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(current) = guard.as_ref() {
            if Arc::ptr_eq(current, flag) {
                *guard = None;
            }
        }
    }
}

impl Default for OperationGate {
    fn default() -> Self {
        Self::new()
    }
}

pub struct OperationLease {
    gate: OperationGate,
    flag: Arc<AtomicBool>,
    released: bool,
}

impl OperationLease {
    pub fn token(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }

    pub fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.gate.release(&self.flag);
    }
}

impl Drop for OperationLease {
    fn drop(&mut self) {
        self.release();
    }
}
