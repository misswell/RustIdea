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
mod terminal_pane;
mod text_input;
mod theme;

use std::path::PathBuf;
use std::sync::Arc;

use ::theme::{ActiveTheme, LoadThemes};
use app::{InitialState, Workbench};
use gpui::{
    App, AppContext as _, Bounds, KeyBinding, WindowBounds, WindowOptions, actions, px, size,
};
use gpui_platform::application;
use host::NativeHost;
use settings::SettingsStore;

actions!(rustidea_zed, [SaveDocument, CloseTab, ToggleSplit]);

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let initial_file = std::env::args().nth(2).map(PathBuf::from);
    let host = NativeHost::start().unwrap_or_else(|error| {
        panic!("start OpenIDEA backend: {error:#}");
    });
    let host_for_open = host.clone();
    let root_for_open = root.clone();
    let initial_file_for_open = initial_file.clone();
    let (project, root_listing, initial_document) = host.runtime().block_on(async move {
        let project = host_for_open
            .open_project(&root_for_open)
            .await
            .unwrap_or_else(|error| panic!("open {}: {}", root_for_open.display(), error.message));
        if std::env::var_os("RUSTIDEA_ZED_TRUST").is_some()
            && let Err(error) = host_for_open
                .set_project_trust(&root_for_open, true, false)
                .await
        {
            eprintln!("WORKBENCH_TRUST_ERROR {}", error.message);
        }
        let listing = host_for_open
            .read_dir(&root_for_open)
            .await
            .unwrap_or_else(|error| {
                eprintln!("WORKBENCH_TREE_ERROR {}", error.message);
                Vec::new()
            });
        let initial_document = match initial_file_for_open {
            Some(path) => match host_for_open.open_document(&path).await {
                Ok(document) => Some(document),
                Err(error) => {
                    eprintln!("WORKBENCH_INITIAL_DOC_ERROR {}", error.message);
                    None
                }
            },
            None => None,
        };
        (project, listing, initial_document)
    });
    eprintln!(
        "WORKBENCH_PROJECT_OPEN name={} trusted={} tree_rows={} initial_doc={}",
        project.name,
        project.trusted,
        root_listing.len(),
        initial_document.is_some()
    );

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
            cx.bind_keys([
                KeyBinding::new("cmd-s", SaveDocument, Some("Editor")),
                KeyBinding::new("cmd-w", CloseTab, None),
                KeyBinding::new("cmd-\\", ToggleSplit, None),
            ]);
            {
                use text_input::{
                    Backspace as InputBackspace, Cancel as InputCancel, Copy as InputCopy,
                    Cut as InputCut, Delete as InputDelete, Down as InputDown, End as InputEnd,
                    Home as InputHome, Left as InputLeft, Paste as InputPaste,
                    Right as InputRight, SelectAll as InputSelectAll,
                    SelectLeft as InputSelectLeft, SelectRight as InputSelectRight,
                    Submit as InputSubmit, Up as InputUp,
                };
                cx.bind_keys([
                    KeyBinding::new("backspace", InputBackspace, Some("TextInput")),
                    KeyBinding::new("delete", InputDelete, Some("TextInput")),
                    KeyBinding::new("left", InputLeft, Some("TextInput")),
                    KeyBinding::new("right", InputRight, Some("TextInput")),
                    KeyBinding::new("shift-left", InputSelectLeft, Some("TextInput")),
                    KeyBinding::new("shift-right", InputSelectRight, Some("TextInput")),
                    KeyBinding::new("home", InputHome, Some("TextInput")),
                    KeyBinding::new("end", InputEnd, Some("TextInput")),
                    KeyBinding::new("up", InputUp, Some("TextInput")),
                    KeyBinding::new("down", InputDown, Some("TextInput")),
                    KeyBinding::new("cmd-a", InputSelectAll, Some("TextInput")),
                    KeyBinding::new("cmd-c", InputCopy, Some("TextInput")),
                    KeyBinding::new("cmd-x", InputCut, Some("TextInput")),
                    KeyBinding::new("cmd-v", InputPaste, Some("TextInput")),
                    KeyBinding::new("enter", InputSubmit, Some("TextInput")),
                    KeyBinding::new("escape", InputCancel, Some("TextInput")),
                ]);
            }

            let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let host = host.clone();
            let root = root.clone();
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
                    move |window, cx| {
                        let host = host.clone();
                        let root = root.clone();
                        let language_registry = language_registry.clone();
                        let project = project;
                        let root_listing = root_listing;
                        let initial_document = initial_document;
                        cx.new(|cx| {
                            Workbench::new(
                                host,
                                language_registry,
                                root,
                                InitialState {
                                    project,
                                    root_listing,
                                    document: initial_document,
                                },
                                window,
                                cx,
                            )
                        })
                    },
                )
                .expect("open RustIdea Zed window");
            let window_handle = window;
            window
                .update(cx, |app, _, _| app.set_window_handle(window_handle))
                .expect("attach workbench window handle");
            if let Ok(marker) = std::env::var("RUSTIDEA_ZED_SMOKE_MARKER") {
                window
                    .update(cx, |_workbench, window, _cx| {
                        window.activate_window();
                    })
                    .expect("activate workbench window");
                cx.activate(true);
                let smoke_split = std::env::var_os("RUSTIDEA_ZED_SMOKE_SPLIT").is_some();
                let smoke_run = std::env::var_os("RUSTIDEA_ZED_SMOKE_RUN").is_some();
                let smoke_terminal =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_TERMINAL").is_some();
                let smoke_outline = std::env::var_os("RUSTIDEA_ZED_SMOKE_OUTLINE").is_some();
                let smoke_http = std::env::var_os("RUSTIDEA_ZED_SMOKE_HTTP").is_some();
                let smoke_db = std::env::var_os("RUSTIDEA_ZED_SMOKE_DB").is_some();
                let smoke_remote = std::env::var_os("RUSTIDEA_ZED_SMOKE_REMOTE").is_some();
                let smoke_agents = std::env::var_os("RUSTIDEA_ZED_SMOKE_AGENTS").is_some();
                let smoke_tools = std::env::var_os("RUSTIDEA_ZED_SMOKE_TOOLS").is_some();
                let smoke_search = std::env::var("RUSTIDEA_ZED_SMOKE_SEARCH").ok();
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
                            this.update_in(cx, |app, window, cx| {
                                if smoke_split {
                                    app.smoke_enable_split(cx);
                                }
                                if smoke_run {
                                    app.smoke_start_run(cx);
                                }
                                if smoke_terminal {
                                    app.smoke_start_terminal(window, cx);
                                }
                                if smoke_outline {
                                    app.smoke_outline(cx);
                                }
                                if smoke_http {
                                    app.smoke_http(cx);
                                }
                                if smoke_db {
                                    app.smoke_db(cx);
                                }
                                if smoke_remote {
                                    app.smoke_remote(cx);
                                }
                                if smoke_agents {
                                    app.smoke_agents(cx);
                                }
                                if smoke_tools {
                                    app.smoke_tools(cx);
                                }
                                if let Some(pattern) = &smoke_search {
                                    app.smoke_search(pattern, cx);
                                }
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
                                    "WORKBENCH_SMOKE status={} language={} editors={} run_status={} run_output={:?} search_status={} search_files={} git_branch={} git_files={} terminal_status={} terminal={:?} outline_status={} outline_items={} problems={} http_status={} http={:?} db_status={} remote_status={} agents={} tools={} text_prefix={:?}",
                                    app.smoke_status(),
                                    app.smoke_has_language(cx),
                                    app.smoke_editor_count(),
                                    app.smoke_run_status(),
                                    app.smoke_run_output().lines().last().unwrap_or_default(),
                                    app.smoke_search_status(),
                                    app.smoke_search_files(),
                                    app.smoke_git_branch(),
                                    app.smoke_git_files(),
                                    app.terminal_status(),
                                    app.terminal_text(cx)
                                        .map(|text| {
                                            text.lines()
                                                .filter(|line| !line.trim().is_empty())
                                                .rev()
                                                .take(3)
                                                .collect::<Vec<_>>()
                                                .join(" | ")
                                        })
                                        .unwrap_or_default(),
                                    app.outline_status(),
                                    app.outline_count(),
                                    app.problem_count(),
                                    app.http_status(),
                                    app.http_output().lines().next().unwrap_or_default(),
                                    app.db_status(),
                                    app.remote_status(),
                                    app.agents_count(),
                                    app.tools_count(),
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
