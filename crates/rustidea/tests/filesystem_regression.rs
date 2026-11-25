use ide_vfs::{EntryKind, read_dir};
use std::fs;

#[test]
fn project_listing_respects_gitignore_and_ignore_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join(".gitignore"), "ignored.txt\ntarget/\n").unwrap();
    fs::write(root.join(".ignore"), "secret.txt\n").unwrap();
    fs::write(root.join("visible.txt"), "ok").unwrap();
    fs::write(root.join("ignored.txt"), "gitignored").unwrap();
    fs::write(root.join("secret.txt"), "ignored by .ignore").unwrap();
    fs::create_dir(root.join("target")).unwrap();
    fs::write(root.join("target/out.txt"), "ignored dir").unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let entries = read_dir(root, &[]).unwrap();
    let visible = entries
        .iter()
        .find(|entry| entry.name == "visible.txt")
        .unwrap();
    assert!(!visible.ignored);
    assert!(
        entries
            .iter()
            .find(|entry| entry.name == "ignored.txt")
            .unwrap()
            .ignored
    );
    assert!(
        entries
            .iter()
            .find(|entry| entry.name == "secret.txt")
            .unwrap()
            .ignored
    );
    assert!(
        entries
            .iter()
            .find(|entry| entry.name == "target")
            .unwrap()
            .ignored
    );
    assert!(
        entries
            .iter()
            .take_while(|entry| matches!(entry.kind, EntryKind::Dir))
            .any(|entry| entry.name == "src")
    );
}

#[test]
fn symlink_entries_are_reported_without_following_into_loops() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("real")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root, root.join("real/loop")).unwrap();
    let entries = read_dir(root, &[]).unwrap();
    let real = entries.iter().find(|entry| entry.name == "real").unwrap();
    assert!(matches!(real.kind, EntryKind::Dir | EntryKind::Symlink));
}
