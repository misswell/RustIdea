use gpui::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, div, prelude::*, px, size};
use gpui_platform::application;

struct Hello;

impl Render for Hello {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .bg(gpui::rgb(0x202020))
            .text_color(gpui::white())
            .text_3xl()
            .child("RustIdea Zed POC window")
    }
}

fn main() {
    application().with_assets(assets::Assets).run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(800.), px(600.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Hello),
        )
        .unwrap();
        cx.activate(true);
    });
}
