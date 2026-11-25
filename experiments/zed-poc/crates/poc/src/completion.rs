use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use editor::{CompletionContext, CompletionProvider, Editor};
use gpui::{AppContext as _, Context, Entity, Task, Window};
use language::{Anchor, Buffer, BufferSnapshot, CodeLabel};
use project::{Completion, CompletionDisplayOptions, CompletionResponse, CompletionSource};
use text::ToOffset;

use crate::backend::OpenIdeaBridge;

/// Feeds Zed's own completions menu from the OpenIdea language-server service.
pub struct OpenIdeaCompletionProvider {
    backend: Arc<OpenIdeaBridge>,
}

impl OpenIdeaCompletionProvider {
    pub fn new(backend: Arc<OpenIdeaBridge>) -> Self {
        Self { backend }
    }
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
        let snapshot = buffer.read(cx).snapshot();
        let byte = buffer_position.to_offset(&snapshot);
        let point = snapshot.offset_to_point_utf16(byte);
        let fallback_range = word_range_at(&snapshot, byte);
        let line = point.row as u32;
        let column = point.column as u32;
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
                        item.insert_text
                    };
                    let replace_range = match item.replace_range {
                        Some(range) => {
                            let start = snapshot.point_utf16_to_offset(
                                text::PointUtf16::new(range.start.line, range.start.col),
                            );
                            let end = snapshot.point_utf16_to_offset(
                                text::PointUtf16::new(range.end.line, range.end.col),
                            );
                            snapshot.anchor_before(start)..snapshot.anchor_after(end)
                        }
                        None => {
                            snapshot.anchor_before(fallback_range.start)
                                ..snapshot.anchor_after(fallback_range.end)
                        }
                    };
                    Completion {
                        replace_range,
                        new_text,
                        label: CodeLabel::plain(item.label, item.filter_text.as_deref()),
                        documentation: None,
                        source: CompletionSource::Custom,
                        icon_path: None,
                        icon_color: None,
                        match_start: None,
                        snippet_deduplication_key: None,
                        insert_text_mode: None,
                        confirm: None,
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
        _buffer: Entity<Buffer>,
        _completion_indices: Vec<usize>,
        _completions: Rc<RefCell<Box<[Completion]>>>,
        _cx: &mut Context<Editor>,
    ) -> Task<anyhow::Result<bool>> {
        Task::ready(Ok(false))
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
