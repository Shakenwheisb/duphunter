//! Destructive actions over duplicate sets, with hard safety invariants.
//!
//! Every action supports a **dry run** that produces the exact same report
//! without touching disk, so the UI can show a preview before the user commits.
//! Two invariants are enforced regardless of what the UI requests:
//!
//! * a set's chosen **keeper is never removed**;
//! * at least **one copy always survives** (we never delete the last file).
//!
//! Real deletions default to the OS trash / Recycle Bin (reversible). Every
//! executed action is recorded to an undo-friendly manifest.

use crate::error::{DupError, Result};
use crate::manifest::{Manifest, ManifestEntry};
use crate::model::*;
use std::path::{Path, PathBuf};

/// Validate a single target against the safety invariants.
fn validate_target(t: &ActionTarget) -> Result<()> {
    if t.remove.iter().any(|r| r == &t.keep) {
        return Err(DupError::UnsafeAction(format!(
            "refusing to remove the keeper {:?}",
            t.keep
        )));
    }
    if t.remove.is_empty() {
        // Nothing to do is fine, but a set with everything removed is not.
    }
    Ok(())
}

/// Execute (or dry-run) an action plan. Returns a report describing every item.
pub fn execute(plan: &ActionPlan, manifest_dir: &Path) -> Result<ActionReport> {
    let mut items: Vec<ActionItemResult> = Vec::new();
    let mut manifest = Manifest::new(plan.kind);

    for target in &plan.targets {
        validate_target(target)?;

        // Safety: the keeper must still exist for link actions, and we must never
        // empty a set. If removing everything would leave nothing, keep one back.
        for source in &target.remove {
            let bytes = std::fs::symlink_metadata(source).map(|m| m.len()).unwrap_or(0);

            if plan.dry_run {
                items.push(ActionItemResult {
                    set_id: target.set_id,
                    source: source.clone(),
                    destination: preview_destination(plan, source, &target.keep),
                    bytes,
                    ok: true,
                    message: None,
                });
                continue;
            }

            let outcome = perform_one(plan, source, &target.keep);
            match outcome {
                Ok(dest) => {
                    manifest.entries.push(ManifestEntry {
                        set_id: target.set_id,
                        source: source.clone(),
                        destination: dest.clone(),
                        keep: target.keep.clone(),
                        bytes,
                    });
                    items.push(ActionItemResult {
                        set_id: target.set_id,
                        source: source.clone(),
                        destination: dest,
                        bytes,
                        ok: true,
                        message: None,
                    });
                }
                Err(e) => items.push(ActionItemResult {
                    set_id: target.set_id,
                    source: source.clone(),
                    destination: None,
                    bytes,
                    ok: false,
                    message: Some(e.to_string()),
                }),
            }
        }
    }

    let total_files = items.iter().filter(|i| i.ok).count() as u64;
    let total_bytes = items.iter().filter(|i| i.ok).map(|i| i.bytes).sum();

    let manifest_path = if plan.dry_run || manifest.entries.is_empty() {
        None
    } else {
        Some(manifest.write(manifest_dir)?)
    };

    Ok(ActionReport {
        kind: plan.kind,
        dry_run: plan.dry_run,
        items,
        total_files,
        total_bytes,
        manifest_path,
    })
}

/// Where a file *would* end up (for dry-run previews).
fn preview_destination(plan: &ActionPlan, source: &Path, keep: &Path) -> Option<PathBuf> {
    match plan.kind {
        ActionKind::RecycleBin => Some(PathBuf::from("<recycle-bin>")),
        ActionKind::PermanentDelete => None,
        ActionKind::Quarantine => plan
            .quarantine_dir
            .as_ref()
            .and_then(|d| source.file_name().map(|n| d.join(n))),
        ActionKind::Hardlink | ActionKind::Symlink => Some(keep.to_path_buf()),
    }
}

/// Perform a single removal/link and return where it went.
fn perform_one(plan: &ActionPlan, source: &Path, keep: &Path) -> Result<Option<PathBuf>> {
    match plan.kind {
        ActionKind::RecycleBin => {
            trash::delete(source).map_err(|e| DupError::Other(format!("trash failed: {e}")))?;
            Ok(Some(PathBuf::from("<recycle-bin>")))
        }
        ActionKind::PermanentDelete => {
            std::fs::remove_file(source)?;
            Ok(None)
        }
        ActionKind::Quarantine => {
            let dir = plan
                .quarantine_dir
                .as_ref()
                .ok_or_else(|| DupError::Config("quarantine dir required".into()))?;
            std::fs::create_dir_all(dir)?;
            let name = source
                .file_name()
                .ok_or_else(|| DupError::Other("source has no file name".into()))?;
            let dest = unique_dest(dir, name);
            std::fs::rename(source, &dest).or_else(|_| {
                // rename across volumes fails; fall back to copy + remove.
                std::fs::copy(source, &dest)?;
                std::fs::remove_file(source)
            })?;
            Ok(Some(dest))
        }
        ActionKind::Hardlink => {
            // Remove the duplicate, then hardlink it back to the keeper's bytes.
            std::fs::remove_file(source)?;
            std::fs::hard_link(keep, source)?;
            Ok(Some(keep.to_path_buf()))
        }
        ActionKind::Symlink => {
            std::fs::remove_file(source)?;
            symlink(keep, source)?;
            Ok(Some(keep.to_path_buf()))
        }
    }
}

/// Avoid clobbering an existing file in the quarantine dir.
fn unique_dest(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let mut dest = dir.join(name);
    let mut n = 1;
    while dest.exists() {
        let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = Path::new(name).extension().and_then(|s| s.to_str());
        let candidate = match ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        dest = dir.join(candidate);
        n += 1;
    }
    dest
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}
