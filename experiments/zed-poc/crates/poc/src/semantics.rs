use std::ops::Range;
use std::sync::Arc;

use collections::{HashMap, HashSet};
use editor::{GotoDefinitionKind, RenameTarget, SemanticsProvider};
use futures::future::Shared;
use gpui::{App, AppContext as _, Entity, Task};
use language::{Anchor, Buffer, BufferRow};
use project::lsp_store::{BufferSemanticTokens, CacheInlayHints, InvalidationStrategy};
use project::{
    DocumentHighlight, Hover, HoverBlock, HoverBlockKind, InlayHint, LocationLink,
    ProjectTransaction,
};
use text::ToOffset;

use crate::backend::OpenIdeaBridge;

/// Maps Zed's semantics requests to OpenIdea LSP operations. Hover is wired in
/// this POC; the remaining required surfaces return empty values explicitly.
pub struct OpenIdeaSemanticsProvider {
    backend: Arc<OpenIdeaBridge>,
}

impl OpenIdeaSemanticsProvider {
    pub fn new(backend: Arc<OpenIdeaBridge>) -> Self {
        Self { backend }
    }

    fn position(
        &self,
        buffer: &Entity<Buffer>,
        position: Anchor,
        cx: &App,
    ) -> (u32, u32) {
        let snapshot = buffer.read(cx).snapshot();
        let byte = position.to_offset(&snapshot);
        let point = snapshot.offset_to_point_utf16(byte);
        (point.row as u32, point.column as u32)
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
        _buffer: &Entity<Buffer>,
        _ranges: &[Range<Anchor>],
        _cx: &mut App,
    ) -> Vec<Range<BufferRow>> {
        Vec::new()
    }

    fn invalidate_inlay_hints(
        &self,
        _for_buffers: &HashSet<language::BufferId>,
        _cx: &mut App,
    ) {
    }

    fn inlay_hints(
        &self,
        _invalidate: InvalidationStrategy,
        _buffer: Entity<Buffer>,
        _ranges: Vec<Range<Anchor>>,
        _known_chunks: Option<(clock::Global, HashSet<Range<BufferRow>>)>,
        _cx: &mut App,
    ) -> Option<HashMap<Range<BufferRow>, Task<anyhow::Result<CacheInlayHints>>>> {
        None
    }

    fn semantic_tokens(
        &self,
        _buffer: Entity<Buffer>,
        _cx: &mut App,
    ) -> Option<Shared<Task<std::result::Result<BufferSemanticTokens, Arc<anyhow::Error>>>>> {
        None
    }

    fn supports_inlay_hints(&self, _buffer: &Entity<Buffer>, _cx: &mut App) -> bool {
        false
    }

    fn supports_semantic_tokens(&self, _buffer: &Entity<Buffer>, _cx: &mut App) -> bool {
        false
    }

    fn document_highlights(
        &self,
        _buffer: &Entity<Buffer>,
        _position: Anchor,
        _cx: &mut App,
    ) -> Option<Task<anyhow::Result<Vec<DocumentHighlight>>>> {
        None
    }

    fn definitions(
        &self,
        _buffer: &Entity<Buffer>,
        _position: Anchor,
        _kind: GotoDefinitionKind,
        _cx: &mut App,
    ) -> Option<Task<anyhow::Result<Option<Vec<LocationLink>>>>> {
        None
    }

    fn range_for_rename(
        &self,
        _buffer: &Entity<Buffer>,
        _position: Anchor,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Option<RenameTarget>>> {
        Task::ready(Ok(None))
    }

    fn perform_rename(
        &self,
        _buffer: &Entity<Buffer>,
        _position: Anchor,
        _new_name: String,
        _language_server_id: Option<lsp::LanguageServerId>,
        _cx: &mut App,
    ) -> Option<Task<anyhow::Result<ProjectTransaction>>> {
        None
    }
}
