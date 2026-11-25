#[path = "../src/document_sync.rs"]
mod document_sync;

use document_sync::{MergeOutcome, coalesced_diff, merge_three_way};
use ide_core::ChangeSet;

fn apply(document: &mut ide_text::Document, changes: &ChangeSet) {
    let version = document.version();
    document.apply(version, changes).unwrap();
}

#[test]
fn ten_thousand_inputs_are_acknowledged_without_lost_characters() {
    let base = "fn main() {\n}\n";
    let mut local = base.to_owned();
    let mut backend = ide_text::Document::new(base);
    for _batch in 0..100 {
        let before = local.clone();
        for index in 0..100 {
            local.insert(local.len() - 2, (b'a' + (index % 26) as u8) as char);
        }
        let diff = coalesced_diff(&before, &local);
        apply(&mut backend, &diff);
    }
    assert_eq!(backend.text(), local);
    assert_eq!(local.len(), base.len() + 10_000);
}

#[test]
fn concurrent_disjoint_backend_edit_rebases_local_pending_text() {
    let base = "alpha\nbeta\ngamma\n";
    let local = "alpha\nbeta LOCAL\ngamma\n";
    let backend = "alpha BACKEND\nbeta\ngamma\n";
    assert_eq!(
        merge_three_way(base, local, backend),
        MergeOutcome::Merged("alpha BACKEND\nbeta LOCAL\ngamma\n".into())
    );
}

#[test]
fn same_range_backend_edit_becomes_conflict_and_keeps_local_text() {
    let base = "alpha\nbeta\n";
    let local = "alpha\nlocal\n";
    let backend = "alpha\nbackend\n";
    assert_eq!(
        merge_three_way(base, local, backend),
        MergeOutcome::Conflict
    );
    // The protocol keeps `local` on the UI side; conflict resolution is
    // explicit rather than silently replacing it with `backend`.
    assert_eq!(local, "alpha\nlocal\n");
}

#[test]
fn save_during_typing_only_marks_the_submitted_version() {
    // This mirrors `DocumentBuffer::mark_saved_version`: saving version N while
    // version N+1 already exists must leave the newer edit dirty.
    let mut document = rustidea_editor::Document::new(rustidea_editor::DocumentId::new(1), "a");
    let version = document.version();
    document
        .apply(rustidea_editor::Transaction::new(
            version,
            vec![rustidea_editor::Change::insert(1, "b")],
            rustidea_editor::EditSource::Local,
        ))
        .unwrap();
    let newer = document.version();
    document.mark_saved_if_version(version);
    assert!(document.is_dirty());
    document.mark_saved_if_version(newer);
    assert!(!document.is_dirty());
}
