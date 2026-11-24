use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use gpui::{
    App, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId,
    InspectorElementId, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PaintQuad, Pixels, Point, ScrollDelta, ScrollWheelEvent, ShapedLine, SharedString, Style,
    TextRun, Timer, UTF16Selection, UnderlineStyle, Window, actions, div, fill, font, point,
    prelude::*, px, relative, size,
};
use rustidea_editor::{Change, Document, DocumentId, EditSource, Selection, Transaction};
use unicode_segmentation::UnicodeSegmentation;

use crate::backend::{RustIdeaBackend, display_name};
use crate::theme::Theme;

actions!(
    rustidea_editor,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Undo,
        Redo,
        ShowCharacterPalette,
    ]
);

const OVERSCAN_LINES: u32 = 4;
const EDITOR_PADDING_X: f32 = 14.0;
const GUTTER_PADDING_RIGHT: f32 = 12.0;
const AUTOSAVE_DELAY: Duration = Duration::from_millis(700);

static NEXT_DOCUMENT_ID: AtomicU64 = AtomicU64::new(1);

pub fn create_document(path: Option<&Path>, text: &str) -> Document {
    let mut document = Document::new(
        DocumentId::new(NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed)),
        text,
    );
    if let Some(path) = path {
        document.set_path(path.to_path_buf());
    }
    document
}

#[derive(Clone, Debug)]
struct LineLayout {
    line: u32,
    line_start: u32,
    line_len: u32,
    text: ShapedLine,
    number: ShapedLine,
    origin: Point<Pixels>,
}

#[derive(Clone, Debug)]
struct Viewport {
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    gutter_width: Pixels,
    lines: Vec<LineLayout>,
    max_line_width: Pixels,
}

pub struct Editor {
    focus_handle: FocusHandle,
    document: Document,
    path: Option<PathBuf>,
    backend: Arc<dyn RustIdeaBackend>,
    theme: Theme,
    selection: Selection,
    marked_range: Option<Range<usize>>,
    scroll_x: f32,
    scroll_y: f32,
    is_selecting: bool,
    autosave: bool,
    autosave_generation: u64,
    saving: bool,
    status: Option<String>,
    error: Option<String>,
    viewport: Option<Viewport>,
    font_size: Pixels,
    line_height: Pixels,
}

impl Editor {
    pub fn new(
        path: Option<PathBuf>,
        document: Document,
        backend: Arc<dyn RustIdeaBackend>,
        theme: Theme,
        autosave: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            document,
            path,
            backend,
            theme,
            selection: Selection::caret(0),
            marked_range: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            is_selecting: false,
            autosave,
            autosave_generation: 0,
            saving: false,
            status: None,
            error: None,
            viewport: None,
            font_size: px(14.0),
            line_height: px(21.0),
        }
    }

    pub fn title(&self) -> String {
        self.path
            .as_deref()
            .map(display_name)
            .unwrap_or_else(|| "Untitled".to_string())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.document.set_path(path.clone());
        self.path = Some(path);
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn set_autosave(&mut self, enabled: bool) {
        self.autosave = enabled;
    }

    pub fn is_dirty(&self) -> bool {
        self.document.is_dirty()
    }

    pub fn is_saving(&self) -> bool {
        self.saving
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn cursor_position(&self) -> (u32, u32) {
        let (line, col) = self.document.offset_to_line_col(self.caret());
        (line + 1, col + 1)
    }

    pub fn line_count(&self) -> u32 {
        self.document.line_count()
    }

    pub fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            self.status = Some("Use Save As to choose a file".into());
            cx.notify();
            return;
        };
        self.save_to(path, false, cx);
    }

    fn save_to(&mut self, path: PathBuf, retarget: bool, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let version = self.document.version();
        let snapshot = self.document.snapshot();
        let backend = self.backend.clone();
        self.saving = true;
        self.error = None;
        self.status = Some("Saving…".into());
        let write_path = path.clone();
        let save_task = cx
            .background_executor()
            .spawn(async move { backend.write_text(&write_path, &snapshot.text_for_save()) });
        cx.spawn(async move |this, cx| {
            let result = save_task.await;
            this.update(cx, |editor, cx| {
                editor.saving = false;
                match result {
                    Ok(()) => {
                        if retarget {
                            editor.set_path(path);
                        }
                        if editor.document.mark_saved_if_version(version) {
                            editor.status = Some("Saved".into());
                            editor.error = None;
                        } else {
                            editor.status = Some("Saved an earlier revision".into());
                            editor.schedule_autosave(cx);
                        }
                    }
                    Err(error) => {
                        editor.error = Some(error.to_string());
                        editor.status = Some("Save failed".into());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn save_as(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.saving {
            self.status = Some("Wait for the current save before using Save As".into());
            cx.notify();
            return;
        }
        self.save_to(path, true, cx);
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        self.marked_range = None;
        match self.document.undo() {
            Ok(true) => {
                self.clamp_selection();
                self.ensure_caret_visible();
                self.schedule_autosave(cx);
            }
            Ok(false) => {
                self.status = Some("Nothing to undo".into());
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        self.marked_range = None;
        match self.document.redo() {
            Ok(true) => {
                self.clamp_selection();
                self.ensure_caret_visible();
                self.schedule_autosave(cx);
            }
            Ok(false) => {
                self.status = Some("Nothing to redo".into());
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }

    pub fn copy(&mut self, cx: &mut Context<Self>) {
        let range = self.selected_range();
        if range.is_empty() {
            return;
        }
        let text = self.document.slice(range.start, range.end);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    pub fn cut(&mut self, cx: &mut Context<Self>) {
        let range = self.selected_range();
        if range.is_empty() {
            return;
        }
        let text = self.document.slice(range.start, range.end);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.replace_utf16(range, "", EditSource::Local, cx, true);
        cx.notify();
    }

    pub fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.replace_utf16(self.selected_range(), &text, EditSource::Local, cx, true);
        cx.notify();
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.selection = Selection::range(0, self.document.len_utf16());
        self.marked_range = None;
        self.ensure_caret_visible();
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.selection.is_empty() {
            self.previous_boundary(self.caret())
        } else {
            self.selected_range().start
        };
        self.move_to(target, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.selection.is_empty() {
            self.next_boundary(self.caret())
        } else {
            self.selected_range().end
        };
        self.move_to(target, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(-1, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(1, false, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.caret()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.caret()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(-1, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(1, true, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let (line, _) = self.document.offset_to_line_col(self.caret());
        self.move_to(self.document.line_start(line), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let (line, _) = self.document.offset_to_line_col(self.caret());
        self.move_to(self.document.line_end(line), cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.selected_range();
        let range = if range.is_empty() {
            self.previous_boundary(self.caret())..self.caret()
        } else {
            range
        };
        self.replace_utf16(range, "", EditSource::Local, cx, true);
        cx.notify();
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.selected_range();
        let range = if range.is_empty() {
            self.caret()..self.next_boundary(self.caret())
        } else {
            range
        };
        self.replace_utf16(range, "", EditSource::Local, cx, true);
        cx.notify();
    }

    fn select_all_action(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all(cx);
    }

    fn paste_action(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        self.paste(cx);
        cx.notify();
    }

    fn cut_action(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        self.cut(cx);
    }

    fn copy_action(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        self.copy(cx);
    }

    fn undo_action(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.undo(cx);
    }

    fn redo_action(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.redo(cx);
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.is_selecting = true;
        let offset = self.offset_for_point(event.position);
        self.marked_range = None;
        if event.click_count >= 3 {
            self.selection = self.line_selection(offset);
        } else if event.click_count == 2 {
            self.selection = self.word_selection(offset);
        } else if event.modifiers.shift {
            self.selection.head = offset;
        } else {
            self.selection = Selection::caret(offset);
        }
        self.ensure_caret_visible();
        cx.notify();
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_selecting || !event.dragging() {
            return;
        }
        let offset = self.offset_for_point_with_autoscroll(event.position, window);
        self.selection.head = offset;
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (delta_x, delta_y) = match event.delta {
            ScrollDelta::Pixels(delta) => (f32::from(delta.x), f32::from(delta.y)),
            ScrollDelta::Lines(delta) => (
                delta.x * f32::from(self.line_height),
                delta.y * f32::from(self.line_height),
            ),
        };
        self.scroll_x = (self.scroll_x - delta_x).max(0.0);
        self.scroll_y = (self.scroll_y - delta_y).max(0.0);
        self.clamp_scroll();
        cx.notify();
    }

    fn caret(&self) -> u32 {
        self.selection.head
    }

    fn selected_range(&self) -> Range<u32> {
        self.selection.normalized()
    }

    fn move_to(&mut self, offset: u32, cx: &mut Context<Self>) {
        self.selection = Selection::caret(offset.min(self.document.len_utf16()));
        self.marked_range = None;
        self.ensure_caret_visible();
        cx.notify();
    }

    fn select_to(&mut self, offset: u32, cx: &mut Context<Self>) {
        self.selection.head = offset.min(self.document.len_utf16());
        self.marked_range = None;
        self.ensure_caret_visible();
        cx.notify();
    }

    fn move_vertical(&mut self, delta: i32, extend: bool, cx: &mut Context<Self>) {
        let (line, col) = self.document.offset_to_line_col(self.caret());
        let line_count = self.document.line_count();
        let target_line =
            (line as i64 + delta as i64).clamp(0, line_count.saturating_sub(1) as i64) as u32;
        let target = self.document.offset_for_line_col(target_line, col);
        if extend {
            self.select_to(target, cx);
        } else {
            self.move_to(target, cx);
        }
    }

    fn replace_utf16(
        &mut self,
        range: Range<u32>,
        text: &str,
        source: EditSource,
        cx: &mut Context<Self>,
        debounce_autosave: bool,
    ) {
        let version = self.document.version();
        match self.document.apply(Transaction::new(
            version,
            vec![Change::replace(range.clone(), text)],
            source,
        )) {
            Ok(_) => {
                let inserted_len = ide_text::normalize_line_endings(text)
                    .encode_utf16()
                    .count() as u32;
                let caret = range.start + inserted_len;
                self.selection = Selection::caret(caret);
                self.marked_range = None;
                self.error = None;
                self.ensure_caret_visible();
                if debounce_autosave {
                    self.schedule_autosave(cx);
                }
            }
            Err(error) => {
                self.error = Some(error.to_string());
            }
        }
    }

    fn replace_in_input(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        mark_inserted: bool,
        new_selected_range_utf16: Option<Range<usize>>,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| range_to_u32(range.clone()))
            .or_else(|| self.marked_range.clone().map(range_to_u32))
            .unwrap_or_else(|| self.selected_range());
        let inserted_len = ide_text::normalize_line_endings(new_text)
            .encode_utf16()
            .count() as u32;
        self.replace_utf16(
            range.clone(),
            new_text,
            EditSource::Local,
            cx,
            !mark_inserted,
        );
        if mark_inserted && !new_text.is_empty() {
            let marked = range.start as usize..(range.start + inserted_len) as usize;
            self.marked_range = Some(marked.clone());
            if let Some(relative) = new_selected_range_utf16 {
                let start = marked.start + relative.start.min(inserted_len as usize);
                let end = marked.start + relative.end.min(inserted_len as usize);
                self.selection = Selection::range(start as u32, end as u32);
            } else {
                self.selection = Selection::caret(range.start + inserted_len);
            }
        } else {
            self.marked_range = None;
            if let Some(relative) = new_selected_range_utf16 {
                let start = range.start + relative.start.min(inserted_len as usize) as u32;
                let end = range.start + relative.end.min(inserted_len as usize) as u32;
                self.selection = Selection::range(start, end);
            }
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: u32) -> u32 {
        if offset == 0 {
            return 0;
        }
        let (line, col) = self.document.offset_to_line_col(offset);
        if col == 0 {
            return line
                .checked_sub(1)
                .map_or(0, |previous_line| self.document.line_end(previous_line));
        }
        let text = self.document.line(line).unwrap_or_default();
        let units: Vec<u16> = text.encode_utf16().collect();
        let clamped = (col as usize).min(units.len());
        let prefix = String::from_utf16_lossy(&units[..clamped]);
        let previous_byte = prefix
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(index, _)| index);
        self.document.line_start(line) + prefix[..previous_byte].encode_utf16().count() as u32
    }

    fn next_boundary(&self, offset: u32) -> u32 {
        let len = self.document.len_utf16();
        if offset >= len {
            return len;
        }
        let (line, col) = self.document.offset_to_line_col(offset);
        let text = self.document.line(line).unwrap_or_default();
        let units: Vec<u16> = text.encode_utf16().collect();
        let clamped = (col as usize).min(units.len());
        if clamped >= units.len() {
            return if line + 1 < self.document.line_count() {
                self.document.line_start(line + 1)
            } else {
                len
            };
        }
        let suffix = String::from_utf16_lossy(&units[clamped..]);
        let next_byte = suffix
            .grapheme_indices(true)
            .nth(1)
            .map_or(suffix.len(), |(index, _)| index);
        self.document.line_start(line)
            + clamped as u32
            + suffix[..next_byte].encode_utf16().count() as u32
    }

    fn word_selection(&self, offset: u32) -> Selection {
        let (line, col) = self.document.offset_to_line_col(offset);
        let text = self.document.line(line).unwrap_or_default();
        let units: Vec<u16> = text.encode_utf16().collect();
        let col = (col as usize).min(units.len());
        let chars: Vec<(usize, usize, char)> = text
            .char_indices()
            .map(|(byte, character)| {
                let utf16_start = text[..byte].encode_utf16().count();
                (utf16_start, utf16_start + character.len_utf16(), character)
            })
            .collect();
        let target_index = chars
            .iter()
            .position(|(start, end, _)| col >= *start && col < *end)
            .or_else(|| chars.len().checked_sub(1));
        let Some(target_index) = target_index else {
            return Selection::caret(offset);
        };
        let class = char_class(chars[target_index].2);
        let mut start = target_index;
        while start > 0 && char_class(chars[start - 1].2) == class {
            start -= 1;
        }
        let mut end = target_index + 1;
        while end < chars.len() && char_class(chars[end].2) == class {
            end += 1;
        }
        let line_start = self.document.line_start(line);
        Selection::range(
            line_start + chars[start].0 as u32,
            line_start + chars[end - 1].1 as u32,
        )
    }

    fn line_selection(&self, offset: u32) -> Selection {
        let (line, _) = self.document.offset_to_line_col(offset);
        let start = self.document.line_start(line);
        let end = if line + 1 < self.document.line_count() {
            self.document.line_start(line + 1)
        } else {
            self.document.line_end(line)
        };
        Selection::range(start, end)
    }

    fn clamp_selection(&mut self) {
        let len = self.document.len_utf16();
        self.selection.anchor = self.selection.anchor.min(len);
        self.selection.head = self.selection.head.min(len);
        self.marked_range = None;
    }

    fn schedule_autosave(&mut self, cx: &mut Context<Self>) {
        if !self.autosave || self.path.is_none() {
            return;
        }
        self.autosave_generation = self.autosave_generation.wrapping_add(1);
        let generation = self.autosave_generation;
        cx.spawn(async move |this, cx| {
            Timer::after(AUTOSAVE_DELAY).await;
            this.update(cx, |editor, cx| {
                if editor.autosave_generation == generation
                    && editor.document.is_dirty()
                    && !editor.saving
                {
                    editor.save(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn clamp_scroll(&mut self) {
        let Some(viewport) = self.viewport.as_ref() else {
            return;
        };
        let line_height = f32::from(viewport.line_height);
        let content_height = self.document.line_count() as f32 * line_height;
        let max_scroll_y =
            (content_height - f32::from(viewport.bounds.size.height) + line_height).max(0.0);
        self.scroll_y = self.scroll_y.clamp(0.0, max_scroll_y);
        let visible_width = f32::from(viewport.bounds.size.width)
            - f32::from(viewport.gutter_width)
            - EDITOR_PADDING_X;
        let max_scroll_x =
            (f32::from(viewport.max_line_width) - visible_width + EDITOR_PADDING_X).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_scroll_x);
    }

    fn ensure_caret_visible(&mut self) {
        let (scroll_x, scroll_y) = {
            let Some(viewport) = self.viewport.as_ref() else {
                return;
            };
            let (line, col) = self.document.offset_to_line_col(self.caret());
            let line_height = f32::from(viewport.line_height);
            let height = f32::from(viewport.bounds.size.height);
            let top = line as f32 * line_height;
            let bottom = top + line_height;
            let mut scroll_y = self.scroll_y;
            if top < scroll_y {
                scroll_y = top;
            } else if bottom > scroll_y + height {
                scroll_y = bottom - height;
            }

            let mut scroll_x = self.scroll_x;
            if let Some(layout) = viewport.lines.iter().find(|layout| layout.line == line) {
                let byte = byte_for_utf16_col(&layout.text.text, col);
                let caret_x = f32::from(layout.text.x_for_index(byte));
                let visible_width = (f32::from(viewport.bounds.size.width)
                    - f32::from(viewport.gutter_width)
                    - EDITOR_PADDING_X)
                    .max(1.0);
                if caret_x < scroll_x {
                    scroll_x = caret_x;
                } else if caret_x + 2.0 > scroll_x + visible_width {
                    scroll_x = caret_x + 2.0 - visible_width;
                }
            }
            (scroll_x, scroll_y)
        };
        self.scroll_x = scroll_x.max(0.0);
        self.scroll_y = scroll_y.max(0.0);
        self.clamp_scroll();
    }

    fn offset_for_point(&self, position: Point<Pixels>) -> u32 {
        let Some(viewport) = self.viewport.as_ref() else {
            return self.caret();
        };
        let line_height = f32::from(viewport.line_height);
        let relative_y = f32::from(position.y - viewport.bounds.top()) + self.scroll_y;
        let line = (relative_y / line_height).floor().max(0.0) as u32;
        let line = line.min(self.document.line_count().saturating_sub(1));
        let text_origin_x =
            f32::from(viewport.bounds.left()) + f32::from(viewport.gutter_width) + EDITOR_PADDING_X
                - self.scroll_x;
        let Some(layout) = viewport.lines.iter().find(|layout| layout.line == line) else {
            return self.document.line_start(line);
        };
        if f32::from(position.x) < text_origin_x {
            return layout.line_start;
        }
        let x = (f32::from(position.x) - text_origin_x).max(0.0);
        let byte = layout.text.closest_index_for_x(px(x));
        let col = utf16_col_for_byte(&layout.text.text, byte);
        (layout.line_start + col).min(self.document.line_end(line))
    }

    fn offset_for_point_with_autoscroll(&mut self, position: Point<Pixels>, _: &mut Window) -> u32 {
        let Some(viewport) = self.viewport.as_ref() else {
            return self.caret();
        };
        let height = f32::from(viewport.bounds.size.height);
        let relative_y = f32::from(position.y - viewport.bounds.top());
        if relative_y < 0.0 {
            self.scroll_y = (self.scroll_y - f32::from(self.line_height)).max(0.0);
        } else if relative_y > height {
            self.scroll_y += f32::from(self.line_height);
            self.clamp_scroll();
        }
        self.offset_for_point(position)
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let len = self.document.len_utf16() as usize;
        let start = range_utf16.start.min(len);
        let end = range_utf16.end.min(len).max(start);
        actual_range.replace(start..end);
        Some(self.document.slice(start as u32, end as u32))
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selected_range().start as usize..self.selected_range().end as usize,
            reversed: self.selection.head < self.selection.anchor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range.clone()
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_in_input(range_utf16, new_text, false, None, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_in_input(range_utf16, new_text, true, new_selected_range_utf16, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let viewport = self.viewport.as_ref()?;
        let start = (range_utf16.start as u32).min(self.document.len_utf16());
        let (line, col) = self.document.offset_to_line_col(start);
        let layout = viewport.lines.iter().find(|layout| layout.line == line);
        if let Some(layout) = layout {
            let byte = byte_for_utf16_col(&layout.text.text, col);
            let x = layout.origin.x + layout.text.x_for_index(byte);
            Some(Bounds::from_corners(
                point(x, layout.origin.y),
                point(x + px(2.0), layout.origin.y + viewport.line_height),
            ))
        } else {
            Some(Bounds::new(
                point(
                    bounds.left() + viewport.gutter_width + px(EDITOR_PADDING_X),
                    bounds.top(),
                ),
                size(px(2.0), viewport.line_height),
            ))
        }
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_for_point(point) as usize)
    }
}

struct EditorElement {
    editor: Entity<Editor>,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct PrepaintState {
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    gutter_width: Pixels,
    lines: Vec<LineLayout>,
    max_line_width: Pixels,
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let editor = self.editor.read(cx);
        let line_height = editor.line_height;
        let gutter_digits = editor.document.line_count().max(1).ilog10() + 1;
        let gutter_width = px(gutter_digits as f32 * 8.4 + GUTTER_PADDING_RIGHT * 2.0);
        let viewport_height = f32::from(bounds.size.height);
        let line_height_f32 = f32::from(line_height);
        let first_line = ((editor.scroll_y / line_height_f32).floor().max(0.0) as u32)
            .saturating_sub(OVERSCAN_LINES);
        let visible_count = (viewport_height / line_height_f32).ceil() as u32 + 1;
        let last_line = (first_line + visible_count + OVERSCAN_LINES * 2)
            .min(editor.document.line_count().max(1));
        let text_origin_x =
            bounds.left() + gutter_width + px(EDITOR_PADDING_X) - px(editor.scroll_x);
        let font = font("Menlo");
        let mut lines = Vec::with_capacity((last_line - first_line) as usize);
        let mut max_line_width = px(0.0);

        for line in first_line..last_line {
            let text = editor.document.line(line).unwrap_or_default();
            let line_start = editor.document.line_start(line);
            let line_len = editor.document.line_end(line).saturating_sub(line_start);
            let run = TextRun {
                len: text.len(),
                font: font.clone(),
                color: editor.theme.text,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let runs = marked_runs(
                &text,
                line_start,
                line_len,
                editor.marked_range.as_ref(),
                run,
            );
            let shaped = window.text_system().shape_line(
                SharedString::from(text.clone()),
                editor.font_size,
                &runs,
                None,
            );
            max_line_width = max_line_width.max(shaped.width);

            let number_text = (line + 1).to_string();
            let number_run = TextRun {
                len: number_text.len(),
                font: font.clone(),
                color: editor.theme.faint,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let number = window.text_system().shape_line(
                SharedString::from(number_text),
                editor.font_size - px(1.0),
                &[number_run],
                None,
            );
            let y = bounds.top() + px(line as f32 * line_height_f32 - editor.scroll_y);
            lines.push(LineLayout {
                line,
                line_start,
                line_len,
                text: shaped,
                number,
                origin: point(text_origin_x, y),
            });
        }

        PrepaintState {
            bounds,
            line_height,
            gutter_width,
            lines,
            max_line_width,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus_handle, theme, selection, caret_line, caret_col) = {
            let editor = self.editor.read(cx);
            let caret = editor.caret();
            let (caret_line, caret_col) = editor.document.offset_to_line_col(caret);
            (
                editor.focus_handle.clone(),
                editor.theme,
                editor.selected_range(),
                caret_line,
                caret_col,
            )
        };
        let is_focused = focus_handle.is_focused(window);
        let clip = ContentMask { bounds };

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        window.with_content_mask(Some(clip), |window| {
            window.paint_quad(fill(
                Bounds::new(
                    bounds.origin,
                    size(prepaint.gutter_width, bounds.size.height),
                ),
                theme.gutter,
            ));
            window.paint_quad(fill(
                Bounds::from_corners(
                    point(bounds.left() + prepaint.gutter_width, bounds.top()),
                    point(
                        bounds.left() + prepaint.gutter_width + px(1.0),
                        bounds.bottom(),
                    ),
                ),
                theme.border,
            ));

            for layout in &prepaint.lines {
                if layout.line == caret_line {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + prepaint.gutter_width, layout.origin.y),
                            size(
                                bounds.size.width - prepaint.gutter_width,
                                prepaint.line_height,
                            ),
                        ),
                        theme.current_line,
                    ));
                }

                let number_x = bounds.left() + prepaint.gutter_width
                    - px(GUTTER_PADDING_RIGHT)
                    - layout.number.width;
                layout
                    .number
                    .paint(
                        point(number_x, layout.origin.y),
                        prepaint.line_height,
                        window,
                        cx,
                    )
                    .ok();

                for quad in selection_quads(
                    layout,
                    selection.clone(),
                    prepaint.line_height,
                    theme.selection,
                ) {
                    window.paint_quad(quad);
                }

                layout
                    .text
                    .paint(layout.origin, prepaint.line_height, window, cx)
                    .ok();

                if is_focused && selection.is_empty() && layout.line == caret_line {
                    let byte = byte_for_utf16_col(&layout.text.text, caret_col);
                    let x = layout.origin.x + layout.text.x_for_index(byte);
                    window.paint_quad(fill(
                        Bounds::new(
                            point(x, layout.origin.y),
                            size(px(2.0), prepaint.line_height),
                        ),
                        theme.cursor,
                    ));
                }
            }
        });

        self.editor.update(cx, |editor, _| {
            editor.viewport = Some(Viewport {
                bounds: prepaint.bounds,
                line_height: prepaint.line_height,
                gutter_width: prepaint.gutter_width,
                lines: prepaint.lines.clone(),
                max_line_width: prepaint.max_line_width,
            });
            editor.clamp_scroll();
        });
    }
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .key_context("Editor")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .bg(self.theme.window)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all_action))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste_action))
            .on_action(cx.listener(Self::cut_action))
            .on_action(cx.listener(Self::copy_action))
            .on_action(cx.listener(Self::undo_action))
            .on_action(cx.listener(Self::redo_action))
            .on_action(cx.listener(Self::show_character_palette))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .child(EditorElement {
                editor: cx.entity(),
            })
    }
}

fn range_to_u32(range: Range<usize>) -> Range<u32> {
    range.start as u32..range.end as u32
}

fn byte_for_utf16_col(text: &str, col: u32) -> usize {
    let mut utf16_count = 0u32;
    for (byte, character) in text.char_indices() {
        if utf16_count >= col {
            return byte;
        }
        utf16_count += character.len_utf16() as u32;
    }
    text.len()
}

fn utf16_col_for_byte(text: &str, byte: usize) -> u32 {
    text[..byte.min(text.len())].encode_utf16().count() as u32
}

fn marked_runs(
    text: &str,
    line_start: u32,
    line_len: u32,
    marked: Option<&Range<usize>>,
    base: TextRun,
) -> Vec<TextRun> {
    let Some(marked) = marked else {
        return vec![base];
    };
    let line_end = line_start + line_len;
    let mark_start = (marked.start as u32).max(line_start);
    let mark_end = (marked.end as u32).min(line_end);
    if mark_start >= mark_end {
        return vec![base];
    }
    let start_byte = byte_for_utf16_col(text, mark_start - line_start);
    let end_byte = byte_for_utf16_col(text, mark_end - line_start);
    let underline = UnderlineStyle {
        color: Some(base.color),
        thickness: px(1.0),
        wavy: false,
    };
    [
        TextRun {
            len: start_byte,
            ..base.clone()
        },
        TextRun {
            len: end_byte - start_byte,
            underline: Some(underline),
            ..base.clone()
        },
        TextRun {
            len: text.len() - end_byte,
            ..base
        },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

fn selection_quads(
    layout: &LineLayout,
    selection: Range<u32>,
    line_height: Pixels,
    color: gpui::Hsla,
) -> Vec<PaintQuad> {
    if selection.is_empty() {
        return Vec::new();
    }
    let line_end = layout.line_start + layout.line_len;
    let start = selection.start.max(layout.line_start).min(line_end);
    let end = selection.end.max(layout.line_start).min(line_end);
    if start >= end {
        return Vec::new();
    }
    let start_byte = byte_for_utf16_col(&layout.text.text, start - layout.line_start);
    let end_byte = byte_for_utf16_col(&layout.text.text, end - layout.line_start);
    let origin = layout.origin;
    vec![fill(
        Bounds::from_corners(
            point(origin.x + layout.text.x_for_index(start_byte), origin.y),
            point(
                origin.x + layout.text.x_for_index(end_byte),
                origin.y + line_height,
            ),
        ),
        color,
    )]
}

fn char_class(character: char) -> u8 {
    if character.is_alphanumeric() || character == '_' {
        1
    } else if character.is_whitespace() {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_and_byte_columns_round_trip_for_astral_text() {
        let text = "a😀中文";
        for (byte, _) in text
            .char_indices()
            .chain(std::iter::once((text.len(), ' ')))
        {
            let col = utf16_col_for_byte(text, byte);
            assert_eq!(byte_for_utf16_col(text, col), byte);
        }
    }

    #[test]
    fn marked_runs_cover_the_whole_utf8_line() {
        let text = "a😀中文";
        let base = TextRun {
            len: text.len(),
            font: font("Menlo"),
            color: gpui::rgb(0xffffff).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = marked_runs(text, 0, 5, Some(&(1..3)), base);
        assert_eq!(runs.iter().map(|run| run.len).sum::<usize>(), text.len());
        assert!(runs.iter().any(|run| run.underline.is_some()));
    }
}
