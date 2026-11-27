//! Headless rendering smoke test.
//!
//! A locked or sleeping display makes `screencapture -l` return stale frames,
//! which is indistinguishable from "the app draws no text". This test renders a
//! view through the same GPUI text/shaping stack the workbench uses, without a
//! visible window, so glyph painting can be verified on CI and on a locked Mac.
//!
//! Run with:
//!   cargo test --release -p rustidea-zed-workbench --test headless_render

use std::sync::Arc;

use gpui::{
    App, AppContext as _, Context, HeadlessAppContext, Render, Window, div, prelude::*, px, rgb,
};

struct TextProbe {
    label: &'static str,
}

impl Render for TextProbe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .child(
                div()
                    .absolute()
                    .left(px(20.))
                    .top(px(20.))
                    .w(px(240.))
                    .h(px(60.))
                    .bg(rgb(0xff0000))
                    .p(px(8.))
                    .child(
                        div()
                            .text_size(px(18.))
                            .text_color(rgb(0x000000))
                            .child(self.label),
                    ),
            )
    }
}

fn render_probe(label: &'static str) -> (usize, usize) {
    let text_system = Arc::new(gpui_apple::AppleTextSystem::new());
    let mut cx = HeadlessAppContext::with_platform(text_system, Arc::new(assets::Assets), || {
        gpui_platform::current_headless_renderer()
    });

    cx.update(|cx: &mut App| {
        if let Err(error) = assets::Assets.load_fonts(cx) {
            panic!("load fonts: {error:#}");
        }
    });

    let window = cx
        .open_window(gpui::size(px(400.), px(200.)), |_, cx| {
            cx.new(|_| TextProbe { label })
        })
        .expect("open headless window");

    cx.run_until_parked();
    let image = cx
        .capture_screenshot(window.into())
        .expect("capture headless screenshot");

    // Count dark pixels: the red box is the only background behind the label.
    let mut dark_in_box = 0usize;
    let mut red_pixels = 0usize;
    for y in 20..80u32 {
        for x in 20..260u32 {
            let pixel = image.get_pixel(x, y);
            let [r, g, b, _] = pixel.0;
            if r > 200 && g < 80 && b < 80 {
                red_pixels += 1;
            } else if r < 60 && g < 60 && b < 60 {
                dark_in_box += 1;
            }
        }
    }

    eprintln!("HEADLESS_PIXELS label={label:?} red={red_pixels} dark_in_box={dark_in_box}");
    (red_pixels, dark_in_box)
}

#[test]
fn headless_render_paints_text() {
    let (red_pixels, dark_in_box) = render_probe("HEADLESS TEXT 123");
    assert!(red_pixels > 5_000, "red box was not painted: {red_pixels}");
    assert!(
        dark_in_box > 50,
        "text was not painted into the box: {dark_in_box} dark pixels"
    );
}

/// Negative control: the same box with no label must produce no glyph pixels,
/// otherwise `headless_render_paints_text` could pass on background noise.
#[test]
fn headless_render_without_text_has_no_glyphs() {
    let (red_pixels, dark_in_box) = render_probe("");
    assert!(red_pixels > 5_000, "red box was not painted: {red_pixels}");
    assert_eq!(
        dark_in_box, 0,
        "empty label still produced {dark_in_box} dark pixels"
    );
}
