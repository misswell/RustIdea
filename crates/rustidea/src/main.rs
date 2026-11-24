mod app;
mod backend;
mod editor;
mod project;
mod session;
mod theme;

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use app::{
    CloseTab, NewFile, NextTab, OpenFile, OpenFolder, PreviousTab, Quit, RustIdea, Save, SaveAs,
    ToggleAutosave, ToggleTheme,
};
use backend::LocalBackend;
use gpui::{
    App, AppContext, Application, Bounds, Focusable, KeyBinding, Menu, MenuItem, SystemMenuType,
    TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use session::SessionStore;
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

        let session_store = SessionStore::discover();
        let session = session_store.load();
        let backend = Arc::new(LocalBackend);
        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
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
                    window_min_size: Some(size(px(720.0), px(480.0))),
                    ..Default::default()
                },
                |window, cx| {
                    let app = cx.new(|cx| RustIdea::new(session_store, session, backend, cx));
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
                MenuItem::separator(),
                MenuItem::action("Close Tab", CloseTab),
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
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Toggle Light/Dark Theme", ToggleTheme),
                MenuItem::action("Toggle Autosave", ToggleAutosave),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Next Tab", NextTab),
                MenuItem::action("Previous Tab", PreviousTab),
            ],
        },
    ]);
}

fn install_key_bindings(cx: &mut App) {
    use editor::{
        Backspace, Copy, Cut, Delete, Down, End, Home, Left, Paste, Redo, Right, SelectAll,
        SelectDown, SelectLeft, SelectRight, SelectUp, ShowCharacterPalette, Undo, Up,
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
