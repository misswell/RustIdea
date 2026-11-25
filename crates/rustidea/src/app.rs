mod ui;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Pixels, PromptLevel, Subscription, Timer,
    UniformListScrollHandle, Window, actions, prelude::*,
};
use ide_core::event::{IdeEvent, NotifyLevel, ServerStatus};
use ide_core::settings::Settings;
use ide_core::{Diagnostic, DocId, Pos, RequestId, Severity};
use ide_host::ops::project::ProjectInfo;
use ide_host::ops::vcs;
use ide_index::SymbolFilter;
use ide_search::{FileMatches, SearchQuery, SearchScope, SearchSummary};
use ide_vcs::{BranchInfo, Commit, FileStatus, LogQuery};

use crate::document_buffer::{BufferEvent, DocumentBuffer};
use crate::document_store::{DocumentKey, DocumentStore};
use crate::editor::Editor;
use crate::native_host::NativeHost;
use crate::project_tree::{ProjectTree, Toggle};
use crate::session::{Session, SessionStore};
use crate::text_input::{TextInput, TextInputEvent};
use crate::theme::Theme;

actions!(
    rustidea,
    [
        Quit,
        NewFile,
        OpenFile,
        OpenFolder,
        Save,
        SaveAll,
        SaveAs,
        CloseTab,
        NextTab,
        PreviousTab,
        ToggleTheme,
        ToggleAutosave,
        ToggleLeftPanel,
        ToggleRightPanel,
        ToggleBottomPanel,
        ShowProject,
        ShowSearch,
        ShowGit,
        ShowStructure,
        ShowProblems,
        ShowSettings,
        ShowTerminal,
        ShowRun,
        ShowDebug,
        ShowDatabase,
        ShowAgents,
        ShowPlugins,
        ShowRemote,
        ShowTools,
        CommandPalette,
        QuickOpen,
        SearchEverywhere,
        SplitRight,
        SplitDown,
        CloseGroup,
        RefreshProject,
        TrustProject,
        FindInFile,
        FindNext,
        FindPrevious,
        FormatDocument,
        GoToDefinition,
        GoToLine,
        Completion,
        Hover,
        RenameSymbol,
        CodeActions,
        DismissOverlay,
        SelectAll,
        ShowCharacterPalette,
        NewTerminal,
        GitRefresh,
        GitCommit,
    ]
);

const DEFAULT_LEFT_WIDTH: f32 = 280.0;
const DEFAULT_RIGHT_WIDTH: f32 = 340.0;
const DEFAULT_BOTTOM_HEIGHT: f32 = 240.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarKind {
    Project,
    Search,
    Git,
    Structure,
    Problems,
    Settings,
    Database,
    Http,
    Agents,
    Remote,
    Plugins,
    Tools,
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn project_state(root: PathBuf, info: ProjectInfo) -> ProjectState {
    ProjectState {
        tree: ProjectTree::new(root.clone()),
        root,
        info,
        scroll: UniformListScrollHandle::new(),
    }
}

impl SidebarKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Search => "Search",
            Self::Git => "Version Control",
            Self::Structure => "Structure",
            Self::Problems => "Problems",
            Self::Settings => "Settings",
            Self::Database => "Database",
            Self::Http => "HTTP Client",
            Self::Agents => "Agents",
            Self::Remote => "Remote Host",
            Self::Plugins => "Plugins",
            Self::Tools => "Tools",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomKind {
    Terminal,
    Problems,
    Run,
    Debug,
    Build,
    VersionControl,
    Output,
}

impl BottomKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Problems => "Problems",
            Self::Run => "Run",
            Self::Debug => "Debug",
            Self::Build => "Build",
            Self::VersionControl => "Version Control",
            Self::Output => "Output",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitAxis {
    Horizontal,
    Vertical,
}

struct EditorTab {
    buffer: Entity<DocumentBuffer>,
    editor: Entity<Editor>,
    _subscriptions: Vec<Subscription>,
}

struct EditorGroup {
    tabs: Vec<EditorTab>,
    active: Option<usize>,
}

impl EditorGroup {
    fn empty() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
        }
    }
}

struct ProjectState {
    root: PathBuf,
    info: ProjectInfo,
    tree: ProjectTree,
    scroll: UniformListScrollHandle,
}

#[derive(Clone, Debug)]
struct ProblemEntry {
    path: PathBuf,
    diagnostic: Diagnostic,
    source: String,
}

#[derive(Default)]
struct GitState {
    status: Vec<FileStatus>,
    branch: Option<BranchInfo>,
    branches: Vec<ide_vcs::Branch>,
    log: Vec<Commit>,
    selected: Option<PathBuf>,
    commit_message: String,
    diff_preview: Option<(PathBuf, String)>,
    output: String,
    loading: bool,
    error: Option<String>,
}

#[derive(Default)]
struct SearchState {
    query: String,
    replace: String,
    case_sensitive: bool,
    regex: bool,
    whole_word: bool,
    replace_mode: bool,
    results: Vec<FileMatches>,
    summary: Option<SearchSummary>,
    request: Option<RequestId>,
    running: bool,
    error: Option<String>,
    generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayKind {
    CommandPalette,
    QuickOpen,
    SearchEverywhere,
    GoToLine,
    Completion,
    Hover,
    Rename,
    CodeActions,
    Find,
}

#[derive(Clone, Debug)]
enum UiAction {
    NewFile,
    OpenFile,
    OpenFolder,
    Save,
    SaveAll,
    SaveAs,
    CloseTab,
    NextTab,
    PreviousTab,
    SplitRight,
    SplitDown,
    CloseGroup,
    ToggleAutosave,
    OpenPath(PathBuf, Option<u32>),
    ShowSidebar(SidebarKind),
    ShowBottom(BottomKind),
    ToggleTheme,
    RefreshProject,
    RefreshGit,
    OpenSettings,
    GoToLinePrompt,
    Completion,
    Hover,
    RenameSymbol,
    CodeActions,
    FindInFile,
    FindNext,
    FindPrevious,
    ApplyCompletion(Box<ide_lsp::types::CompletionItem>),
    ApplyCodeAction(Box<ide_lsp::types::CodeAction>),
    None,
}

#[derive(Clone, Debug)]
struct PaletteItem {
    title: String,
    detail: String,
    action: UiAction,
}

struct OverlayState {
    kind: OverlayKind,
    input: Entity<TextInput>,
    items: Vec<PaletteItem>,
    selected: usize,
    error: Option<String>,
    message: Option<String>,
}

struct DragState {
    left_width: Option<Pixels>,
    right_width: Option<Pixels>,
    bottom_height: Option<Pixels>,
}

struct TerminalTab {
    info: ide_terminal::TerminalInfo,
    output: String,
}

#[derive(Clone)]
struct ContextMenuState {
    position: gpui::Point<Pixels>,
    path: PathBuf,
    is_directory: bool,
}

pub struct RustIdea {
    focus_handle: FocusHandle,
    host: Arc<NativeHost>,
    theme: Theme,
    session_store: SessionStore,
    session: Session,
    project: Option<ProjectState>,
    documents: DocumentStore<Entity<DocumentBuffer>>,
    pending_opens: HashSet<DocumentKey>,
    groups: Vec<EditorGroup>,
    active_group: usize,
    split_axis: SplitAxis,
    sidebar_kind: SidebarKind,
    sidebar_visible: bool,
    right_visible: bool,
    bottom_visible: bool,
    sidebar_width: f32,
    right_width: f32,
    bottom_height: f32,
    drag: Option<DragState>,
    context_menu: Option<ContextMenuState>,
    bottom_kind: BottomKind,
    status: String,
    problems: Vec<ProblemEntry>,
    problem_sources: HashMap<(PathBuf, String), Vec<Diagnostic>>,
    search: SearchState,
    search_input: Entity<TextInput>,
    search_replace_input: Entity<TextInput>,
    _search_subscriptions: Vec<Subscription>,
    git: GitState,
    git_message_input: Entity<TextInput>,
    _git_subscriptions: Vec<Subscription>,
    overlay: Option<OverlayState>,
    palette_input: Entity<TextInput>,
    _palette_subscriptions: Vec<Subscription>,
    settings: Settings,
    settings_filter: String,
    settings_input: Entity<TextInput>,
    _settings_subscriptions: Vec<Subscription>,
    terminal_input: Entity<TextInput>,
    _terminal_subscriptions: Vec<Subscription>,
    terminals: Vec<TerminalTab>,
    active_terminal: usize,
    run_configs: Vec<ide_run::RunConfig>,
    selected_run: usize,
    run_output: String,
    running_run: Option<ide_run::RunInfo>,
    build_summary: Option<String>,
    build_output: String,
    debug_attachables: Vec<String>,
    http_method: String,
    http_url_input: Entity<TextInput>,
    http_headers_input: Entity<TextInput>,
    http_body_input: Entity<TextInput>,
    _http_subscriptions: Vec<Subscription>,
    http_response: String,
    http_status: Option<String>,
    db_sources: Vec<ide_db::DataSource>,
    db_selected: Option<String>,
    db_tables: Vec<String>,
    db_schema: Option<String>,
    db_query_input: Entity<TextInput>,
    _db_subscriptions: Vec<Subscription>,
    db_result: String,
    db_status: Option<String>,
    ssh_configs: Vec<ide_core::remote_settings::SshConfiguration>,
    ssh_status: Option<String>,
    server_statuses: BTreeMap<String, (ServerStatus, Option<String>)>,
    pending_close: Option<Entity<DocumentBuffer>>,
    opening_project: bool,
    last_tree_epoch: u64,
    completion_items: Vec<ide_lsp::types::CompletionItem>,
    code_action_items: Vec<ide_lsp::types::CodeAction>,
    pending_editor_focus: bool,
    pending_ui_action: Option<UiAction>,
    find_query: String,
}

impl RustIdea {
    pub fn new(
        host: Arc<NativeHost>,
        session_store: SessionStore,
        session: Session,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = Theme::for_mode(session.theme);
        let input = |cx: &mut Context<Self>, placeholder: &str| {
            let placeholder = placeholder.to_owned();
            cx.new(|cx| {
                TextInput::new(
                    placeholder,
                    theme.text,
                    theme.faint,
                    theme.selection,
                    theme.cursor,
                    cx,
                )
            })
        };
        let search_input = input(cx, "Search text");
        let search_replace_input = input(cx, "Replace with");
        let git_message_input = input(cx, "Commit message");
        let palette_input = input(cx, "Type a command or file name…");
        let settings_input = input(cx, "Filter settings");
        let terminal_input = input(cx, "Type a command and press Enter");
        let http_url_input = input(cx, "https://api.example.com/items");
        let http_headers_input = input(cx, "Authorization: Bearer …; Accept: application/json");
        let http_body_input = input(cx, "Request body (JSON, text or form data)");
        let db_query_input = input(cx, "SELECT * FROM …");

        let mut search_subscriptions = Vec::new();
        search_subscriptions.push(cx.subscribe(
            &search_input,
            |app: &mut RustIdea, _input, event: &TextInputEvent, cx| match event {
                TextInputEvent::Change(query) => {
                    app.search.query = query.clone();
                    app.schedule_search(cx);
                }
                TextInputEvent::Submit(_) => app.run_search(cx),
                TextInputEvent::MoveSelection(_) => {}
                TextInputEvent::Cancel => {
                    app.sidebar_kind = SidebarKind::Project;
                    cx.notify();
                }
            },
        ));
        search_subscriptions.push(cx.subscribe(
            &search_replace_input,
            |app: &mut RustIdea, _input, event: &TextInputEvent, cx| match event {
                TextInputEvent::Change(value) => app.search.replace = value.clone(),
                TextInputEvent::Submit(_) => app.replace_all(cx),
                TextInputEvent::MoveSelection(_) => {}
                TextInputEvent::Cancel => {
                    app.search.replace_mode = false;
                    cx.notify();
                }
            },
        ));

        let mut git_subscriptions = Vec::new();
        git_subscriptions.push(cx.subscribe(
            &git_message_input,
            |app: &mut RustIdea, _input, event: &TextInputEvent, cx| {
                if let TextInputEvent::Change(message) = event {
                    app.git.commit_message = message.clone();
                    cx.notify();
                }
            },
        ));

        let mut palette_subscriptions = Vec::new();
        palette_subscriptions.push(cx.subscribe(
            &palette_input,
            |app: &mut RustIdea, _input, event: &TextInputEvent, cx| match event {
                TextInputEvent::Change(query) => app.update_overlay_items(query, cx),
                TextInputEvent::Submit(_) => app.accept_overlay(cx),
                TextInputEvent::MoveSelection(delta) => app.move_overlay_selection(*delta, cx),
                TextInputEvent::Cancel => {
                    app.overlay = None;
                    cx.notify();
                }
            },
        ));

        let mut settings_subscriptions = Vec::new();
        settings_subscriptions.push(cx.subscribe(
            &settings_input,
            |app: &mut RustIdea, _input, event: &TextInputEvent, cx| {
                if let TextInputEvent::Change(value) = event {
                    app.settings_filter = value.clone();
                    cx.notify();
                }
            },
        ));

        let mut terminal_subscriptions = Vec::new();
        terminal_subscriptions.push(cx.subscribe(
            &terminal_input,
            |app: &mut RustIdea, _input, event: &TextInputEvent, cx| match event {
                TextInputEvent::Submit(command) => {
                    let command = command.clone();
                    if !command.is_empty() {
                        app.write_terminal(&format!("{command}\n"), cx);
                        app.terminal_input.update(cx, |input, cx| input.clear(cx));
                    }
                }
                TextInputEvent::Cancel => {
                    let _ = app;
                }
                TextInputEvent::Change(_) => {}
                TextInputEvent::MoveSelection(_) => {}
            },
        ));

        let mut http_subscriptions = Vec::new();
        http_subscriptions.push(cx.subscribe(
            &http_url_input,
            |app: &mut RustIdea, _input, event: &TextInputEvent, cx| {
                if matches!(event, TextInputEvent::Submit(_)) {
                    app.send_http_request(cx);
                }
            },
        ));

        let mut db_subscriptions = Vec::new();
        db_subscriptions.push(cx.subscribe(
            &db_query_input,
            |app: &mut RustIdea, _input, event: &TextInputEvent, cx| {
                if matches!(event, TextInputEvent::Submit(_)) {
                    app.run_db_query(cx);
                }
            },
        ));
        http_subscriptions.push(cx.subscribe(
            &http_body_input,
            |app: &mut RustIdea, _input, event: &TextInputEvent, cx| {
                if matches!(event, TextInputEvent::Submit(_)) {
                    app.send_http_request(cx);
                }
            },
        ));

        let host_for_pump = host.clone();
        let app = Self {
            focus_handle: cx.focus_handle(),
            host: host.clone(),
            theme,
            session_store,
            session,
            project: None,
            documents: DocumentStore::default(),
            pending_opens: HashSet::new(),
            groups: vec![EditorGroup::empty()],
            active_group: 0,
            split_axis: SplitAxis::Horizontal,
            sidebar_kind: SidebarKind::Project,
            sidebar_visible: true,
            right_visible: true,
            bottom_visible: true,
            sidebar_width: DEFAULT_LEFT_WIDTH,
            right_width: DEFAULT_RIGHT_WIDTH,
            bottom_height: DEFAULT_BOTTOM_HEIGHT,
            drag: None,
            context_menu: None,
            bottom_kind: BottomKind::Problems,
            status: "Ready".into(),
            problems: Vec::new(),
            problem_sources: HashMap::new(),
            search: SearchState::default(),
            search_input,
            search_replace_input,
            _search_subscriptions: search_subscriptions,
            git: GitState::default(),
            git_message_input,
            _git_subscriptions: git_subscriptions,
            overlay: None,
            palette_input,
            _palette_subscriptions: palette_subscriptions,
            settings: Settings::default(),
            settings_filter: String::new(),
            settings_input,
            _settings_subscriptions: settings_subscriptions,
            terminal_input,
            _terminal_subscriptions: terminal_subscriptions,
            terminals: Vec::new(),
            active_terminal: 0,
            run_configs: Vec::new(),
            selected_run: 0,
            run_output: String::new(),
            running_run: None,
            build_summary: None,
            build_output: String::new(),
            debug_attachables: Vec::new(),
            http_method: "GET".into(),
            http_url_input,
            http_headers_input,
            http_body_input,
            _http_subscriptions: http_subscriptions,
            http_response: String::new(),
            http_status: None,
            db_sources: Vec::new(),
            db_selected: None,
            db_tables: Vec::new(),
            db_schema: None,
            db_query_input,
            _db_subscriptions: db_subscriptions,
            db_result: String::new(),
            db_status: None,
            ssh_configs: Vec::new(),
            ssh_status: None,
            server_statuses: BTreeMap::new(),
            pending_close: None,
            opening_project: false,
            last_tree_epoch: 0,
            completion_items: Vec::new(),
            code_action_items: Vec::new(),
            pending_editor_focus: false,
            pending_ui_action: None,
            find_query: String::new(),
        };
        app.start_event_pump(host_for_pump, cx);
        app
    }

    fn start_event_pump(&self, host: Arc<NativeHost>, cx: &mut Context<Self>) {
        let Some(mut events) = host.take_events() else {
            return;
        };
        let overflow = events.overflow.clone();
        cx.spawn(async move |this, cx| {
            loop {
                tokio::select! {
                    event = events.receiver.recv() => {
                        let Some(event) = event else {
                            break;
                        };
                        if this
                            .update(cx, |app, cx| app.handle_ide_event(event, cx))
                            .is_err()
                        {
                            break;
                        }
                    }
                    _ = overflow.notified() => {
                        let report = overflow.take_report();
                        if this
                            .update(cx, |app, cx| app.handle_event_overflow(report, cx))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        })
        .detach();
    }

    pub fn restore_last_project(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.session.last_project.clone() else {
            return;
        };
        let runtime = self.host.runtime().clone();
        let classify_path = path.clone();
        cx.spawn(async move |this, cx| {
            let (is_dir, is_file) = runtime
                .spawn_blocking(move || (classify_path.is_dir(), classify_path.is_file()))
                .await
                .unwrap_or((false, false));
            this.update(cx, |app, cx| {
                if is_dir {
                    app.open_project(path, cx);
                } else if is_file {
                    app.ensure_project_for_file(path, None, cx);
                } else {
                    app.status = format!("Last project is unavailable: {}", path.display());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn open_startup_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let runtime = self.host.runtime().clone();
        let classify_path = path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let (is_dir, is_file) = runtime
                .spawn_blocking(move || (classify_path.is_dir(), classify_path.is_file()))
                .await
                .unwrap_or((false, false));
            this.update_in(cx, |app, window, cx| {
                if is_dir {
                    app.open_project(path, cx);
                } else if is_file {
                    app.ensure_project_for_file(path, Some(window), cx);
                } else {
                    app.status = format!("Startup path is unavailable: {}", path.display());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn active_group(&self) -> Option<&EditorGroup> {
        self.groups.get(self.active_group)
    }

    fn active_tab(&self) -> Option<&EditorTab> {
        let group = self.active_group()?;
        group.tabs.get(group.active?)
    }

    fn active_buffer(&self) -> Option<Entity<DocumentBuffer>> {
        self.active_tab().map(|tab| tab.buffer.clone())
    }

    fn active_editor(&self) -> Option<Entity<Editor>> {
        self.active_tab().map(|tab| tab.editor.clone())
    }

    fn add_tab(
        &mut self,
        buffer: Entity<DocumentBuffer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let theme = self.theme;
        let autosave = self.session.autosave;
        let editor = cx.new(|cx| Editor::new(buffer.clone(), theme, autosave, cx));
        let buffer_sub = cx.subscribe(
            &buffer,
            |app: &mut RustIdea, buffer, event: &BufferEvent, cx| {
                let pending = app
                    .pending_close
                    .as_ref()
                    .is_some_and(|pending| pending == &buffer);
                match event {
                    BufferEvent::Changed => return,
                    BufferEvent::Error(message) => {
                        app.status = message.clone();
                    }
                    BufferEvent::DirtyChanged => {}
                    BufferEvent::Saved => {
                        app.status = if buffer.read(cx).is_dirty() {
                            "Saved; newer edits are still pending".into()
                        } else {
                            "Saved".into()
                        };
                        if pending && !buffer.read(cx).is_dirty() {
                            app.pending_close = None;
                            if let Some((group, tab)) = app.find_tab_any(&buffer, cx) {
                                app.close_tab_at(group, tab, cx);
                            }
                        }
                    }
                    BufferEvent::TitleChanged => {
                        app.sync_document_key(&buffer, cx);
                        app.status = format!("Renamed to {}", buffer.read(cx).title());
                        if let Some(path) = buffer.read(cx).path().map(Path::to_path_buf) {
                            app.refresh_project_path(&path, cx);
                        }
                    }
                    BufferEvent::Reloaded => {
                        app.status = "Reloaded from disk".into();
                    }
                    BufferEvent::ExternalChanged => {
                        if let Some(message) = buffer.read(cx).external_change_message() {
                            app.status = message;
                        }
                    }
                    BufferEvent::ExternalResolved => {
                        app.status =
                            "Keeping the editor version; the next save will overwrite disk".into();
                    }
                    BufferEvent::SyncChanged => {}
                    BufferEvent::SyncConflict => {
                        if let Some(message) = buffer.read(cx).sync_conflict_message() {
                            app.status = message.to_owned();
                        }
                    }
                }
                cx.notify();
            },
        );
        let group = &mut self.groups[self.active_group];
        group.tabs.push(EditorTab {
            buffer,
            editor: editor.clone(),
            _subscriptions: vec![buffer_sub],
        });
        group.active = Some(group.tabs.len() - 1);
        let focus = editor.read(cx).focus_handle(cx);
        window.focus(&focus);
        cx.notify();
    }

    fn add_tab_without_window(&mut self, buffer: Entity<DocumentBuffer>, cx: &mut Context<Self>) {
        let theme = self.theme;
        let autosave = self.session.autosave;
        let editor = cx.new(|cx| Editor::new(buffer.clone(), theme, autosave, cx));
        let buffer_sub = cx.subscribe(
            &buffer,
            |app: &mut RustIdea, buffer, event: &BufferEvent, cx| {
                if matches!(event, BufferEvent::Changed | BufferEvent::DirtyChanged) {
                    cx.notify();
                    return;
                }
                if let BufferEvent::Error(message) = event {
                    app.status = message.clone();
                }
                if matches!(event, BufferEvent::TitleChanged) {
                    app.sync_document_key(&buffer, cx);
                }
                cx.notify();
            },
        );
        let group = &mut self.groups[self.active_group];
        group.tabs.push(EditorTab {
            buffer,
            editor,
            _subscriptions: vec![buffer_sub],
        });
        group.active = Some(group.tabs.len() - 1);
        cx.notify();
    }

    fn find_tab_any(&self, buffer: &Entity<DocumentBuffer>, _cx: &App) -> Option<(usize, usize)> {
        self.groups.iter().enumerate().find_map(|(group, tabs)| {
            tabs.tabs
                .iter()
                .position(|tab| tab.buffer == *buffer)
                .map(|tab| (group, tab))
        })
    }

    fn activate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(group) = self.groups.get_mut(self.active_group) else {
            return;
        };
        if index >= group.tabs.len() {
            return;
        }
        group.active = Some(index);
        let focus = group.tabs[index].editor.read(cx).focus_handle(cx);
        window.focus(&focus);
        cx.notify();
    }

    fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.active_group().and_then(|group| group.active) {
            self.close_tab(index, window, cx);
        }
    }

    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_group().and_then(|group| group.tabs.get(index)) else {
            return;
        };
        let buffer = tab.buffer.clone();
        if buffer.read(cx).is_saving() {
            self.status = "Wait for the current save before closing".into();
            cx.notify();
            return;
        }
        if !buffer.read(cx).is_dirty() {
            self.close_tab_at(self.active_group, index, cx);
            return;
        }
        let has_path = buffer.read(cx).path().is_some();
        let buttons: &[&str] = if has_path {
            &["Save and Close", "Discard", "Cancel"]
        } else {
            &["Discard", "Cancel"]
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            "This file has unsaved changes.",
            None,
            buttons,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let choice = answer.await.unwrap_or(buttons.len() - 1);
            this.update_in(cx, |app, window, cx| {
                let Some((group, tab)) = app.find_tab_any(&buffer, cx) else {
                    return;
                };
                if has_path {
                    match choice {
                        0 => {
                            app.pending_close = Some(buffer.clone());
                            buffer.update(cx, |buffer, cx| buffer.save(cx));
                            app.status = "Saving before close…".into();
                        }
                        1 => app.close_tab_at(group, tab, cx),
                        _ => {
                            let focus =
                                app.groups[group].tabs[tab].editor.read(cx).focus_handle(cx);
                            window.focus(&focus);
                        }
                    }
                } else if choice == 0 {
                    app.close_tab_at(group, tab, cx);
                } else {
                    let focus = app.groups[group].tabs[tab].editor.read(cx).focus_handle(cx);
                    window.focus(&focus);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn close_tab_at(&mut self, group_index: usize, index: usize, cx: &mut Context<Self>) {
        let Some(buffer) = self
            .groups
            .get(group_index)
            .and_then(|group| group.tabs.get(index))
            .map(|tab| tab.buffer.clone())
        else {
            return;
        };
        let Some(group) = self.groups.get_mut(group_index) else {
            return;
        };
        let tab = group.tabs.remove(index);
        drop(tab);
        if let Some(doc) = buffer.read(cx).host_doc()
            && self.documents.release_doc(doc).is_some()
        {
            let _ = self.host.close_document(doc);
        }
        group.active = match group.active {
            Some(_) if group.tabs.is_empty() => None,
            Some(active) if active > index => Some(active - 1),
            Some(active) if active == index => Some(index.min(group.tabs.len().saturating_sub(1))),
            other => other,
        };
        self.status = "Tab closed".into();
        cx.notify();
    }

    fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(group) = self.active_group() else {
            return;
        };
        if group.tabs.is_empty() {
            return;
        }
        let next = group
            .active
            .map_or(0, |index| (index + 1) % group.tabs.len());
        self.activate_tab(next, window, cx);
    }

    fn previous_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(group) = self.active_group() else {
            return;
        };
        if group.tabs.is_empty() {
            return;
        }
        let previous = group.active.map_or(0, |index| {
            if index == 0 {
                group.tabs.len() - 1
            } else {
                index - 1
            }
        });
        self.activate_tab(previous, window, cx);
    }

    fn split_active(&mut self, axis: SplitAxis, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.active_group().and_then(|group| group.active) else {
            self.status = "Open a file before splitting the editor".into();
            cx.notify();
            return;
        };
        let buffer = self.groups[self.active_group].tabs[source].buffer.clone();
        let Some(key) = self
            .documents
            .key_for_doc(buffer.read(cx).host_doc().unwrap())
            .cloned()
        else {
            self.status = "Document is no longer registered".into();
            cx.notify();
            return;
        };
        if self.documents.acquire(&key).is_none() {
            self.status = "Document is no longer registered".into();
            cx.notify();
            return;
        }
        let theme = self.theme;
        let autosave = self.session.autosave;
        let editor = cx.new(|cx| Editor::new(buffer.clone(), theme, autosave, cx));
        let buffer_sub = cx.subscribe(
            &buffer,
            |app: &mut RustIdea, buffer, event: &BufferEvent, cx| {
                if matches!(event, BufferEvent::Changed) {
                    return;
                }
                if let BufferEvent::Error(message) = event {
                    app.status = message.clone();
                }
                if matches!(event, BufferEvent::TitleChanged) {
                    app.sync_document_key(&buffer, cx);
                }
                cx.notify();
            },
        );
        self.groups.push(EditorGroup {
            tabs: vec![EditorTab {
                buffer,
                editor: editor.clone(),
                _subscriptions: vec![buffer_sub],
            }],
            active: Some(0),
        });
        self.active_group = self.groups.len() - 1;
        self.split_axis = axis;
        let focus = editor.read(cx).focus_handle(cx);
        window.focus(&focus);
        self.status = "Split created".into();
        cx.notify();
    }

    fn close_group(&mut self, cx: &mut Context<Self>) {
        if self.groups.len() == 1 {
            self.status = "There is only one editor group".into();
            cx.notify();
            return;
        }
        let group = self.groups.remove(self.active_group);
        for tab in group.tabs {
            if let Some(doc) = tab.buffer.read(cx).host_doc()
                && self.documents.release_doc(doc).is_some()
            {
                let _ = self.host.close_document(doc);
            }
        }
        self.active_group = self.active_group.min(self.groups.len() - 1);
        self.status = "Split closed".into();
        cx.notify();
    }

    fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let need_light = self.project.is_none();
        cx.spawn_in(window, async move |this, cx| {
            if need_light {
                let _ = host.open_light(None).await;
            }
            let result = host.open_untitled().await;
            this.update_in(cx, |app, window, cx| match result {
                Ok(info) => {
                    let key = DocumentKey::for_untitled(info.id);
                    let buffer =
                        cx.new(|_| DocumentBuffer::from_host(app.host.clone(), info.clone()));
                    if !app.documents.insert_view(key, info.id, buffer.clone()) {
                        app.status = "New file failed: document already registered".into();
                        return;
                    }
                    app.add_tab(buffer, window, cx);
                    app.status = "New file — use Save As to choose a path".into();
                }
                Err(error) => app.status = format!("New file failed: {}", error.message),
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn open_file_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open File".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            this.update_in(cx, |app, window, cx| match result {
                Ok(Some(paths)) => {
                    for path in paths {
                        app.open_path(path, None, Some(window), cx);
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    app.status = format!("Open File failed: {error}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn open_folder_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            this.update_in(cx, |app, window, cx| match result {
                Ok(Some(mut paths)) if !paths.is_empty() => {
                    let path = paths.remove(0);
                    app.open_project(path, cx);
                    window.focus(&app.focus_handle.clone());
                }
                Ok(_) => {}
                Err(error) => {
                    app.status = format!("Open Folder failed: {error}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn open_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.opening_project {
            return;
        }
        let path = crate::document_store::lexical_path(&path);
        self.opening_project = true;
        self.status = format!("Opening project {}…", path.display());
        self.host.close_project();
        self.project = None;
        self.documents.clear();
        self.pending_opens.clear();
        for group in &mut self.groups {
            group.tabs.clear();
            group.active = None;
        }
        let host = self.host.clone();
        let request_path = path.clone();
        cx.spawn(async move |this, cx| {
            let result = host.open_project(&request_path).await;
            this.update(cx, |app, cx| {
                app.opening_project = false;
                match result {
                    Ok(info) => {
                        let root = info.root.0.clone();
                        app.session.remember_project(&root);
                        app.persist_session(cx);
                        app.settings = info.settings.clone();
                        app.last_tree_epoch = app.last_tree_epoch.wrapping_add(1);
                        app.project = Some(project_state(root.clone(), info));
                        app.status = format!("Project {}", root.display());
                        app.load_project_directory(root, cx);
                        app.refresh_git(cx);
                    }
                    Err(error) => {
                        app.status = format!("Open project failed: {}", error.message);
                        tracing::error!(error = %error.message, path = %request_path.display(), "failed to open project");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn ensure_project_for_file(
        &mut self,
        path: PathBuf,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.project.is_some() {
            self.open_path(path, None, window, cx);
            return;
        }
        let host = self.host.clone();
        let open_path = path.clone();
        if window.is_some() {
            cx.spawn(async move |this, cx| {
                let result = host.open_light(Some(&open_path)).await;
                this.update(cx, |app, cx| {
                    if let Ok(info) = result {
                        let root = info.root.0.clone();
                        app.settings = info.settings.clone();
                        app.project = Some(project_state(root, info));
                    }
                    app.open_path(open_path, None, None, cx);
                })
                .ok();
            })
            .detach();
        } else {
            cx.spawn(async move |this, cx| {
                let result = host.open_light(Some(&open_path)).await;
                this.update(cx, |app, cx| {
                    if let Ok(info) = result {
                        let root = info.root.0.clone();
                        app.settings = info.settings.clone();
                        app.project = Some(project_state(root, info));
                    }
                    app.open_path(open_path, None, None, cx);
                })
                .ok();
            })
            .detach();
        }
    }

    fn open_path(
        &mut self,
        path: PathBuf,
        line: Option<u32>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let key = DocumentKey::for_ui_path(&path);
        if let Some(buffer) = self.documents.get(&key).cloned()
            && let Some((group, tab)) = self.find_tab_any(&buffer, cx)
        {
            self.active_group = group;
            self.groups[group].active = Some(tab);
            if let Some(window) = window {
                let focus = self.groups[group].tabs[tab]
                    .editor
                    .read(cx)
                    .focus_handle(cx);
                window.focus(&focus);
            }
            if let Some(line) = line {
                self.groups[group].tabs[tab]
                    .editor
                    .update(cx, |editor, cx| editor.go_to_line(line, cx));
            }
            cx.notify();
            return;
        }
        if self.documents.contains_key(&key) {
            if let Some(buffer) = self.documents.acquire(&key) {
                if let Some(window) = window {
                    self.add_tab(buffer, window, cx);
                } else {
                    self.add_tab_without_window(buffer, cx);
                }
                if let Some(line) = line
                    && let Some(tab) = self.active_tab()
                {
                    tab.editor
                        .update(cx, |editor, cx| editor.go_to_line(line, cx));
                }
            }
            cx.notify();
            return;
        }
        if !self.pending_opens.insert(key.clone()) {
            self.status = format!("{} is already opening", path.display());
            cx.notify();
            return;
        }
        let host = self.host.clone();
        let open_path = path.clone();
        self.status = format!("Opening {}…", display_name(&path));
        let open_key = key.clone();
        if let Some(window) = window {
            cx.spawn_in(window, async move |this, cx| {
                let result = host.open_document(&open_path).await;
                this.update_in(cx, |app, window, cx| {
                    app.finish_open_path(open_path, open_key, line, result, Some(window), cx);
                })
                .ok();
            })
            .detach();
        } else {
            cx.spawn(async move |this, cx| {
                let result = host.open_document(&open_path).await;
                this.update(cx, |app, cx| {
                    app.finish_open_path(open_path, open_key, line, result, None, cx);
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    fn finish_open_path(
        &mut self,
        path: PathBuf,
        key: DocumentKey,
        line: Option<u32>,
        result: ide_core::IdeResult<ide_workspace::DocumentInfo>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.pending_opens.remove(&key);
        match result {
            Ok(info) => {
                let info_key = DocumentKey::for_ui_path(&info.path.0);
                let (buffer, needs_view) =
                    if let Some(existing) = self.documents.get(&info_key).cloned() {
                        let _ = self.host.close_document(info.id);
                        (Some(existing), true)
                    } else {
                        let buffer =
                            cx.new(|_| DocumentBuffer::from_host(self.host.clone(), info.clone()));
                        if self
                            .documents
                            .insert_view(info_key.clone(), info.id, buffer.clone())
                        {
                            (Some(buffer), false)
                        } else {
                            let _ = self.host.close_document(info.id);
                            (self.documents.get(&info_key).cloned(), true)
                        }
                    };
                let Some(buffer) = buffer else {
                    self.status = format!("Open failed: {} is already registered", path.display());
                    cx.notify();
                    return;
                };
                buffer.update(cx, |buffer, cx| buffer.start_disk_baseline_hash(cx));
                if self.find_tab_any(&buffer, cx).is_some() {
                    if let Some((group, tab)) = self.find_tab_any(&buffer, cx) {
                        self.active_group = group;
                        self.groups[group].active = Some(tab);
                        if let Some(window) = window {
                            let focus = self.groups[group].tabs[tab]
                                .editor
                                .read(cx)
                                .focus_handle(cx);
                            window.focus(&focus);
                        }
                    }
                } else {
                    if needs_view && self.documents.acquire(&info_key).is_none() {
                        self.status =
                            format!("Open failed: {} is no longer registered", path.display());
                        cx.notify();
                        return;
                    }
                    if let Some(window) = window {
                        self.add_tab(buffer, window, cx);
                    } else {
                        self.add_tab_without_window(buffer, cx);
                    }
                }
                if let Some(line) = line
                    && let Some(tab) = self.active_tab()
                {
                    tab.editor
                        .update(cx, |editor, cx| editor.go_to_line(line, cx));
                }
                let info_path = crate::document_store::lexical_path(&info.path.0);
                if let Some(project) = self.project.as_mut() {
                    let missing = project.tree.reveal_path(&info_path);
                    if !missing.is_empty() {
                        self.load_project_directories(missing, cx);
                    }
                }
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => {
                self.status = format!("Open failed: {}", error.message);
                tracing::error!(error = %error.message, path = %path.display(), "failed to open file");
            }
        }
        cx.notify();
    }

    fn save_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.active_editor() else {
            self.status = "No active editor".into();
            cx.notify();
            return;
        };
        if editor.read(cx).path(cx).is_none() {
            self.save_active_as(window, cx);
            return;
        }
        editor.update(cx, |editor, cx| editor.save(cx));
        self.status = "Saving…".into();
        cx.notify();
    }

    fn save_all(&mut self, cx: &mut Context<Self>) {
        for group in &self.groups {
            for tab in &group.tabs {
                if tab.buffer.read(cx).path().is_some() {
                    tab.buffer.update(cx, |buffer, cx| buffer.save(cx));
                }
            }
        }
        self.status = "Saving all modified documents…".into();
        cx.notify();
    }

    fn save_active_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(buffer) = self.active_buffer() else {
            self.status = "No active editor".into();
            cx.notify();
            return;
        };
        let title = buffer.read(cx).title();
        let directory = buffer
            .read(cx)
            .path()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .or_else(|| self.project.as_ref().map(|project| project.root.clone()))
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let receiver = cx.prompt_for_new_path(&directory, Some(&title));
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            this.update_in(cx, |app, _window, cx| match result {
                Ok(Some(path)) => {
                    buffer.update(cx, |buffer, cx| buffer.save_as(path, cx));
                    app.status = "Saving…".into();
                }
                Ok(None) => app.status = "Save As cancelled".into(),
                Err(error) => app.status = format!("Save As failed: {error}"),
            })
            .ok();
        })
        .detach();
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.session.theme = self.session.theme.toggled();
        self.theme = Theme::for_mode(self.session.theme);
        for group in &self.groups {
            for tab in &group.tabs {
                tab.editor
                    .update(cx, |editor, _| editor.set_theme(self.theme));
            }
        }
        for input in [
            &self.search_input,
            &self.search_replace_input,
            &self.git_message_input,
            &self.palette_input,
            &self.settings_input,
            &self.terminal_input,
            &self.http_url_input,
            &self.http_headers_input,
            &self.http_body_input,
            &self.db_query_input,
        ] {
            input.update(cx, |input, cx| {
                input.set_colors(
                    self.theme.text,
                    self.theme.faint,
                    self.theme.selection,
                    self.theme.cursor,
                    cx,
                )
            });
        }
        self.persist_session(cx);
        self.status = format!("{} theme", self.theme.mode.name());
        cx.notify();
    }

    fn toggle_autosave(&mut self, cx: &mut Context<Self>) {
        self.session.autosave = !self.session.autosave;
        for group in &self.groups {
            for tab in &group.tabs {
                tab.editor.update(cx, |editor, _| {
                    editor.set_autosave(self.session.autosave);
                });
            }
        }
        self.persist_session(cx);
        self.status = if self.session.autosave {
            "Autosave enabled".into()
        } else {
            "Autosave disabled".into()
        };
        cx.notify();
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    fn toggle_bottom(&mut self, cx: &mut Context<Self>) {
        self.bottom_visible = !self.bottom_visible;
        cx.notify();
    }

    fn toggle_right(&mut self, cx: &mut Context<Self>) {
        self.right_visible = !self.right_visible;
        cx.notify();
    }

    fn show_sidebar(&mut self, kind: SidebarKind, cx: &mut Context<Self>) {
        self.sidebar_kind = kind;
        self.sidebar_visible = true;
        match kind {
            SidebarKind::Git => self.refresh_git(cx),
            SidebarKind::Settings => self.reload_settings(cx),
            SidebarKind::Database => self.load_db_sources(cx),
            SidebarKind::Remote => self.load_ssh_configs(cx),
            _ => {}
        }
        cx.notify();
    }

    fn show_bottom(&mut self, kind: BottomKind, cx: &mut Context<Self>) {
        self.bottom_kind = kind;
        self.bottom_visible = true;
        match kind {
            BottomKind::Problems => self.rebuild_problems(cx),
            BottomKind::VersionControl => self.refresh_git(cx),
            BottomKind::Run => {
                if self.run_configs.is_empty() {
                    self.load_run_configs(cx);
                }
            }
            BottomKind::Debug => {
                if self.debug_attachables.is_empty() {
                    self.refresh_debug(cx);
                }
            }
            BottomKind::Build if self.build_summary.is_none() => self.load_build_model(cx),
            _ => {}
        }
        cx.notify();
    }

    fn reload_settings(&mut self, cx: &mut Context<Self>) {
        self.settings = self.host.settings();
        cx.notify();
    }

    fn persist_session(&self, _cx: &mut Context<Self>) {
        let store = self.session_store.clone();
        let session = self.session.clone();
        self.host.runtime().spawn_blocking(move || {
            if let Err(error) = store.save(&session) {
                tracing::warn!(%error, "failed to persist RustIdea session");
            }
        });
    }

    fn toggle_project_node(
        &mut self,
        index: usize,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let action = self
            .project
            .as_mut()
            .map(|project| project.tree.toggle(index))
            .unwrap_or(Toggle::Missing);
        match action {
            Toggle::OpenFile(path) => self.open_path(path, None, window, cx),
            Toggle::LoadDirectory(directory) => {
                self.load_project_directory(directory, cx);
            }
            Toggle::Rebuilt => cx.notify(),
            Toggle::Symlink => {
                self.status = "Symlink directories are shown but not expanded automatically".into();
                cx.notify();
            }
            Toggle::Missing => {}
        }
    }

    fn load_project_directory(&mut self, directory: PathBuf, cx: &mut Context<Self>) {
        self.load_project_directories(vec![directory], cx);
    }

    /// Loads one or more directory levels on the backend runtime, one at a
    /// time. No filesystem call runs on the GPUI thread; results are applied
    /// only if the project generation still matches.
    fn load_project_directories(&mut self, directories: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let epoch = self.last_tree_epoch;
        let root = project.root.clone();
        let mut requests = Vec::new();
        for directory in directories {
            if directory.starts_with(&root)
                && let Some(generation) = project.tree.request_load(&directory)
            {
                requests.push((directory, generation));
            }
        }
        if requests.is_empty() {
            return;
        }
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            for (directory, generation) in requests {
                let result = host.read_dir(&directory).await;
                let keep_going = this
                    .update(cx, |app, cx| {
                        if app.last_tree_epoch != epoch {
                            return false;
                        }
                        let Some(project) = app.project.as_mut() else {
                            return false;
                        };
                        match result {
                            Ok(listing) => {
                                project.tree.apply_directory_if_current(
                                    directory.clone(),
                                    generation,
                                    listing,
                                );
                            }
                            Err(error) => {
                                if project.tree.is_current_load(&directory, generation) {
                                    project.tree.apply_error(directory, error.message);
                                }
                            }
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn reload_project_tree(&mut self, cx: &mut Context<Self>) {
        self.last_tree_epoch = self.last_tree_epoch.wrapping_add(1);
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let loaded = project.tree.invalidate_loaded();
        let directories = if loaded.is_empty() {
            vec![project.root.clone()]
        } else {
            loaded
        };
        self.load_project_directories(directories, cx);
    }

    /// Refreshes exactly the directory that contains `path` (and the path's
    /// own listing when it is a directory), leaving every other loaded
    /// directory cached.
    fn refresh_project_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let mut reload = Vec::new();
        if let Some(parent) = path.parent()
            && parent.starts_with(&project.root)
        {
            project.tree.invalidate(parent);
            reload.push(parent.to_path_buf());
        }
        if path.starts_with(&project.root) {
            project.tree.invalidate(path);
        }
        if !reload.is_empty() {
            self.load_project_directories(reload, cx);
        }
    }

    fn refresh_project(&mut self, cx: &mut Context<Self>) {
        if self.project.is_some() {
            self.reload_project_tree(cx);
            self.refresh_git(cx);
        }
        cx.notify();
    }

    fn new_project_file(&mut self, window: &mut Window, directory: bool, cx: &mut Context<Self>) {
        let Some(project) = self.project.as_ref() else {
            self.status = "Open a project first".into();
            cx.notify();
            return;
        };
        let base = project.root.clone();
        let prompt = if directory {
            "New folder path"
        } else {
            "New file path"
        };
        let receiver = cx.prompt_for_new_path(&base, Some(prompt));
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            this.update_in(cx, |app, window, cx| match result {
                Ok(Some(path)) => {
                    if directory {
                        let host = app.host.clone();
                        let create_path = path.clone();
                        cx.spawn_in(window, async move |this, cx| {
                            let result = host.create_dir_async(create_path).await;
                            this.update_in(cx, |app, _window, cx| match result {
                                Ok(()) => {
                                    app.status = format!("Created folder {}", path.display());
                                    app.refresh_project_path(&path, cx);
                                }
                                Err(error) => {
                                    app.status = format!("Create failed: {}", error.message)
                                }
                            })
                            .ok();
                        })
                        .detach();
                    } else {
                        let host = app.host.clone();
                        let create_path = path.clone();
                        cx.spawn_in(window, async move |this, cx| {
                            let runtime = host.runtime().clone();
                            let host_handle = host.host().clone();
                            let result = runtime
                                .spawn_blocking(move || {
                                    ide_host::ops::fs::create_file(
                                        &host_handle,
                                        crate::native_host::MAIN_WINDOW,
                                        create_path.clone(),
                                    )
                                })
                                .await;
                            this.update_in(cx, |app, window, cx| match result {
                                Ok(Ok(())) => {
                                    app.status = format!("Created file {}", path.display());
                                    app.refresh_project_path(&path, cx);
                                    app.open_path(path.clone(), None, Some(window), cx);
                                }
                                Ok(Err(error)) => {
                                    app.status = format!("Create failed: {}", error.message)
                                }
                                Err(error) => app.status = format!("Create failed: {error}"),
                            })
                            .ok();
                        })
                        .detach();
                    }
                }
                Ok(None) => {}
                Err(error) => app.status = format!("Create failed: {error}"),
            })
            .ok();
        })
        .detach();
    }

    fn delete_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let buttons = &["Move to Trash", "Cancel"];
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Delete {}?", path.display()),
            None,
            buttons,
            cx,
        );
        let host = self.host.clone();
        cx.spawn_in(window, async move |this, cx| {
            let choice = answer.await.unwrap_or(1);
            this.update_in(cx, |_app, _window, cx| {
                if choice == 0 {
                    let delete_path = path.clone();
                    let delete_task_path = delete_path.clone();
                    cx.spawn(async move |this, cx| {
                        let result = host
                            .run(move |backend| async move {
                                ide_host::ops::fs::delete(
                                    &backend,
                                    crate::native_host::MAIN_WINDOW,
                                    delete_task_path,
                                )
                                .await
                            })
                            .await;
                        this.update(cx, |app, cx| match result {
                            Ok(()) => {
                                app.status = format!("Moved {} to trash", delete_path.display());
                                app.refresh_project_path(&delete_path, cx);
                            }
                            Err(error) => app.status = format!("Delete failed: {}", error.message),
                        })
                        .ok();
                    })
                    .detach();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn create_child_path(
        &mut self,
        parent: PathBuf,
        directory: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prompt = if directory {
            "New folder name"
        } else {
            "New file name"
        };
        let receiver = cx.prompt_for_new_path(&parent, Some(prompt));
        let host = self.host.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            this.update_in(cx, |app, window, cx| match result {
                Ok(Some(path)) => {
                    if directory {
                        let host = host.clone();
                        let create_path = path.clone();
                        cx.spawn_in(window, async move |this, cx| {
                            let result = host.create_dir_async(create_path).await;
                            this.update_in(cx, |app, _window, cx| match result {
                                Ok(()) => {
                                    app.status = format!("Created folder {}", path.display());
                                    app.refresh_project_path(&path, cx);
                                }
                                Err(error) => {
                                    app.status = format!("Create failed: {}", error.message)
                                }
                            })
                            .ok();
                        })
                        .detach();
                    } else {
                        let runtime = host.runtime().clone();
                        let host_handle = host.host().clone();
                        let create_path = path.clone();
                        cx.spawn_in(window, async move |this, cx| {
                            let result = runtime
                                .spawn_blocking(move || {
                                    ide_host::ops::fs::create_file(
                                        &host_handle,
                                        crate::native_host::MAIN_WINDOW,
                                        create_path,
                                    )
                                })
                                .await;
                            this.update_in(cx, |app, window, cx| match result {
                                Ok(Ok(())) => {
                                    app.status = format!("Created file {}", path.display());
                                    app.refresh_project_path(&path, cx);
                                    app.open_path(path.clone(), None, Some(window), cx);
                                }
                                Ok(Err(error)) => {
                                    app.status = format!("Create failed: {}", error.message)
                                }
                                Err(error) => app.status = format!("Create failed: {error}"),
                            })
                            .ok();
                        })
                        .detach();
                    }
                }
                Ok(None) => {}
                Err(error) => app.status = format!("Create failed: {error}"),
            })
            .ok();
        })
        .detach();
    }

    fn rename_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let directory = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&directory, Some(&name));
        let host = self.host.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            this.update_in(cx, |app, _window, cx| match result {
                Ok(Some(new_path)) => {
                    if new_path == path {
                        return;
                    }
                    let old_path = path.clone();
                    let host = host.clone();
                    cx.spawn(async move |this, cx| {
                        let old_task_path = old_path.clone();
                        let new_task_path = new_path.clone();
                        let result = host
                            .run(move |backend| async move {
                                ide_host::ops::fs::rename(
                                    &backend,
                                    crate::native_host::MAIN_WINDOW,
                                    old_task_path,
                                    new_task_path,
                                )
                                .await
                            })
                            .await;
                        this.update(cx, |app, cx| {
                            match result {
                                Ok(()) => {
                                    app.status = format!("Renamed to {}", new_path.display());
                                    app.refresh_project_path(&old_path, cx);
                                    app.refresh_project_path(&new_path, cx);
                                }
                                Err(error) => {
                                    app.status = format!("Rename failed: {}", error.message)
                                }
                            }
                            cx.notify();
                        })
                        .ok();
                    })
                    .detach();
                }
                Ok(None) => {}
                Err(error) => app.status = format!("Rename failed: {error}"),
            })
            .ok();
        })
        .detach();
    }

    fn reveal_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let runtime = self.host.runtime().clone();
        let reveal_path = path.clone();
        runtime.spawn_blocking(move || {
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("open")
                    .arg("-R")
                    .arg(&reveal_path)
                    .spawn();
            }
            #[cfg(not(target_os = "macos"))]
            {
                if let Some(parent) = reveal_path.parent() {
                    let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
                }
            }
        });
        self.status = format!("Revealed {}", path.display());
        cx.notify();
    }

    fn trust_project(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let root = project.root.clone();
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.set_project_trust(&root, true, false).await;
            this.update(cx, |app, _cx| match result {
                Ok(()) => {
                    if let Some(project) = app.project.as_mut() {
                        project.info.trusted = true;
                    }
                    app.status = "Project trusted".into();
                }
                Err(error) => app.status = format!("Trust failed: {}", error.message),
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Apply one settings key through the shared backend, exactly as the
    /// OpenIDEA settings IPC does, then mirror the effective values.
    fn set_setting(&mut self, key: String, value: serde_json::Value, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let serialized = value.to_string();
        self.status = format!("Applying {key}…");
        cx.spawn(async move |this, cx| {
            let runtime = host.runtime().clone();
            let host_handle = host.host().clone();
            let key_for_task = key.clone();
            let result = runtime
                .spawn_blocking(move || {
                    ide_host::ops::project::set_settings(
                        &host_handle,
                        crate::native_host::MAIN_WINDOW,
                        ide_workspace::settings::SettingsScope::Global,
                        &key_for_task,
                        &serialized,
                    )
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(Ok(applied)) => {
                        app.settings = applied.settings;
                        app.status = format!("Updated {key}");
                    }
                    Ok(Err(error)) => {
                        app.status = format!("Setting {key} failed: {}", error.message)
                    }
                    Err(error) => app.status = format!("Setting {key} failed: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Start a local PTY for the bottom Terminal panel.
    fn new_terminal(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self
            .project
            .as_ref()
            .map(|project| project.root.clone())
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let host = self.host.clone();
        let options = ide_terminal::TerminalOptions {
            cwd: ide_core::IpcPath(cwd),
            shell: None,
            args: Vec::new(),
            env: Default::default(),
            cols: 120,
            rows: 30,
            shell_integration: true,
        };
        self.status = "Starting terminal…".into();
        cx.spawn(async move |this, cx| {
            let (tx, mut output) = tokio::sync::mpsc::unbounded_channel::<String>();
            let runtime = host.runtime().clone();
            let host_handle = host.host().clone();
            let result = runtime
                .spawn_blocking(move || {
                    let emit: ide_host::ops::Emit<String> = Arc::new(move |chunk| {
                        let _ = tx.send(chunk);
                    });
                    ide_host::ops::term::spawn(
                        &host_handle,
                        crate::native_host::MAIN_WINDOW,
                        options,
                        emit,
                    )
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(Ok(info)) => {
                        app.status = format!(
                            "Terminal {} started (pid {})",
                            info.title,
                            info.pid
                                .map(|pid| pid.to_string())
                                .unwrap_or_else(|| "?".into())
                        );
                        app.bottom_kind = BottomKind::Terminal;
                        app.bottom_visible = true;
                        let id = info.id;
                        app.active_terminal = app.terminals.len();
                        app.terminals.push(TerminalTab {
                            info,
                            output: String::new(),
                        });
                        cx.spawn(async move |this, cx| {
                            while let Some(chunk) = output.recv().await {
                                this.update(cx, |app, cx| {
                                    if let Some(terminal) = app
                                        .terminals
                                        .iter_mut()
                                        .find(|terminal| terminal.info.id == id)
                                    {
                                        terminal.output.push_str(&chunk);
                                        if terminal.output.len() > 4_000_000 {
                                            let split =
                                                terminal.output.len().saturating_sub(3_500_000);
                                            terminal.output.drain(..split);
                                        }
                                        cx.notify();
                                    }
                                })
                                .ok();
                            }
                        })
                        .detach();
                    }
                    Ok(Err(error)) => app.status = format!("Terminal failed: {}", error.message),
                    Err(error) => app.status = format!("Terminal failed: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn attach_terminal_stream(
        &mut self,
        id: ide_core::TerminalId,
        mut output: tokio::sync::mpsc::UnboundedReceiver<String>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Some(chunk) = output.recv().await {
                this.update(cx, |app, cx| {
                    if let Some(terminal) = app
                        .terminals
                        .iter_mut()
                        .find(|terminal| terminal.info.id == id)
                    {
                        terminal.output.push_str(&chunk);
                        if terminal.output.len() > 4_000_000 {
                            let split = terminal.output.len().saturating_sub(3_500_000);
                            terminal.output.drain(..split);
                        }
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn load_ssh_configs(&mut self, cx: &mut Context<Self>) {
        self.ssh_configs = self.host.host().ssh.configs();
        self.ssh_status = Some(format!("{} SSH configuration(s)", self.ssh_configs.len()));
        cx.notify();
    }

    fn connect_ssh(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(config) = self.ssh_configs.get(index).cloned() else {
            return;
        };
        let host = self.host.clone();
        let id = config.id.clone();
        self.ssh_status = Some(format!("Connecting {}…", config.label()));
        cx.spawn(async move |this, cx| {
            let (tx, output) = tokio::sync::mpsc::unbounded_channel::<String>();
            let emit: ide_host::ops::Emit<String> = Arc::new(move |chunk| {
                let _ = tx.send(chunk);
            });
            let runtime = host.runtime().clone();
            let host_handle = host.host().clone();
            let result = runtime
                .spawn(async move {
                    ide_host::ops::term::spawn_ssh(
                        &host_handle,
                        crate::native_host::MAIN_WINDOW,
                        &id,
                        120,
                        30,
                        emit,
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(Ok(info)) => {
                        app.ssh_status = Some(format!("Connected {}", info.title));
                        app.status = format!("SSH terminal {} started", info.title);
                        app.bottom_kind = BottomKind::Terminal;
                        app.bottom_visible = true;
                        app.active_terminal = app.terminals.len();
                        let terminal_id = info.id;
                        app.terminals.push(TerminalTab {
                            info,
                            output: String::new(),
                        });
                        app.attach_terminal_stream(terminal_id, output, cx);
                    }
                    Ok(Err(error)) => {
                        app.ssh_status = Some(error.message.clone());
                        app.status = format!("SSH connection failed: {}", error.message);
                    }
                    Err(error) => {
                        app.ssh_status = Some(format!("SSH task failed: {error}"));
                        app.status = format!("SSH connection failed: {error}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn disconnect_ssh(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(config) = self.ssh_configs.get(index).cloned() else {
            return;
        };
        self.host.host().ssh.disconnect(&config.id);
        self.ssh_status = Some(format!("Disconnected {}", config.label()));
        self.status = format!("Disconnected {}", config.label());
        cx.notify();
    }

    fn load_run_configs(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.status = "Loading run configurations…".into();
        cx.spawn(async move |this, cx| {
            let runtime = host.runtime().clone();
            let host_handle = host.host().clone();
            let result = runtime
                .spawn_blocking(move || {
                    ide_host::ops::run::configs(&host_handle, crate::native_host::MAIN_WINDOW)
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(Ok(configs)) => {
                        let names: Vec<String> = configs
                            .iter()
                            .take(6)
                            .map(|config| config.name.clone())
                            .collect();
                        app.status = if names.is_empty() {
                            "No run configurations detected".into()
                        } else {
                            format!("{} run configurations: {}", configs.len(), names.join(", "))
                        };
                        app.run_configs = configs;
                        app.selected_run = 0;
                    }
                    Ok(Err(error)) => app.status = format!("Run configs failed: {}", error.message),
                    Err(error) => app.status = format!("Run configs failed: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn write_terminal(&mut self, data: &str, cx: &mut Context<Self>) {
        let Some(terminal) = self.terminals.get(self.active_terminal) else {
            self.status = "Open a terminal first".into();
            cx.notify();
            return;
        };
        let id = terminal.info.id;
        if let Err(error) = ide_host::ops::term::write(self.host.host(), id, data) {
            self.status = format!("Terminal write failed: {}", error.message);
        }
        cx.notify();
    }

    fn close_terminal(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(terminal) = self.terminals.get(index) else {
            return;
        };
        let id = terminal.info.id;
        if let Err(error) = ide_host::ops::term::kill(self.host.host(), id) {
            self.status = format!("Terminal close failed: {}", error.message);
        }
        self.terminals.remove(index);
        if self.active_terminal >= self.terminals.len() {
            self.active_terminal = self.terminals.len().saturating_sub(1);
        }
        cx.notify();
    }

    fn stop_run(&mut self, cx: &mut Context<Self>) {
        let Some(info) = self.running_run.take() else {
            self.status = "No run configuration is running".into();
            cx.notify();
            return;
        };
        if let Err(error) =
            ide_host::ops::run::stop(self.host.host(), crate::native_host::MAIN_WINDOW, info.id)
        {
            self.status = format!("Stop failed: {}", error.message);
            self.running_run = Some(info);
        } else {
            self.status = format!("Stopped {}", info.name);
        }
        cx.notify();
    }

    fn send_http_request(&mut self, cx: &mut Context<Self>) {
        let url = self.http_url_input.read(cx).content().trim().to_owned();
        if url.is_empty() {
            self.status = "Enter a request URL".into();
            cx.notify();
            return;
        }
        let method = self.http_method.clone();
        let headers = self.http_headers_input.read(cx).content().trim().to_owned();
        let body = self.http_body_input.read(cx).content().to_owned();
        let headers = headers
            .split(';')
            .filter_map(|header| {
                let (name, value) = header.split_once(':')?;
                Some(format!("{}: {}", name.trim(), value.trim()))
            })
            .collect::<Vec<_>>()
            .join("\n");
        let request_text = if body.trim().is_empty() {
            format!("{method} {url}\n{headers}\n")
        } else {
            format!("{method} {url}\n{headers}\n\n{body}")
        };
        let root = self.project.as_ref().map(|project| project.root.clone());
        let host = self.host.clone();
        self.status = format!("{method} {url}…");
        self.http_response.clear();
        self.http_status = None;
        let (tx, mut rx) =
            tokio::sync::mpsc::unbounded_channel::<ide_core::IdeResult<ide_http::HttpExchange>>();
        let runtime = host.runtime().clone();
        runtime.spawn(async move {
            let cancel = ide_core::CancellationToken::new();
            let root_ref = root.as_deref();
            let result = ide_http::run(
                ide_http::RunInput {
                    text: &request_text,
                    index: 0,
                    file: None,
                    root: root_ref,
                    env: None,
                },
                &cancel,
            )
            .await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |this, cx| {
            if let Some(result) = rx.recv().await {
                this.update(cx, |app, cx| {
                    match result {
                        Ok(exchange) => {
                            let response = exchange.response;
                            match response {
                                Some(response) => {
                                    let mut text = format!(
                                        "{} {} · {} · {:.1} ms\n",
                                        response.status,
                                        response.status_text,
                                        response.version,
                                        response.total_ms
                                    );
                                    for (name, value) in &response.headers {
                                        text.push_str(&format!("{name}: {value}\n"));
                                    }
                                    text.push('\n');
                                    text.push_str(
                                        response.body.as_deref().unwrap_or("(binary body)"),
                                    );
                                    app.http_status = Some(format!(
                                        "{} {}",
                                        response.status, response.status_text
                                    ));
                                    app.http_response = text;
                                    app.status = format!("HTTP {} received", response.status);
                                }
                                None => {
                                    let error = exchange
                                        .entry
                                        .error
                                        .unwrap_or_else(|| "request failed".into());
                                    app.http_status = Some("request failed".into());
                                    app.http_response = error.clone();
                                    app.status = format!("HTTP failed: {error}");
                                }
                            }
                        }
                        Err(error) => {
                            app.http_status = Some("invalid request".into());
                            app.http_response = error.message.clone();
                            app.status = format!("HTTP request failed: {}", error.message);
                        }
                    }
                    app.bottom_visible = true;
                    app.bottom_kind = BottomKind::Output;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
    }

    fn load_db_sources(&mut self, cx: &mut Context<Self>) {
        match ide_host::ops::db::sources(self.host.host(), crate::native_host::MAIN_WINDOW) {
            Ok(sources) => {
                self.db_sources = sources;
                if self.db_selected.is_none() {
                    self.db_selected = self.db_sources.first().map(|source| source.id.clone());
                }
                self.db_status = Some(format!("{} data source(s)", self.db_sources.len()));
            }
            Err(error) => {
                self.db_status = Some(error.message.clone());
                self.status = format!("Database sources failed: {}", error.message);
            }
        }
        cx.notify();
    }

    fn add_sqlite_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Add SQLite database".into()),
        });
        let host = self.host.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            let Ok(Some(mut paths)) = result else {
                return;
            };
            let Some(path) = paths.pop() else {
                return;
            };
            let name = path
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "SQLite".into());
            let id = format!(
                "sqlite-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis())
                    .unwrap_or_default()
            );
            let mut source = ide_db::DataSource {
                id,
                name,
                driver: ide_db::DbDriver::Sqlite,
                file: Some(ide_core::IpcPath(path)),
                ..Default::default()
            };
            source.read_only = false;
            let runtime = host.runtime().clone();
            let host_handle = host.host().clone();
            let save_id = source.id.clone();
            let save_name = source.name.clone();
            let result = runtime
                .spawn(async move {
                    let mut sources =
                        ide_host::ops::db::sources(&host_handle, crate::native_host::MAIN_WINDOW)?;
                    sources.retain(|existing| existing.id != source.id);
                    sources.push(source);
                    ide_host::ops::db::save_sources(
                        &host_handle,
                        crate::native_host::MAIN_WINDOW,
                        sources,
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(Ok(())) => {
                        app.status = format!("Added SQLite data source {save_name}");
                        app.db_selected = Some(save_id);
                        app.load_db_sources(cx);
                    }
                    Ok(Err(error)) => {
                        app.status = format!("Add data source failed: {}", error.message)
                    }
                    Err(error) => app.status = format!("Add data source failed: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn connect_selected_db(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.db_selected.clone() else {
            self.db_status = Some("Select a data source first".into());
            cx.notify();
            return;
        };
        let host = self.host.clone();
        self.db_status = Some("Connecting…".into());
        cx.spawn(async move |this, cx| {
            let runtime = host.runtime().clone();
            let host_handle = host.host().clone();
            let id_for_task = id.clone();
            let result = runtime
                .spawn(async move {
                    ide_host::ops::db::connect(
                        &host_handle,
                        crate::native_host::MAIN_WINDOW,
                        &id_for_task,
                    )
                    .await?;
                    let schemas = ide_host::ops::db::schemas(
                        &host_handle,
                        crate::native_host::MAIN_WINDOW,
                        &id_for_task,
                    )
                    .await?;
                    let schema = schemas.first().cloned().unwrap_or_default();
                    let tables = ide_host::ops::db::tables(
                        &host_handle,
                        crate::native_host::MAIN_WINDOW,
                        &id_for_task,
                        &schema,
                    )
                    .await?;
                    Ok::<_, ide_core::IdeError>((schema, tables))
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(Ok((schema, tables))) => {
                        app.db_schema = Some(schema.clone());
                        app.db_tables = tables.into_iter().map(|table| table.name).collect();
                        app.db_status = Some(format!(
                            "Connected · schema {} · {} tables",
                            schema,
                            app.db_tables.len()
                        ));
                        app.status = format!("Connected to data source {id}");
                    }
                    Ok(Err(error)) => {
                        app.db_status = Some(error.message.clone());
                        app.status = format!("Database connect failed: {}", error.message);
                    }
                    Err(error) => app.db_status = Some(format!("Database connect failed: {error}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn run_db_query(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.db_selected.clone() else {
            self.db_status = Some("Select a data source first".into());
            cx.notify();
            return;
        };
        let sql = self.db_query_input.read(cx).content().trim().to_owned();
        if sql.is_empty() {
            return;
        }
        let schema = self.db_schema.clone();
        let host = self.host.clone();
        self.db_status = Some("Running query…".into());
        cx.spawn(async move |this, cx| {
            let runtime = host.runtime().clone();
            let host_handle = host.host().clone();
            let request = RequestId::next();
            let text = sql.clone();
            let statement = ide_db::split::SqlStatement {
                from: 0,
                to: text.encode_utf16().count() as u32,
                text,
            };
            let result = runtime
                .spawn(async move {
                    ide_host::ops::db::execute(
                        &host_handle,
                        crate::native_host::MAIN_WINDOW,
                        request,
                        &id,
                        vec![statement],
                        0,
                        200,
                        schema.as_deref(),
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(Ok(outcomes)) => {
                        let mut text = String::new();
                        for outcome in outcomes {
                            if let Some(error) = outcome.error {
                                text.push_str(&format!("ERROR: {error}\n"));
                                continue;
                            }
                            if let Some(result) = outcome.result {
                                if let Some(affected) = result.affected {
                                    text.push_str(&format!("{affected} rows affected\n"));
                                }
                                if !result.columns.is_empty() {
                                    text.push_str(
                                        &result
                                            .columns
                                            .iter()
                                            .map(|column| column.name.clone())
                                            .collect::<Vec<_>>()
                                            .join(" | "),
                                    );
                                    text.push('\n');
                                    for row in result.rows {
                                        text.push_str(
                                            &row.into_iter()
                                                .map(|cell| cell.unwrap_or_else(|| "NULL".into()))
                                                .collect::<Vec<_>>()
                                                .join(" | "),
                                        );
                                        text.push('\n');
                                    }
                                }
                            }
                        }
                        app.db_result = text;
                        app.db_status = Some("Query finished".into());
                    }
                    Ok(Err(error)) => {
                        app.db_result = error.message.clone();
                        app.db_status = Some(format!("Query failed: {}", error.message));
                    }
                    Err(error) => {
                        app.db_result = format!("{error}");
                        app.db_status = Some(format!("Query failed: {error}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn run_active_config(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(config) = self.run_configs.get(self.selected_run).cloned() else {
            self.status = "Refresh run configurations first".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        self.status = format!("Starting {}…", config.name);
        self.run_output.clear();
        self.run_output
            .push_str(&format!("$ {} {}\n", config.command, config.args.join(" ")));
        let name = config.name.clone();
        let (tx, mut rx) =
            tokio::sync::mpsc::unbounded_channel::<Result<ide_run::RunInfo, ide_core::IdeError>>();
        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<ide_run::OutputChunk>();
        let runtime = host.runtime().clone();
        let host_handle = host.host().clone();
        runtime.spawn(async move {
            let emit: ide_host::ops::Emit<ide_run::OutputChunk> = Arc::new(move |chunk| {
                let _ = out_tx.send(chunk);
            });
            let result = ide_host::ops::run::start(
                &host_handle,
                crate::native_host::MAIN_WINDOW,
                config,
                emit,
            )
            .await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |this, cx| {
            loop {
                tokio::select! {
                    Some(chunk) = out_rx.recv() => {
                        this.update(cx, |app, cx| {
                            let text = match chunk {
                                ide_run::OutputChunk::Stdout { text }
                                | ide_run::OutputChunk::Stderr { text }
                                | ide_run::OutputChunk::System { text } => text,
                            };
                            app.run_output.push_str(&text);
                            if app.run_output.len() > 2_000_000 {
                                let split = app.run_output.len().saturating_sub(1_800_000);
                                app.run_output.drain(..split);
                            }
                            cx.notify();
                        }).ok();
                    }
                    Some(result) = rx.recv() => {
                        this.update(cx, |app, cx| {
                            match result {
                                Ok(info) => {
                                    app.status = format!("Started {} (pid {:?})", info.name, info.pid);
                                    app.running_run = Some(info);
                                    app.bottom_kind = BottomKind::Run;
                                    app.bottom_visible = true;
                                }
                                Err(error) => {
                                    app.status = format!("Run {name} failed: {}", error.message);
                                    app.run_output.push_str(&format!("\nRun failed: {}\n", error.message));
                                }
                            }
                            cx.notify();
                        }).ok();
                        break;
                    }
                    else => break,
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn refresh_debug(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.status = "Scanning attachable processes…".into();
        cx.spawn(async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    ide_host::ops::debug::debug_attachable_processes(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(processes) => {
                        app.debug_attachables = processes
                            .into_iter()
                            .map(|process| {
                                format!(
                                    "{} · {} · pid {}",
                                    process.name, process.adapter, process.pid
                                )
                            })
                            .collect();
                        app.status = format!(
                            "{} attachable process(es), {} debuggable run configuration(s)",
                            app.debug_attachables.len(),
                            app.run_configs
                                .iter()
                                .filter(|config| config.debug_adapter.is_some())
                                .count()
                        );
                    }
                    Err(error) => app.status = format!("Debug scan failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn build_project(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.load_build_model(cx);
    }

    fn load_build_model(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.status = "Loading build model…".into();
        cx.spawn(async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    ide_host::ops::build::build_model(&backend, crate::native_host::MAIN_WINDOW)
                        .await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(model) => {
                        let gradle = model.gradle.len();
                        let maven = model.maven.as_ref().map(|model| model.projects.len()).unwrap_or(0);
                        let summary = format!(
                            "Build model: {gradle} Gradle build(s), {maven} Maven project(s), {} sbt build(s)",
                            model.sbt.len()
                        );
                        app.status = summary.clone();
                        app.build_summary = Some(summary);
                        app.bottom_kind = BottomKind::Build;
                        app.bottom_visible = true;
                    }
                    Err(error) => app.status = format!("Build model failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn schedule_search(&mut self, cx: &mut Context<Self>) {
        self.search.generation = self.search.generation.wrapping_add(1);
        let generation = self.search.generation;
        if self.search.query.trim().is_empty() {
            if let Some(request) = self.search.request.take() {
                self.host.host().cancel_request(request);
            }
            self.search.results.clear();
            self.search.summary = None;
            self.search.running = false;
            cx.notify();
            return;
        }
        if let Some(request) = self.search.request.take() {
            self.host.host().cancel_request(request);
        }
        self.search.running = false;
        self.search.results.clear();
        self.search.summary = None;
        cx.spawn(async move |this, cx| {
            Timer::after(Duration::from_millis(180)).await;
            this.update(cx, |app, cx| {
                if app.search.generation == generation && !app.search.query.trim().is_empty() {
                    app.run_search(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        if self.search.query.trim().is_empty() {
            return;
        }
        if let Some(request) = self.search.request.take() {
            self.host.host().cancel_request(request);
        }
        let query = SearchQuery {
            pattern: self.search.query.clone(),
            is_regex: self.search.regex,
            case_sensitive: self.search.case_sensitive,
            whole_word: self.search.whole_word,
            include: Vec::new(),
            exclude: Vec::new(),
            scope: SearchScope::Project,
            max_matches: 10_000,
            context: Default::default(),
        };
        let request = RequestId::next();
        self.search.request = Some(request);
        self.search.running = true;
        self.search.error = None;
        self.search.results.clear();
        self.search.summary = None;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SearchMessage>();
        let host = self.host.clone();
        let query_for_task = query.clone();
        let runtime = host.runtime().clone();
        runtime.spawn(async move {
            let tx_emit = tx.clone();
            let emit: ide_host::ops::Emit<ide_host::ops::search::SearchEvent> =
                Arc::new(move |event| {
                    let _ = tx_emit.send(SearchMessage::Event(event));
                });
            let result = ide_host::ops::search::start(
                host.host(),
                crate::native_host::MAIN_WINDOW,
                request,
                query_for_task,
                emit,
            )
            .await;
            let _ = tx.send(SearchMessage::Finished(result));
        });
        cx.spawn(async move |this, cx| {
            while let Some(message) = rx.recv().await {
                let mut finished = false;
                this.update(cx, |app, cx| match message {
                    SearchMessage::Event(ide_host::ops::search::SearchEvent::File { file }) => {
                        app.search.results.push(file);
                        cx.notify();
                    }
                    SearchMessage::Event(ide_host::ops::search::SearchEvent::Done { summary }) => {
                        app.search.summary = Some(summary);
                    }
                    SearchMessage::Finished(result) => {
                        app.search.running = false;
                        if let Err(error) = result {
                            app.search.error = Some(error.message);
                        }
                        cx.notify();
                        finished = true;
                    }
                })
                .ok();
                if finished {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn replace_all(&mut self, cx: &mut Context<Self>) {
        if self.search.query.is_empty() {
            return;
        }
        let query = SearchQuery {
            pattern: self.search.query.clone(),
            is_regex: self.search.regex,
            case_sensitive: self.search.case_sensitive,
            whole_word: self.search.whole_word,
            include: Vec::new(),
            exclude: Vec::new(),
            scope: SearchScope::Project,
            max_matches: 10_000,
            context: Default::default(),
        };
        let targets: Vec<ide_search::ReplaceTarget> = self
            .search
            .results
            .iter()
            .map(|file| ide_search::ReplaceTarget {
                path: file.path.clone(),
                only: None,
            })
            .collect();
        if targets.is_empty() {
            self.status = "Run a search before replacing".into();
            cx.notify();
            return;
        }
        let replacement = self.search.replace.clone();
        let host = self.host.clone();
        self.search.running = true;
        self.status = "Replacing in files…".into();
        cx.spawn(async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    ide_host::ops::search::replace_in_files(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        RequestId::next(),
                        query,
                        replacement,
                        false,
                        targets,
                        ide_host::ops::Channel::new(|_| {}),
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                app.search.running = false;
                match result {
                    Ok(()) => {
                        app.status = "Replace completed".into();
                        app.run_search(cx);
                    }
                    Err(error) => app.status = format!("Replace failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh_git(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        if !project.info.is_git {
            self.git.error = Some("Project is not a Git repository".into());
            cx.notify();
            return;
        }
        if self.git.loading {
            return;
        }
        self.git.loading = true;
        self.git.error = None;
        let host = self.host.clone();
        let root = project.root.clone();
        cx.spawn(async move |this, cx| {
            let (status, branch, branches, log) = host
                .run(move |backend| async move {
                    let status = vcs::status(&backend, crate::native_host::MAIN_WINDOW).await;
                    let branch = vcs::vcs_branch_info(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        Some(root.clone()),
                    )
                    .await;
                    let branches = vcs::vcs_branches(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        Some(root.clone()),
                    )
                    .await;
                    let log = vcs::vcs_log(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        Some(root),
                        LogQuery {
                            limit: 50,
                            ..Default::default()
                        },
                    )
                    .await;
                    (status, branch, branches, log)
                })
                .await;
            this.update(cx, |app, cx| {
                app.git.loading = false;
                match status {
                    Ok(status) => app.git.status = status,
                    Err(error) => app.git.error = Some(error.message),
                }
                if let Ok(branch) = branch {
                    app.git.branch = Some(branch);
                }
                if let Ok(branches) = branches {
                    app.git.branches = branches;
                }
                if let Ok(log) = log {
                    app.git.log = log;
                }
                if !app.git.status.is_empty() {
                    app.status = format!("{} changed files", app.git.status.len());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn stage_all(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self
            .git
            .status
            .iter()
            .map(|status| status.path.0.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let host = self.host.clone();
        self.status = "Staging files…".into();
        cx.spawn(async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    vcs::vcs_stage(&backend, crate::native_host::MAIN_WINDOW, paths).await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(()) => {
                        app.status = "Staged all changes".into();
                        app.refresh_git(cx);
                    }
                    Err(error) => app.status = format!("Stage failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn unstage_all(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self
            .git
            .status
            .iter()
            .filter(|status| status.staged.is_some())
            .map(|status| status.path.0.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let host = self.host.clone();
        self.status = "Unstaging files…".into();
        cx.spawn(async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    vcs::vcs_unstage(&backend, crate::native_host::MAIN_WINDOW, paths).await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(()) => {
                        app.status = "Unstaged all changes".into();
                        app.refresh_git(cx);
                    }
                    Err(error) => app.status = format!("Unstage failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn discard_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self
            .git
            .status
            .iter()
            .map(|status| status.path.0.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let answer = window.prompt(
            PromptLevel::Critical,
            &format!("Discard changes in {} file(s)?", paths.len()),
            None,
            &["Discard", "Cancel"],
            cx,
        );
        let host = self.host.clone();
        cx.spawn_in(window, async move |this, cx| {
            let choice = answer.await.unwrap_or(1);
            this.update_in(cx, |_app, _window, cx| {
                if choice != 0 {
                    return;
                }
                cx.spawn(async move |this, cx| {
                    let result = host
                        .run(move |backend| async move {
                            vcs::vcs_discard(&backend, crate::native_host::MAIN_WINDOW, paths).await
                        })
                        .await;
                    this.update(cx, |app, cx| {
                        match result {
                            Ok(()) => {
                                app.status = "Discarded changes".into();
                                app.refresh_git(cx);
                            }
                            Err(error) => app.status = format!("Discard failed: {}", error.message),
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn commit_all(&mut self, cx: &mut Context<Self>) {
        let message = self.git.commit_message.trim().to_string();
        if message.is_empty() {
            self.status = "Enter a commit message".into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        let options = ide_vcs::CommitOptions {
            run_hooks: true,
            ..Default::default()
        };
        self.status = "Committing…".into();
        cx.spawn(async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    vcs::vcs_commit(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        None,
                        message,
                        options,
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(commit) => {
                        app.status = format!("Created commit {}", &commit[..commit.len().min(12)]);
                        app.git.commit_message.clear();
                        app.git_message_input
                            .update(cx, |input, cx| input.clear(cx));
                        app.git.diff_preview = None;
                        app.refresh_git(cx);
                    }
                    Err(error) => app.status = format!("Commit failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn open_git_diff(&mut self, path: PathBuf, staged: bool, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.status = format!("Loading diff for {}…", display_name(&path));
        cx.spawn(async move |this, cx| {
            let path_for_task = path.clone();
            let text = host
                .run(move |backend| async move {
                    vcs::vcs_diff_text(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        path_for_task,
                        staged,
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                match text {
                    Ok(text) => {
                        app.git.diff_preview = Some((path, text));
                        app.status = "Diff loaded".into();
                    }
                    Err(error) => app.status = format!("Diff failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Applies file-watcher events incrementally: the project tree re-reads
    /// only affected loaded directories, and open buffers perform an explicit
    /// external-change check before any later save can overwrite disk.
    fn handle_external_fs_changes(
        &mut self,
        paths: &[PathBuf],
        removed: &[PathBuf],
        cx: &mut Context<Self>,
    ) {
        let changed: HashSet<PathBuf> = paths
            .iter()
            .chain(removed)
            .map(|path| crate::document_store::lexical_path(path))
            .collect();
        let buffers: Vec<Entity<DocumentBuffer>> = self
            .documents
            .iter()
            .map(|(_, buffer)| buffer.clone())
            .collect();
        let mut conflict = false;
        for buffer in buffers {
            let Some(path) = buffer.read(cx).path().map(Path::to_path_buf) else {
                continue;
            };
            if !changed.contains(&crate::document_store::lexical_path(&path)) {
                continue;
            }
            buffer.update(cx, |buffer, cx| buffer.check_external_change_with_hash(cx));
            conflict |= buffer.read(cx).has_external_change();
        }
        if conflict {
            cx.notify();
        }

        let Some(project) = self.project.as_mut() else {
            return;
        };
        let reload = project.tree.note_external_changes(paths, removed);
        if !reload.is_empty() {
            self.load_project_directories(reload, cx);
        }
    }

    fn handle_event_overflow(
        &mut self,
        report: crate::native_host::OverflowReport,
        cx: &mut Context<Self>,
    ) {
        if report.document_patches {
            let buffers: Vec<Entity<DocumentBuffer>> = self
                .documents
                .iter()
                .map(|(_, buffer)| buffer.clone())
                .collect();
            for buffer in buffers {
                buffer.update(cx, |buffer, cx| buffer.force_resync(cx));
            }
        }
        if let Some(file_changes) = report.file_changes {
            self.handle_ide_event(file_changes, cx);
        }
        for event in report.coalesced {
            self.handle_ide_event(event, cx);
        }
        if report.general && self.project.is_some() {
            self.reload_project_tree(cx);
            self.refresh_git(cx);
        }
        if report.dropped > 0 {
            self.status = format!(
                "Backend event backlog overflowed; recovered {} coalesced event(s)",
                report.dropped
            );
        }
        cx.notify();
    }

    fn handle_ide_event(&mut self, event: IdeEvent, cx: &mut Context<Self>) {
        match event {
            IdeEvent::Diagnostics {
                path,
                source,
                items,
            } => {
                self.problem_sources.insert((path.0.clone(), source), items);
                self.rebuild_problems(cx);
            }
            IdeEvent::DocPatched {
                doc,
                base_version,
                version,
                changes,
                ..
            } => {
                if let Some(buffer) = self.find_buffer_by_doc(doc, cx) {
                    buffer.update(cx, |buffer, cx| {
                        buffer.apply_backend_patch(base_version, version, changes, cx)
                    });
                }
            }
            IdeEvent::DocDirty { doc, dirty } => {
                if let Some(buffer) = self.find_buffer_by_doc(doc, cx) {
                    buffer.update(cx, |buffer, cx| buffer.set_backend_dirty(dirty, cx));
                }
            }
            IdeEvent::DocLanguage { doc, language, .. } => {
                if let Some(buffer) = self.find_buffer_by_doc(doc, cx) {
                    buffer.update(cx, |buffer, _| buffer.set_language(language.to_string()));
                }
            }
            IdeEvent::ProjectTrustChanged { trusted, .. } => {
                if let Some(project) = self.project.as_mut() {
                    project.info.trusted = trusted;
                }
                self.status = if trusted {
                    "Project trusted".into()
                } else {
                    "Project is in Restricted Mode".into()
                };
                cx.notify();
            }
            IdeEvent::VcsChanged { .. } => {
                if self
                    .project
                    .as_ref()
                    .is_some_and(|project| project.info.is_git)
                    && !self.git.loading
                {
                    self.refresh_git(cx);
                }
            }
            IdeEvent::FsChanged {
                created,
                modified,
                removed,
                ignore_rules,
            } => {
                let mut paths: Vec<PathBuf> = created
                    .into_iter()
                    .chain(modified)
                    .map(|path| path.0)
                    .collect();
                let removed: Vec<PathBuf> = removed.into_iter().map(|path| path.0).collect();
                paths.extend(ignore_rules.into_iter().map(|path| path.0));
                self.handle_external_fs_changes(&paths, &removed, cx);
            }
            IdeEvent::ExcludedDirsChanged { dirs } => {
                let paths: Vec<PathBuf> = dirs.into_iter().map(|path| path.0).collect();
                self.handle_external_fs_changes(&paths, &[], cx);
            }
            IdeEvent::ProjectRootsChanged { .. } => {
                if self.project.is_some() {
                    self.reload_project_tree(cx);
                }
            }
            IdeEvent::LanguageServer {
                name,
                status,
                message,
                ..
            } => {
                self.server_statuses.insert(name, (status, message));
                cx.notify();
            }
            IdeEvent::LanguageServerRemoved { .. } => {}
            IdeEvent::Task {
                title,
                message,
                done,
                ..
            } => {
                if done {
                    self.status = format!("{title} finished");
                } else if let Some(message) = message {
                    self.status = format!("{title}: {message}");
                } else {
                    self.status = title;
                }
                cx.notify();
            }
            IdeEvent::Notify { level, title, body } => {
                self.status = match body {
                    Some(body) => format!("{title}: {body}"),
                    None => title,
                };
                if level == NotifyLevel::Error {
                    tracing::error!(status = %self.status, "backend notification");
                }
                cx.notify();
            }
            IdeEvent::SettingsChanged { .. } => {
                self.settings = self.host.settings();
                cx.notify();
            }
            IdeEvent::IndexState {
                indexing,
                files,
                symbols,
            } => {
                self.status = if indexing {
                    format!("Indexing: {files} files, {symbols} symbols")
                } else {
                    format!("Index ready: {files} files, {symbols} symbols")
                };
                cx.notify();
            }
            IdeEvent::TerminalExited { .. }
            | IdeEvent::RunExited { .. }
            | IdeEvent::Debug { .. } => {}
            _ => {}
        }
    }

    fn find_buffer_by_doc(&self, doc: DocId, cx: &App) -> Option<Entity<DocumentBuffer>> {
        self.documents.get_by_doc(doc).cloned().or_else(|| {
            self.groups
                .iter()
                .flat_map(|group| group.tabs.iter())
                .find(|tab| tab.buffer.read(cx).host_doc() == Some(doc))
                .map(|tab| tab.buffer.clone())
        })
    }

    fn sync_document_key(&mut self, buffer: &Entity<DocumentBuffer>, cx: &App) {
        let Some(doc) = buffer.read(cx).host_doc() else {
            return;
        };
        let Some(path) = buffer.read(cx).path().map(Path::to_path_buf) else {
            return;
        };
        let new_key = DocumentKey::for_ui_path(&path);
        let Some(old_key) = self.documents.key_for_doc(doc).cloned() else {
            return;
        };
        if old_key == new_key {
            return;
        }
        if let Err(()) = self.documents.rekey(&old_key, new_key, doc) {
            tracing::warn!(
                doc = doc.0,
                path = %path.display(),
                "Save As target is already registered"
            );
        }
    }

    fn rebuild_problems(&mut self, cx: &mut Context<Self>) {
        self.problems.clear();
        for ((path, source), diagnostics) in &self.problem_sources {
            for diagnostic in diagnostics {
                self.problems.push(ProblemEntry {
                    path: path.clone(),
                    diagnostic: diagnostic.clone(),
                    source: source.clone(),
                });
            }
        }
        self.problems.sort_by(|left, right| {
            severity_rank(left.diagnostic.severity)
                .cmp(&severity_rank(right.diagnostic.severity))
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| {
                    left.diagnostic
                        .range
                        .start
                        .line
                        .cmp(&right.diagnostic.range.start.line)
                })
        });
        self.sync_editor_diagnostics(cx);
        cx.notify();
    }

    fn sync_editor_diagnostics(&mut self, cx: &mut Context<Self>) {
        let theme = self.theme;
        for group in &self.groups {
            for tab in &group.tabs {
                let Some(path) = tab.buffer.read(cx).path().map(Path::to_path_buf) else {
                    continue;
                };
                let mut spans = Vec::new();
                for problem in &self.problems {
                    if problem.path != path {
                        continue;
                    }
                    let color = match problem.diagnostic.severity {
                        Severity::Error => theme.error,
                        Severity::Warning => theme.warning,
                        Severity::Info => theme.accent,
                        Severity::Hint => theme.faint,
                    };
                    let start = tab.buffer.read(cx).offset_for_line_col(
                        problem.diagnostic.range.start.line,
                        problem.diagnostic.range.start.col,
                    );
                    let end = tab.buffer.read(cx).offset_for_line_col(
                        problem.diagnostic.range.end.line,
                        problem.diagnostic.range.end.col,
                    );
                    spans.push((start, end.max(start + 1), color));
                }
                let editor = tab.editor.clone();
                editor.update(cx, |editor, cx| editor.set_diagnostics(spans, cx));
            }
        }
    }

    fn go_to_definition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        let Some(doc) = tab.buffer.read(cx).host_doc() else {
            self.status = "The active buffer has no language server document".into();
            cx.notify();
            return;
        };
        let offset = tab.editor.read(cx).cursor_offset(cx);
        let (line, col) = tab.buffer.read(cx).offset_to_line_col(offset);
        let host = self.host.clone();
        self.status = "Looking for definition…".into();
        cx.spawn(async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    ide_host::ops::lsp::lsp_goto(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        doc,
                        ide_core::Pos::new(line, col),
                        ide_lsp::types::GotoKind::Definition,
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(locations) => {
                        if let Some(location) = locations.into_iter().next() {
                            app.open_path(
                                location.path.0.clone(),
                                Some(location.range.start.line + 1),
                                None,
                                cx,
                            );
                            app.status = format!("Definition: {}", location.path.0.display());
                        } else {
                            app.status = "No definition found".into();
                        }
                    }
                    Err(error) => {
                        app.status = format!("Go to definition failed: {}", error.message)
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        window.focus(&self.focus_handle.clone());
        cx.notify();
    }

    fn format_active_document(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        let Some(doc) = tab.buffer.read(cx).host_doc() else {
            self.status = "The active buffer has no language server document".into();
            cx.notify();
            return;
        };
        let buffer = tab.buffer.clone();
        let host = self.host.clone();
        self.status = "Formatting…".into();
        cx.spawn(async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    ide_host::ops::lsp::lsp_format(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        doc,
                        None,
                        ide_lsp::types::FormatOptions {
                            tab_size: 4,
                            insert_spaces: true,
                            on_save: false,
                            tool: None,
                        },
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(edits) if edits.is_empty() => {
                        app.status = "Document is already formatted".into()
                    }
                    Ok(edits) => {
                        let mut applied = 0usize;
                        buffer.update(cx, |buffer, cx| {
                            let offsets: Vec<(u32, u32, String)> = edits
                                .iter()
                                .map(|edit| {
                                    (
                                        buffer.offset_for_line_col(
                                            edit.range.start.line,
                                            edit.range.start.col,
                                        ),
                                        buffer.offset_for_line_col(
                                            edit.range.end.line,
                                            edit.range.end.col,
                                        ),
                                        edit.new_text.clone(),
                                    )
                                })
                                .collect();
                            for (start, end, text) in offsets.into_iter().rev() {
                                if buffer.apply_local(
                                    start..end,
                                    &text,
                                    rustidea_editor::EditSource::Local,
                                    cx,
                                ) {
                                    applied += 1;
                                }
                            }
                        });
                        app.status = format!("Formatting applied {applied} edit(s)");
                    }
                    Err(error) => app.status = format!("Format failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Request completions at the active caret and show them in the shared
    /// palette overlay. The backend language-server request runs off the UI
    /// thread; only the ready list is installed in the workbench.
    fn request_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        let Some(doc) = tab.buffer.read(cx).host_doc() else {
            self.status = "The active buffer has no language server document".into();
            cx.notify();
            return;
        };
        let offset = tab.editor.read(cx).cursor_offset(cx);
        let (line, col) = tab.buffer.read(cx).offset_to_line_col(offset);
        let host = self.host.clone();
        self.status = "Loading completions…".into();
        cx.spawn_in(window, async move |this, cx| {
            let request = RequestId::next();
            let result = host
                .run(move |backend| async move {
                    ide_host::ops::lsp::lsp_completion(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        request,
                        doc,
                        Pos::new(line, col),
                        None,
                    )
                    .await
                })
                .await;
            this.update_in(cx, |app, window, cx| {
                match result {
                    Ok(list) if !list.items.is_empty() => {
                        app.completion_items = list.items;
                        let input = app.palette_input.clone();
                        app.overlay = Some(OverlayState {
                            kind: OverlayKind::Completion,
                            input: input.clone(),
                            items: Vec::new(),
                            selected: 0,
                            error: None,
                            message: None,
                        });
                        input.update(cx, |input, cx| input.clear(cx));
                        app.update_overlay_items("", cx);
                        let focus = input.read(cx).focus_handle(cx);
                        window.focus(&focus);
                        app.status = format!("{} completion(s)", app.completion_items.len());
                    }
                    Ok(_) => app.status = "No completions at the caret".into(),
                    Err(error) => app.status = format!("Completion failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Show the language server's documentation for the symbol at the caret.
    fn request_hover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        let Some(doc) = tab.buffer.read(cx).host_doc() else {
            self.status = "The active buffer has no language server document".into();
            cx.notify();
            return;
        };
        let offset = tab.editor.read(cx).cursor_offset(cx);
        let (line, col) = tab.buffer.read(cx).offset_to_line_col(offset);
        let host = self.host.clone();
        self.status = "Loading documentation…".into();
        cx.spawn_in(window, async move |this, cx| {
            let request = RequestId::next();
            let result = host
                .run(move |backend| async move {
                    ide_host::ops::lsp::lsp_hover(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        request,
                        doc,
                        Pos::new(line, col),
                    )
                    .await
                })
                .await;
            this.update_in(cx, |app, window, cx| {
                match result {
                    Ok(Some(hover)) => {
                        app.overlay = Some(OverlayState {
                            kind: OverlayKind::Hover,
                            input: app.palette_input.clone(),
                            items: Vec::new(),
                            selected: 0,
                            error: None,
                            message: Some(hover.contents.value),
                        });
                        window.focus(&app.focus_handle.clone());
                        app.status = "Documentation".into();
                    }
                    Ok(None) => app.status = "No documentation at the caret".into(),
                    Err(error) => app.status = format!("Hover failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Ask for a new symbol name. The rename itself runs after the prompt is
    /// submitted so the popup cannot block on a slow language server.
    fn open_rename_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        if tab.buffer.read(cx).host_doc().is_none() {
            self.status = "The active buffer has no language server document".into();
            cx.notify();
            return;
        }
        let offset = tab.editor.read(cx).cursor_offset(cx);
        let (line, col) = tab.buffer.read(cx).offset_to_line_col(offset);
        let current_range = tab.buffer.read(cx).word_range_at(offset);
        let current = tab
            .buffer
            .read(cx)
            .slice(current_range.start, current_range.end);
        let input = self.palette_input.clone();
        input.update(cx, |input, cx| input.clear(cx));
        self.overlay = Some(OverlayState {
            kind: OverlayKind::Rename,
            input: input.clone(),
            items: vec![PaletteItem {
                title: "Type the new name and press Enter".into(),
                detail: if current.is_empty() {
                    format!("line {}, column {}", line + 1, col + 1)
                } else {
                    format!("Rename {current}")
                },
                action: UiAction::None,
            }],
            selected: 0,
            error: None,
            message: None,
        });
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus);
        self.status = if current.is_empty() {
            "Rename symbol at the caret".into()
        } else {
            format!("Rename {current}")
        };
        cx.notify();
    }

    fn apply_rename(&mut self, new_name: String, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        let Some(doc) = tab.buffer.read(cx).host_doc() else {
            self.status = "The active buffer has no language server document".into();
            cx.notify();
            return;
        };
        let offset = tab.editor.read(cx).cursor_offset(cx);
        let (line, col) = tab.buffer.read(cx).offset_to_line_col(offset);
        let host = self.host.clone();
        self.status = format!("Renaming to {new_name}…");
        cx.spawn(async move |this, cx| {
            let request = RequestId::next();
            let renamed = host
                .run(move |backend| async move {
                    ide_host::ops::lsp::lsp_rename(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        request,
                        doc,
                        Pos::new(line, col),
                        new_name,
                    )
                    .await
                })
                .await;
            let result = match renamed {
                Ok(renamed) if renamed.edit.files.is_empty() => {
                    Err("The language server found nothing to rename".to_owned())
                }
                Ok(renamed) => {
                    let edit = renamed.edit;
                    host.run(move |backend| async move {
                        ide_host::ops::lsp::apply_workspace_edit(
                            &backend,
                            crate::native_host::MAIN_WINDOW,
                            edit,
                        )
                        .await
                    })
                    .await
                    .map(|_| ())
                    .map_err(|error| error.message)
                }
                Err(error) => Err(error.message),
            };
            this.update(cx, |app, cx| {
                match result {
                    Ok(()) => app.status = "Rename applied".into(),
                    Err(message) => app.status = format!("Rename failed: {message}"),
                }
                app.pending_editor_focus = true;
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn request_code_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        let Some(doc) = tab.buffer.read(cx).host_doc() else {
            self.status = "The active buffer has no language server document".into();
            cx.notify();
            return;
        };
        let offset = tab.editor.read(cx).cursor_offset(cx);
        let (line, col) = tab.buffer.read(cx).offset_to_line_col(offset);
        let position = Pos::new(line, col);
        let range = ide_core::Range::new(position, position);
        let host = self.host.clone();
        self.status = "Loading code actions…".into();
        cx.spawn_in(window, async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    ide_host::ops::lsp::lsp_code_actions(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        doc,
                        range,
                        Vec::new(),
                    )
                    .await
                })
                .await;
            this.update_in(cx, |app, window, cx| {
                match result {
                    Ok(actions) if !actions.is_empty() => {
                        app.code_action_items = actions;
                        let input = app.palette_input.clone();
                        input.update(cx, |input, cx| input.clear(cx));
                        app.overlay = Some(OverlayState {
                            kind: OverlayKind::CodeActions,
                            input: input.clone(),
                            items: Vec::new(),
                            selected: 0,
                            error: None,
                            message: None,
                        });
                        app.update_overlay_items("", cx);
                        let focus = input.read(cx).focus_handle(cx);
                        window.focus(&focus);
                        app.status = format!("{} code action(s)", app.code_action_items.len());
                    }
                    Ok(_) => app.status = "No code actions at the caret".into(),
                    Err(error) => app.status = format!("Code actions failed: {}", error.message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn apply_completion(&mut self, item: ide_lsp::types::CompletionItem, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else {
            return;
        };
        let buffer = tab.buffer.clone();
        let anchor = tab.editor.read(cx).cursor_offset(cx);
        let fallback = buffer.read(cx).word_range_at(anchor);
        let replace = match item.replace_range {
            Some(range) => {
                let start = buffer
                    .read(cx)
                    .offset_for_line_col(range.start.line, range.start.col);
                let end = buffer
                    .read(cx)
                    .offset_for_line_col(range.end.line, range.end.col);
                start..end.max(start)
            }
            None => fallback,
        };
        let text = if item.is_snippet {
            ide_lsp::snippet_text(&item.insert_text)
        } else {
            item.insert_text.clone()
        };
        let mut edits: Vec<(u32, u32, String)> = Vec::new();
        edits.push((replace.start, replace.end, text.clone()));
        for edit in &item.additional_edits {
            let start = buffer
                .read(cx)
                .offset_for_line_col(edit.range.start.line, edit.range.start.col);
            let end = buffer
                .read(cx)
                .offset_for_line_col(edit.range.end.line, edit.range.end.col);
            edits.push((start, end.max(start), edit.new_text.clone()));
        }
        edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
        let shift: i64 = edits
            .iter()
            .filter(|(start, end, _)| {
                *end <= replace.start && !(*start == replace.start && *end == replace.end)
            })
            .map(|(start, end, text)| {
                text.encode_utf16().count() as i64 - (*end as i64 - *start as i64)
            })
            .sum();
        buffer.update(cx, |buffer, cx| {
            for (start, end, text) in edits {
                buffer.apply_local(start..end, &text, rustidea_editor::EditSource::Local, cx);
            }
        });
        let caret =
            (replace.start as i64 + text.encode_utf16().count() as i64 + shift).max(0) as u32;
        tab.editor
            .update(cx, |editor, cx| editor.place_caret(caret, cx));
        self.overlay = None;
        self.pending_editor_focus = true;
        self.status = format!("Inserted {}", item.label);
        cx.notify();
    }

    fn apply_code_action(&mut self, action: ide_lsp::types::CodeAction, cx: &mut Context<Self>) {
        self.overlay = None;
        let Some(edit) = action.edit else {
            self.status = if action.command.is_some() {
                format!(
                    "“{}” needs a client command and is not available in the native build yet",
                    action.title
                )
            } else {
                format!("“{}” has no edit to apply", action.title)
            };
            self.pending_editor_focus = true;
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let title = action.title.clone();
        self.status = format!("Applying {}…", action.title);
        cx.spawn(async move |this, cx| {
            let result = host
                .run(move |backend| async move {
                    ide_host::ops::lsp::apply_workspace_edit(
                        &backend,
                        crate::native_host::MAIN_WINDOW,
                        edit,
                    )
                    .await
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(()) => app.status = format!("Applied {title}"),
                    Err(error) => app.status = format!("Code action failed: {}", error.message),
                }
                app.pending_editor_focus = true;
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn go_to_line_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_command_palette(window, OverlayKind::GoToLine, cx);
    }

    fn open_find_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_tab().is_none() {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        }
        let input = self.palette_input.clone();
        input.update(cx, |input, cx| input.clear(cx));
        self.overlay = Some(OverlayState {
            kind: OverlayKind::Find,
            input: input.clone(),
            items: vec![PaletteItem {
                title: "Type text and press Enter to find the next match".into(),
                detail: if self.find_query.is_empty() {
                    "⌘G next · Ctrl+Shift+G previous".into()
                } else {
                    format!("Last: {}", self.find_query)
                },
                action: UiAction::None,
            }],
            selected: 0,
            error: None,
            message: None,
        });
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus);
        self.status = "Find in file".into();
        cx.notify();
    }

    fn run_find(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.find_query.is_empty() {
            self.status = "Enter text to find first".into();
            cx.notify();
            return;
        }
        let Some(tab) = self.active_tab() else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        let query = self.find_query.clone();
        let found = tab
            .editor
            .update(cx, |editor, cx| editor.find(&query, forward, cx));
        self.status = if found {
            format!(
                "{}: {query}",
                if forward {
                    "Found next"
                } else {
                    "Found previous"
                }
            )
        } else {
            format!("No match for {query}")
        };
        self.pending_editor_focus = true;
        cx.notify();
    }

    fn open_problem(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(problem) = self.problems.get(index).cloned() else {
            return;
        };
        self.open_path(
            problem.path,
            Some(problem.diagnostic.range.start.line + 1),
            Some(window),
            cx,
        );
    }

    fn open_command_palette(
        &mut self,
        window: &mut Window,
        kind: OverlayKind,
        cx: &mut Context<Self>,
    ) {
        let input = self.palette_input.clone();
        input.update(cx, |input, cx| input.clear(cx));
        self.overlay = Some(OverlayState {
            kind,
            input: input.clone(),
            items: Vec::new(),
            selected: 0,
            error: None,
            message: None,
        });
        self.update_overlay_items("", cx);
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus);
        cx.notify();
    }

    fn move_overlay_selection(&mut self, delta: i8, cx: &mut Context<Self>) {
        let Some(overlay) = self.overlay.as_mut() else {
            return;
        };
        let len = overlay.items.len();
        if len == 0 {
            return;
        }
        overlay.selected = if delta < 0 {
            overlay.selected.saturating_sub(1)
        } else {
            (overlay.selected + 1).min(len - 1)
        };
        cx.notify();
    }

    fn update_overlay_items(&mut self, query: &str, cx: &mut Context<Self>) {
        let Some(overlay) = self.overlay.as_ref() else {
            return;
        };
        let kind = overlay.kind;
        let query = query.trim().to_lowercase();
        match kind {
            OverlayKind::CommandPalette => {
                let items = self
                    .commands()
                    .into_iter()
                    .filter(|item| {
                        query.is_empty()
                            || item.title.to_lowercase().contains(&query)
                            || item.detail.to_lowercase().contains(&query)
                    })
                    .collect();
                if let Some(overlay) = self.overlay.as_mut() {
                    overlay.items = items;
                    overlay.selected = 0;
                }
            }
            OverlayKind::GoToLine => {
                if let Some(overlay) = self.overlay.as_mut() {
                    overlay.items = vec![PaletteItem {
                        title: if query.is_empty() {
                            "Type a line number".into()
                        } else {
                            format!("Go to line {query}")
                        },
                        detail: "Enter".into(),
                        action: UiAction::None,
                    }];
                    overlay.selected = 0;
                }
            }
            OverlayKind::Completion => {
                let items = self
                    .completion_items
                    .iter()
                    .filter(|item| {
                        query.is_empty()
                            || item.label.to_lowercase().contains(&query)
                            || item
                                .filter_text
                                .as_deref()
                                .is_some_and(|text| text.to_lowercase().contains(&query))
                            || item
                                .detail
                                .as_deref()
                                .is_some_and(|text| text.to_lowercase().contains(&query))
                    })
                    .map(|item| {
                        let mut detail = item.label_detail.clone().unwrap_or_default();
                        if let Some(description) = &item.label_description {
                            if !detail.is_empty() {
                                detail.push(' ');
                            }
                            detail.push_str(description);
                        } else if let Some(description) = &item.detail {
                            if !detail.is_empty() {
                                detail.push(' ');
                            }
                            detail.push_str(description);
                        }
                        if detail.is_empty() {
                            detail = item
                                .kind
                                .map(|kind| format!("{kind:?}"))
                                .unwrap_or_default();
                        }
                        PaletteItem {
                            title: item.label.clone(),
                            detail,
                            action: UiAction::ApplyCompletion(Box::new(item.clone())),
                        }
                    })
                    .collect();
                if let Some(overlay) = self.overlay.as_mut() {
                    overlay.items = items;
                    overlay.selected = 0;
                }
            }
            OverlayKind::CodeActions => {
                let items = self
                    .code_action_items
                    .iter()
                    .filter(|action| {
                        query.is_empty() || action.title.to_lowercase().contains(&query)
                    })
                    .map(|action| PaletteItem {
                        title: action.title.clone(),
                        detail: action
                            .kind
                            .clone()
                            .or_else(|| action.disabled_reason.clone())
                            .unwrap_or_else(|| {
                                if action.edit.is_some() {
                                    "Edit".into()
                                } else {
                                    "Command".into()
                                }
                            }),
                        action: UiAction::ApplyCodeAction(Box::new(action.clone())),
                    })
                    .collect();
                if let Some(overlay) = self.overlay.as_mut() {
                    overlay.items = items;
                    overlay.selected = 0;
                }
            }
            OverlayKind::Rename | OverlayKind::Hover | OverlayKind::Find => {}
            OverlayKind::QuickOpen | OverlayKind::SearchEverywhere => {
                if query.is_empty() {
                    if let Some(overlay) = self.overlay.as_mut() {
                        overlay.items.clear();
                    }
                    cx.notify();
                    return;
                }
                let host = self.host.clone();
                let search_kind = kind;
                let query_for_task = query.to_string();
                cx.spawn(async move |this, cx| {
                    let file_query = query_for_task.clone();
                    let files = host
                        .run(move |backend| async move {
                            ide_host::ops::nav::find_files(
                                &backend,
                                crate::native_host::MAIN_WINDOW,
                                file_query,
                                120,
                            )
                            .await
                        })
                        .await;
                    let symbols = if search_kind == OverlayKind::SearchEverywhere {
                        host.run(move |backend| async move {
                            ide_host::ops::nav::find_symbols(
                                &backend,
                                crate::native_host::MAIN_WINDOW,
                                query_for_task,
                                SymbolFilter::All,
                                120,
                            )
                            .await
                        })
                        .await
                        .ok()
                    } else {
                        None
                    };
                    this.update(cx, |app, cx| {
                        let Some(overlay) = app.overlay.as_mut() else {
                            return;
                        };
                        if overlay.kind != search_kind {
                            return;
                        }
                        overlay.items.clear();
                        match files {
                            Ok(files) => {
                                for file in files {
                                    overlay.items.push(PaletteItem {
                                        title: file.rel_path.clone(),
                                        detail: file.path.0.display().to_string(),
                                        action: UiAction::OpenPath(file.path.0, None),
                                    });
                                }
                            }
                            Err(error) => overlay.error = Some(error.message),
                        }
                        if let Some(symbols) = symbols {
                            for symbol in symbols {
                                overlay.items.push(PaletteItem {
                                    title: symbol.name.clone(),
                                    detail: format!(
                                        "{} · {:?}:{}",
                                        symbol.rel_path,
                                        symbol.kind,
                                        symbol.range.start.line + 1
                                    ),
                                    action: UiAction::OpenPath(
                                        symbol.path.0.clone(),
                                        Some(symbol.range.start.line + 1),
                                    ),
                                });
                            }
                        }
                        overlay.selected = 0;
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
        cx.notify();
    }

    fn accept_overlay(&mut self, cx: &mut Context<Self>) {
        let Some(overlay) = self.overlay.as_ref() else {
            return;
        };
        if overlay.kind == OverlayKind::GoToLine {
            let line = overlay.input.read(cx).content().trim().parse::<u32>();
            self.overlay = None;
            match line {
                Ok(line) if line > 0 => {
                    if let Some(tab) = self.active_tab() {
                        tab.editor
                            .update(cx, |editor, cx| editor.go_to_line(line, cx));
                        self.status = format!("Line {line}");
                    }
                }
                _ => self.status = "Enter a positive line number".into(),
            }
            cx.notify();
            return;
        }
        if matches!(
            overlay.kind,
            OverlayKind::Completion
                | OverlayKind::CodeActions
                | OverlayKind::Rename
                | OverlayKind::Hover
                | OverlayKind::Find
        ) {
            let kind = overlay.kind;
            let action = overlay
                .items
                .get(overlay.selected)
                .map(|item| item.action.clone());
            let name = overlay.input.read(cx).content().trim().to_owned();
            self.overlay = None;
            match kind {
                OverlayKind::Completion | OverlayKind::CodeActions => {
                    if let Some(action) = action {
                        self.perform_ui_action(action, None, cx);
                    } else {
                        self.pending_editor_focus = true;
                    }
                }
                OverlayKind::Rename => {
                    if name.is_empty() {
                        self.status = "Enter a new symbol name".into();
                        self.pending_editor_focus = true;
                    } else {
                        self.apply_rename(name, cx);
                    }
                }
                OverlayKind::Hover => {
                    self.pending_editor_focus = true;
                }
                OverlayKind::Find => {
                    if name.is_empty() {
                        self.status = "Enter text to find".into();
                        self.pending_editor_focus = true;
                    } else {
                        self.find_query = name;
                        self.run_find(true, cx);
                    }
                }
                _ => {}
            }
            cx.notify();
            return;
        }
        let Some(item) = overlay.items.get(overlay.selected).cloned() else {
            return;
        };
        self.overlay = None;
        self.perform_ui_action(item.action, None, cx);
        cx.notify();
    }

    fn commands(&self) -> Vec<PaletteItem> {
        let mut items = Vec::new();
        let mut push = |title: &str, detail: &str, action: UiAction| {
            items.push(PaletteItem {
                title: title.to_owned(),
                detail: detail.to_owned(),
                action,
            });
        };
        push("New File", "File", UiAction::NewFile);
        push("Open File…", "⌘O", UiAction::OpenFile);
        push("Open Folder…", "⇧⌘O", UiAction::OpenFolder);
        push("Save", "⌘S", UiAction::Save);
        push("Save All", "⌥⌘S", UiAction::SaveAll);
        push("Save As…", "⇧⌘S", UiAction::SaveAs);
        push("Close Tab", "⌘W", UiAction::CloseTab);
        push("Next Tab", "⇧⌘]", UiAction::NextTab);
        push("Previous Tab", "⇧⌘[", UiAction::PreviousTab);
        push("Split Right", "Editor", UiAction::SplitRight);
        push("Split Down", "Editor", UiAction::SplitDown);
        push("Close Split", "Editor", UiAction::CloseGroup);
        push(
            "Toggle Light/Dark Theme",
            "Appearance",
            UiAction::ToggleTheme,
        );
        push("Toggle Autosave", "Editor", UiAction::ToggleAutosave);
        push(
            "Project",
            "Tool window",
            UiAction::ShowSidebar(SidebarKind::Project),
        );
        push(
            "Find in Files",
            "⇧⌘F",
            UiAction::ShowSidebar(SidebarKind::Search),
        );
        push(
            "Version Control",
            "Tool window",
            UiAction::ShowSidebar(SidebarKind::Git),
        );
        push(
            "Problems",
            "Tool window",
            UiAction::ShowSidebar(SidebarKind::Problems),
        );
        push(
            "Terminal",
            "Tool window",
            UiAction::ShowBottom(BottomKind::Terminal),
        );
        push("Run", "Tool window", UiAction::ShowBottom(BottomKind::Run));
        push(
            "Debug",
            "Tool window",
            UiAction::ShowBottom(BottomKind::Debug),
        );
        push("Settings", "⌘,", UiAction::OpenSettings);
        push(
            "Database",
            "Tool window",
            UiAction::ShowSidebar(SidebarKind::Database),
        );
        push(
            "HTTP Client",
            "Tool window",
            UiAction::ShowSidebar(SidebarKind::Http),
        );
        push(
            "Agents",
            "Tool window",
            UiAction::ShowSidebar(SidebarKind::Agents),
        );
        push(
            "Plugins",
            "Tool window",
            UiAction::ShowSidebar(SidebarKind::Plugins),
        );
        push(
            "Remote Host",
            "Tool window",
            UiAction::ShowSidebar(SidebarKind::Remote),
        );
        push("Refresh Project", "Project", UiAction::RefreshProject);
        push("Refresh Git", "Version Control", UiAction::RefreshGit);
        push("Go to Line…", "⌥⌘G", UiAction::GoToLinePrompt);
        push("Find in File…", "⌘F", UiAction::FindInFile);
        push("Find Next", "⌘G", UiAction::FindNext);
        push("Find Previous", "Ctrl+Shift+G", UiAction::FindPrevious);
        push("Code Completion", "Ctrl+Space", UiAction::Completion);
        push("Quick Documentation", "Ctrl+Shift+H", UiAction::Hover);
        push("Rename Symbol…", "F2", UiAction::RenameSymbol);
        push("Code Actions", "⌥Enter", UiAction::CodeActions);
        items
    }

    fn perform_ui_action(
        &mut self,
        action: UiAction,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if window.is_none()
            && matches!(
                &action,
                UiAction::NewFile
                    | UiAction::OpenFile
                    | UiAction::OpenFolder
                    | UiAction::Save
                    | UiAction::SaveAs
                    | UiAction::CloseTab
                    | UiAction::NextTab
                    | UiAction::PreviousTab
                    | UiAction::SplitRight
                    | UiAction::SplitDown
                    | UiAction::GoToLinePrompt
                    | UiAction::Completion
                    | UiAction::Hover
                    | UiAction::RenameSymbol
                    | UiAction::CodeActions
                    | UiAction::FindInFile
            )
        {
            self.pending_ui_action = Some(action);
            cx.notify();
            return;
        }
        match action {
            UiAction::NewFile => {
                if let Some(window) = window {
                    self.new_file(window, cx);
                }
            }
            UiAction::OpenFile => {
                if let Some(window) = window {
                    self.open_file_prompt(window, cx);
                }
            }
            UiAction::OpenFolder => {
                if let Some(window) = window {
                    self.open_folder_prompt(window, cx);
                }
            }
            UiAction::Save => {
                if let Some(window) = window {
                    self.save_active(window, cx);
                }
            }
            UiAction::SaveAll => self.save_all(cx),
            UiAction::SaveAs => {
                if let Some(window) = window {
                    self.save_active_as(window, cx);
                }
            }
            UiAction::CloseTab => {
                if let Some(window) = window {
                    self.close_active_tab(window, cx);
                }
            }
            UiAction::NextTab => {
                if let Some(window) = window {
                    self.next_tab(window, cx);
                }
            }
            UiAction::PreviousTab => {
                if let Some(window) = window {
                    self.previous_tab(window, cx);
                }
            }
            UiAction::SplitRight => {
                if let Some(window) = window {
                    self.split_active(SplitAxis::Horizontal, window, cx);
                }
            }
            UiAction::SplitDown => {
                if let Some(window) = window {
                    self.split_active(SplitAxis::Vertical, window, cx);
                }
            }
            UiAction::CloseGroup => self.close_group(cx),
            UiAction::ToggleAutosave => self.toggle_autosave(cx),
            UiAction::ShowSidebar(kind) => self.show_sidebar(kind, cx),
            UiAction::ShowBottom(kind) => self.show_bottom(kind, cx),
            UiAction::ToggleTheme => self.toggle_theme(cx),
            UiAction::RefreshProject => self.refresh_project(cx),
            UiAction::RefreshGit => self.refresh_git(cx),
            UiAction::OpenSettings => self.show_sidebar(SidebarKind::Settings, cx),
            UiAction::GoToLinePrompt => {
                if let Some(window) = window {
                    self.go_to_line_prompt(window, cx);
                }
            }
            UiAction::Completion => {
                if let Some(window) = window {
                    self.request_completion(window, cx);
                }
            }
            UiAction::Hover => {
                if let Some(window) = window {
                    self.request_hover(window, cx);
                }
            }
            UiAction::RenameSymbol => {
                if let Some(window) = window {
                    self.open_rename_prompt(window, cx);
                }
            }
            UiAction::CodeActions => {
                if let Some(window) = window {
                    self.request_code_actions(window, cx);
                }
            }
            UiAction::FindInFile => {
                if let Some(window) = window {
                    self.open_find_prompt(window, cx);
                }
            }
            UiAction::FindNext => self.run_find(true, cx),
            UiAction::FindPrevious => self.run_find(false, cx),
            UiAction::OpenPath(path, line) => self.open_path(path, line, window, cx),
            UiAction::ApplyCompletion(item) => self.apply_completion(*item, cx),
            UiAction::ApplyCodeAction(action) => self.apply_code_action(*action, cx),
            UiAction::None => {}
        }
    }
}

impl Focusable for RustIdea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

enum SearchMessage {
    Event(ide_host::ops::search::SearchEvent),
    Finished(ide_core::IdeResult<()>),
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
        Severity::Hint => 3,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::display_name;

    #[test]
    fn display_name_uses_the_last_path_component() {
        assert_eq!(
            display_name(Path::new("/tmp/project/src/main.rs")),
            "main.rs"
        );
        assert_eq!(display_name(Path::new("Cargo.toml")), "Cargo.toml");
        assert_eq!(display_name(Path::new("/")), "/");
    }
}
