mod app;
mod document_buffer;
mod document_store;
mod document_sync;
mod editor;
mod native_host;
mod project_tree;
mod session;
mod terminal_display;
mod text_input;
mod theme;

use std::io::Write;
use std::path::PathBuf;

use app::{
    CloseGroup, CloseTab, CodeActions, CommandPalette, Completion, DismissOverlay, FindInFile,
    FindNext, FindPrevious, FormatDocument, GitCommit, GitRefresh, GoToDefinition, GoToLine, Hover,
    NewFile, NewTerminal, NextTab, OpenFile, OpenFolder, PreviousTab, QuickOpen, Quit,
    RefreshProject, RenameSymbol, RustIdea, Save, SaveAll, SaveAs, SearchEverywhere, ShowAgents,
    ShowDatabase, ShowDebug, ShowGit, ShowPlugins, ShowProblems, ShowProject, ShowRemote, ShowRun,
    ShowSearch, ShowSettings, ShowStructure, ShowTerminal, ShowTools, SplitDown, SplitRight,
    ToggleAutosave, ToggleBottomPanel, ToggleLeftPanel, ToggleRightPanel, ToggleTheme,
    TrustProject,
};
use gpui::{
    App, AppContext, Application, Bounds, Focusable, KeyBinding, Menu, MenuItem, SystemMenuType,
    TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use native_host::NativeHost;
use session::SessionStore;
use text_input::{Cancel as InputCancel, Submit as InputSubmit};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

fn main() {
    let _log_guard = init_logging();
    install_panic_hook();
    let startup_path = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| !path.to_string_lossy().starts_with('-'));

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "starting RustIdea native GPUI application"
    );

    Application::new().run(move |cx: &mut App| {
        cx.on_action(quit);
        install_menus(cx);
        install_key_bindings(cx);

        let host = match NativeHost::start() {
            Ok(host) => host,
            Err(error) => {
                tracing::error!(%error, "failed to start the RustIdea backend");
                eprintln!("RustIdea backend failed to start: {error:#}");
                return;
            }
        };
        let session_store = SessionStore::discover();
        let session = session_store.load();
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        let startup_path = startup_path.clone();
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("RustIdea".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    app_id: Some("RustIdea".into()),
                    window_min_size: Some(size(px(900.0), px(600.0))),
                    ..Default::default()
                },
                |window, cx| {
                    let host_for_app = host.clone();
                    let app = cx.new(|cx| RustIdea::new(host_for_app, session_store, session, cx));
                    app.update(cx, |app, cx| {
                        if let Some(path) = startup_path {
                            app.open_startup_path(path, window, cx);
                        } else {
                            app.restore_last_project(cx);
                        }
                    });
                    window.focus(&app.focus_handle(cx));
                    app
                },
            )
            .expect("failed to open RustIdea window");

        window
            .update(cx, |app, window, cx| {
                window.focus(&app.focus_handle(cx));
            })
            .ok();
        cx.activate(true);
    });
}

fn quit(_: &Quit, cx: &mut App) {
    tracing::info!("quitting RustIdea");
    cx.quit();
}

fn install_menus(cx: &mut App) {
    use editor::{Copy as EditorCopy, Cut as EditorCut, Paste as EditorPaste};
    use editor::{Redo as EditorRedo, SelectAll as EditorSelectAll, Undo as EditorUndo};

    cx.set_menus(vec![
        Menu {
            name: "RustIdea".into(),
            items: vec![
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Quit RustIdea", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New File", NewFile),
                MenuItem::action("Open File…", OpenFile),
                MenuItem::action("Open Folder…", OpenFolder),
                MenuItem::separator(),
                MenuItem::action("Save", Save),
                MenuItem::action("Save As…", SaveAs),
                MenuItem::action("Save All", SaveAll),
                MenuItem::separator(),
                MenuItem::action("Close Tab", CloseTab),
                MenuItem::action("Close Split", CloseGroup),
                MenuItem::separator(),
                MenuItem::action("New Terminal", NewTerminal),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", EditorUndo),
                MenuItem::action("Redo", EditorRedo),
                MenuItem::separator(),
                MenuItem::action("Cut", EditorCut),
                MenuItem::action("Copy", EditorCopy),
                MenuItem::action("Paste", EditorPaste),
                MenuItem::action("Select All", EditorSelectAll),
                MenuItem::separator(),
                MenuItem::action("Find in Files", ShowSearch),
                MenuItem::action("Format Document", FormatDocument),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Project", ShowProject),
                MenuItem::action("Version Control", ShowGit),
                MenuItem::action("Structure", ShowStructure),
                MenuItem::action("Problems", ShowProblems),
                MenuItem::action("Database", ShowDatabase),
                MenuItem::action("Agents", ShowAgents),
                MenuItem::action("Remote Host", ShowRemote),
                MenuItem::action("Plugins", ShowPlugins),
                MenuItem::action("Tools", ShowTools),
                MenuItem::separator(),
                MenuItem::action("Terminal", ShowTerminal),
                MenuItem::action("Run", ShowRun),
                MenuItem::action("Debug", ShowDebug),
                MenuItem::separator(),
                MenuItem::action("Toggle Left Panel", ToggleLeftPanel),
                MenuItem::action("Toggle Right Panel", ToggleRightPanel),
                MenuItem::action("Toggle Bottom Panel", ToggleBottomPanel),
                MenuItem::separator(),
                MenuItem::action("Toggle Light/Dark Theme", ToggleTheme),
                MenuItem::action("Toggle Autosave", ToggleAutosave),
            ],
        },
        Menu {
            name: "Navigate".into(),
            items: vec![
                MenuItem::action("Command Palette", CommandPalette),
                MenuItem::action("Quick Open", QuickOpen),
                MenuItem::action("Search Everywhere", SearchEverywhere),
                MenuItem::separator(),
                MenuItem::action("Go to Definition", GoToDefinition),
                MenuItem::action("Go to Line", GoToLine),
                MenuItem::separator(),
                MenuItem::action("Settings", ShowSettings),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Next Tab", NextTab),
                MenuItem::action("Previous Tab", PreviousTab),
                MenuItem::action("Split Right", SplitRight),
                MenuItem::action("Split Down", SplitDown),
            ],
        },
        Menu {
            name: "Tools".into(),
            items: vec![
                MenuItem::action("Refresh Project", RefreshProject),
                MenuItem::action("Trust Project", TrustProject),
                MenuItem::action("Refresh Git", GitRefresh),
                MenuItem::action("Commit", GitCommit),
            ],
        },
    ]);
}

fn install_key_bindings(cx: &mut App) {
    use editor::{
        Backspace, Copy, Cut, Delete, Down, End, Home, Left, Paste, Redo, Right, SelectAll,
        SelectDown, SelectLeft, SelectRight, SelectUp, ShowCharacterPalette, Undo, Up,
    };
    use text_input::{
        Backspace as InputBackspace, Copy as InputCopy, Cut as InputCut, Delete as InputDelete,
        Down as InputDown, End as InputEnd, Home as InputHome, Left as InputLeft,
        Paste as InputPaste, Right as InputRight, SelectAll as InputSelectAll,
        SelectLeft as InputSelectLeft, SelectRight as InputSelectRight, Up as InputUp,
    };

    cx.bind_keys([
        KeyBinding::new("cmd-n", NewFile, None),
        KeyBinding::new("cmd-o", OpenFile, None),
        KeyBinding::new("cmd-shift-o", OpenFolder, None),
        KeyBinding::new("cmd-s", Save, None),
        KeyBinding::new("cmd-shift-s", SaveAs, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-shift-]", NextTab, None),
        KeyBinding::new("cmd-shift-[", PreviousTab, None),
        KeyBinding::new("cmd-shift-l", ToggleTheme, None),
        KeyBinding::new("cmd-shift-a", ToggleAutosave, None),
        KeyBinding::new("cmd-b", ToggleLeftPanel, None),
        KeyBinding::new("cmd-alt-1", ToggleRightPanel, None),
        KeyBinding::new("cmd-j", ToggleBottomPanel, None),
        KeyBinding::new("cmd-0", ShowProject, None),
        KeyBinding::new("cmd-shift-f", ShowSearch, None),
        KeyBinding::new("cmd-p", QuickOpen, None),
        KeyBinding::new("cmd-shift-p", CommandPalette, None),
        KeyBinding::new("cmd-e", SearchEverywhere, None),
        KeyBinding::new("cmd-shift-g", ShowGit, None),
        KeyBinding::new("cmd-shift-d", ShowDatabase, None),
        KeyBinding::new("cmd-shift-x", ShowAgents, None),
        KeyBinding::new("cmd-alt-s", SaveAll, None),
        KeyBinding::new("cmd-\\", SplitRight, None),
        KeyBinding::new("cmd-shift-\\", SplitDown, None),
        KeyBinding::new("cmd-shift-t", NewTerminal, None),
        KeyBinding::new("f12", GoToDefinition, None),
        KeyBinding::new("cmd-alt-g", GoToLine, None),
        KeyBinding::new("cmd-alt-shift-f", FormatDocument, None),
        KeyBinding::new("cmd-f", FindInFile, None),
        KeyBinding::new("cmd-g", FindNext, None),
        KeyBinding::new("ctrl-shift-g", FindPrevious, None),
        KeyBinding::new("ctrl-space", Completion, None),
        KeyBinding::new("ctrl-shift-h", Hover, None),
        KeyBinding::new("f2", RenameSymbol, None),
        KeyBinding::new("alt-enter", CodeActions, None),
        KeyBinding::new("escape", DismissOverlay, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("backspace", Backspace, Some("Editor")),
        KeyBinding::new("delete", Delete, Some("Editor")),
        KeyBinding::new("left", Left, Some("Editor")),
        KeyBinding::new("right", Right, Some("Editor")),
        KeyBinding::new("up", Up, Some("Editor")),
        KeyBinding::new("down", Down, Some("Editor")),
        KeyBinding::new("shift-left", SelectLeft, Some("Editor")),
        KeyBinding::new("shift-right", SelectRight, Some("Editor")),
        KeyBinding::new("shift-up", SelectUp, Some("Editor")),
        KeyBinding::new("shift-down", SelectDown, Some("Editor")),
        KeyBinding::new("home", Home, Some("Editor")),
        KeyBinding::new("end", End, Some("Editor")),
        KeyBinding::new("cmd-a", SelectAll, Some("Editor")),
        KeyBinding::new("cmd-c", Copy, Some("Editor")),
        KeyBinding::new("cmd-x", Cut, Some("Editor")),
        KeyBinding::new("cmd-v", Paste, Some("Editor")),
        KeyBinding::new("cmd-z", Undo, Some("Editor")),
        KeyBinding::new("cmd-shift-z", Redo, Some("Editor")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("Editor")),
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

fn init_logging() -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rustidea=info,gpui=warn"));
    let log_directory = dirs::data_dir().map(|directory| directory.join("RustIdea").join("logs"));
    if let Some(log_directory) = log_directory
        && std::fs::create_dir_all(&log_directory).is_ok()
    {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_directory.join("rustidea.log"));
        if let Ok(file) = file {
            let (writer, guard) = tracing_appender::non_blocking(file);
            if tracing_subscriber::fmt()
                .with_env_filter(filter.clone())
                .with_writer(writer)
                .with_ansi(false)
                .try_init()
                .is_ok()
            {
                return Some(guard);
            }
        }
    }
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
        .ok();
    None
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(panic = %info, "RustIdea panicked");
        default_hook(info);
        let _ = std::io::stderr().flush();
    }));
}
