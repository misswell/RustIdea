use rustidea_editor::{Change, Document, DocumentId, EditSource, Transaction};

fn local(document: &mut Document, change: Change) {
    let version = document.version();
    document
        .apply(Transaction::new(version, vec![change], EditSource::Local))
        .unwrap();
}

#[test]
fn external_edits_do_not_corrupt_local_undo_history() {
    let mut document = Document::new(DocumentId::new(1), "one");
    local(&mut document, Change::insert(3, " local"));
    let version = document.version();
    document
        .apply(Transaction::new(
            version,
            vec![Change::insert(0, "external ")],
            EditSource::External,
        ))
        .unwrap();
    assert_eq!(document.text(), "external one local");
    assert!(document.undo().unwrap());
    assert_eq!(document.text(), "external one");
    assert!(document.undo().is_err() || document.text() == "external one");
}

#[test]
fn undo_and_redo_stay_identical_for_multibyte_text() {
    let mut document = Document::new(DocumentId::new(2), "中文😀");
    local(&mut document, Change::insert(0, "前"));
    assert_eq!(document.text(), "前中文😀");
    assert!(document.undo().unwrap());
    assert_eq!(document.text(), "中文😀");
    assert!(document.redo().unwrap());
    assert_eq!(document.text(), "前中文😀");
}
