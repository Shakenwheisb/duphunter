//! Tests for action safety and dry-run behavior.

use dupcore::actions::execute;
use dupcore::model::*;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn target(set_id: u64, keep: PathBuf, remove: Vec<PathBuf>) -> ActionTarget {
    ActionTarget { set_id, keep, remove }
}

#[test]
fn dry_run_touches_nothing_and_previews_everything() {
    let dir = tempdir().unwrap();
    let keep = dir.path().join("keep.bin");
    let dup = dir.path().join("dup.bin");
    fs::write(&keep, b"data").unwrap();
    fs::write(&dup, b"data").unwrap();

    let plan = ActionPlan {
        kind: ActionKind::PermanentDelete,
        dry_run: true,
        quarantine_dir: None,
        targets: vec![target(0, keep.clone(), vec![dup.clone()])],
    };
    let report = execute(&plan, dir.path()).unwrap();

    assert!(report.dry_run);
    assert_eq!(report.total_files, 1);
    // Nothing actually deleted.
    assert!(dup.exists(), "dry run must not delete files");
    assert!(report.manifest_path.is_none(), "dry run writes no manifest");
}

#[test]
fn refuses_to_remove_the_keeper() {
    let dir = tempdir().unwrap();
    let keep = dir.path().join("keep.bin");
    fs::write(&keep, b"data").unwrap();

    // Malformed plan: keeper also listed for removal.
    let plan = ActionPlan {
        kind: ActionKind::PermanentDelete,
        dry_run: false,
        quarantine_dir: None,
        targets: vec![target(0, keep.clone(), vec![keep.clone()])],
    };
    let err = execute(&plan, dir.path());
    assert!(err.is_err(), "removing the keeper must be refused");
    assert!(keep.exists(), "keeper must survive a refused plan");
}

#[test]
fn quarantine_moves_file_and_writes_manifest() {
    let dir = tempdir().unwrap();
    let keep = dir.path().join("keep.bin");
    let dup = dir.path().join("dup.bin");
    fs::write(&keep, b"payload").unwrap();
    fs::write(&dup, b"payload").unwrap();
    let qdir = dir.path().join("quarantine");
    let mdir = dir.path().join("manifests");

    let plan = ActionPlan {
        kind: ActionKind::Quarantine,
        dry_run: false,
        quarantine_dir: Some(qdir.clone()),
        targets: vec![target(0, keep.clone(), vec![dup.clone()])],
    };
    let report = execute(&plan, &mdir).unwrap();

    assert_eq!(report.total_files, 1);
    assert!(!dup.exists(), "duplicate moved out of source");
    assert!(qdir.join("dup.bin").exists(), "duplicate landed in quarantine");
    assert!(keep.exists(), "keeper untouched");
    let manifest = report.manifest_path.expect("manifest written");
    assert!(manifest.exists());
}
