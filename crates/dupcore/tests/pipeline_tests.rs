//! Tests for the exact-match pipeline: size grouping, partial vs full hashing,
//! hardlink awareness, and paranoid byte-verification catching a forced collision.

use dupcore::hashing::{full_hash, partial_hash};
use dupcore::identity::file_id;
use dupcore::model::*;
use dupcore::pipeline::{build_set, group_by_size, refine_by_hash, verify_paranoid};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Build a FileEntry from a real file on disk.
fn entry(path: &Path, root: &Path) -> FileEntry {
    let meta = fs::metadata(path).unwrap();
    FileEntry {
        path: path.to_path_buf(),
        root: root.to_path_buf(),
        size: meta.len(),
        mtime: 0,
        identity: file_id(path),
        video: None,
    }
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn size_grouping_keeps_only_collisions() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let a = write(root, "a.bin", b"hello world");
    let b = write(root, "b.bin", b"HELLO WORLD"); // same length, different bytes
    let c = write(root, "c.bin", b"different length entirely");

    let entries = vec![entry(&a, root), entry(&b, root), entry(&c, root)];
    let groups = group_by_size(entries);

    // a and b share a size (11 bytes); c is unique → exactly one group of two.
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].len(), 2);
}

#[test]
fn partial_hash_separates_files_differing_at_start() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    // Two big same-size files that differ only in the first byte.
    let mut data1 = vec![0u8; 2 * 1024 * 1024];
    let mut data2 = data1.clone();
    data1[0] = 1;
    data2[0] = 2;
    let a = write(root, "a.bin", &data1);
    let b = write(root, "b.bin", &data2);

    let ha = partial_hash(&a, data1.len() as u64, HashAlgo::Blake3).unwrap();
    let hb = partial_hash(&b, data2.len() as u64, HashAlgo::Blake3).unwrap();
    assert_ne!(ha, hb, "partial hash should catch a head-byte difference cheaply");
}

#[test]
fn full_hash_matches_identical_and_differs_otherwise() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let a = write(root, "a.bin", b"identical content here");
    let b = write(root, "b.bin", b"identical content here");
    let c = write(root, "c.bin", b"identical content HERE");

    let ha = full_hash(&a, HashAlgo::Blake3).unwrap();
    let hb = full_hash(&b, HashAlgo::Blake3).unwrap();
    let hc = full_hash(&c, HashAlgo::Blake3).unwrap();
    assert_eq!(ha, hb);
    assert_ne!(ha, hc);
}

#[test]
fn refine_by_hash_groups_identical_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let a = write(root, "a.bin", b"dup");
    let b = write(root, "b.bin", b"dup");
    let c = write(root, "c.bin", b"dup");
    let group = vec![entry(&a, root), entry(&b, root), entry(&c, root)];

    let refined = refine_by_hash(group, |e| full_hash(&e.path, HashAlgo::Blake3).ok());
    assert_eq!(refined.len(), 1);
    assert_eq!(refined[0].len(), 3);
}

#[test]
#[cfg(unix)]
fn hardlinks_are_grouped_but_not_reclaimable() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let content = b"shared bytes across hardlink and an independent copy";
    let a = write(root, "a.bin", content);
    // b is a hardlink to a (same inode).
    let b = root.join("b.bin");
    fs::hard_link(&a, &b).unwrap();
    // c is an independent file with identical content (distinct inode).
    let c = write(root, "c.bin", content);

    let group = vec![entry(&a, root), entry(&b, root), entry(&c, root)];
    let set = build_set(0, MatchMode::Exact, Some("h".into()), group);

    // All three are reported in the set.
    assert_eq!(set.members.len(), 3);
    // Exactly one member is flagged as a hardlink of another.
    let hardlinks = set.members.iter().filter(|m| m.is_hardlink_of_other).count();
    assert_eq!(hardlinks, 1, "b should be detected as a hardlink of a");
    // Two distinct physical copies (a/b share one extent, c is separate) ⇒
    // reclaimable equals exactly one file's size, not two.
    assert_eq!(set.reclaimable, content.len() as u64);
}

#[test]
fn paranoid_verification_splits_a_forced_hash_collision() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    // Two same-size files with DIFFERENT content...
    let a = write(root, "a.bin", b"AAAAAAAAAAAA");
    let b = write(root, "b.bin", b"BBBBBBBBBBBB");
    let group = vec![entry(&a, root), entry(&b, root)];

    // ...forced into one group by a deliberately colliding hash function.
    let collided = refine_by_hash(group, |_e| Some("same-hash-for-everyone".to_string()));
    assert_eq!(collided.len(), 1);
    assert_eq!(collided[0].len(), 2, "collision puts both files in one group");

    // Paranoid byte-for-byte comparison must separate them: no real duplicates.
    let verified = verify_paranoid(collided.into_iter().next().unwrap());
    assert!(
        verified.is_empty(),
        "byte verification must reject the false collision (no cluster of >1)"
    );
}

#[test]
fn paranoid_verification_keeps_truly_identical_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let a = write(root, "a.bin", b"truly identical");
    let b = write(root, "b.bin", b"truly identical");
    let group = vec![entry(&a, root), entry(&b, root)];
    let verified = verify_paranoid(group);
    assert_eq!(verified.len(), 1);
    assert_eq!(verified[0].len(), 2);
}

#[test]
fn zero_byte_set_reclaims_nothing() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let a = write(root, "a.empty", b"");
    let b = write(root, "b.empty", b"");
    let set = build_set(0, MatchMode::Exact, None, vec![entry(&a, root), entry(&b, root)]);
    assert!(set.zero_byte);
    assert_eq!(set.reclaimable, 0);
}
