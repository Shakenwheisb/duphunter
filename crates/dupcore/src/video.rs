//! Video near-duplicate detection (a mode strictly separate from exact match).
//!
//! Pipeline per file:
//!   1. `ffprobe` → duration, resolution, codecs, bitrate ([`VideoMeta`]).
//!   2. `ffmpeg` samples N frames at evenly spaced percentage offsets, each scaled
//!      to a 9×8 grayscale buffer piped to stdout, from which we compute a 64-bit
//!      **dHash** (difference hash). The ordered list of frame hashes is the
//!      video's perceptual **fingerprint**.
//!
//! Two videos are scored by combining:
//!   * visual similarity — best-aligned Hamming distance over frame hashes
//!     (a small alignment shift tolerates trims / different lengths);
//!   * duration ratio;
//!   * normalized filename similarity.
//!
//! Files whose combined score ≥ the user threshold are clustered into near-dup
//! sets. This is heuristic — clearly distinct from byte-exact matching.

use crate::control::{ProgressSink, ScanControl};
use crate::error::{DupError, Result};
use crate::model::*;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

/// dHash grayscale sample dimensions: 9 wide × 8 tall → 8×8 = 64 comparison bits.
const HASH_W: u32 = 9;
const HASH_H: u32 = 8;

/// Are both ffmpeg and ffprobe available on PATH?
pub fn ffmpeg_available() -> bool {
    tool_ok("ffprobe") && tool_ok("ffmpeg")
}

fn tool_ok(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Probe container/stream metadata via ffprobe.
pub fn probe(path: &Path) -> Result<VideoMeta> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|_| DupError::FfmpegMissing)?;
    if !out.status.success() {
        return Err(DupError::Ffmpeg(format!("ffprobe failed for {path:?}")));
    }
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| DupError::Ffmpeg(e.to_string()))?;

    let mut meta = VideoMeta::default();
    if let Some(fmt) = json.get("format") {
        meta.duration_secs = fmt
            .get("duration")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        meta.bitrate = fmt
            .get("bit_rate")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
    }
    if let Some(streams) = json.get("streams").and_then(|v| v.as_array()) {
        for s in streams {
            match s.get("codec_type").and_then(|v| v.as_str()) {
                Some("video") if meta.video_codec.is_empty() => {
                    meta.video_codec =
                        s.get("codec_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    meta.width = s.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    meta.height = s.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                }
                Some("audio") if meta.audio_codec.is_empty() => {
                    meta.audio_codec =
                        s.get("codec_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                }
                _ => {}
            }
        }
    }
    Ok(meta)
}

/// Compute the perceptual fingerprint: one 64-bit dHash per sampled frame.
pub fn fingerprint(path: &Path, duration_secs: f64, samples: u8) -> Result<Vec<u64>> {
    let samples = samples.max(1);
    let mut hashes = Vec::with_capacity(samples as usize);
    for i in 0..samples {
        // Sample at evenly spaced interior percentages (e.g. 10,30,50,70,90%).
        let frac = (i as f64 + 0.5) / samples as f64;
        let ts = (duration_secs * frac).max(0.0);
        match sample_frame_hash(path, ts) {
            Ok(h) => hashes.push(h),
            // A failed frame contributes a neutral 0; the others still inform.
            Err(_) => hashes.push(0),
        }
    }
    Ok(hashes)
}

/// Extract one frame at `ts` seconds, scaled to 9×8 grayscale raw bytes, and
/// compute its dHash.
fn sample_frame_hash(path: &Path, ts: f64) -> Result<u64> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &format!("{ts:.3}")])
        .arg("-i")
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={HASH_W}:{HASH_H},format=gray"),
            "-f",
            "rawvideo",
            "-",
        ])
        .stderr(Stdio::null())
        .output()
        .map_err(|_| DupError::FfmpegMissing)?;

    let buf = out.stdout;
    let expected = (HASH_W * HASH_H) as usize;
    if buf.len() < expected {
        return Err(DupError::Ffmpeg("short frame buffer".into()));
    }
    Ok(dhash(&buf))
}

/// Difference hash over a `HASH_W`×`HASH_H` grayscale buffer (row-major):
/// for each row compare horizontally adjacent pixels → 1 bit each → 64 bits.
fn dhash(buf: &[u8]) -> u64 {
    let mut bits: u64 = 0;
    let mut idx = 0;
    for row in 0..HASH_H {
        let base = (row * HASH_W) as usize;
        for col in 0..(HASH_W - 1) {
            let left = buf[base + col as usize];
            let right = buf[base + col as usize + 1];
            if left < right {
                bits |= 1 << idx;
            }
            idx += 1;
        }
    }
    bits
}

/// Serialize a fingerprint for the cache.
pub fn fingerprint_to_string(fp: &[u64]) -> String {
    fp.iter().map(|h| format!("{h:016x}")).collect::<Vec<_>>().join(",")
}

/// Parse a cached fingerprint.
pub fn fingerprint_from_string(s: &str) -> Vec<u64> {
    s.split(',').filter_map(|p| u64::from_str_radix(p, 16).ok()).collect()
}

/// Visual similarity (0..=100) between two fingerprints, allowing a ±1 frame
/// alignment shift so trimmed copies still line up.
pub fn visual_similarity(a: &[u64], b: &[u64]) -> u8 {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let mut best = 0u32;
    for shift in -1i32..=1 {
        let mut total_bits = 0u32;
        let mut matching = 0u32;
        for (i, &ha) in a.iter().enumerate() {
            let j = i as i32 + shift;
            if j < 0 || j as usize >= b.len() {
                continue;
            }
            let hb = b[j as usize];
            let dist = (ha ^ hb).count_ones();
            matching += 64 - dist;
            total_bits += 64;
        }
        if total_bits > 0 {
            let pct = matching * 100 / total_bits;
            best = best.max(pct);
        }
    }
    best.min(100) as u8
}

/// Duration similarity (0..=100): ratio of shorter to longer.
fn duration_similarity(a: f64, b: f64) -> u8 {
    if a <= 0.0 || b <= 0.0 {
        return 0;
    }
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    ((lo / hi) * 100.0).round() as u8
}

/// Normalized filename similarity (0..=100) ignoring extension and case.
fn name_similarity(a: &Path, b: &Path) -> u8 {
    let na = norm_name(a);
    let nb = norm_name(b);
    (strsim::normalized_levenshtein(&na, &nb) * 100.0).round() as u8
}

fn norm_name(p: &Path) -> String {
    p.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Combined near-dup score for two files (0..=100). Visual is primary; duration
/// and filename corroborate.
pub fn combined_score(a: &FileEntry, fa: &[u64], b: &FileEntry, fb: &[u64]) -> u8 {
    let visual = visual_similarity(fa, fb) as f64;
    let (da, db) = (
        a.video.as_ref().map(|m| m.duration_secs).unwrap_or(0.0),
        b.video.as_ref().map(|m| m.duration_secs).unwrap_or(0.0),
    );
    let dur = duration_similarity(da, db) as f64;
    let name = name_similarity(&a.path, &b.path) as f64;
    (visual * 0.75 + dur * 0.15 + name * 0.10).round().min(100.0) as u8
}

/// Cluster entries (with precomputed fingerprints) into near-dup sets using a
/// simple union-find over pairs scoring ≥ threshold.
pub fn cluster(
    entries: Vec<(FileEntry, Vec<u64>)>,
    threshold: u8,
) -> Vec<DupSet> {
    let n = entries.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut score_sum = vec![0u32; n];
    let mut score_cnt = vec![0u32; n];

    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }

    for i in 0..n {
        for j in (i + 1)..n {
            let s = combined_score(&entries[i].0, &entries[i].1, &entries[j].0, &entries[j].1);
            if s >= threshold {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    parent[ri] = rj;
                }
                score_sum[i] += s as u32;
                score_cnt[i] += 1;
                score_sum[j] += s as u32;
                score_cnt[j] += 1;
            }
        }
    }

    // Bucket entries by cluster root.
    use std::collections::HashMap;
    let mut buckets: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        buckets.entry(r).or_default().push(i);
    }

    let mut sets = Vec::new();
    let mut id = 0u64;
    for (_root, idxs) in buckets {
        if idxs.len() < 2 {
            continue;
        }
        let mut total_score = 0u32;
        let mut total_cnt = 0u32;
        let mut members = Vec::new();
        let mut reclaimable = 0u64;
        // Keep the longest-duration / largest file; reclaim the rest.
        let mut sorted = idxs.clone();
        sorted.sort_by(|&x, &y| entries[y].0.size.cmp(&entries[x].0.size));
        for (k, &i) in sorted.iter().enumerate() {
            total_score += score_sum[i];
            total_cnt += score_cnt[i];
            let role = if k == 0 { MemberRole::Keep } else { MemberRole::Remove };
            if k > 0 {
                reclaimable += entries[i].0.size;
            }
            members.push(DupMember {
                entry: entries[i].0.clone(),
                role,
                is_hardlink_of_other: false,
            });
        }
        let similarity = if total_cnt > 0 { (total_score / total_cnt) as u8 } else { threshold };
        sets.push(DupSet {
            id,
            mode: MatchMode::VideoNearDup,
            members,
            hash: None,
            similarity: Some(similarity),
            reclaimable,
            zero_byte: false,
        });
        id += 1;
    }
    sets.sort_by(|a, b| b.reclaimable.cmp(&a.reclaimable));
    sets
}

/// Run the full video near-dup scan.
pub fn run_video(
    config: &ScanConfig,
    control: &ScanControl,
    progress: &dyn ProgressSink,
) -> Result<ScanResult> {
    if !ffmpeg_available() {
        return Err(DupError::FfmpegMissing);
    }
    let start = Instant::now();
    let mut issues = Vec::new();

    // Discovery.
    progress.report(&prog(Phase::Discovering, 0, 0, start));
    let (entries, disc_issues) =
        crate::discovery::discover(&config.roots, &config.excludes, config.symlinks, control);
    issues.extend(disc_issues);
    let files_total = entries.len() as u64;
    let bytes_total: u64 = entries.iter().map(|e| e.size).sum();

    // Probe + fingerprint each video sequentially (ffmpeg already uses threads).
    let mut prepared: Vec<(FileEntry, Vec<u64>)> = Vec::new();
    for (i, mut e) in entries.into_iter().enumerate() {
        if !control.checkpoint() {
            return Ok(ScanResult {
                sets: Vec::new(),
                issues,
                files_scanned: files_total,
                bytes_scanned: bytes_total,
                elapsed_secs: start.elapsed().as_secs_f64(),
                cancelled: true,
            });
        }
        progress.report(&prog_path(Phase::Probing, i as u64, files_total, &e.path, start));
        let meta = match probe(&e.path) {
            Ok(m) => m,
            Err(err) => {
                issues.push(ScanIssue { path: e.path.clone(), message: err.to_string() });
                continue;
            }
        };
        progress.report(&prog_path(Phase::SamplingFrames, i as u64, files_total, &e.path, start));
        let fp = fingerprint(&e.path, meta.duration_secs, config.video.frame_samples)
            .unwrap_or_default();
        e.video = Some(meta);
        prepared.push((e, fp));
    }

    progress.report(&prog(Phase::Clustering, files_total, files_total, start));
    let sets = cluster(prepared, config.video.similarity_threshold);

    progress.report(&prog(Phase::Done, files_total, files_total, start));
    Ok(ScanResult {
        sets,
        issues,
        files_scanned: files_total,
        bytes_scanned: bytes_total,
        elapsed_secs: start.elapsed().as_secs_f64(),
        cancelled: false,
    })
}

fn prog(phase: Phase, done: u64, total: u64, start: Instant) -> Progress {
    Progress {
        phase,
        files_done: done,
        files_total: total,
        bytes_done: 0,
        bytes_total: 0,
        current_path: None,
        dup_sets: 0,
        reclaimable: 0,
        elapsed_secs: start.elapsed().as_secs_f64(),
    }
}

fn prog_path(phase: Phase, done: u64, total: u64, path: &Path, start: Instant) -> Progress {
    let mut p = prog(phase, done, total, start);
    p.current_path = Some(path.to_path_buf());
    p
}
