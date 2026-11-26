use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use collections::{HashMap, HashSet};
use editor::{GotoDefinitionKind, RenameTarget, SemanticsProvider};
use futures::future::Shared;
use gpui::{App, AppContext as _, Entity, Task};
use language::{Anchor, Buffer, BufferRow, language_settings::InlayHintKind};
use project::lsp_store::{BufferSemanticTokens, CacheInlayHints, InvalidationStrategy};
use project::{
    DocumentHighlight, Hover, HoverBlock, HoverBlockKind, InlayHint, InlayHintLabel,
    InlayHintLabelPart, InlayHintLabelPartTooltip, InlayHintTooltip, InlayId, LocationLink,
    ProjectTransaction, ResolveState,
};
use text::ToOffset;

use crate::lsp::DocumentLsp;

/// Maps Zed's semantics requests to OpenIdea LSP operations. Hover is wired in
/// this POC; the remaining required surfaces return empty values explicitly.
pub struct OpenIdeaSemanticsProvider {
    backend: Arc<DocumentLsp>,
    next_inlay_id: Arc<AtomicUsize>,
}

impl OpenIdeaSemanticsProvider {
    pub fn new(backend: Arc<DocumentLsp>) -> Self {
        Self {
            backend,
            next_inlay_id: Arc::new(AtomicUsize::new(1)),
        }
    }

    fn position(&self, buffer: &Entity<Buffer>, position: Anchor, cx: &App) -> (u32, u32) {
        let snapshot = buffer.read(cx).snapshot();
        let byte = position.to_offset(&snapshot);
        let point = snapshot.offset_to_point_utf16(byte);
        (point.row, point.column)
    }
}

impl SemanticsProvider for OpenIdeaSemanticsProvider {
    fn hover(
        &self,
        buffer: &Entity<Buffer>,
        position: Anchor,
        cx: &mut App,
    ) -> Option<Task<Option<Vec<Hover>>>> {
        let (line, column) = self.position(buffer, position, cx);
        let backend = self.backend.clone();
        Some(cx.background_spawn(async move {
            backend
                .hover(line, column)
                .await
                .ok()
                .flatten()
                .map(|hover| {
                    vec![Hover {
                        contents: vec![HoverBlock {
                            text: hover.contents.value,
                            kind: if hover.contents.kind == "markdown" {
                                HoverBlockKind::Markdown
                            } else {
                                HoverBlockKind::PlainText
                            },
                        }],
                        range: None,
                        language: None,
                    }]
                })
        }))
    }

    fn inline_values(
        &self,
        _buffer_handle: Entity<Buffer>,
        _range: Range<Anchor>,
        _cx: &mut App,
    ) -> Option<Task<anyhow::Result<Vec<InlayHint>>>> {
        None
    }

    fn applicable_inlay_chunks(
        &self,
        buffer: &Entity<Buffer>,
        ranges: &[Range<Anchor>],
        cx: &mut App,
    ) -> Vec<Range<BufferRow>> {
        let snapshot = buffer.read(cx).snapshot();
        ranges
            .iter()
            .map(|range| {
                let start = snapshot.offset_to_point(range.start.to_offset(&snapshot));
                let end = snapshot.offset_to_point(range.end.to_offset(&snapshot));
                start.row..end.row.saturating_add(1)
            })
            .collect()
    }

    fn invalidate_inlay_hints(&self, _for_buffers: &HashSet<language::BufferId>, _cx: &mut App) {}

    fn inlay_hints(
        &self,
        _invalidate: InvalidationStrategy,
        buffer: Entity<Buffer>,
        ranges: Vec<Range<Anchor>>,
        known_chunks: Option<(clock::Global, HashSet<Range<BufferRow>>)>,
        cx: &mut App,
    ) -> Option<HashMap<Range<BufferRow>, Task<anyhow::Result<CacheInlayHints>>>> {
        let snapshot = buffer.read(cx).snapshot();
        let known = known_chunks.map(|(_, chunks)| chunks).unwrap_or_default();
        let mut requests = HashMap::default();
        for range in ranges {
            let start = snapshot.offset_to_point(range.start.to_offset(&snapshot));
            let end = snapshot.offset_to_point(range.end.to_offset(&snapshot));
            let rows = start.row..end.row.saturating_add(1);
            if known
                .iter()
                .any(|known| known.start <= rows.start && known.end >= rows.end)
            {
                continue;
            }
            let ide_range = ide_core::Range::new(
                ide_core::Pos::new(start.row, start.column),
                ide_core::Pos::new(end.row, end.column),
            );
            let backend = self.backend.clone();
            let snapshot = snapshot.clone();
            let next_id = self.next_inlay_id.clone();
            let task = cx.background_spawn(async move {
                let hints = backend
                    .inlay_hints(ide_range)
                    .await
                    .map_err(|error| anyhow::anyhow!(error.message))?;
                let hints = hints
                    .into_iter()
                    .map(|hint| {
                        let position = snapshot.anchor_before(text::PointUtf16::new(
                            hint.position.line,
                            hint.position.col,
                        ));
                        let label = if hint.parts.is_empty() {
                            InlayHintLabel::String(hint.label)
                        } else {
                            InlayHintLabel::LabelParts(
                                hint.parts
                                    .into_iter()
                                    .map(|part| InlayHintLabelPart {
                                        value: part.value,
                                        tooltip: part
                                            .tooltip
                                            .map(InlayHintLabelPartTooltip::String),
                                        location: None,
                                        command: None,
                                    })
                                    .collect(),
                            )
                        };
                        let id = InlayId::Hint(next_id.fetch_add(1, Ordering::Relaxed));
                        (
                            id,
                            InlayHint {
                                position,
                                label,
                                kind: Some(match hint.kind {
                                    ide_lsp::types::InlayKind::Type => InlayHintKind::Type,
                                    ide_lsp::types::InlayKind::Parameter => {
                                        InlayHintKind::Parameter
                                    }
                                    ide_lsp::types::InlayKind::Other => InlayHintKind::Type,
                                }),
                                padding_left: hint.padding_left,
                                padding_right: hint.padding_right,
                                tooltip: hint.tooltip.map(InlayHintTooltip::String),
                                text_edits: None,
                                resolve_state: ResolveState::Resolved,
                            },
                        )
                    })
                    .collect();
                let mut cache = HashMap::default();
                cache.insert(lsp::LanguageServerId(1), hints);
                Ok(cache)
            });
            requests.insert(rows, task);
        }
        Some(requests)
    }

    fn semantic_tokens(
        &self,
        _buffer: Entity<Buffer>,
        _cx: &mut App,
    ) -> Option<Shared<Task<std::result::Result<BufferSemanticTokens, Arc<anyhow::Error>>>>> {
        None
    }

    fn supports_inlay_hints(&self, _buffer: &Entity<Buffer>, _cx: &mut App) -> bool {
        true
    }

    fn supports_semantic_tokens(&self, _buffer: &Entity<Buffer>, _cx: &mut App) -> bool {
        false
    }

    fn document_highlights(
        &self,
        buffer: &Entity<Buffer>,
        position: Anchor,
        cx: &mut App,
    ) -> Option<Task<anyhow::Result<Vec<DocumentHighlight>>>> {
        let (line, column) = self.position(buffer, position, cx);
        let snapshot = buffer.read(cx).snapshot();
        let backend = self.backend.clone();
        Some(cx.spawn(async move |_cx| {
            let highlights = backend
                .document_highlights(line, column)
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            Ok(highlights
                .into_iter()
                .map(|highlight| DocumentHighlight {
                    range: anchor_range(&snapshot, &highlight.range),
                    kind: match highlight.kind {
                        ide_lsp::types::HighlightKind::Text => lsp::DocumentHighlightKind::TEXT,
                        ide_lsp::types::HighlightKind::Read => lsp::DocumentHighlightKind::READ,
                        ide_lsp::types::HighlightKind::Write => lsp::DocumentHighlightKind::WRITE,
                    },
                })
                .collect())
        }))
    }

    fn definitions(
        &self,
        buffer: &Entity<Buffer>,
        position: Anchor,
        kind: GotoDefinitionKind,
        cx: &mut App,
    ) -> Option<Task<anyhow::Result<Option<Vec<LocationLink>>>>> {
        let (line, column) = self.position(buffer, position, cx);
        let snapshot = buffer.read(cx).snapshot();
        let buffer = buffer.clone();
        let backend = self.backend.clone();
        let path = backend.path.clone();
        let kind = match kind {
            GotoDefinitionKind::Symbol => ide_lsp::types::GotoKind::Definition,
            GotoDefinitionKind::Declaration => ide_lsp::types::GotoKind::Declaration,
            GotoDefinitionKind::Type => ide_lsp::types::GotoKind::TypeDefinition,
            GotoDefinitionKind::Implementation => ide_lsp::types::GotoKind::Implementation,
        };
        Some(cx.spawn(async move |cx| {
            let locations = backend
                .definition(line, column, kind)
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            let mut links = Vec::new();
            for location in locations {
                let same_file = location.path.0 == path;
                let (target_buffer, target_snapshot) = if same_file {
                    (buffer.clone(), snapshot.clone())
                } else {
                    let info = backend
                        .open_target(location.path.0.clone())
                        .await
                        .map_err(|error| anyhow::anyhow!(error.message))?;
                    let target = cx.new(|cx| Buffer::local(info.text, cx));
                    target.update(cx, |buffer, cx| {
                        buffer.set_capability(language::Capability::ReadOnly, cx);
                    });
                    let snapshot = cx.read_entity(&target, |buffer, _| buffer.snapshot());
                    (target, snapshot)
                };
                let start = target_snapshot.anchor_before(text::PointUtf16::new(
                    location.range.start.line,
                    location.range.start.col,
                ));
                let end = target_snapshot.anchor_before(text::PointUtf16::new(
                    location.range.end.line,
                    location.range.end.col,
                ));
                links.push(LocationLink {
                    origin: None,
                    target: language::Location {
                        buffer: target_buffer,
                        range: start..end,
                    },
                });
            }
            Ok(Some(links))
        }))
    }

    fn range_for_rename(
        &self,
        buffer: &Entity<Buffer>,
        position: Anchor,
        cx: &mut App,
    ) -> Task<anyhow::Result<Option<RenameTarget>>> {
        let (line, column) = self.position(buffer, position, cx);
        let snapshot = buffer.read(cx).snapshot();
        let backend = self.backend.clone();
        cx.spawn(async move |_cx| {
            let range = backend
                .prepare_rename(line, column)
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            Ok(range.map(|range| {
                let start = snapshot
                    .anchor_before(text::PointUtf16::new(range.start.line, range.start.col));
                let end =
                    snapshot.anchor_before(text::PointUtf16::new(range.end.line, range.end.col));
                RenameTarget::new(start..end, None)
            }))
        })
    }

    fn perform_rename(
        &self,
        buffer: &Entity<Buffer>,
        position: Anchor,
        new_name: String,
        _language_server_id: Option<lsp::LanguageServerId>,
        cx: &mut App,
    ) -> Option<Task<anyhow::Result<ProjectTransaction>>> {
        let (line, column) = self.position(buffer, position, cx);
        let backend = self.backend.clone();
        Some(cx.spawn(async move |_cx| {
            backend
                .rename(line, column, new_name)
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            Ok(ProjectTransaction::default())
        }))
    }
}

fn anchor_range(snapshot: &language::BufferSnapshot, range: &ide_core::Range) -> Range<Anchor> {
    snapshot.anchor_before(text::PointUtf16::new(range.start.line, range.start.col))
        ..snapshot.anchor_after(text::PointUtf16::new(range.end.line, range.end.col))
}
