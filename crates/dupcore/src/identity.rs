//! Filesystem identity for hardlink awareness.
//!
//! Two paths that resolve to the same physical extent on disk are hardlinks. We
//! must report them so the user sees them, but they contribute **zero**
//! reclaimable space (deleting one doesn't free the bytes).
//!
//! Hardlink grouping in the pipeline uses [`same_file::Handle`] (cross-platform,
//! stable: inode+device on Unix, `GetFileInformationByHandle` on Windows). This
//! module additionally exposes a serializable [`FileId`] for export/debugging on
//! Unix, where it's available cheaply from already-fetched metadata. (The Windows
//! equivalents in `std` are still nightly-only, so we return `None` there and rely
//! on `Handle` for the actual hardlink logic.)

use crate::model::FileId;
use std::path::Path;

/// A serializable identity, when cheaply available. Used only for export; the
/// pipeline's hardlink detection uses `same_file::Handle` directly.
pub fn file_id(path: &Path) -> Option<FileId> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(path).ok()?;
        Some(FileId { volume: meta.dev(), file_id: meta.ino() })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}
