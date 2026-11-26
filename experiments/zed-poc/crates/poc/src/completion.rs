use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use editor::{CompletionContext, CompletionProvider, Editor};
use gpui::{App, AppContext as _, Context, Entity, Task, Window};
use language::{Anchor, Buffer, BufferSnapshot, CodeLabel};
use project::lsp_store::CompletionDocumentation;
use project::{
    Completion, CompletionDisplayOptions, CompletionIntent, CompletionResponse, CompletionSource,
};
use text::ToOffset;

use crate::backend::OpenIdeaBridge;

type CompletionConfirm = Arc<dyn Send + Sync + Fn(CompletionIntent, &mut Window, &mut App) -> bool>;

/// Feeds Zed's own completions menu from the OpenIdea language-server service.
pub struct OpenIdeaCompletionProvider {
    backend: Arc<OpenIdeaBridge>,
}

impl OpenIdeaCompletionProvider {
    pub fn new(backend: Arc<OpenIdeaBridge>) -> Self {
        Self { backend }
    }
}

fn completion_documentation(
    item: &ide_lsp::types::CompletionItem,
) -> Option<CompletionDocumentation> {
    item.documentation.as_ref().map(|markup| {
        if markup.kind == "markdown" {
            CompletionDocumentation::MultiLineMarkdown(markup.value.clone().into())
        } else if markup.value.contains('\n') {
            CompletionDocumentation::MultiLinePlainText(markup.value.clone().into())
        } else {
            CompletionDocumentation::SingleLine(markup.value.clone().into())
        }
    })
}

fn raw_lsp_completion(item: &ide_lsp::types::CompletionItem) -> lsp::CompletionItem {
    lsp::CompletionItem {
        label: item.label.clone(),
        insert_text: Some(item.insert_text.clone()),
        filter_text: item.filter_text.clone(),
        detail: item.detail.clone(),
        commit_characters: (!item.commit_characters.is_empty())
            .then(|| item.commit_characters.clone()),
        data: serde_json::to_value(item).ok(),
        ..Default::default()
    }
}

fn additional_edits_confirm(
    buffer: Entity<Buffer>,
    item: &ide_lsp::types::CompletionItem,
    snapshot: &BufferSnapshot,
) -> Option<CompletionConfirm> {
    if item.additional_edits.is_empty() {
        return None;
    }
    let additional_edits: Vec<(std::ops::Range<Anchor>, String)> = item
        .additional_edits
        .iter()
        .map(|edit| {
            let start = snapshot.point_utf16_to_offset(text::PointUtf16::new(
                edit.range.start.line,
                edit.range.start.col,
            ));
            let end = snapshot.point_utf16_to_offset(text::PointUtf16::new(
                edit.range.end.line,
                edit.range.end.col,
            ));
            (
                snapshot.anchor_before(start)..snapshot.anchor_after(end),
                edit.new_text.clone(),
            )
        })
        .collect();
    Some(Arc::new(
        move |_intent: CompletionIntent, _window: &mut Window, cx: &mut App| {
            let snapshot = buffer.read(cx).snapshot();
            let edits = additional_edits
                .iter()
                .map(|(range, text)| {
                    (
                        range.start.to_offset(&snapshot)..range.end.to_offset(&snapshot),
                        text.clone(),
                    )
                })
                .collect::<Vec<_>>();
            drop(snapshot);
            buffer.update(cx, |buffer, cx| {
                buffer.edit(edits, None, cx);
            });
            false
        },
    ))
}

impl CompletionProvider for OpenIdeaCompletionProvider {
    fn completions(
        &self,
        buffer: &Entity<Buffer>,
        buffer_position: Anchor,
        _trigger: CompletionContext,
        _window: &mut Window,
        cx: &mut Context<Editor>,
    ) -> Task<anyhow::Result<Vec<CompletionResponse>>> {
        let backend = self.backend.clone();
        let buffer_entity = buffer.clone();
        let snapshot = buffer.read(cx).snapshot();
        let byte = buffer_position.to_offset(&snapshot);
        let point = snapshot.offset_to_point_utf16(byte);
        let fallback_range = word_range_at(&snapshot, byte);
        let line = point.row;
        let column = point.column;
        cx.background_spawn(async move {
            let list = backend
                .completion(line, column, None)
                .await
                .map_err(|error| anyhow::anyhow!(error.message))?;
            let completions = list
                .items
                .into_iter()
                .map(|item| {
                    let new_text = if item.is_snippet {
                        ide_lsp::snippet_text(&item.insert_text)
                    } else {
                        item.insert_text.clone()
                    };
                    let replace_range = match item.replace_range {
                        Some(range) => {
                            let start = snapshot.point_utf16_to_offset(text::PointUtf16::new(
                                range.start.line,
                                range.start.col,
                            ));
                            let end = snapshot.point_utf16_to_offset(text::PointUtf16::new(
                                range.end.line,
                                range.end.col,
                            ));
                            snapshot.anchor_before(start)..snapshot.anchor_after(end)
                        }
                        None => {
                            snapshot.anchor_before(fallback_range.start)
                                ..snapshot.anchor_after(fallback_range.end)
                        }
                    };
                    let documentation = completion_documentation(&item);
                    let confirm = additional_edits_confirm(buffer_entity.clone(), &item, &snapshot);
                    let lsp_completion = raw_lsp_completion(&item);
                    Completion {
                        replace_range,
                        new_text,
                        label: CodeLabel::plain(item.label, item.filter_text.as_deref()),
                        documentation,
                        source: CompletionSource::Lsp {
                            insert_range: None,
                            server_id: lsp::LanguageServerId(item.server.0 as usize),
                            lsp_completion: Box::new(lsp_completion),
                            lsp_defaults: None,
                            resolved: false,
                        },
                        icon_path: None,
                        icon_color: None,
                        match_start: None,
                        snippet_deduplication_key: None,
                        insert_text_mode: None,
                        confirm,
                        group: None,
                    }
                })
                .collect::<Vec<_>>();
            eprintln!("POC_PROVIDER_COMPLETIONS count={}", completions.len());
            Ok(vec![CompletionResponse {
                completions,
                display_options: CompletionDisplayOptions::default(),
                is_incomplete: list.is_incomplete,
            }])
        })
    }

    fn resolve_completions(
        &self,
        buffer: Entity<Buffer>,
        completion_indices: Vec<usize>,
        completions: Rc<RefCell<Box<[Completion]>>>,
        cx: &mut Context<Editor>,
    ) -> Task<anyhow::Result<bool>> {
        let backend = self.backend.clone();
        cx.spawn(async move |_editor, cx| {
            let mut resolved_any = false;
            for index in completion_indices {
                let item = {
                    let guard = completions.borrow();
                    let Some(completion) = guard.get(index) else {
                        continue;
                    };
                    let CompletionSource::Lsp { lsp_completion, .. } = &completion.source else {
                        continue;
                    };
                    let Some(data) = lsp_completion.data.clone() else {
                        continue;
                    };
                    serde_json::from_value::<ide_lsp::types::CompletionItem>(data).ok()
                };
                let Some(item) = item else {
                    continue;
                };
                let resolved = backend
                    .resolve_completion(item)
                    .await
                    .map_err(|error| anyhow::anyhow!(error.message))?;
                let snapshot = cx.read_entity(&buffer, |buffer, _| buffer.snapshot());
                let new_text = if resolved.is_snippet {
                    ide_lsp::snippet_text(&resolved.insert_text)
                } else {
                    resolved.insert_text.clone()
                };
                let documentation = completion_documentation(&resolved);
                let confirm = additional_edits_confirm(buffer.clone(), &resolved, &snapshot);
                let lsp_completion = raw_lsp_completion(&resolved);
                let mut guard = completions.borrow_mut();
                if let Some(completion) = guard.get_mut(index) {
                    completion.new_text = new_text;
                    completion.documentation = documentation;
                    completion.confirm = confirm;
                    completion.source = CompletionSource::Lsp {
                        insert_range: None,
                        server_id: lsp::LanguageServerId(resolved.server.0 as usize),
                        lsp_completion: Box::new(lsp_completion),
                        lsp_defaults: None,
                        resolved: true,
                    };
                    resolved_any = true;
                }
            }
            Ok(resolved_any)
        })
    }

    fn is_completion_trigger(
        &self,
        _buffer: &Entity<Buffer>,
        _position: Anchor,
        text: &str,
        trigger_in_words: bool,
        _cx: &mut Context<Editor>,
    ) -> bool {
        let Some(last) = text.chars().last() else {
            return false;
        };
        last == '.'
            || last == ':'
            || last == '_'
            || last == '$'
            || (trigger_in_words && last.is_alphanumeric())
    }
}

fn word_range_at(snapshot: &BufferSnapshot, byte: usize) -> std::ops::Range<usize> {
    let point = snapshot.offset_to_point(byte);
    let line_start = snapshot.point_to_offset(text::Point::new(point.row, 0));
    let line_end = snapshot
        .point_to_offset(text::Point::new(point.row + 1, 0))
        .min(snapshot.len());
    let text = snapshot
        .text_for_range(line_start..line_end)
        .collect::<String>();
    let local = byte.saturating_sub(line_start).min(text.len());
    let is_word = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '$';
    let mut start = local;
    for (index, ch) in text[..local].char_indices().rev() {
        if !is_word(ch) {
            break;
        }
        start = index;
    }
    let mut end = local;
    for (index, ch) in text[local..].char_indices() {
        if !is_word(ch) {
            break;
        }
        end = local + index + ch.len_utf8();
    }
    line_start + start..line_start + end
}
