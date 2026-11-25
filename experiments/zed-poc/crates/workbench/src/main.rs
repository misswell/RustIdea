#![allow(dead_code)]

mod app;
mod bookmarks;
mod bridge;
mod build_tools;
mod code_actions;
mod completion;
mod coverage;
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
mod todo;

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

actions!(
    rustidea_zed,
    [
        SaveDocument,
        CloseTab,
        ToggleSplit,
        ToggleQuickOpen,
        ToggleCommandPalette,
        ToggleSymbolSearch,
        FindUsages,
        ToggleBookmark
    ]
);

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
            match settings::KeymapFile::load_asset_allow_partial_failure(
                settings::DEFAULT_KEYMAP_PATH,
                cx,
            ) {
                Ok(bindings) => {
                    eprintln!("WORKBENCH_KEYMAP bindings={}", bindings.len());
                    cx.bind_keys(bindings);
                }
                Err(error) => eprintln!("WORKBENCH_DEFAULT_KEYMAP_ERROR {error:#}"),
            }
            cx.bind_keys([
                KeyBinding::new("cmd-s", SaveDocument, Some("Editor")),
                KeyBinding::new("cmd-w", CloseTab, None),
                KeyBinding::new("cmd-\\", ToggleSplit, None),
                KeyBinding::new("cmd-p", ToggleQuickOpen, None),
                KeyBinding::new("cmd-shift-p", ToggleCommandPalette, None),
                KeyBinding::new("cmd-shift-o", ToggleSymbolSearch, None),
                KeyBinding::new("alt-f7", FindUsages, None),
                KeyBinding::new("f3", ToggleBookmark, None),
                KeyBinding::new("backspace", editor::actions::Backspace, Some("Editor")),
                KeyBinding::new("delete", editor::actions::Delete, Some("Editor")),
                KeyBinding::new("left", editor::actions::MoveLeft, Some("Editor")),
                KeyBinding::new("right", editor::actions::MoveRight, Some("Editor")),
                KeyBinding::new("up", editor::actions::LineUp, Some("Editor")),
                KeyBinding::new("down", editor::actions::LineDown, Some("Editor")),
                KeyBinding::new("shift-left", editor::actions::SelectLeft, Some("Editor")),
                KeyBinding::new("shift-right", editor::actions::SelectRight, Some("Editor")),
                KeyBinding::new("shift-up", editor::actions::SelectUp, Some("Editor")),
                KeyBinding::new("shift-down", editor::actions::SelectDown, Some("Editor")),
                KeyBinding::new(
                    "home",
                    editor::actions::MoveToBeginningOfLine::default(),
                    Some("Editor"),
                ),
                KeyBinding::new(
                    "end",
                    editor::actions::MoveToEndOfLine::default(),
                    Some("Editor"),
                ),
                KeyBinding::new("pageup", editor::actions::PageUp, Some("Editor")),
                KeyBinding::new("pagedown", editor::actions::PageDown, Some("Editor")),
                KeyBinding::new("cmd-z", editor::actions::Undo, Some("Editor")),
                KeyBinding::new("cmd-shift-z", editor::actions::Redo, Some("Editor")),
                KeyBinding::new("cmd-c", editor::actions::Copy, Some("Editor")),
                KeyBinding::new("cmd-x", editor::actions::Cut, Some("Editor")),
                KeyBinding::new("cmd-v", editor::actions::Paste, Some("Editor")),
                KeyBinding::new("cmd-a", editor::actions::SelectAll, Some("Editor")),
                KeyBinding::new(
                    "ctrl-space",
                    editor::actions::ShowCompletions,
                    Some("Editor"),
                ),
                KeyBinding::new(
                    "f12",
                    editor::actions::GoToDefinition::default(),
                    Some("Editor"),
                ),
                KeyBinding::new(
                    "shift-f12",
                    editor::actions::GoToDeclaration::default(),
                    Some("Editor"),
                ),
                KeyBinding::new(
                    "cmd-/",
                    editor::actions::ToggleComments::default(),
                    Some("Editor"),
                ),
                KeyBinding::new("cmd-]", editor::actions::Indent, Some("Editor")),
                KeyBinding::new("cmd-[", editor::actions::Outdent, Some("Editor")),
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
                let smoke_build = std::env::var_os("RUSTIDEA_ZED_SMOKE_BUILD").is_some();
                let smoke_plugins = std::env::var_os("RUSTIDEA_ZED_SMOKE_PLUGINS").is_some();
                let smoke_market = std::env::var_os("RUSTIDEA_ZED_SMOKE_MARKET").is_some();
                let smoke_mcp_roundtrip =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_MCP_ROUNDTRIP").is_some();
                let smoke_mcp_transports =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_MCP_TRANSPORTS").is_some();
                let smoke_quick_open = std::env::var("RUSTIDEA_ZED_SMOKE_QUICK_OPEN").ok();
                let smoke_command_palette =
                    std::env::var("RUSTIDEA_ZED_SMOKE_COMMAND_PALETTE").ok();
                let smoke_command_execute =
                    std::env::var("RUSTIDEA_ZED_SMOKE_COMMAND_EXEC").ok();
                let smoke_symbol = std::env::var("RUSTIDEA_ZED_SMOKE_SYMBOL").ok();
                let smoke_usages = std::env::var("RUSTIDEA_ZED_SMOKE_USAGES").ok();
                let smoke_settings =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_SETTINGS_TOGGLE").is_some();
                let smoke_settings_string =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_SETTINGS_STRING").is_some();
                let smoke_debug = std::env::var_os("RUSTIDEA_ZED_SMOKE_DEBUG").is_some();
                let smoke_debug_attach =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_DEBUG_ATTACH").is_some();
                let smoke_breakpoint =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_BREAKPOINT").is_some();
                let smoke_notebook =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_NOTEBOOK").is_some();
                let smoke_notebook_edit =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_NOTEBOOK_EDIT").is_some();
                let smoke_notebook_add =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_NOTEBOOK_ADD").is_some();
                let smoke_search = std::env::var("RUSTIDEA_ZED_SMOKE_SEARCH").ok();
                let smoke_replace = std::env::var("RUSTIDEA_ZED_SMOKE_REPLACE").ok();
                let smoke_todo = std::env::var_os("RUSTIDEA_ZED_SMOKE_TODO").is_some();
                let smoke_dependencies =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_DEPENDENCIES").is_some();
                let smoke_coverage = std::env::var("RUSTIDEA_ZED_SMOKE_COVERAGE").ok();
                let smoke_bookmark =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_BOOKMARK").is_some();
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
                                if smoke_build {
                                    app.smoke_build(cx);
                                }
                                if smoke_plugins {
                                    app.smoke_plugins(cx);
                                }
                                if smoke_market {
                                    app.smoke_market(cx);
                                }
                                if smoke_mcp_roundtrip {
                                    app.smoke_mcp_roundtrip(cx);
                                }
                                if smoke_mcp_transports {
                                    app.smoke_mcp_transport_roundtrip(cx);
                                }
                                if let Some(query) = &smoke_quick_open {
                                    app.smoke_quick_open(query.clone(), window, cx);
                                }
                                if let Some(query) = &smoke_command_palette {
                                    app.smoke_command_palette(query.clone(), window, cx);
                                }
                                if let Some(query) = &smoke_command_execute {
                                    app.smoke_command_palette_execute(query.clone(), window, cx);
                                }
                                if let Some(query) = &smoke_symbol {
                                    app.smoke_symbol_search(query.clone(), window, cx);
                                }
                                if let Some(position) = &smoke_usages
                                    && let Some((line, column)) = position.split_once(':')
                                    && let (Ok(line), Ok(column)) =
                                        (line.parse(), column.parse())
                                {
                                    app.smoke_find_usages(line, column, window, cx);
                                }
                                if smoke_settings {
                                    app.smoke_settings_toggle(cx);
                                }
                                if smoke_settings_string {
                                    app.smoke_settings_string(cx);
                                }
                                if smoke_debug {
                                    app.smoke_debug(cx);
                                }
                                if smoke_debug_attach {
                                    app.smoke_debug(cx);
                                }
                                if smoke_breakpoint {
                                    app.smoke_breakpoint(cx);
                                }
                                if smoke_notebook {
                                    app.smoke_notebook(cx);
                                }
                                if let Some(pattern) = &smoke_search {
                                    app.smoke_search(pattern, cx);
                                }
                                if smoke_todo {
                                    app.smoke_todo(cx);
                                }
                                if smoke_dependencies {
                                    app.smoke_dependencies(cx);
                                }
                                if let Some(path) = &smoke_coverage {
                                    app.smoke_coverage(path.clone(), cx);
                                }
                                if smoke_bookmark {
                                    app.smoke_bookmark(cx);
                                }
                                if let Some((line, column)) = completion_position {
                                    app.smoke_request_completion(line, column, cx);
                                }
                                if !smoke_notebook_edit && smoke_usages.is_none() {
                                    app.smoke_insert_marker(&marker, cx);
                                }
                            })
                            .ok();
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            this.update_in(cx, |app, window, cx| {
                                if !smoke_notebook_edit && smoke_usages.is_none() {
                                    app.smoke_save_active(cx);
                                }
                                if smoke_notebook_edit {
                                    app.smoke_notebook_edit(window, cx);
                                }
                                if smoke_notebook_add {
                                    app.smoke_notebook_add_markdown(cx);
                                }
                                if smoke_debug_attach {
                                    app.smoke_debug_attach(cx);
                                }
                                if let Some(replacement) = &smoke_replace {
                                    app.smoke_replace(replacement.clone(), cx);
                                }
                            })
                            .ok();
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            this.update(cx, |app, cx| {
                                eprintln!(
                                    "WORKBENCH_SMOKE status={} language={} editors={} run_status={} run_output={:?} search_status={} search_files={} todo={} todo_status={} todo_first={:?} dep_modules={} dep_libraries={} dep_status={} dep_first_module={:?} dep_first_library={:?} coverage_files={} coverage_status={} coverage_first={:?} bookmarks={} bookmark_status={} bookmark_first={:?} git_branch={} git_files={} terminal_status={} terminal={:?} outline_status={} outline_items={} problems={} http_status={} http={:?} db_status={} remote_status={} notebook_cells={} notebook_first={:?} agents={} agent_tasks={} mcp={} tools={} plugins={} market={} market_status={} preview_status={} quick_open={} quick_open_status={} commands={} command_first={:?} sidebar={} symbols={} symbol_first={:?} references={} references_status={} settings_rows={} show_whitespace={} font_family={:?} debug_attachables={} debug_status={} breakpoints={} build_status={} build={:?} text_prefix={:?}",
                                    app.smoke_status(),
                                    app.smoke_has_language(cx),
                                    app.smoke_editor_count(),
                                    app.smoke_run_status(),
                                    app.smoke_run_output().lines().last().unwrap_or_default(),
                                    app.smoke_search_status(),
                                    app.smoke_search_files(),
                                    app.todo_count(),
                                    app.todo_status(),
                                    app.todo_first(),
                                    app.dependency_module_count(),
                                    app.dependency_library_count(),
                                    app.dependency_status(),
                                    app.dependency_first_module(),
                                    app.dependency_first_library(),
                                    app.coverage_file_count(),
                                    app.coverage_status(),
                                    app.coverage_first(),
                                    app.bookmark_count(),
                                    app.bookmark_status(),
                                    app.bookmark_first(),
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
                                    app.notebook_cell_count(),
                                    app.notebook_first_source(),
                                    app.agents_count(),
                                    app.agent_tasks_count(),
                                    app.mcp_count(),
                                    app.tools_count(),
                                    app.plugins_count(),
                                    app.market_count(),
                                    app.market_status(),
                                    app.plugin_preview_status(),
                                    app.fast_open_count(),
                                    app.fast_open_status(),
                                    app.command_palette_count(),
                                    app.command_palette_first(),
                                    app.sidebar_mode_name(),
                                    app.symbol_count(),
                                    app.symbol_first(),
                                    app.reference_count(),
                                    app.references_status(),
                                    app.settings_row_count(),
                                    app.settings_whitespace(),
                                    app.settings_font_family(),
                                    app.debug_attachable_count(),
                                    app.debug_status_text(),
                                    app.breakpoint_count(),
                                    app.build_status(),
                                    app.build_output().lines().rev().find(|line| !line.trim().is_empty()).unwrap_or_default(),
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
