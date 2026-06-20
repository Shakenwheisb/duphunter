//! Shared, serde-serializable data types.
//!
//! Everything the frontend needs to see crosses the Tauri IPC boundary as one of
//! these types, so they all derive `Serialize`/`Deserialize`. Keeping them in one
//! module makes the contract between the Rust core and the React UI explicit.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Which content-hash algorithm to use for the exact-match pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HashAlgo {
    /// BLAKE3 — fast, default.
    #[default]
    Blake3,
    /// xxHash3 — fastest, non-cryptographic.
    Xxh3,
    /// SHA-256 — cryptographically strong, slower.
    Sha256,
}

/// Top-level detection mode. Exact and near-dup are kept strictly separate so
/// "similar" is never silently treated as "duplicate".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MatchMode {
    /// Byte-identical detection via the tiered size → partial → full → (paranoid) pipeline.
    #[default]
    Exact,
    /// Content-aware video matching via perceptual keyframe hashing + metadata.
    VideoNearDup,
}

/// How to treat symbolic links during discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SymlinkPolicy {
    /// Do not traverse or hash symlink targets (default, safest).
    #[default]
    Skip,
    /// Follow symlinks, with loop detection.
    Follow,
}

/// Per-scan exclusion rules. Applied during discovery so excluded files never
/// reach the hashing stages.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExcludeRules {
    /// Absolute subfolder paths to skip entirely.
    pub exclude_dirs: Vec<PathBuf>,
    /// Glob patterns (matched against the full path, e.g. `**/node_modules/**`).
    pub glob_patterns: Vec<String>,
    /// Extensions to skip (without the dot, case-insensitive, e.g. `tmp`).
    pub exclude_extensions: Vec<String>,
    /// Skip hidden and OS/system files.
    pub skip_hidden_system: bool,
    /// Minimum file size in bytes (inclusive). `None` = no lower bound.
    pub min_size: Option<u64>,
    /// Maximum file size in bytes (inclusive). `None` = no upper bound.
    pub max_size: Option<u64>,
}

/// Options that only apply to [`MatchMode::VideoNearDup`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoOptions {
    /// Similarity threshold in 0..=100. Two videos cluster together when their
    /// combined score is at or above this value.
    pub similarity_threshold: u8,
    /// Number of frames sampled per video at evenly spaced percentage offsets.
    pub frame_samples: u8,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self { similarity_threshold: 88, frame_samples: 5 }
    }
}

/// A complete scan request from the UI.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanConfig {
    /// Pooled root folders. Every file under every root is compared against every
    /// other — duplicates are found across roots, not just within one.
    pub roots: Vec<PathBuf>,
    pub excludes: ExcludeRules,
    pub mode: MatchMode,
    pub hash_algo: HashAlgo,
    /// Run a streaming byte-for-byte comparison on hash-identical groups.
    pub paranoid: bool,
    pub symlinks: SymlinkPolicy,
    pub video: VideoOptions,
}

/// Stable filesystem identity used to detect existing hardlinks. Two paths with
/// the same `(volume, file_id)` point at the same physical bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileId {
    pub volume: u64,
    pub file_id: u64,
}

/// One file discovered during a scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: PathBuf,
    /// Which scan root this file was found under (for cross-folder display).
    pub root: PathBuf,
    pub size: u64,
    /// Last-modified time, seconds since the Unix epoch.
    pub mtime: i64,
    /// Filesystem identity, when obtainable (used for hardlink awareness).
    pub identity: Option<FileId>,
    /// Set during video mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoMeta>,
}

/// Video/audio metadata extracted via ffprobe (video mode only).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VideoMeta {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub video_codec: String,
    pub audio_codec: String,
    pub bitrate: u64,
}

/// The role a member plays in a duplicate set (decided by the UI / bulk rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MemberRole {
    /// The copy to keep. Every set has at least one keeper.
    #[default]
    Keep,
    /// Marked for deletion / quarantine / linking.
    Remove,
}

/// One file within a duplicate set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DupMember {
    pub entry: FileEntry,
    pub role: MemberRole,
    /// True when this member is a hardlink to another member (shares the same
    /// physical bytes), so deleting it reclaims nothing.
    pub is_hardlink_of_other: bool,
}

/// A group of files determined to be duplicates of each other.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DupSet {
    pub id: u64,
    pub mode: MatchMode,
    pub members: Vec<DupMember>,
    /// Content hash for exact sets (hex). `None` for video near-dup sets.
    pub hash: Option<String>,
    /// Average pairwise similarity (0..=100) for video sets. `None` for exact.
    pub similarity: Option<u8>,
    /// Bytes reclaimable if all-but-one *distinct physical* copy is removed.
    /// Hardlinked copies and zero-byte files contribute nothing.
    pub reclaimable: u64,
    /// True if every member is zero bytes (handled specially, never auto-deleted).
    pub zero_byte: bool,
}

/// A non-fatal problem encountered during a scan (permission denied, unreadable
/// file, file changed mid-scan, …). Collected and reported, never panicked on.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanIssue {
    pub path: PathBuf,
    pub message: String,
}

/// The pipeline phase currently executing (drives the UI phase indicator).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Discovering,
    GroupingBySize,
    PartialHashing,
    FullHashing,
    Verifying,
    // Video phases:
    Probing,
    SamplingFrames,
    Fingerprinting,
    Clustering,
    Done,
}

/// A progress snapshot emitted to the UI during scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: Phase,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current_path: Option<PathBuf>,
    pub dup_sets: u64,
    pub reclaimable: u64,
    pub elapsed_secs: f64,
}

/// Outcome of a whole scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub sets: Vec<DupSet>,
    pub issues: Vec<ScanIssue>,
    pub files_scanned: u64,
    pub bytes_scanned: u64,
    pub elapsed_secs: f64,
    /// True if the scan was cancelled (results are partial).
    pub cancelled: bool,
}

/// What to do with the members marked `Remove`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionKind {
    /// Move to the OS trash / Recycle Bin (default, reversible).
    RecycleBin,
    /// Permanently delete (explicit opt-in only).
    PermanentDelete,
    /// Move to a quarantine folder.
    Quarantine,
    /// Replace the removed copy with a hardlink to the kept original.
    Hardlink,
    /// Replace the removed copy with a symlink to the kept original.
    Symlink,
}

/// A requested action over a set of duplicate sets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionPlan {
    pub kind: ActionKind,
    /// When true, compute and return the plan without touching the filesystem.
    pub dry_run: bool,
    /// Destination for [`ActionKind::Quarantine`].
    pub quarantine_dir: Option<PathBuf>,
    /// (set_id, keeper_path, removal_paths) tuples.
    pub targets: Vec<ActionTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionTarget {
    pub set_id: u64,
    pub keep: PathBuf,
    pub remove: Vec<PathBuf>,
}

/// What one action did to one file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionItemResult {
    pub set_id: u64,
    pub source: PathBuf,
    /// Where it went (trash handle, quarantine path, link target). `None` if skipped/failed.
    pub destination: Option<PathBuf>,
    pub bytes: u64,
    pub ok: bool,
    pub message: Option<String>,
}

/// Result of executing (or dry-running) an [`ActionPlan`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionReport {
    pub kind: ActionKind,
    pub dry_run: bool,
    pub items: Vec<ActionItemResult>,
    pub total_files: u64,
    pub total_bytes: u64,
    /// Path to the written undo manifest (`None` for dry runs).
    pub manifest_path: Option<PathBuf>,
}
