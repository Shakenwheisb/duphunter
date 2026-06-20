//! Pure (no-ffmpeg) tests for the video near-dup scoring/clustering math, plus an
//! ffmpeg-gated end-to-end fingerprint test that only runs when ffmpeg is present.

use dupcore::model::*;
use dupcore::video::{
    cluster, ffmpeg_available, fingerprint, fingerprint_from_string, fingerprint_to_string, probe,
    visual_similarity,
};
use std::path::PathBuf;

fn entry(name: &str, size: u64, duration: f64) -> FileEntry {
    FileEntry {
        path: PathBuf::from(name),
        root: PathBuf::from("/root"),
        size,
        mtime: 0,
        identity: None,
        video: Some(VideoMeta { duration_secs: duration, ..Default::default() }),
    }
}

#[test]
fn visual_similarity_is_full_for_identical_fingerprints() {
    let fp = vec![0xDEAD_BEEF_u64, 0x0102_0304, 0xFFFF_0000];
    assert_eq!(visual_similarity(&fp, &fp), 100);
}

#[test]
fn visual_similarity_drops_for_different_fingerprints() {
    let a = vec![0x0000_0000_0000_0000_u64; 3];
    let b = vec![0xFFFF_FFFF_FFFF_FFFF_u64; 3];
    assert_eq!(visual_similarity(&a, &b), 0);
}

#[test]
fn fingerprint_round_trips_through_string() {
    let fp = vec![1u64, 0xABCD, u64::MAX];
    let s = fingerprint_to_string(&fp);
    assert_eq!(fingerprint_from_string(&s), fp);
}

#[test]
fn cluster_groups_similar_videos_across_formats() {
    // Same visual content, different container/length/name → should cluster.
    let fp_a = vec![0x1111_2222_3333_4444_u64, 0x5555_6666_7777_8888];
    let fp_b = fp_a.clone(); // re-encode preserves perceptual hash
    let a = entry("/movie.mp4", 100_000_000, 600.0);
    let b = entry("/movie.mkv", 70_000_000, 598.0);
    // An unrelated video.
    let fp_c = vec![0xFFFF_FFFF_FFFF_FFFF_u64, 0x0000_0000_0000_0000];
    let c = entry("/other.mp4", 50_000_000, 120.0);

    let sets = cluster(vec![(a, fp_a), (b, fp_b), (c, fp_c)], 88);
    assert_eq!(sets.len(), 1, "the two matching videos form one near-dup set");
    let set = &sets[0];
    assert_eq!(set.members.len(), 2);
    assert_eq!(set.mode, MatchMode::VideoNearDup);
    assert!(set.similarity.unwrap() >= 88);
    // Keeps the larger file; reclaims the smaller.
    assert_eq!(set.reclaimable, 70_000_000);
}

#[test]
fn cluster_below_threshold_makes_no_sets() {
    let a = entry("/a.mp4", 10, 60.0);
    let b = entry("/b.mp4", 10, 60.0);
    let fa = vec![0x0000_0000_0000_0000_u64];
    let fb = vec![0xFFFF_FFFF_FFFF_FFFF_u64];
    let sets = cluster(vec![(a, fa), (b, fb)], 88);
    assert!(sets.is_empty());
}

/// End-to-end: synthesize two tiny clips (same content, different codec) with
/// ffmpeg and confirm they fingerprint similarly. Skipped when ffmpeg is absent.
#[test]
fn ffmpeg_fingerprint_matches_reencode() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mp4 = dir.path().join("clip.mp4");
    let mkv = dir.path().join("clip.mkv");

    // Generate a 2s test pattern as mp4, then transcode to mkv.
    let gen = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i", "testsrc=duration=2:size=320x240:rate=10"])
        .arg(&mp4)
        .status()
        .unwrap();
    assert!(gen.success());
    let trans = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&mp4)
        .args(["-c:v", "libx264", "-crf", "30"])
        .arg(&mkv)
        .status()
        .unwrap();
    assert!(trans.success());

    let ma = probe(&mp4).unwrap();
    let mb = probe(&mkv).unwrap();
    let fa = fingerprint(&mp4, ma.duration_secs, 5).unwrap();
    let fb = fingerprint(&mkv, mb.duration_secs, 5).unwrap();
    let sim = visual_similarity(&fa, &fb);
    assert!(sim >= 80, "re-encoded clip should fingerprint similarly (got {sim})");
}
