//! Undo-friendly action manifest.
//!
//! Every executed destructive action appends to a timestamped JSON manifest so
//! the user has a record of exactly what was removed, where it went, and which
//! original it was kept against. Trashed files can be restored from the Recycle
//! Bin; quarantined/linked files can be reversed from the recorded destinations.

use crate::error::Result;
use crate::model::ActionKind;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub set_id: u64,
    pub source: PathBuf,
    /// Where the file went (trash marker, quarantine path, or link target).
    pub destination: Option<PathBuf>,
    pub keep: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub kind: ActionKind,
    pub created_unix: u64,
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    pub fn new(kind: ActionKind) -> Self {
        let created_unix =
            SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        Self { kind, created_unix, entries: Vec::new() }
    }

    /// Write the manifest into `dir`, returning the file path.
    pub fn write(&self, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("duphunter-manifest-{}.json", self.created_unix));
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::DupError::Other(e.to_string()))?;
        std::fs::write(&path, json)?;
        Ok(path)
    }
}
