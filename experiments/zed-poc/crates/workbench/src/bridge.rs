use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash as _, Hasher as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use gpui::{AppContext as _, Context, Entity};
use ide_core::{ChangeSet, DocId};
use ide_workspace::DocumentInfo;
use language::{Buffer, BufferEvent};
use text::{OffsetUtf16, PointUtf16};

pub struct ZedDocument {
    pub doc: DocId,
    pub path: PathBuf,
    pub language: String,
    pub buffer: Entity<Buffer>,
    pub backend_version: u32,
    pub last_clock: clock::Global,
    pub confirmed_clock: clock::Global,
    pub syncing: bool,
    pub applying_backend_patch: bool,
    pub save_requested: bool,
    pub save_force_requested: bool,
    pub conflict: Option<String>,
    pub disk_stamp: Option<DiskStamp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskStamp {
    len: u64,
    hash: u64,
}

impl DiskStamp {
    pub fn read(path: &Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        Some(Self {
            len: bytes.len() as u64,
            hash: hasher.finish(),
        })
    }

    pub fn is_current(&self, path: &Path) -> bool {
        Self::read(path).as_ref() == Some(self)
    }
}

impl ZedDocument {
    pub fn open<T: 'static>(
        info: DocumentInfo,
        language_registry: Arc<language::LanguageRegistry>,
        cx: &mut Context<T>,
    ) -> Self {
        let text = info.text.clone();
        let buffer = cx.new(|cx| Buffer::local(text, cx));
        let version = buffer.read(cx).snapshot().version().clone();
        let language = info.language.to_string();
        let registry = language_registry;
        let language_name = language.clone();
        let language_buffer = buffer.clone();
        cx.spawn(async move |_this, cx| {
            match registry
                .language_for_name_or_extension(&language_name)
                .await
            {
                Ok(language) => {
                    language_buffer.update(cx, |buffer, cx| {
                        buffer.set_language(Some(language), cx);
                    });
                }
                Err(error) => {
                    eprintln!("WORKBENCH_LANGUAGE_ERROR {language_name}: {error:#}");
                }
            }
        })
        .detach();
        let path = info.path.0;
        let disk_stamp = DiskStamp::read(&path);
        Self {
            doc: info.id,
            path,
            language,
            buffer,
            backend_version: info.version,
            last_clock: version.clone(),
            confirmed_clock: version,
            syncing: false,
            applying_backend_patch: false,
            save_requested: false,
            save_force_requested: false,
            conflict: None,
            disk_stamp,
        }
    }

    pub fn text(&self, cx: &gpui::App) -> String {
        self.buffer.read(cx).snapshot().text()
    }

    pub fn is_dirty(&self, cx: &gpui::App) -> bool {
        self.buffer.read(cx).is_dirty()
    }

    pub fn pending_changes(&mut self, cx: &mut gpui::App) -> Option<(clock::Global, ChangeSet)> {
        let snapshot = self.buffer.read(cx).snapshot();
        let edits: Vec<text::Edit<OffsetUtf16>> = snapshot
            .edits_since::<OffsetUtf16>(&self.last_clock)
            .collect();
        if edits.is_empty() {
            return None;
        }
        let changes = edits
            .iter()
            .map(|edit| ide_core::TextChange {
                from: edit.old.start.0 as u32,
                to: edit.old.end.0 as u32,
                insert: snapshot
                    .text_for_range(
                        snapshot.offset_utf16_to_offset(edit.new.start)
                            ..snapshot.offset_utf16_to_offset(edit.new.end),
                    )
                    .collect(),
            })
            .collect();
        let request_clock = snapshot.version().clone();
        self.last_clock = request_clock.clone();
        self.syncing = true;
        Some((request_clock, ChangeSet { changes }))
    }

    pub fn apply_backend_patch(
        &mut self,
        version: u32,
        changes: &[ide_core::TextChange],
        cx: &mut gpui::App,
    ) -> Result<()> {
        let snapshot = self.buffer.read(cx).snapshot();
        if snapshot.version() != &self.confirmed_clock {
            self.conflict = Some(
                "External edit arrived while the Zed buffer had unconfirmed local edits".into(),
            );
            return Ok(());
        }
        let edits: Vec<(std::ops::Range<usize>, String)> = changes
            .iter()
            .map(|change| {
                (
                    snapshot.offset_utf16_to_offset(OffsetUtf16(change.from as usize))
                        ..snapshot.offset_utf16_to_offset(OffsetUtf16(change.to as usize)),
                    change.insert.clone(),
                )
            })
            .collect();
        self.applying_backend_patch = true;
        self.buffer.update(cx, |buffer, cx| {
            buffer.edit(edits, None, cx);
        });
        self.applying_backend_patch = false;
        self.backend_version = version;
        self.confirmed_clock = self.buffer.read(cx).snapshot().version().clone();
        self.last_clock = self.confirmed_clock.clone();
        self.conflict = None;
        Ok(())
    }

    pub fn position(&self, line: u32, column: u32, cx: &gpui::App) -> Option<text::Anchor> {
        let snapshot = self.buffer.read(cx).snapshot();
        Some(snapshot.anchor_before(PointUtf16::new(line, column)))
    }

    pub fn disk_changed(&self) -> bool {
        self.disk_stamp
            .as_ref()
            .is_some_and(|stamp| !stamp.is_current(&self.path))
    }

    pub fn refresh_disk_stamp(&mut self) {
        self.disk_stamp = DiskStamp::read(&self.path);
    }
}

pub fn is_buffer_edited(event: &BufferEvent) -> bool {
    matches!(event, BufferEvent::Edited { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_stamp_detects_a_same_length_external_edit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(&path, "abc\n").unwrap();
        let stamp = DiskStamp::read(&path).unwrap();
        assert!(stamp.is_current(&path));
        std::fs::write(&path, "abd\n").unwrap();
        assert!(!stamp.is_current(&path));
    }
}
