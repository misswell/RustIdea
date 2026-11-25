use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use ide_core::DocId;

/// Identity of one editable document. File documents are keyed by a
/// canonicalized path, so `./src/main.rs`, `src/../src/main.rs` and a symlink
/// all resolve to the same buffer. Untitled buffers have no path and are keyed
/// by the backend document id until Save As assigns one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DocumentKey {
    Path(PathBuf),
    Untitled(DocId),
}

impl DocumentKey {
    #[cfg(test)]
    pub fn for_path(path: &Path) -> Self {
        Self::Path(normalize_path(path))
    }

    /// UI-thread-safe identity: lexical only. Backend `DocumentInfo` paths are
    /// already absolute/canonical and should be used once the document opens.
    pub fn for_ui_path(path: &Path) -> Self {
        Self::Path(lexical_path(path))
    }

    pub fn for_untitled(doc: DocId) -> Self {
        Self::Untitled(doc)
    }
}

/// Canonicalizes an existing file, or normalizes a path lexically when the
/// file does not exist yet (for example a Save As target).
#[cfg(test)]
pub fn normalize_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }
    lexical_normalize(path)
}

/// Pure lexical normalization. Never touches the filesystem.
pub fn lexical_path(path: &Path) -> PathBuf {
    lexical_normalize(path)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = output
                    .components()
                    .next_back()
                    .is_some_and(|last| matches!(last, Component::Normal(_)));
                if can_pop {
                    output.pop();
                } else if !output.has_root() {
                    output.push(component.as_os_str());
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                output.push(component.as_os_str());
            }
        }
    }
    if output.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        output
    }
}

/// One entry per canonical document. `views` counts tabs/splits that currently
/// show the buffer; the handle is released when the last view closes.
#[derive(Debug)]
pub struct DocumentStore<T> {
    by_key: HashMap<DocumentKey, T>,
    key_by_doc: HashMap<DocId, DocumentKey>,
    views: HashMap<DocumentKey, usize>,
}

impl<T> Default for DocumentStore<T> {
    fn default() -> Self {
        Self {
            by_key: HashMap::new(),
            key_by_doc: HashMap::new(),
            views: HashMap::new(),
        }
    }
}

impl<T> DocumentStore<T> {
    pub fn get(&self, key: &DocumentKey) -> Option<&T> {
        self.by_key.get(key)
    }

    pub fn get_by_doc(&self, doc: DocId) -> Option<&T> {
        self.key_by_doc
            .get(&doc)
            .and_then(|key| self.by_key.get(key))
    }

    pub fn key_for_doc(&self, doc: DocId) -> Option<&DocumentKey> {
        self.key_by_doc.get(&doc)
    }

    pub fn contains_key(&self, key: &DocumentKey) -> bool {
        self.by_key.contains_key(key)
    }

    /// Inserts a document and starts with one view. Returns `false` when the
    /// canonical key is already registered; callers must then reuse the
    /// existing handle and close the redundant backend reference.
    pub fn insert_view(&mut self, key: DocumentKey, doc: DocId, handle: T) -> bool {
        if self.by_key.contains_key(&key) {
            return false;
        }
        self.key_by_doc.insert(doc, key.clone());
        self.views.insert(key.clone(), 1);
        self.by_key.insert(key, handle);
        true
    }

    /// Adds one tab/split view and returns the shared handle.
    pub fn acquire(&mut self, key: &DocumentKey) -> Option<T>
    where
        T: Clone,
    {
        let handle = self.by_key.get(key)?.clone();
        *self.views.entry(key.clone()).or_insert(0) += 1;
        Some(handle)
    }

    /// Releases one tab/split view. The document is removed only after the
    /// last view closes; the removed handle is returned so the caller can
    /// close the backend reference exactly once.
    pub fn release(&mut self, key: &DocumentKey) -> Option<T> {
        let remaining = {
            let views = self.views.get_mut(key)?;
            *views = views.saturating_sub(1);
            *views
        };
        if remaining > 0 {
            return None;
        }
        self.views.remove(key);
        self.key_by_doc.retain(|_, value| value != key);
        self.by_key.remove(key)
    }

    pub fn release_doc(&mut self, doc: DocId) -> Option<T> {
        let key = self.key_by_doc.get(&doc)?.clone();
        self.release(&key)
    }

    /// Save As changes the identity of an existing untitled document without
    /// creating a second buffer. Fails when the target path is already open.
    pub fn rekey(&mut self, old: &DocumentKey, new: DocumentKey, doc: DocId) -> Result<(), ()> {
        if old == &new {
            return Ok(());
        }
        if self.by_key.contains_key(&new) {
            return Err(());
        }
        let Some(handle) = self.by_key.remove(old) else {
            return Err(());
        };
        let views = self.views.remove(old).unwrap_or(1);
        self.by_key.insert(new.clone(), handle);
        self.views.insert(new.clone(), views);
        self.key_by_doc.insert(doc, new);
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&DocumentKey, &T)> {
        self.by_key.iter()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    #[cfg(test)]
    pub fn views(&self, key: &DocumentKey) -> usize {
        self.views.get(key).copied().unwrap_or(0)
    }

    pub fn clear(&mut self) {
        self.by_key.clear();
        self.key_by_doc.clear();
        self.views.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn lexical_normalization_removes_dot_segments() {
        assert_eq!(
            lexical_normalize(Path::new("/tmp/a/./b/../c")),
            PathBuf::from("/tmp/a/c")
        );
        assert_eq!(
            lexical_normalize(Path::new("src/../src/main.rs")),
            PathBuf::from("src/main.rs")
        );
    }

    #[test]
    fn symlinked_paths_share_one_key() {
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("real");
        fs::write(&real, "fn main() {}\n").unwrap();
        let link = root.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(not(unix))]
        fs::copy(&real, &link).unwrap();

        assert_eq!(
            DocumentKey::for_path(&real),
            DocumentKey::for_path(&link),
            "canonical paths must identify the same document"
        );
    }

    #[test]
    fn store_keeps_one_handle_until_the_last_view_closes() {
        let doc = DocId(7);
        let untitled = DocumentKey::for_untitled(doc);
        let mut store = DocumentStore::default();
        assert!(store.insert_view(untitled.clone(), doc, "shared"));
        assert!(!store.insert_view(untitled.clone(), doc, "duplicate"));
        assert_eq!(store.acquire(&untitled), Some("shared"));
        assert_eq!(store.release(&untitled), None);
        assert_eq!(store.release(&untitled), Some("shared"));
        assert!(store.is_empty());
    }

    #[test]
    fn save_as_rekeys_without_duplicating_the_handle() {
        let doc = DocId(3);
        let old = DocumentKey::for_untitled(doc);
        let new = DocumentKey::Path(PathBuf::from("/tmp/saved.rs"));
        let mut store = DocumentStore::default();
        store.insert_view(old.clone(), doc, "buffer");
        assert!(store.rekey(&old, new.clone(), doc).is_ok());
        assert_eq!(store.get(&new), Some(&"buffer"));
        assert!(store.get(&old).is_none());
        assert_eq!(store.views(&new), 1);
    }
}
