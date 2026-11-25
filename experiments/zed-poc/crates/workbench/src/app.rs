use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use editor::Editor;
use gpui::{
    ClickEvent, Context, Entity, Focusable as _, SharedString, Subscription,
    UniformListScrollHandle, Window, div, prelude::*, px, uniform_list,
};
use ide_core::{DocId, IdeEvent, OutlineItem, Severity};
use language::{
    Buffer, BufferEvent, Diagnostic, DiagnosticEntry, DiagnosticMessage, DiagnosticSet,
};
use lsp::LanguageServerId;
use text::Point as TextPoint;

use crate::bridge::{ZedDocument, is_buffer_edited};
use crate::code_actions::OpenIdeaCodeActionProvider;
use crate::completion::OpenIdeaCompletionProvider;
use crate::documents::{DocumentKey, DocumentStore};
use crate::host::NativeHost;
use crate::lsp::DocumentLsp;
use crate::project::{ProjectTree, Toggle};
use crate::semantics::OpenIdeaSemanticsProvider;
use crate::terminal_pane::TerminalPane;
use crate::text_input::{TextInput, TextInputEvent};
use crate::theme::{Theme, ThemeMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarMode {
    Project,
    Search,
    Git,
    Outline,
    Problems,
    Database,
    Http,
    Remote,
    Notebook,
    Mcp,
    Agents,
    Plugins,
    Tools,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BottomMode {
    Run,
    Terminal,
    Build,
    Debug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Save,
    CloseTab,
    ToggleSplit,
    QuickOpen,
    GoToSymbol,
    ShowProject,
    ShowSearch,
    ShowGit,
    ShowOutline,
    ShowProblems,
    ShowDatabase,
    ShowHttp,
    ShowRemote,
    ShowNotebook,
    ShowMcp,
    ShowAgents,
    ShowPlugins,
    ShowTools,
    ShowSettings,
    ShowRun,
    ShowTerminal,
    ShowBuild,
    ShowDebug,
    RunSelected,
    BuildProject,
    RebuildProject,
    RefreshGit,
    RefreshAgents,
    RefreshTools,
    RefreshPlugins,
    RefreshMarket,
    RefreshRemote,
    RefreshDatabase,
}

impl Command {
    const ALL: [Command; 33] = [
        Command::Save,
        Command::CloseTab,
        Command::ToggleSplit,
        Command::QuickOpen,
        Command::GoToSymbol,
        Command::ShowProject,
        Command::ShowSearch,
        Command::ShowGit,
        Command::ShowOutline,
        Command::ShowProblems,
        Command::ShowDatabase,
        Command::ShowHttp,
        Command::ShowRemote,
        Command::ShowNotebook,
        Command::ShowMcp,
        Command::ShowAgents,
        Command::ShowPlugins,
        Command::ShowTools,
        Command::ShowSettings,
        Command::ShowRun,
        Command::ShowTerminal,
        Command::ShowBuild,
        Command::ShowDebug,
        Command::RunSelected,
        Command::BuildProject,
        Command::RebuildProject,
        Command::RefreshGit,
        Command::RefreshAgents,
        Command::RefreshTools,
        Command::RefreshPlugins,
        Command::RefreshMarket,
        Command::RefreshRemote,
        Command::RefreshDatabase,
    ];

    fn label(self) -> &'static str {
        match self {
            Command::Save => "Save file",
            Command::CloseTab => "Close tab",
            Command::ToggleSplit => "Toggle split editor",
            Command::QuickOpen => "Quick open file",
            Command::GoToSymbol => "Go to symbol",
            Command::ShowProject => "Show Project",
            Command::ShowSearch => "Show Find in Files",
            Command::ShowGit => "Show Version Control",
            Command::ShowOutline => "Show Outline",
            Command::ShowProblems => "Show Problems",
            Command::ShowDatabase => "Show Database",
            Command::ShowHttp => "Show HTTP Client",
            Command::ShowRemote => "Show Remote Host",
            Command::ShowNotebook => "Show Notebook",
            Command::ShowMcp => "Show MCP Servers",
            Command::ShowAgents => "Show Agents",
            Command::ShowPlugins => "Show Plugins",
            Command::ShowTools => "Show Tools",
            Command::ShowSettings => "Show Settings",
            Command::ShowRun => "Show Run",
            Command::ShowTerminal => "Show Terminal",
            Command::ShowBuild => "Show Build",
            Command::ShowDebug => "Show Debug",
            Command::RunSelected => "Run selected configuration",
            Command::BuildProject => "Build project",
            Command::RebuildProject => "Rebuild project",
            Command::RefreshGit => "Refresh Git status",
            Command::RefreshAgents => "Refresh agent providers",
            Command::RefreshTools => "Refresh tools",
            Command::RefreshPlugins => "Refresh installed plugins",
            Command::RefreshMarket => "Refresh plugin marketplace",
            Command::RefreshRemote => "Refresh SSH configurations",
            Command::RefreshDatabase => "Refresh data sources",
        }
    }

    fn matches(self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let label = self.label().to_lowercase();
        let mut terms = label.split_whitespace();
        query
            .split_whitespace()
            .all(|term| label.contains(term) || terms.any(|word| word.starts_with(term)))
    }
}

fn refresh_window(handle: Option<gpui::WindowHandle<Workbench>>, cx: &mut gpui::AsyncApp) {
    if let Some(handle) = handle
        && let Err(error) = handle.update(cx, |_, window, _| window.refresh())
    {
        eprintln!("WORKBENCH_REFRESH_ERROR {error:#}");
    }
}

fn change_letter(kind: Option<ide_vcs::ChangeKind>) -> &'static str {
    match kind {
        Some(ide_vcs::ChangeKind::Added) => "A",
        Some(ide_vcs::ChangeKind::Modified) => "M",
        Some(ide_vcs::ChangeKind::Deleted) => "D",
        Some(ide_vcs::ChangeKind::Renamed) => "R",
        Some(ide_vcs::ChangeKind::Untracked) => "?",
        Some(ide_vcs::ChangeKind::Ignored) => "I",
        Some(ide_vcs::ChangeKind::Conflicted) => "C",
        None => "·",
    }
}

fn notebook_output_text(value: &serde_json::Value) -> String {
    if let Some(text) = value.get("text").and_then(serde_json::Value::as_str) {
        return text.to_owned();
    }
    if let Some(text) = value
        .pointer("/data/text/plain")
        .and_then(serde_json::Value::as_str)
    {
        return text.to_owned();
    }
    if let Some(text) = value.get("ename").and_then(serde_json::Value::as_str) {
        let evalue = value
            .get("evalue")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        return if evalue.is_empty() {
            text.to_owned()
        } else {
            format!("{text}: {evalue}")
        };
    }
    serde_json::to_string(value).unwrap_or_default()
}

fn flatten_json(
    prefix: &str,
    value: &serde_json::Value,
    output: &mut Vec<(String, serde_json::Value)>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_json(&path, value, output);
            }
        }
        _ => output.push((prefix.to_owned(), value.clone())),
    }
}

fn display_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) if value.is_empty() => "(empty)".into(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Null => "null".into(),
        value => value.to_string(),
    }
}

fn utf16_prefix(text: &str, units: u32) -> String {
    let mut consumed = 0u32;
    let mut end = text.len();
    for (index, character) in text.char_indices() {
        let width = character.len_utf16() as u32;
        if consumed + width > units {
            end = index;
            break;
        }
        consumed += width;
    }
    text[..end].to_owned()
}

fn append_agent_entry_text(entry: &mut ide_agent::AgentEntry, from: u32, addition: &str) {
    match entry {
        ide_agent::AgentEntry::Agent { text }
        | ide_agent::AgentEntry::Thought { text }
        | ide_agent::AgentEntry::Message { text, .. } => {
            let prefix = utf16_prefix(text, from);
            *text = format!("{prefix}{addition}");
        }
        ide_agent::AgentEntry::Tool { output, .. } => {
            let prefix = utf16_prefix(output.as_deref().unwrap_or_default(), from);
            *output = Some(format!("{prefix}{addition}"));
        }
        _ => {}
    }
}

fn agent_entry_display(entry: &ide_agent::AgentEntry) -> (&'static str, String) {
    use ide_agent::AgentEntry;
    match entry {
        AgentEntry::User { text, .. } => ("You", text.clone()),
        AgentEntry::Agent { text } => ("Agent", text.clone()),
        AgentEntry::Thought { text } => ("Thought", text.clone()),
        AgentEntry::Tool {
            title,
            kind,
            status,
            output,
            ..
        } => (
            "Tool",
            format!(
                "[{kind}] {title} · {status:?}{}",
                output
                    .as_ref()
                    .map(|output| format!("\n{output}"))
                    .unwrap_or_default()
            ),
        ),
        AgentEntry::Plan { items } => (
            "Plan",
            items
                .iter()
                .map(|item| format!("[{}] {}", item.status, item.content))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        AgentEntry::Subagent {
            title,
            description,
            status,
            reply,
            prompt,
            ..
        } => (
            "Subagent",
            format!(
                "{} · {status:?}\n{}",
                title
                    .as_deref()
                    .or(description.as_deref())
                    .unwrap_or("subagent"),
                reply.as_deref().or(prompt.as_deref()).unwrap_or_default()
            ),
        ),
        AgentEntry::Message { incoming, text } => (
            if *incoming {
                "Message in"
            } else {
                "Message out"
            },
            text.clone(),
        ),
        AgentEntry::Permission {
            title,
            command,
            chosen,
            ..
        } => (
            "Permission",
            format!(
                "{}{}{}",
                title,
                command
                    .as_ref()
                    .map(|command| format!("\n{command}"))
                    .unwrap_or_default(),
                chosen
                    .as_ref()
                    .map(|chosen| format!("\n→ {chosen}"))
                    .unwrap_or_default()
            ),
        ),
        AgentEntry::Error { message } => ("Error", message.clone()),
    }
}

pub struct InitialState {
    pub project: ide_host::ops::project::ProjectInfo,
    pub root_listing: Vec<ide_vfs::FileEntry>,
    pub document: Option<ide_workspace::DocumentInfo>,
}

#[derive(Clone)]
struct ProblemItem {
    path: PathBuf,
    line: u32,
    column: u32,
    severity: Severity,
    message: String,
    source: Option<String>,
}

pub struct Workbench {
    host: Arc<NativeHost>,
    language_registry: Arc<language::LanguageRegistry>,
    root: PathBuf,
    tree: ProjectTree,
    tree_scroll: UniformListScrollHandle,
    documents: HashMap<DocId, ZedDocument>,
    store: DocumentStore<DocId>,
    editors: HashMap<(DocId, bool), Entity<Editor>>,
    split: bool,
    order: Vec<DocId>,
    active: Option<DocId>,
    subscriptions: HashMap<DocId, Subscription>,
    diagnostic_servers: HashMap<String, LanguageServerId>,
    next_diagnostic_server: usize,
    theme: Theme,
    status: gpui::SharedString,
    diagnostics: HashMap<PathBuf, usize>,
    window_handle: Option<gpui::WindowHandle<Workbench>>,
    run_configs: Vec<ide_run::RunConfig>,
    selected_run: usize,
    running_run: Option<ide_run::RunInfo>,
    run_output: String,
    run_status: gpui::SharedString,
    sidebar_mode: SidebarMode,
    search_input: Entity<TextInput>,
    search_pattern: String,
    search_results: Vec<ide_search::FileMatches>,
    search_status: gpui::SharedString,
    _search_subscription: Subscription,
    git_status: Vec<ide_vcs::FileStatus>,
    git_branch: Option<ide_vcs::BranchInfo>,
    git_message_input: Entity<TextInput>,
    git_status_message: gpui::SharedString,
    _git_subscription: Subscription,
    bottom_mode: BottomMode,
    terminal: Option<Entity<TerminalPane>>,
    terminal_starting: bool,
    terminal_status: gpui::SharedString,
    terminal_output: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
    outline: Vec<OutlineItem>,
    outline_status: gpui::SharedString,
    outline_doc: Option<DocId>,
    problems: Vec<ProblemItem>,
    settings_data: ide_core::settings::Settings,
    settings_filter_input: Entity<TextInput>,
    settings_filter: String,
    settings_status: gpui::SharedString,
    _settings_filter_subscription: Subscription,
    setting_edit_key: Option<String>,
    setting_edit_input: Entity<TextInput>,
    setting_edit_status: gpui::SharedString,
    _setting_edit_subscription: Subscription,
    pending_goto: Option<(PathBuf, u32, u32)>,
    http_blocks: Vec<ide_http::HttpRequestBlock>,
    http_selected: usize,
    http_status: gpui::SharedString,
    http_output: String,
    db_sources: Vec<ide_db::DataSource>,
    db_connected: Vec<String>,
    db_selected: usize,
    db_status: gpui::SharedString,
    db_output: String,
    ssh_configs: Vec<ide_core::remote_settings::SshConfiguration>,
    ssh_selected: usize,
    remote_status: gpui::SharedString,
    remote_path: Option<String>,
    remote_entries: Vec<ide_ssh::RemoteEntry>,
    remote_preview: String,
    remote_busy: bool,
    notebook: Option<ide_notebook::NotebookSnapshot>,
    notebook_status: gpui::SharedString,
    notebook_running: bool,
    notebook_edit_cell: Option<String>,
    notebook_editor: Option<Entity<Editor>>,
    notebook_edit_buffer: Option<Entity<Buffer>>,
    notebook_edit_status: gpui::SharedString,
    agents: Vec<ide_agent::AgentProvider>,
    agents_status: gpui::SharedString,
    agent_prompt_input: Entity<TextInput>,
    _agent_prompt_subscription: Subscription,
    agent_selected: usize,
    agent_tasks: Vec<ide_agent::AgentTask>,
    agent_selected_task: Option<String>,
    agent_entries: HashMap<String, BTreeMap<u32, ide_agent::AgentEntry>>,
    agent_subentries: HashMap<String, BTreeMap<u32, ide_agent::AgentEntry>>,
    agent_viewing_subagent: Option<String>,
    agent_answer_target: Option<(String, u32)>,
    agent_answer_input: Entity<TextInput>,
    agent_answer_status: gpui::SharedString,
    _agent_answer_subscription: Subscription,
    agent_task_status: gpui::SharedString,
    mcp_servers: Vec<ide_agent::McpServer>,
    mcp_status: gpui::SharedString,
    mcp_selected: Option<String>,
    mcp_name_input: Entity<TextInput>,
    mcp_command_input: Entity<TextInput>,
    mcp_args_input: Entity<TextInput>,
    tools: Vec<ide_tools::ToolInfo>,
    tools_selected: usize,
    tools_status: gpui::SharedString,
    build_model: Option<ide_build::BuildModel>,
    build_status: gpui::SharedString,
    build_output: String,
    build_running: bool,
    fast_open: bool,
    fast_open_input: Entity<TextInput>,
    fast_open_results: Vec<ide_index::FileMatch>,
    fast_open_selected: usize,
    fast_open_status: gpui::SharedString,
    _fast_open_subscription: Subscription,
    command_palette: bool,
    command_palette_input: Entity<TextInput>,
    command_palette_query: String,
    command_palette_selected: usize,
    pending_command: Option<Command>,
    _command_palette_subscription: Subscription,
    symbol_search: bool,
    symbol_input: Entity<TextInput>,
    symbol_query: String,
    symbol_results: Vec<ide_index::SymbolMatch>,
    symbol_selected: usize,
    symbol_status: gpui::SharedString,
    _symbol_subscription: Subscription,
    plugins: Vec<ide_host::plugins::PluginInfo>,
    plugins_status: gpui::SharedString,
    market_plugins: Vec<ide_host::plugins::MarketPlugin>,
    market_status: gpui::SharedString,
    market_loading: bool,
    market_selected: usize,
    plugin_preview: Option<ide_host::plugins::InstallPreview>,
    plugin_preview_status: gpui::SharedString,
    plugin_install_busy: bool,
    debug_attachables: Vec<ide_dap::attach::AttachableProcess>,
    debug_selected: usize,
    debug_status: gpui::SharedString,
}

impl Workbench {
    pub fn new(
        host: Arc<NativeHost>,
        language_registry: Arc<language::LanguageRegistry>,
        root: PathBuf,
        initial: InitialState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let InitialState {
            project,
            root_listing,
            document,
        } = initial;
        let window_handle = window.window_handle().downcast::<Workbench>();
        let mut tree = ProjectTree::new(root.clone());
        tree.apply_directory(root.clone(), root_listing);
        tree.rebuild();
        let registry_for_initial = language_registry.clone();
        let split = std::env::var_os("RUSTIDEA_ZED_SMOKE_SPLIT").is_some();
        let status = format!("{} · {}", project.name, root.display());
        let run_configs = host.run_configs().unwrap_or_default();
        let run_status = format!("{} run configurations", run_configs.len()).into();
        let theme = Theme::for_mode(ThemeMode::Dark);
        let search_input = cx.new(|cx| {
            TextInput::new(
                "Search project files…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let search_subscription = cx.subscribe(&search_input, |app, _, event, cx| match event {
            TextInputEvent::Submit(pattern) => {
                app.start_search(pattern.clone(), cx);
            }
            TextInputEvent::Change(pattern) => {
                app.search_pattern = pattern.clone();
            }
            TextInputEvent::Cancel => {
                app.sidebar_mode = SidebarMode::Project;
                cx.notify();
            }
            TextInputEvent::MoveSelection(_) => {}
        });
        let git_message_input = cx.new(|cx| {
            TextInput::new(
                "Commit message…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let git_subscription = cx.subscribe(&git_message_input, |app, _, event, cx| {
            if let TextInputEvent::Submit(message) = event {
                app.commit_git(message.clone(), cx);
            }
        });
        let agent_prompt_input = cx.new(|cx| {
            TextInput::new(
                "Describe a task for the selected agent…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let agent_prompt_subscription = cx.subscribe(&agent_prompt_input, |app, _, event, cx| {
            if let TextInputEvent::Submit(_) = event {
                app.start_agent_task(cx);
            }
        });
        let agent_answer_input = cx.new(|cx| {
            TextInput::new(
                "JSON answers (e.g. {\"question\": [\"answer\"]})",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let agent_answer_subscription =
            cx.subscribe(&agent_answer_input, |app, _, event, cx| match event {
                TextInputEvent::Submit(_) => app.submit_agent_form_answer(cx),
                TextInputEvent::Cancel => {
                    app.agent_answer_target = None;
                    cx.notify();
                }
                _ => {}
            });
        let mcp_name_input = cx.new(|cx| {
            TextInput::new(
                "MCP server name",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let mcp_command_input = cx.new(|cx| {
            TextInput::new(
                "Command (e.g. npx)",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let mcp_args_input = cx.new(|cx| {
            TextInput::new(
                "Arguments (space separated)",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let fast_open_input = cx.new(|cx| {
            TextInput::new(
                "Search project files…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let fast_open_subscription =
            cx.subscribe(&fast_open_input, |app, _, event, cx| match event {
                TextInputEvent::Change(query) => {
                    app.search_quick_open(query.clone(), cx);
                }
                TextInputEvent::Submit(_) => {
                    app.open_quick_open_result(cx);
                }
                TextInputEvent::Cancel => {
                    app.fast_open = false;
                    cx.notify();
                }
                TextInputEvent::MoveSelection(_) => {}
            });
        let command_palette_input = cx.new(|cx| {
            TextInput::new(
                "Type a command…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let command_palette_subscription =
            cx.subscribe(&command_palette_input, |app, _, event, cx| match event {
                TextInputEvent::Change(query) => {
                    app.command_palette_query = query.clone();
                    app.command_palette_selected = 0;
                    cx.notify();
                }
                TextInputEvent::MoveSelection(delta) => {
                    let commands = app.filtered_commands();
                    if commands.is_empty() {
                        return;
                    }
                    let last = commands.len() - 1;
                    app.command_palette_selected = if *delta < 0 {
                        app.command_palette_selected.saturating_sub(1)
                    } else {
                        (app.command_palette_selected + 1).min(last)
                    };
                    cx.notify();
                }
                TextInputEvent::Submit(_) => {
                    app.run_selected_command(cx);
                }
                TextInputEvent::Cancel => {
                    app.command_palette = false;
                    cx.notify();
                }
            });
        let symbol_input = cx.new(|cx| {
            TextInput::new(
                "Search project symbols…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let symbol_subscription = cx.subscribe(&symbol_input, |app, _, event, cx| match event {
            TextInputEvent::Change(query) => {
                app.symbol_query = query.clone();
                app.search_symbols(query.clone(), cx);
            }
            TextInputEvent::MoveSelection(delta) => {
                if app.symbol_results.is_empty() {
                    return;
                }
                let last = app.symbol_results.len() - 1;
                let current = app.symbol_selected.min(last);
                app.symbol_selected = if *delta < 0 {
                    current.saturating_sub(1)
                } else {
                    (current + 1).min(last)
                };
                cx.notify();
            }
            TextInputEvent::Submit(_) => {
                if let Some(result) = app.symbol_results.get(app.symbol_selected).cloned() {
                    app.schedule_goto(
                        result.path.0.clone(),
                        result.range.start.line,
                        result.range.start.col,
                        cx,
                    );
                    app.symbol_search = false;
                }
                cx.notify();
            }
            TextInputEvent::Cancel => {
                app.symbol_search = false;
                cx.notify();
            }
        });
        let settings = host.settings();
        let settings_filter_input = cx.new(|cx| {
            TextInput::new(
                "Filter settings…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let settings_filter_subscription =
            cx.subscribe(&settings_filter_input, |app, _, event, cx| {
                if let TextInputEvent::Change(filter) = event {
                    app.settings_filter = filter.clone();
                    cx.notify();
                }
            });
        let setting_edit_input = cx.new(|cx| {
            TextInput::new(
                "Setting value",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let setting_edit_subscription =
            cx.subscribe(&setting_edit_input, |app, _, event, cx| match event {
                TextInputEvent::Submit(_) => app.save_setting_editor(cx),
                TextInputEvent::Cancel => {
                    app.setting_edit_key = None;
                    cx.notify();
                }
                _ => {}
            });
        let mut app = Self {
            host,
            language_registry,
            root,
            tree,
            tree_scroll: UniformListScrollHandle::new(),
            documents: HashMap::default(),
            store: DocumentStore::default(),
            editors: HashMap::default(),
            split,
            order: Vec::new(),
            active: None,
            subscriptions: HashMap::default(),
            diagnostic_servers: HashMap::default(),
            next_diagnostic_server: 1,
            theme,
            status: status.into(),
            diagnostics: HashMap::default(),
            window_handle,
            run_configs,
            selected_run: 0,
            running_run: None,
            run_output: String::new(),
            run_status,
            sidebar_mode: SidebarMode::Project,
            search_input,
            search_pattern: String::new(),
            search_results: Vec::new(),
            search_status: "Type a pattern and press Enter".into(),
            _search_subscription: search_subscription,
            git_status: Vec::new(),
            git_branch: None,
            git_message_input,
            git_status_message: "Loading Git status…".into(),
            _git_subscription: git_subscription,
            bottom_mode: if std::env::var_os("RUSTIDEA_ZED_SMOKE_TERMINAL").is_some() {
                BottomMode::Terminal
            } else {
                BottomMode::Run
            },
            terminal: None,
            terminal_starting: false,
            terminal_status: "Click TERMINAL to start a shell".into(),
            terminal_output: None,
            outline: Vec::new(),
            outline_status: "Open a file to see its structure".into(),
            outline_doc: None,
            problems: Vec::new(),
            settings_data: settings,
            settings_filter_input,
            settings_filter: String::new(),
            settings_status: "Effective OpenIDEA settings".into(),
            _settings_filter_subscription: settings_filter_subscription,
            setting_edit_key: None,
            setting_edit_input,
            setting_edit_status: "Edit setting value".into(),
            _setting_edit_subscription: setting_edit_subscription,
            pending_goto: None,
            http_blocks: Vec::new(),
            http_selected: 0,
            http_status: "Open an .http file to list requests".into(),
            http_output: String::new(),
            db_sources: Vec::new(),
            db_connected: Vec::new(),
            db_selected: 0,
            db_status: "Select DATABASE to load data sources".into(),
            db_output: String::new(),
            ssh_configs: Vec::new(),
            ssh_selected: 0,
            remote_status: "Select REMOTE to load SSH configurations".into(),
            remote_path: None,
            remote_entries: Vec::new(),
            remote_preview: String::new(),
            remote_busy: false,
            notebook: None,
            notebook_status: "Open a .ipynb file to view cells".into(),
            notebook_running: false,
            notebook_edit_cell: None,
            notebook_editor: None,
            notebook_edit_buffer: None,
            notebook_edit_status: "Edit cell source".into(),
            agents: Vec::new(),
            agents_status: "Select AGENTS to load providers".into(),
            agent_prompt_input,
            _agent_prompt_subscription: agent_prompt_subscription,
            agent_selected: 0,
            agent_tasks: Vec::new(),
            agent_selected_task: None,
            agent_entries: HashMap::default(),
            agent_subentries: HashMap::default(),
            agent_viewing_subagent: None,
            agent_answer_target: None,
            agent_answer_input,
            agent_answer_status: "Answer as a JSON object".into(),
            _agent_answer_subscription: agent_answer_subscription,
            agent_task_status: "No agent tasks".into(),
            mcp_servers: Vec::new(),
            mcp_status: "No MCP servers".into(),
            mcp_selected: None,
            mcp_name_input,
            mcp_command_input,
            mcp_args_input,
            tools: Vec::new(),
            tools_selected: 0,
            tools_status: "Select TOOLS to load tool status".into(),
            build_model: None,
            build_status: "Builds have not been loaded".into(),
            build_output: String::new(),
            build_running: false,
            fast_open: false,
            fast_open_input,
            _fast_open_subscription: fast_open_subscription,
            fast_open_results: Vec::new(),
            fast_open_selected: 0,
            fast_open_status: "Type a file name".into(),
            command_palette: false,
            command_palette_input,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            pending_command: None,
            _command_palette_subscription: command_palette_subscription,
            symbol_search: false,
            symbol_input,
            symbol_query: String::new(),
            symbol_results: Vec::new(),
            symbol_selected: 0,
            symbol_status: "Type a symbol name".into(),
            _symbol_subscription: symbol_subscription,
            plugins: Vec::new(),
            plugins_status: "Select PLUGINS to load installed plugins".into(),
            market_plugins: Vec::new(),
            market_status: "Marketplace not loaded".into(),
            market_loading: false,
            market_selected: 0,
            plugin_preview: None,
            plugin_preview_status: "Select a marketplace plugin to preview".into(),
            plugin_install_busy: false,
            debug_attachables: Vec::new(),
            debug_selected: 0,
            debug_status: "Select DEBUG to scan attachable processes".into(),
        };
        if let Some(document) = document {
            app.insert_document(document, &registry_for_initial, cx);
        }
        let (agent_tx, mut agent_rx) = tokio::sync::mpsc::unbounded_channel();
        if let Ok(tasks) = app.host.agent_subscribe(agent_tx) {
            app.agent_tasks = tasks;
            app.agent_task_status = format!("{} agent tasks", app.agent_tasks.len()).into();
            cx.spawn(async move |this, cx| {
                while let Some(event) = agent_rx.recv().await {
                    let handle = this
                        .update(cx, |app, cx| {
                            app.apply_agent_event(event, cx);
                            cx.notify();
                            app.window_handle
                        })
                        .ok()
                        .flatten();
                    refresh_window(handle, cx);
                }
            })
            .detach();
        }
        app.refresh_git(cx);
        let ui_events = app.host.take_events();
        if let Some(mut events) = ui_events {
            cx.spawn(async move |this, cx| {
                while let Some(event) = events.receiver.recv().await {
                    let handle = this
                        .update(cx, |app, cx| {
                            app.handle_event(event, cx);
                            app.window_handle
                        })
                        .ok()
                        .flatten();
                    refresh_window(handle, cx);
                }
            })
            .detach();
        }
        app
    }

    pub fn set_window_handle(&mut self, window: gpui::WindowHandle<Workbench>) {
        self.window_handle = Some(window);
    }

    fn load_root(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let root = self.root.clone();
        cx.spawn(async move |this, cx| {
            let listing = host.read_dir(&root).await;
            let handle = this
                .update(cx, |app, cx| {
                    match listing {
                        Ok(listing) => {
                            app.tree.apply_directory(root, listing);
                            eprintln!("WORKBENCH_TREE rows={}", app.tree.nodes().len());
                        }
                        Err(error) => app.tree.apply_error(root, error.message),
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn load_directory(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let listing = host.read_dir(&path).await;
            let handle = this
                .update(cx, |app, cx| {
                    match listing {
                        Ok(listing) => app.tree.apply_directory(path, listing),
                        Err(error) => app.tree.apply_error(path, error.message),
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn activate(&mut self, doc: DocId, cx: &mut Context<Self>) {
        self.active = Some(doc);
        if self.sidebar_mode == SidebarMode::Outline {
            self.refresh_outline(cx);
        }
        if self.sidebar_mode == SidebarMode::Http {
            self.refresh_http(cx);
        }
        if self.sidebar_mode == SidebarMode::Notebook {
            self.refresh_notebook(cx);
        }
        if self.sidebar_mode == SidebarMode::Mcp {
            self.refresh_mcp(cx);
        }
        cx.notify();
    }

    fn select_sidebar(&mut self, mode: SidebarMode, cx: &mut Context<Self>) {
        self.sidebar_mode = mode;
        match mode {
            SidebarMode::Outline => self.refresh_outline(cx),
            SidebarMode::Settings => {
                self.settings_data = self.host.settings();
                self.settings_status = "Effective OpenIDEA settings".into();
            }
            SidebarMode::Git => self.refresh_git(cx),
            SidebarMode::Http => self.refresh_http(cx),
            SidebarMode::Database => self.refresh_db(cx),
            SidebarMode::Remote => self.refresh_remote(cx),
            SidebarMode::Notebook => self.refresh_notebook(cx),
            SidebarMode::Mcp => self.refresh_mcp(cx),
            SidebarMode::Agents => self.refresh_agents(cx),
            SidebarMode::Tools => self.refresh_tools(cx),
            SidebarMode::Plugins => self.refresh_plugins(cx),
            _ => {}
        }
        cx.notify();
    }

    fn set_setting(&mut self, key: String, value: serde_json::Value, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.settings_status = format!("Applying {key}…").into();
        cx.spawn(async move |this, cx| {
            let result = host.set_setting(key.clone(), value).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(settings) => {
                            app.settings_data = settings;
                            app.settings_status = format!("Updated {key}").into();
                        }
                        Err(error) => {
                            app.settings_status =
                                format!("Setting {key} failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn open_setting_editor(
        &mut self,
        key: String,
        value: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        let raw = match value {
            serde_json::Value::String(value) => value,
            value => value.to_string(),
        };
        self.setting_edit_input
            .update(cx, |input, cx| input.set_content(raw, cx));
        self.setting_edit_status = format!("Editing {key}").into();
        self.setting_edit_key = Some(key);
        cx.notify();
    }

    fn save_setting_editor(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.setting_edit_key.take() else {
            return;
        };
        let text = self.setting_edit_input.read(cx).content().trim().to_owned();
        let value =
            serde_json::from_str(&text).unwrap_or_else(|_| serde_json::Value::String(text.clone()));
        if let serde_json::Value::String(value) = &value
            && value.is_empty()
        {
            self.settings_status = format!("{key} cannot be empty here").into();
            cx.notify();
            return;
        }
        self.set_setting(key, value, cx);
    }

    fn refresh_outline(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            self.outline.clear();
            self.outline_doc = None;
            self.outline_status = "Open a file to see its structure".into();
            cx.notify();
            return;
        };
        if self.outline_doc == Some(doc) && !self.outline.is_empty() {
            return;
        }
        self.outline_doc = Some(doc);
        self.outline.clear();
        self.outline_status = "Loading document outline…".into();
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let outline = host.document_outline(doc).await;
            let handle = this
                .update(cx, |app, cx| {
                    match outline {
                        Ok(items) => {
                            let count = items.len();
                            app.outline = items;
                            app.outline_status = format!("{count} top-level items").into();
                        }
                        Err(error) => {
                            app.outline.clear();
                            app.outline_status =
                                format!("Outline failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn refresh_http(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            self.http_blocks.clear();
            self.http_status = "Open an .http file to list requests".into();
            cx.notify();
            return;
        };
        let Some(document) = self.documents.get(&doc) else {
            self.http_blocks.clear();
            self.http_status = "Open an .http file to list requests".into();
            cx.notify();
            return;
        };
        let text = document.text(cx);
        let parsed = ide_http::parse::parse(&text);
        self.http_blocks = parsed
            .requests
            .into_iter()
            .map(|request| request.block)
            .collect();
        self.http_selected = self
            .http_selected
            .min(self.http_blocks.len().saturating_sub(1));
        self.http_status = if self.http_blocks.is_empty() {
            "No requests parsed from this file".into()
        } else {
            format!("{} requests", self.http_blocks.len()).into()
        };
        cx.notify();
    }

    fn run_selected_http(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            self.http_status = "Open an .http file first".into();
            cx.notify();
            return;
        };
        if self.http_blocks.is_empty() {
            self.refresh_http(cx);
        }
        let Some(block) = self.http_blocks.get(self.http_selected) else {
            self.http_status = "No request selected".into();
            cx.notify();
            return;
        };
        let method = block.method.clone();
        let url = block.url.clone();
        let Some(document) = self.documents.get(&doc) else {
            self.http_status = "Open an .http file first".into();
            cx.notify();
            return;
        };
        let text = document.text(cx);
        let path = document.path.clone();
        let index = self.http_selected as u32;
        let root = self.root.clone();
        let host = self.host.clone();
        self.http_status = format!("Running {method} {url}…").into();
        self.http_output.clear();
        cx.spawn(async move |this, cx| {
            let result = host.run_http(text, index, Some(path), root).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(exchange) => {
                            if let Some(response) = &exchange.response {
                                app.http_status = format!(
                                    "{} {} · {} bytes · {:.1} ms",
                                    response.status,
                                    response.status_text,
                                    response.size,
                                    response.total_ms
                                )
                                .into();
                                let mut output = format!(
                                    "{} {} {}\n",
                                    response.version, response.status, response.status_text
                                );
                                for (name, value) in &response.headers {
                                    output.push_str(&format!("{name}: {value}\n"));
                                }
                                output.push('\n');
                                if let Some(body) = &response.body {
                                    output.push_str(body);
                                    if response.truncated {
                                        output.push_str("\n… body truncated");
                                    }
                                } else {
                                    output.push_str("(binary body)");
                                }
                                app.http_output = output;
                                if app.http_output.len() > 2_000_000 {
                                    let split = app.http_output.len().saturating_sub(1_800_000);
                                    app.http_output.drain(..split);
                                }
                            } else {
                                app.http_status = exchange
                                    .entry
                                    .error
                                    .clone()
                                    .unwrap_or_else(|| "Request failed".into())
                                    .into();
                                app.http_output.clear();
                            }
                        }
                        Err(error) => {
                            app.http_status = format!("HTTP failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn refresh_db(&mut self, cx: &mut Context<Self>) {
        let sources = self.host.db_sources();
        let connected = self.host.db_connected();
        match sources {
            Ok(sources) => {
                self.db_sources = sources;
                self.db_selected = self
                    .db_selected
                    .min(self.db_sources.len().saturating_sub(1));
                self.db_status = if self.db_sources.is_empty() {
                    "No data sources configured".into()
                } else {
                    format!("{} data sources", self.db_sources.len()).into()
                };
            }
            Err(error) => {
                self.db_sources.clear();
                self.db_status = format!("Data sources failed: {}", error.message).into();
            }
        }
        self.db_connected = connected.unwrap_or_default();
        cx.notify();
    }

    fn connect_selected_db(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.db_sources.get(self.db_selected).cloned() else {
            self.db_status = "No data source selected".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let id = source.id.clone();
        self.db_status = format!("Connecting to {}…", source.name).into();
        cx.spawn(async move |this, cx| {
            let result = host.db_connect(id).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.db_status = match result {
                        Ok(()) => "Connected".into(),
                        Err(error) => format!("Connect failed: {}", error.message).into(),
                    };
                    app.db_connected = app.host.db_connected().unwrap_or_default();
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn run_selected_db_sql(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.db_sources.get(self.db_selected).cloned() else {
            self.db_status = "No data source selected".into();
            cx.notify();
            return;
        };
        let Some(doc) = self.active else {
            self.db_status = "Open a .sql file to run a statement".into();
            cx.notify();
            return;
        };
        let Some(document) = self.documents.get(&doc) else {
            self.db_status = "Open a .sql file to run a statement".into();
            cx.notify();
            return;
        };
        let text = document.text(cx);
        let id = source.id.clone();
        let host = self.host.clone();
        self.db_status = format!("Running on {}…", source.name).into();
        self.db_output.clear();
        cx.spawn(async move |this, cx| {
            let result = host.db_execute(id, text, 0, 0).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(outcomes) => {
                            let summary = outcomes
                                .iter()
                                .map(|outcome| {
                                    if let Some(error) = &outcome.error {
                                        format!("error: {error}")
                                    } else if let Some(result) = &outcome.result {
                                        format!(
                                            "{} row(s), {} ms",
                                            result.rows.len(),
                                            result.elapsed_ms
                                        )
                                    } else {
                                        "no result".to_string()
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" | ");
                            app.db_status = summary.into();
                            app.db_output = serde_json::to_string_pretty(&outcomes)
                                .unwrap_or_else(|error| format!("format result: {error}"));
                            if app.db_output.len() > 2_000_000 {
                                let split = app.db_output.len().saturating_sub(1_800_000);
                                app.db_output.drain(..split);
                            }
                        }
                        Err(error) => {
                            app.db_status = format!("Query failed: {}", error.message).into();
                            app.db_output.clear();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn refresh_remote(&mut self, cx: &mut Context<Self>) {
        self.ssh_configs = self.host.ssh_configs();
        self.ssh_selected = self
            .ssh_selected
            .min(self.ssh_configs.len().saturating_sub(1));
        self.remote_status = if self.ssh_configs.is_empty() {
            "No SSH configurations".into()
        } else {
            format!("{} SSH configurations", self.ssh_configs.len()).into()
        };
        self.remote_path = None;
        self.remote_entries.clear();
        self.remote_preview.clear();
        cx.notify();
    }

    fn browse_remote(&mut self, dir: Option<String>, cx: &mut Context<Self>) {
        let Some(config) = self.ssh_configs.get(self.ssh_selected).cloned() else {
            self.remote_status = "No SSH configuration selected".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let id = config.id.clone();
        self.remote_busy = true;
        self.remote_status = format!("Listing {}…", dir.as_deref().unwrap_or("home")).into();
        cx.spawn(async move |this, cx| {
            let result = host.remote_list(id, dir).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.remote_busy = false;
                    match result {
                        Ok((path, entries)) => {
                            app.remote_status =
                                format!("{} entries · {path}", entries.len()).into();
                            app.remote_path = Some(path);
                            app.remote_entries = entries;
                        }
                        Err(error) => {
                            app.remote_status =
                                format!("Remote list failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn preview_remote_file(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(config) = self.ssh_configs.get(self.ssh_selected).cloned() else {
            return;
        };
        let host = self.host.clone();
        let id = config.id.clone();
        self.remote_busy = true;
        self.remote_status = format!("Reading {path}…").into();
        cx.spawn(async move |this, cx| {
            let result = host.remote_read(id, path).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.remote_busy = false;
                    match result {
                        Ok(text) => {
                            app.remote_status = format!("{} bytes", text.len()).into();
                            app.remote_preview = text;
                        }
                        Err(error) => {
                            app.remote_status =
                                format!("Remote read failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn refresh_notebook(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            self.notebook = None;
            self.notebook_status = "Open a .ipynb file to view cells".into();
            cx.notify();
            return;
        };
        let Some(document) = self.documents.get(&doc) else {
            self.notebook = None;
            self.notebook_status = "Open a .ipynb file to view cells".into();
            cx.notify();
            return;
        };
        if document.path.extension().and_then(|ext| ext.to_str()) != Some("ipynb") {
            self.notebook = None;
            self.notebook_status = "The active file is not a Jupyter notebook".into();
            cx.notify();
            return;
        }
        let path = document.path.display().to_string();
        if self
            .notebook
            .as_ref()
            .is_some_and(|notebook| notebook.path.0 == document.path)
        {
            return;
        }
        let host = self.host.clone();
        self.notebook_status = "Opening notebook…".into();
        cx.spawn(async move |this, cx| {
            let result = host.notebook_open(path).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(notebook) => {
                            app.notebook_status = format!(
                                "{} cells · version {}",
                                notebook.cells.len(),
                                notebook.version
                            )
                            .into();
                            app.notebook = Some(notebook);
                        }
                        Err(error) => {
                            app.notebook = None;
                            app.notebook_status =
                                format!("Notebook failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn run_notebook(&mut self, cx: &mut Context<Self>) {
        let Some(notebook) = self.notebook.clone() else {
            self.notebook_status = "Open a notebook first".into();
            cx.notify();
            return;
        };
        if self.notebook_running {
            return;
        }
        let cells: Vec<String> = notebook
            .cells
            .iter()
            .filter(|cell| cell.cell_type == "code")
            .map(|cell| cell.id.clone())
            .collect();
        if cells.is_empty() {
            self.notebook_status = "No code cells".into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        let id = notebook.id.clone();
        self.notebook_running = true;
        self.notebook_status = "Executing notebook…".into();
        cx.spawn(async move |this, cx| {
            let result = host.notebook_execute(id, cells).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.notebook_running = false;
                    match result {
                        Ok(notebook) => {
                            app.notebook_status =
                                format!("Executed · version {}", notebook.version).into();
                            app.notebook = Some(notebook);
                        }
                        Err(error) => {
                            app.notebook_status =
                                format!("Notebook execution failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn save_notebook(&mut self, cx: &mut Context<Self>) {
        let Some(notebook) = self.notebook.clone() else {
            self.notebook_status = "Open a notebook first".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let id = notebook.id.clone();
        self.notebook_status = "Saving notebook…".into();
        cx.spawn(async move |this, cx| {
            let result = host.notebook_save(id).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(notebook) => {
                            app.notebook_status =
                                format!("Saved · version {}", notebook.version).into();
                            app.notebook = Some(notebook);
                        }
                        Err(error) => {
                            app.notebook_status =
                                format!("Notebook save failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn open_notebook_cell_editor(
        &mut self,
        cell: String,
        source: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let buffer = cx.new(|cx| Buffer::local(source, cx));
        let editor = cx.new(|cx| Editor::for_buffer(buffer.clone(), None, window, cx));
        let focus = editor.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.notebook_edit_cell = Some(cell);
        self.notebook_edit_buffer = Some(buffer);
        self.notebook_editor = Some(editor);
        self.notebook_edit_status = "Edit cell source; Commit writes through OpenIDEA".into();
        cx.notify();
    }

    fn commit_notebook_cell_editor(&mut self, cx: &mut Context<Self>) {
        let Some(cell) = self.notebook_edit_cell.take() else {
            return;
        };
        let Some(buffer) = self.notebook_edit_buffer.take() else {
            return;
        };
        self.notebook_editor = None;
        let source = buffer.read(cx).snapshot().text();
        let Some(notebook) = self.notebook.clone() else {
            self.notebook_status = "Notebook is not open".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let id = notebook.id.clone();
        let version = notebook.version;
        self.notebook_status = "Updating cell…".into();
        cx.spawn(async move |this, cx| {
            let result = host
                .notebook_edit(
                    id.clone(),
                    version,
                    ide_notebook::NotebookEdit::SetSource { cell, source },
                )
                .await;
            let result = match result {
                Ok(_) => host.notebook_save(id).await,
                Err(error) => Err(error),
            };
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(notebook) => {
                            app.notebook_status =
                                format!("Cell updated · version {}", notebook.version).into();
                            app.notebook = Some(notebook);
                        }
                        Err(error) => {
                            app.notebook_status =
                                format!("Cell update failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn open_ssh_terminal(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(config) = self.ssh_configs.get(self.ssh_selected).cloned() else {
            self.remote_status = "No SSH configuration selected".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let theme = self.theme;
        let window_handle = self.window_handle;
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        self.remote_status = format!("Connecting to {}…", config.label()).into();
        self.terminal_status = self.remote_status.clone();
        self.bottom_mode = BottomMode::Terminal;
        cx.spawn(async move |this, cx| {
            let result = host
                .spawn_ssh_terminal(
                    config.id.clone(),
                    crate::terminal_pane::COLUMNS as u16,
                    crate::terminal_pane::ROWS as u16,
                    output_tx,
                )
                .await;
            this.update_in(cx, |app, window, cx| {
                match result {
                    Ok(info) => {
                        if let Some(old) = app.terminal.take() {
                            let _ = app.host.kill_terminal(old.read(cx).id());
                        }
                        let terminal = cx.new(|cx| {
                            TerminalPane::new(
                                host.clone(),
                                info.id,
                                info.title.clone().into(),
                                info.pid,
                                theme,
                                window,
                                cx,
                            )
                        });
                        app.terminal = Some(terminal);
                        app.terminal_status = format!("SSH · {}", config.name).into();
                        app.remote_status = format!("SSH session open · {}", config.label()).into();
                        app.bottom_mode = BottomMode::Terminal;
                        app.attach_terminal_output(output_rx, cx);
                    }
                    Err(error) => {
                        app.remote_status = format!("SSH failed: {}", error.message).into();
                        app.terminal_status = app.remote_status.clone();
                    }
                }
                cx.notify();
            })
            .ok();
            refresh_window(window_handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn refresh_agents(&mut self, cx: &mut Context<Self>) {
        match self.host.agent_providers() {
            Ok(agents) => {
                self.agents_status = if agents.is_empty() {
                    "No agent providers".into()
                } else {
                    format!("{} agent providers", agents.len()).into()
                };
                self.agents = agents;
            }
            Err(error) => {
                self.agents.clear();
                self.agents_status = format!("Agents failed: {}", error.message).into();
            }
        }
        self.refresh_mcp(cx);
        cx.notify();
    }

    fn refresh_mcp(&mut self, cx: &mut Context<Self>) {
        match self.host.mcp_list() {
            Ok(servers) => {
                self.mcp_status = format!("{} MCP servers", servers.len()).into();
                self.mcp_servers = servers;
                if self
                    .mcp_selected
                    .as_ref()
                    .is_some_and(|id| !self.mcp_servers.iter().any(|server| &server.id == id))
                {
                    self.mcp_selected = None;
                }
            }
            Err(error) => {
                self.mcp_servers.clear();
                self.mcp_status = format!("MCP failed: {}", error.message).into();
            }
        }
        cx.notify();
    }

    fn select_mcp_server(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(server) = self.mcp_servers.get(index).cloned() else {
            return;
        };
        self.mcp_selected = Some(server.id.clone());
        let name = server.name.clone();
        self.mcp_name_input
            .update(cx, |input, cx| input.set_content(name.clone(), cx));
        self.mcp_command_input.update(cx, |input, cx| {
            input.set_content(server.command.clone().unwrap_or_default(), cx)
        });
        self.mcp_args_input
            .update(cx, |input, cx| input.set_content(server.args.join(" "), cx));
        self.mcp_status = format!("Selected {name}").into();
        cx.notify();
    }

    fn new_mcp_server(&mut self, cx: &mut Context<Self>) {
        self.mcp_selected = None;
        self.mcp_name_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.mcp_command_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.mcp_args_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.mcp_status = "New stdio MCP server".into();
        cx.notify();
    }

    fn save_mcp_server(&mut self, cx: &mut Context<Self>) {
        let name = self.mcp_name_input.read(cx).content().trim().to_owned();
        let command = self.mcp_command_input.read(cx).content().trim().to_owned();
        let args = self.mcp_args_input.read(cx).content().trim().to_owned();
        if name.is_empty() || command.is_empty() {
            self.mcp_status = "MCP name and command are required".into();
            cx.notify();
            return;
        }
        let input = ide_agent::McpServerInput {
            id: self.mcp_selected.clone(),
            name,
            scope: ide_agent::McpScope::User,
            transport: ide_agent::McpTransport::Stdio,
            enabled: true,
            command: Some(command),
            args: args
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
            env: Vec::new(),
            url: None,
            headers: Vec::new(),
            agents: Vec::new(),
        };
        match self.host.mcp_save(input) {
            Ok(servers) => {
                self.mcp_servers = servers;
                self.mcp_status = "MCP server saved".into();
                self.mcp_selected = None;
            }
            Err(error) => {
                self.mcp_status = format!("MCP save failed: {}", error.message).into();
            }
        }
        cx.notify();
    }

    fn remove_selected_mcp_server(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.mcp_selected.clone() else {
            self.mcp_status = "Select an MCP server first".into();
            cx.notify();
            return;
        };
        match self.host.mcp_remove(id) {
            Ok(servers) => {
                self.mcp_servers = servers;
                self.mcp_selected = None;
                self.mcp_status = "MCP server removed".into();
                self.new_mcp_server(cx);
            }
            Err(error) => {
                self.mcp_status = format!("MCP remove failed: {}", error.message).into();
            }
        }
        cx.notify();
    }

    fn toggle_mcp_server(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(server) = self.mcp_servers.get(index).cloned() else {
            return;
        };
        let enabled = !server.enabled;
        match self.host.mcp_set_enabled(server.id, enabled) {
            Ok(servers) => {
                self.mcp_servers = servers;
                self.mcp_status = format!(
                    "{} {}",
                    server.name,
                    if enabled { "enabled" } else { "disabled" }
                )
                .into();
            }
            Err(error) => {
                self.mcp_status = format!("MCP update failed: {}", error.message).into();
            }
        }
        cx.notify();
    }

    fn apply_agent_event(&mut self, event: ide_agent::AgentEvent, _cx: &mut Context<Self>) {
        match event {
            ide_agent::AgentEvent::Task { task } => {
                let id = task.id.clone();
                if let Some(existing) = self
                    .agent_tasks
                    .iter_mut()
                    .find(|existing| existing.id == id)
                {
                    *existing = *task;
                } else {
                    self.agent_tasks.push(*task);
                }
                if self.agent_selected_task.is_none() {
                    self.agent_selected_task = Some(id);
                }
            }
            ide_agent::AgentEvent::Removed { id } => {
                self.agent_tasks.retain(|task| task.id != id);
                self.agent_entries.remove(&id);
                self.agent_subentries.clear();
                self.agent_viewing_subagent = None;
                if self.agent_selected_task.as_deref() == Some(id.as_str()) {
                    self.agent_selected_task = self.agent_tasks.first().map(|task| task.id.clone());
                }
            }
            ide_agent::AgentEvent::Truncated { task, length } => {
                if let Some(entries) = self.agent_entries.get_mut(&task) {
                    entries.retain(|index, _| *index < length);
                }
            }
            ide_agent::AgentEvent::Entry {
                task,
                subagent,
                index,
                entry,
            } => match subagent {
                Some(subagent) => {
                    self.agent_subentries
                        .entry(subagent)
                        .or_default()
                        .insert(index, entry);
                }
                None => {
                    self.agent_entries
                        .entry(task)
                        .or_default()
                        .insert(index, entry);
                }
            },
            ide_agent::AgentEvent::Append {
                task,
                subagent,
                index,
                from,
                text,
            } => {
                let entries = match subagent {
                    Some(subagent) => self.agent_subentries.entry(subagent).or_default(),
                    None => self.agent_entries.entry(task).or_default(),
                };
                if let Some(entry) = entries.get_mut(&index) {
                    append_agent_entry_text(entry, from, &text);
                }
            }
            _ => {}
        }
        self.agent_task_status = format!("{} agent tasks", self.agent_tasks.len()).into();
    }

    fn start_agent_task(&mut self, cx: &mut Context<Self>) {
        if self.agents.is_empty() {
            self.refresh_agents(cx);
        }
        let Some(agent) = self.agents.get(self.agent_selected).cloned() else {
            self.agent_task_status = "No agent provider selected".into();
            cx.notify();
            return;
        };
        let prompt = self.agent_prompt_input.read(cx).content().trim().to_owned();
        if prompt.is_empty() {
            self.agent_task_status = "Enter a task prompt".into();
            cx.notify();
            return;
        }
        if !agent.installed {
            self.agent_task_status = format!("{} is not installed", agent.name).into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        let provider = agent.id.clone();
        let name = agent.name.clone();
        self.agent_task_status = format!("Starting a task with {name}…").into();
        cx.spawn(async move |this, cx| {
            let result = host.agent_create(provider, prompt).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(task) => {
                            app.agent_task_status = format!("Created task {}", task.title).into();
                            app.agent_selected_task = Some(task.id.clone());
                            if !app
                                .agent_tasks
                                .iter()
                                .any(|existing| existing.id == task.id)
                            {
                                app.agent_tasks.push(task);
                            }
                            app.agent_prompt_input.update(cx, |input, cx| {
                                input.set_content("", cx);
                            });
                        }
                        Err(error) => {
                            app.agent_task_status =
                                format!("Agent task failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn send_agent_followup(&mut self, cx: &mut Context<Self>) {
        let Some(task) = self.agent_selected_task.clone() else {
            self.agent_task_status = "Select a task first".into();
            cx.notify();
            return;
        };
        let prompt = self.agent_prompt_input.read(cx).content().trim().to_owned();
        if prompt.is_empty() {
            self.agent_task_status = "Enter a message".into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        self.agent_task_status = "Sending message…".into();
        cx.spawn(async move |this, cx| {
            let result = host.agent_prompt(task, prompt).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(()) => {
                            app.agent_task_status = "Message queued".into();
                            app.agent_prompt_input.update(cx, |input, cx| {
                                input.set_content("", cx);
                            });
                        }
                        Err(error) => {
                            app.agent_task_status =
                                format!("Prompt failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn cancel_selected_agent_task(&mut self, cx: &mut Context<Self>) {
        let Some(task) = self.agent_selected_task.clone() else {
            return;
        };
        match self.host.agent_cancel(task) {
            Ok(()) => self.agent_task_status = "Cancelled".into(),
            Err(error) => {
                self.agent_task_status = format!("Cancel failed: {}", error.message).into();
            }
        }
        cx.notify();
    }

    fn respond_agent_permission(
        &mut self,
        task: String,
        permission: u32,
        option: Option<String>,
        cx: &mut Context<Self>,
    ) {
        match self
            .host
            .agent_respond_permission(task, permission, option.clone())
        {
            Ok(()) => {
                self.agent_task_status = if option.is_some() {
                    "Permission answered".into()
                } else {
                    "Permission cancelled".into()
                };
            }
            Err(error) => {
                self.agent_task_status =
                    format!("Permission answer failed: {}", error.message).into();
            }
        }
        cx.notify();
    }

    fn open_agent_form_answer(&mut self, task: String, permission: u32, cx: &mut Context<Self>) {
        self.agent_answer_target = Some((task, permission));
        self.agent_answer_input
            .update(cx, |input, cx| input.set_content("{}", cx));
        self.agent_answer_status = "Type the JSON answer object the agent expects".into();
        cx.notify();
    }

    fn submit_agent_form_answer(&mut self, cx: &mut Context<Self>) {
        let Some((task, permission)) = self.agent_answer_target.take() else {
            return;
        };
        let answer = self.agent_answer_input.read(cx).content().trim().to_owned();
        if answer.is_empty() {
            self.agent_answer_status = "Answer cannot be empty".into();
            cx.notify();
            return;
        }
        self.respond_agent_permission(task, permission, Some(answer), cx);
    }

    fn refresh_tools(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.tools_status = "Loading tools…".into();
        cx.spawn(async move |this, cx| {
            let result = host.tools_list().await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(tools) => {
                            app.tools_status = format!("{} tools", tools.len()).into();
                            app.tools = tools;
                            app.tools_selected =
                                app.tools_selected.min(app.tools.len().saturating_sub(1));
                        }
                        Err(error) => {
                            app.tools.clear();
                            app.tools_status = format!("Tools failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn install_selected_tool(&mut self, cx: &mut Context<Self>) {
        let Some(tool) = self.tools.get(self.tools_selected).cloned() else {
            self.tools_status = "No tool selected".into();
            cx.notify();
            return;
        };
        if !tool.can_install {
            self.tools_status = tool
                .unavailable_reason
                .clone()
                .unwrap_or_else(|| "This tool cannot be installed here".into())
                .into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        let name = tool.name.clone();
        self.tools_status = format!("Installing {}…", tool.display_name).into();
        cx.spawn(async move |this, cx| {
            let result = host.tools_install(name).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(tool) => {
                            app.tools_status = format!("{} installed", tool.display_name).into();
                        }
                        Err(error) => {
                            app.tools_status = format!("Install failed: {}", error.message).into();
                        }
                    }
                    app.refresh_tools(cx);
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn refresh_plugins(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let runtime = host.runtime().clone();
        self.plugins_status = "Loading plugins…".into();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .spawn_blocking(move || host.plugins_snapshot())
                .await
                .map_err(|error| ide_core::IdeError::internal(error.to_string()));
            let result = match result {
                Ok(result) => result,
                Err(error) => Err(error),
            };
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(snapshot) => {
                            app.plugins_status =
                                format!("{} installed plugins", snapshot.plugins.len()).into();
                            app.plugins = snapshot.plugins;
                        }
                        Err(error) => {
                            app.plugins.clear();
                            app.plugins_status =
                                format!("Plugins failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn refresh_market(&mut self, cx: &mut Context<Self>) {
        if self.market_loading {
            return;
        }
        let host = self.host.clone();
        self.market_loading = true;
        self.market_status = "Loading marketplace…".into();
        cx.spawn(async move |this, cx| {
            let result = host.plugin_market().await;
            let handle = this
                .update(cx, |app, cx| {
                    app.market_loading = false;
                    match result {
                        Ok(plugins) => {
                            app.market_status =
                                format!("{} marketplace plugins", plugins.len()).into();
                            app.market_plugins = plugins;
                            if std::env::var_os("RUSTIDEA_ZED_SMOKE_MARKET_PREVIEW").is_some()
                                && !app.market_plugins.is_empty()
                            {
                                app.preview_market_plugin(cx);
                            }
                        }
                        Err(error) => {
                            app.market_status =
                                format!("Marketplace failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn preview_market_plugin(&mut self, cx: &mut Context<Self>) {
        let Some(plugin) = self.market_plugins.get(self.market_selected) else {
            self.plugin_preview_status = "No marketplace plugin selected".into();
            cx.notify();
            return;
        };
        let id = plugin.entry.id.clone();
        let host = self.host.clone();
        self.plugin_preview = None;
        self.plugin_preview_status = format!("Downloading {}…", plugin.entry.name).into();
        cx.spawn(async move |this, cx| {
            let result = host.plugin_preview(id).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(preview) => {
                            app.plugin_preview_status = format!(
                                "{} {} · {} capabilities · {}",
                                preview.name,
                                preview.version,
                                preview.capabilities.len(),
                                if preview.unsigned {
                                    "UNSIGNED"
                                } else {
                                    "signed"
                                }
                            )
                            .into();
                            app.plugin_preview = Some(preview);
                        }
                        Err(error) => {
                            app.plugin_preview_status =
                                format!("Preview failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn install_previewed_plugin(&mut self, cx: &mut Context<Self>) {
        let Some(preview) = self.plugin_preview.clone() else {
            self.plugin_preview_status = "Preview the plugin first".into();
            cx.notify();
            return;
        };
        if self.plugin_install_busy {
            return;
        }
        let host = self.host.clone();
        let id = preview.id.clone();
        self.plugin_install_busy = true;
        self.plugin_preview_status = format!(
            "Installing {}… (optional capabilities are not granted)",
            preview.name
        )
        .into();
        cx.spawn(async move |this, cx| {
            let result = host.plugin_install(id).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.plugin_install_busy = false;
                    match result {
                        Ok(plugin) => {
                            app.plugin_preview_status =
                                format!("Installed {} {}", plugin.name, plugin.version).into();
                            app.refresh_plugins(cx);
                        }
                        Err(error) => {
                            app.plugin_preview_status =
                                format!("Install failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn refresh_build(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.build_status = "Loading build model…".into();
        cx.spawn(async move |this, cx| {
            let result = host.build_model().await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(model) => {
                            let summary = match (&model.maven, model.gradle.len()) {
                                (Some(_), 0) => "Maven build model".to_string(),
                                (None, 0) => "No Maven or Gradle build".to_string(),
                                (None, gradle) => format!("{gradle} Gradle builds"),
                                (Some(_), gradle) => format!("Maven + {gradle} Gradle builds"),
                            };
                            app.build_status = summary.into();
                            app.build_model = Some(model);
                        }
                        Err(error) => {
                            app.build_model = None;
                            app.build_status =
                                format!("Build model failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn run_build(&mut self, kind: ide_build::compile::BuildKind, cx: &mut Context<Self>) {
        if self.build_running {
            return;
        }
        self.build_running = true;
        self.build_status = match kind {
            ide_build::compile::BuildKind::Build => "Building…",
            ide_build::compile::BuildKind::Rebuild => "Rebuilding…",
            ide_build::compile::BuildKind::Artifacts => "Building artifacts…",
        }
        .into();
        self.build_output.clear();
        let host = self.host.clone();
        let (output_tx, mut output_rx) =
            tokio::sync::mpsc::unbounded_channel::<ide_build::compile::BuildEvent>();
        cx.spawn(async move |this, cx| {
            let feed_this = this.clone();
            let feed = cx.spawn(async move |cx| {
                while let Some(event) = output_rx.recv().await {
                    let handle = feed_this
                        .update(cx, |app, cx| {
                            app.build_running = true;
                            match event {
                                ide_build::compile::BuildEvent::Started { title, .. } => {
                                    app.build_output.push_str(&format!("== {title} ==\n"));
                                }
                                ide_build::compile::BuildEvent::Step { command, .. } => {
                                    app.build_output.push_str(&format!("$ {command}\n"));
                                }
                                ide_build::compile::BuildEvent::Output { text, .. } => {
                                    app.build_output.push_str(&text);
                                }
                                ide_build::compile::BuildEvent::Message { message } => {
                                    app.build_output.push_str(&format!(
                                        "{:?}: {}\n",
                                        message.severity, message.text
                                    ));
                                }
                                ide_build::compile::BuildEvent::Progress { message, .. } => {
                                    app.build_status = message.into();
                                }
                            }
                            if app.build_output.len() > 2_000_000 {
                                let split = app.build_output.len().saturating_sub(1_800_000);
                                app.build_output.drain(..split);
                            }
                            cx.notify();
                            app.window_handle
                        })
                        .ok()
                        .flatten();
                    refresh_window(handle, cx);
                }
            });
            let result = host.build_run(kind, output_tx).await;
            feed.await;
            let handle = this
                .update(cx, |app, cx| {
                    app.build_running = false;
                    match result {
                        Ok(result) => {
                            app.build_status = format!(
                                "{} · {} errors · {} warnings · {:.1} s",
                                result.title,
                                result.errors,
                                result.warnings,
                                result.duration_ms / 1000.0
                            )
                            .into();
                        }
                        Err(error) => {
                            app.build_status = format!("Build failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn refresh_debug(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.debug_status = "Scanning attachable processes…".into();
        cx.spawn(async move |this, cx| {
            let result = host.debug_attachables().await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(processes) => {
                            app.debug_status = format!(
                                "{} attachable processes · {} debuggable run configs",
                                processes.len(),
                                app.run_configs
                                    .iter()
                                    .filter(|config| config.debug_adapter.is_some())
                                    .count()
                            )
                            .into();
                            app.debug_attachables = processes;
                            app.debug_selected = app
                                .debug_selected
                                .min(app.debug_attachables.len().saturating_sub(1));
                        }
                        Err(error) => {
                            app.debug_attachables.clear();
                            app.debug_status =
                                format!("Debug scan failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn goto_position(
        &mut self,
        doc: DocId,
        line: u32,
        column: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate(doc, cx);
        let Some(editor) = self.ensure_editor(doc, false, window, cx) else {
            return;
        };
        let multi_buffer = editor.read(cx).buffer().clone();
        let snapshot = multi_buffer.read(cx).snapshot(cx);
        let anchor = snapshot.anchor_before(TextPoint::new(line, column));
        editor.update(cx, |editor, cx| {
            editor.change_selections(Default::default(), window, cx, |selection| {
                selection.select_ranges([anchor..anchor]);
            });
        });
        let focus = editor.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    fn open_or_goto(
        &mut self,
        path: PathBuf,
        line: u32,
        column: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((doc, _)) = self
            .documents
            .iter()
            .find(|(_, document)| document.path == path)
        {
            let doc = *doc;
            self.goto_position(doc, line, column, window, cx);
            return;
        }
        self.pending_goto = Some((path.clone(), line, column));
        self.open_path(path, cx);
    }

    fn ensure_editor(
        &mut self,
        doc: DocId,
        secondary: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<Editor>> {
        if let Some(editor) = self.editors.get(&(doc, secondary)) {
            return Some(editor.clone());
        }
        let document = self.documents.get(&doc)?;
        let buffer = document.buffer.clone();
        let editor = cx.new(|cx| Editor::for_buffer(buffer, None, window, cx));
        let lsp = Arc::new(DocumentLsp::new(
            self.host.clone(),
            doc,
            document.path.clone(),
        ));
        editor.update(cx, |editor, _| {
            editor.set_completion_provider(Some(Rc::new(OpenIdeaCompletionProvider::new(
                lsp.clone(),
            ))));
            editor
                .set_semantics_provider(Some(Rc::new(OpenIdeaSemanticsProvider::new(lsp.clone()))));
            editor.set_code_action_provider(Some(Rc::new(OpenIdeaCodeActionProvider::new(lsp))));
        });
        self.editors.insert((doc, secondary), editor.clone());
        Some(editor)
    }

    fn close_tab(&mut self, doc: DocId, cx: &mut Context<Self>) {
        let Some(position) = self.order.iter().position(|candidate| *candidate == doc) else {
            return;
        };
        let path = self
            .documents
            .get(&doc)
            .map(|document| document.path.clone());
        let was_active = self.active == Some(doc);
        self.order.remove(position);
        self.subscriptions.remove(&doc);
        self.documents.remove(&doc);
        self.editors.remove(&(doc, false));
        self.editors.remove(&(doc, true));
        if let Some(path) = path {
            self.store.release(&DocumentKey::Path(path.clone()));
            let _ = self.host.close_document(doc);
            if self.split && self.order.is_empty() {
                self.split = false;
            }
        }
        if was_active {
            self.active = self
                .order
                .get(position.min(self.order.len().saturating_sub(1)))
                .copied();
        }
        cx.notify();
    }

    fn toggle_split(&mut self, cx: &mut Context<Self>) {
        self.split = !self.split;
        cx.notify();
    }

    fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(doc) = self.store.get(&DocumentKey::for_ui_path(&path)).copied() {
            self.activate(doc, cx);
            return;
        }
        if self
            .documents
            .values()
            .any(|document| document.path == path)
        {
            if let Some((doc, _)) = self.documents.iter().find(|(_, d)| d.path == path) {
                self.activate(*doc, cx);
            }
            return;
        }
        self.status = format!("Opening {}…", path.display()).into();
        let host = self.host.clone();
        let registry = self.language_registry.clone();
        cx.spawn(async move |this, cx| {
            let info = host.open_document(&path).await;
            let handle = this
                .update(cx, |app, cx| {
                    match info {
                        Ok(info) => app.insert_document(info, &registry, cx),
                        Err(error) => {
                            app.status = format!("Open failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn toggle_quick_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fast_open = !self.fast_open;
        if self.fast_open {
            self.fast_open_selected = 0;
            let focus = self.fast_open_input.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn search_quick_open(&mut self, query: String, cx: &mut Context<Self>) {
        let query = query.trim().to_owned();
        if query.is_empty() {
            self.fast_open_results.clear();
            self.fast_open_status = "Type a file name".into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.find_files(query.clone(), 40).await;
            let handle = this
                .update(cx, |app, cx| {
                    if !app.fast_open || app.fast_open_input.read(cx).content() != query {
                        return app.window_handle;
                    }
                    match result {
                        Ok(results) => {
                            app.fast_open_status = format!("{} files", results.len()).into();
                            app.fast_open_results = results;
                            app.fast_open_selected = 0;
                        }
                        Err(error) => {
                            app.fast_open_status =
                                format!("Search failed: {}", error.message).into();
                            app.fast_open_results.clear();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn open_quick_open_result(&mut self, cx: &mut Context<Self>) {
        let Some(result) = self.fast_open_results.get(self.fast_open_selected).cloned() else {
            self.fast_open_status = "No file selected".into();
            cx.notify();
            return;
        };
        self.fast_open = false;
        self.open_path(result.path.0, cx);
        cx.notify();
    }

    fn toggle_symbol_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.symbol_search = !self.symbol_search;
        if self.symbol_search {
            self.command_palette = false;
            self.fast_open = false;
            self.symbol_selected = 0;
            let focus = self.symbol_input.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn search_symbols(&mut self, query: String, cx: &mut Context<Self>) {
        let query = query.trim().to_owned();
        if query.is_empty() {
            self.symbol_results.clear();
            self.symbol_status = "Type a symbol name".into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host
                .find_symbols(query.clone(), ide_index::SymbolFilter::All, 60)
                .await;
            let handle = this
                .update(cx, |app, cx| {
                    if !app.symbol_search || app.symbol_input.read(cx).content() != query {
                        return app.window_handle;
                    }
                    match result {
                        Ok(results) => {
                            app.symbol_status = format!("{} symbols", results.len()).into();
                            app.symbol_results = results;
                            app.symbol_selected = 0;
                        }
                        Err(error) => {
                            app.symbol_status =
                                format!("Symbol search failed: {}", error.message).into();
                            app.symbol_results.clear();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn schedule_goto(&mut self, path: PathBuf, line: u32, column: u32, cx: &mut Context<Self>) {
        let open = self
            .documents
            .values()
            .any(|document| document.path == path);
        self.pending_goto = Some((path.clone(), line, column));
        if !open {
            self.open_path(path, cx);
        }
        cx.notify();
    }

    fn open_symbol_result(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(result) = self.symbol_results.get(index).cloned() else {
            return;
        };
        self.symbol_search = false;
        self.open_or_goto(
            result.path.0,
            result.range.start.line,
            result.range.start.col,
            window,
            cx,
        );
    }

    fn filtered_commands(&self) -> Vec<Command> {
        let query = self.command_palette_query.to_lowercase();
        Command::ALL
            .iter()
            .copied()
            .filter(|command| command.matches(&query))
            .collect()
    }

    fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_palette = !self.command_palette;
        if self.command_palette {
            self.fast_open = false;
            self.command_palette_query.clear();
            self.command_palette_selected = 0;
            self.command_palette_input
                .update(cx, |input, cx| input.set_content("", cx));
            let focus = self.command_palette_input.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn run_selected_command(&mut self, cx: &mut Context<Self>) {
        let commands = self.filtered_commands();
        let Some(command) = commands.get(self.command_palette_selected).copied() else {
            return;
        };
        self.command_palette = false;
        self.pending_command = Some(command);
        cx.notify();
    }

    fn execute_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        self.command_palette = false;
        match command {
            Command::Save => {
                if let Some(doc) = self.active {
                    self.save_document(doc, cx);
                }
            }
            Command::CloseTab => {
                if let Some(doc) = self.active {
                    self.close_tab(doc, cx);
                }
            }
            Command::ToggleSplit => self.toggle_split(cx),
            Command::QuickOpen => {
                self.fast_open = true;
                self.fast_open_selected = 0;
                let focus = self.fast_open_input.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
            }
            Command::GoToSymbol => {
                self.symbol_search = true;
                self.symbol_selected = 0;
                let focus = self.symbol_input.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
            }
            Command::ShowProject => self.select_sidebar(SidebarMode::Project, cx),
            Command::ShowSearch => self.select_sidebar(SidebarMode::Search, cx),
            Command::ShowGit => self.select_sidebar(SidebarMode::Git, cx),
            Command::ShowOutline => self.select_sidebar(SidebarMode::Outline, cx),
            Command::ShowProblems => self.select_sidebar(SidebarMode::Problems, cx),
            Command::ShowDatabase => self.select_sidebar(SidebarMode::Database, cx),
            Command::ShowHttp => self.select_sidebar(SidebarMode::Http, cx),
            Command::ShowRemote => self.select_sidebar(SidebarMode::Remote, cx),
            Command::ShowNotebook => self.select_sidebar(SidebarMode::Notebook, cx),
            Command::ShowMcp => self.select_sidebar(SidebarMode::Mcp, cx),
            Command::ShowAgents => self.select_sidebar(SidebarMode::Agents, cx),
            Command::ShowPlugins => self.select_sidebar(SidebarMode::Plugins, cx),
            Command::ShowTools => self.select_sidebar(SidebarMode::Tools, cx),
            Command::ShowSettings => self.select_sidebar(SidebarMode::Settings, cx),
            Command::ShowRun => {
                self.bottom_mode = BottomMode::Run;
            }
            Command::ShowTerminal => {
                self.bottom_mode = BottomMode::Terminal;
                self.ensure_terminal(window, cx);
            }
            Command::ShowBuild => {
                self.bottom_mode = BottomMode::Build;
                if self.build_model.is_none() {
                    self.refresh_build(cx);
                }
            }
            Command::ShowDebug => {
                self.bottom_mode = BottomMode::Debug;
                if self.debug_attachables.is_empty() {
                    self.refresh_debug(cx);
                }
            }
            Command::RunSelected => self.start_selected_run(cx),
            Command::BuildProject => {
                self.bottom_mode = BottomMode::Build;
                self.run_build(ide_build::compile::BuildKind::Build, cx);
            }
            Command::RebuildProject => {
                self.bottom_mode = BottomMode::Build;
                self.run_build(ide_build::compile::BuildKind::Rebuild, cx);
            }
            Command::RefreshGit => self.refresh_git(cx),
            Command::RefreshAgents => self.refresh_agents(cx),
            Command::RefreshTools => self.refresh_tools(cx),
            Command::RefreshPlugins => self.refresh_plugins(cx),
            Command::RefreshMarket => self.refresh_market(cx),
            Command::RefreshRemote => self.refresh_remote(cx),
            Command::RefreshDatabase => self.refresh_db(cx),
        }
        cx.notify();
    }

    fn insert_document(
        &mut self,
        info: ide_workspace::DocumentInfo,
        registry: &Arc<language::LanguageRegistry>,
        cx: &mut Context<Self>,
    ) {
        let key = DocumentKey::Path(info.path.0.clone());
        if let Some(existing) = self.store.get(&key).copied() {
            self.activate(existing, cx);
            return;
        }
        let doc = ZedDocument::open(info.clone(), registry.clone(), cx);
        let buffer = doc.buffer.clone();
        let doc_id = doc.doc;
        let subscription = cx.subscribe(&buffer, move |app, _, event: &BufferEvent, cx| {
            if is_buffer_edited(event) {
                app.sync_document(doc_id, cx);
            }
        });
        self.documents.insert(doc_id, doc);
        eprintln!(
            "WORKBENCH_DOC_OPEN doc={} path={} language={} tabs={}",
            doc_id.0,
            info.path.0.display(),
            info.language,
            self.order.len() + 1
        );
        self.subscriptions.insert(doc_id, subscription);
        self.order.push(doc_id);
        if self.store.insert_view(key, doc_id, doc_id) {
            self.active = Some(doc_id);
            self.status = format!("{} · {}", info.path.0.display(), info.language).into();
        }
        cx.notify();
    }

    fn sync_document(&mut self, doc_id: DocId, cx: &mut Context<Self>) {
        let Some(document) = self.documents.get_mut(&doc_id) else {
            return;
        };
        if document.syncing || document.applying_backend_patch {
            return;
        }
        let Some((request_clock, changes)) = document.pending_changes(cx) else {
            return;
        };
        let host = self.host.clone();
        let base_version = document.backend_version;
        cx.spawn(async move |this, cx| {
            let result = host
                .apply_document_edit_async(doc_id, base_version, changes)
                .await;
            let handle = this
                .update(cx, |app, cx| {
                    let Some(document) = app.documents.get_mut(&doc_id) else {
                        return app.window_handle;
                    };
                    document.syncing = false;
                    match result {
                        Ok(version) => {
                            document.backend_version = version;
                            document.confirmed_clock = request_clock.clone();
                            let current = document.buffer.read(cx).snapshot().version().clone();
                            if current != request_clock {
                                app.sync_document(doc_id, cx);
                            } else if document.save_requested {
                                document.save_requested = false;
                                app.save_document(doc_id, cx);
                            }
                        }
                        Err(error) => {
                            document.conflict =
                                Some(format!("OpenIDEA edit failed: {}", error.message));
                            app.status = document.conflict.clone().unwrap().into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn save_document(&mut self, doc_id: DocId, cx: &mut Context<Self>) {
        let Some(document) = self.documents.get(&doc_id) else {
            return;
        };
        if document.syncing {
            if let Some(document) = self.documents.get_mut(&doc_id) {
                document.save_requested = true;
            }
            return;
        }
        let version = document.buffer.read(cx).snapshot().version().clone();
        let buffer = document.buffer.clone();
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.save_document(doc_id).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(()) => {
                            buffer.update(cx, |buffer, cx| buffer.did_save(version, None, cx));
                            app.status = "Saved through OpenIDEA".into();
                        }
                        Err(error) => {
                            app.status = format!("Save failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn start_selected_run(&mut self, cx: &mut Context<Self>) {
        let Some(config) = self.run_configs.get(self.selected_run).cloned() else {
            self.run_status = "No run configuration selected".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
        let emit: ide_host::ops::Emit<ide_run::OutputChunk> = Arc::new(move |chunk| {
            let _ = out_tx.send(chunk);
        });
        self.run_output.clear();
        self.run_status = format!("Starting {}…", config.name).into();
        cx.spawn(async move |this, cx| {
            let result = host.start_run(config, emit).await;
            let started = result.is_ok();
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(info) => {
                            app.run_status =
                                format!("Running {} (pid {:?})", info.name, info.pid).into();
                            app.running_run = Some(info);
                        }
                        Err(error) => {
                            app.run_status = format!("Run failed: {}", error.message).into();
                            app.running_run = None;
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
            if !started {
                return;
            }
            while let Some(chunk) = out_rx.recv().await {
                let text = match chunk {
                    ide_run::OutputChunk::Stdout { text }
                    | ide_run::OutputChunk::Stderr { text }
                    | ide_run::OutputChunk::System { text } => text,
                };
                let handle = this
                    .update(cx, |app, cx| {
                        app.run_output.push_str(&text);
                        if app.run_output.len() > 2_000_000 {
                            let split = app.run_output.len().saturating_sub(1_800_000);
                            app.run_output.drain(..split);
                        }
                        cx.notify();
                        app.window_handle
                    })
                    .ok()
                    .flatten();
                refresh_window(handle, cx);
            }
            let handle = this
                .update(cx, |app, cx| {
                    app.running_run = None;
                    app.run_status = "Process finished".into();
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn stop_active_run(&mut self, cx: &mut Context<Self>) {
        let Some(run) = self.running_run.as_ref() else {
            return;
        };
        if let Err(error) = self.host.stop_run(run.id) {
            self.run_status = format!("Stop failed: {}", error.message).into();
        } else {
            self.run_status = "Stopping…".into();
        }
        cx.notify();
    }

    fn ensure_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminal.is_some() || self.terminal_starting {
            return;
        }
        self.start_terminal(window, cx);
    }

    fn start_terminal(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.terminal.is_some() || self.terminal_starting {
            return;
        }
        self.terminal_starting = true;
        self.terminal_status = "Starting shell…".into();
        self.bottom_mode = BottomMode::Terminal;
        let host = self.host.clone();
        let root = self.root.clone();
        let theme = self.theme;
        let window_handle = self.window_handle;
        let options = ide_terminal::TerminalOptions {
            cwd: ide_core::IpcPath(root),
            shell: None,
            args: Vec::new(),
            env: Default::default(),
            cols: crate::terminal_pane::COLUMNS as u16,
            rows: crate::terminal_pane::ROWS as u16,
            shell_integration: true,
        };
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        cx.spawn(async move |this, cx| {
            let result = host.spawn_terminal(options, output_tx).await;
            this.update_in(cx, |app, window, cx| {
                match result {
                    Ok(info) => {
                        eprintln!(
                            "WORKBENCH_TERMINAL_SPAWN id={} title={} pid={:?}",
                            info.id, info.title, info.pid
                        );
                        let terminal_id = info.id;
                        let terminal = cx.new(|cx| {
                            TerminalPane::new(
                                host.clone(),
                                info.id,
                                info.title.clone().into(),
                                info.pid,
                                theme,
                                window,
                                cx,
                            )
                        });
                        app.terminal = Some(terminal.clone());
                        app.terminal_status = format!("{} · pid {:?}", info.title, info.pid).into();
                        app.terminal_starting = false;
                        app.bottom_mode = BottomMode::Terminal;
                        app.attach_terminal_output(output_rx, cx);
                        if std::env::var_os("RUSTIDEA_ZED_SMOKE_TERMINAL").is_some() {
                            let command = "printf 'WORKBENCH_TERMINAL_OK\\n'\r";
                            if let Err(error) = host.write_terminal(terminal_id, command) {
                                eprintln!("WORKBENCH_TERMINAL_WRITE_ERROR {}", error.message);
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!("WORKBENCH_TERMINAL_ERROR {}", error.message);
                        app.terminal_starting = false;
                        app.terminal_status = format!("Terminal failed: {}", error.message).into();
                    }
                }
                cx.notify();
            })
            .ok();
            refresh_window(window_handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn attach_terminal_output(
        &self,
        mut output_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Some(chunk) = output_rx.recv().await {
                let handle = this
                    .update(cx, |app, cx| {
                        if let Some(terminal) = &app.terminal {
                            terminal.update(cx, |pane, cx| pane.feed(&chunk, cx));
                        }
                        cx.notify();
                        app.window_handle
                    })
                    .ok()
                    .flatten();
                refresh_window(handle, cx);
            }
            let handle = this
                .update(cx, |app, cx| {
                    if let Some(terminal) = &app.terminal {
                        terminal.update(cx, |pane, cx| pane.mark_exited(None, cx));
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    pub fn terminal_text(&self, cx: &gpui::App) -> Option<String> {
        self.terminal
            .as_ref()
            .map(|terminal| terminal.read(cx).text(cx))
    }

    pub fn terminal_status(&self) -> &gpui::SharedString {
        &self.terminal_status
    }

    pub fn smoke_start_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bottom_mode = BottomMode::Terminal;
        self.start_terminal(window, cx);
    }

    fn start_search(&mut self, pattern: String, cx: &mut Context<Self>) {
        let pattern = pattern.trim().to_owned();
        if pattern.is_empty() {
            self.search_results.clear();
            self.search_status = "Type a pattern and press Enter".into();
            cx.notify();
            return;
        }
        self.search_results.clear();
        self.search_status = format!("Searching for {pattern}…").into();
        let query = ide_search::SearchQuery {
            pattern,
            is_regex: false,
            case_sensitive: false,
            whole_word: false,
            include: Vec::new(),
            exclude: Vec::new(),
            scope: ide_search::SearchScope::Project,
            max_matches: 10_000,
            context: ide_search::SearchContext::Any,
        };
        let host = self.host.clone();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let emit: ide_host::ops::Emit<ide_host::ops::search::SearchEvent> =
            Arc::new(move |event| {
                let _ = event_tx.send(event);
            });
        cx.spawn(async move |this, cx| {
            let result = host.start_search(query, emit).await;
            if let Err(error) = &result {
                let handle = this
                    .update(cx, |app, cx| {
                        app.search_status = format!("Search failed: {}", error.message).into();
                        cx.notify();
                        app.window_handle
                    })
                    .ok()
                    .flatten();
                refresh_window(handle, cx);
                return;
            }
            while let Some(event) = event_rx.recv().await {
                let handle = this
                    .update(cx, |app, cx| {
                        match event {
                            ide_host::ops::search::SearchEvent::File { file } => {
                                app.search_results.push(file);
                                app.search_status = format!(
                                    "{} files, {} matches",
                                    app.search_results.len(),
                                    app.search_results
                                        .iter()
                                        .map(|file| file.matches.len())
                                        .sum::<usize>()
                                )
                                .into();
                            }
                            ide_host::ops::search::SearchEvent::Done { summary } => {
                                app.search_status = format!(
                                    "{} files, {} matches{}",
                                    summary.files,
                                    summary.matches,
                                    if summary.limit_hit {
                                        " (limit reached)"
                                    } else {
                                        ""
                                    }
                                )
                                .into();
                            }
                        }
                        cx.notify();
                        app.window_handle
                    })
                    .ok()
                    .flatten();
                refresh_window(handle, cx);
            }
        })
        .detach();
        cx.notify();
    }

    fn refresh_git(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.git_status_message = "Loading Git status…".into();
        cx.spawn(async move |this, cx| {
            let (status, branch) = tokio::join!(host.git_status(), host.git_branch());
            let handle = this
                .update(cx, |app, cx| {
                    match status {
                        Ok(status) => {
                            app.git_status_message = if status.is_empty() {
                                "No changes".into()
                            } else {
                                format!("{} changed files", status.len()).into()
                            };
                            app.git_status = status;
                        }
                        Err(error) => {
                            app.git_status.clear();
                            app.git_status_message = error.message.into();
                        }
                    }
                    if let Ok(branch) = branch {
                        app.git_branch = Some(branch);
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn stage_all(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self
            .git_status
            .iter()
            .map(|status| status.path.0.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let host = self.host.clone();
        self.git_status_message = "Staging files…".into();
        cx.spawn(async move |this, cx| {
            let result = host.git_stage(paths).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.git_status_message = match result {
                        Ok(()) => "Staged all changes".into(),
                        Err(error) => format!("Stage failed: {}", error.message).into(),
                    };
                    app.refresh_git(cx);
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn unstage_all(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self
            .git_status
            .iter()
            .filter(|status| status.staged.is_some())
            .map(|status| status.path.0.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let host = self.host.clone();
        self.git_status_message = "Unstaging files…".into();
        cx.spawn(async move |this, cx| {
            let result = host.git_unstage(paths).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.git_status_message = match result {
                        Ok(()) => "Unstaged changes".into(),
                        Err(error) => format!("Unstage failed: {}", error.message).into(),
                    };
                    app.refresh_git(cx);
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn commit_git(&mut self, message: String, cx: &mut Context<Self>) {
        let message = message.trim().to_owned();
        if message.is_empty() {
            self.git_status_message = "Enter a commit message".into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        self.git_status_message = "Committing…".into();
        cx.spawn(async move |this, cx| {
            let result = host.git_commit(message).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(commit) => {
                            app.git_status_message =
                                format!("Created commit {}", &commit[..commit.len().min(12)])
                                    .into();
                            app.git_message_input.update(cx, |input, cx| {
                                input.set_content("", cx);
                            });
                        }
                        Err(error) => {
                            app.git_status_message =
                                format!("Commit failed: {}", error.message).into();
                        }
                    }
                    app.refresh_git(cx);
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    pub fn smoke_insert_marker(&mut self, marker: &str, cx: &mut Context<Self>) -> bool {
        let Some(doc) = self.active else {
            return false;
        };
        let Some(document) = self.documents.get(&doc) else {
            return false;
        };
        let buffer = document.buffer.clone();
        buffer.update(cx, |buffer, cx| {
            buffer.edit([(0..0, format!("{marker}\n"))], None, cx);
        });
        true
    }

    pub fn smoke_save_active(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(doc) = self.active else {
            return false;
        };
        self.save_document(doc, cx);
        true
    }

    pub fn smoke_request_completion(&mut self, line: u32, column: u32, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            return;
        };
        let Some(document) = self.documents.get(&doc) else {
            return;
        };
        let lsp = Arc::new(DocumentLsp::new(
            self.host.clone(),
            doc,
            document.path.clone(),
        ));
        cx.spawn(async move |_this, cx| {
            let mut count = 0;
            for attempt in 0..30 {
                match lsp.completion(line, column, Some(".".into())).await {
                    Ok(list) => {
                        count = list.items.len();
                        if count > 0 {
                            break;
                        }
                    }
                    Err(error) if attempt == 29 => {
                        eprintln!("WORKBENCH_COMPLETION_ERROR {}", error.message);
                    }
                    Err(_) => {}
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
            }
            eprintln!("WORKBENCH_COMPLETION count={count}");
        })
        .detach();
    }

    pub fn smoke_active_text(&self, cx: &gpui::App) -> String {
        self.active
            .and_then(|doc| self.documents.get(&doc))
            .map(|document| document.text(cx))
            .unwrap_or_default()
    }

    pub fn smoke_status(&self) -> String {
        self.status.to_string()
    }

    pub fn smoke_enable_split(&mut self, cx: &mut Context<Self>) {
        if !self.split {
            self.toggle_split(cx);
        }
    }

    pub fn smoke_editor_count(&self) -> usize {
        self.editors.len()
    }

    pub fn smoke_start_run(&mut self, cx: &mut Context<Self>) {
        self.start_selected_run(cx);
    }

    pub fn smoke_outline(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Outline, cx);
    }

    pub fn outline_status(&self) -> &gpui::SharedString {
        &self.outline_status
    }

    pub fn outline_count(&self) -> usize {
        fn count(items: &[OutlineItem]) -> usize {
            items.iter().map(|item| 1 + count(&item.children)).sum()
        }
        count(&self.outline)
    }

    pub fn problem_count(&self) -> usize {
        self.problems.len()
    }

    pub fn smoke_http(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Http, cx);
        self.run_selected_http(cx);
    }

    pub fn http_status(&self) -> &gpui::SharedString {
        &self.http_status
    }

    pub fn http_output(&self) -> &str {
        &self.http_output
    }

    pub fn db_status(&self) -> &gpui::SharedString {
        &self.db_status
    }

    pub fn smoke_db(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Database, cx);
    }

    pub fn smoke_remote(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Remote, cx);
    }

    pub fn remote_status(&self) -> &gpui::SharedString {
        &self.remote_status
    }

    pub fn smoke_agents(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Agents, cx);
    }

    pub fn smoke_tools(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Tools, cx);
    }

    pub fn agents_count(&self) -> usize {
        self.agents.len()
    }

    pub fn agent_tasks_count(&self) -> usize {
        self.agent_tasks.len()
    }

    pub fn mcp_count(&self) -> usize {
        self.mcp_servers.len()
    }

    pub fn tools_count(&self) -> usize {
        self.tools.len()
    }

    pub fn smoke_build(&mut self, cx: &mut Context<Self>) {
        self.bottom_mode = BottomMode::Build;
        self.run_build(ide_build::compile::BuildKind::Build, cx);
    }

    pub fn build_status(&self) -> &gpui::SharedString {
        &self.build_status
    }

    pub fn build_output(&self) -> &str {
        &self.build_output
    }

    pub fn smoke_plugins(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Plugins, cx);
    }

    pub fn smoke_mcp_roundtrip(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Mcp, cx);
        self.new_mcp_server(cx);
        self.mcp_name_input.update(cx, |input, cx| {
            input.set_content("smoke-echo", cx);
        });
        self.mcp_command_input.update(cx, |input, cx| {
            input.set_content("/bin/echo", cx);
        });
        self.mcp_args_input.update(cx, |input, cx| {
            input.set_content("hello", cx);
        });
        self.save_mcp_server(cx);
        let added = self.mcp_servers.len();
        let Some(server) = self
            .mcp_servers
            .iter()
            .find(|server| server.name == "smoke-echo")
            .cloned()
        else {
            eprintln!(
                "WORKBENCH_MCP_ROUNDTRIP add_failed status={}",
                self.mcp_status
            );
            return;
        };
        self.mcp_selected = Some(server.id);
        self.remove_selected_mcp_server(cx);
        eprintln!(
            "WORKBENCH_MCP_ROUNDTRIP added={added} removed={} status={}",
            self.mcp_servers.len(),
            self.mcp_status
        );
    }

    pub fn smoke_quick_open(&mut self, query: String, window: &mut Window, cx: &mut Context<Self>) {
        self.fast_open = true;
        self.fast_open_input.update(cx, |input, cx| {
            input.set_content(query.clone(), cx);
        });
        let focus = self.fast_open_input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.search_quick_open(query, cx);
        cx.notify();
    }

    pub fn fast_open_count(&self) -> usize {
        self.fast_open_results.len()
    }

    pub fn fast_open_status(&self) -> &gpui::SharedString {
        &self.fast_open_status
    }

    pub fn smoke_command_palette(
        &mut self,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_palette = true;
        self.command_palette_input.update(cx, |input, cx| {
            input.set_content(query.clone(), cx);
        });
        self.command_palette_query = query;
        self.command_palette_selected = 0;
        let focus = self.command_palette_input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    pub fn command_palette_count(&self) -> usize {
        self.filtered_commands().len()
    }

    pub fn command_palette_first(&self) -> String {
        self.filtered_commands()
            .first()
            .map(|command| command.label().to_owned())
            .unwrap_or_default()
    }

    pub fn smoke_command_palette_execute(
        &mut self,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_palette = true;
        self.command_palette_query = query;
        self.command_palette_selected = 0;
        let command = self.filtered_commands().first().copied();
        if let Some(command) = command {
            self.execute_command(command, window, cx);
        }
    }

    pub fn sidebar_mode_name(&self) -> &'static str {
        match self.sidebar_mode {
            SidebarMode::Project => "Project",
            SidebarMode::Search => "Search",
            SidebarMode::Git => "Git",
            SidebarMode::Outline => "Outline",
            SidebarMode::Problems => "Problems",
            SidebarMode::Database => "Database",
            SidebarMode::Http => "Http",
            SidebarMode::Remote => "Remote",
            SidebarMode::Notebook => "Notebook",
            SidebarMode::Mcp => "Mcp",
            SidebarMode::Agents => "Agents",
            SidebarMode::Plugins => "Plugins",
            SidebarMode::Tools => "Tools",
            SidebarMode::Settings => "Settings",
        }
    }

    pub fn smoke_symbol_search(
        &mut self,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.symbol_search = true;
        self.symbol_input.update(cx, |input, cx| {
            input.set_content(query.clone(), cx);
        });
        let focus = self.symbol_input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.search_symbols(query, cx);
        cx.notify();
    }

    pub fn symbol_count(&self) -> usize {
        self.symbol_results.len()
    }

    pub fn symbol_first(&self) -> String {
        self.symbol_results
            .first()
            .map(|symbol| format!("{} · {}", symbol.name, symbol.rel_path))
            .unwrap_or_default()
    }

    pub fn smoke_settings_toggle(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Settings, cx);
        let current = self.settings_data.editor.show_whitespace;
        self.set_setting(
            "editor.showWhitespace".into(),
            serde_json::Value::Bool(!current),
            cx,
        );
    }

    pub fn settings_whitespace(&self) -> bool {
        self.settings_data.editor.show_whitespace
    }

    pub fn settings_row_count(&self) -> usize {
        let mut leaves = Vec::new();
        if let Ok(value) = serde_json::to_value(&self.settings_data) {
            flatten_json("", &value, &mut leaves);
        }
        leaves.len()
    }

    pub fn smoke_settings_string(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Settings, cx);
        self.open_setting_editor(
            "editor.fontFamily".into(),
            serde_json::Value::String("Menlo".into()),
            cx,
        );
        self.save_setting_editor(cx);
    }

    pub fn settings_font_family(&self) -> &str {
        &self.settings_data.editor.font_family
    }

    pub fn plugins_count(&self) -> usize {
        self.plugins.len()
    }

    pub fn smoke_market(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Plugins, cx);
        self.refresh_market(cx);
    }

    pub fn market_count(&self) -> usize {
        self.market_plugins.len()
    }

    pub fn market_status(&self) -> &gpui::SharedString {
        &self.market_status
    }

    pub fn plugin_preview_status(&self) -> &gpui::SharedString {
        &self.plugin_preview_status
    }

    pub fn smoke_debug(&mut self, cx: &mut Context<Self>) {
        self.bottom_mode = BottomMode::Debug;
        self.refresh_debug(cx);
    }

    pub fn debug_attachable_count(&self) -> usize {
        self.debug_attachables.len()
    }

    pub fn smoke_notebook(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Notebook, cx);
    }

    pub fn smoke_notebook_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(cell) = self
            .notebook
            .as_ref()
            .and_then(|notebook| notebook.cells.first())
            .cloned()
        else {
            return;
        };
        self.open_notebook_cell_editor(cell.id, cell.source, window, cx);
        if let Some(buffer) = self.notebook_edit_buffer.clone() {
            buffer.update(cx, |buffer, cx| {
                buffer.edit([(0..0, "# rustidea notebook edit\n")], None, cx);
            });
        }
        self.commit_notebook_cell_editor(cx);
    }

    pub fn notebook_first_source(&self) -> String {
        self.notebook
            .as_ref()
            .and_then(|notebook| notebook.cells.first())
            .map(|cell| cell.source.lines().next().unwrap_or_default().to_owned())
            .unwrap_or_default()
    }

    pub fn notebook_cell_count(&self) -> usize {
        self.notebook
            .as_ref()
            .map(|notebook| notebook.cells.len())
            .unwrap_or(0)
    }

    pub fn smoke_run_status(&self) -> String {
        self.run_status.to_string()
    }

    pub fn smoke_run_output(&self) -> &str {
        &self.run_output
    }

    pub fn smoke_search(&mut self, pattern: &str, cx: &mut Context<Self>) {
        self.sidebar_mode = SidebarMode::Search;
        self.start_search(pattern.to_owned(), cx);
    }

    pub fn smoke_search_status(&self) -> String {
        self.search_status.to_string()
    }

    pub fn smoke_search_files(&self) -> usize {
        self.search_results.len()
    }

    pub fn smoke_git_files(&self) -> usize {
        self.git_status.len()
    }

    pub fn smoke_git_branch(&self) -> String {
        self.git_branch
            .as_ref()
            .and_then(|branch| branch.head.clone())
            .unwrap_or_default()
    }

    pub fn smoke_has_language(&self, cx: &gpui::App) -> bool {
        self.active
            .and_then(|doc| self.documents.get(&doc))
            .is_some_and(|document| document.buffer.read(cx).language().is_some())
    }

    fn handle_event(&mut self, event: IdeEvent, cx: &mut Context<Self>) {
        match event {
            IdeEvent::DocPatched {
                doc,
                version,
                changes,
                ..
            } => {
                if let Some(document) = self.documents.get_mut(&doc)
                    && let Err(error) = document.apply_backend_patch(version, &changes, cx)
                {
                    self.status = format!("Document patch failed: {error:#}").into();
                }
            }
            IdeEvent::Diagnostics {
                path,
                source,
                items,
            } => {
                let Some((doc_id, _)) = self
                    .documents
                    .iter()
                    .find(|(_, document)| document.path == path.0)
                else {
                    return;
                };
                let doc_id = *doc_id;
                let server_id = *self
                    .diagnostic_servers
                    .entry(source.clone())
                    .or_insert_with(|| {
                        let id = LanguageServerId(self.next_diagnostic_server);
                        self.next_diagnostic_server += 1;
                        id
                    });
                let Some(document) = self.documents.get(&doc_id) else {
                    return;
                };
                let buffer = document.buffer.clone();
                let snapshot = buffer.read(cx).snapshot();
                self.problems.retain(|problem| problem.path != path.0);
                for diagnostic in &items {
                    self.problems.push(ProblemItem {
                        path: path.0.clone(),
                        line: diagnostic.range.start.line,
                        column: diagnostic.range.start.col,
                        severity: diagnostic.severity,
                        message: diagnostic.message.clone(),
                        source: diagnostic.source.clone().or(Some(source.clone())),
                    });
                }
                if self.problems.len() > 5_000 {
                    self.problems.drain(..self.problems.len() - 5_000);
                }
                let entries: Vec<DiagnosticEntry<text::PointUtf16>> = items
                    .into_iter()
                    .map(|diagnostic| {
                        let range = text::PointUtf16::new(
                            diagnostic.range.start.line,
                            diagnostic.range.start.col,
                        )
                            ..text::PointUtf16::new(
                                diagnostic.range.end.line,
                                diagnostic.range.end.col,
                            );
                        let zed = Diagnostic {
                            source: Some(source.clone()),
                            severity: match diagnostic.severity {
                                ide_core::Severity::Error => lsp::DiagnosticSeverity::ERROR,
                                ide_core::Severity::Warning => lsp::DiagnosticSeverity::WARNING,
                                ide_core::Severity::Info => lsp::DiagnosticSeverity::INFORMATION,
                                ide_core::Severity::Hint => lsp::DiagnosticSeverity::HINT,
                            },
                            message: DiagnosticMessage::plain(diagnostic.message),
                            ..Default::default()
                        };
                        DiagnosticEntry::new(range, zed)
                    })
                    .collect();
                let set = DiagnosticSet::new(entries, &snapshot);
                let count = set.len();
                buffer.update(cx, |buffer, cx| {
                    buffer.update_diagnostics(server_id, set, cx);
                });
                self.diagnostics.insert(path.0, count);
            }
            IdeEvent::LanguageServer { name, status, .. } => {
                self.status = format!("{name}: {status:?}").into();
            }
            IdeEvent::RunExited { run, code } => {
                if self
                    .running_run
                    .as_ref()
                    .is_some_and(|running| running.id == run)
                {
                    self.running_run = None;
                    self.run_status = format!("Exited with {:?}", code).into();
                }
            }
            IdeEvent::TerminalExited { terminal, code } => {
                if let Some(pane) = &self.terminal
                    && pane.read(cx).id() == terminal
                {
                    pane.update(cx, |pane, cx| pane.mark_exited(code, cx));
                    self.terminal_status = match code {
                        Some(code) => format!("Terminal exited with {code}").into(),
                        None => "Terminal closed".into(),
                    };
                }
            }
            IdeEvent::FsChanged {
                modified, created, ..
            } => {
                let changed: Vec<PathBuf> = created
                    .into_iter()
                    .chain(modified)
                    .map(|path| path.0)
                    .collect();
                let reload = self.tree.note_external_changes(&changed, &[]);
                for directory in reload {
                    self.load_directory(directory, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut row = div()
            .h(px(34.))
            .flex()
            .items_center()
            .bg(theme.titlebar)
            .border_b_1()
            .border_color(theme.border);
        for doc_id in &self.order {
            let Some(document) = self.documents.get(doc_id) else {
                continue;
            };
            let active = self.active == Some(*doc_id);
            let title = document
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| document.path.display().to_string());
            let dirty = document.is_dirty(cx);
            let id = *doc_id;
            row = row.child(
                div()
                    .id(("tab", id.0))
                    .h_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_r_1()
                    .border_color(theme.border)
                    .bg(if active {
                        theme.tab_active
                    } else {
                        theme.titlebar
                    })
                    .text_color(if active { theme.text } else { theme.muted })
                    .text_size(px(12.))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.activate(id, cx);
                        if let Some(editor) = this.editors.get(&(id, false)) {
                            let focus = editor.read(cx).focus_handle(cx);
                            window.focus(&focus, cx);
                        }
                    }))
                    .child(title)
                    .when(dirty, |tab| {
                        tab.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(theme.accent))
                    })
                    .child(
                        div()
                            .id(("close-tab", id.0))
                            .px_1()
                            .text_color(theme.faint)
                            .hover(|style| style.text_color(theme.error))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.close_tab(id, cx);
                            }))
                            .child("×"),
                    ),
            );
        }
        row = row.child(div().flex_1()).child(
            div()
                .id("toggle-split")
                .px_3()
                .text_color(theme.muted)
                .text_size(px(11.))
                .cursor_pointer()
                .hover(|style| style.text_color(theme.text))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_split(cx);
                }))
                .child(if self.split { "Unsplit" } else { "Split" }),
        );
        row
    }

    fn render_quick_open(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.fast_open {
            return div().into_any_element();
        }
        let theme = self.theme;
        let mut results = div()
            .id("quick-open-results")
            .flex_1()
            .min_h(px(0.))
            .max_h(px(320.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, result) in self.fast_open_results.iter().take(40).enumerate() {
            let selected = index == self.fast_open_selected;
            results = results.child(
                div()
                    .id(("quick-open-result", index))
                    .h(px(26.))
                    .px_2()
                    .flex()
                    .items_center()
                    .text_size(px(11.))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.fast_open_selected = index;
                        this.open_quick_open_result(cx);
                    }))
                    .child(result.rel_path.clone()),
            );
        }
        div()
            .absolute()
            .top(px(70.))
            .left(px(300.))
            .w(px(560.))
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_1()
            .border_color(theme.accent)
            .rounded_md()
            .shadow_lg()
            .child(
                div()
                    .h(px(36.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.fast_open_input.clone()),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(9.5))
                    .text_color(theme.faint)
                    .child(self.fast_open_status.clone()),
            )
            .child(results)
            .into_any_element()
    }

    fn render_command_palette(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.command_palette {
            return div().into_any_element();
        }
        let theme = self.theme;
        let commands = self.filtered_commands();
        let mut rows = div()
            .id("command-palette-results")
            .flex_1()
            .min_h(px(0.))
            .max_h(px(360.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, command) in commands.iter().take(80).enumerate() {
            let selected = index == self.command_palette_selected;
            let command = *command;
            rows = rows.child(
                div()
                    .id(("command-palette-row", index))
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .text_size(px(11.))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.command_palette_selected = index;
                        this.execute_command(command, window, cx);
                    }))
                    .child(command.label()),
            );
        }
        div()
            .absolute()
            .top(px(70.))
            .left(px(280.))
            .w(px(600.))
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_1()
            .border_color(theme.accent)
            .rounded_md()
            .shadow_lg()
            .child(
                div()
                    .h(px(36.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.command_palette_input.clone()),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .text_size(px(9.5))
                    .text_color(theme.faint)
                    .child(format!("{} commands · ⌘⇧P", commands.len())),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_symbol_search(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.symbol_search {
            return div().into_any_element();
        }
        let theme = self.theme;
        let mut rows = div()
            .id("symbol-results")
            .flex_1()
            .min_h(px(0.))
            .max_h(px(360.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, symbol) in self.symbol_results.iter().take(60).enumerate() {
            let selected = index == self.symbol_selected;
            rows = rows.child(
                div()
                    .id(("symbol-result", index))
                    .h(px(34.))
                    .px_2()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .text_size(px(10.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.symbol_selected = index;
                        this.open_symbol_result(index, window, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().truncate().child(symbol.name.clone()))
                            .when_some(symbol.container.clone(), |row, container| {
                                row.child(
                                    div()
                                        .text_size(px(9.))
                                        .text_color(theme.faint)
                                        .child(container),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(symbol.rel_path.clone()),
                    ),
            );
        }
        div()
            .absolute()
            .top(px(70.))
            .left(px(280.))
            .w(px(600.))
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_1()
            .border_color(theme.accent)
            .rounded_md()
            .shadow_lg()
            .child(
                div()
                    .h(px(36.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.symbol_input.clone()),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(9.5))
                    .text_color(theme.faint)
                    .child(self.symbol_status.clone()),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_project(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let nodes = self.tree.nodes();
        let row_count = nodes.len();
        let app = cx.entity();
        uniform_list("project-tree", row_count, move |range, _window, _cx| {
            let nodes = nodes.clone();
            let app = app.clone();
            range
                .map(|index| {
                    let node = nodes[index].clone();
                    let name = node.name.clone();
                    let depth = node.depth;
                    let is_directory = node.is_directory;
                    let expanded = node.expanded;
                    let is_more = node.more;
                    let color = if is_more {
                        theme.accent
                    } else if node.ignored {
                        theme.faint
                    } else if is_directory {
                        theme.muted
                    } else {
                        theme.text
                    };
                    let click_app = app.clone();
                    div()
                        .id(("tree-row", index))
                        .h(px(23.))
                        .pl(px(6.0 + depth as f32 * 13.0))
                        .pr_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(12.5))
                        .text_color(color)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(move |_: &ClickEvent, window, cx| {
                            click_app.update(cx, |app, cx| {
                                app.toggle_tree_row(index, Some(window), cx);
                            });
                        })
                        .child(div().w(px(13.)).flex_none().text_color(theme.faint).child(
                            if is_more {
                                "＋".to_string()
                            } else if is_directory {
                                if expanded { "▾" } else { "▸" }.to_string()
                            } else {
                                "·".to_string()
                            },
                        ))
                        .child(div().flex_1().min_w(px(0.)).truncate().child(name))
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.tree_scroll)
    }

    fn append_outline_rows(
        &self,
        items: &[OutlineItem],
        depth: usize,
        doc: DocId,
        rows: &mut Vec<gpui::AnyElement>,
        cx: &mut Context<Self>,
    ) {
        let theme = self.theme;
        for item in items.iter().take(200) {
            let line = item.selection_range.start.line;
            let column = item.selection_range.start.col;
            let label = item.name.clone();
            let detail = item.detail.clone();
            rows.push(
                div()
                    .id(("outline-item", rows.len()))
                    .h(px(24.))
                    .pl(px(8.0 + depth as f32 * 12.0))
                    .pr_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .text_color(theme.text)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.goto_position(doc, line, column, window, cx);
                    }))
                    .child(div().flex_1().truncate().child(label))
                    .when_some(detail, |row, detail| {
                        row.child(
                            div()
                                .text_size(px(9.5))
                                .text_color(theme.faint)
                                .truncate()
                                .child(detail),
                        )
                    })
                    .into_any_element(),
            );
            self.append_outline_rows(&item.children, depth + 1, doc, rows, cx);
        }
    }

    fn render_outline(&self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let Some(doc) = self.active else {
            return div()
                .p_3()
                .text_size(px(11.))
                .text_color(theme.faint)
                .child("Open a file to see its structure")
                .into_any_element();
        };
        let mut rows = Vec::new();
        self.append_outline_rows(&self.outline, 0, doc, &mut rows, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(self.outline_status.clone()),
            )
            .child(
                div()
                    .id("outline-list")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_problems(&self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = Vec::new();
        let mut problems = self.problems.clone();
        problems.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.line.cmp(&right.line))
                .then(left.column.cmp(&right.column))
        });
        for (index, problem) in problems.into_iter().take(500).enumerate() {
            let color = match problem.severity {
                Severity::Error => theme.error,
                Severity::Warning => theme.warning,
                Severity::Info => theme.accent,
                Severity::Hint => theme.muted,
            };
            let path = problem.path.clone();
            let file = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            let line = problem.line;
            let column = problem.column;
            let location = format!("{file}:{}:{}", line + 1, column + 1);
            let message = problem.message.clone();
            let source = problem.source.clone().unwrap_or_default();
            rows.push(
                div()
                    .id(("problem", index))
                    .min_h(px(30.))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.open_or_goto(path.clone(), line, column, window, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(10.))
                            .text_color(color)
                            .child(location)
                            .when(!source.is_empty(), |row| {
                                row.child(div().text_color(theme.faint).child(source))
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.text)
                            .truncate()
                            .child(message),
                    )
                    .into_any_element(),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(format!(
                        "{} problems · {} errors",
                        self.problems.len(),
                        self.problems
                            .iter()
                            .filter(|problem| problem.severity == Severity::Error)
                            .count()
                    )),
            )
            .child(
                div()
                    .id("problems-list")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .when(rows.is_empty(), |list| {
                        list.child(
                            div()
                                .p_3()
                                .text_size(px(11.))
                                .text_color(theme.faint)
                                .child("No problems reported by the backend"),
                        )
                    })
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let filter = self.settings_filter.to_lowercase();
        let mut leaves = Vec::new();
        if let Ok(value) = serde_json::to_value(&self.settings_data) {
            flatten_json("", &value, &mut leaves);
        }
        let mut rows = Vec::new();
        for (index, (key, value)) in leaves.into_iter().enumerate() {
            if !filter.is_empty() && !key.to_lowercase().contains(&filter) {
                continue;
            }
            if rows.len() >= 1_500 {
                break;
            }
            let key_for_toggle = key.clone();
            let key_for_edit = key.clone();
            let value_for_edit = value.clone();
            let enabled = value.as_bool();
            rows.push(
                div()
                    .id(("setting-row", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(px(10.5))
                    .when(enabled.is_none(), |row| {
                        row.cursor_pointer()
                            .hover(|style| style.bg(theme.panel_alt))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.open_setting_editor(
                                    key_for_edit.clone(),
                                    value_for_edit.clone(),
                                    cx,
                                );
                            }))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_color(theme.muted)
                            .child(key),
                    )
                    .child(match enabled {
                        Some(enabled) => div()
                            .id(("setting-toggle", index))
                            .w(px(26.))
                            .h(px(14.))
                            .rounded_full()
                            .bg(if enabled {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .border_1()
                            .border_color(if enabled { theme.accent } else { theme.border })
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.set_setting(
                                    key_for_toggle.clone(),
                                    serde_json::Value::Bool(!enabled),
                                    cx,
                                );
                            }))
                            .child(
                                div()
                                    .m(px(1.))
                                    .w(px(11.))
                                    .h(px(11.))
                                    .rounded_full()
                                    .bg(if enabled { theme.accent } else { theme.faint }),
                            )
                            .into_any_element(),
                        None => div()
                            .max_w(px(180.))
                            .truncate()
                            .text_color(theme.faint)
                            .child(display_json_value(&value))
                            .into_any_element(),
                    })
                    .into_any_element(),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div().p_2().child(
                    div()
                        .h(px(30.))
                        .px_2()
                        .rounded_md()
                        .bg(theme.window)
                        .border_1()
                        .border_color(theme.border)
                        .child(self.settings_filter_input.clone()),
                ),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(9.5))
                    .text_color(theme.faint)
                    .child(self.settings_status.clone()),
            )
            .child(
                div()
                    .id("settings-list")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_notebook_editor(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(editor) = &self.notebook_editor else {
            return div().into_any_element();
        };
        let theme = self.theme;
        div()
            .absolute()
            .top(px(60.))
            .left(px(220.))
            .w(px(760.))
            .h(px(520.))
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_1()
            .border_color(theme.accent)
            .rounded_md()
            .shadow_lg()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .text_size(px(10.5))
                    .text_color(theme.muted)
                    .child(self.notebook_edit_status.clone()),
            )
            .child(div().flex_1().min_h(px(0.)).child(editor.clone()))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("notebook-edit-commit")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.commit_notebook_cell_editor(cx);
                            }))
                            .child("Commit"),
                    )
                    .child(
                        div()
                            .id("notebook-edit-cancel")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.notebook_editor = None;
                                this.notebook_edit_buffer = None;
                                this.notebook_edit_cell = None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    ),
            )
            .into_any_element()
    }

    fn render_agent_form_answer(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some((_, permission)) = &self.agent_answer_target else {
            return div().into_any_element();
        };
        let theme = self.theme;
        div()
            .absolute()
            .top(px(140.))
            .left(px(300.))
            .w(px(560.))
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_1()
            .border_color(theme.accent)
            .rounded_md()
            .shadow_lg()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.5))
                    .text_color(theme.muted)
                    .child(format!("Agent form request #{permission}")),
            )
            .child(div().px_2().pb_1().child(self.agent_answer_input.clone()))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(9.5))
                    .text_color(theme.faint)
                    .child(self.agent_answer_status.clone()),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("agent-form-save")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.submit_agent_form_answer(cx);
                            }))
                            .child("Submit"),
                    )
                    .child(
                        div()
                            .id("agent-form-cancel")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.agent_answer_target = None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    ),
            )
            .into_any_element()
    }

    fn render_setting_editor(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(key) = &self.setting_edit_key else {
            return div().into_any_element();
        };
        let theme = self.theme;
        div()
            .absolute()
            .top(px(140.))
            .left(px(300.))
            .w(px(540.))
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_1()
            .border_color(theme.accent)
            .rounded_md()
            .shadow_lg()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.5))
                    .text_color(theme.muted)
                    .child(key.clone()),
            )
            .child(div().px_2().pb_1().child(self.setting_edit_input.clone()))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(9.5))
                    .text_color(theme.faint)
                    .child(self.setting_edit_status.clone()),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("setting-editor-save")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.save_setting_editor(cx);
                            }))
                            .child("Save"),
                    )
                    .child(
                        div()
                            .id("setting-editor-cancel")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.setting_edit_key = None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    ),
            )
            .into_any_element()
    }

    fn render_http(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut requests = div()
            .id("http-requests")
            .flex_none()
            .max_h(px(150.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, block) in self.http_blocks.iter().take(80).enumerate() {
            let selected = index == self.http_selected;
            requests = requests.child(
                div()
                    .id(("http-request", index))
                    .px_2()
                    .py_1()
                    .text_size(px(10.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.http_selected = index;
                        cx.notify();
                    }))
                    .child(format!("{} {}", block.method, block.url)),
            );
        }
        let mut lines: Vec<String> = self.http_output.lines().map(str::to_owned).collect();
        if lines.len() > 400 {
            lines.drain(..lines.len() - 400);
        }
        let output = div()
            .id("http-output")
            .flex_1()
            .min_h(px(0.))
            .overflow_scroll()
            .p_2()
            .font_family("Menlo")
            .text_size(px(10.5))
            .text_color(theme.text)
            .flex()
            .flex_col()
            .when(lines.is_empty(), |view| {
                view.child(
                    div()
                        .text_color(theme.faint)
                        .child("Response appears here."),
                )
            })
            .children(lines.into_iter().map(|line| div().child(line)));
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("http-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_http(cx);
                            }))
                            .child("Parse"),
                    )
                    .child(
                        div()
                            .id("http-send")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.run_selected_http(cx);
                            }))
                            .child("Send"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.http_status.clone()),
                    ),
            )
            .child(requests)
            .child(output)
            .into_any_element()
    }

    fn render_db(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut sources = div()
            .id("db-sources")
            .flex_none()
            .max_h(px(180.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, source) in self.db_sources.iter().take(80).enumerate() {
            let selected = index == self.db_selected;
            let connected = self.db_connected.contains(&source.id);
            sources = sources.child(
                div()
                    .id(("db-source", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.db_selected = index;
                        cx.notify();
                    }))
                    .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(if connected {
                        theme.accent
                    } else {
                        theme.faint
                    }))
                    .child(div().flex_1().truncate().child(source.name.clone()))
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .child(format!("{:?}", source.driver).to_lowercase()),
                    ),
            );
        }
        let mut lines: Vec<String> = self.db_output.lines().map(str::to_owned).collect();
        if lines.len() > 400 {
            lines.drain(..lines.len() - 400);
        }
        let output = div()
            .id("db-output")
            .flex_1()
            .min_h(px(0.))
            .overflow_scroll()
            .p_2()
            .font_family("Menlo")
            .text_size(px(10.5))
            .text_color(theme.text)
            .flex()
            .flex_col()
            .when(lines.is_empty(), |view| {
                view.child(
                    div()
                        .text_color(theme.faint)
                        .child("Open a SQL file, select a source, then run."),
                )
            })
            .children(lines.into_iter().map(|line| div().child(line)));
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("db-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_db(cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(
                        div()
                            .id("db-connect")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.connect_selected_db(cx);
                            }))
                            .child("Connect"),
                    )
                    .child(
                        div()
                            .id("db-run")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.run_selected_db_sql(cx);
                            }))
                            .child("Run SQL"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.db_status.clone()),
                    ),
            )
            .child(sources)
            .child(output)
            .into_any_element()
    }

    fn render_remote(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("ssh-configs")
            .flex_none()
            .max_h(px(100.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, config) in self.ssh_configs.iter().take(80).enumerate() {
            let selected = index == self.ssh_selected;
            let name = config.name.clone();
            let label = config.label();
            rows = rows.child(
                div()
                    .id(("ssh-config", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .text_size(px(10.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ssh_selected = index;
                        this.remote_path = None;
                        this.remote_entries.clear();
                        this.remote_preview.clear();
                        cx.notify();
                    }))
                    .child(name)
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint)
                            .child(label),
                    ),
            );
        }
        let mut remote_files = div()
            .id("ssh-files")
            .flex_none()
            .max_h(px(160.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if let Some(path) = &self.remote_path {
            let parent = std::path::Path::new(path)
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .filter(|parent| !parent.is_empty());
            if let Some(parent) = parent {
                remote_files = remote_files.child(
                    div()
                        .id("ssh-parent")
                        .px_2()
                        .py_1()
                        .text_size(px(10.))
                        .text_color(theme.muted)
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.browse_remote(Some(parent.clone()), cx);
                        }))
                        .child(".."),
                );
            }
            for (index, entry) in self.remote_entries.iter().take(200).enumerate() {
                let is_dir = entry.is_dir;
                let path = entry.path.clone();
                let name = entry.name.clone();
                let size = entry.size;
                remote_files = remote_files.child(
                    div()
                        .id(("ssh-entry", index))
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(10.))
                        .text_color(if is_dir { theme.text } else { theme.muted })
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if is_dir {
                                this.browse_remote(Some(path.clone()), cx);
                            } else {
                                this.preview_remote_file(path.clone(), cx);
                            }
                        }))
                        .child(if is_dir { "▸" } else { "·" })
                        .child(div().flex_1().truncate().child(name))
                        .when(!is_dir, |row| {
                            row.child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(theme.faint)
                                    .child(size.to_string()),
                            )
                        }),
                );
            }
        }
        let preview_lines: Vec<String> = self
            .remote_preview
            .lines()
            .take(400)
            .map(str::to_owned)
            .collect();
        let remote_preview = div()
            .id("ssh-preview")
            .flex_1()
            .min_h(px(0.))
            .overflow_scroll()
            .p_2()
            .font_family("Menlo")
            .text_size(px(9.5))
            .text_color(theme.text)
            .flex()
            .flex_col()
            .children(preview_lines.into_iter().map(|line| div().child(line)));
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("ssh-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_remote(cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(
                        div()
                            .id("ssh-open")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_ssh_terminal(window, cx);
                            }))
                            .child("Open SSH"),
                    )
                    .child(
                        div()
                            .id("ssh-browse")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.browse_remote(None, cx);
                            }))
                            .child("Browse"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.remote_status.clone()),
                    ),
            )
            .child(rows)
            .child(remote_files)
            .child(remote_preview)
            .into_any_element()
    }

    fn render_notebook(&self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut cells = div()
            .id("notebook-cells")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if let Some(notebook) = &self.notebook {
            for (index, cell) in notebook.cells.iter().take(500).enumerate() {
                let cell_id = cell.id.clone();
                let source = cell.source.clone();
                let mut body = div()
                    .id(("notebook-cell", index))
                    .mx_2()
                    .my_1()
                    .p_2()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.panel_alt)
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(9.))
                                    .text_color(theme.faint)
                                    .child(format!(
                                        "{}{}",
                                        cell.cell_type,
                                        cell.execution_count
                                            .map(|count| format!(" [{count}]"))
                                            .unwrap_or_default()
                                    )),
                            )
                            .child(
                                div()
                                    .id(("notebook-edit", index))
                                    .px_1()
                                    .text_size(px(9.))
                                    .text_color(theme.muted)
                                    .cursor_pointer()
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            this.open_notebook_cell_editor(
                                                cell_id.clone(),
                                                source.clone(),
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .child("Edit"),
                            ),
                    )
                    .child(
                        div()
                            .font_family("Menlo")
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .child(cell.source.clone()),
                    );
                for output in &cell.outputs {
                    let text = notebook_output_text(output);
                    if text.trim().is_empty() {
                        continue;
                    }
                    body = body.child(
                        div()
                            .pt_1()
                            .border_t_1()
                            .border_color(theme.border)
                            .font_family("Menlo")
                            .text_size(px(9.5))
                            .text_color(theme.muted)
                            .child(text),
                    );
                }
                cells = cells.child(body);
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("notebook-run")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.run_notebook(cx);
                            }))
                            .child("Run all"),
                    )
                    .child(
                        div()
                            .id("notebook-save")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.save_notebook(cx);
                            }))
                            .child("Save"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.notebook_status.clone()),
                    ),
            )
            .child(cells)
            .into_any_element()
    }

    fn render_mcp(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("mcp-manage-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, server) in self.mcp_servers.iter().take(100).enumerate() {
            let selected = self.mcp_selected.as_deref() == Some(server.id.as_str());
            let enabled = server.enabled;
            rows = rows.child(
                div()
                    .id(("mcp-manage", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.5))
                    .text_color(if enabled { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select_mcp_server(index, cx);
                    }))
                    .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(if enabled {
                        theme.accent
                    } else {
                        theme.faint
                    }))
                    .child(div().flex_1().truncate().child(server.name.clone()))
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .child(format!("{:?}", server.scope).to_lowercase()),
                    )
                    .child(
                        div()
                            .id(("mcp-manage-toggle", index))
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.toggle_mcp_server(index, cx);
                            }))
                            .child(if enabled { "off" } else { "on" }),
                    ),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("mcp-new")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.new_mcp_server(cx);
                            }))
                            .child("New"),
                    )
                    .child(
                        div()
                            .id("mcp-remove")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.error)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.remove_selected_mcp_server(cx);
                            }))
                            .child("Remove"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.mcp_status.clone()),
                    ),
            )
            .child(rows)
            .child(
                div()
                    .flex_none()
                    .p_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(self.mcp_name_input.clone())
                    .child(self.mcp_command_input.clone())
                    .child(self.mcp_args_input.clone())
                    .child(
                        div()
                            .id("mcp-save")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.save_mcp_server(cx);
                            }))
                            .child(if self.mcp_selected.is_some() {
                                "Update stdio server"
                            } else {
                                "Add stdio server"
                            }),
                    ),
            )
            .into_any_element()
    }

    fn render_agents(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("agent-providers")
            .flex_none()
            .max_h(px(150.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, agent) in self.agents.iter().take(80).enumerate() {
            let selected = index == self.agent_selected;
            rows = rows.child(
                div()
                    .id(("agent", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.5))
                    .text_color(if agent.installed {
                        theme.text
                    } else {
                        theme.muted
                    })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.agent_selected = index;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(6.))
                            .h(px(6.))
                            .rounded_full()
                            .bg(if agent.installed {
                                theme.accent
                            } else {
                                theme.faint
                            }),
                    )
                    .child(div().flex_1().truncate().child(agent.name.clone()))
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(agent.program.clone()),
                    ),
            );
        }
        let mut task_rows = div()
            .id("agent-tasks")
            .flex_none()
            .max_h(px(90.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, task) in self.agent_tasks.iter().take(100).enumerate() {
            let selected = self.agent_selected_task.as_deref() == Some(task.id.as_str());
            let task_id = task.id.clone();
            task_rows = task_rows.child(
                div()
                    .id(("agent-task", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .text_size(px(10.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.agent_selected_task = Some(task_id.clone());
                        this.agent_viewing_subagent = None;
                        cx.notify();
                    }))
                    .child(div().truncate().child(task.title.clone()))
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .child(format!("{:?} · {}", task.status, task.provider)),
                    ),
            );
        }
        let mut mcp_rows = div()
            .id("mcp-servers")
            .flex_none()
            .max_h(px(110.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, server) in self.mcp_servers.iter().take(80).enumerate() {
            let enabled = server.enabled;
            mcp_rows = mcp_rows.child(
                div()
                    .id(("mcp-server", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.))
                    .text_color(if enabled { theme.text } else { theme.muted })
                    .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(if enabled {
                        theme.accent
                    } else {
                        theme.faint
                    }))
                    .child(div().flex_1().truncate().child(server.name.clone()))
                    .child(
                        div()
                            .id(("mcp-toggle", index))
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.toggle_mcp_server(index, cx);
                            }))
                            .child(if enabled { "disable" } else { "enable" }),
                    ),
            );
        }
        let mut timeline = div()
            .id("agent-timeline")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p_2()
            .flex()
            .flex_col()
            .gap(px(4.));
        if self.agent_viewing_subagent.is_some() {
            timeline = timeline.child(
                div()
                    .id("subagent-back")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_size(px(9.5))
                    .text_color(theme.muted)
                    .bg(theme.panel_alt)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.agent_viewing_subagent = None;
                        cx.notify();
                    }))
                    .child("← Back to task"),
            );
        }
        let selected_entries = self.agent_selected_task.as_ref().map(|task_id| {
            if let Some(subagent) = &self.agent_viewing_subagent {
                (task_id.clone(), self.agent_subentries.get(subagent))
            } else {
                (task_id.clone(), self.agent_entries.get(task_id))
            }
        });
        if let Some((task_id, Some(entries))) = selected_entries {
            let task_id = task_id.clone();
            for (_, entry) in entries.iter().take(500) {
                let (role, text) = agent_entry_display(entry);
                if text.trim().is_empty() {
                    continue;
                }
                let mut row = div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(8.5))
                            .text_color(if role == "Error" {
                                theme.error
                            } else {
                                theme.faint
                            })
                            .child(role),
                    )
                    .child(div().text_size(px(10.)).text_color(theme.text).child(text));
                if let ide_agent::AgentEntry::Permission {
                    id,
                    options,
                    questions,
                    form,
                    chosen: None,
                    ..
                } = entry
                {
                    let mut actions = div().flex().flex_wrap().gap_1();
                    let permission = *id;
                    let task = task_id.clone();
                    for option in options {
                        let option_id = option.id.clone();
                        let task = task.clone();
                        actions = actions.child(
                            div()
                                .id(SharedString::from(format!(
                                    "permission-{permission}-{option_id}"
                                )))
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(theme.accent_muted)
                                .text_size(px(9.))
                                .text_color(theme.text)
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.respond_agent_permission(
                                        task.clone(),
                                        permission,
                                        Some(option_id.clone()),
                                        cx,
                                    );
                                }))
                                .child(option.name.clone()),
                        );
                    }
                    let task = task_id.clone();
                    actions = actions.child(
                        div()
                            .id(SharedString::from(format!(
                                "permission-{permission}-cancel"
                            )))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.error)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.respond_agent_permission(task.clone(), permission, None, cx);
                            }))
                            .child("Cancel"),
                    );
                    if !questions.is_empty() || form.is_some() {
                        let task = task_id.clone();
                        actions = actions.child(
                            div()
                                .id(SharedString::from(format!("permission-{permission}-form")))
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(theme.accent_muted)
                                .text_size(px(9.))
                                .text_color(theme.text)
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.open_agent_form_answer(task.clone(), permission, cx);
                                }))
                                .child("Answer form"),
                        );
                    }
                    row = row.child(actions);
                }
                if let ide_agent::AgentEntry::Subagent {
                    id,
                    title,
                    description,
                    ..
                } = entry
                {
                    let id = id.clone();
                    let label = title
                        .clone()
                        .or_else(|| description.clone())
                        .unwrap_or_else(|| "subagent".into());
                    row = row.child(
                        div()
                            .id(SharedString::from(format!("open-subagent-{id}")))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(theme.panel_alt)
                            .text_size(px(9.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.agent_viewing_subagent = Some(id.clone());
                                cx.notify();
                            }))
                            .child(format!("Open subagent · {label}")),
                    );
                }
                timeline = timeline.child(row);
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("agents-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_agents(cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.agents_status.clone()),
                    ),
            )
            .child(
                div()
                    .p_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().flex_1().child(self.agent_prompt_input.clone()))
                    .child(
                        div()
                            .id("agent-send")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.send_agent_followup(cx);
                            }))
                            .child("Send"),
                    )
                    .child(
                        div()
                            .id("agent-new")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.start_agent_task(cx);
                            }))
                            .child("New"),
                    )
                    .child(
                        div()
                            .id("agent-cancel")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.error)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.cancel_selected_agent_task(cx);
                            }))
                            .child("Stop"),
                    ),
            )
            .child(rows)
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.))
                    .text_color(theme.faint)
                    .child(self.agent_task_status.clone()),
            )
            .child(task_rows)
            .child(timeline)
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.))
                    .text_color(theme.faint)
                    .child(self.mcp_status.clone()),
            )
            .child(mcp_rows)
            .when(self.agents.is_empty(), |view| {
                view.child(
                    div()
                        .p_3()
                        .text_size(px(10.5))
                        .text_color(theme.faint)
                        .child("ACP agent providers will appear here."),
                )
            })
            .into_any_element()
    }

    fn render_tools(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("tool-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, tool) in self.tools.iter().take(200).enumerate() {
            let selected = index == self.tools_selected;
            rows = rows.child(
                div()
                    .id(("tool", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .text_size(px(10.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.tools_selected = index;
                        cx.notify();
                    }))
                    .child(tool.display_name.clone())
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(format!(
                                "{:?} · {}",
                                tool.status,
                                tool.version.clone().unwrap_or_default()
                            )),
                    ),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("tools-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_tools(cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(
                        div()
                            .id("tools-install")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.install_selected_tool(cx);
                            }))
                            .child("Install"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.tools_status.clone()),
                    ),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_plugins(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("plugin-list")
            .flex_none()
            .max_h(px(120.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, plugin) in self.plugins.iter().take(200).enumerate() {
            let color = if plugin.error.is_some() {
                theme.error
            } else if plugin.enabled {
                theme.text
            } else {
                theme.muted
            };
            rows =
                rows.child(
                    div()
                        .id(("plugin", index))
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(10.5))
                        .text_color(color)
                        .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(
                            if plugin.error.is_some() {
                                theme.error
                            } else if plugin.enabled {
                                theme.accent
                            } else {
                                theme.faint
                            },
                        ))
                        .child(div().flex_1().truncate().child(plugin.name.clone()))
                        .child(
                            div()
                                .text_size(px(9.))
                                .text_color(theme.faint)
                                .child(plugin.version.clone()),
                        ),
                );
        }
        let mut market_rows = div()
            .id("market-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, plugin) in self.market_plugins.iter().take(200).enumerate() {
            let installed = plugin.installed.as_deref();
            let compatible = plugin.compatible;
            let selected = index == self.market_selected;
            market_rows = market_rows.child(
                div()
                    .id(("market-plugin", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .text_size(px(10.5))
                    .text_color(if compatible { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.market_selected = index;
                        this.plugin_preview = None;
                        this.plugin_preview_status = "Preview the selected plugin".into();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().truncate().child(plugin.entry.name.clone()))
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(theme.faint)
                                    .child(plugin.entry.version.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(if installed.is_some() {
                                theme.accent
                            } else {
                                theme.faint
                            })
                            .truncate()
                            .child(format!(
                                "{}{}{}",
                                installed
                                    .map(|version| format!("installed {version} · "))
                                    .unwrap_or_default(),
                                plugin.entry.publisher,
                                plugin
                                    .publisher_name
                                    .as_ref()
                                    .map(|name| format!(" ({name})"))
                                    .unwrap_or_default()
                            )),
                    ),
            );
        }
        let mut preview_panel = div()
            .id("plugin-preview")
            .flex_none()
            .max_h(px(180.))
            .overflow_y_scroll()
            .p_2()
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(9.5))
                    .text_color(theme.faint)
                    .child(self.plugin_preview_status.clone()),
            );
        if let Some(preview) = &self.plugin_preview {
            if !preview.description.is_empty() {
                preview_panel = preview_panel.child(
                    div()
                        .text_size(px(10.))
                        .text_color(theme.muted)
                        .child(preview.description.clone()),
                );
            }
            for capability in &preview.capabilities {
                preview_panel =
                    preview_panel.child(div().text_size(px(9.)).text_color(theme.faint).child(
                        format!(
                            "{} · {:?}{}",
                            capability.id,
                            capability.risk,
                            if capability.optional {
                                " (optional)"
                            } else {
                                ""
                            }
                        ),
                    ));
            }
            for warning in &preview.warnings {
                preview_panel = preview_panel.child(
                    div()
                        .text_size(px(9.))
                        .text_color(theme.warning)
                        .child(warning.clone()),
                );
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("plugins-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_plugins(cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(
                        div()
                            .id("plugins-market")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_market(cx);
                            }))
                            .child("Market"),
                    )
                    .child(
                        div()
                            .id("plugins-preview")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.preview_market_plugin(cx);
                            }))
                            .child("Preview"),
                    )
                    .child(
                        div()
                            .id("plugins-install")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.install_previewed_plugin(cx);
                            }))
                            .child("Install"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.plugins_status.clone()),
                    ),
            )
            .child(rows)
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(9.5))
                    .text_color(theme.faint)
                    .border_t_1()
                    .border_color(theme.border)
                    .child(self.market_status.clone()),
            )
            .child(market_rows)
            .child(preview_panel)
            .when(self.plugins.is_empty(), |view| {
                view.child(
                    div()
                        .p_3()
                        .text_size(px(10.5))
                        .text_color(theme.faint)
                        .child("Installed OpenIDEA plugins will appear here."),
                )
            })
            .into_any_element()
    }

    fn render_placeholder(&self, title: &'static str) -> gpui::AnyElement {
        let theme = self.theme;
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .p_4()
            .child(div().text_size(px(12.)).text_color(theme.text).child(title))
            .child(
                div()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child("Backend integration for this panel is not implemented yet."),
            )
            .into_any_element()
    }

    fn render_git_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let branch = self
            .git_branch
            .as_ref()
            .map(|branch| {
                let head = branch.head.as_deref().unwrap_or("detached");
                format!(
                    "{head}{}{}",
                    if branch.ahead > 0 {
                        format!(" ↑{}", branch.ahead)
                    } else {
                        String::new()
                    },
                    if branch.behind > 0 {
                        format!(" ↓{}", branch.behind)
                    } else {
                        String::new()
                    }
                )
            })
            .unwrap_or_else(|| "No Git repository".into());
        let mut files = div()
            .id("git-files")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, file) in self.git_status.iter().take(200).enumerate() {
            let path = file.path.0.clone();
            let open_app = cx.entity();
            let letter = change_letter(file.unstaged.or(file.staged));
            let color = if file.unstaged.is_some() {
                theme.warning
            } else {
                theme.accent
            };
            files = files.child(
                div()
                    .id(("git-file", index))
                    .h(px(22.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.5))
                    .text_color(theme.text)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(move |_: &ClickEvent, _, cx| {
                        open_app.update(cx, |app, cx| {
                            app.open_path(path.clone(), cx);
                        });
                    })
                    .child(div().w(px(12.)).text_color(color).child(letter))
                    .child(div().flex_1().truncate().child(file.rel_path.clone())),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_3()
                    .flex()
                    .items_center()
                    .text_size(px(11.5))
                    .text_color(theme.accent)
                    .child(branch),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .border_y_1()
                    .border_color(theme.border)
                    .child(self.git_message_input.clone()),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(10.5))
                    .text_color(theme.muted)
                    .child(self.git_status_message.clone())
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("git-stage-all")
                            .px_2()
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.stage_all(cx);
                            }))
                            .child("Stage all"),
                    )
                    .child(
                        div()
                            .id("git-unstage-all")
                            .px_2()
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.unstage_all(cx);
                            }))
                            .child("Unstage all"),
                    ),
            )
            .child(files)
            .into_any_element()
    }

    fn render_search_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut results = div()
            .id("search-results")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (file_index, file) in self.search_results.iter().take(80).enumerate() {
            let path = file.path.0.clone();
            let open_app = cx.entity();
            results = results.child(
                div()
                    .id(("search-file", file_index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.5))
                    .text_color(theme.muted)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                    .on_click(move |_: &ClickEvent, _, cx| {
                        open_app.update(cx, |app, cx| {
                            app.open_path(path.clone(), cx);
                        });
                    })
                    .child(div().flex_1().truncate().child(file.rel_path.clone()))
                    .child(
                        div()
                            .text_color(theme.faint)
                            .child(file.matches.len().to_string()),
                    ),
            );
            for (match_index, line) in file.matches.iter().take(20).enumerate() {
                let path = file.path.0.clone();
                let open_app = cx.entity();
                let line_number = line.line + 1;
                results = results.child(
                    div()
                        .id(("search-match", file_index * 100 + match_index))
                        .h(px(22.))
                        .pl_5()
                        .pr_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.5))
                        .text_color(theme.text)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(move |_: &ClickEvent, _, cx| {
                            open_app.update(cx, |app, cx| {
                                app.open_path(path.clone(), cx);
                            });
                        })
                        .child(
                            div()
                                .w(px(34.))
                                .text_color(theme.faint)
                                .child(line_number.to_string()),
                        )
                        .child(div().flex_1().truncate().child(line.text.clone())),
                );
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(34.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.search_input.clone()),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(self.search_status.clone()),
            )
            .child(results)
            .into_any_element()
    }

    fn render_bottom_panel(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let running = self.running_run.is_some();
        let mut configs = Vec::new();
        for (index, config) in self.run_configs.iter().take(6).enumerate() {
            let selected = index == self.selected_run;
            configs.push(
                div()
                    .id(("run-config", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_size(px(10.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.titlebar
                    })
                    .cursor_pointer()
                    .hover(|style| style.text_color(theme.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.selected_run = index;
                        cx.notify();
                    }))
                    .child(config.name.clone()),
            );
        }
        let mut lines: Vec<String> = self.run_output.lines().map(str::to_owned).collect();
        if lines.len() > 200 {
            lines.drain(..lines.len() - 200);
        }
        let mut output = div()
            .id("run-output")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p_2()
            .font_family("Menlo")
            .text_size(px(11.5))
            .text_color(theme.text)
            .flex()
            .flex_col();
        if lines.is_empty() {
            output = output.child(
                div()
                    .text_color(theme.faint)
                    .child("Run output appears here."),
            );
        } else {
            output = output.children(lines.into_iter().map(|line| div().child(line)));
        }

        let run_body = div().size_full().flex().flex_col().child(output);
        let terminal_body = match &self.terminal {
            Some(terminal) => div().size_full().child(terminal.clone()).into_any_element(),
            None => div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .text_size(px(11.))
                .text_color(theme.faint)
                .child(self.terminal_status.clone())
                .child(
                    div()
                        .id("start-terminal")
                        .px_3()
                        .py_1()
                        .rounded_sm()
                        .bg(theme.accent_muted)
                        .text_color(theme.text)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.ensure_terminal(window, cx);
                        }))
                        .child("Start shell"),
                )
                .into_any_element(),
        };
        let mut build_lines: Vec<String> = self.build_output.lines().map(str::to_owned).collect();
        if build_lines.len() > 400 {
            build_lines.drain(..build_lines.len() - 400);
        }
        let build_body = div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("build-output")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_scroll()
                    .p_2()
                    .font_family("Menlo")
                    .text_size(px(10.5))
                    .text_color(theme.text)
                    .flex()
                    .flex_col()
                    .when(build_lines.is_empty(), |view| {
                        view.child(
                            div()
                                .text_color(theme.faint)
                                .child("Build output appears here."),
                        )
                    })
                    .children(build_lines.into_iter().map(|line| div().child(line))),
            )
            .into_any_element();
        let mut debug_rows = div()
            .id("debug-attachables")
            .size_full()
            .overflow_scroll()
            .p_2()
            .flex()
            .flex_col();
        for (index, process) in self.debug_attachables.iter().take(200).enumerate() {
            let selected = index == self.debug_selected;
            let label = format!(
                "{} · {} · pid {}",
                process.name, process.adapter, process.pid
            );
            debug_rows = debug_rows.child(
                div()
                    .id(("debug-process", index))
                    .h(px(22.))
                    .px_2()
                    .flex()
                    .items_center()
                    .text_size(px(10.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.debug_selected = index;
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        let debug_body = if self.debug_attachables.is_empty() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.5))
                .text_color(theme.faint)
                .child(self.debug_status.clone())
                .into_any_element()
        } else {
            debug_rows.into_any_element()
        };
        let body = match self.bottom_mode {
            BottomMode::Run => run_body.into_any_element(),
            BottomMode::Terminal => terminal_body,
            BottomMode::Build => build_body,
            BottomMode::Debug => debug_body,
        };
        let run_tab_active = self.bottom_mode == BottomMode::Run;
        let terminal_tab_active = self.bottom_mode == BottomMode::Terminal;
        let build_tab_active = self.bottom_mode == BottomMode::Build;
        let debug_tab_active = self.bottom_mode == BottomMode::Debug;
        div()
            .h(px(240.))
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("bottom-run")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.5))
                            .text_color(if run_tab_active {
                                theme.text
                            } else {
                                theme.muted
                            })
                            .bg(if run_tab_active {
                                theme.panel_alt
                            } else {
                                theme.panel
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.bottom_mode = BottomMode::Run;
                                cx.notify();
                            }))
                            .child("RUN"),
                    )
                    .child(
                        div()
                            .id("bottom-terminal")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.5))
                            .text_color(if terminal_tab_active {
                                theme.text
                            } else {
                                theme.muted
                            })
                            .bg(if terminal_tab_active {
                                theme.panel_alt
                            } else {
                                theme.panel
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.bottom_mode = BottomMode::Terminal;
                                this.ensure_terminal(window, cx);
                                cx.notify();
                            }))
                            .child("TERMINAL"),
                    )
                    .child(
                        div()
                            .id("bottom-build")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.5))
                            .text_color(if build_tab_active {
                                theme.text
                            } else {
                                theme.muted
                            })
                            .bg(if build_tab_active {
                                theme.panel_alt
                            } else {
                                theme.panel
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.bottom_mode = BottomMode::Build;
                                if this.build_model.is_none() {
                                    this.refresh_build(cx);
                                }
                                cx.notify();
                            }))
                            .child("BUILD"),
                    )
                    .child(
                        div()
                            .id("bottom-debug")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.5))
                            .text_color(if debug_tab_active {
                                theme.text
                            } else {
                                theme.muted
                            })
                            .bg(if debug_tab_active {
                                theme.panel_alt
                            } else {
                                theme.panel
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.bottom_mode = BottomMode::Debug;
                                if this.debug_attachables.is_empty() {
                                    this.refresh_debug(cx);
                                }
                                cx.notify();
                            }))
                            .child("DEBUG"),
                    )
                    .children(if run_tab_active { configs } else { Vec::new() })
                    .child(div().flex_1())
                    .when(run_tab_active, |row| {
                        row.child(
                            div()
                                .id("start-run")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.text)
                                .bg(theme.accent_muted)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.start_selected_run(cx);
                                }))
                                .child("Run"),
                        )
                    })
                    .when(run_tab_active && running, |row| {
                        row.child(
                            div()
                                .id("stop-run")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.error)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.stop_active_run(cx);
                                }))
                                .child("Stop"),
                        )
                    })
                    .when(build_tab_active, |row| {
                        row.child(
                            div()
                                .id("build-project")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.text)
                                .bg(theme.accent_muted)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.run_build(ide_build::compile::BuildKind::Build, cx);
                                }))
                                .child("Build"),
                        )
                        .child(
                            div()
                                .id("rebuild-project")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.text)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.run_build(ide_build::compile::BuildKind::Rebuild, cx);
                                }))
                                .child("Rebuild"),
                        )
                    })
                    .when(debug_tab_active, |row| {
                        row.child(
                            div()
                                .id("debug-refresh")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.muted)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.refresh_debug(cx);
                                }))
                                .child("Refresh"),
                        )
                    })
                    .child(
                        div()
                            .px_2()
                            .text_size(px(10.5))
                            .text_color(theme.muted)
                            .child(if run_tab_active {
                                self.run_status.clone()
                            } else if terminal_tab_active {
                                self.terminal_status.clone()
                            } else if build_tab_active {
                                self.build_status.clone()
                            } else {
                                self.debug_status.clone()
                            }),
                    ),
            )
            .child(div().flex_1().min_h(px(0.)).child(body))
            .into_any_element()
    }

    fn toggle_tree_row(
        &mut self,
        index: usize,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let toggle = self.tree.toggle(index);
        match toggle {
            Toggle::OpenFile(path) => self.open_path(path, cx),
            Toggle::LoadDirectory(path) => self.load_directory(path, cx),
            Toggle::Symlink => {
                self.status = "Symlinked directories are not expanded".into();
            }
            Toggle::Rebuilt | Toggle::Missing => {}
        }
        if let Some(window) = window
            && let Some(doc) = self.active
            && let Some(editor) = self.editors.get(&(doc, false))
        {
            window.focus(&editor.read(cx).focus_handle(cx), cx);
        }
        cx.notify();
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut focus_primary = false;
        if let Some(doc) = self.active {
            focus_primary = !self.editors.contains_key(&(doc, false));
            self.ensure_editor(doc, false, window, cx);
            if self.split {
                self.ensure_editor(doc, true, window, cx);
            }
        }
        if focus_primary
            && let Some(doc) = self.active
            && let Some(editor) = self.editors.get(&(doc, false))
        {
            let focus = editor.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        }
        if let Some((path, line, column)) = self.pending_goto.clone()
            && let Some((doc, _)) = self
                .documents
                .iter()
                .find(|(_, document)| document.path == path)
        {
            let doc = *doc;
            self.pending_goto = None;
            self.goto_position(doc, line, column, window, cx);
        }
        if let Some(command) = self.pending_command.take() {
            self.execute_command(command, window, cx);
        }
        let active_editor = self
            .active
            .and_then(|doc| self.editors.get(&(doc, false)).cloned());
        let split_editor = self
            .active
            .filter(|_| self.split)
            .and_then(|doc| self.editors.get(&(doc, true)).cloned());
        let project = self.render_project(cx).into_any_element();
        let search = self.render_search_panel(cx);
        let git = self.render_git_panel(cx);
        let outline = self.render_outline(window, cx);
        let problems = self.render_problems(window, cx);
        let settings = self.render_settings(cx);
        let sidebar_body = match self.sidebar_mode {
            SidebarMode::Project => project,
            SidebarMode::Search => search,
            SidebarMode::Git => git,
            SidebarMode::Outline => outline,
            SidebarMode::Problems => problems,
            SidebarMode::Settings => settings,
            SidebarMode::Database => self.render_db(cx),
            SidebarMode::Http => self.render_http(cx),
            SidebarMode::Remote => self.render_remote(cx),
            SidebarMode::Notebook => self.render_notebook(window, cx),
            SidebarMode::Mcp => self.render_mcp(cx),
            SidebarMode::Agents => self.render_agents(cx),
            SidebarMode::Plugins => self.render_plugins(cx),
            SidebarMode::Tools => self.render_tools(cx),
        };
        let sidebar_tabs = [
            ("PRJ", SidebarMode::Project),
            ("SRC", SidebarMode::Search),
            ("GIT", SidebarMode::Git),
            ("OUT", SidebarMode::Outline),
            ("PRB", SidebarMode::Problems),
            ("DB", SidebarMode::Database),
            ("HTTP", SidebarMode::Http),
            ("SSH", SidebarMode::Remote),
            ("NB", SidebarMode::Notebook),
            ("MCP", SidebarMode::Mcp),
            ("AGT", SidebarMode::Agents),
            ("PLG", SidebarMode::Plugins),
            ("TLS", SidebarMode::Tools),
            ("SET", SidebarMode::Settings),
        ];
        let mut sidebar_buttons = Vec::new();
        for (index, (label, mode)) in sidebar_tabs.into_iter().enumerate() {
            let active = self.sidebar_mode == mode;
            sidebar_buttons.push(
                div()
                    .id(("sidebar-tab", index))
                    .px(px(6.))
                    .py(px(3.))
                    .rounded_sm()
                    .text_size(px(9.5))
                    .cursor_pointer()
                    .text_color(if active { theme.text } else { theme.muted })
                    .bg(if active { theme.panel_alt } else { theme.panel })
                    .hover(|style| style.text_color(theme.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select_sidebar(mode, cx);
                    }))
                    .child(label)
                    .into_any_element(),
            );
        }
        let tabs = self.render_tabs(cx).into_any_element();
        let run_panel = self.render_bottom_panel(window, cx);
        let quick_open = self.render_quick_open(cx);
        let command_palette = self.render_command_palette(cx);
        let symbol_search = self.render_symbol_search(cx);
        let setting_editor = self.render_setting_editor(cx);
        let agent_form_answer = self.render_agent_form_answer(cx);
        let notebook_editor = self.render_notebook_editor(cx);
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(theme.window)
            .text_color(theme.text)
            .on_action(cx.listener(|this, _: &crate::SaveDocument, _, cx| {
                if let Some(doc) = this.active {
                    this.save_document(doc, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &crate::CloseTab, _, cx| {
                if let Some(doc) = this.active {
                    this.close_tab(doc, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &crate::ToggleSplit, _, cx| {
                this.toggle_split(cx);
            }))
            .on_action(cx.listener(|this, _: &crate::ToggleQuickOpen, window, cx| {
                this.toggle_quick_open(window, cx);
            }))
            .on_action(
                cx.listener(|this, _: &crate::ToggleCommandPalette, window, cx| {
                    this.toggle_command_palette(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::ToggleSymbolSearch, window, cx| {
                    this.toggle_symbol_search(window, cx);
                }),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .child(
                        div()
                            .w(px(250.))
                            .h_full()
                            .flex_none()
                            .bg(theme.panel)
                            .border_r_1()
                            .border_color(theme.border)
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(px(3.))
                                    .children(sidebar_buttons),
                            )
                            .child(div().flex_1().min_h(px(0.)).child(sidebar_body)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .child(tabs)
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .flex()
                                    .child(
                                        div().flex_1().min_w(px(0.)).h_full().child(
                                            match active_editor {
                                                Some(editor) => editor.into_any_element(),
                                                None => div()
                                                    .size_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(theme.faint)
                                                    .child("Open a file from the project tree")
                                                    .into_any_element(),
                                            },
                                        ),
                                    )
                                    .when_some(split_editor, |row, editor| {
                                        row.child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .h_full()
                                                .border_l_1()
                                                .border_color(theme.border)
                                                .child(editor),
                                        )
                                    }),
                            ),
                    ),
            )
            .child(run_panel)
            .child(
                div()
                    .h(px(24.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .bg(theme.titlebar)
                    .border_t_1()
                    .border_color(theme.border)
                    .text_size(px(11.))
                    .text_color(theme.muted)
                    .child(self.status.clone())
                    .child(div().flex_1())
                    .child(format!(
                        "{} tabs · {} diagnostics",
                        self.order.len(),
                        self.diagnostics.values().sum::<usize>()
                    )),
            )
            .child(quick_open)
            .child(command_palette)
            .child(symbol_search)
            .child(setting_editor)
            .child(agent_form_answer)
            .child(notebook_editor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_prefix_counts_astral_characters() {
        assert_eq!(utf16_prefix("a😀b", 1), "a");
        assert_eq!(utf16_prefix("a😀b", 3), "a😀");
        assert_eq!(utf16_prefix("a😀b", 4), "a😀b");
    }

    #[test]
    fn append_agent_entry_replaces_the_reported_suffix() {
        let mut entry = ide_agent::AgentEntry::Agent {
            text: "hello".into(),
        };
        append_agent_entry_text(&mut entry, 2, "llo world");
        let ide_agent::AgentEntry::Agent { text } = entry else {
            panic!("expected agent entry");
        };
        assert_eq!(text, "hello world");
    }

    #[test]
    fn error_entries_are_displayed_as_errors() {
        let (role, text) = agent_entry_display(&ide_agent::AgentEntry::Error {
            message: "boom".into(),
        });
        assert_eq!(role, "Error");
        assert_eq!(text, "boom");
    }

    #[test]
    fn command_filter_matches_label_words_case_insensitively() {
        assert!(Command::ShowProblems.matches("problems"));
        assert!(Command::ShowProblems.matches("show pro"));
        assert!(!Command::ShowProblems.matches("market"));
    }
}
