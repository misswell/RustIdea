#![allow(dead_code)]

mod app;
mod bookmarks;
mod bridge;
mod build_tools;
mod code_actions;
mod color_preview;
mod completion;
mod coverage;
mod documents;
mod host;
mod java;
mod logcat;
mod lsp;
mod pr_diff;
mod project;
mod recent;
mod scratch;
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
        SaveDocumentAs,
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
                KeyBinding::new("cmd-shift-s", SaveDocumentAs, None),
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
                let smoke_run_config_name =
                    std::env::var("RUSTIDEA_ZED_SMOKE_RUN_CONFIG_NAME").ok();
                let smoke_run_config_command = std::env::var("RUSTIDEA_ZED_SMOKE_RUN_CONFIG_COMMAND")
                    .unwrap_or_else(|_| "echo".into());
                let smoke_run_config_args = std::env::var("RUSTIDEA_ZED_SMOKE_RUN_CONFIG_ARGS")
                    .unwrap_or_else(|_| "hello".into());
                let smoke_run_config_cwd = std::env::var("RUSTIDEA_ZED_SMOKE_RUN_CONFIG_CWD")
                    .unwrap_or_default();
                let smoke_run_config_env = std::env::var("RUSTIDEA_ZED_SMOKE_RUN_CONFIG_ENV")
                    .unwrap_or_default();
                let smoke_run_config_edit =
                    std::env::var("RUSTIDEA_ZED_SMOKE_RUN_CONFIG_EDIT").ok();
                let smoke_run_template =
                    std::env::var("RUSTIDEA_ZED_SMOKE_RUN_TEMPLATE").ok();
                let smoke_run_config_delete =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_RUN_CONFIG_DELETE").is_some();
                let smoke_terminal =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_TERMINAL").is_some();
                let smoke_outline = std::env::var_os("RUSTIDEA_ZED_SMOKE_OUTLINE").is_some();
                let smoke_http = std::env::var_os("RUSTIDEA_ZED_SMOKE_HTTP").is_some();
                let smoke_http_env = std::env::var("RUSTIDEA_ZED_SMOKE_HTTP_ENV").ok();
                let smoke_http_files =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_HTTP_FILES").is_some();
                let smoke_http_editor =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_HTTP_EDITOR").is_some();
                let smoke_http_editor_run =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_HTTP_EDITOR_RUN").is_some();
                let smoke_db = std::env::var_os("RUSTIDEA_ZED_SMOKE_DB").is_some();
                let smoke_db_browser =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_DB_BROWSER").is_some();
                let smoke_db_console =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_DB_CONSOLE").is_some();
                let smoke_db_describe =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_DB_DESCRIBE").is_some();
                let smoke_db_query = std::env::var("RUSTIDEA_ZED_SMOKE_DB_QUERY").ok();
                let smoke_db_add_sqlite =
                    std::env::var("RUSTIDEA_ZED_SMOKE_DB_ADD_SQLITE").ok();
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
                let smoke_settings_json_key =
                    std::env::var("RUSTIDEA_ZED_SMOKE_SETTINGS_JSON_KEY").ok();
                let smoke_settings_json_value =
                    std::env::var("RUSTIDEA_ZED_SMOKE_SETTINGS_JSON_VALUE").ok();
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
                let smoke_notebook_clear_outputs =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_NOTEBOOK_CLEAR_OUTPUTS").is_some();
                let smoke_notebook_outputs =
                    std::env::var("RUSTIDEA_ZED_SMOKE_NOTEBOOK_OUTPUTS").ok();
                let smoke_search = std::env::var("RUSTIDEA_ZED_SMOKE_SEARCH").ok();
                let smoke_replace = std::env::var("RUSTIDEA_ZED_SMOKE_REPLACE").ok();
                let smoke_todo = std::env::var_os("RUSTIDEA_ZED_SMOKE_TODO").is_some();
                let smoke_dependencies =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_DEPENDENCIES").is_some();
                let smoke_coverage = std::env::var("RUSTIDEA_ZED_SMOKE_COVERAGE").ok();
                let smoke_coverage_run =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_COVERAGE_RUN").is_some();
                let smoke_coverage_run_name = std::env::var("RUSTIDEA_ZED_SMOKE_RUN_NAME")
                    .unwrap_or_else(|_| "test".into());
                let smoke_bookmark =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_BOOKMARK").is_some();
                let smoke_git_log = std::env::var_os("RUSTIDEA_ZED_SMOKE_GIT_LOG").is_some();
                let smoke_services =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_SERVICES").is_some();
                let smoke_docker_logs = std::env::var("RUSTIDEA_ZED_SMOKE_DOCKER_LOGS").ok();
                let smoke_docker_pull = std::env::var("RUSTIDEA_ZED_SMOKE_DOCKER_PULL").ok();
                let smoke_docker_prune = std::env::var("RUSTIDEA_ZED_SMOKE_DOCKER_PRUNE").ok();
                let smoke_docker_compose = std::env::var("RUSTIDEA_ZED_SMOKE_DOCKER_COMPOSE").ok();
                let smoke_github = std::env::var_os("RUSTIDEA_ZED_SMOKE_GITHUB").is_some();
                let smoke_github_pr = std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_PR")
                    .ok()
                    .and_then(|number| number.parse::<u32>().ok());
                let smoke_github_review =
                    std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_REVIEW").ok();
                let smoke_github_review_body =
                    std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_REVIEW_BODY")
                        .unwrap_or_else(|_| "Looks good to me".into());
                let smoke_github_reviewer =
                    std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_REVIEWER").ok();
                let smoke_github_dismiss = std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_DISMISS")
                    .ok()
                    .and_then(|id| id.parse::<u64>().ok());
                let smoke_github_dismiss_body =
                    std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_DISMISS_BODY")
                        .unwrap_or_else(|_| "Dismissed from RustIdea".into());
                let smoke_github_diff =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_GITHUB_DIFF").is_some();
                let smoke_github_inline = std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_INLINE").ok();
                let smoke_github_inline_body =
                    std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_INLINE_BODY")
                        .unwrap_or_else(|_| "Inline smoke comment".into());
                let smoke_github_state = std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_STATE").ok();
                let smoke_github_merge = std::env::var("RUSTIDEA_ZED_SMOKE_GITHUB_MERGE").ok();
                let smoke_heap = std::env::var("RUSTIDEA_ZED_SMOKE_HEAP").ok();
                let smoke_heap_compare = std::env::var("RUSTIDEA_ZED_SMOKE_HEAP_COMPARE").ok();
                let smoke_heap_objects =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_HEAP_OBJECTS").is_some();
                let smoke_profile = std::env::var("RUSTIDEA_ZED_SMOKE_PROFILE").ok();
                let smoke_profile_tree =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_PROFILE_TREE").is_some();
                let smoke_logcat = std::env::var("RUSTIDEA_ZED_SMOKE_LOGCAT").ok();
                let smoke_logcat_filter =
                    std::env::var("RUSTIDEA_ZED_SMOKE_LOGCAT_FILTER").ok();
                let smoke_logcat_export =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_LOGCAT_EXPORT").is_some();
                let smoke_scratch = std::env::var("RUSTIDEA_ZED_SMOKE_SCRATCH").ok();
                let smoke_scratch_template =
                    std::env::var("RUSTIDEA_ZED_SMOKE_SCRATCH_TEMPLATE").ok();
                let smoke_recent = std::env::var_os("RUSTIDEA_ZED_SMOKE_RECENT").is_some();
                let smoke_save_as = std::env::var("RUSTIDEA_ZED_SMOKE_SAVE_AS").ok();
                let smoke_color_preview =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_COLOR_PREVIEW").is_some();
                let smoke_analysis =
                    std::env::var_os("RUSTIDEA_ZED_SMOKE_ANALYSIS").is_some();
                let smoke_structural = std::env::var("RUSTIDEA_ZED_SMOKE_STRUCTURAL").ok();
                let smoke_structural_replace =
                    std::env::var("RUSTIDEA_ZED_SMOKE_STRUCTURAL_REPLACE").ok();
                let smoke_structural_language = std::env::var("RUSTIDEA_ZED_SMOKE_STRUCTURAL_LANG")
                    .unwrap_or_else(|_| "java".into());
                let smoke_inspect = std::env::var_os("RUSTIDEA_ZED_SMOKE_INSPECT").is_some();
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
                                if let Some(name) = &smoke_run_config_name {
                                    app.smoke_create_run_config(
                                        name.clone(),
                                        smoke_run_config_command.clone(),
                                        smoke_run_config_args.clone(),
                                        smoke_run_config_cwd.clone(),
                                        smoke_run_config_env.clone(),
                                        cx,
                                    );
                                }
                                if let Some(template) = &smoke_run_template {
                                    app.smoke_run_template(template.clone(), cx);
                                }
                                if smoke_terminal {
                                    app.smoke_start_terminal(window, cx);
                                }
                                if smoke_outline {
                                    app.smoke_outline(cx);
                                }
                                if let Some(env) = &smoke_http_env {
                                    app.smoke_http_env(env.clone(), cx);
                                }
                                if smoke_http {
                                    app.smoke_http(cx);
                                }
                                if smoke_http_files {
                                    app.smoke_http_files(cx);
                                }
                                if smoke_http_editor {
                                    app.smoke_http_editor(window, cx);
                                }
                                if smoke_http_editor_run {
                                    app.smoke_http_editor_run(cx);
                                }
                                if smoke_db {
                                    app.smoke_db(cx);
                                }
                                if smoke_db_browser {
                                    app.smoke_db_browser(cx);
                                }
                                if smoke_db_console {
                                    app.smoke_db_console(cx);
                                }
                                if let Some(sql) = &smoke_db_query {
                                    app.smoke_prepare_db_query(sql.clone(), cx);
                                }
                                if let Some(spec) = &smoke_db_add_sqlite
                                    && let Some((name, file)) = spec.split_once('|')
                                {
                                    app.smoke_add_sqlite_source(
                                        name.to_owned(),
                                        file.to_owned(),
                                        cx,
                                    );
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
                                if let (Some(key), Some(value)) =
                                    (&smoke_settings_json_key, &smoke_settings_json_value)
                                {
                                    app.smoke_settings_json(
                                        key.clone(),
                                        value.clone(),
                                        window,
                                        cx,
                                    );
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
                                    app.smoke_open_editor(window, cx);
                                    app.smoke_coverage(path.clone(), cx);
                                }
                                if smoke_coverage_run {
                                    app.smoke_open_editor(window, cx);
                                    app.smoke_run_with_coverage_for(
                                        &smoke_coverage_run_name,
                                        cx,
                                    );
                                }
                                if smoke_bookmark {
                                    app.smoke_open_editor(window, cx);
                                    app.smoke_bookmark(cx);
                                }
                                if smoke_git_log {
                                    app.smoke_git_log(cx);
                                }
                                if smoke_services {
                                    app.smoke_services(cx);
                                }
                                if smoke_github {
                                    app.smoke_github(cx);
                                }
                                if let Some(number) = smoke_github_pr {
                                    app.smoke_github_pr(number, cx);
                                }
                                if let Some(path) = &smoke_heap {
                                    app.smoke_heap(path.clone(), cx);
                                }
                                if let Some(paths) = &smoke_heap_compare
                                    && let Some((base, current)) = paths.split_once(',')
                                {
                                    app.smoke_heap_compare(
                                        base.to_owned(),
                                        current.to_owned(),
                                        cx,
                                    );
                                }
                                if let Some(path) = &smoke_profile {
                                    app.smoke_profile(path.clone(), cx);
                                }
                                if smoke_profile_tree {
                                    app.smoke_profile_tree(cx);
                                }
                                if let Some(serial) = &smoke_logcat {
                                    app.smoke_logcat(serial.clone(), cx);
                                }
                                if let Some(name) = &smoke_scratch {
                                    app.smoke_scratch(name.clone(), cx);
                                }
                                if let Some(extension) = &smoke_scratch_template {
                                    app.smoke_scratch_template(extension.clone(), cx);
                                }
                                if smoke_recent {
                                    app.smoke_recent(cx);
                                }
                                if let Some(path) = &smoke_save_as {
                                    app.smoke_save_as(path.clone(), cx);
                                }
                                if smoke_color_preview {
                                    app.smoke_color_preview(window, cx);
                                }
                                if smoke_analysis {
                                    app.smoke_analysis(cx);
                                }
                                if let Some(replacement) = &smoke_structural_replace {
                                    app.smoke_structural_replacement(replacement.clone(), cx);
                                }
                                if let Some(pattern) = &smoke_structural {
                                    app.smoke_structural(
                                        pattern.clone(),
                                        smoke_structural_language.clone(),
                                        cx,
                                    );
                                }
                                if smoke_inspect {
                                    app.smoke_inspect(cx);
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
                                if smoke_db_query.is_some() {
                                    app.smoke_run_db_query(cx);
                                }
                                if smoke_db_describe {
                                    app.smoke_describe_db_table(cx);
                                }
                                if smoke_notebook_edit {
                                    app.smoke_notebook_edit(window, cx);
                                }
                                if smoke_notebook_add {
                                    app.smoke_notebook_add_markdown(cx);
                                }
                                if smoke_notebook_clear_outputs {
                                    app.smoke_notebook_clear_outputs(cx);
                                }
                                if let Some(json) = &smoke_notebook_outputs {
                                    app.smoke_notebook_set_outputs(json.clone(), cx);
                                }
                                if smoke_debug_attach {
                                    app.smoke_debug_attach(cx);
                                }
                                if smoke_heap_objects {
                                    app.smoke_heap_objects(cx);
                                }
                                if let Some(replacement) = &smoke_replace {
                                    app.smoke_replace(replacement.clone(), cx);
                                }
                                if smoke_structural_replace.is_some() {
                                    app.smoke_apply_structural_replacements(cx);
                                }
                                if let Some(event) = &smoke_github_review {
                                    app.smoke_github_review(
                                        event,
                                        smoke_github_review_body.clone(),
                                        cx,
                                    );
                                }
                                if let Some(login) = &smoke_github_reviewer {
                                    app.smoke_request_github_reviewer(login.clone(), cx);
                                }
                                if let Some(review_id) = smoke_github_dismiss {
                                    app.smoke_dismiss_github_review(
                                        review_id,
                                        smoke_github_dismiss_body.clone(),
                                        cx,
                                    );
                                }
                                if smoke_github_diff {
                                    app.smoke_github_diff(cx);
                                }
                                if let Some(target) = &smoke_github_inline {
                                    app.smoke_github_inline(
                                        target.clone(),
                                        smoke_github_inline_body.clone(),
                                        cx,
                                    );
                                }
                                if let Some(action) = &smoke_github_state {
                                    app.smoke_github_state(action, cx);
                                }
                                if let Some(method) = &smoke_github_merge {
                                    app.smoke_github_merge(method, cx);
                                }
                                if let Some(target) = &smoke_docker_logs {
                                    app.smoke_docker_logs(target, cx);
                                }
                                if let Some(reference) = &smoke_docker_pull {
                                    app.smoke_docker_pull(reference.clone(), cx);
                                }
                                if let Some(target) = &smoke_docker_prune {
                                    app.smoke_docker_prune(target, cx);
                                }
                                if let Some(name) = &smoke_run_config_edit {
                                    app.smoke_edit_run_config(name.clone(), cx);
                                }
                                if smoke_run_config_delete {
                                    app.smoke_delete_run_config(cx);
                                }
                                if let Some(action) = &smoke_docker_compose {
                                    app.smoke_docker_compose(action, cx);
                                }
                                if let Some(filter) = &smoke_logcat_filter {
                                    app.smoke_logcat_filter(filter.clone(), cx);
                                }
                                if smoke_logcat_export {
                                    app.smoke_logcat_export(cx);
                                }
                            })
                            .ok();
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            this.update(cx, |app, cx| {
                                eprintln!(
    "WORKBENCH_SMOKE status={} language={} editors={} run_status={} run_output={:?} run_configs={} search_status={} search_files={} todo={} todo_status={} todo_first={:?} dep_modules={} dep_libraries={} dep_status={} dep_first_module={:?} dep_first_library={:?} coverage_files={} coverage_status={} coverage_first={:?} coverage_badges={} coverage_gutter={} coverage_run_busy={} bookmarks={} bookmark_status={} bookmark_first={:?} bookmark_gutter={} color_preview={} git_branch={} git_files={} git_log={} git_log_status={} git_log_first={:?} docker_containers={} docker_images={} docker_status={} docker_first={:?} docker_logs={} docker_logs_status={} github_prs={} github_status={} github_first={:?} github_detail={} github_comments={} github_timeline={} github_review={} github_diff_files={} github_diff_lines={} github_diff_status={} github_inline={} github_detail_status={} heap_classes={} heap_object_rows={} heap_objects_status={} heap_object_first={:?} heap_status={} heap_first={:?} heap_compare={} heap_compare_status={} profile_methods={} profile_tree={} profile_status={} profile_first={:?} logcat_lines={} logcat_matches={} logcat_export={:?} logcat_status={} logcat_first={:?} scratch_files={} scratch_status={} scratch_first={:?} recent_files={} recent_status={} recent_first={:?} duplicate_groups={} duplicate_status={} duplicate_first={:?} structural_matches={} structural_status={} structural_apply={} structural_first={:?} inspect_problems={} inspect_status={} inspect_first={:?} active_path={} status={} terminal_status={} terminal={:?} outline_status={} outline_items={} problems={} http_status={} http_editor={} http={:?} db_status={} remote_status={} notebook_cells={} notebook_outputs={} notebook_first={:?} agents={} agent_tasks={} mcp={} tools={} plugins={} market={} market_status={} preview_status={} quick_open={} quick_open_status={} commands={} command_first={:?} sidebar={} symbols={} symbol_first={:?} references={} references_status={} settings_rows={} settings_keymap_overrides={} show_whitespace={} font_family={:?} debug_attachables={} debug_status={} breakpoints={} build_status={} build={:?} text_prefix={:?}",
                                    app.smoke_status(),
                                    app.smoke_has_language(cx),
                                    app.smoke_editor_count(),
                                    app.smoke_run_status(),
                                    app.smoke_run_output().lines().last().unwrap_or_default(),
                                    app.run_config_count(),
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
                                    app.coverage_badge_count(),
                                    app.coverage_gutter_run_count(),
                                    app.coverage_run_busy(),
                                    app.bookmark_count(),
                                    app.bookmark_status(),
                                    app.bookmark_first(),
                                    app.bookmark_gutter_run_count(),
                                    app.color_preview_count(cx),
                                    app.smoke_git_branch(),
                                    app.smoke_git_files(),
                                    app.git_log_count(),
                                    app.git_log_status(),
                                    app.git_log_first(),
                                    app.docker_container_count(),
                                    app.docker_image_count(),
                                    app.docker_status(),
                                    app.docker_first_container(),
                                    app.docker_log_count(),
                                    app.docker_logs_status_text(),
                                    app.github_pull_count(),
                                    app.github_status_text(),
                                    app.github_first_pull(),
                                    app.github_detail_summary(),
                                    app.github_comment_count(),
                                    app.github_timeline_count(),
                                    app.github_review_status_text(),
                                    app.github_diff_file_count(),
                                    app.github_diff_line_count(),
                                    app.github_diff_status_text(),
                                    app.github_inline_status_text(),
                                    app.github_detail_status_text(),
                                    app.heap_class_count(),
                                    app.heap_object_row_count(),
                                    app.heap_objects_status_text(),
                                    app.heap_first_object(),
                                    app.heap_status_text(),
                                    app.heap_first_class(),
                                    app.heap_compare_count(),
                                    app.heap_compare_status_text(),
                                    app.profile_method_count(),
                                    app.profile_tree_count(),
                                    app.profile_status_text(),
                                    app.profile_first_method(),
                                    app.logcat_line_count(),
                                    app.logcat_match_count(),
                                    app.logcat_export_path_text(),
                                    app.logcat_status_text(),
                                    app.logcat_first_line(),
                                    app.scratch_count(),
                                    app.scratch_status_text(),
                                    app.scratch_first(),
                                    app.recent_count(),
                                    app.recent_status_text(),
                                    app.recent_first(),
                                    app.duplicate_group_count(),
                                    app.duplicate_status_text(),
                                    app.duplicate_first_fragment(),
                                    app.structural_match_count(),
                                    app.structural_status_text(),
                                    app.structural_apply_status_text(),
                                    app.structural_first(),
                                    app.inspect_diagnostic_count(),
                                    app.inspect_status_text(),
                                    app.inspect_first(),
                                    app.active_path(),
                                    app.status_text(),
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
                                    app.http_editor_status_text(),
                                    app.http_output().lines().next().unwrap_or_default(),
                                    app.db_status(),
                                    app.remote_status(),
                                    app.notebook_cell_count(),
                                    app.notebook_output_count(),
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
                                    app.settings_keymap_override_count(),
                                    app.settings_whitespace(),
                                    app.settings_font_family(),
                                    app.debug_attachable_count(),
                                    app.debug_status_text(),
                                    app.breakpoint_count(),
                                    app.build_status(),
                                    app.build_output().lines().rev().find(|line| !line.trim().is_empty()).unwrap_or_default(),
                                    app.smoke_active_text(cx).lines().next().unwrap_or_default()
                                );
                                eprintln!(
                                    "WORKBENCH_DB_UI sources={} schemas={} tables={} consoles={} consoles_status={:?} history={} history_status={:?} status={:?} console={:?} output={:?}",
                                    app.db_source_count(),
                                    app.db_schema_count(),
                                    app.db_table_count(),
                                    app.db_console_count(),
                                    app.db_consoles_status_text(),
                                    app.db_history_count(),
                                    app.db_history_status_text(),
                                    app.db_tables_status_text(),
                                    app.db_console_path_text(),
                                    app.db_output_first()
                                );
                                eprintln!(
                                    "WORKBENCH_HTTP_UI env={:?} history={} files={} files_status={:?} status={:?}",
                                    app.http_env_text(),
                                    app.http_history_count(),
                                    app.http_file_count(),
                                    app.http_files_status_text(),
                                    app.http_history_status_text()
                                );
                                eprintln!(
                                    "WORKBENCH_RUN_TEMPLATE configs={} command={:?}",
                                    app.run_config_count(),
                                    app.run_config_first_command()
                                );
                                eprintln!(
                                    "WORKBENCH_GITHUB_UI reviews={} status={:?}",
                                    app.github_review_count(),
                                    app.github_review_status_text()
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
