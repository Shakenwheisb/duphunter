//! Cooperative pause / resume / cancel and progress reporting.
//!
//! The scan runs on a background thread; the UI thread flips an atomic on this
//! shared handle. Worker code calls [`ScanControl::checkpoint`] at chunk/file
//! boundaries, where it blocks while paused and returns `false` once cancelled —
//! so cancellation is prompt and never leaves a half-finished destructive action
//! (the pipeline only ever *reads* during a scan).

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex};

const RUNNING: u8 = 0;
const PAUSED: u8 = 1;
const CANCELLED: u8 = 2;

/// Clone-able handle shared between the UI thread and scan workers.
#[derive(Clone)]
pub struct ScanControl {
    state: Arc<AtomicU8>,
    /// Used to wake paused workers without busy-spinning.
    waker: Arc<(Mutex<()>, Condvar)>,
}

impl Default for ScanControl {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanControl {
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(RUNNING)),
            waker: Arc::new((Mutex::new(()), Condvar::new())),
        }
    }

    pub fn pause(&self) {
        self.state.store(PAUSED, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.state.store(RUNNING, Ordering::SeqCst);
        self.waker.1.notify_all();
    }

    pub fn cancel(&self) {
        self.state.store(CANCELLED, Ordering::SeqCst);
        self.waker.1.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::SeqCst) == CANCELLED
    }

    /// Block while paused; return `false` if the scan has been cancelled.
    /// Workers should bail out cleanly when this returns `false`.
    pub fn checkpoint(&self) -> bool {
        loop {
            match self.state.load(Ordering::SeqCst) {
                CANCELLED => return false,
                PAUSED => {
                    let guard = self.waker.0.lock().unwrap();
                    // Re-check under the lock to avoid missing a notify.
                    if self.state.load(Ordering::SeqCst) == PAUSED {
                        let _unused = self
                            .waker
                            .1
                            .wait_timeout(guard, std::time::Duration::from_millis(100))
                            .unwrap();
                    }
                }
                _ => return true,
            }
        }
    }
}

/// Sink for progress updates. The Tauri layer implements this to emit IPC events;
/// tests use [`NullProgress`].
pub trait ProgressSink: Send + Sync {
    fn report(&self, progress: &crate::model::Progress);
}

/// No-op progress sink for tests and headless use.
pub struct NullProgress;
impl ProgressSink for NullProgress {
    fn report(&self, _progress: &crate::model::Progress) {}
}
