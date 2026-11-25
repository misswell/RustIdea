#![allow(dead_code)]

mod app;
mod bridge;
mod code_actions;
mod completion;
mod documents;
mod host;
mod java;
mod lsp;
mod project;
mod semantics;
mod sync;
mod theme;

use std::path::PathBuf;
use std::sync::Arc;

use ::theme::{ActiveTheme, LoadThemes};
use app::Workbench;
use gpui::{
    App, AppContext as _, Bounds, KeyBinding, WindowBounds, WindowOptions, actions, px, size,
};
use gpui_platform::application;
use host::NativeHost;
use settings::SettingsStore;

actions!(rustidea_zed, [SaveDocument]);

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let initial_file = std::env::args().nth(2).map(PathBuf::from);
    let host = NativeHost::start().unwrap_or_else(|error| {
        panic!("start OpenIDEA backend: {error:#}");
    });

    application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            if let Err(error) = assets::Assets.load_fonts(cx) {
                panic!("load editor fonts: {error:#}");
            }
            let settings_store = SettingsStore::new(cx, &settings::default_settings());
            cx.set_global(settings_store);
            theme_settings::init(LoadThemes::All(Box::new(assets::Assets)), cx);
            release_channel::init(semver::Version::new(0, 0, 0), cx);
            editor::init(cx);
            let language_registry = Arc::new(language::LanguageRegistry::new(
                cx.background_executor().clone(),
            ));
            language_registry.set_theme(cx.theme().clone());
            languages::init(
                language_registry.clone(),
                fs::RealFs::new(None, cx.background_executor().clone()),
                node_runtime::NodeRuntime::unavailable(),
                cx,
            );
            if let Err(error) = java::register(&language_registry) {
                eprintln!("WORKBENCH_JAVA_LANGUAGE_ERROR {error:#}");
            }
            cx.bind_keys([KeyBinding::new("cmd-s", SaveDocument, Some("Editor"))]);

            let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let host = host.clone();
            let root = root.clone();
            let initial_file = initial_file.clone();
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(gpui::TitlebarOptions {
                            title: Some("RustIdea Zed".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    move |_window, cx| {
                        let host = host.clone();
                        let root = root.clone();
                        let language_registry = language_registry.clone();
                        let initial_file = initial_file.clone();
                        cx.new(|cx| Workbench::new(host, language_registry, root, initial_file, cx))
                    },
                )
                .expect("open RustIdea Zed window");
            if let Ok(marker) = std::env::var("RUSTIDEA_ZED_SMOKE_MARKER") {
                let completion_position = std::env::var("RUSTIDEA_ZED_SMOKE_COMPLETION_POS")
                    .ok()
                    .and_then(|position| {
                        let (line, column) = position.split_once(':')?;
                        Some((line.parse().ok()?, column.parse().ok()?))
                    });
                window
                    .update(cx, |_workbench, window, cx| {
                        cx.spawn_in(window, async move |this, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            this.update(cx, |app, cx| {
                                if let Some((line, column)) = completion_position {
                                    app.smoke_request_completion(line, column, cx);
                                }
                                app.smoke_insert_marker(&marker, cx);
                            })
                            .ok();
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            this.update(cx, |app, cx| {
                                app.smoke_save_active(cx);
                            })
                            .ok();
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            this.update(cx, |app, cx| {
                                eprintln!(
                                    "WORKBENCH_SMOKE status={} language={} text_prefix={:?}",
                                    app.smoke_status(),
                                    app.smoke_has_language(cx),
                                    app.smoke_active_text(cx).lines().next().unwrap_or_default()
                                );
                                cx.quit();
                            })
                            .ok();
                        })
                        .detach();
                    })
                    .expect("run workbench smoke");
            }
        });
}
