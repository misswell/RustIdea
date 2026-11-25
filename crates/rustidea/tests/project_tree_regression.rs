#[allow(dead_code)]
#[path = "../src/project_tree.rs"]
mod project_tree;

use ide_core::IpcPath;
use ide_vfs::{EntryKind, FileEntry};
use project_tree::{ProjectTree, Toggle};
use std::path::{Path, PathBuf};

fn file(path: &str) -> FileEntry {
    FileEntry {
        name: path.rsplit('/').next().unwrap_or(path).into(),
        path: IpcPath(PathBuf::from(path)),
        kind: EntryKind::File,
        target_kind: None,
        size: 0,
        mtime_ms: 0.0,
        hidden: false,
        ignored: false,
        excluded: false,
    }
}

#[test]
fn one_hundred_thousand_files_stay_paged_and_reachable() {
    let root = PathBuf::from("/project");
    let mut tree = ProjectTree::new(root.clone());
    let entries: Vec<_> = (0..100_000)
        .map(|index| file(&format!("/project/file-{index:05}.txt")))
        .collect();
    tree.apply_directory(root, entries);
    assert_eq!(tree.nodes().len(), 2_001);
    assert!(tree.nodes().last().unwrap().more);
    for _ in 0..49 {
        let last = tree.nodes().len().saturating_sub(1);
        assert!(matches!(tree.toggle(last), Toggle::Rebuilt));
    }
    assert_eq!(tree.nodes().len(), 100_000);
    assert!(tree.nodes().last().is_some_and(|node| !node.more));
}

#[test]
fn stale_directory_request_cannot_overwrite_new_generation() {
    let mut tree = ProjectTree::new(PathBuf::from("/project"));
    let old = tree.request_load(Path::new("/project")).unwrap();
    assert!(
        tree.request_load(Path::new("/project")).is_none(),
        "duplicate in-flight requests must be dropped"
    );
    assert!(!tree.apply_directory_if_current(
        PathBuf::from("/project"),
        old + 7,
        vec![file("/project/stale.txt")]
    ));
    assert!(tree.apply_directory_if_current(
        PathBuf::from("/project"),
        old,
        vec![file("/project/current.txt")]
    ));
    assert!(tree.nodes().iter().any(|node| node.name == "current.txt"));
}
