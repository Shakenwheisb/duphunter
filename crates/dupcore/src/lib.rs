//! # dupcore
//!
//! GUI-independent core for DupHunter: directory discovery, the tiered exact-match
//! hashing pipeline, video near-duplicate fingerprinting, filesystem-identity
//! (hardlink) awareness, safe destructive actions, an undo manifest, an on-disk
//! cache, and JSON/CSV export.
//!
//! The Tauri layer is a thin wrapper around [`scan`] plus [`actions::execute`].
//! Everything here is synchronous and testable with `cargo test`.

pub mod actions;
pub mod cache;
pub mod control;
pub mod discovery;
pub mod error;
pub mod export;
pub mod hashing;
pub mod identity;
pub mod manifest;
pub mod model;
pub mod pipeline;
pub mod video;

pub use control::{ProgressSink, ScanControl};
pub use error::{DupError, Result};
pub use model::*;

/// Run a scan according to `config`, dispatching to the exact or video pipeline.
///
/// `control` drives pause/resume/cancel; `progress` receives live updates. This
/// call blocks, so the Tauri layer runs it on a background thread.
pub fn scan(
    config: &ScanConfig,
    control: &ScanControl,
    progress: &dyn ProgressSink,
) -> Result<ScanResult> {
    if config.roots.is_empty() {
        return Err(DupError::Config("no folders selected".into()));
    }
    match config.mode {
        MatchMode::Exact => Ok(pipeline::run_exact(config, control, progress)),
        MatchMode::VideoNearDup => video::run_video(config, control, progress),
    }
}
