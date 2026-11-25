use std::sync::Arc;

use gpui::{
    App, Bounds, ClipboardItem, Context, Edges, Element, ElementId, ElementInputHandler,
    EntityInputHandler, FocusHandle, Focusable, FontStyle, FontWeight, GlobalElementId, Hsla,
    InspectorElementId, KeyDownEvent, LayoutId, MouseButton, Position, Render, ScrollEvent,
    SharedString, Style, StyledText, TextRun, UTF16Selection, Window, div, point, prelude::*, px,
    relative, rgb, size,
};
use ide_core::TerminalId;
use terminal::{
    Color, CursorShape, Event, IndexedCell, Modes, MouseInputMode, NamedColor, Terminal,
    TerminalBounds, TerminalBuilder, terminal_settings::AlternateScroll,
};
use util::paths::PathStyle;

use crate::host::NativeHost;
use crate::theme::Theme;

pub const CELL_WIDTH: f32 = 7.25;
pub const LINE_HEIGHT: f32 = 16.0;
pub const COLUMNS: usize = 120;
pub const ROWS: usize = 30;

pub struct TerminalPane {
    host: Arc<NativeHost>,
    id: TerminalId,
    terminal: gpui::Entity<Terminal>,
    focus_handle: FocusHandle,
    theme: Theme,
    title: SharedString,
    pid: Option<u32>,
    exited: bool,
    marked_range: Option<std::ops::Range<usize>>,
    _subscription: gpui::Subscription,
}

impl TerminalPane {
    pub fn new(
        host: Arc<NativeHost>,
        id: TerminalId,
        title: SharedString,
        pid: Option<u32>,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let window_id = window.window_handle().window_id().as_u64();
        let terminal = cx.new(|cx| {
            TerminalBuilder::new_display_only_with_bounds(
                terminal::terminal_settings::CursorShape::Block,
                AlternateScroll::On,
                Some(4000),
                window_id,
                cx.background_executor(),
                PathStyle::local(),
                TerminalBounds::new(
                    px(LINE_HEIGHT),
                    px(CELL_WIDTH),
                    gpui::Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(
                            px(CELL_WIDTH * COLUMNS as f32),
                            px(LINE_HEIGHT * ROWS as f32),
                        ),
                    },
                ),
            )
            .subscribe(cx)
        });
        let subscription = cx.subscribe(&terminal, |pane, _, event, cx| {
            if matches!(event, Event::Wakeup | Event::OutputReplaced) {
                pane.exited = false;
                cx.notify();
            }
        });
        let focus_handle = cx.focus_handle();
        Self {
            host,
            id,
            terminal,
            focus_handle,
            theme,
            title,
            pid,
            exited: false,
            marked_range: None,
            _subscription: subscription,
        }
    }

    pub fn feed(&mut self, chunk: &str, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, cx| {
            terminal.write_output(chunk.as_bytes(), cx);
        });
        cx.notify();
    }

    pub fn mark_exited(&mut self, code: Option<i32>, cx: &mut Context<Self>) {
        self.exited = true;
        self.title = match code {
            Some(code) => format!("Terminal · exited {code}").into(),
            None => "Terminal · closed".into(),
        };
        cx.notify();
    }

    pub fn text(&self, cx: &gpui::App) -> String {
        self.terminal.read(cx).get_content()
    }

    pub fn id(&self) -> TerminalId {
        self.id
    }

    fn send_input(&mut self, bytes: &str, cx: &mut Context<Self>) {
        if self.exited {
            return;
        }
        if let Err(error) = self.host.write_terminal(self.id, bytes) {
            self.title = format!("Terminal · write failed: {}", error.message).into();
            cx.notify();
        }
    }

    fn copy_selection(&mut self, cx: &mut Context<Self>) {
        let selection = self.terminal.read(cx).last_content().selection_text.clone();
        if let Some(text) = selection {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let bracketed = self
            .terminal
            .read(cx)
            .last_content()
            .mode
            .contains(Modes::BRACKETED_PASTE);
        let payload = if bracketed {
            format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', ""))
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r")
        };
        self.send_input(&payload, cx);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if event.prefer_character_input && keystroke.key_char.is_some() {
            return;
        }
        if keystroke.modifiers.platform {
            match keystroke.key.as_str() {
                "c" => {
                    self.copy_selection(cx);
                    cx.stop_propagation();
                    return;
                }
                "v" => {
                    self.paste(cx);
                    cx.stop_propagation();
                    return;
                }
                _ => return,
            }
        }
        let mode = self.terminal.read(cx).last_content().mode;
        if let Some(escape) = terminal::to_esc_str(keystroke, mode, false) {
            self.send_input(&escape, cx);
            cx.stop_propagation();
            return;
        }
        if let Some(text) = &keystroke.key_char
            && !keystroke.modifiers.control
            && !keystroke.modifiers.alt
        {
            self.send_input(text, cx);
            cx.stop_propagation();
        }
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, _| {
            terminal.scroll_wheel(event, 1.0, MouseInputMode::LocalSelection);
        });
        cx.notify();
    }

    fn render_row(&self, line: i32, cells: &[&IndexedCell]) -> gpui::AnyElement {
        let mut text = String::new();
        let mut runs: Vec<TextRun> = Vec::new();
        let mut run_start = 0usize;
        let mut run_style: Option<(Hsla, Option<Hsla>, bool, bool)> = None;

        for cell in cells {
            if cell.is_wide_char_spacer() {
                continue;
            }
            let mut foreground = self.resolve_color(cell.foreground());
            let mut background = self.resolve_background(cell.background());
            if cell.is_inverse() {
                let inverse_foreground = background.unwrap_or(self.theme.window);
                background = Some(foreground);
                foreground = inverse_foreground;
            }
            if cell.is_dim() {
                foreground.a *= 0.65;
            }
            let style = (foreground, background, cell.is_bold(), cell.is_italic());
            let character = cell.character();
            let start = text.len();
            text.push(character);
            if let Some(zerowidth) = cell.zerowidth() {
                text.extend(zerowidth);
            }
            if run_style != Some(style) {
                if let Some((foreground, background, bold, italic)) = run_style {
                    let mut font = gpui::font("Menlo");
                    if bold {
                        font.weight = FontWeight::BOLD;
                    }
                    if italic {
                        font.style = FontStyle::Italic;
                    }
                    runs.push(TextRun {
                        len: start - run_start,
                        font,
                        color: foreground,
                        background_color: background,
                        underline: None,
                        strikethrough: None,
                    });
                }
                run_start = start;
                run_style = Some(style);
            }
        }
        if let Some((foreground, background, bold, italic)) = run_style {
            let mut font = gpui::font("Menlo");
            if bold {
                font.weight = FontWeight::BOLD;
            }
            if italic {
                font.style = FontStyle::Italic;
            }
            runs.push(TextRun {
                len: text[run_start..].len(),
                font,
                color: foreground,
                background_color: background,
                underline: None,
                strikethrough: None,
            });
        }
        let content = if text.is_empty() || runs.is_empty() {
            StyledText::new(" ").into_any_element()
        } else {
            StyledText::new(text).with_runs(runs).into_any_element()
        };
        div()
            .id(("terminal-row", line as usize))
            .h(px(LINE_HEIGHT))
            .w(px(CELL_WIDTH * COLUMNS as f32))
            .whitespace_nowrap()
            .child(content)
            .into_any_element()
    }

    fn resolve_background(&self, color: Color) -> Option<Hsla> {
        match color {
            Color::Named(NamedColor::Background) => None,
            _ => Some(self.resolve_color(color)),
        }
    }

    fn resolve_color(&self, color: Color) -> Hsla {
        match color {
            Color::Named(named) => self.named_color(named),
            Color::Spec(rgb_value) => {
                rgb(((rgb_value.r as u32) << 16) | ((rgb_value.g as u32) << 8) | rgb_value.b as u32)
                    .into()
            }
            Color::Indexed(index) => self.indexed_color(index),
        }
    }

    fn named_color(&self, named: NamedColor) -> Hsla {
        let value = match named {
            NamedColor::Black => 0x1b1c20,
            NamedColor::Red => 0xe06c75,
            NamedColor::Green => 0x98c379,
            NamedColor::Yellow => 0xe5c07b,
            NamedColor::Blue => 0x61afef,
            NamedColor::Magenta => 0xc678dd,
            NamedColor::Cyan => 0x56b6c2,
            NamedColor::White => 0xdcdfe4,
            NamedColor::BrightBlack | NamedColor::DimBlack => 0x5c6370,
            NamedColor::BrightRed | NamedColor::DimRed => 0xe06c75,
            NamedColor::BrightGreen | NamedColor::DimGreen => 0x98c379,
            NamedColor::BrightYellow | NamedColor::DimYellow => 0xe5c07b,
            NamedColor::BrightBlue | NamedColor::DimBlue => 0x61afef,
            NamedColor::BrightMagenta | NamedColor::DimMagenta => 0xc678dd,
            NamedColor::BrightCyan | NamedColor::DimCyan => 0x56b6c2,
            NamedColor::BrightWhite | NamedColor::DimWhite => 0xffffff,
            NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::DimForeground => {
                return self.theme.text;
            }
            NamedColor::Background => return self.theme.window,
            NamedColor::Cursor => return self.theme.cursor,
        };
        rgb(value).into()
    }

    fn indexed_color(&self, index: u8) -> Hsla {
        const BASE: [u32; 16] = [
            0x1b1c20, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xdcdfe4,
            0x5c6370, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xffffff,
        ];
        if index < 16 {
            return rgb(BASE[index as usize]).into();
        }
        if index < 232 {
            let index = index - 16;
            let red = index / 36;
            let green = (index % 36) / 6;
            let blue = index % 6;
            let component = |value: u8| {
                if value == 0 {
                    0u32
                } else {
                    55 + 40 * value as u32
                }
            };
            return rgb((component(red) << 16) | (component(green) << 8) | component(blue)).into();
        }
        let gray = 8 + 10 * (index - 232) as u32;
        rgb((gray << 16) | (gray << 8) | gray).into()
    }
}

impl Focusable for TerminalPane {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for TerminalPane {
    fn text_for_range(
        &mut self,
        _range: std::ops::Range<usize>,
        _adjusted_range: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
        self.marked_range.clone()
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_range = None;
        if !text.is_empty() {
            self.send_input(text, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        new_text: &str,
        new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.marked_range =
            new_selected_range.or_else(|| (!new_text.is_empty()).then_some(0..new_text.len()));
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: std::ops::Range<usize>,
        element_bounds: Bounds<gpui::Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<gpui::Pixels>> {
        let content = self.terminal.read(cx).last_content();
        let line = content.cursor.point.line.max(0) as f32;
        let column = content.cursor.point.column as f32;
        Some(Bounds::new(
            point(
                element_bounds.left() + px(4.0 + column * CELL_WIDTH),
                element_bounds.top() + px(28.0 + line * LINE_HEIGHT),
            ),
            size(px(2.), px(LINE_HEIGHT)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<gpui::Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        !self.exited
    }
}

struct TerminalInputElement {
    pane: gpui::Entity<TerminalPane>,
}

impl Element for TerminalInputElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            position: Position::Absolute,
            inset: Edges::all(relative(0.0).into()),
            size: size(relative(1.0).into(), relative(1.0).into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<gpui::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.pane.read(cx).focus_handle.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.pane.clone()),
            cx,
        );
    }
}

impl Render for TerminalPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self.terminal.update(cx, |terminal, cx| {
            terminal.sync(MouseInputMode::LocalSelection, window, cx);
            terminal.last_content().clone()
        });
        let mut rows = Vec::with_capacity(content.screen_lines);
        for line in 0..content.screen_lines as i32 {
            let cells: Vec<&IndexedCell> = content
                .cells
                .iter()
                .filter(|cell| cell.point.line == line)
                .collect();
            rows.push(self.render_row(line, &cells));
        }

        let cursor = content.cursor.point;
        let cursor_visible = content.mode.contains(Modes::SHOW_CURSOR)
            && cursor.line >= 0
            && cursor.line < content.screen_lines as i32;
        let cursor_element = if cursor_visible {
            let x = cursor.column as f32 * CELL_WIDTH;
            let y = cursor.line as f32 * LINE_HEIGHT;
            let mut color = self.theme.cursor;
            color.a = 0.45;
            let shape = match content.cursor.shape {
                CursorShape::Underline => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y + LINE_HEIGHT - 2.0))
                    .w(px(CELL_WIDTH))
                    .h(px(2.))
                    .bg(color),
                CursorShape::Bar => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(2.))
                    .h(px(LINE_HEIGHT))
                    .bg(color),
                CursorShape::Hidden => div(),
                CursorShape::Block | CursorShape::HollowBlock => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(CELL_WIDTH))
                    .h(px(LINE_HEIGHT))
                    .bg(color),
            };
            shape.into_any_element()
        } else {
            div().into_any_element()
        };

        let title = self.title.clone();
        let pid = self.pid;
        let focus_handle = self.focus_handle.clone();
        let pane = cx.entity();
        div()
            .id("terminal-pane")
            .size_full()
            .flex()
            .flex_col()
            .bg(self.theme.window)
            .text_color(self.theme.text)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_scroll(cx.listener(Self::on_scroll))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_, _: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&focus_handle, cx);
                }),
            )
            .child(
                div()
                    .h(px(24.))
                    .flex_none()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.5))
                    .text_color(self.theme.muted)
                    .child(title)
                    .when_some(pid, |row, pid| row.child(format!("pid {pid}")))
                    .child(div().flex_1())
                    .child(format!("{COLUMNS}x{ROWS}")),
            )
            .child(
                div()
                    .id("terminal-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_scroll()
                    .bg(self.theme.window)
                    .child(
                        div()
                            .relative()
                            .w(px(CELL_WIDTH * COLUMNS as f32))
                            .h(px(LINE_HEIGHT * ROWS as f32))
                            .p(px(4.))
                            .text_size(px(12.))
                            .children(rows)
                            .child(cursor_element),
                    ),
            )
            .child(TerminalInputElement { pane })
    }
}
