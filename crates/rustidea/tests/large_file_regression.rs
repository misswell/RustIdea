use rustidea_editor::{Change, Document, DocumentId, EditSource, Transaction};

#[test]
fn million_line_document_supports_edit_undo_and_viewport_reads() {
    let base = "fn generated() {}\n".repeat(1_000_000);
    let mut document = Document::new(DocumentId::new(1), &base);
    assert_eq!(document.line_count(), 1_000_001);
    let middle = document.line_start(500_000);
    document
        .apply(Transaction::new(
            document.version(),
            vec![Change::insert(middle, "// stress\n")],
            EditSource::Local,
        ))
        .unwrap();
    assert_eq!(document.line(500_000).as_deref(), Some("// stress"));
    for line in 500_010..500_070 {
        assert!(document.line(line).is_some());
    }
    assert!(document.undo().unwrap());
    assert_eq!(document.line_count(), 1_000_001);
}

#[test]
fn ultra_long_single_line_remains_addressable() {
    let text = "x".repeat(2_000_000);
    let mut document = Document::new(DocumentId::new(2), &text);
    assert_eq!(document.line_count(), 1);
    let end = document.line_end(0);
    document
        .apply(Transaction::new(
            document.version(),
            vec![Change::insert(end, " tail")],
            EditSource::Local,
        ))
        .unwrap();
    assert!(document.text().ends_with(" tail"));
}
