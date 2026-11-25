#[allow(dead_code)]
#[path = "../src/document_store.rs"]
mod document_store;

use document_store::{DocumentKey, DocumentStore};
use ide_core::DocId;

#[test]
fn closing_the_last_view_releases_resources_exactly_once() {
    let doc = DocId(1);
    let key = DocumentKey::Path(std::path::PathBuf::from("/tmp/a.rs"));
    let mut store = DocumentStore::default();
    assert!(store.insert_view(key.clone(), doc, 1u8));
    assert!(store.acquire(&key).is_some());
    assert_eq!(store.release(&key), None);
    assert_eq!(store.release(&key), Some(1));
    assert!(store.release(&key).is_none(), "release must be idempotent");
}

#[test]
fn reopening_a_file_creates_new_identity_after_full_release() {
    let key = DocumentKey::Path(std::path::PathBuf::from("/tmp/b.rs"));
    let mut store = DocumentStore::default();
    store.insert_view(key.clone(), DocId(1), "first");
    store.release(&key);
    store.insert_view(key.clone(), DocId(2), "second");
    assert_eq!(store.get(&key), Some(&"second"));
    assert_eq!(store.get_by_doc(DocId(1)), None);
    assert_eq!(store.get_by_doc(DocId(2)), Some(&"second"));
}
