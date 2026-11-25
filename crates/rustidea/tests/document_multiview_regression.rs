#[allow(dead_code)]
#[path = "../src/document_store.rs"]
mod document_store;

use document_store::{DocumentKey, DocumentStore};
use ide_core::DocId;

#[test]
fn one_file_has_one_handle_for_many_tabs_and_splits() {
    let doc = DocId(42);
    let key = DocumentKey::Path(std::path::PathBuf::from("/tmp/shared.rs"));
    let mut store = DocumentStore::default();
    assert!(store.insert_view(key.clone(), doc, "buffer"));
    assert_eq!(store.acquire(&key), Some("buffer"));
    assert_eq!(store.acquire(&key), Some("buffer"));
    assert_eq!(store.views(&key), 3);
    assert!(store.get(&key).is_some());
    assert_eq!(store.release(&key), None);
    assert_eq!(store.release(&key), None);
    assert_eq!(store.release(&key), Some("buffer"));
    assert!(store.is_empty());
}

#[test]
fn save_as_keeps_the_handle_and_all_views() {
    let doc = DocId(7);
    let old = DocumentKey::Untitled(doc);
    let new = DocumentKey::Path(std::path::PathBuf::from("/tmp/saved.rs"));
    let mut store = DocumentStore::default();
    store.insert_view(old.clone(), doc, "buffer");
    store.acquire(&old);
    store.rekey(&old, new.clone(), doc).unwrap();
    assert_eq!(store.views(&new), 2);
    assert_eq!(store.get_by_doc(doc), Some(&"buffer"));
    assert!(store.get(&old).is_none());
}

#[test]
fn closing_one_view_does_not_release_the_last_shared_document_early() {
    let doc = DocId(9);
    let key = DocumentKey::Path(std::path::PathBuf::from("/tmp/file.rs"));
    let mut store = DocumentStore::default();
    store.insert_view(key.clone(), doc, "buffer");
    store.acquire(&key);
    assert!(store.release_doc(doc).is_none());
    assert_eq!(store.release_doc(doc), Some("buffer"));
    assert!(!store.contains_key(&key));
}
