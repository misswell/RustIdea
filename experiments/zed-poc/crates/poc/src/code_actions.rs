use std::ops::Range;
use std::sync::Arc;

use editor::CodeActionProvider;
use gpui::{App, AppContext as _, Entity, Task, Window};
use language::{Anchor, Buffer};
use project::{CodeAction, LspAction, ProjectTransaction};
use text::ToOffset;

use crate::backend::OpenIdeaBridge;

pub struct OpenIdeaCodeActionProvider {
    backend: Arc<OpenIdeaBridge>,
}

impl OpenIdeaCodeActionProvider {
    pub fn new(backend: Arc<OpenIdeaBridge>) -> Self {
        Self { backend }
    }

    fn position(snapshot: &language::BufferSnapshot, anchor: Anchor) -> ide_core::Pos {
        let point = snapshot.offset_to_point_utf16(anchor.to_offset(snapshot));
        ide_core::Pos::new(point.row, point.column)
    }
}

impl CodeActionProvider for OpenIdeaCodeActionProvider {
    fn id(&self) -> Arc<str> {
        "openidea".into()
    }

    fn code_actions(
        &self,
        buffer: &Entity<Buffer>,
        range: Range<Anchor>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<CodeAction>>> {
        let snapshot = buffer.read(cx).snapshot();
        let ide_range = ide_core::Range::new(
            Self::position(&snapshot, range.start),
            Self::position(&snapshot, range.end),
        );
        let requested_range = range;
        let backend = self.backend.clone();
        cx.background_spawn(async move {
            let actions = backend
                .code_actions(ide_range)
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            Ok(actions
                .into_iter()
                .map(|action| {
                    let data = serde_json::to_value(&action).ok();
                    let kind = action.kind.clone().map(lsp::CodeActionKind::from);
                    CodeAction {
                        server_id: lsp::LanguageServerId(action.server.0 as usize),
                        range: requested_range.clone(),
                        resolved: true,
                        lsp_action: LspAction::Action(Box::new(lsp::CodeAction {
                            title: action.title,
                            kind,
                            diagnostics: None,
                            edit: None,
                            command: None,
                            is_preferred: Some(action.is_preferred),
                            disabled: action
                                .disabled_reason
                                .map(|reason| lsp::CodeActionDisabled { reason }),
                            data,
                        })),
                    }
                })
                .collect())
        })
    }

    fn apply_code_action(
        &self,
        _buffer_handle: Entity<Buffer>,
        action: CodeAction,
        _push_to_history: bool,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<ProjectTransaction>> {
        let backend = self.backend.clone();
        cx.background_spawn(async move {
            let data = match &action.lsp_action {
                LspAction::Action(action) => action.data.clone(),
                LspAction::Command(_) | LspAction::CodeLens(_) => None,
            };
            let action = data
                .and_then(|data| serde_json::from_value::<ide_lsp::types::CodeAction>(data).ok())
                .ok_or_else(|| anyhow::anyhow!("code action did not originate from OpenIDEA"))?;
            backend
                .apply_code_action(action)
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            Ok(ProjectTransaction::default())
        })
    }
}
