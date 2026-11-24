use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{Context, EventEmitter};
use ide_core::{ChangeSet, DocId, TextChange};
use ide_workspace::DocumentInfo;
use rustidea_editor::{Change, Document, DocumentId, EditSource, Transaction};

use crate::native_host::NativeHost;

const MAX_HIGHLIGHT_LINES: u32 = 4_000;
const MAX_HIGHLIGHT_UTF16: u32 = 512 * 1024;

#[derive(Clone, Debug)]
pub enum BufferEvent {
    Changed,
    DirtyChanged,
    Saved,
    TitleChanged,
    Reloaded,
    ExternalChanged,
    ExternalResolved,
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiskStamp {
    pub exists: bool,
    pub mtime_ms: f64,
    pub len: u64,
}

impl DiskStamp {
    pub fn of(path: &Path) -> Self {
        match std::fs::metadata(path) {
            Ok(metadata) => Self {
                exists: true,
                mtime_ms: ide_vfs::mtime_ms(&metadata),
                len: metadata.len(),
            },
            Err(_) => Self {
                exists: false,
                mtime_ms: 0.0,
                len: 0,
            },
        }
    }
}

/// Shared, authoritative editor buffer. Every editor view points at one
/// `DocumentBuffer`; the backend `ide-workspace::Documents` registry remains
/// the source of truth for saves, LSP, external changes and workspace edits.
pub struct DocumentBuffer {
    host: Arc<NativeHost>,
    host_doc: Option<DocId>,
    document: Document,
    path: Option<PathBuf>,
    language: String,
    read_only: bool,
    large: bool,
    error: Option<String>,
    backend_version: u32,
    saving: bool,
    disk_stamp: DiskStamp,
    external_change: Option<DiskStamp>,
    external_reload_inflight: bool,
}

impl EventEmitter<BufferEvent> for DocumentBuffer {}

impl DocumentBuffer {
    pub fn from_host(host: Arc<NativeHost>, info: DocumentInfo) -> Self {
        let path = match &normalize_host_path(info.path.0.clone()) {
            path if path.to_string_lossy().starts_with("untitled:") => None,
            path => Some(path.clone()),
        };
        let disk_stamp = path.as_deref().map(DiskStamp::of).unwrap_or(DiskStamp {
            exists: false,
            mtime_ms: 0.0,
            len: 0,
        });
        let id = DocumentId::new(info.id.0 as u64);
        let document =
            Document::from_backend(id, path.clone(), &info.text, info.version, info.dirty);
        Self {
            host,
            host_doc: Some(info.id),
            document,
            path,
            language: info.language.to_string(),
            read_only: info.read_only,
            large: info.large,
            error: None,
            backend_version: info.version,
            saving: false,
            disk_stamp,
            external_change: None,
            external_reload_inflight: false,
        }
    }

    pub fn host_doc(&self) -> Option<DocId> {
        self.host_doc
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn title(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn set_language(&mut self, language: impl Into<String>) {
        self.language = language.into();
    }

    pub fn is_large(&self) -> bool {
        self.large
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_saving(&self) -> bool {
        self.saving
    }

    pub fn has_external_change(&self) -> bool {
        self.external_change.is_some()
    }

    pub fn external_change_message(&self) -> Option<String> {
        let path = self.path.as_deref()?;
        let change = self.external_change?;
        if change.exists {
            Some(format!(
                "{} changed on disk while this buffer has unsaved edits. \
                 Reload it or explicitly keep your version.",
                path.display()
            ))
        } else {
            Some(format!(
                "{} was removed on disk while this buffer has unsaved edits. \
                 Save to recreate it, or discard the buffer.",
                path.display()
            ))
        }
    }

    /// Detects an external write before a save can overwrite it. Clean
    /// documents reload from the backend; dirty ones keep their text and raise
    /// an explicit conflict that must be resolved by the user.
    pub fn check_external_change(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(path) = self.path.clone() else {
            return false;
        };
        let current = DiskStamp::of(&path);
        if current == self.disk_stamp {
            return false;
        }
        if !current.exists {
            self.external_change = Some(current);
            self.error = self.external_change_message();
            cx.emit(BufferEvent::ExternalChanged);
            cx.notify();
            return true;
        }
        if self.document.is_dirty() {
            self.external_change = Some(current);
            self.error = self.external_change_message();
            cx.emit(BufferEvent::ExternalChanged);
            cx.notify();
            return true;
        }
        if self.external_reload_inflight {
            return true;
        }
        let Some(host_doc) = self.host_doc else {
            return false;
        };
        let host = self.host.clone();
        self.external_reload_inflight = true;
        cx.spawn(async move |this, cx| {
            let result = host.reload_document_discarding_changes(host_doc).await;
            this.update(cx, |buffer, cx| {
                buffer.external_reload_inflight = false;
                match result {
                    Ok(()) => {
                        buffer.reload_from_backend();
                        buffer.external_change = None;
                        buffer.error = None;
                        cx.emit(BufferEvent::Reloaded);
                    }
                    Err(error) => {
                        buffer.error = Some(error.message.clone());
                        cx.emit(BufferEvent::Error(error.message));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
        true
    }

    /// Reload discards the local buffer and takes the current disk text after
    /// the user explicitly chose that side of a conflict.
    pub fn reload_after_conflict(&mut self, cx: &mut Context<Self>) {
        let Some(host_doc) = self.host_doc else {
            return;
        };
        if self.external_reload_inflight {
            return;
        }
        self.external_reload_inflight = true;
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.reload_document_discarding_changes(host_doc).await;
            this.update(cx, |buffer, cx| {
                buffer.external_reload_inflight = false;
                match result {
                    Ok(()) => {
                        buffer.reload_from_backend();
                        buffer.external_change = None;
                        buffer.error = None;
                        cx.emit(BufferEvent::Reloaded);
                    }
                    Err(error) => {
                        buffer.error = Some(error.message.clone());
                        cx.emit(BufferEvent::Error(error.message));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Keep the local buffer and allow an explicit save to overwrite the
    /// external file. The current disk stamp is recorded so the same change is
    /// not reported again.
    pub fn keep_local_after_conflict(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.path.as_deref() {
            self.disk_stamp = DiskStamp::of(path);
        }
        self.external_change = None;
        self.error = None;
        cx.emit(BufferEvent::ExternalResolved);
        cx.notify();
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        let Some(host_doc) = self.host_doc else {
            self.error = Some("This buffer has no backend document".into());
            cx.notify();
            return;
        };
        if self.saving {
            return;
        }
        if self.path.is_none() {
            self.error = Some("Use Save As to choose a file".into());
            cx.notify();
            return;
        }
        if let Some(message) = self.external_change_message() {
            self.error = Some(message.clone());
            cx.emit(BufferEvent::Error(message));
            cx.notify();
            return;
        }
        if self.external_reload_inflight {
            self.error = Some("Wait for the external reload to finish".into());
            cx.notify();
            return;
        }
        let version = self.document.version();
        let host = self.host.clone();
        self.saving = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = host.save_document(host_doc).await;
            this.update(cx, |buffer, cx| {
                buffer.saving = false;
                match result {
                    Ok(()) => {
                        if let Some(path) = buffer.path.as_deref() {
                            buffer.disk_stamp = DiskStamp::of(path);
                        }
                        buffer.mark_saved_version(version, cx);
                    }
                    Err(error) => {
                        buffer.error = Some(error.message.clone());
                        cx.emit(BufferEvent::Error(error.message));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn save_as(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(host_doc) = self.host_doc else {
            self.error = Some("This buffer has no backend document".into());
            cx.notify();
            return;
        };
        if self.saving {
            self.error = Some("Wait for the current save before using Save As".into());
            cx.notify();
            return;
        }
        let version = self.document.version();
        let host = self.host.clone();
        let write_path = path.clone();
        self.saving = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = host.save_document_as(host_doc, write_path).await;
            this.update(cx, |buffer, cx| {
                buffer.saving = false;
                match result {
                    Ok(info) => {
                        let canonical = normalize_host_path(info.path.0.clone());
                        buffer.path = Some(canonical.clone());
                        buffer.document.set_path(canonical.clone());
                        buffer.backend_version = info.version;
                        buffer.language = info.language.to_string();
                        buffer.disk_stamp = DiskStamp::of(&canonical);
                        buffer.external_change = None;
                        buffer.mark_saved_version(version, cx);
                        cx.emit(BufferEvent::TitleChanged);
                    }
                    Err(error) => {
                        buffer.error = Some(error.message.clone());
                        cx.emit(BufferEvent::Error(error.message));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn text(&self) -> String {
        self.document.text()
    }

    /// Tree-sitter highlights for the current language and version. Large
    /// documents and plain text deliberately skip the parse.
    pub fn syntax_highlights(&self) -> Option<ide_language::SyntaxHighlights> {
        if self.large
            || self.language.is_empty()
            || self.language == "plaintext"
            || self.document.line_count() > MAX_HIGHLIGHT_LINES
            || self.document.len_utf16() > MAX_HIGHLIGHT_UTF16
        {
            return None;
        }
        self.host
            .highlights(&ide_core::LanguageId(self.language.clone()), &self.text())
    }

    pub fn version(&self) -> u32 {
        self.document.version()
    }

    pub fn is_dirty(&self) -> bool {
        self.document.is_dirty()
    }

    pub fn line_count(&self) -> u32 {
        self.document.line_count()
    }

    pub fn line(&self, line: u32) -> Option<String> {
        self.document.line(line)
    }

    pub fn line_start(&self, line: u32) -> u32 {
        self.document.line_start(line)
    }

    pub fn line_end(&self, line: u32) -> u32 {
        self.document.line_end(line)
    }

    pub fn len_utf16(&self) -> u32 {
        self.document.len_utf16()
    }

    pub fn offset_to_line_col(&self, offset: u32) -> (u32, u32) {
        self.document.offset_to_line_col(offset)
    }

    pub fn offset_for_line_col(&self, line: u32, col: u32) -> u32 {
        self.document.offset_for_line_col(line, col)
    }

    pub fn slice(&self, from: u32, to: u32) -> String {
        self.document.slice(from, to)
    }

    pub fn apply_local(
        &mut self,
        range: std::ops::Range<u32>,
        text: &str,
        source: EditSource,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.read_only {
            self.error = Some("The document is read-only".into());
            cx.emit(BufferEvent::Error(self.error.clone().unwrap_or_default()));
            return false;
        }
        let version = self.document.version();
        let change = Change::replace(range.clone(), text);
        match self
            .document
            .apply(Transaction::new(version, vec![change.clone()], source))
        {
            Ok(_) => {
                self.push_to_backend(vec![change], false);
                self.error = None;
                cx.emit(BufferEvent::Changed);
                cx.notify();
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.emit(BufferEvent::Error(error.to_string()));
                cx.notify();
                false
            }
        }
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) -> bool {
        match self.document.undo_with_changes() {
            Ok(Some(changes)) => {
                self.push_to_backend(changes, false);
                self.error = None;
                cx.emit(BufferEvent::Changed);
                cx.notify();
                true
            }
            Ok(None) => false,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.emit(BufferEvent::Error(error.to_string()));
                cx.notify();
                false
            }
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) -> bool {
        match self.document.redo_with_changes() {
            Ok(Some(changes)) => {
                self.push_to_backend(changes, false);
                self.error = None;
                cx.emit(BufferEvent::Changed);
                cx.notify();
                true
            }
            Ok(None) => false,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.emit(BufferEvent::Error(error.to_string()));
                cx.notify();
                false
            }
        }
    }

    fn push_to_backend(&mut self, changes: Vec<Change>, workspace_edit: bool) {
        let Some(host_doc) = self.host_doc else {
            return;
        };
        if changes.is_empty() {
            return;
        }
        let change_set = ChangeSet {
            changes: changes
                .iter()
                .map(|change| TextChange {
                    from: change.from,
                    to: change.to,
                    insert: change.insert.to_string(),
                })
                .collect(),
        };
        let _ = workspace_edit;
        match self
            .host
            .apply_document_edit(host_doc, self.backend_version, change_set)
        {
            Ok(version) => self.backend_version = version,
            Err(error) => {
                self.error = Some(format!(
                    "Backend edit rejected ({}); reloading the document",
                    error.message
                ));
                self.reload_from_backend();
            }
        }
    }

    pub fn apply_backend_patch(
        &mut self,
        base_version: u32,
        version: u32,
        changes: Vec<TextChange>,
        cx: &mut Context<Self>,
    ) {
        let changes: Vec<Change> = changes
            .into_iter()
            .map(|change| Change {
                from: change.from,
                to: change.to,
                insert: Arc::from(change.insert),
            })
            .collect();
        match self.document.apply_backend_patch(base_version, changes) {
            Ok(_) => {
                self.backend_version = version;
                self.error = None;
                cx.emit(BufferEvent::Changed);
                cx.notify();
            }
            Err(_) => {
                self.reload_from_backend();
                cx.emit(BufferEvent::Reloaded);
                cx.notify();
            }
        }
    }

    pub fn set_backend_dirty(&mut self, dirty: bool, cx: &mut Context<Self>) {
        self.document.set_dirty(dirty);
        cx.emit(BufferEvent::DirtyChanged);
        cx.notify();
    }

    pub fn mark_saved_version(&mut self, version: u32, cx: &mut Context<Self>) {
        if self.document.mark_saved_if_version(version) {
            cx.emit(BufferEvent::Saved);
        } else {
            self.document.set_dirty(self.document.is_dirty());
            cx.emit(BufferEvent::DirtyChanged);
        }
        cx.notify();
    }

    pub fn reload_from_backend(&mut self) {
        let Some(host_doc) = self.host_doc else {
            return;
        };
        match self.host.document_info(host_doc) {
            Ok(info) => {
                self.document
                    .replace_from_backend(&info.text, info.version, info.dirty);
                self.backend_version = info.version;
                self.language = info.language.to_string();
                self.read_only = info.read_only;
                self.large = info.large;
                if let Some(path) = self.path.as_deref() {
                    self.disk_stamp = DiskStamp::of(path);
                }
                self.error = None;
            }
            Err(error) => self.error = Some(error.message),
        }
    }
}

fn normalize_host_path(path: PathBuf) -> PathBuf {
    if path.to_string_lossy().starts_with("untitled:") {
        path
    } else {
        crate::document_store::normalize_path(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_stamp_changes_on_write_and_deletion() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("file.txt");
        std::fs::write(&path, "one\n").unwrap();
        let first = DiskStamp::of(&path);
        assert!(first.exists);
        std::fs::write(&path, "a much longer value\n").unwrap();
        let second = DiskStamp::of(&path);
        assert!(second.exists);
        assert_ne!(first, second);
        std::fs::remove_file(&path).unwrap();
        let gone = DiskStamp::of(&path);
        assert!(!gone.exists);
        assert_ne!(second, gone);
    }
}
