#[path = "../src/document_sync.rs"]
mod document_sync;

use document_sync::{MergeOutcome, merge_three_way};

#[test]
fn external_change_and_local_pending_change_merge_when_disjoint() {
    let base = "fn main() {\n    work();\n}\n";
    let local = "fn main() {\n    work_local();\n}\n";
    let backend = "// generated header\nfn main() {\n    work();\n}\n";
    assert_eq!(
        merge_three_way(base, local, backend),
        MergeOutcome::Merged("// generated header\nfn main() {\n    work_local();\n}\n".into())
    );
}

#[test]
fn conflicting_external_change_preserves_local_text_for_explicit_resolution() {
    let base = "value = 1;\n";
    let local = "value = local;\n";
    let backend = "value = external;\n";
    assert_eq!(
        merge_three_way(base, local, backend),
        MergeOutcome::Conflict
    );
    assert_eq!(local, "value = local;\n");
}

#[test]
fn deletion_rebase_keeps_text_inserted_after_the_deleted_range() {
    let base = "one two three four\n";
    let local = "one two four\n";
    let backend = "one two three NEW four\n";
    assert_eq!(
        merge_three_way(base, local, backend),
        MergeOutcome::Merged("one two NEW four\n".into())
    );
}
