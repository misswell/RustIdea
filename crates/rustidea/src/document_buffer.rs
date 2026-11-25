use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::{Context, EventEmitter, Timer};
use ide_core::{ChangeSet, DocId, TextChange};
use ide_workspace::DocumentInfo;
use rustidea_editor::{Change, Document, DocumentId, EditSource, Transaction};

use crate::native_host::NativeHost;

const MAX_HIGHLIGHT_LINES: u32 = 4_000;
const MAX_HIGHLIGHT_UTF16: u32 = 512 * 1024;
const SYNC_DEBOUNCE: Duration = Duration::from_millis(50);

#[derive(Clone, Debug)]
pub enum BufferEvent {
    Changed,
    DirtyChanged,
    Saved,
    TitleChanged,
    Reloaded,
    ExternalChanged,
    ExternalResolved,
    SyncChanged,
    SyncConflict,
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    Synced,
    Dirty,
    Syncing,
    Conflict,
    Error,
}

struct InflightSync {
    base_text: Arc<str>,
    sent_text: Arc<str>,
    sent_local_version: u32,
    changes: ChangeSet,
}

struct SyncConflict {
    backend_version: u32,
    backend_text: Arc<str>,
    message: String,
}

enum PendingSave {
    Save,
    SaveAs(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiskStamp {
    pub exists: bool,
    pub mtime_ms: f64,
    pub len: u64,
    pub content_hash: Option<[u8; 32]>,
}

impl DiskStamp {
    pub const fn empty() -> Self {
        Self {
            exists: false,
            mtime_ms: 0.0,
            len: 0,
            content_hash: None,
        }
    }

    pub fn of(path: &Path) -> Self {
        match std::fs::metadata(path) {
            Ok(metadata) => Self {
                exists: true,
                mtime_ms: ide_vfs::mtime_ms(&metadata),
                len: metadata.len(),
                content_hash: None,
            },
            Err(_) => Self {
                exists: false,
                mtime_ms: 0.0,
                len: 0,
                content_hash: None,
            },
        }
    }

    /// Blocking full-content check. Callers must run it off the GPUI thread.
    pub fn read_with_hash(path: &Path) -> Self {
        let mut stamp = Self::of(path);
        if stamp.exists {
            stamp.content_hash = hash_file(path);
        }
        stamp
    }

    pub fn metadata_matches(&self, other: &Self) -> bool {
        self.exists == other.exists && self.len == other.len && self.mtime_ms == other.mtime_ms
    }

    #[cfg(test)]
    pub fn same_content(&self, other: &Self) -> bool {
        match (self.content_hash, other.content_hash) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
    }

    pub fn content_changed(&self, other: &Self) -> bool {
        if !self.exists || !other.exists {
            return self.exists != other.exists;
        }
        match (self.content_hash, other.content_hash) {
            (Some(left), Some(right)) => left != right,
            _ => !self.metadata_matches(other),
        }
    }
}

fn hash_file(path: &Path) -> Option<[u8; 32]> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(hasher.finalize().into())
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
    disk_hash_inflight: bool,
    disk_baseline_ready: bool,
    sync_reload_inflight: bool,
    sync_status: SyncStatus,
    base_text: Option<Arc<str>>,
    diff_inflight: bool,
    inflight: Option<InflightSync>,
    sync_conflict: Option<SyncConflict>,
    sync_generation: u64,
    sync_timer_active: bool,
    ignore_patches_through: u32,
    pending_save: Option<PendingSave>,
}

impl EventEmitter<BufferEvent> for DocumentBuffer {}

impl DocumentBuffer {
    pub fn from_host(host: Arc<NativeHost>, info: DocumentInfo) -> Self {
        let path = match &normalize_host_path(info.path.0.clone()) {
            path if path.to_string_lossy().starts_with("untitled:") => None,
            path => Some(path.clone()),
        };
        let disk_stamp = DiskStamp::empty();
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
            disk_hash_inflight: false,
            disk_baseline_ready: false,
            sync_reload_inflight: false,
            sync_status: SyncStatus::Synced,
            base_text: None,
            diff_inflight: false,
            inflight: None,
            sync_conflict: None,
            sync_generation: 0,
            sync_timer_active: false,
            ignore_patches_through: 0,
            pending_save: None,
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

    pub fn sync_status(&self) -> SyncStatus {
        self.sync_status
    }

    pub fn has_pending_sync(&self) -> bool {
        self.base_text.is_some() || self.inflight.is_some()
    }

    pub fn has_sync_conflict(&self) -> bool {
        self.sync_conflict.is_some()
    }

    pub fn sync_conflict_message(&self) -> Option<&str> {
        self.sync_conflict
            .as_ref()
            .map(|conflict| conflict.message.as_str())
    }

    pub fn conflict_message(&self) -> Option<String> {
        self.sync_conflict_message()
            .map(str::to_owned)
            .or_else(|| self.external_change_message())
    }

    pub fn has_conflict(&self) -> bool {
        self.has_sync_conflict() || self.has_external_change()
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

    pub fn start_disk_baseline_hash(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        if self.disk_stamp.content_hash.is_some() || self.disk_hash_inflight {
            return;
        }
        self.spawn_disk_check(path, true, cx);
    }

    /// Reliable content check for watcher events that preserve size and mtime.
    pub fn check_external_change_with_hash(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        if self.disk_hash_inflight {
            return;
        }
        self.spawn_disk_check(path, false, cx);
    }

    fn spawn_disk_check(&mut self, path: PathBuf, baseline_only: bool, cx: &mut Context<Self>) {
        self.disk_hash_inflight = true;
        let runtime = self.host.runtime().clone();
        cx.spawn(async move |this, cx| {
            let current = runtime
                .spawn_blocking(move || DiskStamp::read_with_hash(&path))
                .await
                .unwrap_or(DiskStamp {
                    exists: false,
                    mtime_ms: 0.0,
                    len: 0,
                    content_hash: None,
                });
            this.update(cx, |buffer, cx| {
                buffer.disk_hash_inflight = false;
                buffer.apply_disk_check(current, baseline_only, cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn apply_disk_check(
        &mut self,
        current: DiskStamp,
        baseline_only: bool,
        cx: &mut Context<Self>,
    ) {
        if baseline_only && !self.disk_stamp.exists {
            self.disk_stamp = current;
            self.disk_baseline_ready = true;
            self.run_pending_save(cx);
            cx.notify();
            return;
        }
        if !current.exists {
            if baseline_only && !self.disk_stamp.exists {
                self.disk_stamp = current;
                return;
            }
            self.external_change = Some(current);
            self.error = self.external_change_message();
            cx.emit(BufferEvent::ExternalChanged);
            cx.notify();
            return;
        }
        if !self.disk_stamp.content_changed(&current) {
            self.disk_stamp = current;
            if baseline_only {
                self.disk_baseline_ready = true;
                self.run_pending_save(cx);
            }
            cx.notify();
            return;
        }
        if self.document.is_dirty() {
            self.external_change = Some(current);
            self.error = self.external_change_message();
            cx.emit(BufferEvent::ExternalChanged);
        } else {
            self.disk_stamp = current;
            self.resync_from_backend(cx);
        }
        cx.notify();
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
        self.disk_stamp = self.external_change.unwrap_or_else(DiskStamp::empty);
        self.external_change = None;
        self.error = None;
        cx.emit(BufferEvent::ExternalResolved);
        self.start_disk_baseline_hash(cx);
        cx.notify();
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        if self.host_doc.is_none() {
            self.error = Some("This buffer has no backend document".into());
            cx.notify();
            return;
        }
        if self.saving {
            return;
        }
        if self.path.is_none() {
            self.error = Some("Use Save As to choose a file".into());
            cx.notify();
            return;
        }
        if self.has_conflict() {
            self.error = Some(
                self.conflict_message()
                    .unwrap_or_else(|| "Resolve the conflict before saving".into()),
            );
            cx.notify();
            return;
        }
        if self.has_pending_sync() {
            self.pending_save = Some(PendingSave::Save);
            self.status_saving(cx);
            self.schedule_sync(cx);
            return;
        }
        self.perform_save(cx);
    }

    fn status_saving(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        if self.external_reload_inflight || self.sync_reload_inflight {
            self.error = Some("Wait for the external reload to finish".into());
        }
        cx.notify();
    }

    fn perform_save(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            self.error = Some("Use Save As to choose a file".into());
            cx.notify();
            return;
        };
        if self.saving {
            return;
        }
        if self.disk_hash_inflight {
            self.pending_save = Some(PendingSave::Save);
            return;
        }
        if !self.disk_baseline_ready {
            self.pending_save = Some(PendingSave::Save);
            self.start_disk_baseline_hash(cx);
            cx.notify();
            return;
        }
        if let Some(message) = self.conflict_message() {
            self.error = Some(message.clone());
            cx.emit(BufferEvent::Error(message));
            cx.notify();
            return;
        }
        let stored = self.disk_stamp;
        let runtime = self.host.runtime().clone();
        self.saving = true;
        self.error = None;
        cx.spawn(async move |this, cx| {
            let current = runtime
                .spawn_blocking(move || DiskStamp::read_with_hash(&path))
                .await
                .unwrap_or(DiskStamp {
                    exists: false,
                    mtime_ms: 0.0,
                    len: 0,
                    content_hash: None,
                });
            this.update(cx, |buffer, cx| {
                buffer.saving = false;
                if stored.content_changed(&current) {
                    buffer.external_change = Some(current);
                    buffer.error = buffer.external_change_message();
                    cx.emit(BufferEvent::ExternalChanged);
                    cx.notify();
                    return;
                }
                buffer.disk_stamp = current;
                if buffer.has_pending_sync() {
                    buffer.pending_save = Some(PendingSave::Save);
                    buffer.schedule_sync(cx);
                    cx.notify();
                    return;
                }
                buffer.perform_save_verified(cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn perform_save_verified(&mut self, cx: &mut Context<Self>) {
        let Some(host_doc) = self.host_doc else {
            self.error = Some("This buffer has no backend document".into());
            cx.notify();
            return;
        };
        if let Some(message) = self.conflict_message() {
            self.error = Some(message.clone());
            cx.emit(BufferEvent::Error(message));
            cx.notify();
            return;
        }
        if self.external_reload_inflight || self.sync_reload_inflight {
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
                        buffer.disk_stamp = DiskStamp::empty();
                        buffer.mark_saved_version(version, cx);
                        buffer.start_disk_baseline_hash(cx);
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
        if self.host_doc.is_none() {
            self.error = Some("This buffer has no backend document".into());
            cx.notify();
            return;
        }
        if self.saving {
            self.error = Some("Wait for the current save before using Save As".into());
            cx.notify();
            return;
        }
        if self.has_conflict() {
            self.error = Some(
                self.conflict_message()
                    .unwrap_or_else(|| "Resolve the conflict before saving".into()),
            );
            cx.notify();
            return;
        }
        if self.has_pending_sync() {
            self.pending_save = Some(PendingSave::SaveAs(path));
            self.status_saving(cx);
            self.schedule_sync(cx);
            return;
        }
        self.perform_save_as(path, cx);
    }

    fn perform_save_as(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(host_doc) = self.host_doc else {
            self.error = Some("This buffer has no backend document".into());
            cx.notify();
            return;
        };
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
                        buffer.disk_stamp = DiskStamp::empty();
                        buffer.external_change = None;
                        buffer.mark_saved_version(version, cx);
                        if buffer.document.version() == version {
                            buffer.document.set_version(info.version);
                        }
                        buffer.start_disk_baseline_hash(cx);
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

    pub fn highlight_context(&self) -> Option<(Arc<NativeHost>, ide_core::LanguageId, String)> {
        if self.large
            || self.language.is_empty()
            || self.language == "plaintext"
            || self.document.line_count() > MAX_HIGHLIGHT_LINES
            || self.document.len_utf16() > MAX_HIGHLIGHT_UTF16
        {
            return None;
        }
        Some((
            self.host.clone(),
            ide_core::LanguageId(self.language.clone()),
            self.document.text(),
        ))
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

    pub fn offset_to_byte(&self, offset: u32) -> usize {
        self.document.offset_to_byte(offset)
    }

    pub fn byte_to_offset(&self, byte: usize) -> u32 {
        self.document.byte_to_offset(byte)
    }

    pub fn slice(&self, from: u32, to: u32) -> String {
        self.document.slice(from, to)
    }

    /// UTF-16 identifier range around a caret on one line. Completion uses
    /// this as the fallback replacement range when a server sends none.
    pub fn word_range_at(&self, offset: u32) -> std::ops::Range<u32> {
        let (line, col) = self.offset_to_line_col(offset);
        let line_start = self.line_start(line);
        self.document
            .line(line)
            .map(|text| word_range_in_line(&text, line_start, col))
            .unwrap_or(offset..offset)
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
        let base_before_edit = if self.base_text.is_none() {
            Some(Arc::<str>::from(self.document.text()))
        } else {
            None
        };
        let version = self.document.version();
        let change = Change::replace(range.clone(), text);
        match self
            .document
            .apply(Transaction::new(version, vec![change.clone()], source))
        {
            Ok(_) => {
                if let Some(base) = base_before_edit {
                    self.base_text = Some(base);
                }
                self.error = None;
                self.set_sync_status(SyncStatus::Dirty, cx);
                cx.emit(BufferEvent::Changed);
                self.schedule_sync(cx);
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
        let base_before_edit = if self.base_text.is_none() {
            Some(Arc::<str>::from(self.document.text()))
        } else {
            None
        };
        match self.document.undo_with_changes() {
            Ok(Some(_)) => {
                if let Some(base) = base_before_edit {
                    self.base_text = Some(base);
                }
                self.error = None;
                self.set_sync_status(SyncStatus::Dirty, cx);
                cx.emit(BufferEvent::Changed);
                self.schedule_sync(cx);
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
        let base_before_edit = if self.base_text.is_none() {
            Some(Arc::<str>::from(self.document.text()))
        } else {
            None
        };
        match self.document.redo_with_changes() {
            Ok(Some(_)) => {
                if let Some(base) = base_before_edit {
                    self.base_text = Some(base);
                }
                self.error = None;
                self.set_sync_status(SyncStatus::Dirty, cx);
                cx.emit(BufferEvent::Changed);
                self.schedule_sync(cx);
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

    fn set_sync_status(&mut self, status: SyncStatus, cx: &mut Context<Self>) {
        if self.sync_status != status {
            self.sync_status = status;
            cx.emit(BufferEvent::SyncChanged);
        }
    }

    fn schedule_sync(&mut self, cx: &mut Context<Self>) {
        if self.host_doc.is_none()
            || self.base_text.is_none()
            || self.inflight.is_some()
            || self.sync_conflict.is_some()
        {
            return;
        }
        self.sync_generation = self.sync_generation.wrapping_add(1);
        if self.sync_timer_active {
            return;
        }
        self.sync_timer_active = true;
        let generation = self.sync_generation;
        cx.spawn(async move |this, cx| {
            Timer::after(SYNC_DEBOUNCE).await;
            this.update(cx, |buffer, cx| {
                buffer.sync_timer_active = false;
                if buffer.sync_generation != generation {
                    buffer.schedule_sync(cx);
                    return;
                }
                buffer.start_sync(cx);
            })
            .ok();
        })
        .detach();
    }

    fn start_sync(&mut self, cx: &mut Context<Self>) {
        if self.inflight.is_some() || self.diff_inflight || self.sync_conflict.is_some() {
            return;
        }
        let Some(base) = self.base_text.clone() else {
            return;
        };
        let local_text = self.document.text();
        let local_version = self.document.version();
        let sent_text = Arc::<str>::from(local_text);
        let diff_text = sent_text.clone();
        let diff_base = base.clone();
        let runtime = self.host.runtime().clone();
        self.diff_inflight = true;
        cx.spawn(async move |this, cx| {
            let changes = runtime
                .spawn_blocking(move || {
                    crate::document_sync::coalesced_diff(&diff_base, &diff_text)
                })
                .await
                .unwrap_or_default();
            this.update(cx, |buffer, cx| {
                buffer.diff_inflight = false;
                if buffer.sync_conflict.is_some()
                    || buffer.base_text.as_deref() != Some(base.as_ref())
                {
                    buffer.schedule_sync(cx);
                    return;
                }
                buffer.submit_sync(base, sent_text, local_version, changes, cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn submit_sync(
        &mut self,
        base: Arc<str>,
        sent_text: Arc<str>,
        sent_local_version: u32,
        changes: ChangeSet,
        cx: &mut Context<Self>,
    ) {
        if changes.changes.is_empty() {
            self.base_text = None;
            self.document.set_version(self.backend_version);
            self.set_sync_status(SyncStatus::Synced, cx);
            self.run_pending_save(cx);
            cx.notify();
            return;
        }
        let Some(host_doc) = self.host_doc else {
            return;
        };
        self.inflight = Some(InflightSync {
            base_text: base,
            sent_text,
            sent_local_version,
            changes: changes.clone(),
        });
        self.set_sync_status(SyncStatus::Syncing, cx);
        let host = self.host.clone();
        let base_version = self.backend_version;
        cx.spawn(async move |this, cx| {
            let result = host
                .apply_document_edit_async(host_doc, base_version, changes)
                .await;
            this.update(cx, |buffer, cx| buffer.finish_sync(result, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn finish_sync(&mut self, result: ide_core::IdeResult<u32>, cx: &mut Context<Self>) {
        let Some(inflight) = self.inflight.take() else {
            return;
        };
        match result {
            Ok(version) => {
                self.backend_version = version;
                self.ignore_patches_through = self.ignore_patches_through.max(version);
                self.error = None;
                if self.document.version() == inflight.sent_local_version {
                    self.base_text = None;
                    self.document.set_version(version);
                    self.set_sync_status(SyncStatus::Synced, cx);
                    self.run_pending_save(cx);
                } else {
                    self.base_text = Some(inflight.sent_text);
                    self.set_sync_status(SyncStatus::Dirty, cx);
                    self.schedule_sync(cx);
                }
                cx.emit(BufferEvent::DirtyChanged);
            }
            Err(error) if error.kind == ide_core::error::ErrorKind::OutOfSync => {
                self.base_text = Some(inflight.base_text);
                self.set_sync_status(SyncStatus::Dirty, cx);
                self.resync_from_backend(cx);
            }
            Err(error) => {
                self.base_text = Some(inflight.base_text);
                self.error = Some(error.message.clone());
                self.set_sync_status(SyncStatus::Error, cx);
                cx.emit(BufferEvent::Error(error.message));
            }
        }
        cx.notify();
    }

    fn resync_from_backend(&mut self, cx: &mut Context<Self>) {
        let Some(host_doc) = self.host_doc else {
            return;
        };
        if self.sync_reload_inflight {
            return;
        }
        self.sync_reload_inflight = true;
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.document_info_async(host_doc).await;
            this.update(cx, |buffer, cx| {
                buffer.sync_reload_inflight = false;
                buffer.finish_resync(result, cx);
            })
            .ok();
        })
        .detach();
    }

    pub fn force_resync(&mut self, cx: &mut Context<Self>) {
        self.resync_from_backend(cx);
    }

    fn finish_resync(&mut self, result: ide_core::IdeResult<DocumentInfo>, cx: &mut Context<Self>) {
        let info = match result {
            Ok(info) => info,
            Err(error) => {
                self.error = Some(error.message.clone());
                self.set_sync_status(SyncStatus::Error, cx);
                cx.emit(BufferEvent::Error(error.message));
                cx.notify();
                return;
            }
        };
        let local_text = self.document.text();
        let local_version = self.document.version();
        let backend_text = Arc::<str>::from(info.text.as_str());
        let merged = if let Some(base) = self.base_text.as_deref() {
            match crate::document_sync::merge_three_way(base, &local_text, &info.text) {
                crate::document_sync::MergeOutcome::Merged(text) => Some(text),
                crate::document_sync::MergeOutcome::Conflict => None,
            }
        } else {
            Some(info.text.clone())
        };
        let Some(merged) = merged else {
            self.sync_conflict = Some(SyncConflict {
                backend_version: info.version,
                backend_text,
                message: format!(
                    "{} has concurrent backend changes that cannot be merged safely. \
                     Keep your version or reload the backend text.",
                    self.path
                        .as_deref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| self.title())
                ),
            });
            self.set_sync_status(SyncStatus::Conflict, cx);
            cx.emit(BufferEvent::SyncConflict);
            cx.notify();
            return;
        };
        if merged != local_text {
            let diff = crate::document_sync::coalesced_diff(&local_text, &merged);
            let changes = diff
                .changes
                .into_iter()
                .map(|change| Change {
                    from: change.from,
                    to: change.to,
                    insert: Arc::from(change.insert),
                })
                .collect();
            if let Err(error) = self.document.apply(Transaction::new(
                local_version,
                changes,
                EditSource::External,
            )) {
                self.error = Some(error.to_string());
                self.set_sync_status(SyncStatus::Error, cx);
                cx.emit(BufferEvent::Error(error.to_string()));
                cx.notify();
                return;
            }
        }
        self.document.set_version(info.version);
        self.backend_version = info.version;
        self.ignore_patches_through = self.ignore_patches_through.max(info.version);
        self.error = None;
        if self.base_text.is_some() {
            self.base_text = Some(backend_text);
            self.set_sync_status(SyncStatus::Dirty, cx);
            cx.emit(BufferEvent::Changed);
            self.schedule_sync(cx);
        } else {
            self.set_sync_status(SyncStatus::Synced, cx);
            cx.emit(BufferEvent::Reloaded);
        }
        cx.notify();
    }

    pub fn apply_backend_patch(
        &mut self,
        base_version: u32,
        version: u32,
        changes: Vec<TextChange>,
        cx: &mut Context<Self>,
    ) {
        if version <= self.ignore_patches_through {
            return;
        }
        if let Some(inflight) = self.inflight.as_ref()
            && base_version == self.backend_version
            && inflight.changes.changes == changes
        {
            return;
        }
        if base_version != self.backend_version {
            self.set_sync_status(SyncStatus::Dirty, cx);
            self.resync_from_backend(cx);
            return;
        }
        let Some(base) = self.base_text.clone() else {
            let local_version = self.document.version();
            let changes = changes
                .into_iter()
                .map(|change| Change {
                    from: change.from,
                    to: change.to,
                    insert: Arc::from(change.insert),
                })
                .collect();
            match self.document.apply(Transaction::new(
                local_version,
                changes,
                EditSource::External,
            )) {
                Ok(_) => {
                    self.backend_version = version;
                    self.ignore_patches_through = self.ignore_patches_through.max(version);
                    self.document.set_version(version);
                    self.error = None;
                    self.set_sync_status(SyncStatus::Synced, cx);
                    cx.emit(BufferEvent::Changed);
                    cx.notify();
                }
                Err(_) => self.resync_from_backend(cx),
            }
            return;
        };
        let backend_text = match text_after_changes(&base, &changes) {
            Some(text) => text,
            None => {
                self.resync_from_backend(cx);
                return;
            }
        };
        let local_text = self.document.text();
        let merged = match crate::document_sync::merge_three_way(&base, &local_text, &backend_text)
        {
            crate::document_sync::MergeOutcome::Merged(text) => text,
            crate::document_sync::MergeOutcome::Conflict => {
                self.sync_conflict = Some(SyncConflict {
                    backend_version: version,
                    backend_text: Arc::from(backend_text),
                    message: format!(
                        "{} received concurrent backend changes while local edits were pending. \
                         Keep your version or reload the backend text.",
                        self.path
                            .as_deref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| self.title())
                    ),
                });
                self.set_sync_status(SyncStatus::Conflict, cx);
                cx.emit(BufferEvent::SyncConflict);
                cx.notify();
                return;
            }
        };
        if merged != local_text {
            let local_version = self.document.version();
            let diff = crate::document_sync::coalesced_diff(&local_text, &merged);
            let edits = diff
                .changes
                .into_iter()
                .map(|change| Change {
                    from: change.from,
                    to: change.to,
                    insert: Arc::from(change.insert),
                })
                .collect();
            if self
                .document
                .apply(Transaction::new(local_version, edits, EditSource::External))
                .is_err()
            {
                self.resync_from_backend(cx);
                return;
            }
        }
        self.base_text = Some(Arc::from(backend_text));
        self.backend_version = version;
        self.ignore_patches_through = self.ignore_patches_through.max(version);
        self.set_sync_status(SyncStatus::Dirty, cx);
        cx.emit(BufferEvent::Changed);
        self.schedule_sync(cx);
        cx.notify();
    }

    pub fn resolve_sync_conflict_keep_local(&mut self, cx: &mut Context<Self>) {
        let Some(conflict) = self.sync_conflict.take() else {
            return;
        };
        self.base_text = Some(conflict.backend_text);
        self.backend_version = conflict.backend_version;
        self.ignore_patches_through = self.ignore_patches_through.max(conflict.backend_version);
        self.error = None;
        self.set_sync_status(SyncStatus::Dirty, cx);
        self.schedule_sync(cx);
        cx.notify();
    }

    pub fn resolve_sync_conflict_reload_backend(&mut self, cx: &mut Context<Self>) {
        let Some(conflict) = self.sync_conflict.take() else {
            return;
        };
        self.document
            .replace_from_backend(&conflict.backend_text, conflict.backend_version, false);
        self.backend_version = conflict.backend_version;
        self.ignore_patches_through = self.ignore_patches_through.max(conflict.backend_version);
        self.base_text = None;
        self.error = None;
        self.set_sync_status(SyncStatus::Synced, cx);
        cx.emit(BufferEvent::Reloaded);
        cx.notify();
    }

    pub fn resolve_conflict_keep_local(&mut self, cx: &mut Context<Self>) {
        if self.has_sync_conflict() {
            self.resolve_sync_conflict_keep_local(cx);
        } else {
            self.keep_local_after_conflict(cx);
        }
    }

    pub fn resolve_conflict_reload(&mut self, cx: &mut Context<Self>) {
        if self.has_sync_conflict() {
            self.resolve_sync_conflict_reload_backend(cx);
        } else {
            self.reload_after_conflict(cx);
        }
    }

    fn run_pending_save(&mut self, cx: &mut Context<Self>) {
        match self.pending_save.take() {
            Some(PendingSave::Save) => self.perform_save(cx),
            Some(PendingSave::SaveAs(path)) => self.perform_save_as(path, cx),
            None => {}
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
                self.disk_stamp = DiskStamp::empty();
                self.error = None;
            }
            Err(error) => self.error = Some(error.message),
        }
    }
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

fn word_range_in_line(line: &str, line_start: u32, col: u32) -> std::ops::Range<u32> {
    let mut utf16 = 0u32;
    let mut byte = line.len();
    for (index, ch) in line.char_indices() {
        let width = ch.len_utf16() as u32;
        if utf16 + width > col {
            byte = index;
            break;
        }
        utf16 += width;
        if utf16 == col {
            byte = index + ch.len_utf8();
            break;
        }
    }
    if !line[..byte]
        .chars()
        .next_back()
        .is_some_and(is_identifier_char)
    {
        let caret = line_start + line[..byte].encode_utf16().count() as u32;
        return caret..caret;
    }
    let mut start = byte;
    for (index, ch) in line[..byte].char_indices().rev() {
        if !is_identifier_char(ch) {
            break;
        }
        start = index;
    }
    let mut end = byte;
    for (index, ch) in line[byte..].char_indices() {
        if !is_identifier_char(ch) {
            break;
        }
        end = byte + index + ch.len_utf8();
    }
    let start = line_start + line[..start].encode_utf16().count() as u32;
    let end = line_start + line[..end].encode_utf16().count() as u32;
    start..end
}

fn normalize_host_path(path: PathBuf) -> PathBuf {
    if path.to_string_lossy().starts_with("untitled:") {
        path
    } else {
        crate::document_store::lexical_path(&path)
    }
}

fn text_after_changes(base: &str, changes: &[TextChange]) -> Option<String> {
    let mut document = ide_text::Document::new(base);
    document
        .apply(
            0,
            &ChangeSet {
                changes: changes.to_vec(),
            },
        )
        .ok()?;
    Some(document.text())
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

    #[test]
    fn content_hash_detects_same_length_rewrites() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("same-length.txt");
        std::fs::write(&path, "one\n").unwrap();
        let first = DiskStamp::read_with_hash(&path);
        std::fs::write(&path, "two\n").unwrap();
        let second = DiskStamp::read_with_hash(&path);
        assert_eq!(first.len, second.len);
        assert!(first.content_changed(&second));
        assert!(!first.same_content(&second));
    }

    #[test]
    fn word_range_uses_utf16_and_unicode_identifiers() {
        assert_eq!(word_range_in_line("let value = 1;", 0, 5), 4..9);
        assert_eq!(word_range_in_line("let value = 1;", 0, 8), 4..9);
        assert_eq!(word_range_in_line("let 中文 = 1;", 0, 5), 4..6);
        assert_eq!(word_range_in_line("foo bar", 0, 3), 0..3);
        assert_eq!(word_range_in_line("foo bar", 0, 4), 4..4);
        assert_eq!(word_range_in_line("😀name", 0, 3), 2..6);
    }
}
