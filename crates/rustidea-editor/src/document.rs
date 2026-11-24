use std::collections::VecDeque;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use ide_core::{ChangeSet, Pos, TextChange};
use ide_text::Document as TextDocument;

const DEFAULT_HISTORY_LIMIT: usize = 1_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(u64);

impl DocumentId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditSource {
    Local,
    External,
    LanguageServer,
    Formatter,
    Undo,
    Redo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub from: u32,
    pub to: u32,
    pub insert: Arc<str>,
}

impl Change {
    pub fn insert(at: u32, text: impl Into<Arc<str>>) -> Self {
        Self {
            from: at,
            to: at,
            insert: text.into(),
        }
    }

    pub fn replace(range: Range<u32>, text: impl Into<Arc<str>>) -> Self {
        Self {
            from: range.start,
            to: range.end,
            insert: text.into(),
        }
    }

    pub fn inverse(&self, inserted_text: &str) -> Self {
        let inserted_len = inserted_text.encode_utf16().count() as u32;
        Self {
            from: self.from,
            to: self.from + inserted_len,
            insert: Arc::from(inserted_text),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    pub base_version: u32,
    pub changes: Vec<Change>,
    pub source: EditSource,
}

impl Transaction {
    pub fn new(base_version: u32, changes: Vec<Change>, source: EditSource) -> Self {
        Self {
            base_version,
            changes,
            source,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: u32,
    pub head: u32,
}

impl Selection {
    pub const fn caret(offset: u32) -> Self {
        Self {
            anchor: offset,
            head: offset,
        }
    }

    pub const fn range(anchor: u32, head: u32) -> Self {
        Self { anchor, head }
    }

    pub fn normalized(self) -> Range<u32> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub fn is_empty(self) -> bool {
        self.anchor == self.head
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionSet {
    pub primary: Selection,
}

impl Default for SelectionSet {
    fn default() -> Self {
        Self {
            primary: Selection::caret(0),
        }
    }
}

#[derive(Debug)]
pub enum DocumentError {
    OutOfSync { expected: u32, actual: u32 },
    InvalidRange { from: u32, to: u32, len: u32 },
    OverlappingChanges,
    InvalidUtf16Boundary(u32),
    Io(std::io::Error),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfSync { expected, actual } => {
                write!(
                    f,
                    "transaction base version {expected} does not match document version {actual}"
                )
            }
            Self::InvalidRange { from, to, len } => {
                write!(
                    f,
                    "edit range {from}..{to} is outside document length {len}"
                )
            }
            Self::OverlappingChanges => f.write_str("edit changes overlap or are not sorted"),
            Self::InvalidUtf16Boundary(offset) => {
                write!(f, "offset {offset} is not a UTF-16 code unit boundary")
            }
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for DocumentError {}

impl From<std::io::Error> for DocumentError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    inverse: Vec<Change>,
    forward: Vec<Change>,
}

pub struct Document {
    id: DocumentId,
    text: TextDocument,
    saved_version: u32,
    path: Option<std::path::PathBuf>,
    undo: VecDeque<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    history_limit: usize,
}

impl Document {
    pub fn new(id: DocumentId, text: &str) -> Self {
        Self {
            id,
            text: TextDocument::new(text),
            saved_version: 0,
            path: None,
            undo: VecDeque::new(),
            redo: Vec::new(),
            history_limit: DEFAULT_HISTORY_LIMIT,
        }
    }

    /// Builds the local view of a document that already exists in the backend
    /// (an OpenIDEA `Documents` buffer). `version` is the backend's version;
    /// the local mirror must start at the same version so the first edit can
    /// be applied to both without a gap.
    pub fn from_backend(
        id: DocumentId,
        path: Option<std::path::PathBuf>,
        text: &str,
        version: u32,
        dirty: bool,
    ) -> Self {
        let mut document = Self::new(id, text);
        document.path = path;
        document.text.set_version(version);
        if !dirty {
            document.saved_version = version;
        }
        document
    }

    pub fn open(
        id: DocumentId,
        path: impl Into<std::path::PathBuf>,
    ) -> Result<Self, DocumentError> {
        let path = path.into();
        let text = std::fs::read_to_string(&path)?;
        let mut document = Self::new(id, &text);
        document.path = Some(path);
        Ok(document)
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: impl Into<std::path::PathBuf>) {
        self.path = Some(path.into());
    }

    pub fn version(&self) -> u32 {
        self.text.version()
    }

    pub fn line_count(&self) -> u32 {
        self.text.line_count()
    }

    pub fn len_utf16(&self) -> u32 {
        self.text.len_utf16()
    }

    pub fn text(&self) -> String {
        self.text.text()
    }

    pub fn text_for_save(&self) -> String {
        self.text.text_for_save()
    }

    pub fn snapshot(&self) -> TextDocument {
        self.text.clone()
    }

    pub fn line(&self, line: u32) -> Option<String> {
        self.text.line(line)
    }

    pub fn line_start(&self, line: u32) -> u32 {
        self.text.pos_to_offset(Pos::new(line, 0))
    }

    pub fn line_end(&self, line: u32) -> u32 {
        self.text.pos_to_offset(Pos::new(line, u32::MAX))
    }

    pub fn offset_to_line_col(&self, offset: u32) -> (u32, u32) {
        let pos = self.text.offset_to_pos(offset.min(self.len_utf16()));
        (pos.line, pos.col)
    }

    pub fn offset_for_line_col(&self, line: u32, col: u32) -> u32 {
        self.text.pos_to_offset(Pos::new(line, col))
    }

    pub fn offset_to_byte(&self, offset: u32) -> usize {
        self.text.offset_to_byte(offset.min(self.len_utf16()))
    }

    pub fn byte_to_offset(&self, byte: usize) -> u32 {
        self.text.byte_to_offset(byte)
    }

    pub fn slice(&self, from: u32, to: u32) -> String {
        self.text
            .slice(from.min(self.len_utf16()), to.min(self.len_utf16()))
    }

    pub fn is_dirty(&self) -> bool {
        self.version() != self.saved_version
    }

    pub fn mark_saved(&mut self) {
        self.saved_version = self.version();
    }

    /// Mirrors a backend dirty notification. A dirty flag on a document that
    /// matches its saved version moves the saved marker one version back; the
    /// exact backend version is restored by the next snapshot or save.
    pub fn set_dirty(&mut self, dirty: bool) {
        if dirty {
            if self.saved_version == self.version() {
                self.saved_version = self.version().wrapping_sub(1);
            }
        } else {
            self.saved_version = self.version();
        }
    }

    /// Replaces the mirror after the backend reports a full document snapshot.
    /// Local history is discarded because the backend is authoritative and
    /// may have changed the text through a reload, format or workspace edit.
    pub fn replace_from_backend(&mut self, text: &str, version: u32, dirty: bool) {
        self.text = TextDocument::new(text);
        self.text.set_version(version);
        self.saved_version = if dirty {
            version.saturating_sub(1)
        } else {
            version
        };
        self.undo.clear();
        self.redo.clear();
    }

    /// Applies a backend-originated patch without recording a local undo step.
    pub fn apply_backend_patch(
        &mut self,
        base_version: u32,
        changes: Vec<Change>,
    ) -> Result<u32, DocumentError> {
        if base_version != self.version() {
            return Err(DocumentError::OutOfSync {
                expected: base_version,
                actual: self.version(),
            });
        }
        validate_changes(&changes, self.len_utf16())?;
        let change_set = to_change_set(&changes);
        self.text
            .apply(base_version, &change_set)
            .map_err(|error| DocumentError::Io(std::io::Error::other(error.to_string())))?;
        Ok(self.version())
    }

    pub fn mark_saved_if_version(&mut self, version: u32) -> bool {
        if self.version() == version {
            self.saved_version = version;
            true
        } else {
            false
        }
    }

    pub fn save(&mut self) -> Result<(), DocumentError> {
        let Some(path) = self.path.clone() else {
            return Err(DocumentError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "document has no path",
            )));
        };
        std::fs::write(&path, self.text.text_for_save())?;
        self.mark_saved();
        Ok(())
    }

    pub fn apply(&mut self, transaction: Transaction) -> Result<Vec<Change>, DocumentError> {
        if transaction.base_version != self.version() {
            return Err(DocumentError::OutOfSync {
                expected: transaction.base_version,
                actual: self.version(),
            });
        }
        validate_changes(&transaction.changes, self.len_utf16())?;
        for change in &transaction.changes {
            for offset in [change.from, change.to] {
                if self.byte_to_offset(self.offset_to_byte(offset)) != offset {
                    return Err(DocumentError::InvalidUtf16Boundary(offset));
                }
            }
        }
        if transaction
            .changes
            .iter()
            .all(|change| change.from == change.to && change.insert.is_empty())
        {
            return Ok(Vec::new());
        }
        let mut delta: i64 = 0;
        let inverse: Vec<Change> = transaction
            .changes
            .iter()
            .map(|change| {
                let inserted_len = ide_text::normalize_line_endings(&change.insert)
                    .encode_utf16()
                    .count() as u32;
                let final_from = (change.from as i64 + delta) as u32;
                let final_to = final_from + inserted_len;
                let old_text = self.text.slice(change.from, change.to);
                delta += inserted_len as i64 - (change.to - change.from) as i64;
                Change {
                    from: final_from,
                    to: final_to,
                    insert: Arc::from(old_text),
                }
            })
            .collect();
        let change_set = to_change_set(&transaction.changes);
        self.text
            .apply(transaction.base_version, &change_set)
            .map_err(|error| DocumentError::Io(std::io::Error::other(error.to_string())))?;

        if transaction.source == EditSource::Local {
            let forward = transaction.changes.clone();
            self.undo.push_back(HistoryEntry {
                inverse: inverse.clone(),
                forward,
            });
            if self.undo.len() > self.history_limit {
                self.undo.pop_front();
            }
            self.redo.clear();
        }
        Ok(inverse)
    }

    pub fn replace_all(
        &mut self,
        text: &str,
        source: EditSource,
    ) -> Result<Vec<Change>, DocumentError> {
        let len = self.len_utf16();
        self.apply(Transaction::new(
            self.version(),
            vec![Change::replace(0..len, text)],
            source,
        ))
    }

    pub fn undo(&mut self) -> Result<bool, DocumentError> {
        Ok(self.undo_with_changes()?.is_some())
    }

    /// Undo and return the exact edits that must be mirrored to the backend.
    /// This avoids diffing two full copies of large documents on every undo.
    pub fn undo_with_changes(&mut self) -> Result<Option<Vec<Change>>, DocumentError> {
        let Some(entry) = self.undo.pop_back() else {
            return Ok(None);
        };
        let version = self.version();
        if let Err(error) = self.apply(Transaction::new(
            version,
            entry.inverse.clone(),
            EditSource::Undo,
        )) {
            self.undo.push_back(entry);
            return Err(error);
        }
        let changes = entry.inverse.clone();
        self.redo.push(entry);
        Ok(Some(changes))
    }

    pub fn redo(&mut self) -> Result<bool, DocumentError> {
        Ok(self.redo_with_changes()?.is_some())
    }

    pub fn redo_with_changes(&mut self) -> Result<Option<Vec<Change>>, DocumentError> {
        let Some(entry) = self.redo.pop() else {
            return Ok(None);
        };
        let version = self.version();
        if let Err(error) = self.apply(Transaction::new(
            version,
            entry.forward.clone(),
            EditSource::Redo,
        )) {
            self.redo.push(entry);
            return Err(error);
        }
        let changes = entry.forward.clone();
        self.undo.push_back(entry);
        Ok(Some(changes))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

fn validate_changes(changes: &[Change], len: u32) -> Result<(), DocumentError> {
    let mut previous_to = 0;
    for change in changes {
        if change.from > change.to || change.to > len {
            return Err(DocumentError::InvalidRange {
                from: change.from,
                to: change.to,
                len,
            });
        }
        if change.from < previous_to {
            return Err(DocumentError::OverlappingChanges);
        }
        previous_to = change.to;
    }
    Ok(())
}

fn to_change_set(changes: &[Change]) -> ChangeSet {
    ChangeSet {
        changes: changes
            .iter()
            .map(|change| TextChange {
                from: change.from,
                to: change.to,
                insert: change.insert.to_string(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Document {
        Document::new(DocumentId::new(1), text)
    }

    #[test]
    fn insert_replace_and_versions() {
        let mut document = doc("hello world");
        document
            .apply(Transaction::new(
                0,
                vec![Change::replace(6..11, "RustIdea")],
                EditSource::Local,
            ))
            .unwrap();
        assert_eq!(document.text(), "hello RustIdea");
        assert_eq!(document.version(), 1);
        assert!(document.is_dirty());

        document
            .apply(Transaction::new(
                1,
                vec![Change::insert(5, " native")],
                EditSource::Local,
            ))
            .unwrap();
        assert_eq!(document.text(), "hello native RustIdea");
        assert_eq!(document.version(), 2);
    }

    #[test]
    fn stale_transactions_are_rejected_without_mutation() {
        let mut document = doc("abc");
        let error = document
            .apply(Transaction::new(
                1,
                vec![Change::insert(0, "x")],
                EditSource::Local,
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            DocumentError::OutOfSync {
                expected: 1,
                actual: 0
            }
        ));
        assert_eq!(document.text(), "abc");
    }

    #[test]
    fn undo_redo_round_trip_multiple_changes() {
        let mut document = doc("one two three");
        document
            .apply(Transaction::new(
                0,
                vec![Change::replace(0..3, "1"), Change::replace(8..13, "3")],
                EditSource::Local,
            ))
            .unwrap();
        assert_eq!(document.text(), "1 two 3");
        assert!(document.undo().unwrap());
        assert_eq!(document.text(), "one two three");
        assert!(document.redo().unwrap());
        assert_eq!(document.text(), "1 two 3");
        assert_eq!(document.version(), 3);
        assert!(document.undo().unwrap());
        assert_eq!(document.text(), "one two three");
        assert!(document.redo().unwrap());
        assert_eq!(document.text(), "1 two 3");
    }

    #[test]
    fn undo_returns_the_exact_inverse_change() {
        let mut document = doc("hello");
        document
            .apply(Transaction::new(
                0,
                vec![Change::replace(1..4, "ELL")],
                EditSource::Local,
            ))
            .unwrap();
        let changes = document.undo_with_changes().unwrap().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].from, 1);
        assert_eq!(changes[0].to, 4);
        assert_eq!(&*changes[0].insert, "ell");
        assert_eq!(document.text(), "hello");
    }

    #[test]
    fn history_is_bounded() {
        let mut document = doc("");
        for index in 0..1_100 {
            let len = document.len_utf16();
            document
                .apply(Transaction::new(
                    document.version(),
                    vec![Change::insert(len, format!("{index}\n"))],
                    EditSource::Local,
                ))
                .unwrap();
        }
        let mut undo_count = 0;
        while document.undo().unwrap() {
            undo_count += 1;
        }
        assert_eq!(undo_count, DEFAULT_HISTORY_LIMIT);
    }

    #[test]
    fn million_line_document_stays_editable() {
        let text = "fn generated() {}\n".repeat(1_000_000);
        let mut document = Document::new(DocumentId::new(9), &text);
        assert_eq!(document.line_count(), 1_000_001);
        let middle = document.line_start(500_000);
        document
            .apply(Transaction::new(
                document.version(),
                vec![Change::insert(middle, "// stress\n")],
                EditSource::Local,
            ))
            .unwrap();
        assert_eq!(document.line_count(), 1_000_002);
        assert_eq!(document.line(500_000).as_deref(), Some("// stress"));
        assert!(document.undo().unwrap());
        assert_eq!(document.line_count(), 1_000_001);
    }

    #[test]
    fn undo_redo_normalizes_pasted_line_endings() {
        let mut document = doc("xy");
        document
            .apply(Transaction::new(
                0,
                vec![Change::insert(1, "a\r\nb\rc")],
                EditSource::Local,
            ))
            .unwrap();
        assert_eq!(document.text(), "xa\nb\ncy");
        assert!(document.undo().unwrap());
        assert_eq!(document.text(), "xy");
        assert!(document.redo().unwrap());
        assert_eq!(document.text(), "xa\nb\ncy");
    }

    #[test]
    fn undo_restores_multibyte_text_and_utf16_offsets() {
        let mut document = doc("a😀中b");
        document
            .apply(Transaction::new(
                0,
                vec![Change::replace(3..4, "文")],
                EditSource::Local,
            ))
            .unwrap();
        assert_eq!(document.text(), "a😀文b");
        assert!(document.undo().unwrap());
        assert_eq!(document.text(), "a😀中b");
        assert_eq!(document.offset_to_line_col(4), (0, 4));
        assert_eq!(document.offset_for_line_col(0, 4), 4);
        assert_eq!(document.offset_to_byte(4), 8);
        assert_eq!(document.byte_to_offset(8), 4);
    }

    #[test]
    fn rejects_edits_inside_surrogate_pairs_without_mutating() {
        let mut document = doc("a😀b");
        let error = document
            .apply(Transaction::new(
                0,
                vec![Change::insert(2, "x")],
                EditSource::Local,
            ))
            .unwrap_err();
        assert!(matches!(error, DocumentError::InvalidUtf16Boundary(2)));
        assert_eq!(document.text(), "a😀b");
        assert_eq!(document.version(), 0);
        assert!(!document.can_undo());
    }

    #[test]
    fn empty_edit_does_not_dirty_document_or_add_history() {
        let mut document = doc("");
        document
            .apply(Transaction::new(
                0,
                vec![Change::replace(0..0, "")],
                EditSource::Local,
            ))
            .unwrap();
        assert_eq!(document.version(), 0);
        assert!(!document.is_dirty());
        assert!(!document.can_undo());
    }

    #[test]
    fn line_coordinates_are_utf16_and_clamped() {
        let document = doc("a😀b\n中文\n");
        assert_eq!(document.line_count(), 3);
        assert_eq!(document.line_start(1), 5);
        assert_eq!(document.line_end(0), 4);
        assert_eq!(document.line_end(1), 7);
        assert_eq!(document.offset_for_line_col(0, 99), 4);
        assert_eq!(document.offset_for_line_col(99, 0), 8);
        assert_eq!(document.offset_to_line_col(5), (1, 0));
    }

    #[test]
    fn save_preserves_original_line_endings_and_clears_dirty() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.txt");
        std::fs::write(&path, "one\r\ntwo\r\n").unwrap();
        let mut document = Document::open(DocumentId::new(1), &path).unwrap();
        document
            .apply(Transaction::new(
                0,
                vec![Change::replace(0..3, "1")],
                EditSource::Local,
            ))
            .unwrap();
        assert!(document.is_dirty());
        document.save().unwrap();
        assert!(!document.is_dirty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "1\r\ntwo\r\n");
    }

    #[test]
    fn history_limit_bounds_memory() {
        let mut document = doc("");
        document.history_limit = 2;
        for i in 0..5 {
            document
                .apply(Transaction::new(
                    document.version(),
                    vec![Change::insert(0, format!("{i}"))],
                    EditSource::Local,
                ))
                .unwrap();
        }
        assert_eq!(document.undo.len(), 2);
        assert!(document.undo().unwrap());
        assert!(document.undo().unwrap());
        assert!(!document.undo().unwrap());
    }

    #[test]
    fn saved_version_is_not_advanced_past_newer_edits() {
        let mut document = doc("a");
        document
            .apply(Transaction::new(
                0,
                vec![Change::insert(1, "b")],
                EditSource::Local,
            ))
            .unwrap();
        let saving_version = document.version();
        document
            .apply(Transaction::new(
                saving_version,
                vec![Change::insert(2, "c")],
                EditSource::Local,
            ))
            .unwrap();
        assert!(!document.mark_saved_if_version(saving_version));
        assert!(document.is_dirty());
        assert!(document.mark_saved_if_version(document.version()));
        assert!(!document.is_dirty());
    }

    #[test]
    fn save_snapshot_is_independent_of_later_edits() {
        let mut document = doc("first\r\nline\r\n");
        let snapshot = document.snapshot();
        document
            .apply(Transaction::new(
                0,
                vec![Change::replace(0..5, "second")],
                EditSource::Local,
            ))
            .unwrap();
        assert_eq!(snapshot.text_for_save(), "first\r\nline\r\n");
        assert_eq!(document.text_for_save(), "second\r\nline\r\n");
    }

    #[test]
    fn overlap_is_rejected() {
        let mut document = doc("abcdef");
        let error = document
            .apply(Transaction::new(
                0,
                vec![Change::replace(2..4, "x"), Change::replace(3..5, "y")],
                EditSource::Local,
            ))
            .unwrap_err();
        assert!(matches!(error, DocumentError::OverlappingChanges));
        assert_eq!(document.text(), "abcdef");
    }
}
