use std::collections::{BTreeMap, BTreeSet, HashMap};
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
    Todo,
    Dependencies,
    Coverage,
    Bookmarks,
    Git,
    GitLog,
    Services,
    GitHub,
    Heap,
    Profiler,
    Logcat,
    Scratch,
    Recent,
    Analysis,
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
enum DockerTab {
    Containers,
    Images,
    Volumes,
    Networks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GitHubTab {
    Overview,
    Files,
    Diff,
    Checks,
    Comments,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnalysisTab {
    Duplicates,
    Structural,
    Inspect,
}

#[derive(Clone, Debug)]
enum LogcatEvent {
    Line(crate::logcat::LogcatLine),
    Finished(Result<(), String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Save,
    SaveAs,
    CloseTab,
    ToggleSplit,
    QuickOpen,
    GoToSymbol,
    FindUsages,
    ShowProject,
    ShowSearch,
    ShowTodo,
    ShowDependencies,
    ShowCoverage,
    ShowBookmarks,
    ShowGit,
    ShowGitLog,
    ShowServices,
    ShowGitHub,
    ShowHeap,
    ShowProfiler,
    ShowLogcat,
    ShowScratches,
    NewScratch,
    ShowRecent,
    ShowAnalysis,
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
    RefreshTodo,
    RefreshDependencies,
}

impl Command {
    const ALL: [Command; 51] = [
        Command::Save,
        Command::SaveAs,
        Command::CloseTab,
        Command::ToggleSplit,
        Command::QuickOpen,
        Command::GoToSymbol,
        Command::FindUsages,
        Command::ShowProject,
        Command::ShowSearch,
        Command::ShowTodo,
        Command::ShowDependencies,
        Command::ShowCoverage,
        Command::ShowBookmarks,
        Command::ShowGit,
        Command::ShowGitLog,
        Command::ShowServices,
        Command::ShowGitHub,
        Command::ShowHeap,
        Command::ShowProfiler,
        Command::ShowLogcat,
        Command::ShowScratches,
        Command::NewScratch,
        Command::ShowRecent,
        Command::ShowAnalysis,
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
        Command::RefreshTodo,
        Command::RefreshDependencies,
    ];

    fn label(self) -> &'static str {
        match self {
            Command::Save => "Save file",
            Command::SaveAs => "Save a copy…",
            Command::CloseTab => "Close tab",
            Command::ToggleSplit => "Toggle split editor",
            Command::QuickOpen => "Quick open file",
            Command::GoToSymbol => "Go to symbol",
            Command::FindUsages => "Find usages at cursor",
            Command::ShowProject => "Show Project",
            Command::ShowSearch => "Show Find in Files",
            Command::ShowTodo => "Show TODO",
            Command::ShowDependencies => "Show Dependencies",
            Command::ShowCoverage => "Show Coverage",
            Command::ShowBookmarks => "Show Bookmarks",
            Command::ShowGit => "Show Version Control",
            Command::ShowGitLog => "Show Git Log",
            Command::ShowServices => "Show Services",
            Command::ShowGitHub => "Show GitHub Pull Requests",
            Command::ShowHeap => "Show Heap Snapshot",
            Command::ShowProfiler => "Show CPU Profile",
            Command::ShowLogcat => "Show Logcat",
            Command::ShowScratches => "Show Scratch Files",
            Command::NewScratch => "New Scratch File",
            Command::ShowRecent => "Show Recent Files",
            Command::ShowAnalysis => "Show Code Analysis",
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
            Command::RefreshTodo => "Refresh TODO items",
            Command::RefreshDependencies => "Refresh build dependencies",
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

fn mcp_transport_label(transport: ide_agent::McpTransport) -> &'static str {
    match transport {
        ide_agent::McpTransport::Stdio => "stdio",
        ide_agent::McpTransport::Http => "http",
        ide_agent::McpTransport::Sse => "sse",
    }
}

fn build_mcp_server_input(
    id: Option<String>,
    name: &str,
    transport: ide_agent::McpTransport,
    command: &str,
    args: &str,
    url: &str,
) -> Result<ide_agent::McpServerInput, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("MCP server name is required".into());
    }
    let is_stdio = transport == ide_agent::McpTransport::Stdio;
    let command = command.trim();
    if is_stdio && command.is_empty() {
        return Err("An MCP stdio server requires a command".into());
    }
    let url = url.trim();
    if !is_stdio && url.is_empty() {
        return Err(format!(
            "An MCP {} server requires a URL",
            mcp_transport_label(transport)
        ));
    }
    Ok(ide_agent::McpServerInput {
        id,
        name: name.to_owned(),
        scope: ide_agent::McpScope::User,
        transport,
        enabled: true,
        command: is_stdio.then(|| command.to_owned()),
        args: if is_stdio {
            args.split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        env: Vec::new(),
        url: (!is_stdio).then(|| url.to_owned()),
        headers: Vec::new(),
        agents: Vec::new(),
    })
}

fn search_replace_targets(results: &[ide_search::FileMatches]) -> Vec<ide_search::ReplaceTarget> {
    results
        .iter()
        .filter_map(|file| {
            let mut positions = BTreeSet::new();
            for line in &file.matches {
                for &(start, _) in &line.ranges {
                    positions.insert(ide_core::Pos::new(
                        line.line,
                        line.text_offset.saturating_add(start),
                    ));
                }
            }
            (!positions.is_empty()).then(|| ide_search::ReplaceTarget {
                path: file.path.clone(),
                only: Some(positions.into_iter().collect()),
            })
        })
        .collect()
}

fn build_search_query(
    pattern: String,
    regex: bool,
    case_sensitive: bool,
    whole_word: bool,
) -> ide_search::SearchQuery {
    ide_search::SearchQuery {
        pattern,
        is_regex: regex,
        case_sensitive,
        whole_word,
        include: Vec::new(),
        exclude: Vec::new(),
        scope: ide_search::SearchScope::Project,
        max_matches: 10_000,
        context: ide_search::SearchContext::Any,
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn short_log_time(time: &str) -> &str {
    if time.len() > 12 {
        &time[time.len() - 12..]
    } else {
        time
    }
}

fn format_age(modified_ms: u64) -> String {
    if modified_ms == 0 {
        return "unknown".into();
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(modified_ms);
    let seconds = now_ms.saturating_sub(modified_ms) / 1_000;
    if seconds < 60 {
        format!("{seconds}s ago")
    } else if seconds < 3_600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h ago", seconds / 3_600)
    } else {
        format!("{}d ago", seconds / 86_400)
    }
}

fn heap_edge_label(edge: &ide_heap::HeapEdge) -> String {
    use ide_heap::HeapEdge;
    match edge {
        HeapEdge::Field { name } if !name.is_empty() => format!("field {name}"),
        HeapEdge::Field { .. } => "field".into(),
        HeapEdge::Element { index } => format!("element [{index}]"),
        HeapEdge::Static { name } => format!("static {name}"),
        HeapEdge::ClassOf => "class of".into(),
        HeapEdge::SuperClass => "superclass".into(),
        HeapEdge::ClassLoader => "class loader".into(),
        HeapEdge::Root { root, thread } => format!(
            "GC root {root:?}{}",
            thread
                .as_ref()
                .map(|thread| format!(" ({thread})"))
                .unwrap_or_default()
        ),
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
    save_as: bool,
    save_as_input: Entity<TextInput>,
    _save_as_subscription: Subscription,
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
    search_replace_input: Entity<TextInput>,
    search_replacement: String,
    _search_replace_subscription: Subscription,
    search_preserve_case: bool,
    search_replacing: bool,
    search_regex: bool,
    search_case_sensitive: bool,
    search_whole_word: bool,
    todo_items: Vec<crate::todo::TodoItem>,
    todo_status: gpui::SharedString,
    todo_case_sensitive: bool,
    todo_comments_only: bool,
    todo_generation: u64,
    dependency_modules: Vec<crate::build_tools::BuildModuleRow>,
    dependency_coordinates: Vec<ide_build::DependencyCoordinate>,
    dependency_status: gpui::SharedString,
    dependency_filter_input: Entity<TextInput>,
    dependency_filter: String,
    dependency_show_modules: bool,
    _dependency_filter_subscription: Subscription,
    coverage_suite: Option<ide_coverage::CoverageSuite>,
    coverage_rows: Vec<crate::coverage::CoverageRow>,
    coverage_status: gpui::SharedString,
    coverage_path_input: Entity<TextInput>,
    coverage_sort_lines: bool,
    _coverage_input_subscription: Subscription,
    bookmarks: Vec<crate::bookmarks::Bookmark>,
    bookmark_store: crate::bookmarks::BookmarkStore,
    bookmarks_file: PathBuf,
    bookmarks_status: gpui::SharedString,
    git_status: Vec<ide_vcs::FileStatus>,
    git_branch: Option<ide_vcs::BranchInfo>,
    git_message_input: Entity<TextInput>,
    git_status_message: gpui::SharedString,
    _git_subscription: Subscription,
    git_commits: Vec<ide_vcs::Commit>,
    git_log_status: gpui::SharedString,
    git_log_filter_input: Entity<TextInput>,
    git_log_filter: String,
    _git_log_filter_subscription: Subscription,
    docker_snapshot: Option<ide_docker::DockerSnapshot>,
    docker_status: gpui::SharedString,
    docker_tab: DockerTab,
    docker_busy: bool,
    github_availability: Option<ide_github::GhAvailability>,
    github_pulls: Vec<ide_github::PullRequestSummary>,
    github_status: gpui::SharedString,
    github_state: ide_github::PrStateFilter,
    github_busy: bool,
    github_selected: Option<u32>,
    github_details: Option<ide_github::PullRequestDetails>,
    github_comments: Vec<ide_github::PrReviewComment>,
    github_detail_status: gpui::SharedString,
    github_tab: GitHubTab,
    github_comment_input: Entity<TextInput>,
    github_review_status: gpui::SharedString,
    github_review_busy: bool,
    _github_comment_subscription: Subscription,
    github_diff_files: Vec<crate::pr_diff::DiffFile>,
    github_diff_status: gpui::SharedString,
    github_diff_busy: bool,
    heap_info: Option<ide_host::ops::heap::HeapSnapshotInfo>,
    heap_classes: Vec<ide_host::ops::heap::HeapClassRow>,
    heap_path_input: Entity<TextInput>,
    heap_status: gpui::SharedString,
    heap_busy: bool,
    _heap_input_subscription: Subscription,
    heap_selected_class: Option<u32>,
    heap_class_name: Option<String>,
    heap_objects: Vec<ide_heap::HeapObjectRow>,
    heap_objects_total: u32,
    heap_objects_status: gpui::SharedString,
    heap_path_text: String,
    profile: Option<ide_profiler::Profile>,
    profile_dir_input: Entity<TextInput>,
    profile_status: gpui::SharedString,
    profile_busy: bool,
    _profile_input_subscription: Subscription,
    logcat_devices: Vec<crate::logcat::LogcatDevice>,
    logcat_lines: Vec<crate::logcat::LogcatLine>,
    logcat_status: gpui::SharedString,
    logcat_device_input: Entity<TextInput>,
    logcat_filter_input: Entity<TextInput>,
    logcat_filter: String,
    logcat_level: crate::logcat::LogLevel,
    logcat_cancel: Option<ide_core::CancellationToken>,
    logcat_busy: bool,
    _logcat_device_subscription: Subscription,
    _logcat_filter_subscription: Subscription,
    scratch_dir: PathBuf,
    scratch_files: Vec<crate::scratch::ScratchFile>,
    scratch_name_input: Entity<TextInput>,
    scratch_status: gpui::SharedString,
    _scratch_input_subscription: Subscription,
    recent_store: crate::recent::RecentStore,
    recent_file_path: PathBuf,
    recent_files: Vec<PathBuf>,
    recent_status: gpui::SharedString,
    duplicates_report: Option<ide_analysis::DuplicatesReport>,
    duplicates_status: gpui::SharedString,
    duplicates_busy: bool,
    duplicates_anonymize_identifiers: bool,
    duplicates_anonymize_literals: bool,
    analysis_tab: AnalysisTab,
    structural_pattern_input: Entity<TextInput>,
    structural_replacement_input: Entity<TextInput>,
    structural_language_input: Entity<TextInput>,
    structural_files: Vec<ide_analysis::StructuralFile>,
    structural_status: gpui::SharedString,
    structural_apply_status: gpui::SharedString,
    structural_busy: bool,
    _structural_subscriptions: Vec<Subscription>,
    inspect_files: Vec<ide_host::ops::analysis::InspectedFile>,
    inspect_summary: Option<ide_host::ops::analysis::InspectSummary>,
    inspect_status: gpui::SharedString,
    inspect_busy: bool,
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
    remote_preview_path: Option<String>,
    remote_editor: Option<Entity<Editor>>,
    remote_edit_buffer: Option<Entity<Buffer>>,
    remote_edit_status: gpui::SharedString,
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
    mcp_transport: ide_agent::McpTransport,
    mcp_name_input: Entity<TextInput>,
    mcp_command_input: Entity<TextInput>,
    mcp_args_input: Entity<TextInput>,
    mcp_url_input: Entity<TextInput>,
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
    references_open: bool,
    references: Vec<ide_core::Location>,
    references_selected: usize,
    references_status: gpui::SharedString,
    plugins: Vec<ide_host::plugins::PluginInfo>,
    plugins_status: gpui::SharedString,
    market_plugins: Vec<ide_host::plugins::MarketPlugin>,
    market_status: gpui::SharedString,
    market_loading: bool,
    market_selected: usize,
    plugin_preview: Option<ide_host::plugins::InstallPreview>,
    plugin_consent_optional: BTreeSet<String>,
    plugin_preview_status: gpui::SharedString,
    plugin_install_busy: bool,
    debug_attachables: Vec<ide_dap::attach::AttachableProcess>,
    debug_selected: usize,
    debug_sessions: Vec<ide_dap::SessionInfo>,
    debug_session_selected: usize,
    debug_console: String,
    debug_breakpoints: BTreeMap<PathBuf, BTreeSet<u32>>,
    debug_breakpoint_states: Vec<ide_dap::BreakpointState>,
    debug_active_session: Option<ide_core::DebugSessionId>,
    debug_thread_id: Option<i64>,
    debug_frames: Vec<ide_dap::StackFrame>,
    debug_frame_selected: usize,
    debug_scopes: Vec<ide_dap::Scope>,
    debug_variables: Vec<ide_dap::Variable>,
    debug_eval_input: Entity<TextInput>,
    _debug_eval_subscription: Subscription,
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
        let search_replace_input = cx.new(|cx| {
            TextInput::new(
                "Replace with… (Enter replaces every match)",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let search_replace_subscription =
            cx.subscribe(&search_replace_input, |app, _, event, cx| match event {
                TextInputEvent::Submit(_) => app.replace_search_results(cx),
                TextInputEvent::Change(replacement) => {
                    app.search_replacement = replacement.clone();
                }
                _ => {}
            });
        let save_as_input = cx.new(|cx| {
            TextInput::new(
                "Save as path… (relative to the project root)",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let save_as_subscription = cx.subscribe(&save_as_input, |app, _, event, cx| match event {
            TextInputEvent::Submit(_) => app.perform_save_as(cx),
            TextInputEvent::Cancel => {
                app.save_as = false;
                cx.notify();
            }
            _ => {}
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
        let git_log_filter_input = cx.new(|cx| {
            TextInput::new(
                "Filter commits by message or hash…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let git_log_filter_subscription =
            cx.subscribe(&git_log_filter_input, |app, _, event, cx| match event {
                TextInputEvent::Submit(filter) => {
                    app.git_log_filter = filter.clone();
                    app.refresh_git_log(cx);
                }
                TextInputEvent::Change(filter) => {
                    app.git_log_filter = filter.clone();
                }
                _ => {}
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
        let debug_eval_input = cx.new(|cx| {
            TextInput::new(
                "Evaluate expression in the selected frame…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let debug_eval_subscription = cx.subscribe(&debug_eval_input, |app, _, event, cx| {
            if let TextInputEvent::Submit(expression) = event {
                app.evaluate_debug_expression(expression.clone(), cx);
            }
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
        let mcp_url_input = cx.new(|cx| {
            TextInput::new(
                "Server URL (http:// or https://)",
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
        let dependency_filter_input = cx.new(|cx| {
            TextInput::new(
                "Filter modules and dependencies…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let dependency_filter_subscription =
            cx.subscribe(&dependency_filter_input, |app, _, event, cx| {
                if let TextInputEvent::Change(filter) = event {
                    app.dependency_filter = filter.clone();
                    cx.notify();
                }
            });
        let coverage_path_input = cx.new(|cx| {
            TextInput::new(
                "Coverage report path (lcov, JaCoCo XML, Go cover profile)…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let coverage_input_subscription =
            cx.subscribe(&coverage_path_input, |app, _, event, cx| {
                if let TextInputEvent::Submit(_) = event {
                    app.import_coverage(cx);
                }
            });
        let github_comment_input = cx.new(|cx| {
            TextInput::new(
                "Write a review or conversation comment…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let github_comment_subscription =
            cx.subscribe(&github_comment_input, |app, _, event, cx| {
                if let TextInputEvent::Submit(_) = event {
                    app.add_github_comment(cx);
                }
            });
        let heap_path_input = cx.new(|cx| {
            TextInput::new(
                "Heap snapshot path (.hprof)…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let heap_input_subscription = cx.subscribe(&heap_path_input, |app, _, event, cx| {
            if let TextInputEvent::Submit(_) = event {
                app.open_heap_snapshot(cx);
            }
        });
        let profile_dir_input = cx.new(|cx| {
            TextInput::new(
                "Folder of Node .cpuprofile files…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let profile_input_subscription = cx.subscribe(&profile_dir_input, |app, _, event, cx| {
            if let TextInputEvent::Submit(_) = event {
                app.open_profile(cx);
            }
        });
        let logcat_device_input = cx.new(|cx| {
            TextInput::new(
                "Device serial (leave empty for the first online device)",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let logcat_device_subscription = cx.subscribe(&logcat_device_input, |app, _, event, cx| {
            if let TextInputEvent::Submit(_) = event {
                app.start_logcat(cx);
            }
        });
        let logcat_filter_input = cx.new(|cx| {
            TextInput::new(
                "Filter log text or tag…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let logcat_filter_subscription = cx.subscribe(&logcat_filter_input, |app, _, event, cx| {
            if let TextInputEvent::Change(filter) = event {
                app.logcat_filter = filter.clone();
                cx.notify();
            }
        });
        let scratch_name_input = cx.new(|cx| {
            TextInput::new(
                "Scratch file name (notes, Main.java…) then Enter",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let scratch_input_subscription = cx.subscribe(&scratch_name_input, |app, _, event, cx| {
            if let TextInputEvent::Submit(_) = event {
                app.create_scratch(cx);
            }
        });
        let structural_pattern_input = cx.new(|cx| {
            TextInput::new(
                "Structural pattern, e.g. return $A + $B;",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let structural_replacement_input = cx.new(|cx| {
            TextInput::new(
                "Replacement template (optional)",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let structural_language_input = cx.new(|cx| {
            TextInput::new(
                "Language id (java, typescript, rust…)",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        structural_language_input.update(cx, |input, cx| input.set_content("java", cx));
        let structural_subscriptions = vec![
            cx.subscribe(&structural_pattern_input, |app, _, event, cx| {
                if let TextInputEvent::Submit(_) = event {
                    app.run_structural_search(cx);
                }
            }),
            cx.subscribe(&structural_replacement_input, |app, _, event, cx| {
                if let TextInputEvent::Submit(_) = event {
                    app.run_structural_search(cx);
                }
            }),
            cx.subscribe(&structural_language_input, |app, _, event, cx| {
                if let TextInputEvent::Submit(_) = event {
                    app.run_structural_search(cx);
                }
            }),
        ];
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
        let scratch_dir = host.scratch_dir();
        let scratch_files = crate::scratch::list(&scratch_dir);
        let recent_file_path = host.config_dir().join("recent-files.json");
        let recent_store = crate::recent::RecentStore::load(&recent_file_path);
        let recent_files = recent_store.project(&root);
        let bookmarks_file = host.config_dir().join("bookmarks.json");
        let bookmark_store = crate::bookmarks::BookmarkStore::load(&bookmarks_file);
        let bookmarks = bookmark_store.project(&root).to_vec();
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
            save_as: false,
            save_as_input,
            _save_as_subscription: save_as_subscription,
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
            search_replace_input,
            search_replacement: String::new(),
            _search_replace_subscription: search_replace_subscription,
            search_preserve_case: true,
            search_replacing: false,
            search_regex: false,
            search_case_sensitive: false,
            search_whole_word: false,
            todo_items: Vec::new(),
            todo_status: "Open TODO to scan the project".into(),
            todo_case_sensitive: true,
            todo_comments_only: true,
            todo_generation: 0,
            dependency_modules: Vec::new(),
            dependency_coordinates: Vec::new(),
            dependency_status: "Open Dependencies to load build tools".into(),
            dependency_filter_input,
            dependency_filter: String::new(),
            dependency_show_modules: true,
            _dependency_filter_subscription: dependency_filter_subscription,
            coverage_suite: None,
            coverage_rows: Vec::new(),
            coverage_status: "Import a coverage report to inspect it".into(),
            coverage_path_input,
            coverage_sort_lines: true,
            _coverage_input_subscription: coverage_input_subscription,
            bookmarks,
            bookmark_store,
            bookmarks_file,
            bookmarks_status: "F3 toggles a line bookmark".into(),
            git_status: Vec::new(),
            git_branch: None,
            git_message_input,
            git_status_message: "Loading Git status…".into(),
            _git_subscription: git_subscription,
            git_commits: Vec::new(),
            git_log_status: "Open Git Log to load history".into(),
            git_log_filter_input,
            git_log_filter: String::new(),
            _git_log_filter_subscription: git_log_filter_subscription,
            docker_snapshot: None,
            docker_status: "Open Services to load Docker".into(),
            docker_tab: DockerTab::Containers,
            docker_busy: false,
            github_availability: None,
            github_pulls: Vec::new(),
            github_status: "Open GitHub to load pull requests".into(),
            github_state: ide_github::PrStateFilter::Open,
            github_busy: false,
            github_selected: None,
            github_details: None,
            github_comments: Vec::new(),
            github_detail_status: "Select a pull request".into(),
            github_tab: GitHubTab::Overview,
            github_comment_input,
            github_review_status: gpui::SharedString::default(),
            github_review_busy: false,
            _github_comment_subscription: github_comment_subscription,
            github_diff_files: Vec::new(),
            github_diff_status: "Select Diff to load changed files".into(),
            github_diff_busy: false,
            heap_info: None,
            heap_classes: Vec::new(),
            heap_path_input,
            heap_status: "Import an .hprof heap snapshot".into(),
            heap_busy: false,
            _heap_input_subscription: heap_input_subscription,
            heap_selected_class: None,
            heap_class_name: None,
            heap_objects: Vec::new(),
            heap_objects_total: 0,
            heap_objects_status: "Select a class to list objects".into(),
            heap_path_text: String::new(),
            profile: None,
            profile_dir_input,
            profile_status: "Import a folder of .cpuprofile files".into(),
            profile_busy: false,
            _profile_input_subscription: profile_input_subscription,
            logcat_devices: Vec::new(),
            logcat_lines: Vec::new(),
            logcat_status: "Open Logcat to list devices".into(),
            logcat_device_input,
            logcat_filter_input,
            logcat_filter: String::new(),
            logcat_level: crate::logcat::LogLevel::Verbose,
            logcat_cancel: None,
            logcat_busy: false,
            _logcat_device_subscription: logcat_device_subscription,
            _logcat_filter_subscription: logcat_filter_subscription,
            scratch_dir,
            scratch_files,
            scratch_name_input,
            scratch_status: "Create or open a scratch file".into(),
            _scratch_input_subscription: scratch_input_subscription,
            recent_store,
            recent_file_path,
            recent_files,
            recent_status: "Open a file to build a recent list".into(),
            duplicates_report: None,
            duplicates_status: "Run Locate Duplicates to scan the project".into(),
            duplicates_busy: false,
            duplicates_anonymize_identifiers: true,
            duplicates_anonymize_literals: true,
            analysis_tab: AnalysisTab::Duplicates,
            structural_pattern_input,
            structural_replacement_input,
            structural_language_input,
            structural_files: Vec::new(),
            structural_status: "Run a structural search to find syntax patterns".into(),
            structural_apply_status: gpui::SharedString::default(),
            structural_busy: false,
            _structural_subscriptions: structural_subscriptions,
            inspect_files: Vec::new(),
            inspect_summary: None,
            inspect_status: "Run Inspect Code to collect backend diagnostics".into(),
            inspect_busy: false,
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
            remote_preview_path: None,
            remote_editor: None,
            remote_edit_buffer: None,
            remote_edit_status: "Edit remote file".into(),
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
            mcp_transport: ide_agent::McpTransport::Stdio,
            mcp_name_input,
            mcp_command_input,
            mcp_args_input,
            mcp_url_input,
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
            references_open: false,
            references: Vec::new(),
            references_selected: 0,
            references_status: "Find usages at cursor".into(),
            plugins: Vec::new(),
            plugins_status: "Select PLUGINS to load installed plugins".into(),
            market_plugins: Vec::new(),
            market_status: "Marketplace not loaded".into(),
            market_loading: false,
            market_selected: 0,
            plugin_preview: None,
            plugin_consent_optional: BTreeSet::new(),
            plugin_preview_status: "Select a marketplace plugin to preview".into(),
            plugin_install_busy: false,
            debug_attachables: Vec::new(),
            debug_selected: 0,
            debug_sessions: Vec::new(),
            debug_session_selected: 0,
            debug_console: String::new(),
            debug_breakpoints: BTreeMap::new(),
            debug_breakpoint_states: Vec::new(),
            debug_active_session: None,
            debug_thread_id: None,
            debug_frames: Vec::new(),
            debug_frame_selected: 0,
            debug_scopes: Vec::new(),
            debug_variables: Vec::new(),
            debug_eval_input,
            _debug_eval_subscription: debug_eval_subscription,
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
            SidebarMode::GitLog => self.refresh_git_log(cx),
            SidebarMode::Services => self.refresh_docker(cx),
            SidebarMode::GitHub => self.refresh_github(cx),
            SidebarMode::Logcat => self.refresh_logcat_devices(cx),
            SidebarMode::Scratch => self.refresh_scratches(cx),
            SidebarMode::Recent => self.refresh_recent(cx),
            SidebarMode::Http => self.refresh_http(cx),
            SidebarMode::Database => self.refresh_db(cx),
            SidebarMode::Remote => self.refresh_remote(cx),
            SidebarMode::Notebook => self.refresh_notebook(cx),
            SidebarMode::Mcp => self.refresh_mcp(cx),
            SidebarMode::Agents => self.refresh_agents(cx),
            SidebarMode::Tools => self.refresh_tools(cx),
            SidebarMode::Plugins => self.refresh_plugins(cx),
            SidebarMode::Todo => self.refresh_todo(cx),
            SidebarMode::Dependencies => self.refresh_dependencies(cx),
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
        let path_for_app = path.clone();
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
                            app.remote_preview_path = Some(path_for_app.clone());
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

    fn open_remote_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.remote_preview_path.clone() else {
            self.remote_status = "Preview a remote file first".into();
            cx.notify();
            return;
        };
        let buffer = cx.new(|cx| Buffer::local(self.remote_preview.clone(), cx));
        let editor = cx.new(|cx| Editor::for_buffer(buffer.clone(), None, window, cx));
        let focus = editor.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.remote_editor = Some(editor);
        self.remote_edit_buffer = Some(buffer);
        self.remote_edit_status = format!("Editing {path}").into();
        cx.notify();
    }

    fn commit_remote_editor(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.remote_preview_path.clone() else {
            self.remote_editor = None;
            return;
        };
        let Some(buffer) = self.remote_edit_buffer.take() else {
            self.remote_editor = None;
            return;
        };
        self.remote_editor = None;
        let text = buffer.read(cx).snapshot().text();
        let Some(config) = self.ssh_configs.get(self.ssh_selected).cloned() else {
            self.remote_status = "No SSH configuration selected".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let id = config.id.clone();
        self.remote_status = format!("Writing {path}…").into();
        cx.spawn(async move |this, cx| {
            let result = host.remote_write(id, path.clone(), text.clone()).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(()) => {
                            app.remote_status = format!("Saved {} bytes", text.len()).into();
                            app.remote_preview = text;
                        }
                        Err(error) => {
                            app.remote_status =
                                format!("Remote write failed: {}", error.message).into();
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

    fn apply_notebook_edit(
        &mut self,
        edit: ide_notebook::NotebookEdit,
        status: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(notebook) = self.notebook.clone() else {
            self.notebook_status = "Open a notebook first".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let id = notebook.id.clone();
        let version = notebook.version;
        self.notebook_status = status.into();
        cx.spawn(async move |this, cx| {
            let result = host.notebook_edit(id.clone(), version, edit).await;
            let result = match result {
                Ok(_) => host.notebook_save(id).await,
                Err(error) => Err(error),
            };
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(notebook) => {
                            app.notebook_status =
                                format!("Updated · version {}", notebook.version).into();
                            app.notebook = Some(notebook);
                        }
                        Err(error) => {
                            app.notebook_status =
                                format!("Notebook update failed: {}", error.message).into();
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

    fn add_notebook_cell(&mut self, cell_type: &str, cx: &mut Context<Self>) {
        let after = self
            .notebook
            .as_ref()
            .and_then(|notebook| notebook.cells.last())
            .map(|cell| cell.id.clone());
        self.apply_notebook_edit(
            ide_notebook::NotebookEdit::InsertCell {
                after,
                cell_type: cell_type.to_owned(),
            },
            "Adding cell…",
            cx,
        );
    }

    fn delete_notebook_cell(&mut self, cell: String, cx: &mut Context<Self>) {
        self.apply_notebook_edit(
            ide_notebook::NotebookEdit::DeleteCell { cell },
            "Deleting cell…",
            cx,
        );
    }

    fn set_notebook_cell_type(&mut self, cell: String, cell_type: &str, cx: &mut Context<Self>) {
        self.apply_notebook_edit(
            ide_notebook::NotebookEdit::SetCellType {
                cell,
                cell_type: cell_type.to_owned(),
            },
            "Changing cell type…",
            cx,
        );
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
        self.mcp_transport = server.transport;
        let name = server.name.clone();
        self.mcp_name_input
            .update(cx, |input, cx| input.set_content(name.clone(), cx));
        self.mcp_command_input.update(cx, |input, cx| {
            input.set_content(server.command.clone().unwrap_or_default(), cx)
        });
        self.mcp_args_input
            .update(cx, |input, cx| input.set_content(server.args.join(" "), cx));
        self.mcp_url_input.update(cx, |input, cx| {
            input.set_content(server.url.clone().unwrap_or_default(), cx)
        });
        self.mcp_status = format!("Selected {name}").into();
        cx.notify();
    }

    fn new_mcp_server(&mut self, cx: &mut Context<Self>) {
        self.mcp_selected = None;
        self.mcp_transport = ide_agent::McpTransport::Stdio;
        self.mcp_name_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.mcp_command_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.mcp_args_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.mcp_url_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.mcp_status = "New MCP server".into();
        cx.notify();
    }

    fn set_mcp_transport(&mut self, transport: ide_agent::McpTransport, cx: &mut Context<Self>) {
        self.mcp_transport = transport;
        self.mcp_status = format!("Transport: {}", mcp_transport_label(transport)).into();
        cx.notify();
    }

    fn save_mcp_server(&mut self, cx: &mut Context<Self>) {
        let input = match build_mcp_server_input(
            self.mcp_selected.clone(),
            self.mcp_name_input.read(cx).content(),
            self.mcp_transport,
            self.mcp_command_input.read(cx).content(),
            self.mcp_args_input.read(cx).content(),
            self.mcp_url_input.read(cx).content(),
        ) {
            Ok(input) => input,
            Err(error) => {
                self.mcp_status = error.into();
                cx.notify();
                return;
            }
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
        self.plugin_consent_optional.clear();
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
                            app.plugin_consent_optional.clear();
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
        let optional = self
            .plugin_consent_optional
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let signer_change = preview.signer_changed;
        self.plugin_install_busy = true;
        self.plugin_preview_status = format!(
            "Installing {}… with {} optional grants",
            preview.name,
            optional.len()
        )
        .into();
        cx.spawn(async move |this, cx| {
            let result = host.plugin_install(id, optional, signer_change).await;
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

    fn refresh_dependencies(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let root = self.root.clone();
        self.dependency_status = "Loading build modules…".into();
        cx.spawn(async move |this, cx| {
            let result = host.build_model().await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(model) => {
                            app.dependency_modules = crate::build_tools::module_rows(&root, &model);
                            app.dependency_coordinates =
                                crate::build_tools::dependency_rows(&root, &model);
                            app.dependency_status = format!(
                                "{} modules · {} dependencies",
                                app.dependency_modules.len(),
                                app.dependency_coordinates.len()
                            )
                            .into();
                            app.build_model = Some(model);
                        }
                        Err(error) => {
                            app.dependency_modules.clear();
                            app.dependency_coordinates.clear();
                            app.dependency_status =
                                format!("Build tools failed: {}", error.message).into();
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

    fn import_coverage(&mut self, cx: &mut Context<Self>) {
        let raw = self
            .coverage_path_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        if raw.is_empty() {
            self.coverage_status = "Enter a coverage report path".into();
            cx.notify();
            return;
        }
        let path = PathBuf::from(raw);
        let path = if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        };
        let host = self.host.clone();
        let root = self.root.clone();
        self.coverage_status = format!("Importing {}…", path.display()).into();
        cx.spawn(async move |this, cx| {
            let result = host.coverage_import(path.clone()).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(suite) => {
                            let mut rows = crate::coverage::coverage_rows(&root, &suite);
                            crate::coverage::sort_rows(&mut rows, app.coverage_sort_lines);
                            let (covered, total, _, _) = crate::coverage::totals(&suite.files);
                            app.coverage_status = format!(
                                "{} · {} files · {} lines",
                                suite.runner,
                                suite.files.len(),
                                crate::coverage::format_percent(covered, total)
                            )
                            .into();
                            app.coverage_rows = rows;
                            app.coverage_suite = Some(suite);
                        }
                        Err(error) => {
                            app.coverage_rows.clear();
                            app.coverage_suite = None;
                            app.coverage_status =
                                format!("Coverage import failed: {}", error.message).into();
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
                                "{} attachable · {} sessions · {} debuggable run configs",
                                processes.len(),
                                app.debug_sessions.len(),
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
                    app.debug_sessions = app.host.debug_sessions().unwrap_or_default();
                    app.debug_session_selected = app
                        .debug_session_selected
                        .min(app.debug_sessions.len().saturating_sub(1));
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

    fn attach_debug_process(&mut self, cx: &mut Context<Self>) {
        use ide_dap::attach::AttachKind;
        let Some(process) = self.debug_attachables.get(self.debug_selected).cloned() else {
            self.debug_status = "No attachable process selected".into();
            cx.notify();
            return;
        };
        let extra = match process.kind {
            AttachKind::Node => serde_json::json!({
                "type": "pwa-node",
                "address": process.host.clone().unwrap_or_else(|| "127.0.0.1".into()),
                "port": process.port,
                "processId": if process.port.is_none() { Some(process.pid.to_string()) } else { None },
                "continueOnAttach": true,
            }),
            AttachKind::Python => {
                if let Some(port) = process.port {
                    serde_json::json!({"debugServer": port, "justMyCode": true})
                } else {
                    serde_json::json!({"processId": process.pid, "justMyCode": true})
                }
            }
            AttachKind::Dotnet => {
                serde_json::json!({"processId": process.pid, "justMyCode": true})
            }
            AttachKind::Go => {
                serde_json::json!({"mode": "local", "processId": process.pid})
            }
            AttachKind::Native => serde_json::json!({"pid": process.pid}),
            AttachKind::Java | AttachKind::Unity => {
                self.debug_status = format!(
                    "Attach config for {:?} is not implemented yet",
                    process.kind
                )
                .into();
                cx.notify();
                return;
            }
        };
        let config = ide_dap::LaunchConfig {
            name: format!("{} ({})", process.name, process.pid),
            adapter: process.adapter.clone(),
            request: "attach".into(),
            program: None,
            args: Vec::new(),
            cwd: Some(ide_core::IpcPath(self.root.clone())),
            env: Default::default(),
            extra_json: Some(extra.to_string()),
        };
        let host = self.host.clone();
        self.debug_status = format!("Starting debug adapter for {}…", process.name).into();
        cx.spawn(async move |this, cx| {
            let result = host.debug_start(config).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(session) => {
                            app.debug_status = format!("Session {session} started").into();
                            app.debug_sessions = app.host.debug_sessions().unwrap_or_default();
                        }
                        Err(error) => {
                            app.debug_status =
                                format!("Debug start failed: {}", error.message).into();
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

    fn stop_debug_session(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self
            .debug_sessions
            .get(self.debug_session_selected)
            .cloned()
        else {
            self.debug_status = "No debug session selected".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let name = session.name.clone();
        self.debug_status = format!("Stopping {name}…").into();
        cx.spawn(async move |this, cx| {
            let result = host.debug_stop(session.id).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(()) => {
                            app.debug_status = format!("Stopped {name}").into();
                            app.debug_sessions = app.host.debug_sessions().unwrap_or_default();
                        }
                        Err(error) => {
                            app.debug_status =
                                format!("Debug stop failed: {}", error.message).into();
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

    fn toggle_breakpoint_at_cursor(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            self.debug_status = "Open a file first".into();
            cx.notify();
            return;
        };
        let Some(path) = self
            .documents
            .get(&doc)
            .map(|document| document.path.clone())
        else {
            self.debug_status = "The active file is not open".into();
            cx.notify();
            return;
        };
        let Some(editor) = self.editors.get(&(doc, false)).cloned() else {
            self.debug_status = "The active file has no editor".into();
            cx.notify();
            return;
        };
        let line = editor.update(cx, |editor, cx| {
            let snapshot = editor.display_snapshot(cx);
            editor.selections.newest_adjusted(&snapshot).head().row
        });
        let lines = self.debug_breakpoints.entry(path.clone()).or_default();
        if !lines.insert(line) {
            lines.remove(&line);
        }
        let breakpoints: Vec<ide_dap::Breakpoint> = lines
            .iter()
            .map(|line| ide_dap::Breakpoint {
                line: *line,
                condition: None,
                hit_condition: None,
                log_message: None,
                enabled: true,
            })
            .collect();
        let host = self.host.clone();
        let path_for_call = path.clone();
        cx.spawn(async move |this, cx| {
            let result = host
                .debug_set_breakpoints(path_for_call.clone(), breakpoints)
                .await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(states) => {
                            app.debug_breakpoint_states = states;
                            app.debug_status =
                                format!("Breakpoint toggled at line {}", line + 1).into();
                        }
                        Err(error) => {
                            app.debug_status =
                                format!("Breakpoint failed: {}", error.message).into();
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

    fn refresh_debug_stack(
        &mut self,
        session: ide_core::DebugSessionId,
        thread_id: i64,
        cx: &mut Context<Self>,
    ) {
        let host = self.host.clone();
        self.debug_active_session = Some(session);
        self.debug_thread_id = Some(thread_id);
        self.debug_status = "Loading stack frames…".into();
        cx.spawn(async move |this, cx| {
            let result = host.debug_stack_trace(session, thread_id).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(frames) => {
                            app.debug_frames = frames;
                            app.debug_frame_selected = 0;
                            if !app.debug_frames.is_empty() {
                                app.select_debug_frame(0, cx);
                            }
                        }
                        Err(error) => {
                            app.debug_status =
                                format!("Stack trace failed: {}", error.message).into();
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

    fn select_debug_frame(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(frame) = self.debug_frames.get(index).cloned() else {
            return;
        };
        let Some(session) = self.debug_active_session else {
            return;
        };
        self.debug_frame_selected = index;
        let host = self.host.clone();
        self.debug_scopes.clear();
        self.debug_variables.clear();
        cx.spawn(async move |this, cx| {
            let result = host.debug_scopes(session, frame.id).await;
            let variables = match result {
                Ok(scopes) => {
                    let mut variables = Vec::new();
                    for scope in &scopes {
                        match host
                            .debug_variables(session, scope.variables_reference)
                            .await
                        {
                            Ok(mut values) => variables.append(&mut values),
                            Err(error) => {
                                eprintln!("WORKBENCH_DEBUG_VARIABLES_ERROR {}", error.message);
                            }
                        }
                    }
                    Ok((scopes, variables))
                }
                Err(error) => Err(error),
            };
            let handle = this
                .update(cx, |app, cx| {
                    match variables {
                        Ok((scopes, variables)) => {
                            app.debug_scopes = scopes;
                            app.debug_variables = variables;
                        }
                        Err(error) => {
                            app.debug_status = format!("Scopes failed: {}", error.message).into();
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

    fn debug_resume(&mut self, cx: &mut Context<Self>) {
        let (Some(session), Some(thread_id)) = (self.debug_active_session, self.debug_thread_id)
        else {
            return;
        };
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.debug_resume(session, thread_id).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(()) => {
                            app.debug_status = "Resumed".into();
                            app.debug_frames.clear();
                            app.debug_scopes.clear();
                            app.debug_variables.clear();
                        }
                        Err(error) => {
                            app.debug_status = format!("Resume failed: {}", error.message).into();
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

    fn debug_pause(&mut self, cx: &mut Context<Self>) {
        let (Some(session), Some(thread_id)) = (self.debug_active_session, self.debug_thread_id)
        else {
            return;
        };
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.debug_pause(session, thread_id).await;
            let handle = this
                .update(cx, |app, cx| {
                    if let Err(error) = result {
                        app.debug_status = format!("Pause failed: {}", error.message).into();
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

    fn debug_step(&mut self, kind: ide_dap::StepKind, cx: &mut Context<Self>) {
        let (Some(session), Some(thread_id)) = (self.debug_active_session, self.debug_thread_id)
        else {
            return;
        };
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.debug_step(session, thread_id, kind).await;
            let handle = this
                .update(cx, |app, cx| {
                    if let Err(error) = result {
                        app.debug_status = format!("Step failed: {}", error.message).into();
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

    fn evaluate_debug_expression(&mut self, expression: String, cx: &mut Context<Self>) {
        let Some(session) = self.debug_active_session else {
            self.debug_status = "No active debug session".into();
            cx.notify();
            return;
        };
        let expression = expression.trim().to_owned();
        if expression.is_empty() {
            return;
        }
        let frame_id = self
            .debug_frames
            .get(self.debug_frame_selected)
            .map(|frame| frame.id);
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host
                .debug_evaluate(session, frame_id, expression.clone())
                .await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(variable) => {
                            app.debug_console.push_str(&format!(
                                "> {expression}\n= {}{}\n",
                                variable.value,
                                variable
                                    .type_name
                                    .as_ref()
                                    .map(|ty| format!(" : {ty}"))
                                    .unwrap_or_default()
                            ));
                            app.debug_eval_input.update(cx, |input, cx| {
                                input.set_content("", cx);
                            });
                        }
                        Err(error) => {
                            app.debug_status = format!("Evaluate failed: {}", error.message).into();
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
            self.record_recent_file(&path, cx);
            self.activate(doc, cx);
            return;
        }
        if self
            .documents
            .values()
            .any(|document| document.path == path)
        {
            if let Some(doc) = self
                .documents
                .iter()
                .find(|(_, document)| document.path == path)
                .map(|(doc, _)| *doc)
            {
                self.record_recent_file(&path, cx);
                self.activate(doc, cx);
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

    fn find_usages_at_cursor(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            self.references_status = "Open a file first".into();
            cx.notify();
            return;
        };
        let Some(editor) = self.editors.get(&(doc, false)).cloned() else {
            self.references_status = "The active file has no editor".into();
            cx.notify();
            return;
        };
        let point = editor.update(cx, |editor, cx| {
            let snapshot = editor.display_snapshot(cx);
            editor.selections.newest_adjusted(&snapshot).head()
        });
        let pos = ide_core::Pos::new(point.row, point.column);
        let host = self.host.clone();
        self.references_open = true;
        self.references.clear();
        self.references_selected = 0;
        self.references_status = "Finding usages…".into();
        cx.spawn(async move |this, cx| {
            let result = host.lsp_references(doc, pos).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(locations) => {
                            app.references_status = format!("{} usages", locations.len()).into();
                            app.references = locations;
                        }
                        Err(error) => {
                            app.references_status =
                                format!("Find usages failed: {}", error.message).into();
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

    fn toggle_bookmark(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            self.bookmarks_status = "Open a file first".into();
            cx.notify();
            return;
        };
        let Some(editor) = self.editors.get(&(doc, false)).cloned() else {
            self.bookmarks_status = "The active file has no editor".into();
            cx.notify();
            return;
        };
        let row = editor.update(cx, |editor, cx| {
            let snapshot = editor.display_snapshot(cx);
            editor.selections.newest_adjusted(&snapshot).head().row
        });
        let Some(path) = self
            .documents
            .get(&doc)
            .map(|document| document.path.clone())
        else {
            return;
        };
        let multi_buffer = editor.read(cx).buffer().clone();
        let text = multi_buffer
            .read(cx)
            .snapshot(cx)
            .text_for_range(TextPoint::new(row, 0)..TextPoint::new(row + 1, 0))
            .collect::<String>()
            .trim_end()
            .to_owned();
        let added = crate::bookmarks::toggle(&mut self.bookmarks, path.clone(), row, text);
        self.bookmark_store
            .set_project(&self.root, self.bookmarks.clone());
        match self.bookmark_store.save(&self.bookmarks_file) {
            Ok(()) => {
                self.bookmarks_status = if added {
                    format!("Bookmarked {}:{}", path.display(), row + 1)
                } else {
                    format!("Removed bookmark {}:{}", path.display(), row + 1)
                }
                .into();
            }
            Err(error) => {
                self.bookmarks_status = format!("Bookmark save failed: {error}").into();
            }
        }
        if self.sidebar_mode == SidebarMode::Bookmarks {
            self.bookmarks_status = format!("{} bookmarks", self.bookmarks.len()).into();
        }
        cx.notify();
    }

    fn remove_bookmark(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.bookmarks.len() {
            return;
        }
        self.bookmarks.remove(index);
        self.bookmark_store
            .set_project(&self.root, self.bookmarks.clone());
        self.bookmarks_status = match self.bookmark_store.save(&self.bookmarks_file) {
            Ok(()) => format!("{} bookmarks", self.bookmarks.len()).into(),
            Err(error) => format!("Bookmark save failed: {error}").into(),
        };
        cx.notify();
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
            Command::SaveAs => self.open_save_as(window, cx),
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
            Command::FindUsages => self.find_usages_at_cursor(cx),
            Command::ShowProject => self.select_sidebar(SidebarMode::Project, cx),
            Command::ShowSearch => self.select_sidebar(SidebarMode::Search, cx),
            Command::ShowTodo => self.select_sidebar(SidebarMode::Todo, cx),
            Command::ShowDependencies => self.select_sidebar(SidebarMode::Dependencies, cx),
            Command::ShowCoverage => self.select_sidebar(SidebarMode::Coverage, cx),
            Command::ShowBookmarks => self.select_sidebar(SidebarMode::Bookmarks, cx),
            Command::ShowGit => self.select_sidebar(SidebarMode::Git, cx),
            Command::ShowGitLog => self.select_sidebar(SidebarMode::GitLog, cx),
            Command::ShowServices => self.select_sidebar(SidebarMode::Services, cx),
            Command::ShowGitHub => self.select_sidebar(SidebarMode::GitHub, cx),
            Command::ShowHeap => self.select_sidebar(SidebarMode::Heap, cx),
            Command::ShowProfiler => self.select_sidebar(SidebarMode::Profiler, cx),
            Command::ShowLogcat => self.select_sidebar(SidebarMode::Logcat, cx),
            Command::ShowScratches => self.select_sidebar(SidebarMode::Scratch, cx),
            Command::NewScratch => {
                self.select_sidebar(SidebarMode::Scratch, cx);
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_secs())
                    .unwrap_or_default();
                let name = format!("scratch-{stamp}.md");
                self.scratch_name_input
                    .update(cx, |input, cx| input.set_content(name, cx));
                self.create_scratch(cx);
            }
            Command::ShowRecent => self.select_sidebar(SidebarMode::Recent, cx),
            Command::ShowAnalysis => self.select_sidebar(SidebarMode::Analysis, cx),
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
            Command::RefreshTodo => self.refresh_todo(cx),
            Command::RefreshDependencies => self.refresh_dependencies(cx),
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
        self.record_recent_file(&info.path.0, cx);
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

    fn open_save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        let path = self
            .documents
            .get(&doc)
            .map(|document| document.path.clone());
        self.save_as_input.update(cx, |input, cx| {
            input.set_content(
                path.as_deref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                cx,
            );
        });
        self.save_as = true;
        let focus = self.save_as_input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    fn perform_save_as(&mut self, cx: &mut Context<Self>) {
        if !self.save_as {
            return;
        }
        let Some(doc) = self.active else {
            self.status = "Open a file first".into();
            cx.notify();
            return;
        };
        let raw = self.save_as_input.read(cx).content().trim().to_owned();
        if raw.is_empty() {
            self.status = "Enter a save path".into();
            cx.notify();
            return;
        }
        let path = PathBuf::from(raw);
        let path = if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        };
        let Some(text) = self
            .documents
            .get(&doc)
            .map(|document| document.buffer.read(cx).snapshot().text())
        else {
            return;
        };
        let host = self.host.clone();
        self.save_as = false;
        self.status = format!("Saving a copy as {}…", path.display()).into();
        cx.spawn(async move |this, cx| {
            let result = host.save_file_copy(path.clone(), text).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(()) => {
                            eprintln!("WORKBENCH_SAVE_COPY ok path={}", path.display());
                            app.record_recent_file(&path, cx);
                            app.status = format!("Saved a copy as {}", path.display()).into();
                        }
                        Err(error) => {
                            eprintln!("WORKBENCH_SAVE_COPY failed {}", error.message);
                            app.status = format!("Save copy failed: {}", error.message).into();
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
        let query = build_search_query(
            pattern,
            self.search_regex,
            self.search_case_sensitive,
            self.search_whole_word,
        );
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

    fn replace_search_results(&mut self, cx: &mut Context<Self>) {
        if self.search_replacing {
            return;
        }
        let pattern = self.search_pattern.trim().to_owned();
        if pattern.is_empty() {
            self.search_status = "Enter a search pattern before replacing".into();
            cx.notify();
            return;
        }
        let targets = search_replace_targets(&self.search_results);
        if targets.is_empty() {
            self.search_status = "No current search matches to replace".into();
            cx.notify();
            return;
        }
        let replacement = self.search_replacement.clone();
        let preserve_case = self.search_preserve_case;
        let query = build_search_query(
            pattern.clone(),
            self.search_regex,
            self.search_case_sensitive,
            self.search_whole_word,
        );
        let host = self.host.clone();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let events = ide_host::ops::Channel::new(move |event: ide_search::ReplaceEvent| {
            let _ = event_tx.send(event);
        });
        self.search_replacing = true;
        self.search_status = format!("Replacing {} matches…", targets.len()).into();
        cx.spawn(async move |this, cx| {
            let result = host
                .replace_in_files(query, replacement, preserve_case, targets, events)
                .await;
            let handle = this
                .update(cx, |app, cx| {
                    app.search_replacing = false;
                    match result {
                        Ok(()) => {
                            let mut summary = None;
                            let mut failed = Vec::new();
                            while let Ok(event) = event_rx.try_recv() {
                                match event {
                                    ide_search::ReplaceEvent::Done { summary: done } => {
                                        summary = Some(done);
                                    }
                                    ide_search::ReplaceEvent::Failed { path, message, .. } => {
                                        failed.push(format!("{}: {message}", path.0.display()));
                                    }
                                    ide_search::ReplaceEvent::File { .. } => {}
                                }
                            }
                            app.search_status = match summary {
                                Some(summary) => {
                                    eprintln!(
                                        "WORKBENCH_REPLACE replacements={} files={} failed={}",
                                        summary.replacements, summary.files, summary.failed
                                    );
                                    format!(
                                        "Replaced {} matches in {} files{}",
                                        summary.replacements,
                                        summary.files,
                                        if summary.failed > 0 {
                                            format!(
                                                " · {} failed: {}",
                                                summary.failed,
                                                failed.join("; ")
                                            )
                                        } else {
                                            String::new()
                                        }
                                    )
                                    .into()
                                }
                                None => "Replace finished without a summary".into(),
                            };
                            app.start_search(pattern.clone(), cx);
                        }
                        Err(error) => {
                            app.search_status = format!("Replace failed: {}", error.message).into();
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

    fn refresh_todo(&mut self, cx: &mut Context<Self>) {
        self.todo_generation = self.todo_generation.wrapping_add(1);
        let generation = self.todo_generation;
        self.todo_items.clear();
        self.todo_status = "Scanning project TODO comments…".into();
        let query =
            crate::todo::build_todo_query(self.todo_case_sensitive, self.todo_comments_only);
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
                        if app.todo_generation == generation {
                            app.todo_status = format!("TODO scan failed: {}", error.message).into();
                            cx.notify();
                        }
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
                        if app.todo_generation != generation {
                            return app.window_handle;
                        }
                        match event {
                            ide_host::ops::search::SearchEvent::File { file } => {
                                app.todo_items
                                    .extend(crate::todo::todo_items_from_file(&file));
                                app.todo_status = format!(
                                    "{} TODO items in {} files",
                                    app.todo_items.len(),
                                    app.todo_items
                                        .iter()
                                        .map(|item| &item.path)
                                        .collect::<BTreeSet<_>>()
                                        .len()
                                )
                                .into();
                            }
                            ide_host::ops::search::SearchEvent::Done { summary } => {
                                app.todo_status = format!(
                                    "{} TODO items · {} files scanned{}",
                                    app.todo_items.len(),
                                    summary.files,
                                    if summary.limit_hit {
                                        " · limit reached"
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

    fn refresh_git_log(&mut self, cx: &mut Context<Self>) {
        let filter = self.git_log_filter.trim().to_owned();
        let query = ide_vcs::LogQuery {
            text: (!filter.is_empty()).then_some(filter),
            limit: 100,
            ..Default::default()
        };
        let host = self.host.clone();
        self.git_log_status = "Loading Git history…".into();
        cx.spawn(async move |this, cx| {
            let result = host.git_log(query).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(commits) => {
                            app.git_log_status = format!("{} commits", commits.len()).into();
                            app.git_commits = commits;
                        }
                        Err(error) => {
                            app.git_commits.clear();
                            app.git_log_status =
                                format!("Git log failed: {}", error.message).into();
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

    fn refresh_docker(&mut self, cx: &mut Context<Self>) {
        if self.docker_busy {
            return;
        }
        self.docker_busy = true;
        self.docker_status = "Loading Docker services…".into();
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.docker_snapshot().await;
            let handle = this
                .update(cx, |app, cx| {
                    app.docker_busy = false;
                    match result {
                        Ok(snapshot) => {
                            app.docker_status = match &snapshot.availability {
                                ide_docker::DockerAvailability::Ready {
                                    client_version,
                                    server_version,
                                    context,
                                } => format!(
                                    "Docker {client_version} · engine {server_version}{}{}",
                                    context
                                        .as_ref()
                                        .map(|context| format!(" · {context}"))
                                        .unwrap_or_default(),
                                    if snapshot.errors.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" · {}", snapshot.errors.join("; "))
                                    }
                                )
                                .into(),
                                ide_docker::DockerAvailability::NotInstalled => {
                                    "Docker is not installed".into()
                                }
                                ide_docker::DockerAvailability::DaemonDown { message, .. } => {
                                    format!("Docker daemon unavailable: {message}").into()
                                }
                            };
                            app.docker_snapshot = Some(snapshot);
                        }
                        Err(error) => {
                            app.docker_snapshot = None;
                            app.docker_status = format!("Docker failed: {}", error.message).into();
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

    fn docker_container_action(
        &mut self,
        id: String,
        name: String,
        op: ide_docker::ContainerOp,
        cx: &mut Context<Self>,
    ) {
        if self.docker_busy {
            return;
        }
        self.docker_busy = true;
        let verb = match op {
            ide_docker::ContainerOp::Start => "Starting",
            ide_docker::ContainerOp::Stop => "Stopping",
            ide_docker::ContainerOp::Restart => "Restarting",
            ide_docker::ContainerOp::Pause => "Pausing",
            ide_docker::ContainerOp::Unpause => "Unpausing",
            ide_docker::ContainerOp::Kill => "Killing",
        };
        self.docker_status = format!("{verb} {name}…").into();
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.docker_container_op(id, op).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.docker_busy = false;
                    match result {
                        Ok(()) => {
                            app.docker_status = format!("{verb} {name}").into();
                            app.refresh_docker(cx);
                        }
                        Err(error) => {
                            app.docker_status =
                                format!("Docker action failed: {}", error.message).into();
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

    fn refresh_github(&mut self, cx: &mut Context<Self>) {
        if self.github_busy {
            return;
        }
        self.github_busy = true;
        self.github_status = "Checking GitHub CLI…".into();
        let host = self.host.clone();
        let root = self.root.clone();
        let state = self.github_state;
        cx.spawn(async move |this, cx| {
            let availability = host.github_availability(root.clone()).await;
            let pulls = if matches!(&availability, Ok(ide_github::GhAvailability::Ready { .. })) {
                Some(
                    host.github_pr_list(
                        root,
                        ide_github::PrListQuery {
                            state,
                            author_me: false,
                            review_requested: false,
                            assignee_me: false,
                            search: String::new(),
                            limit: 100,
                        },
                    )
                    .await,
                )
            } else {
                None
            };
            let handle = this
                .update(cx, |app, cx| {
                    app.github_busy = false;
                    match availability {
                        Ok(availability) => {
                            app.github_status = match &availability {
                                ide_github::GhAvailability::Ready { repo, login } => format!(
                                    "{} · {}",
                                    repo.name_with_owner,
                                    login.as_deref().unwrap_or("signed in")
                                )
                                .into(),
                                ide_github::GhAvailability::NotInstalled => {
                                    "GitHub CLI (gh) is not installed".into()
                                }
                                ide_github::GhAvailability::NotLoggedIn { message } => {
                                    format!("GitHub CLI is not signed in: {message}").into()
                                }
                                ide_github::GhAvailability::NotGitHub { message } => {
                                    format!("No GitHub repository: {message}").into()
                                }
                                ide_github::GhAvailability::Error { message } => {
                                    format!("GitHub failed: {message}").into()
                                }
                            };
                            app.github_availability = Some(availability);
                            if let Some(result) = pulls {
                                match result {
                                    Ok(pulls) => {
                                        app.github_status = format!(
                                            "{} · {} pull requests",
                                            app.github_status,
                                            pulls.len()
                                        )
                                        .into();
                                        app.github_pulls = pulls;
                                    }
                                    Err(error) => {
                                        app.github_pulls.clear();
                                        app.github_status =
                                            format!("Pull requests failed: {}", error.message)
                                                .into();
                                    }
                                }
                            } else {
                                app.github_pulls.clear();
                            }
                        }
                        Err(error) => {
                            app.github_availability = None;
                            app.github_pulls.clear();
                            app.github_status = format!("GitHub failed: {}", error.message).into();
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

    fn open_github_pr(&mut self, number: u32, cx: &mut Context<Self>) {
        self.github_selected = Some(number);
        self.github_details = None;
        self.github_comments.clear();
        self.github_diff_files.clear();
        self.github_diff_status = "Select Diff to load changed files".into();
        self.github_tab = GitHubTab::Overview;
        self.github_detail_status = format!("Loading pull request #{number}…").into();
        self.load_github_pr(number, false, cx);
    }

    fn reload_github_pr(&mut self, number: u32, cx: &mut Context<Self>) {
        if self.github_selected != Some(number) {
            return;
        }
        self.github_detail_status = format!("Refreshing pull request #{number}…").into();
        self.load_github_pr(number, true, cx);
    }

    fn load_github_pr(&mut self, number: u32, preserve_on_error: bool, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let root = self.root.clone();
        cx.spawn(async move |this, cx| {
            let (details, comments) = tokio::join!(
                host.github_pr_view(root.clone(), number),
                host.github_review_comments(root, number)
            );
            let handle = this
                .update(cx, |app, cx| {
                    match details {
                        Ok(details) => {
                            app.github_detail_status = format!(
                                "{} files · {} checks",
                                details.files.len(),
                                details.checks.len()
                            )
                            .into();
                            app.github_details = Some(details);
                            match comments {
                                Ok(comments) => app.github_comments = comments,
                                Err(error) => {
                                    app.github_comments.clear();
                                    app.github_detail_status =
                                        format!("Review comments failed: {}", error.message).into();
                                }
                            }
                        }
                        Err(error) => {
                            if !preserve_on_error {
                                app.github_details = None;
                            }
                            app.github_detail_status =
                                format!("Pull request failed: {}", error.message).into();
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

    fn load_github_diff(&mut self, cx: &mut Context<Self>) {
        if self.github_diff_busy {
            return;
        }
        let Some(number) = self.github_selected else {
            self.github_diff_status = "Select a pull request first".into();
            cx.notify();
            return;
        };
        let Some(details) = self.github_details.clone() else {
            self.github_diff_status = "Wait for pull request details to load".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let root = self.root.clone();
        self.github_diff_busy = true;
        self.github_diff_status = "Loading diff…".into();
        cx.spawn(async move |this, cx| {
            let result = host.github_pr_diff(root, number, details).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.github_diff_busy = false;
                    match result {
                        Ok(diff) => {
                            let files = crate::pr_diff::parse(&diff.patch);
                            let lines = files.iter().map(|file| file.lines.len()).sum::<usize>();
                            app.github_diff_status = format!(
                                "{} files · {} diff lines · {} → {}",
                                files.len(),
                                lines,
                                &diff.base[..diff.base.len().min(7)],
                                &diff.head[..diff.head.len().min(7)]
                            )
                            .into();
                            app.github_diff_files = files;
                        }
                        Err(error) => {
                            app.github_diff_files.clear();
                            app.github_diff_status =
                                format!("Diff failed: {}", error.message).into();
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

    fn submit_github_review(&mut self, event: ide_github::ReviewEvent, cx: &mut Context<Self>) {
        if self.github_review_busy {
            return;
        }
        let Some(number) = self.github_selected else {
            self.github_review_status = "Select a pull request first".into();
            cx.notify();
            return;
        };
        let body = self
            .github_comment_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        if !matches!(event, ide_github::ReviewEvent::Approve) && body.is_empty() {
            self.github_review_status = "Write a review comment first".into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        let root = self.root.clone();
        self.github_review_busy = true;
        self.github_review_status = "Submitting review…".into();
        cx.spawn(async move |this, cx| {
            let result = host.github_submit_review(root, number, event, body).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.github_review_busy = false;
                    match result {
                        Ok(()) => {
                            app.github_review_status =
                                format!("Review submitted for #{number}").into();
                            app.github_comment_input.update(cx, |input, cx| {
                                input.set_content("", cx);
                            });
                            app.reload_github_pr(number, cx);
                        }
                        Err(error) => {
                            app.github_review_status =
                                format!("Review failed: {}", error.message).into();
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

    fn add_github_comment(&mut self, cx: &mut Context<Self>) {
        if self.github_review_busy {
            return;
        }
        let Some(number) = self.github_selected else {
            self.github_review_status = "Select a pull request first".into();
            cx.notify();
            return;
        };
        let body = self
            .github_comment_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        if body.is_empty() {
            self.github_review_status = "Write a comment first".into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        let root = self.root.clone();
        self.github_review_busy = true;
        self.github_review_status = "Adding comment…".into();
        cx.spawn(async move |this, cx| {
            let result = host.github_add_comment(root, number, body).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.github_review_busy = false;
                    match result {
                        Ok(()) => {
                            app.github_review_status = format!("Comment added to #{number}").into();
                            app.github_comment_input.update(cx, |input, cx| {
                                input.set_content("", cx);
                            });
                            app.reload_github_pr(number, cx);
                        }
                        Err(error) => {
                            app.github_review_status =
                                format!("Comment failed: {}", error.message).into();
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

    fn close_github_pr(&mut self, cx: &mut Context<Self>) {
        self.github_selected = None;
        self.github_details = None;
        self.github_comments.clear();
        cx.notify();
    }

    fn open_heap_snapshot(&mut self, cx: &mut Context<Self>) {
        if self.heap_busy {
            return;
        }
        let raw = self.heap_path_input.read(cx).content().trim().to_owned();
        if raw.is_empty() {
            self.heap_status = "Enter an .hprof file path".into();
            cx.notify();
            return;
        }
        let path = PathBuf::from(raw);
        let path = if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        };
        let previous = self.heap_info.as_ref().map(|info| info.id);
        let host = self.host.clone();
        self.heap_busy = true;
        self.heap_status = format!("Analyzing {}…", path.display()).into();
        cx.spawn(async move |this, cx| {
            let mut previous_error = None;
            if let Some(previous) = previous
                && let Err(error) = host.heap_close(previous).await
            {
                previous_error = Some(error.message);
            }
            let result = host.heap_open(path).await;
            let result = match result {
                Ok(ide_host::ops::heap::HeapOpenResult::Ready { snapshot }) => {
                    let classes = host.heap_classes(snapshot.id).await;
                    Ok((snapshot, classes))
                }
                Ok(ide_host::ops::heap::HeapOpenResult::NotEnoughDiskSpace {
                    needed,
                    available,
                }) => Err(format!(
                    "Not enough disk space for the index: need {}, have {}",
                    format_bytes(needed),
                    format_bytes(available)
                )),
                Ok(ide_host::ops::heap::HeapOpenResult::Cancelled) => {
                    Err("Heap analysis cancelled".to_owned())
                }
                Err(error) => Err(error.message),
            };
            let handle = this
                .update(cx, |app, cx| {
                    app.heap_busy = false;
                    match result {
                        Ok((snapshot, Ok(classes))) => {
                            app.heap_status = format!(
                                "{} · {} objects · {} classes",
                                snapshot.label, snapshot.summary.objects, snapshot.summary.classes
                            )
                            .into();
                            app.heap_classes = classes;
                            app.heap_info = Some(snapshot);
                        }
                        Ok((snapshot, Err(error))) => {
                            app.heap_status =
                                format!("Heap classes failed: {}", error.message).into();
                            app.heap_classes.clear();
                            app.heap_info = Some(snapshot);
                        }
                        Err(message) => {
                            app.heap_status = format!(
                                "{}{}",
                                previous_error
                                    .map(|error| format!(
                                        "Close previous snapshot failed: {error}; "
                                    ))
                                    .unwrap_or_default(),
                                message
                            )
                            .into();
                            app.heap_classes.clear();
                            app.heap_info = None;
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

    fn close_heap_snapshot(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self.heap_info.as_ref().map(|info| info.id) else {
            return;
        };
        if self.heap_busy {
            return;
        }
        self.heap_busy = true;
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.heap_close(snapshot).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.heap_busy = false;
                    match result {
                        Ok(()) => {
                            app.heap_info = None;
                            app.heap_classes.clear();
                            app.heap_status = "Heap snapshot closed".into();
                        }
                        Err(error) => {
                            app.heap_status =
                                format!("Close heap failed: {}", error.message).into();
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

    fn open_heap_class(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(class) = self.heap_classes.get(index).cloned() else {
            return;
        };
        let Some(snapshot) = self.heap_info.as_ref().map(|info| info.id) else {
            return;
        };
        self.heap_selected_class = Some(class.class_index);
        self.heap_class_name = Some(class.name.clone());
        self.heap_objects.clear();
        self.heap_objects_total = 0;
        self.heap_path_text.clear();
        self.heap_objects_status = format!("Loading objects of {}…", class.name).into();
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.heap_dominators(snapshot, class.class_index).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(page) => {
                            app.heap_objects_total = page.total;
                            app.heap_objects_status = format!("{} objects", page.total).into();
                            app.heap_objects = page.rows;
                        }
                        Err(error) => {
                            app.heap_objects.clear();
                            app.heap_objects_status =
                                format!("Heap objects failed: {}", error.message).into();
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

    fn close_heap_class(&mut self, cx: &mut Context<Self>) {
        self.heap_selected_class = None;
        self.heap_class_name = None;
        self.heap_objects.clear();
        self.heap_objects_total = 0;
        self.heap_path_text.clear();
        cx.notify();
    }

    fn inspect_heap_object(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(object) = self.heap_objects.get(index).map(|row| row.object) else {
            return;
        };
        let Some(snapshot) = self.heap_info.as_ref().map(|info| info.id) else {
            return;
        };
        let host = self.host.clone();
        let label = self
            .heap_objects
            .get(index)
            .map(|row| format!("{} {}", row.class_name, row.address))
            .unwrap_or_default();
        self.heap_objects_status = format!("Finding GC path for {label}…").into();
        cx.spawn(async move |this, cx| {
            let result = host.heap_path(snapshot, object).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(Some(path)) => {
                            let mut lines = Vec::new();
                            for (index, step) in path.steps.iter().enumerate() {
                                lines.push(format!(
                                    "{} {}",
                                    step.object.class_name, step.object.address
                                ));
                                if index + 1 < path.steps.len() {
                                    lines.push(format!("  ← {}", heap_edge_label(&step.via)));
                                }
                            }
                            if path.omitted > 0 {
                                lines.push(format!("  … {} steps omitted", path.omitted));
                            }
                            app.heap_path_text = lines.join("\n");
                            app.heap_objects_status =
                                format!("GC path · {} steps", path.steps.len()).into();
                        }
                        Ok(None) => {
                            app.heap_path_text.clear();
                            app.heap_objects_status = "Object is unreachable from a GC root".into();
                        }
                        Err(error) => {
                            app.heap_path_text.clear();
                            app.heap_objects_status =
                                format!("GC path failed: {}", error.message).into();
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

    fn run_duplicate_analysis(&mut self, cx: &mut Context<Self>) {
        if self.duplicates_busy {
            return;
        }
        self.duplicates_busy = true;
        self.duplicates_status = "Scanning for duplicate code…".into();
        let options = ide_analysis::DuplicateOptions {
            min_tokens: 60,
            anonymize_identifiers: self.duplicates_anonymize_identifiers,
            anonymize_literals: self.duplicates_anonymize_literals,
        };
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.analysis_duplicates(options).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.duplicates_busy = false;
                    match result {
                        Ok(report) => {
                            app.duplicates_status = format!(
                                "{} duplicate groups · {} files · {} tokens · {} ms{}",
                                report.groups.len(),
                                report.files,
                                report.tokens,
                                report.elapsed_ms,
                                if report.truncated {
                                    " · truncated"
                                } else {
                                    ""
                                }
                            )
                            .into();
                            app.duplicates_report = Some(report);
                        }
                        Err(error) => {
                            app.duplicates_report = None;
                            app.duplicates_status =
                                format!("Duplicate scan failed: {}", error.message).into();
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

    fn run_structural_search(&mut self, cx: &mut Context<Self>) {
        if self.structural_busy {
            return;
        }
        let pattern = self
            .structural_pattern_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        if pattern.is_empty() {
            self.structural_status = "Enter a structural pattern".into();
            cx.notify();
            return;
        }
        let language = self
            .structural_language_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        if language.is_empty() {
            self.structural_status = "Enter a language id".into();
            cx.notify();
            return;
        }
        let replacement = self
            .structural_replacement_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        let query = ide_analysis::StructuralQuery {
            pattern,
            language: ide_core::LanguageId::new(language),
            replacement: (!replacement.is_empty()).then_some(replacement),
            max_matches: 5_000,
        };
        let host = self.host.clone();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let events =
            ide_host::ops::Channel::new(move |event: ide_host::ops::analysis::StructuralEvent| {
                let _ = event_tx.send(event);
            });
        self.structural_busy = true;
        self.structural_files.clear();
        self.structural_status = "Searching structurally…".into();
        cx.spawn(async move |this, cx| {
            let result = host.structural_search(query, events).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.structural_busy = false;
                    match result {
                        Ok(()) => {
                            let mut summary = None;
                            while let Ok(event) = event_rx.try_recv() {
                                match event {
                                    ide_host::ops::analysis::StructuralEvent::File { file } => {
                                        app.structural_files.push(file);
                                    }
                                    ide_host::ops::analysis::StructuralEvent::Done {
                                        summary: done,
                                    } => summary = Some(done),
                                }
                            }
                            app.structural_status = summary
                                .map(|summary| {
                                    format!(
                                        "{} matches in {} files · {} ms{}",
                                        summary.matches,
                                        summary.files,
                                        summary.elapsed_ms,
                                        if summary.limit_hit {
                                            " · limit reached"
                                        } else {
                                            ""
                                        }
                                    )
                                })
                                .unwrap_or_else(|| "Structural search finished".into())
                                .into();
                        }
                        Err(error) => {
                            app.structural_files.clear();
                            app.structural_status =
                                format!("Structural search failed: {}", error.message).into();
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

    fn apply_structural_replacements(&mut self, cx: &mut Context<Self>) {
        if self.structural_busy {
            return;
        }
        let mut grouped: BTreeMap<PathBuf, Vec<ide_core::TextEdit>> = BTreeMap::new();
        for file in &self.structural_files {
            for matched in &file.matches {
                let Some(replacement) = matched.replacement.clone() else {
                    continue;
                };
                grouped
                    .entry(file.path.0.clone())
                    .or_default()
                    .push(ide_core::TextEdit {
                        range: matched.range,
                        new_text: replacement,
                    });
            }
        }
        if grouped.is_empty() {
            self.structural_status =
                "No replacement template is set for the current matches".into();
            self.structural_apply_status = self.structural_status.clone();
            cx.notify();
            return;
        }
        let replacements = grouped
            .into_iter()
            .map(|(path, edits)| crate::host::StructuralReplacement { path, edits })
            .collect::<Vec<_>>();
        let host = self.host.clone();
        self.structural_busy = true;
        self.structural_status = "Applying structural replacements…".into();
        self.structural_apply_status = "Applying structural replacements…".into();
        cx.spawn(async move |this, cx| {
            let result = host.apply_structural_replacements(replacements).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.structural_busy = false;
                    match result {
                        Ok(summary) => {
                            app.structural_apply_status = format!(
                                "Applied {} replacements in {} files{}",
                                summary.replacements,
                                summary.files,
                                if summary.failed > 0 {
                                    format!(" · {} failed", summary.failed)
                                } else {
                                    String::new()
                                }
                            )
                            .into();
                            app.structural_status = app.structural_apply_status.clone();
                            app.run_structural_search(cx);
                        }
                        Err(error) => {
                            app.structural_apply_status =
                                format!("Structural replace failed: {}", error.message).into();
                            app.structural_status = app.structural_apply_status.clone();
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

    fn run_inspect_code(&mut self, cx: &mut Context<Self>) {
        if self.inspect_busy {
            return;
        }
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let events =
            ide_host::ops::Channel::new(move |event: ide_host::ops::analysis::InspectEvent| {
                let _ = event_tx.send(event);
            });
        let host = self.host.clone();
        self.inspect_busy = true;
        self.inspect_files.clear();
        self.inspect_summary = None;
        self.inspect_status = "Inspecting project…".into();
        cx.spawn(async move |this, cx| {
            let result = host.inspect_code(events).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.inspect_busy = false;
                    match result {
                        Ok(()) => {
                            let mut started = None;
                            let mut summary = None;
                            while let Ok(event) = event_rx.try_recv() {
                                match event {
                                    ide_host::ops::analysis::InspectEvent::Started { total } => {
                                        started = Some(total)
                                    }
                                    ide_host::ops::analysis::InspectEvent::File {
                                        file, ..
                                    } => app.inspect_files.push(file),
                                    ide_host::ops::analysis::InspectEvent::Done {
                                        summary: done,
                                    } => summary = Some(done),
                                }
                            }
                            app.inspect_status = summary
                                .as_ref()
                                .map(|summary| {
                                    format!(
                                        "{} files · {} problems · {} skipped · {} failed · {} ms{}",
                                        summary.files,
                                        summary.problems,
                                        summary.skipped,
                                        summary.failed,
                                        summary.elapsed_ms,
                                        started
                                            .map(|total| format!(" · scanned {total}"))
                                            .unwrap_or_default()
                                    )
                                })
                                .unwrap_or_else(|| "Inspect Code finished".into())
                                .into();
                            app.inspect_summary = summary;
                        }
                        Err(error) => {
                            app.inspect_files.clear();
                            app.inspect_summary = None;
                            app.inspect_status =
                                format!("Inspect Code failed: {}", error.message).into();
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

    fn open_profile(&mut self, cx: &mut Context<Self>) {
        if self.profile_busy {
            return;
        }
        let raw = self.profile_dir_input.read(cx).content().trim().to_owned();
        if raw.is_empty() {
            self.profile_status = "Enter a folder containing .cpuprofile files".into();
            cx.notify();
            return;
        }
        let dir = PathBuf::from(raw);
        let dir = if dir.is_absolute() {
            dir
        } else {
            self.root.join(dir)
        };
        self.profile_status = format!("Reading profiles in {}…", dir.display()).into();
        let spec = ide_profiler::ProfileSpec::CpuProfiles {
            dir: dir.clone().into(),
            root: self.root.clone().into(),
        };
        let host = self.host.clone();
        self.profile_busy = true;
        cx.spawn(async move |this, cx| {
            let result = host.profile_collect(spec).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.profile_busy = false;
                    match result {
                        Ok(profile) => {
                            app.profile_status = format!(
                                "{} · {} frames · {} methods · {:.1} {}",
                                profile.profiler,
                                profile.frames.len(),
                                profile.methods.len(),
                                profile.total,
                                profile.unit
                            )
                            .into();
                            app.profile = Some(profile);
                        }
                        Err(error) => {
                            app.profile = None;
                            app.profile_status =
                                format!("Profile failed: {}", error.message).into();
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

    fn refresh_logcat_devices(&mut self, cx: &mut Context<Self>) {
        self.logcat_status = "Listing Android devices…".into();
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.logcat_devices().await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(devices) => {
                            app.logcat_status = if devices.is_empty() {
                                "No Android devices".into()
                            } else {
                                format!("{} Android devices", devices.len()).into()
                            };
                            app.logcat_devices = devices;
                        }
                        Err(error) => {
                            app.logcat_devices.clear();
                            app.logcat_status =
                                format!("adb devices failed: {}", error.message).into();
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

    fn start_logcat(&mut self, cx: &mut Context<Self>) {
        if self.logcat_busy {
            return;
        }
        let requested = self
            .logcat_device_input
            .read(cx)
            .content()
            .trim()
            .to_owned();
        let serial = if requested.is_empty() {
            self.logcat_devices
                .iter()
                .find(|device| device.state == "device")
                .map(|device| device.serial.clone())
        } else {
            Some(requested)
        };
        let Some(serial) = serial else {
            self.logcat_status = "Select an online Android device".into();
            cx.notify();
            return;
        };
        let cancel = ide_core::CancellationToken::new();
        self.logcat_cancel = Some(cancel.clone());
        self.logcat_busy = true;
        self.logcat_lines.clear();
        self.logcat_status = format!("Streaming logcat from {serial}…").into();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<LogcatEvent>();
        let producer = event_tx.clone();
        let host = self.host.clone();
        cx.spawn(async move |_, _| {
            let on_line: ide_host::ops::Emit<crate::logcat::LogcatLine> = Arc::new(move |line| {
                let _ = producer.send(LogcatEvent::Line(line));
            });
            let result = host
                .logcat_stream(serial, on_line, cancel)
                .await
                .map_err(|error| error.message);
            let _ = event_tx.send(LogcatEvent::Finished(result));
        })
        .detach();
        cx.spawn(async move |this, cx| {
            while let Some(event) = event_rx.recv().await {
                let handle = this
                    .update(cx, |app, cx| {
                        match event {
                            LogcatEvent::Line(line) => {
                                app.logcat_lines.push(line);
                                if app.logcat_lines.len() > 5_000 {
                                    app.logcat_lines.drain(..1_000);
                                }
                                app.logcat_status =
                                    format!("{} log lines", app.logcat_lines.len()).into();
                            }
                            LogcatEvent::Finished(result) => {
                                app.logcat_busy = false;
                                app.logcat_cancel = None;
                                app.logcat_status = match result {
                                    Ok(()) => {
                                        format!("logcat stopped · {} lines", app.logcat_lines.len())
                                            .into()
                                    }
                                    Err(error) => format!("logcat stopped: {error}").into(),
                                };
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

    fn stop_logcat(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = self.logcat_cancel.take() {
            cancel.cancel();
            self.logcat_status = "Stopping logcat…".into();
            cx.notify();
        }
    }

    fn refresh_scratches(&mut self, cx: &mut Context<Self>) {
        self.scratch_files = crate::scratch::list(&self.scratch_dir);
        self.scratch_status = format!("{} scratch files", self.scratch_files.len()).into();
        cx.notify();
    }

    fn create_scratch(&mut self, cx: &mut Context<Self>) {
        let raw = self.scratch_name_input.read(cx).content().trim().to_owned();
        let Some(name) = crate::scratch::sanitize_file_name(&raw) else {
            self.scratch_status = "Enter a flat file name without path separators".into();
            cx.notify();
            return;
        };
        let path = self.scratch_dir.join(&name);
        let created = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(error) => {
                self.scratch_status = format!("Could not create scratch: {error}").into();
                cx.notify();
                return;
            }
        };
        self.scratch_status = if created {
            format!("Created {name}").into()
        } else {
            format!("Opened existing {name}").into()
        };
        self.scratch_name_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.refresh_scratches(cx);
        self.open_path(path, cx);
    }

    fn open_scratch(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(file) = self.scratch_files.get(index).cloned() else {
            return;
        };
        self.scratch_status = format!("Opened {}", file.name).into();
        self.open_path(file.path, cx);
    }

    fn record_recent_file(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        self.recent_store.record(&self.root, path, 50);
        self.recent_files = self.recent_store.project(&self.root);
        self.recent_status = match self.recent_store.save(&self.recent_file_path) {
            Ok(()) => format!("{} recent files", self.recent_files.len()).into(),
            Err(error) => format!("Recent files save failed: {error}").into(),
        };
        if self.sidebar_mode == SidebarMode::Recent {
            cx.notify();
        }
    }

    fn refresh_recent(&mut self, cx: &mut Context<Self>) {
        self.recent_files = self.recent_store.project(&self.root);
        self.recent_status = format!("{} recent files", self.recent_files.len()).into();
        cx.notify();
    }

    fn open_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(path) = self.recent_files.get(index).cloned() else {
            return;
        };
        self.open_path(path, cx);
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

    pub fn smoke_mcp_transport_roundtrip(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Mcp, cx);
        for (name, transport) in [
            ("smoke-http", ide_agent::McpTransport::Http),
            ("smoke-sse", ide_agent::McpTransport::Sse),
        ] {
            self.new_mcp_server(cx);
            self.set_mcp_transport(transport, cx);
            self.mcp_name_input.update(cx, |input, cx| {
                input.set_content(name, cx);
            });
            self.mcp_url_input.update(cx, |input, cx| {
                input.set_content("https://mcp.example.test/mcp", cx);
            });
            self.save_mcp_server(cx);
            let Some(server) = self
                .mcp_servers
                .iter()
                .find(|server| server.name == name)
                .cloned()
            else {
                eprintln!(
                    "WORKBENCH_MCP_TRANSPORT name={name} saved=false status={}",
                    self.mcp_status
                );
                continue;
            };
            eprintln!(
                "WORKBENCH_MCP_TRANSPORT name={name} transport={} url={:?} command={:?}",
                mcp_transport_label(server.transport),
                server.url,
                server.command
            );
            self.mcp_selected = Some(server.id);
            self.remove_selected_mcp_server(cx);
        }
        eprintln!(
            "WORKBENCH_MCP_TRANSPORT remaining={}",
            self.mcp_servers.len()
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
            SidebarMode::Todo => "TODO",
            SidebarMode::Dependencies => "Dependencies",
            SidebarMode::Coverage => "Coverage",
            SidebarMode::Bookmarks => "Bookmarks",
            SidebarMode::Git => "Git",
            SidebarMode::GitLog => "Git Log",
            SidebarMode::Services => "Services",
            SidebarMode::GitHub => "GitHub",
            SidebarMode::Heap => "Heap",
            SidebarMode::Profiler => "Profiler",
            SidebarMode::Logcat => "Logcat",
            SidebarMode::Scratch => "Scratch",
            SidebarMode::Recent => "Recent",
            SidebarMode::Analysis => "Analysis",
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

    pub fn smoke_find_usages(
        &mut self,
        line: u32,
        column: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(doc) = self.active {
            self.goto_position(doc, line, column, window, cx);
        }
        self.find_usages_at_cursor(cx);
    }

    pub fn reference_count(&self) -> usize {
        self.references.len()
    }

    pub fn references_status(&self) -> &gpui::SharedString {
        &self.references_status
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

    pub fn smoke_debug_attach(&mut self, cx: &mut Context<Self>) {
        self.debug_selected = 0;
        self.attach_debug_process(cx);
    }

    pub fn debug_status_text(&self) -> &gpui::SharedString {
        &self.debug_status
    }

    pub fn smoke_breakpoint(&mut self, cx: &mut Context<Self>) {
        self.bottom_mode = BottomMode::Debug;
        self.toggle_breakpoint_at_cursor(cx);
    }

    pub fn breakpoint_count(&self) -> usize {
        self.debug_breakpoints.values().map(BTreeSet::len).sum()
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

    pub fn smoke_notebook_add_markdown(&mut self, cx: &mut Context<Self>) {
        self.add_notebook_cell("markdown", cx);
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
        self.search_pattern = pattern.to_owned();
        self.search_input.update(cx, |input, cx| {
            input.set_content(pattern, cx);
        });
        self.start_search(pattern.to_owned(), cx);
    }

    pub fn smoke_search_status(&self) -> String {
        self.search_status.to_string()
    }

    pub fn smoke_search_files(&self) -> usize {
        self.search_results.len()
    }

    pub fn smoke_replace(&mut self, replacement: String, cx: &mut Context<Self>) {
        self.search_replacement = replacement.clone();
        self.search_replace_input.update(cx, |input, cx| {
            input.set_content(replacement, cx);
        });
        self.replace_search_results(cx);
    }

    pub fn search_status(&self) -> String {
        self.search_status.to_string()
    }

    pub fn smoke_todo(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Todo, cx);
    }

    pub fn todo_count(&self) -> usize {
        self.todo_items.len()
    }

    pub fn todo_status(&self) -> String {
        self.todo_status.to_string()
    }

    pub fn todo_first(&self) -> String {
        self.todo_items
            .first()
            .map(|item| format!("{}:{}", item.rel_path, item.line + 1))
            .unwrap_or_default()
    }

    pub fn smoke_dependencies(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Dependencies, cx);
    }

    pub fn dependency_module_count(&self) -> usize {
        self.dependency_modules.len()
    }

    pub fn dependency_library_count(&self) -> usize {
        self.dependency_coordinates.len()
    }

    pub fn dependency_status(&self) -> String {
        self.dependency_status.to_string()
    }

    pub fn dependency_first_module(&self) -> String {
        self.dependency_modules
            .first()
            .map(|module| module.label.clone())
            .unwrap_or_default()
    }

    pub fn dependency_first_library(&self) -> String {
        self.dependency_coordinates
            .first()
            .map(crate::build_tools::coordinate_label)
            .unwrap_or_default()
    }

    pub fn smoke_coverage(&mut self, path: String, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Coverage, cx);
        self.coverage_path_input.update(cx, |input, cx| {
            input.set_content(path, cx);
        });
        self.import_coverage(cx);
    }

    pub fn coverage_file_count(&self) -> usize {
        self.coverage_rows.len()
    }

    pub fn coverage_status(&self) -> String {
        self.coverage_status.to_string()
    }

    pub fn coverage_first(&self) -> String {
        self.coverage_rows
            .first()
            .map(|row| row.rel_path.clone())
            .unwrap_or_default()
    }

    pub fn smoke_bookmark(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Bookmarks, cx);
        self.toggle_bookmark(cx);
    }

    pub fn bookmark_count(&self) -> usize {
        self.bookmarks.len()
    }

    pub fn bookmark_status(&self) -> String {
        self.bookmarks_status.to_string()
    }

    pub fn bookmark_first(&self) -> String {
        self.bookmarks
            .first()
            .map(|bookmark| format!("{}:{}", bookmark.path, bookmark.line + 1))
            .unwrap_or_default()
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

    pub fn smoke_git_log(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::GitLog, cx);
    }

    pub fn git_log_count(&self) -> usize {
        self.git_commits.len()
    }

    pub fn git_log_status(&self) -> String {
        self.git_log_status.to_string()
    }

    pub fn git_log_first(&self) -> String {
        self.git_commits
            .first()
            .map(|commit| format!("{} {}", commit.short_id, commit.summary))
            .unwrap_or_default()
    }

    pub fn smoke_services(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Services, cx);
    }

    pub fn docker_container_count(&self) -> usize {
        self.docker_snapshot
            .as_ref()
            .map(|snapshot| snapshot.containers.len())
            .unwrap_or_default()
    }

    pub fn docker_image_count(&self) -> usize {
        self.docker_snapshot
            .as_ref()
            .map(|snapshot| snapshot.images.len())
            .unwrap_or_default()
    }

    pub fn docker_status(&self) -> String {
        self.docker_status.to_string()
    }

    pub fn docker_first_container(&self) -> String {
        self.docker_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.containers.first())
            .map(|container| {
                format!(
                    "{} · {} · {}",
                    container.name, container.image, container.state
                )
            })
            .unwrap_or_default()
    }

    pub fn smoke_github(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::GitHub, cx);
    }

    pub fn github_pull_count(&self) -> usize {
        self.github_pulls.len()
    }

    pub fn github_status_text(&self) -> String {
        self.github_status.to_string()
    }

    pub fn github_first_pull(&self) -> String {
        self.github_pulls
            .first()
            .map(|pull| format!("#{} {}", pull.number, pull.title))
            .unwrap_or_default()
    }

    pub fn smoke_github_pr(&mut self, number: u32, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::GitHub, cx);
        self.open_github_pr(number, cx);
    }

    pub fn github_detail_summary(&self) -> String {
        self.github_details
            .as_ref()
            .map(|details| {
                format!(
                    "#{} {} · {} files · {} checks",
                    details.summary.number,
                    details.summary.title,
                    details.files.len(),
                    details.checks.len()
                )
            })
            .unwrap_or_default()
    }

    pub fn github_comment_count(&self) -> usize {
        self.github_comments.len()
    }

    pub fn github_detail_status_text(&self) -> String {
        self.github_detail_status.to_string()
    }

    pub fn github_timeline_count(&self) -> usize {
        self.github_details
            .as_ref()
            .map(|details| details.timeline.len())
            .unwrap_or_default()
    }

    pub fn github_file_count(&self) -> usize {
        self.github_details
            .as_ref()
            .map(|details| details.files.len())
            .unwrap_or_default()
    }

    pub fn github_review_status_text(&self) -> String {
        self.github_review_status.to_string()
    }

    pub fn github_diff_file_count(&self) -> usize {
        self.github_diff_files.len()
    }

    pub fn github_diff_line_count(&self) -> usize {
        self.github_diff_files
            .iter()
            .map(|file| file.lines.len())
            .sum()
    }

    pub fn github_diff_status_text(&self) -> String {
        self.github_diff_status.to_string()
    }

    pub fn smoke_github_diff(&mut self, cx: &mut Context<Self>) {
        self.github_tab = GitHubTab::Diff;
        self.load_github_diff(cx);
    }

    pub fn smoke_github_review(&mut self, event: &str, body: String, cx: &mut Context<Self>) {
        self.github_comment_input.update(cx, |input, cx| {
            input.set_content(body, cx);
        });
        let event = match event {
            "approve" => ide_github::ReviewEvent::Approve,
            "request-changes" => ide_github::ReviewEvent::RequestChanges,
            _ => ide_github::ReviewEvent::Comment,
        };
        self.submit_github_review(event, cx);
    }

    pub fn smoke_heap(&mut self, path: String, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Heap, cx);
        self.heap_path_input.update(cx, |input, cx| {
            input.set_content(path, cx);
        });
        self.open_heap_snapshot(cx);
    }

    pub fn heap_class_count(&self) -> usize {
        self.heap_classes.len()
    }

    pub fn heap_object_count(&self) -> u64 {
        self.heap_info
            .as_ref()
            .map(|info| info.summary.objects)
            .unwrap_or_default()
    }

    pub fn heap_status_text(&self) -> String {
        self.heap_status.to_string()
    }

    pub fn heap_first_class(&self) -> String {
        self.heap_classes
            .first()
            .map(|class| format!("{} · {}", class.name, class.count))
            .unwrap_or_default()
    }

    pub fn smoke_heap_objects(&mut self, cx: &mut Context<Self>) {
        if !self.heap_classes.is_empty() {
            self.open_heap_class(0, cx);
        }
    }

    pub fn heap_object_row_count(&self) -> usize {
        self.heap_objects.len()
    }

    pub fn heap_objects_status_text(&self) -> String {
        self.heap_objects_status.to_string()
    }

    pub fn heap_first_object(&self) -> String {
        self.heap_objects
            .first()
            .map(|object| format!("{} {}", object.class_name, object.address))
            .unwrap_or_default()
    }

    pub fn smoke_profile(&mut self, dir: String, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Profiler, cx);
        self.profile_dir_input.update(cx, |input, cx| {
            input.set_content(dir, cx);
        });
        self.open_profile(cx);
    }

    pub fn profile_method_count(&self) -> usize {
        self.profile
            .as_ref()
            .map(|profile| profile.methods.len())
            .unwrap_or_default()
    }

    pub fn profile_status_text(&self) -> String {
        self.profile_status.to_string()
    }

    pub fn profile_first_method(&self) -> String {
        self.profile
            .as_ref()
            .and_then(|profile| {
                profile
                    .methods
                    .first()
                    .and_then(|method| profile.frames.get(method.frame as usize))
            })
            .map(|frame| frame.name.clone())
            .unwrap_or_default()
    }

    pub fn smoke_logcat(&mut self, serial: String, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Logcat, cx);
        if !serial.is_empty() {
            self.logcat_device_input.update(cx, |input, cx| {
                input.set_content(serial, cx);
            });
        }
        self.start_logcat(cx);
    }

    pub fn logcat_line_count(&self) -> usize {
        self.logcat_lines.len()
    }

    pub fn logcat_status_text(&self) -> String {
        self.logcat_status.to_string()
    }

    pub fn logcat_first_line(&self) -> String {
        self.logcat_lines
            .first()
            .map(|line| format!("{} {}: {}", line.level.letter(), line.tag, line.message))
            .unwrap_or_default()
    }

    pub fn smoke_scratch(&mut self, name: String, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Scratch, cx);
        self.scratch_name_input.update(cx, |input, cx| {
            input.set_content(name, cx);
        });
        self.create_scratch(cx);
    }

    pub fn scratch_count(&self) -> usize {
        self.scratch_files.len()
    }

    pub fn scratch_status_text(&self) -> String {
        self.scratch_status.to_string()
    }

    pub fn scratch_first(&self) -> String {
        self.scratch_files
            .first()
            .map(|file| file.name.clone())
            .unwrap_or_default()
    }

    pub fn smoke_recent(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Recent, cx);
    }

    pub fn recent_count(&self) -> usize {
        self.recent_files.len()
    }

    pub fn recent_status_text(&self) -> String {
        self.recent_status.to_string()
    }

    pub fn recent_first(&self) -> String {
        self.recent_files
            .first()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn smoke_save_as(&mut self, path: String, cx: &mut Context<Self>) {
        self.save_as_input.update(cx, |input, cx| {
            input.set_content(path, cx);
        });
        self.save_as = true;
        self.perform_save_as(cx);
    }

    pub fn active_path(&self) -> String {
        self.active
            .and_then(|doc| self.documents.get(&doc))
            .map(|document| document.path.display().to_string())
            .unwrap_or_default()
    }

    pub fn status_text(&self) -> String {
        self.status.to_string()
    }

    pub fn smoke_analysis(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Analysis, cx);
        self.run_duplicate_analysis(cx);
    }

    pub fn duplicate_group_count(&self) -> usize {
        self.duplicates_report
            .as_ref()
            .map(|report| report.groups.len())
            .unwrap_or_default()
    }

    pub fn duplicate_status_text(&self) -> String {
        self.duplicates_status.to_string()
    }

    pub fn duplicate_first_fragment(&self) -> String {
        self.duplicates_report
            .as_ref()
            .and_then(|report| report.groups.first())
            .and_then(|group| group.fragments.first())
            .map(|fragment| format!("{}:{}", fragment.rel_path, fragment.range.start.line + 1))
            .unwrap_or_default()
    }

    pub fn smoke_structural(&mut self, pattern: String, language: String, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Analysis, cx);
        self.analysis_tab = AnalysisTab::Structural;
        self.structural_language_input.update(cx, |input, cx| {
            input.set_content(language, cx);
        });
        self.structural_pattern_input.update(cx, |input, cx| {
            input.set_content(pattern, cx);
        });
        self.run_structural_search(cx);
    }

    pub fn smoke_structural_replacement(&mut self, replacement: String, cx: &mut Context<Self>) {
        self.structural_replacement_input.update(cx, |input, cx| {
            input.set_content(replacement, cx);
        });
    }

    pub fn smoke_apply_structural_replacements(&mut self, cx: &mut Context<Self>) {
        self.apply_structural_replacements(cx);
    }

    pub fn structural_match_count(&self) -> usize {
        self.structural_files
            .iter()
            .map(|file| file.matches.len())
            .sum()
    }

    pub fn structural_status_text(&self) -> String {
        self.structural_status.to_string()
    }

    pub fn structural_apply_status_text(&self) -> String {
        self.structural_apply_status.to_string()
    }

    pub fn structural_first(&self) -> String {
        self.structural_files
            .first()
            .and_then(|file| file.matches.first().map(|matched| (file, matched)))
            .map(|(file, matched)| format!("{}:{}", file.rel_path, matched.range.start.line + 1))
            .unwrap_or_default()
    }

    pub fn smoke_inspect(&mut self, cx: &mut Context<Self>) {
        self.select_sidebar(SidebarMode::Analysis, cx);
        self.analysis_tab = AnalysisTab::Inspect;
        self.run_inspect_code(cx);
    }

    pub fn inspect_diagnostic_count(&self) -> usize {
        self.inspect_files
            .iter()
            .map(|file| file.diagnostics.len())
            .sum()
    }

    pub fn inspect_status_text(&self) -> String {
        self.inspect_status.to_string()
    }

    pub fn inspect_first(&self) -> String {
        self.inspect_files
            .iter()
            .find_map(|file| {
                file.diagnostics.first().map(|diagnostic| {
                    format!("{}:{}", file.rel_path, diagnostic.range.start.line + 1)
                })
            })
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
            IdeEvent::Debug { session, event } => match event {
                ide_core::event::DebugEvent::Started { name, adapter, .. } => {
                    self.debug_console
                        .push_str(&format!("== {name} ({adapter}) ==\n"));
                    self.debug_status = format!("{name} started").into();
                    if self.debug_active_session.is_none() {
                        self.debug_active_session = Some(session);
                    }
                    self.debug_sessions = self.host.debug_sessions().unwrap_or_default();
                }
                ide_core::event::DebugEvent::Output { output, category } => {
                    if !output.is_empty() {
                        self.debug_console
                            .push_str(&format!("[{category}] {output}\n"));
                        if self.debug_console.len() > 2_000_000 {
                            let split = self.debug_console.len().saturating_sub(1_800_000);
                            self.debug_console.drain(..split);
                        }
                    }
                }
                ide_core::event::DebugEvent::Stopped {
                    thread_id, reason, ..
                } => {
                    self.debug_status = format!("Stopped: {reason}").into();
                    self.refresh_debug_stack(session, thread_id, cx);
                }
                ide_core::event::DebugEvent::Continued { .. } => {
                    self.debug_status = "Running".into();
                    self.debug_frames.clear();
                    self.debug_scopes.clear();
                    self.debug_variables.clear();
                }
                ide_core::event::DebugEvent::Terminated => {
                    self.debug_status = "Terminated".into();
                    self.debug_active_session = None;
                    self.debug_thread_id = None;
                    self.debug_frames.clear();
                    self.debug_scopes.clear();
                    self.debug_variables.clear();
                    self.debug_sessions = self.host.debug_sessions().unwrap_or_default();
                }
                ide_core::event::DebugEvent::Exited { code } => {
                    self.debug_status = format!("Exited with {code}").into();
                }
                _ => {}
            },
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

    fn render_save_as(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.save_as {
            return div().into_any_element();
        }
        let theme = self.theme;
        div()
            .absolute()
            .top(px(90.))
            .left(px(360.))
            .w(px(520.))
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_1()
            .border_color(theme.accent)
            .rounded_md()
            .shadow_lg()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_size(px(11.))
                    .text_color(theme.text)
                    .border_b_1()
                    .border_color(theme.border)
                    .child("Save a copy"),
            )
            .child(
                div()
                    .h(px(34.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.save_as_input.clone()),
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
                            .id("save-as-confirm")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.perform_save_as(cx);
                            }))
                            .child("Save"),
                    )
                    .child(
                        div()
                            .id("save-as-cancel")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.save_as = false;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .child("Enter saves · Esc cancels"),
                    ),
            )
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

    fn render_references(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.references_open {
            return div().into_any_element();
        }
        let theme = self.theme;
        let mut rows = div()
            .id("references-list")
            .flex_1()
            .min_h(px(0.))
            .max_h(px(360.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, location) in self.references.iter().take(200).enumerate() {
            let selected = index == self.references_selected;
            let path = location.path.0.clone();
            let line = location.range.start.line;
            let column = location.range.start.col;
            let file = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            rows = rows.child(
                div()
                    .id(("reference", index))
                    .h(px(24.))
                    .px_2()
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
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.references_selected = index;
                        this.references_open = false;
                        this.open_or_goto(path.clone(), line, column, window, cx);
                    }))
                    .child(format!("{file}:{}:{}", line + 1, column + 1)),
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
                    .h(px(32.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .child(self.references_status.clone()),
                    )
                    .child(
                        div()
                            .id("references-close")
                            .text_size(px(10.))
                            .text_color(theme.faint)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.references_open = false;
                                cx.notify();
                            }))
                            .child("close"),
                    ),
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

    fn render_remote_editor(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(editor) = &self.remote_editor else {
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
                    .child(self.remote_edit_status.clone()),
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
                            .id("remote-edit-save")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.commit_remote_editor(cx);
                            }))
                            .child("Write remote file"),
                    )
                    .child(
                        div()
                            .id("remote-edit-cancel")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.remote_editor = None;
                                this.remote_edit_buffer = None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    ),
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
                    .when(self.remote_preview_path.is_some(), |row| {
                        row.child(
                            div()
                                .id("ssh-edit")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.))
                                .text_color(theme.text)
                                .bg(theme.accent_muted)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.open_remote_editor(window, cx);
                                }))
                                .child("Edit"),
                        )
                    })
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
                let edit_id = cell.id.clone();
                let type_id = cell.id.clone();
                let delete_id = cell.id.clone();
                let cell_type = cell.cell_type.clone();
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
                                                edit_id.clone(),
                                                source.clone(),
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .child("Edit"),
                            )
                            .child(
                                div()
                                    .id(("notebook-type", index))
                                    .px_1()
                                    .text_size(px(9.))
                                    .text_color(theme.muted)
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        let other = if cell_type == "code" {
                                            "markdown"
                                        } else {
                                            "code"
                                        };
                                        this.set_notebook_cell_type(type_id.clone(), other, cx);
                                    }))
                                    .child("type"),
                            )
                            .child(
                                div()
                                    .id(("notebook-delete", index))
                                    .px_1()
                                    .text_size(px(9.))
                                    .text_color(theme.error)
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.delete_notebook_cell(delete_id.clone(), cx);
                                    }))
                                    .child("delete"),
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
                    .child(
                        div()
                            .id("notebook-add-code")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.add_notebook_cell("code", cx);
                            }))
                            .child("+ Code"),
                    )
                    .child(
                        div()
                            .id("notebook-add-markdown")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.add_notebook_cell("markdown", cx);
                            }))
                            .child("+ Markdown"),
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
        let mut transport_row = div().flex().items_center().gap(px(3.));
        for (index, transport) in [
            ide_agent::McpTransport::Stdio,
            ide_agent::McpTransport::Http,
            ide_agent::McpTransport::Sse,
        ]
        .into_iter()
        .enumerate()
        {
            let selected = self.mcp_transport == transport;
            transport_row = transport_row.child(
                div()
                    .id(("mcp-transport", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_size(px(9.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.accent_muted
                    } else {
                        theme.panel_alt
                    })
                    .border_1()
                    .border_color(if selected { theme.accent } else { theme.border })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_mcp_transport(transport, cx);
                    }))
                    .child(mcp_transport_label(transport)),
            );
        }
        let is_stdio = self.mcp_transport == ide_agent::McpTransport::Stdio;
        let save_label = format!(
            "{} {} server",
            if self.mcp_selected.is_some() {
                "Update"
            } else {
                "Add"
            },
            mcp_transport_label(self.mcp_transport).to_uppercase()
        );
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
            let scope = format!("{:?}", server.scope).to_lowercase();
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
                            .child(format!(
                                "{} · {}",
                                mcp_transport_label(server.transport),
                                scope
                            )),
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
                    .child(transport_row)
                    .when(is_stdio, |form| {
                        form.child(self.mcp_command_input.clone())
                            .child(self.mcp_args_input.clone())
                    })
                    .when(!is_stdio, |form| form.child(self.mcp_url_input.clone()))
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
                            .child(save_label),
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
                        this.plugin_consent_optional.clear();
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
                let optional = capability.optional;
                let id = capability.id.clone();
                let selected = self.plugin_consent_optional.contains(&id);
                let label = format!(
                    "{} {} · {:?}{}",
                    if optional {
                        if selected { "[x]" } else { "[ ]" }
                    } else {
                        "[required]"
                    },
                    id,
                    capability.risk,
                    if optional { "" } else { " · required" }
                );
                preview_panel = preview_panel.child(
                    div()
                        .id(SharedString::from(format!("capability-{id}")))
                        .text_size(px(9.))
                        .text_color(if optional { theme.muted } else { theme.faint })
                        .when(optional, |row| {
                            row.cursor_pointer().on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    if this.plugin_consent_optional.contains(&id) {
                                        this.plugin_consent_optional.remove(&id);
                                    } else {
                                        this.plugin_consent_optional.insert(id.clone());
                                    }
                                    cx.notify();
                                },
                            ))
                        })
                        .child(label),
                );
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

    fn render_git_log(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("git-log")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.git_commits.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(if self.git_log_status.starts_with("Loading") {
                        "Loading history…"
                    } else {
                        "No commits"
                    }),
            );
        }
        for (index, commit) in self.git_commits.iter().take(200).enumerate() {
            let refs = if commit.refs.is_empty() {
                String::new()
            } else {
                format!("  [{}]", commit.refs.join(", "))
            };
            rows = rows.child(
                div()
                    .id(("git-log-row", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .border_b_1()
                    .border_color(theme.border)
                    .hover(|style| style.bg(theme.panel_alt))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .text_color(theme.accent)
                                    .child(commit.short_id.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_size(px(10.5))
                                    .text_color(theme.text)
                                    .child(format!("{}{refs}", commit.summary)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .child(commit.author.clone())
                            .child(div().flex_1())
                            .child(if commit.parents.len() > 1 {
                                "merge"
                            } else {
                                ""
                            }),
                    ),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("git-log-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_git_log(cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.git_log_status.clone()),
                    ),
            )
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.git_log_filter_input.clone()),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_services(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let snapshot = self.docker_snapshot.as_ref();
        let ready = snapshot.is_some_and(|snapshot| {
            matches!(
                snapshot.availability,
                ide_docker::DockerAvailability::Ready { .. }
            )
        });
        let mut rows = div()
            .id("docker-services")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if !ready {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(self.docker_status.clone()),
            );
        } else if let Some(snapshot) = snapshot {
            match self.docker_tab {
                DockerTab::Containers => {
                    if snapshot.containers.is_empty() {
                        rows = rows.child(
                            div()
                                .p_3()
                                .text_size(px(10.5))
                                .text_color(theme.faint)
                                .child("No containers"),
                        );
                    }
                    for (index, container) in snapshot.containers.iter().take(200).enumerate() {
                        let running = container.running();
                        let state_color = if running { theme.accent } else { theme.faint };
                        let id = container.id.clone();
                        let name = container.name.clone();
                        let action = if running {
                            ide_docker::ContainerOp::Stop
                        } else {
                            ide_docker::ContainerOp::Start
                        };
                        let action_label = if running { "Stop" } else { "Start" };
                        rows = rows.child(
                            div()
                                .id(("docker-container", index))
                                .px_2()
                                .py_1()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .border_b_1()
                                .border_color(theme.border)
                                .hover(|style| style.bg(theme.panel_alt))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .text_size(px(10.5))
                                        .child(
                                            div()
                                                .w(px(6.))
                                                .h(px(6.))
                                                .rounded_full()
                                                .bg(state_color),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .truncate()
                                                .text_color(theme.text)
                                                .child(container.name.clone()),
                                        )
                                        .when_some(
                                            container.compose_project.clone(),
                                            |row, project| {
                                                row.child(
                                                    div()
                                                        .text_size(px(8.5))
                                                        .text_color(theme.faint)
                                                        .child(project),
                                                )
                                            },
                                        )
                                        .child(
                                            div()
                                                .id(("docker-container-action", index))
                                                .text_size(px(9.))
                                                .text_color(theme.muted)
                                                .cursor_pointer()
                                                .on_click(cx.listener(
                                                    move |this, _: &ClickEvent, _, cx| {
                                                        this.docker_container_action(
                                                            id.clone(),
                                                            name.clone(),
                                                            action,
                                                            cx,
                                                        );
                                                    },
                                                ))
                                                .child(action_label),
                                        ),
                                )
                                .child(
                                    div()
                                        .pl(px(14.))
                                        .text_size(px(9.))
                                        .text_color(theme.faint)
                                        .truncate()
                                        .child(format!(
                                            "{} · {}{}",
                                            container.image,
                                            container.status,
                                            if container.ports.is_empty() {
                                                String::new()
                                            } else {
                                                format!(" · {}", container.ports)
                                            }
                                        )),
                                ),
                        );
                    }
                }
                DockerTab::Images => {
                    if snapshot.images.is_empty() {
                        rows = rows.child(
                            div()
                                .p_3()
                                .text_size(px(10.5))
                                .text_color(theme.faint)
                                .child("No images"),
                        );
                    }
                    for (index, image) in snapshot.images.iter().take(200).enumerate() {
                        rows = rows.child(
                            div()
                                .id(("docker-image", index))
                                .px_2()
                                .py_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_size(px(10.))
                                .border_b_1()
                                .border_color(theme.border)
                                .hover(|style| style.bg(theme.panel_alt))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .text_color(theme.text)
                                        .child(image.reference()),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.))
                                        .text_color(theme.faint)
                                        .child(image.size.clone()),
                                ),
                        );
                    }
                }
                DockerTab::Volumes => {
                    if snapshot.volumes.is_empty() {
                        rows = rows.child(
                            div()
                                .p_3()
                                .text_size(px(10.5))
                                .text_color(theme.faint)
                                .child("No volumes"),
                        );
                    }
                    for (index, volume) in snapshot.volumes.iter().take(200).enumerate() {
                        rows = rows.child(
                            div()
                                .id(("docker-volume", index))
                                .px_2()
                                .py_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_size(px(10.))
                                .border_b_1()
                                .border_color(theme.border)
                                .hover(|style| style.bg(theme.panel_alt))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .text_color(theme.text)
                                        .child(volume.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.))
                                        .text_color(theme.faint)
                                        .child(volume.driver.clone()),
                                ),
                        );
                    }
                }
                DockerTab::Networks => {
                    if snapshot.networks.is_empty() {
                        rows = rows.child(
                            div()
                                .p_3()
                                .text_size(px(10.5))
                                .text_color(theme.faint)
                                .child("No networks"),
                        );
                    }
                    for (index, network) in snapshot.networks.iter().take(200).enumerate() {
                        rows = rows.child(
                            div()
                                .id(("docker-network", index))
                                .px_2()
                                .py_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_size(px(10.))
                                .border_b_1()
                                .border_color(theme.border)
                                .hover(|style| style.bg(theme.panel_alt))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .text_color(theme.text)
                                        .child(network.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.))
                                        .text_color(theme.faint)
                                        .child(format!("{} · {}", network.driver, network.scope)),
                                ),
                        );
                    }
                }
            }
        }
        let counts = snapshot
            .map(|snapshot| {
                (
                    snapshot.containers.len(),
                    snapshot.images.len(),
                    snapshot.volumes.len(),
                    snapshot.networks.len(),
                )
            })
            .unwrap_or_default();
        let tabs = [
            (DockerTab::Containers, format!("C {}", counts.0)),
            (DockerTab::Images, format!("I {}", counts.1)),
            (DockerTab::Volumes, format!("V {}", counts.2)),
            (DockerTab::Networks, format!("N {}", counts.3)),
        ];
        let mut tab_row = div().flex().items_center().gap(px(3.));
        for (index, (tab, label)) in tabs.into_iter().enumerate() {
            let selected = self.docker_tab == tab;
            tab_row = tab_row.child(
                div()
                    .id(("docker-tab", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_size(px(9.))
                    .text_color(if selected { theme.text } else { theme.faint })
                    .bg(if selected {
                        theme.accent_muted
                    } else {
                        theme.panel_alt
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.docker_tab = tab;
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("docker-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.docker_busy {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_docker(cx);
                            }))
                            .child(if self.docker_busy {
                                "Loading…"
                            } else {
                                "Refresh"
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.docker_status.clone()),
                    ),
            )
            .child(div().px_2().py_1().child(tab_row))
            .child(rows)
            .into_any_element()
    }

    fn render_github(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        if self.github_selected.is_some() {
            return self.render_github_detail(cx);
        }
        let ready = self
            .github_availability
            .as_ref()
            .is_some_and(|availability| {
                matches!(availability, ide_github::GhAvailability::Ready { .. })
            });
        let mut rows = div()
            .id("github-pulls")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if !ready {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(self.github_status.clone()),
            );
        } else if self.github_pulls.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(if self.github_busy {
                        "Loading pull requests…"
                    } else {
                        "No pull requests"
                    }),
            );
        }
        for (index, pull) in self.github_pulls.iter().take(200).enumerate() {
            let number = pull.number;
            let state_color = match pull.state {
                ide_github::PrState::Open => theme.accent,
                ide_github::PrState::Merged => theme.accent,
                ide_github::PrState::Closed => theme.faint,
            };
            rows = rows.child(
                div()
                    .id(("github-pull", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .border_b_1()
                    .border_color(theme.border)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.open_github_pr(number, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(10.5))
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(state_color)
                                    .child(format!("#{}", pull.number)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_color(theme.text)
                                    .child(if pull.is_draft {
                                        format!("Draft: {}", pull.title)
                                    } else {
                                        pull.title.clone()
                                    }),
                            )
                            .when_some(pull.review_decision.clone(), |row, decision| {
                                row.child(
                                    div()
                                        .text_size(px(8.5))
                                        .text_color(theme.warning)
                                        .child(decision),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .child(pull.author.clone())
                            .child(
                                div()
                                    .flex_1()
                                    .truncate()
                                    .child(format!("{} → {}", pull.head_ref, pull.base_ref)),
                            )
                            .child(format!("+{} −{}", pull.additions, pull.deletions)),
                    ),
            );
        }
        let tabs = [
            (ide_github::PrStateFilter::Open, "Open"),
            (ide_github::PrStateFilter::Closed, "Closed"),
            (ide_github::PrStateFilter::Merged, "Merged"),
            (ide_github::PrStateFilter::All, "All"),
        ];
        let mut tab_row = div().flex().items_center().gap(px(3.));
        for (index, (tab, label)) in tabs.into_iter().enumerate() {
            let selected = self.github_state == tab;
            tab_row = tab_row.child(
                div()
                    .id(("github-tab", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_size(px(9.))
                    .text_color(if selected { theme.text } else { theme.faint })
                    .bg(if selected {
                        theme.accent_muted
                    } else {
                        theme.panel_alt
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.github_state = tab;
                        this.refresh_github(cx);
                    }))
                    .child(label),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("github-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.github_busy {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_github(cx);
                            }))
                            .child(if self.github_busy {
                                "Loading…"
                            } else {
                                "Refresh"
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.github_status.clone()),
                    ),
            )
            .child(div().px_2().py_1().child(tab_row))
            .child(rows)
            .into_any_element()
    }

    fn render_github_detail(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let Some(details) = self.github_details.as_ref() else {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(30.))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .id("github-detail-back")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(9.5))
                                .text_color(theme.muted)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.close_github_pr(cx);
                                }))
                                .child("← Pull requests"),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(px(9.))
                                .text_color(theme.faint)
                                .child(self.github_detail_status.clone()),
                        ),
                )
                .child(
                    div()
                        .p_3()
                        .text_size(px(10.5))
                        .text_color(theme.faint)
                        .child(self.github_detail_status.clone()),
                )
                .into_any_element();
        };
        let summary = &details.summary;
        let browser_url = summary.url.clone();
        let mut body = div()
            .id("github-detail-body")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p_2()
            .flex()
            .flex_col()
            .gap(px(5.));
        match self.github_tab {
            GitHubTab::Overview => {
                body = body
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text)
                            .child(summary.title.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .child(format!(
                                "#{} · {} · {} → {}",
                                summary.number, summary.author, summary.head_ref, summary.base_ref
                            ))
                            .child(format!("+{} −{}", summary.additions, summary.deletions)),
                    )
                    .when_some(summary.review_decision.clone(), |body, decision| {
                        body.child(
                            div()
                                .text_size(px(9.))
                                .text_color(theme.warning)
                                .child(decision),
                        )
                    });
                if !summary.labels.is_empty() {
                    body = body.child(div().flex().flex_wrap().gap_1().children(
                        summary.labels.iter().map(|label| {
                            div()
                                .px_1()
                                .rounded_sm()
                                .bg(theme.panel_alt)
                                .text_size(px(8.5))
                                .text_color(theme.muted)
                                .child(label.name.clone())
                        }),
                    ));
                }
                if !details.body.trim().is_empty() {
                    body = body.child(
                        div()
                            .p_2()
                            .rounded_sm()
                            .bg(theme.panel_alt)
                            .text_size(px(10.5))
                            .text_color(theme.muted)
                            .child(details.body.clone()),
                    );
                }
                if !details.reviewers.is_empty() {
                    body = body.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .text_size(px(9.5))
                            .text_color(theme.muted)
                            .child("Reviewers")
                            .children(details.reviewers.iter().map(|reviewer| {
                                div().child(format!("{} · {}", reviewer.login, reviewer.state))
                            })),
                    );
                }
                if !details.commits.is_empty() {
                    body = body.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .text_size(px(9.5))
                            .text_color(theme.muted)
                            .child("Commits")
                            .children(details.commits.iter().map(|commit| {
                                div().child(format!(
                                    "{} {}",
                                    &commit.oid[..commit.oid.len().min(7)],
                                    commit.headline
                                ))
                            })),
                    );
                }
                if !details.timeline.is_empty() {
                    body = body.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .text_size(px(9.5))
                            .text_color(theme.muted)
                            .child("Conversation")
                            .children(details.timeline.iter().take(100).map(|item| {
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .text_size(px(8.5))
                                            .text_color(theme.faint)
                                            .child(item.author.clone())
                                            .when_some(item.review_state.clone(), |row, state| {
                                                row.child(
                                                    div().text_color(theme.warning).child(state),
                                                )
                                            }),
                                    )
                                    .when(!item.body.trim().is_empty(), |item_row| {
                                        item_row.child(
                                            div()
                                                .text_size(px(9.5))
                                                .text_color(theme.muted)
                                                .child(item.body.clone()),
                                        )
                                    })
                            })),
                    );
                }
                body = body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .border_t_1()
                        .border_color(theme.border)
                        .pt_2()
                        .child(
                            div()
                                .text_size(px(9.5))
                                .text_color(theme.muted)
                                .child("Review"),
                        )
                        .child(self.github_comment_input.clone())
                        .when(!self.github_review_status.is_empty(), |composer| {
                            composer.child(
                                div()
                                    .text_size(px(8.5))
                                    .text_color(theme.faint)
                                    .child(self.github_review_status.clone()),
                            )
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .id("github-review-comment")
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(theme.panel_alt)
                                        .text_size(px(8.5))
                                        .text_color(theme.text)
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.add_github_comment(cx);
                                        }))
                                        .child("Comment"),
                                )
                                .child(
                                    div()
                                        .id("github-review-approve")
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(theme.accent_muted)
                                        .text_size(px(8.5))
                                        .text_color(theme.text)
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.submit_github_review(
                                                ide_github::ReviewEvent::Approve,
                                                cx,
                                            );
                                        }))
                                        .child("Approve"),
                                )
                                .child(
                                    div()
                                        .id("github-review-request-changes")
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(theme.panel_alt)
                                        .text_size(px(8.5))
                                        .text_color(theme.warning)
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.submit_github_review(
                                                ide_github::ReviewEvent::RequestChanges,
                                                cx,
                                            );
                                        }))
                                        .child("Request changes"),
                                ),
                        ),
                );
            }
            GitHubTab::Files => {
                if details.files.is_empty() {
                    body = body.child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.faint)
                            .child("No changed files"),
                    );
                }
                for (index, file) in details.files.iter().take(500).enumerate() {
                    let path = self.root.join(&file.path);
                    body = body.child(
                        div()
                            .id(("github-file", index))
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(10.))
                            .border_b_1()
                            .border_color(theme.border)
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.panel_alt))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.open_path(path.clone(), cx);
                            }))
                            .child(div().flex_1().truncate().child(file.path.clone()))
                            .child(div().text_size(px(8.5)).text_color(theme.faint).child(
                                format!(
                                    "{} +{} −{}",
                                    file.change_type, file.additions, file.deletions
                                ),
                            )),
                    );
                }
            }
            GitHubTab::Diff => {
                if self.github_diff_busy || self.github_diff_files.is_empty() {
                    body = body.child(
                        div()
                            .p_2()
                            .text_size(px(10.5))
                            .text_color(theme.faint)
                            .child(self.github_diff_status.clone()),
                    );
                } else {
                    for (file_index, file) in self.github_diff_files.iter().take(200).enumerate() {
                        body = body.child(
                            div()
                                .px_2()
                                .py_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .bg(theme.panel_alt)
                                .text_size(px(9.))
                                .text_color(theme.muted)
                                .child(file.path.clone())
                                .child(
                                    div()
                                        .text_size(px(8.5))
                                        .text_color(theme.faint)
                                        .child(format!("+{} −{}", file.added, file.removed)),
                                ),
                        );
                        if file.binary {
                            body = body.child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_size(px(9.))
                                    .text_color(theme.faint)
                                    .child("Binary file"),
                            );
                            continue;
                        }
                        for (line_index, line) in file.lines.iter().take(1_000).enumerate() {
                            let (prefix, color, bg) = match line.kind {
                                crate::pr_diff::DiffLineKind::Hunk => {
                                    (" ", theme.accent, theme.panel_alt)
                                }
                                crate::pr_diff::DiffLineKind::Added => {
                                    ("+", theme.accent, theme.accent_muted)
                                }
                                crate::pr_diff::DiffLineKind::Removed => {
                                    ("−", theme.error, theme.panel_alt)
                                }
                                crate::pr_diff::DiffLineKind::Context => {
                                    (" ", theme.muted, theme.panel)
                                }
                                crate::pr_diff::DiffLineKind::Meta => {
                                    ("\\", theme.faint, theme.panel)
                                }
                            };
                            let old_number = line
                                .old_line
                                .map(|line| line.to_string())
                                .unwrap_or_default();
                            let new_number = line
                                .new_line
                                .map(|line| line.to_string())
                                .unwrap_or_default();
                            body = body.child(
                                div()
                                    .id(("github-diff-line", file_index * 1_000 + line_index))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .bg(bg)
                                    .font_family("Menlo")
                                    .text_size(px(8.5))
                                    .text_color(color)
                                    .child(
                                        div().w(px(28.)).text_color(theme.faint).child(old_number),
                                    )
                                    .child(
                                        div().w(px(28.)).text_color(theme.faint).child(new_number),
                                    )
                                    .child(div().w(px(8.)).child(prefix))
                                    .child(div().flex_1().truncate().child(line.text.clone())),
                            );
                        }
                    }
                }
            }
            GitHubTab::Checks => {
                if details.checks.is_empty() {
                    body = body.child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.faint)
                            .child("No checks"),
                    );
                }
                for (index, check) in details.checks.iter().take(500).enumerate() {
                    let url = check.url.clone();
                    let color = match check.status {
                        ide_github::CheckStatus::Success => theme.accent,
                        ide_github::CheckStatus::Failure => theme.error,
                        ide_github::CheckStatus::Pending => theme.warning,
                        _ => theme.faint,
                    };
                    body = body.child(
                        div()
                            .id(("github-check", index))
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(10.))
                            .border_b_1()
                            .border_color(theme.border)
                            .when(url.is_some(), |row| {
                                row.cursor_pointer()
                                    .hover(|style| style.bg(theme.panel_alt))
                            })
                            .when_some(url, |row, url| {
                                row.on_click(move |_: &ClickEvent, _, cx| {
                                    cx.open_url(&url);
                                })
                            })
                            .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(color))
                            .child(div().flex_1().truncate().child(check.name.clone()))
                            .child(
                                div()
                                    .text_size(px(8.5))
                                    .text_color(theme.faint)
                                    .child(format!("{:?}", check.status)),
                            ),
                    );
                }
            }
            GitHubTab::Comments => {
                if self.github_comments.is_empty() {
                    body = body.child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.faint)
                            .child("No inline review comments"),
                    );
                }
                for (index, comment) in self.github_comments.iter().take(300).enumerate() {
                    body = body.child(
                        div()
                            .id(("github-comment", index))
                            .px_2()
                            .py_1()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .border_b_1()
                            .border_color(theme.border)
                            .text_size(px(9.5))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_color(theme.faint)
                                    .child(comment.author.clone())
                                    .child(format!(
                                        "{}:{}",
                                        comment.path,
                                        comment.line.unwrap_or(comment.original_line.unwrap_or(0))
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(theme.muted)
                                    .child(comment.body.clone()),
                            ),
                    );
                }
            }
        }
        let tabs = [
            (GitHubTab::Overview, "Overview"),
            (GitHubTab::Files, "Files"),
            (GitHubTab::Diff, "Diff"),
            (GitHubTab::Checks, "Checks"),
            (GitHubTab::Comments, "Comments"),
        ];
        let mut tab_row = div().flex().items_center().gap(px(3.));
        for (index, (tab, label)) in tabs.into_iter().enumerate() {
            let selected = self.github_tab == tab;
            tab_row = tab_row.child(
                div()
                    .id(("github-detail-tab", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_size(px(9.))
                    .text_color(if selected { theme.text } else { theme.faint })
                    .bg(if selected {
                        theme.accent_muted
                    } else {
                        theme.panel_alt
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.github_tab = tab;
                        if tab == GitHubTab::Diff && this.github_diff_files.is_empty() {
                            this.load_github_diff(cx);
                        }
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("github-detail-back")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.close_github_pr(cx);
                            }))
                            .child("← Pull requests"),
                    )
                    .child(
                        div()
                            .id("github-open-browser")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(move |_: &ClickEvent, _, cx| {
                                cx.open_url(&browser_url);
                            })
                            .child("Browser"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.github_detail_status.clone()),
                    ),
            )
            .child(div().px_2().py_1().child(tab_row))
            .child(body)
            .into_any_element()
    }

    fn render_heap(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.heap_selected_class.is_some() {
            return self.render_heap_objects(cx);
        }
        let theme = self.theme;
        let mut rows = div()
            .id("heap-classes")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.heap_classes.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(if self.heap_busy {
                        "Analyzing heap snapshot…"
                    } else {
                        "No heap snapshot loaded"
                    }),
            );
        }
        for (index, class) in self.heap_classes.iter().take(500).enumerate() {
            rows = rows.child(
                div()
                    .id(("heap-class", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.))
                    .border_b_1()
                    .border_color(theme.border)
                    .hover(|style| style.bg(theme.panel_alt))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.open_heap_class(index, cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(
                                div()
                                    .truncate()
                                    .text_color(theme.text)
                                    .child(class.name.clone()),
                            )
                            .child(div().text_size(px(8.5)).text_color(theme.faint).child(
                                format!(
                                    "{} · shallow {}",
                                    class.count,
                                    format_bytes(class.shallow)
                                ),
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.accent)
                            .child(format_bytes(class.retained)),
                    ),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("heap-open")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.heap_busy {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.open_heap_snapshot(cx);
                            }))
                            .child(if self.heap_busy { "Loading…" } else { "Open" }),
                    )
                    .child(
                        div()
                            .id("heap-close")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.heap_info.is_some() {
                                theme.error
                            } else {
                                theme.faint
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.close_heap_snapshot(cx);
                            }))
                            .child("Close"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.heap_status.clone()),
                    ),
            )
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.heap_path_input.clone()),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_heap_objects(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("heap-objects")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.heap_objects.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(self.heap_objects_status.clone()),
            );
        }
        for (index, object) in self.heap_objects.iter().take(200).enumerate() {
            rows = rows.child(
                div()
                    .id(("heap-object", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(9.5))
                    .border_b_1()
                    .border_color(theme.border)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.inspect_heap_object(index, cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(
                                div().truncate().text_color(theme.text).child(
                                    object
                                        .preview
                                        .clone()
                                        .unwrap_or_else(|| object.address.clone()),
                                ),
                            )
                            .child(
                                div()
                                    .text_size(px(8.5))
                                    .text_color(theme.faint)
                                    .child(object.address.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(8.5))
                            .text_color(theme.accent)
                            .child(format_bytes(object.retained)),
                    ),
            );
        }
        let path_panel = if self.heap_path_text.is_empty() {
            div().into_any_element()
        } else {
            div()
                .id("heap-path")
                .flex_none()
                .max_h(px(140.))
                .overflow_y_scroll()
                .p_2()
                .border_t_1()
                .border_color(theme.border)
                .bg(theme.panel_alt)
                .font_family("Menlo")
                .text_size(px(9.))
                .text_color(theme.muted)
                .child(self.heap_path_text.clone())
                .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("heap-class-back")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.close_heap_class(cx);
                            }))
                            .child("← Classes"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .child(self.heap_class_name.clone().unwrap_or_default()),
                    )
                    .child(
                        div()
                            .text_size(px(8.5))
                            .text_color(theme.faint)
                            .child(format!(
                                "{} of {}",
                                self.heap_objects.len(),
                                self.heap_objects_total
                            )),
                    ),
            )
            .child(rows)
            .child(path_panel)
            .into_any_element()
    }

    fn render_profiler(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("profile-methods")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        let Some(profile) = self.profile.as_ref() else {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(if self.profile_busy {
                        "Reading CPU profiles…"
                    } else {
                        "No CPU profile loaded"
                    }),
            );
            return div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(30.))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .id("profile-open")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(9.5))
                                .text_color(theme.text)
                                .bg(theme.accent_muted)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.open_profile(cx);
                                }))
                                .child("Load"),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(px(9.))
                                .text_color(theme.faint)
                                .truncate()
                                .child(self.profile_status.clone()),
                        ),
                )
                .child(
                    div()
                        .h(px(30.))
                        .px_2()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(self.profile_dir_input.clone()),
                )
                .child(rows)
                .into_any_element();
        };
        for (index, method) in profile.methods.iter().take(500).enumerate() {
            let Some(frame) = profile.frames.get(method.frame as usize) else {
                continue;
            };
            let file = frame.file.clone();
            let line = frame.line.unwrap_or_default();
            rows = rows.child(
                div()
                    .id(("profile-method", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.))
                    .border_b_1()
                    .border_color(theme.border)
                    .when(file.is_some(), |row| {
                        row.cursor_pointer()
                            .hover(|style| style.bg(theme.panel_alt))
                    })
                    .when_some(file, |row, file| {
                        row.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.open_or_goto(file.0.clone(), line, 0, window, cx);
                        }))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(
                                div()
                                    .truncate()
                                    .text_color(theme.text)
                                    .child(frame.name.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(8.5))
                                    .text_color(theme.faint)
                                    .child(
                                        frame
                                            .file
                                            .as_ref()
                                            .map(|file| {
                                                format!("{}:{}", file.0.display(), line + 1)
                                            })
                                            .unwrap_or_else(|| "(unknown source)".into()),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .w(px(66.))
                            .text_size(px(9.))
                            .text_color(theme.accent)
                            .child(format!("{:.1} {}", method.total, profile.unit)),
                    )
                    .child(
                        div()
                            .w(px(58.))
                            .text_size(px(8.5))
                            .text_color(theme.faint)
                            .child(format!("{:.1}", method.self_weight)),
                    ),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("profile-open")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.profile_busy {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.open_profile(cx);
                            }))
                            .child(if self.profile_busy {
                                "Loading…"
                            } else {
                                "Load"
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.profile_status.clone()),
                    ),
            )
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.profile_dir_input.clone()),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_logcat(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let filter = self.logcat_filter.to_lowercase();
        let mut device_rows = div()
            .id("logcat-devices")
            .flex_none()
            .max_h(px(90.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, device) in self.logcat_devices.iter().take(50).enumerate() {
            let serial = device.serial.clone();
            let label = device
                .model
                .as_ref()
                .map(|model| format!("{model} ({})", device.serial))
                .unwrap_or_else(|| device.serial.clone());
            device_rows = device_rows.child(
                div()
                    .id(("logcat-device", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(9.5))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.logcat_device_input.update(cx, |input, cx| {
                            input.set_content(serial.clone(), cx);
                        });
                        cx.notify();
                    }))
                    .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(
                        if device.state == "device" {
                            theme.accent
                        } else {
                            theme.faint
                        },
                    ))
                    .child(div().flex_1().truncate().child(label))
                    .child(
                        div()
                            .text_size(px(8.5))
                            .text_color(theme.faint)
                            .child(device.state.clone()),
                    ),
            );
        }
        let mut rows = div()
            .id("logcat-lines")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.logcat_lines.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(self.logcat_status.clone()),
            );
        }
        for (index, line) in self.logcat_lines.iter().enumerate() {
            if line.level < self.logcat_level {
                continue;
            }
            if !filter.is_empty()
                && !line.message.to_lowercase().contains(&filter)
                && !line.tag.to_lowercase().contains(&filter)
            {
                continue;
            }
            if index >= 1_000 {
                break;
            }
            let color = match line.level {
                crate::logcat::LogLevel::Verbose => theme.faint,
                crate::logcat::LogLevel::Debug => theme.muted,
                crate::logcat::LogLevel::Info => theme.text,
                crate::logcat::LogLevel::Warn => theme.warning,
                crate::logcat::LogLevel::Error | crate::logcat::LogLevel::Fatal => theme.error,
                crate::logcat::LogLevel::Unknown => theme.muted,
            };
            rows = rows.child(
                div()
                    .id(("logcat-line", index))
                    .px_2()
                    .py(px(1.))
                    .flex()
                    .items_start()
                    .gap_1()
                    .font_family("Menlo")
                    .text_size(px(9.5))
                    .hover(|style| style.bg(theme.panel_alt))
                    .child(
                        div()
                            .w(px(66.))
                            .flex_none()
                            .text_size(px(8.5))
                            .text_color(theme.faint)
                            .child(short_log_time(&line.time).to_owned()),
                    )
                    .child(
                        div()
                            .w(px(8.))
                            .flex_none()
                            .text_color(color)
                            .child(line.level.letter().to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .truncate()
                                    .text_color(theme.accent)
                                    .child(line.tag.clone()),
                            )
                            .child(div().text_color(theme.text).child(line.message.clone())),
                    ),
            );
        }
        let levels = [
            crate::logcat::LogLevel::Verbose,
            crate::logcat::LogLevel::Debug,
            crate::logcat::LogLevel::Info,
            crate::logcat::LogLevel::Warn,
            crate::logcat::LogLevel::Error,
        ];
        let mut level_row = div().flex().items_center().gap(px(2.));
        for (index, level) in levels.into_iter().enumerate() {
            let selected = self.logcat_level == level;
            level_row = level_row.child(
                div()
                    .id(("logcat-level", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_size(px(8.5))
                    .text_color(if selected { theme.text } else { theme.faint })
                    .bg(if selected {
                        theme.accent_muted
                    } else {
                        theme.panel_alt
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.logcat_level = level;
                        cx.notify();
                    }))
                    .child(level.letter().to_string()),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("logcat-start")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.logcat_busy {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.start_logcat(cx);
                            }))
                            .child(if self.logcat_busy {
                                "Streaming…"
                            } else {
                                "Start"
                            }),
                    )
                    .child(
                        div()
                            .id("logcat-stop")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(theme.error)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.stop_logcat(cx);
                            }))
                            .child("Stop"),
                    )
                    .child(
                        div()
                            .id("logcat-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_logcat_devices(cx);
                            }))
                            .child("Devices"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(8.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.logcat_status.clone()),
                    ),
            )
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.logcat_device_input.clone()),
            )
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(self.logcat_filter_input.clone()))
                    .child(level_row),
            )
            .child(device_rows)
            .child(rows)
            .into_any_element()
    }

    fn render_scratch(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("scratch-files")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.scratch_files.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child("No scratch files"),
            );
        }
        for (index, file) in self.scratch_files.iter().take(500).enumerate() {
            rows = rows.child(
                div()
                    .id(("scratch-file", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.5))
                    .border_b_1()
                    .border_color(theme.border)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.open_scratch(index, cx);
                    }))
                    .child(div().flex_1().truncate().child(file.name.clone()))
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .child(format_age(file.modified_ms)),
                    ),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("scratch-new")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                let stamp = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|duration| duration.as_secs())
                                    .unwrap_or_default();
                                let name = format!("scratch-{stamp}.md");
                                this.scratch_name_input.update(cx, |input, cx| {
                                    input.set_content(name, cx);
                                });
                                this.create_scratch(cx);
                            }))
                            .child("New"),
                    )
                    .child(
                        div()
                            .id("scratch-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_scratches(cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.scratch_status.clone()),
                    ),
            )
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.scratch_name_input.clone()),
            )
            .child(rows)
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(8.5))
                    .text_color(theme.faint)
                    .truncate()
                    .child(self.scratch_dir.display().to_string()),
            )
            .into_any_element()
    }

    fn render_recent(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let root = crate::bookmarks::project_key(&self.root);
        let prefix = format!("{root}/");
        let mut rows = div()
            .id("recent-files")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.recent_files.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child("No recent files"),
            );
        }
        for (index, path) in self.recent_files.iter().take(200).enumerate() {
            let full = path.to_string_lossy();
            let label = full.strip_prefix(&prefix).unwrap_or(&full).to_owned();
            rows = rows.child(
                div()
                    .id(("recent-file", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.5))
                    .border_b_1()
                    .border_color(theme.border)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.open_recent(index, cx);
                    }))
                    .child(
                        div()
                            .w(px(18.))
                            .text_size(px(8.5))
                            .text_color(theme.faint)
                            .child((index + 1).to_string()),
                    )
                    .child(div().flex_1().truncate().child(label)),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("recent-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_recent(cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.recent_status.clone()),
                    ),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_analysis(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        match self.analysis_tab {
            AnalysisTab::Structural => return self.render_structural(cx),
            AnalysisTab::Inspect => return self.render_inspect(cx),
            AnalysisTab::Duplicates => {}
        }
        let theme = self.theme;
        let mut rows = div()
            .id("duplicate-groups")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        match self.duplicates_report.as_ref() {
            None => {
                rows = rows.child(
                    div()
                        .p_3()
                        .text_size(px(10.5))
                        .text_color(theme.faint)
                        .child(self.duplicates_status.clone()),
                );
            }
            Some(report) if report.groups.is_empty() => {
                rows = rows.child(
                    div()
                        .p_3()
                        .text_size(px(10.5))
                        .text_color(theme.faint)
                        .child("No duplicate groups found"),
                );
            }
            Some(report) => {
                for (group_index, group) in report.groups.iter().take(100).enumerate() {
                    rows = rows.child(
                        div()
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .bg(theme.panel_alt)
                            .text_size(px(9.5))
                            .text_color(theme.accent)
                            .child(format!(
                                "{} tokens · {} lines · {}",
                                group.tokens, group.lines, group.language
                            )),
                    );
                    for (fragment_index, fragment) in group.fragments.iter().take(20).enumerate() {
                        let path = fragment.path.0.clone();
                        let line = fragment.range.start.line;
                        let column = fragment.range.start.col;
                        rows = rows.child(
                            div()
                                .id(("duplicate-fragment", group_index * 100 + fragment_index))
                                .px_2()
                                .py_1()
                                .flex()
                                .flex_col()
                                .gap(px(1.))
                                .border_b_1()
                                .border_color(theme.border)
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.panel_alt))
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    this.open_or_goto(path.clone(), line, column, window, cx);
                                }))
                                .child(div().text_size(px(9.5)).text_color(theme.text).child(
                                    format!(
                                        "{}:{}-{}",
                                        fragment.rel_path,
                                        line + 1,
                                        fragment.range.end.line + 1
                                    ),
                                ))
                                .child(
                                    div()
                                        .font_family("Menlo")
                                        .text_size(px(8.5))
                                        .text_color(theme.faint)
                                        .child(
                                            fragment
                                                .text
                                                .lines()
                                                .next()
                                                .unwrap_or_default()
                                                .to_owned(),
                                        ),
                                ),
                        );
                    }
                }
            }
        }
        let identifiers = self.duplicates_anonymize_identifiers;
        let literals = self.duplicates_anonymize_literals;
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("analysis-tab-duplicates")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .child("Duplicates"),
                    )
                    .child(
                        div()
                            .id("analysis-tab-structural")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.muted)
                            .bg(theme.panel_alt)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.analysis_tab = AnalysisTab::Structural;
                                cx.notify();
                            }))
                            .child("Structural"),
                    )
                    .child(
                        div()
                            .id("analysis-tab-inspect")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.muted)
                            .bg(theme.panel_alt)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.analysis_tab = AnalysisTab::Inspect;
                                cx.notify();
                            }))
                            .child("Inspect"),
                    )
                    .child(
                        div()
                            .id("duplicates-run")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.duplicates_busy {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.run_duplicate_analysis(cx);
                            }))
                            .child(if self.duplicates_busy {
                                "Scanning…"
                            } else {
                                "Locate Duplicates"
                            }),
                    )
                    .child(
                        div()
                            .id("duplicates-identifiers")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(8.5))
                            .text_color(if identifiers { theme.text } else { theme.faint })
                            .bg(if identifiers {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.duplicates_anonymize_identifiers =
                                    !this.duplicates_anonymize_identifiers;
                                cx.notify();
                            }))
                            .child("Ignore names"),
                    )
                    .child(
                        div()
                            .id("duplicates-literals")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(8.5))
                            .text_color(if literals { theme.text } else { theme.faint })
                            .bg(if literals {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.duplicates_anonymize_literals =
                                    !this.duplicates_anonymize_literals;
                                cx.notify();
                            }))
                            .child("Ignore literals"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(8.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.duplicates_status.clone()),
                    ),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_structural(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("structural-results")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.structural_files.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(self.structural_status.clone()),
            );
        }
        for (file_index, file) in self.structural_files.iter().take(100).enumerate() {
            rows = rows.child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(9.5))
                    .text_color(theme.muted)
                    .bg(theme.panel_alt)
                    .child(format!("{} · {}", file.rel_path, file.matches.len())),
            );
            for (match_index, matched) in file.matches.iter().take(50).enumerate() {
                let path = file.path.0.clone();
                let line = matched.range.start.line;
                let column = matched.range.start.col;
                rows = rows.child(
                    div()
                        .id(("structural-match", file_index * 100 + match_index))
                        .px_2()
                        .py_1()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .border_b_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.open_or_goto(path.clone(), line, column, window, cx);
                        }))
                        .child(
                            div()
                                .text_size(px(9.))
                                .text_color(theme.faint)
                                .child(format!("{}:{}", file.rel_path, line + 1)),
                        )
                        .child(
                            div()
                                .font_family("Menlo")
                                .text_size(px(8.5))
                                .text_color(theme.text)
                                .child(matched.line_text.clone()),
                        )
                        .when_some(matched.replacement.clone(), |row, replacement| {
                            row.child(
                                div()
                                    .font_family("Menlo")
                                    .text_size(px(8.5))
                                    .text_color(theme.accent)
                                    .child(format!("→ {replacement}")),
                            )
                        }),
                );
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("analysis-tab-duplicates")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.muted)
                            .bg(theme.panel_alt)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.analysis_tab = AnalysisTab::Duplicates;
                                cx.notify();
                            }))
                            .child("Duplicates"),
                    )
                    .child(
                        div()
                            .id("analysis-tab-structural")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .child("Structural"),
                    )
                    .child(
                        div()
                            .id("analysis-tab-inspect")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.muted)
                            .bg(theme.panel_alt)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.analysis_tab = AnalysisTab::Inspect;
                                cx.notify();
                            }))
                            .child("Inspect"),
                    )
                    .child(
                        div()
                            .id("structural-run")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(if self.structural_busy {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.run_structural_search(cx);
                            }))
                            .child(if self.structural_busy {
                                "Searching…"
                            } else {
                                "Search"
                            }),
                    )
                    .when(
                        self.structural_match_count() > 0
                            && !self
                                .structural_replacement_input
                                .read(cx)
                                .content()
                                .trim()
                                .is_empty(),
                        |toolbar| {
                            toolbar.child(
                                div()
                                    .id("structural-apply")
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .text_size(px(9.))
                                    .text_color(if self.structural_busy {
                                        theme.faint
                                    } else {
                                        theme.text
                                    })
                                    .bg(theme.accent_muted)
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.apply_structural_replacements(cx);
                                    }))
                                    .child("Apply Replacements"),
                            )
                        },
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(8.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.structural_status.clone()),
                    ),
            )
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.structural_language_input.clone()),
            )
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.structural_pattern_input.clone()),
            )
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.structural_replacement_input.clone()),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_inspect(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("inspect-results")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.inspect_files.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(self.inspect_status.clone()),
            );
        }
        for (file_index, file) in self.inspect_files.iter().take(200).enumerate() {
            if file.diagnostics.is_empty() {
                continue;
            }
            rows = rows.child(
                div()
                    .px_2()
                    .py_1()
                    .bg(theme.panel_alt)
                    .text_size(px(9.5))
                    .text_color(theme.accent)
                    .child(format!("{} · {}", file.rel_path, file.diagnostics.len())),
            );
            for (diagnostic_index, diagnostic) in file.diagnostics.iter().take(50).enumerate() {
                let path = file.path.0.clone();
                let line = diagnostic.range.start.line;
                let column = diagnostic.range.start.col;
                let color = match diagnostic.severity {
                    Severity::Error => theme.error,
                    Severity::Warning => theme.warning,
                    Severity::Info => theme.accent,
                    Severity::Hint => theme.muted,
                };
                rows = rows.child(
                    div()
                        .id(("inspect-diagnostic", file_index * 100 + diagnostic_index))
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(9.5))
                        .border_b_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.open_or_goto(path.clone(), line, column, window, cx);
                        }))
                        .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(color))
                        .child(
                            div()
                                .w(px(30.))
                                .text_size(px(8.5))
                                .text_color(theme.faint)
                                .child(format!("{}", line + 1)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .text_color(theme.muted)
                                .child(diagnostic.message.clone()),
                        ),
                );
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("analysis-tab-duplicates")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.muted)
                            .bg(theme.panel_alt)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.analysis_tab = AnalysisTab::Duplicates;
                                cx.notify();
                            }))
                            .child("Duplicates"),
                    )
                    .child(
                        div()
                            .id("analysis-tab-structural")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.muted)
                            .bg(theme.panel_alt)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.analysis_tab = AnalysisTab::Structural;
                                cx.notify();
                            }))
                            .child("Structural"),
                    )
                    .child(
                        div()
                            .id("analysis-tab-inspect")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .child("Inspect"),
                    )
                    .child(
                        div()
                            .id("inspect-run")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.))
                            .text_color(if self.inspect_busy {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.run_inspect_code(cx);
                            }))
                            .child(if self.inspect_busy {
                                "Inspecting…"
                            } else {
                                "Inspect Code"
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(8.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.inspect_status.clone()),
                    ),
            )
            .child(rows)
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
                let line_index = line.line;
                let column = line.text_offset
                    + line
                        .ranges
                        .first()
                        .map(|(start, _)| *start)
                        .unwrap_or_default();
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
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.open_or_goto(path.clone(), line_index, column, window, cx);
                        }))
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
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.search_replace_input.clone()),
            )
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
                            .id("search-replace-all")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.search_replacing {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.replace_search_results(cx);
                            }))
                            .child(if self.search_replacing {
                                "Replacing…"
                            } else {
                                "Replace All"
                            }),
                    )
                    .child(
                        div()
                            .id("search-regex")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.search_regex {
                                theme.text
                            } else {
                                theme.faint
                            })
                            .bg(if self.search_regex {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.search_regex = !this.search_regex;
                                if !this.search_pattern.is_empty() {
                                    this.start_search(this.search_pattern.clone(), cx);
                                }
                            }))
                            .child(".*"),
                    )
                    .child(
                        div()
                            .id("search-case")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.search_case_sensitive {
                                theme.text
                            } else {
                                theme.faint
                            })
                            .bg(if self.search_case_sensitive {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.search_case_sensitive = !this.search_case_sensitive;
                                if !this.search_pattern.is_empty() {
                                    this.start_search(this.search_pattern.clone(), cx);
                                }
                            }))
                            .child("Aa"),
                    )
                    .child(
                        div()
                            .id("search-whole-word")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.search_whole_word {
                                theme.text
                            } else {
                                theme.faint
                            })
                            .bg(if self.search_whole_word {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.search_whole_word = !this.search_whole_word;
                                if !this.search_pattern.is_empty() {
                                    this.start_search(this.search_pattern.clone(), cx);
                                }
                            }))
                            .child("W"),
                    )
                    .child(
                        div()
                            .id("search-preserve-case")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if self.search_preserve_case {
                                theme.text
                            } else {
                                theme.faint
                            })
                            .bg(if self.search_preserve_case {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.search_preserve_case = !this.search_preserve_case;
                                cx.notify();
                            }))
                            .child("Preserve"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(theme.faint)
                            .child(format!(
                                "{} matches",
                                self.search_results
                                    .iter()
                                    .map(|file| file
                                        .matches
                                        .iter()
                                        .map(|line| line.ranges.len())
                                        .sum::<usize>())
                                    .sum::<usize>()
                            )),
                    ),
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

    fn render_todo(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("todo-items")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.todo_items.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(if self.todo_status.starts_with("Scanning") {
                        "Scanning…"
                    } else {
                        "No TODO comments found"
                    }),
            );
        }
        for (index, item) in self.todo_items.iter().take(500).enumerate() {
            let path = item.path.clone();
            let line = item.line;
            let column = item.column;
            let color = if item.keyword == "FIXME" {
                theme.error
            } else if item.keyword == "TODO" {
                theme.accent
            } else {
                theme.warning
            };
            rows = rows.child(
                div()
                    .id(("todo-item", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.5))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.open_or_goto(path.clone(), line, column, window, cx);
                    }))
                    .child(
                        div()
                            .flex_none()
                            .w(px(38.))
                            .text_color(color)
                            .child(item.keyword.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(
                                div()
                                    .truncate()
                                    .text_color(theme.text)
                                    .child(item.text.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(9.))
                                    .text_color(theme.faint)
                                    .child(format!("{}:{}", item.rel_path, item.line + 1)),
                            ),
                    ),
            );
        }
        let case_sensitive = self.todo_case_sensitive;
        let comments_only = self.todo_comments_only;
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("todo-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .bg(theme.panel_alt)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_todo(cx);
                            }))
                            .child("Scan"),
                    )
                    .child(
                        div()
                            .id("todo-case")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if case_sensitive {
                                theme.text
                            } else {
                                theme.faint
                            })
                            .bg(if case_sensitive {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.todo_case_sensitive = !this.todo_case_sensitive;
                                this.refresh_todo(cx);
                            }))
                            .child("Aa"),
                    )
                    .child(
                        div()
                            .id("todo-comments")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if comments_only {
                                theme.text
                            } else {
                                theme.faint
                            })
                            .bg(if comments_only {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.todo_comments_only = !this.todo_comments_only;
                                this.refresh_todo(cx);
                            }))
                            .child("Comments"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.todo_status.clone()),
                    ),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_dependencies(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let filter = self.dependency_filter.to_lowercase();
        let mut rows = div()
            .id("build-dependencies")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.dependency_show_modules {
            for (index, module) in self.dependency_modules.iter().take(500).enumerate() {
                if !filter.is_empty()
                    && !module.label.to_lowercase().contains(&filter)
                    && !module.detail.to_lowercase().contains(&filter)
                {
                    continue;
                }
                let path = module.path.clone();
                rows = rows.child(
                    div()
                        .id(("build-module", index))
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(10.5))
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.open_path(path.clone(), cx);
                        }))
                        .child(div().flex_1().truncate().child(module.label.clone()))
                        .child(
                            div()
                                .max_w(px(145.))
                                .truncate()
                                .text_size(px(9.))
                                .text_color(theme.faint)
                                .child(module.detail.clone()),
                        ),
                );
            }
        } else {
            for (index, dependency) in self.dependency_coordinates.iter().take(1_000).enumerate() {
                let label = crate::build_tools::coordinate_label(dependency);
                if !filter.is_empty() && !label.to_lowercase().contains(&filter) {
                    continue;
                }
                rows = rows.child(
                    div()
                        .id(("build-dependency", index))
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(10.5))
                        .hover(|style| style.bg(theme.panel_alt))
                        .child(div().flex_1().min_w(px(0.)).truncate().child(label))
                        .child(
                            div()
                                .text_size(px(9.))
                                .text_color(theme.faint)
                                .child(crate::build_tools::source_label(dependency)),
                        ),
                );
            }
        }
        if self.dependency_modules.is_empty()
            && self.dependency_coordinates.is_empty()
            && !self.dependency_status.starts_with("Loading")
        {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child("No Maven, Gradle or sbt modules found"),
            );
        }
        let show_modules = self.dependency_show_modules;
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("dependencies-refresh")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.muted)
                            .bg(theme.panel_alt)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.refresh_dependencies(cx);
                            }))
                            .child("Sync"),
                    )
                    .child(
                        div()
                            .id("dependencies-modules")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if show_modules {
                                theme.text
                            } else {
                                theme.faint
                            })
                            .bg(if show_modules {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.dependency_show_modules = true;
                                cx.notify();
                            }))
                            .child("Modules"),
                    )
                    .child(
                        div()
                            .id("dependencies-libraries")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if show_modules {
                                theme.faint
                            } else {
                                theme.text
                            })
                            .bg(if show_modules {
                                theme.panel_alt
                            } else {
                                theme.accent_muted
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.dependency_show_modules = false;
                                cx.notify();
                            }))
                            .child("Libraries"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.dependency_status.clone()),
                    ),
            )
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.dependency_filter_input.clone()),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_coverage(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let has_branches = self
            .coverage_suite
            .as_ref()
            .is_some_and(|suite| suite.has_branches);
        let mut rows = div()
            .id("coverage-files")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.coverage_rows.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(if self.coverage_status.starts_with("Importing") {
                        "Importing report…"
                    } else {
                        "No coverage data. Import an lcov, JaCoCo or Go report."
                    }),
            );
        }
        for (index, row) in self.coverage_rows.iter().take(1_000).enumerate() {
            let path = row.path.clone();
            let branches_covered = row.branches_covered;
            let branches_total = row.branches_total;
            let lines_percent = crate::coverage::percent(row.lines_covered, row.lines_total);
            let color = match lines_percent {
                Some(percent) if percent < 50.0 => theme.error,
                Some(percent) if percent < 80.0 => theme.warning,
                Some(_) => theme.accent,
                None => theme.faint,
            };
            rows = rows.child(
                div()
                    .id(("coverage-file", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.5))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.open_path(path.clone(), cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(row.rel_path.clone()),
                    )
                    .when(has_branches, |item| {
                        item.child(
                            div()
                                .w(px(72.))
                                .text_size(px(9.))
                                .text_color(theme.faint)
                                .child(crate::coverage::format_percent(
                                    branches_covered,
                                    branches_total,
                                )),
                        )
                    })
                    .child(div().w(px(82.)).text_size(px(9.5)).text_color(color).child(
                        crate::coverage::format_percent(row.lines_covered, row.lines_total),
                    )),
            );
        }
        let sort_lines = self.coverage_sort_lines;
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("coverage-import")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.import_coverage(cx);
                            }))
                            .child("Import"),
                    )
                    .child(
                        div()
                            .id("coverage-sort")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(if sort_lines { theme.text } else { theme.faint })
                            .bg(if sort_lines {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.coverage_sort_lines = !this.coverage_sort_lines;
                                crate::coverage::sort_rows(
                                    &mut this.coverage_rows,
                                    this.coverage_sort_lines,
                                );
                                cx.notify();
                            }))
                            .child(if sort_lines { "Line % ↑" } else { "Path A-Z" }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.coverage_status.clone()),
                    ),
            )
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.coverage_path_input.clone()),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_bookmarks(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = div()
            .id("bookmark-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        if self.bookmarks.is_empty() {
            rows = rows.child(
                div()
                    .p_3()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child("No bookmarks. Put the caret on a line and press F3."),
            );
        }
        let root_prefix = crate::bookmarks::project_key(&self.root);
        let prefix = format!("{root_prefix}/");
        for (index, bookmark) in self.bookmarks.iter().take(1_000).enumerate() {
            let rel_path = bookmark
                .path
                .strip_prefix(&prefix)
                .unwrap_or(&bookmark.path)
                .to_owned();
            let path = PathBuf::from(&bookmark.path);
            let line = bookmark.line;
            rows = rows.child(
                div()
                    .id(("bookmark-row", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.5))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.open_or_goto(path.clone(), line, 0, window, cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(div().truncate().text_color(theme.text).child(
                                if bookmark.text.trim().is_empty() {
                                    "(empty line)".to_owned()
                                } else {
                                    bookmark.text.clone()
                                },
                            ))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(9.))
                                    .text_color(theme.faint)
                                    .child(format!("{rel_path}:{}", line + 1)),
                            ),
                    )
                    .child(
                        div()
                            .id(("bookmark-remove", index))
                            .px_1()
                            .rounded_sm()
                            .text_color(theme.faint)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.remove_bookmark(index, cx);
                            }))
                            .child("×"),
                    ),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .id("bookmark-add")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(9.5))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.toggle_bookmark(cx);
                            }))
                            .child("Toggle at caret"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(theme.faint)
                            .truncate()
                            .child(self.bookmarks_status.clone()),
                    ),
            )
            .child(rows)
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
            .flex_none()
            .max_h(px(120.))
            .overflow_y_scroll()
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
        let mut session_rows = div()
            .id("debug-sessions")
            .flex_none()
            .max_h(px(80.))
            .overflow_y_scroll()
            .px_2()
            .flex()
            .flex_col();
        for (index, session) in self.debug_sessions.iter().take(80).enumerate() {
            let selected = index == self.debug_session_selected;
            session_rows = session_rows.child(
                div()
                    .id(("debug-session", index))
                    .h(px(20.))
                    .px_1()
                    .flex()
                    .items_center()
                    .text_size(px(10.))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.debug_session_selected = index;
                        cx.notify();
                    }))
                    .child(format!("{} · {}", session.name, session.adapter)),
            );
        }
        let mut breakpoint_rows = div()
            .id("debug-breakpoints")
            .flex_none()
            .max_h(px(60.))
            .overflow_y_scroll()
            .px_2()
            .flex()
            .flex_col();
        for (index, state) in self.debug_breakpoint_states.iter().take(80).enumerate() {
            breakpoint_rows = breakpoint_rows.child(
                div()
                    .id(("debug-breakpoint", index))
                    .h(px(18.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(9.5))
                    .text_color(if state.verified {
                        theme.accent
                    } else {
                        theme.faint
                    })
                    .child(format!("● line {}", state.line + 1))
                    .when_some(state.message.clone(), |row, message| {
                        row.child(div().text_color(theme.error).child(message))
                    }),
            );
        }
        let mut frame_rows = div()
            .id("debug-frames")
            .flex_none()
            .max_h(px(70.))
            .overflow_y_scroll()
            .px_2()
            .flex()
            .flex_col();
        for (index, frame) in self.debug_frames.iter().take(80).enumerate() {
            let selected = index == self.debug_frame_selected;
            let location = frame
                .path
                .as_ref()
                .map(|path| format!("{}:{}", path.0.display(), frame.line + 1))
                .unwrap_or_default();
            frame_rows = frame_rows.child(
                div()
                    .id(("debug-frame", index))
                    .h(px(18.))
                    .px_1()
                    .text_size(px(9.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select_debug_frame(index, cx);
                    }))
                    .child(format!("{} · {}", frame.name, location)),
            );
        }
        let mut variable_rows = div()
            .id("debug-variables")
            .flex_none()
            .max_h(px(90.))
            .overflow_y_scroll()
            .px_2()
            .flex()
            .flex_col();
        for (index, variable) in self.debug_variables.iter().take(100).enumerate() {
            variable_rows = variable_rows.child(
                div()
                    .id(("debug-variable", index))
                    .min_h(px(18.))
                    .text_size(px(9.5))
                    .text_color(theme.muted)
                    .child(format!(
                        "{} = {}{}",
                        variable.name,
                        variable.value,
                        variable
                            .type_name
                            .as_ref()
                            .map(|ty| format!(" : {ty}"))
                            .unwrap_or_default()
                    )),
            );
        }
        let mut debug_lines: Vec<String> = self.debug_console.lines().map(str::to_owned).collect();
        if debug_lines.len() > 300 {
            debug_lines.drain(..debug_lines.len() - 300);
        }
        let debug_console = div()
            .id("debug-console")
            .flex_1()
            .min_h(px(0.))
            .overflow_scroll()
            .p_2()
            .font_family("Menlo")
            .text_size(px(9.5))
            .text_color(theme.text)
            .flex()
            .flex_col()
            .when(debug_lines.is_empty(), |view| {
                view.child(
                    div()
                        .text_color(theme.faint)
                        .child(self.debug_status.clone()),
                )
            })
            .children(debug_lines.into_iter().map(|line| div().child(line)));
        let debug_body = div()
            .size_full()
            .flex()
            .flex_col()
            .child(debug_rows)
            .child(session_rows)
            .child(breakpoint_rows)
            .child(frame_rows)
            .child(variable_rows)
            .child(
                div()
                    .h(px(28.))
                    .px_2()
                    .py_1()
                    .child(self.debug_eval_input.clone()),
            )
            .child(debug_console)
            .into_any_element();
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
                        .child(
                            div()
                                .id("debug-breakpoint-toggle")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.text)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.toggle_breakpoint_at_cursor(cx);
                                }))
                                .child("Breakpoint"),
                        )
                        .child(
                            div()
                                .id("debug-resume")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.text)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.debug_resume(cx);
                                }))
                                .child("Resume"),
                        )
                        .child(
                            div()
                                .id("debug-pause")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.muted)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.debug_pause(cx);
                                }))
                                .child("Pause"),
                        )
                        .child(
                            div()
                                .id("debug-step-over")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.text)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.debug_step(ide_dap::StepKind::Over, cx);
                                }))
                                .child("Step"),
                        )
                        .child(
                            div()
                                .id("debug-attach")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.text)
                                .bg(theme.accent_muted)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.attach_debug_process(cx);
                                }))
                                .child("Attach"),
                        )
                        .child(
                            div()
                                .id("debug-stop")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(10.5))
                                .text_color(theme.error)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.stop_debug_session(cx);
                                }))
                                .child("Stop"),
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
        let todo = self.render_todo(cx);
        let dependencies = self.render_dependencies(cx);
        let coverage = self.render_coverage(cx);
        let bookmarks = self.render_bookmarks(cx);
        let git = self.render_git_panel(cx);
        let git_log = self.render_git_log(cx);
        let services = self.render_services(cx);
        let github = self.render_github(cx);
        let heap = self.render_heap(cx);
        let profiler = self.render_profiler(cx);
        let logcat = self.render_logcat(cx);
        let scratch = self.render_scratch(cx);
        let recent = self.render_recent(cx);
        let analysis = self.render_analysis(cx);
        let outline = self.render_outline(window, cx);
        let problems = self.render_problems(window, cx);
        let settings = self.render_settings(cx);
        let sidebar_body = match self.sidebar_mode {
            SidebarMode::Project => project,
            SidebarMode::Search => search,
            SidebarMode::Todo => todo,
            SidebarMode::Dependencies => dependencies,
            SidebarMode::Coverage => coverage,
            SidebarMode::Bookmarks => bookmarks,
            SidebarMode::Git => git,
            SidebarMode::GitLog => git_log,
            SidebarMode::Services => services,
            SidebarMode::GitHub => github,
            SidebarMode::Heap => heap,
            SidebarMode::Profiler => profiler,
            SidebarMode::Logcat => logcat,
            SidebarMode::Scratch => scratch,
            SidebarMode::Recent => recent,
            SidebarMode::Analysis => analysis,
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
            ("TODO", SidebarMode::Todo),
            ("DEP", SidebarMode::Dependencies),
            ("COV", SidebarMode::Coverage),
            ("BMK", SidebarMode::Bookmarks),
            ("GIT", SidebarMode::Git),
            ("LOG", SidebarMode::GitLog),
            ("SRV", SidebarMode::Services),
            ("GH", SidebarMode::GitHub),
            ("HEAP", SidebarMode::Heap),
            ("PROF", SidebarMode::Profiler),
            ("LOGCAT", SidebarMode::Logcat),
            ("SCR", SidebarMode::Scratch),
            ("RCNT", SidebarMode::Recent),
            ("ANLZ", SidebarMode::Analysis),
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
        let save_as = self.render_save_as(cx);
        let command_palette = self.render_command_palette(cx);
        let symbol_search = self.render_symbol_search(cx);
        let references = self.render_references(cx);
        let setting_editor = self.render_setting_editor(cx);
        let agent_form_answer = self.render_agent_form_answer(cx);
        let notebook_editor = self.render_notebook_editor(cx);
        let remote_editor = self.render_remote_editor(cx);
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
            .on_action(cx.listener(|this, _: &crate::SaveDocumentAs, window, cx| {
                this.open_save_as(window, cx);
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
            .on_action(cx.listener(|this, _: &crate::FindUsages, _, cx| {
                this.find_usages_at_cursor(cx);
            }))
            .on_action(cx.listener(|this, _: &crate::ToggleBookmark, _, cx| {
                this.toggle_bookmark(cx);
            }))
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
            .child(save_as)
            .child(command_palette)
            .child(symbol_search)
            .child(references)
            .child(setting_editor)
            .child(agent_form_answer)
            .child(notebook_editor)
            .child(remote_editor)
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

    #[test]
    fn stdio_mcp_input_uses_command_and_rejects_missing_executable() {
        let input = build_mcp_server_input(
            None,
            "echo",
            ide_agent::McpTransport::Stdio,
            " /bin/echo ",
            "hello world",
            "https://ignored.example.test",
        )
        .expect("stdio input");
        assert_eq!(input.command.as_deref(), Some("/bin/echo"));
        assert_eq!(input.args, ["hello", "world"]);
        assert_eq!(input.url, None);
        assert!(
            build_mcp_server_input(None, "echo", ide_agent::McpTransport::Stdio, " ", "", "")
                .is_err()
        );
    }

    #[test]
    fn http_and_sse_mcp_input_use_url_and_clear_stdio_fields() {
        for transport in [ide_agent::McpTransport::Http, ide_agent::McpTransport::Sse] {
            let input = build_mcp_server_input(
                None,
                "remote",
                transport,
                "/bin/ignored",
                "--ignored",
                " https://mcp.example.test/mcp ",
            )
            .expect("remote input");
            assert_eq!(input.transport, transport);
            assert_eq!(input.command, None);
            assert!(input.args.is_empty());
            assert_eq!(input.url.as_deref(), Some("https://mcp.example.test/mcp"));
        }
        assert!(
            build_mcp_server_input(None, "remote", ide_agent::McpTransport::Http, "", "", " ")
                .is_err()
        );
    }

    #[test]
    fn replace_targets_keep_each_match_position_once() {
        let file = ide_search::FileMatches {
            path: ide_core::IpcPath("/tmp/App.java".into()),
            rel_path: "src/App.java".into(),
            matches: vec![ide_search::LineMatch {
                line: 3,
                text: "TODO and TODO".into(),
                ranges: vec![(0, 4), (8, 12), (0, 4)],
                text_offset: 2,
            }],
        };
        let targets = search_replace_targets(&[file]);
        assert_eq!(targets.len(), 1);
        let only = targets[0].only.as_ref().expect("selected matches");
        assert_eq!(only, &[ide_core::Pos::new(3, 2), ide_core::Pos::new(3, 10)]);
    }

    #[test]
    fn search_query_honours_pattern_options() {
        let query = build_search_query("TODO.*".into(), true, true, true);
        assert!(query.is_regex);
        assert!(query.case_sensitive);
        assert!(query.whole_word);
        assert_eq!(query.pattern, "TODO.*");
        assert_eq!(query.max_matches, 10_000);
    }

    #[test]
    fn formats_heap_byte_counts() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.0 MB");
    }
}
