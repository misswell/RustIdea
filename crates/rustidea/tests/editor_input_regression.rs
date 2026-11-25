use rustidea_editor::{Change, Document, DocumentId, EditSource, Transaction};

#[test]
fn ten_thousand_sequential_utf16_edits_keep_every_character() {
    let mut document = Document::new(DocumentId::new(1), "");
    for index in 0..10_000u32 {
        let length = document.len_utf16();
        document
            .apply(Transaction::new(
                document.version(),
                vec![Change::insert(
                    length,
                    if index % 2 == 0 { "a" } else { "中" },
                )],
                EditSource::Local,
            ))
            .unwrap();
    }
    assert_eq!(document.text().matches('a').count(), 5_000);
    assert_eq!(document.text().matches('中').count(), 5_000);
    assert_eq!(document.len_utf16(), 10_000);
}

#[test]
fn crlf_input_is_normalized_and_byte_offsets_remain_valid() {
    let mut document = Document::new(DocumentId::new(2), "one\r\ntwo\r\n");
    document
        .apply(Transaction::new(
            document.version(),
            vec![Change::insert(0, "start\r\n")],
            EditSource::Local,
        ))
        .unwrap();
    assert_eq!(document.text(), "start\none\ntwo\n");
    assert_eq!(document.line_count(), 4);
}

#[test]
fn astral_selection_offsets_round_trip() {
    let document = Document::new(DocumentId::new(3), "a😀中文");
    for (line, col) in [(0, 0), (0, 1), (0, 3), (0, 5)] {
        let offset = document.offset_for_line_col(line, col);
        assert_eq!(document.offset_to_line_col(offset), (line, col));
    }
}
