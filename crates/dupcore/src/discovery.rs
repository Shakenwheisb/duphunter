//! File discovery: walk the pooled roots and apply exclusion rules.
//!
//! Runs first, before any hashing. Recoverable problems (permission denied,
//! vanished files) are collected as [`ScanIssue`]s rather than aborting the walk.
//! All roots are pooled into one flat list of [`FileEntry`], so later stages
//! compare every file against every other regardless of which root it came from.

use crate::control::ScanControl;
use crate::model::{ExcludeRules, FileEntry, ScanIssue, SymlinkPolicy};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Compiled form of [`ExcludeRules`] for fast per-file checks.
struct Excluder {
    dirs: Vec<PathBuf>,
    globs: Option<GlobSet>,
    extensions: Vec<String>,
    skip_hidden_system: bool,
    min_size: Option<u64>,
    max_size: Option<u64>,
}

impl Excluder {
    fn compile(rules: &ExcludeRules) -> Self {
        let globs = if rules.glob_patterns.is_empty() {
            None
        } else {
            let mut b = GlobSetBuilder::new();
            for pat in &rules.glob_patterns {
                if let Ok(g) = Glob::new(pat) {
                    b.add(g);
                }
            }
            b.build().ok()
        };
        Self {
            dirs: rules.exclude_dirs.clone(),
            globs,
            extensions: rules.exclude_extensions.iter().map(|e| e.to_lowercase()).collect(),
            skip_hidden_system: rules.skip_hidden_system,
            min_size: rules.min_size,
            max_size: rules.max_size,
        }
    }

    fn dir_excluded(&self, path: &Path) -> bool {
        self.dirs.iter().any(|d| path.starts_with(d))
    }

    /// Returns true if the file should be skipped.
    fn file_excluded(&self, path: &Path, size: u64) -> bool {
        if let Some(min) = self.min_size {
            if size < min {
                return true;
            }
        }
        if let Some(max) = self.max_size {
            if size > max {
                return true;
            }
        }
        if let Some(globs) = &self.globs {
            if globs.is_match(path) {
                return true;
            }
        }
        if !self.extensions.is_empty() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if self.extensions.contains(&ext.to_lowercase()) {
                    return true;
                }
            }
        }
        if self.skip_hidden_system && is_hidden_or_system(path) {
            return true;
        }
        false
    }
}

/// Cross-platform "hidden or system file" check.
fn is_hidden_or_system(path: &Path) -> bool {
    // Unix-style dotfiles everywhere.
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.starts_with('.') {
            return true;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const HIDDEN: u32 = 0x2;
        const SYSTEM: u32 = 0x4;
        if let Ok(meta) = std::fs::metadata(path) {
            let attrs = meta.file_attributes();
            if attrs & (HIDDEN | SYSTEM) != 0 {
                return true;
            }
        }
    }
    false
}

/// Walk all roots, returning discovered files and any non-fatal issues.
pub fn discover(
    roots: &[PathBuf],
    rules: &ExcludeRules,
    symlinks: SymlinkPolicy,
    control: &ScanControl,
) -> (Vec<FileEntry>, Vec<ScanIssue>) {
    let excluder = Excluder::compile(rules);
    let mut entries = Vec::new();
    let mut issues = Vec::new();

    for root in roots {
        if !control.checkpoint() {
            break;
        }
        walk_root(root, root, &excluder, symlinks, control, &mut entries, &mut issues);
    }
    (entries, issues)
}

fn walk_root(
    root: &Path,
    dir: &Path,
    excluder: &Excluder,
    symlinks: SymlinkPolicy,
    control: &ScanControl,
    entries: &mut Vec<FileEntry>,
    issues: &mut Vec<ScanIssue>,
) {
    // We use jwalk's parallel walker but consume sequentially here; hashing is
    // where the heavy parallelism lives. follow_links is gated by policy.
    let follow = matches!(symlinks, SymlinkPolicy::Follow);
    let walker = jwalk::WalkDir::new(dir).follow_links(follow).skip_hidden(false);

    for dent in walker {
        if !control.checkpoint() {
            return;
        }
        let dent = match dent {
            Ok(d) => d,
            Err(e) => {
                issues.push(ScanIssue {
                    path: dir.to_path_buf(),
                    message: format!("walk error: {e}"),
                });
                continue;
            }
        };
        let path = dent.path();

        if dent.file_type().is_dir() {
            if excluder.dir_excluded(&path) {
                // jwalk can't prune mid-stream here; we simply skip its files via
                // the dir_excluded check below. (Children carry the prefix.)
            }
            continue;
        }

        // Skip symlinks entirely under Skip policy.
        if dent.file_type().is_symlink() && !follow {
            continue;
        }
        if excluder.dir_excluded(&path) {
            continue;
        }

        let meta = match dent.metadata() {
            Ok(m) => m,
            Err(e) => {
                issues.push(ScanIssue { path: path.clone(), message: format!("stat failed: {e}") });
                continue;
            }
        };
        let size = meta.len();
        if excluder.file_excluded(&path, size) {
            continue;
        }

        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        entries.push(FileEntry {
            path,
            root: root.to_path_buf(),
            size,
            mtime,
            // Identity is resolved lazily in `pipeline::build_set` (only for
            // confirmed duplicate candidates) via `same_file::Handle`, so we
            // avoid an extra file open per discovered file here.
            identity: None,
            video: None,
        });
    }
}
