//! Real-window renderer probe for the visual CI runner.
//!
//! Draws a red box with black text in a normal GPUI window. The headless test
//! cannot catch a missing `font-kit` feature because it uses the software
//! renderer; this window must be captured by `tests/visual/window-render-smoke.sh`.
use gpui::{
    App, AppContext as _, Bounds, Context, IntoElement, Render, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;

struct Probe;

impl Render for Probe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0xffffff)).child(
            div()
                .absolute()
                .left(px(20.))
                .top(px(20.))
                .w(px(320.))
                .h(px(80.))
                .bg(rgb(0xff0000))
                .p(px(8.))
                .child(
                    div()
                        .text_size(px(24.))
                        .text_color(rgb(0x000000))
                        .child("WINDOW TEXT PROBE 123"),
                ),
        )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(600.), px(300.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("RustIdea Window Probe".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Probe),
        )
        .expect("open probe window");
    });
}
