//! Export duplicate-set results to JSON and CSV.

use crate::error::{DupError, Result};
use crate::model::DupSet;
use std::path::Path;

/// Pretty JSON dump of all sets.
pub fn to_json(sets: &[DupSet], path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(sets).map_err(|e| DupError::Other(e.to_string()))?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Flat CSV: one row per file, with its set id, recommended keep, hash and root.
pub fn to_csv(sets: &[DupSet], path: &Path) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path).map_err(|e| DupError::Other(e.to_string()))?;
    wtr.write_record([
        "set_id",
        "mode",
        "role",
        "path",
        "root",
        "size_bytes",
        "mtime_unix",
        "hash",
        "similarity",
        "reclaimable_bytes",
        "is_hardlink",
    ])
    .map_err(|e| DupError::Other(e.to_string()))?;

    for set in sets {
        for m in &set.members {
            wtr.write_record([
                set.id.to_string(),
                format!("{:?}", set.mode),
                format!("{:?}", m.role),
                m.entry.path.display().to_string(),
                m.entry.root.display().to_string(),
                m.entry.size.to_string(),
                m.entry.mtime.to_string(),
                set.hash.clone().unwrap_or_default(),
                set.similarity.map(|s| s.to_string()).unwrap_or_default(),
                set.reclaimable.to_string(),
                m.is_hardlink_of_other.to_string(),
            ])
            .map_err(|e| DupError::Other(e.to_string()))?;
        }
    }
    wtr.flush()?;
    Ok(())
}
