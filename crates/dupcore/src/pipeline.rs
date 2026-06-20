//! The tiered exact-match pipeline.
//!
//! Candidates are narrowed from cheap checks to expensive ones so we read as few
//! bytes as possible:
//!
//! 1. **Group by size** — different byte-length ⇒ cannot be identical.
//! 2. **Partial hash** — head/middle/tail sample; splits most non-dupes cheaply.
//! 3. **Full hash** — definitive whole-file hash on the survivors.
//! 4. **Paranoid byte-compare** (optional) — rules out the rare hash collision
//!    before we treat files as identical, because results drive deletion.
//!
//! Hashing fans out across cores with rayon. The orchestrator reports progress
//! and honours pause/cancel; the small grouping helpers below are pure and are
//! exercised directly by the unit tests.

use crate::control::{ProgressSink, ScanControl};
use crate::discovery;
use crate::hashing::{bytes_equal, full_hash, partial_hash};
use crate::model::*;
use rayon::prelude::*;
use same_file::Handle;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Group entries by exact byte size, keeping only groups with more than one
/// member (singletons can't be duplicates). Zero-byte files are kept together.
pub fn group_by_size(entries: Vec<FileEntry>) -> Vec<Vec<FileEntry>> {
    group_by(entries, |e| e.size).into_iter().filter(|g| g.len() > 1).collect()
}

/// Generic stable grouping by an arbitrary key.
fn group_by<K, F>(entries: Vec<FileEntry>, key_fn: F) -> Vec<Vec<FileEntry>>
where
    K: std::cmp::Eq + std::hash::Hash,
    F: Fn(&FileEntry) -> K,
{
    let mut map: HashMap<K, Vec<FileEntry>> = HashMap::new();
    let mut order: Vec<K> = Vec::new();
    for e in entries {
        let k = key_fn(&e);
        if !map.contains_key(&k) {
            // Preserve first-seen order for deterministic output/tests.
            // (We can't clone arbitrary K cheaply, so re-key via entry.)
        }
        map.entry(k).or_default().push(e);
    }
    // Order isn't meaningful for hashes; sort groups by first path for determinism.
    let _ = &mut order;
    let mut groups: Vec<Vec<FileEntry>> = map.into_values().collect();
    groups.sort_by(|a, b| a[0].path.cmp(&b[0].path));
    groups
}

/// Refine one same-size (or same-partial-hash) group by a hashing function,
/// returning sub-groups that still collide. Generic over the hasher so tests can
/// inject a deliberately colliding hash to exercise paranoid verification.
pub fn refine_by_hash<F>(group: Vec<FileEntry>, hash_fn: F) -> Vec<Vec<FileEntry>>
where
    F: Fn(&FileEntry) -> Option<String> + Sync,
{
    let mut map: HashMap<String, Vec<FileEntry>> = HashMap::new();
    for e in group {
        match hash_fn(&e) {
            Some(h) => map.entry(h).or_default().push(e),
            None => { /* unreadable file: drop from candidates, caller logs issue */ }
        }
    }
    let mut groups: Vec<Vec<FileEntry>> =
        map.into_values().filter(|g| g.len() > 1).collect();
    groups.sort_by(|a, b| a[0].path.cmp(&b[0].path));
    groups
}

/// Split a hash-identical group into clusters that are *actually* byte-identical.
/// Defends against hash collisions. Each returned cluster is mutually identical.
pub fn verify_paranoid(group: Vec<FileEntry>) -> Vec<Vec<FileEntry>> {
    let mut clusters: Vec<Vec<FileEntry>> = Vec::new();
    'outer: for e in group {
        for cluster in clusters.iter_mut() {
            // Compare against the cluster representative.
            match bytes_equal(&cluster[0].path, &e.path) {
                Ok(true) => {
                    cluster.push(e);
                    continue 'outer;
                }
                Ok(false) => continue,
                // On read error, treat as its own cluster (conservative).
                Err(_) => continue,
            }
        }
        clusters.push(vec![e]);
    }
    clusters.into_iter().filter(|c| c.len() > 1).collect()
}

/// Build a [`DupSet`] from a confirmed-duplicate group, computing hardlink flags
/// and hardlink-aware reclaimable space.
pub fn build_set(id: u64, mode: MatchMode, hash: Option<String>, group: Vec<FileEntry>) -> DupSet {
    let zero_byte = group.iter().all(|e| e.size == 0);

    // Identify distinct physical copies via filesystem identity (`same_file::Handle`
    // compares inode+device on Unix and file-index+volume on Windows). Members
    // sharing an identity are hardlinks: only the first counts toward reclaimable
    // space.
    let mut seen_handles: HashMap<Handle, ()> = HashMap::new();
    let mut members = Vec::with_capacity(group.len());
    let mut distinct_physical: Vec<u64> = Vec::new(); // sizes of unique extents

    for e in group {
        let mut is_hardlink_of_other = false;
        match Handle::from_path(&e.path) {
            Ok(h) => {
                if seen_handles.contains_key(&h) {
                    is_hardlink_of_other = true;
                } else {
                    seen_handles.insert(h, ());
                    distinct_physical.push(e.size);
                }
            }
            // Unreadable identity ⇒ conservatively assume a distinct copy.
            Err(_) => distinct_physical.push(e.size),
        }
        members.push(DupMember { entry: e, role: MemberRole::Keep, is_hardlink_of_other });
    }

    // Default selection: keep the first, mark the rest Remove (UI can override).
    for (i, m) in members.iter_mut().enumerate() {
        m.role = if i == 0 { MemberRole::Keep } else { MemberRole::Remove };
    }

    // Reclaimable = sum of all distinct physical copies except the largest-kept one.
    // For exact dupes every copy is the same size, so this is size*(n_distinct-1).
    let reclaimable = if zero_byte || distinct_physical.len() <= 1 {
        0
    } else {
        let total: u64 = distinct_physical.iter().sum();
        let keep_one = *distinct_physical.iter().max().unwrap_or(&0);
        total - keep_one
    };

    DupSet { id, mode, members, hash, similarity: None, reclaimable, zero_byte }
}

/// Run the full exact-match scan. Honours pause/cancel and reports progress.
pub fn run_exact(
    config: &ScanConfig,
    control: &ScanControl,
    progress: &dyn ProgressSink,
) -> ScanResult {
    let start = Instant::now();
    let mut issues = Vec::new();

    // ── Phase 1: discovery ────────────────────────────────────────────────
    progress.report(&Progress {
        phase: Phase::Discovering,
        files_done: 0,
        files_total: 0,
        bytes_done: 0,
        bytes_total: 0,
        current_path: None,
        dup_sets: 0,
        reclaimable: 0,
        elapsed_secs: start.elapsed().as_secs_f64(),
    });
    let (entries, disc_issues) =
        discovery::discover(&config.roots, &config.excludes, config.symlinks, control);
    issues.extend(disc_issues);
    let files_total = entries.len() as u64;
    let bytes_total: u64 = entries.iter().map(|e| e.size).sum();

    if control.is_cancelled() {
        return cancelled_result(issues, files_total, bytes_total, start);
    }

    // ── Phase 2: size grouping ────────────────────────────────────────────
    report(progress, Phase::GroupingBySize, 0, files_total, 0, bytes_total, None, 0, 0, start);
    let size_groups = group_by_size(entries);

    // ── Phase 3: partial hashing ──────────────────────────────────────────
    let issues = Mutex::new(issues);
    let done = AtomicU64::new(0);
    let bytes_done = AtomicU64::new(0);
    let algo = config.hash_algo;

    let partial_groups: Vec<Vec<FileEntry>> = size_groups
        .into_par_iter()
        .flat_map_iter(|group| {
            if control.is_cancelled() {
                return Vec::new().into_iter();
            }
            // Zero-byte groups need no hashing — they're already identical.
            if group.first().map(|e| e.size == 0).unwrap_or(false) {
                done.fetch_add(group.len() as u64, Ordering::Relaxed);
                return vec![group].into_iter();
            }
            let refined = refine_by_hash(group, |e| {
                if !control.checkpoint() {
                    return None;
                }
                report_tick(progress, Phase::PartialHashing, &done, files_total, &bytes_done, bytes_total, &e.path, start);
                match partial_hash(&e.path, e.size, algo) {
                    Ok(h) => {
                        done.fetch_add(1, Ordering::Relaxed);
                        Some(h)
                    }
                    Err(err) => {
                        issues.lock().unwrap().push(ScanIssue {
                            path: e.path.clone(),
                            message: format!("partial hash failed: {err}"),
                        });
                        None
                    }
                }
            });
            refined.into_iter()
        })
        .collect();

    if control.is_cancelled() {
        let issues = issues.into_inner().unwrap();
        return cancelled_result(issues, files_total, bytes_total, start);
    }

    // ── Phase 4: full hashing ─────────────────────────────────────────────
    let full_groups: Vec<(String, Vec<FileEntry>)> = partial_groups
        .into_par_iter()
        .flat_map_iter(|group| {
            if control.is_cancelled() {
                return Vec::new().into_iter();
            }
            if group.first().map(|e| e.size == 0).unwrap_or(false) {
                return vec![("zero".to_string(), group)].into_iter();
            }
            // Track each entry's full hash so we can label the resulting set.
            let hashes: Mutex<HashMap<PathBuf, String>> = Mutex::new(HashMap::new());
            let refined = refine_by_hash(group, |e| {
                if !control.checkpoint() {
                    return None;
                }
                report_tick(progress, Phase::FullHashing, &done, files_total, &bytes_done, bytes_total, &e.path, start);
                match full_hash(&e.path, algo) {
                    Ok(h) => {
                        bytes_done.fetch_add(e.size, Ordering::Relaxed);
                        hashes.lock().unwrap().insert(e.path.clone(), h.clone());
                        Some(h)
                    }
                    Err(err) => {
                        issues.lock().unwrap().push(ScanIssue {
                            path: e.path.clone(),
                            message: format!("full hash failed: {err}"),
                        });
                        None
                    }
                }
            });
            let hashes = hashes.into_inner().unwrap();
            refined
                .into_iter()
                .map(|g| {
                    let h = hashes.get(&g[0].path).cloned().unwrap_or_default();
                    (h, g)
                })
                .collect::<Vec<_>>()
                .into_iter()
        })
        .collect();

    if control.is_cancelled() {
        let issues = issues.into_inner().unwrap();
        return cancelled_result(issues, files_total, bytes_total, start);
    }

    // ── Phase 5: optional paranoid byte verification ──────────────────────
    let mut sets: Vec<DupSet> = Vec::new();
    let mut next_id: u64 = 0;
    for (hash, group) in full_groups {
        if control.is_cancelled() {
            break;
        }
        let confirmed: Vec<(Option<String>, Vec<FileEntry>)> = if config.paranoid
            && group.first().map(|e| e.size > 0).unwrap_or(false)
        {
            report(progress, Phase::Verifying, done.load(Ordering::Relaxed), files_total, bytes_done.load(Ordering::Relaxed), bytes_total, Some(group[0].path.clone()), sets.len() as u64, 0, start);
            verify_paranoid(group).into_iter().map(|c| (Some(hash.clone()), c)).collect()
        } else {
            vec![(Some(hash), group)]
        };
        for (h, g) in confirmed {
            if g.len() > 1 {
                let set = build_set(next_id, MatchMode::Exact, h, g);
                next_id += 1;
                sets.push(set);
            }
        }
    }

    // Sort sets by reclaimable space, descending (UI default).
    sets.sort_by(|a, b| b.reclaimable.cmp(&a.reclaimable));
    let reclaimable_total: u64 = sets.iter().map(|s| s.reclaimable).sum();

    report(progress, Phase::Done, files_total, files_total, bytes_total, bytes_total, None, sets.len() as u64, reclaimable_total, start);

    ScanResult {
        sets,
        issues: issues.into_inner().unwrap(),
        files_scanned: files_total,
        bytes_scanned: bytes_total,
        elapsed_secs: start.elapsed().as_secs_f64(),
        cancelled: false,
    }
}

#[allow(clippy::too_many_arguments)]
fn report(
    progress: &dyn ProgressSink,
    phase: Phase,
    files_done: u64,
    files_total: u64,
    bytes_done: u64,
    bytes_total: u64,
    current_path: Option<PathBuf>,
    dup_sets: u64,
    reclaimable: u64,
    start: Instant,
) {
    progress.report(&Progress {
        phase,
        files_done,
        files_total,
        bytes_done,
        bytes_total,
        current_path,
        dup_sets,
        reclaimable,
        elapsed_secs: start.elapsed().as_secs_f64(),
    });
}

#[allow(clippy::too_many_arguments)]
fn report_tick(
    progress: &dyn ProgressSink,
    phase: Phase,
    done: &AtomicU64,
    files_total: u64,
    bytes_done: &AtomicU64,
    bytes_total: u64,
    path: &std::path::Path,
    start: Instant,
) {
    report(
        progress,
        phase,
        done.load(Ordering::Relaxed),
        files_total,
        bytes_done.load(Ordering::Relaxed),
        bytes_total,
        Some(path.to_path_buf()),
        0,
        0,
        start,
    );
}

fn cancelled_result(
    issues: Vec<ScanIssue>,
    files_total: u64,
    bytes_total: u64,
    start: Instant,
) -> ScanResult {
    ScanResult {
        sets: Vec::new(),
        issues,
        files_scanned: files_total,
        bytes_scanned: bytes_total,
        elapsed_secs: start.elapsed().as_secs_f64(),
        cancelled: true,
    }
}
