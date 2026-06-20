//! Error types. Fatal errors abort an operation; recoverable per-file problems
//! are collected as [`crate::model::ScanIssue`] instead, so one bad file never
//! kills a whole scan.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DupError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid scan config: {0}")]
    Config(String),

    #[error("cache error: {0}")]
    Cache(String),

    #[error("ffmpeg/ffprobe not found on PATH — install it to use video mode")]
    FfmpegMissing,

    #[error("ffmpeg error: {0}")]
    Ffmpeg(String),

    #[error("action refused for safety: {0}")]
    UnsafeAction(String),

    #[error("operation cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, DupError>;
