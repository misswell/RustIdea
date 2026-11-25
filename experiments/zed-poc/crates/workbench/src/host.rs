use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{Context as _, Result};
use ide_core::{DocId, IdeEvent, IdeResult, SharedSink, TerminalId};
use ide_host::{Host, HostConfig};
use ide_language::{LanguageRegistry, Syntax};
use ide_vfs::FileEntry;
use ide_workspace::DocumentInfo;
use parking_lot::Mutex;
use tokio::runtime::Runtime;
use tokio::sync::Notify;
use tokio::sync::mpsc::{Receiver, Sender, channel};

pub const MAIN_WINDOW: &str = "main";
const IDENTIFIER: &str = "com.guofeng.rustidea.zed";
const EVENT_CAPACITY: usize = 2048;

#[derive(Clone, Debug, Default)]
pub struct OverflowReport {
    pub document_patches: bool,
    pub file_changes: Option<IdeEvent>,
    pub general: bool,
    pub coalesced: Vec<IdeEvent>,
    pub dropped: u64,
}

#[derive(Default)]
struct OverflowState {
    document_patches: bool,
    general: bool,
    created: BTreeSet<PathBuf>,
    modified: BTreeSet<PathBuf>,
    removed: BTreeSet<PathBuf>,
    ignore_rules: BTreeSet<PathBuf>,
    coalesced: BTreeMap<String, IdeEvent>,
    dropped: u64,
}

pub struct EventOverflow {
    state: Mutex<OverflowState>,
    notify: Notify,
}

impl EventOverflow {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(OverflowState::default()),
            notify: Notify::new(),
        }
    }

    pub fn record(&self, event: IdeEvent) {
        {
            let mut state = self.state.lock();
            state.dropped += 1;
            match event {
                IdeEvent::DocPatched { .. } => state.document_patches = true,
                IdeEvent::FsChanged {
                    created,
                    modified,
                    removed,
                    ignore_rules,
                } => {
                    state.created.extend(created.into_iter().map(|path| path.0));
                    state
                        .modified
                        .extend(modified.into_iter().map(|path| path.0));
                    state.removed.extend(removed.into_iter().map(|path| path.0));
                    state
                        .ignore_rules
                        .extend(ignore_rules.into_iter().map(|path| path.0));
                }
                IdeEvent::Diagnostics {
                    ref path,
                    ref source,
                    ..
                } => {
                    state.coalesced.insert(
                        format!("diagnostics:{}:{}", path.0.display(), source),
                        event,
                    );
                }
                IdeEvent::Task { ref id, .. } => {
                    state.coalesced.insert(format!("task:{}", id.0), event);
                }
                IdeEvent::IndexState { .. } => {
                    state.coalesced.insert("index".into(), event);
                }
                IdeEvent::LanguageServer { ref name, .. } => {
                    state.coalesced.insert(format!("server:{name}"), event);
                }
                IdeEvent::LanguageServerWarning { ref name, .. } => {
                    state
                        .coalesced
                        .insert(format!("server-warning:{name}"), event);
                }
                IdeEvent::LanguageServerRemoved { ref server } => {
                    state
                        .coalesced
                        .insert(format!("server-removed:{}", server.0), event);
                }
                IdeEvent::VcsChanged { .. } => {
                    state.coalesced.insert("vcs".into(), event);
                }
                IdeEvent::Notify { ref level, .. } => {
                    state.coalesced.insert(format!("notify:{level:?}"), event);
                }
                IdeEvent::SettingsChanged { .. } => {
                    state.coalesced.insert("settings".into(), event);
                }
                IdeEvent::ProjectRootsChanged { .. } => {
                    state.general = true;
                    state.coalesced.insert("project-roots".into(), event);
                }
                _ => state.general = true,
            }
        }
        self.notify.notify_one();
    }

    pub fn take_report(&self) -> OverflowReport {
        let mut state = self.state.lock();
        let dropped = state.dropped;
        let file_changes = if state.created.is_empty()
            && state.modified.is_empty()
            && state.removed.is_empty()
            && state.ignore_rules.is_empty()
        {
            None
        } else {
            Some(IdeEvent::FsChanged {
                created: std::mem::take(&mut state.created)
                    .into_iter()
                    .map(ide_core::IpcPath)
                    .collect(),
                modified: std::mem::take(&mut state.modified)
                    .into_iter()
                    .map(ide_core::IpcPath)
                    .collect(),
                removed: std::mem::take(&mut state.removed)
                    .into_iter()
                    .map(ide_core::IpcPath)
                    .collect(),
                ignore_rules: std::mem::take(&mut state.ignore_rules)
                    .into_iter()
                    .map(ide_core::IpcPath)
                    .collect(),
            })
        };
        let report = OverflowReport {
            document_patches: state.document_patches,
            file_changes,
            general: state.general,
            coalesced: std::mem::take(&mut state.coalesced).into_values().collect(),
            dropped,
        };
        state.document_patches = false;
        state.general = false;
        state.dropped = 0;
        report
    }

    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}

pub struct UiEvents {
    pub receiver: Receiver<IdeEvent>,
    pub overflow: Arc<EventOverflow>,
}

struct UiEventSink {
    sender: Sender<IdeEvent>,
    overflow: Arc<EventOverflow>,
}

impl ide_core::EventSink for UiEventSink {
    fn emit(&self, event: IdeEvent) {
        match self.sender.try_send(event) {
            Ok(()) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(event)) => {
                self.overflow.record(event);
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {}
        }
    }
}

fn same_window(window: &str) -> &str {
    window
}

/// Owns the OpenIDEA backend used by the native workbench. The backend is
/// independent of Tauri and WebView; the UI receives its events through a
/// bounded queue instead of IPC.
pub struct NativeHost {
    runtime: Arc<Runtime>,
    host: Arc<Host>,
    events: Mutex<Option<Receiver<IdeEvent>>>,
    overflow: Arc<EventOverflow>,
    plugins_dir: PathBuf,
    plugin_host: OnceLock<Result<Arc<ide_host::plugins::PluginHost>, String>>,
}

#[derive(Clone, Debug)]
pub struct StructuralReplacement {
    pub path: PathBuf,
    pub edits: Vec<ide_core::TextEdit>,
}

#[derive(Clone, Debug, Default)]
pub struct StructuralApplySummary {
    pub files: u32,
    pub replacements: u32,
    pub failed: u32,
}

impl NativeHost {
    pub fn config_dir(&self) -> PathBuf {
        self.host.config_dir.clone()
    }

    pub fn scratch_dir(&self) -> PathBuf {
        let dir = self.host.data_dir.join("scratches");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    pub fn start() -> Result<Arc<Self>> {
        let config_dir = ide_host::data_dirs::config_dir(IDENTIFIER)
            .map_err(anyhow::Error::msg)
            .context("resolving RustIdea config directory")?;
        let data_dir = ide_host::data_dirs::data_dir(IDENTIFIER)
            .map_err(anyhow::Error::msg)
            .context("resolving RustIdea data directory")?;
        let local_data_dir = ide_host::data_dirs::startup_local_data_dir(IDENTIFIER)
            .map_err(anyhow::Error::msg)
            .context("resolving RustIdea local data directory")?;
        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(&local_data_dir)?;

        let languages = Arc::new(
            LanguageRegistry::load(Some(&config_dir.join("languages")))
                .map_err(anyhow::Error::msg)
                .context("loading RustIdea language definitions")?,
        );
        let syntax = Arc::new(OnceLock::new());
        {
            let syntax = syntax.clone();
            std::thread::Builder::new()
                .name("rustidea-syntax-init".into())
                .spawn(move || {
                    syntax.get_or_init(|| Arc::new(Syntax::new()));
                })?;
        }

        ide_tools::extend_process_path(&local_data_dir);
        ide_dap::adapter::set_tools_root(ide_tools::tools_dir(&local_data_dir));
        ide_sdk::java::add_jdk_root(&ide_tools::jdk::jdks_dir(&local_data_dir));

        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_name("rustidea-backend")
                .enable_all()
                .build()?,
        );
        let (sender, receiver) = channel(EVENT_CAPACITY);
        let sink_sender = sender.clone();
        let overflow = Arc::new(EventOverflow::new());
        let sink_overflow = overflow.clone();
        let plugins_dir = data_dir.join("plugins");
        let host = Arc::new(Host::new(HostConfig {
            config_dir,
            data_dir,
            local_data_dir,
            languages,
            syntax,
            sink_for: Box::new(move |_| {
                Arc::new(UiEventSink {
                    sender: sink_sender.clone(),
                    overflow: sink_overflow.clone(),
                }) as SharedSink
            }),
            owner: same_window,
            runtime: runtime.handle().clone(),
            project_environment: false,
        }));
        let _ = sender;
        Ok(Arc::new(Self {
            runtime,
            host,
            events: Mutex::new(Some(receiver)),
            overflow,
            plugins_dir,
            plugin_host: OnceLock::new(),
        }))
    }

    pub fn host(&self) -> &Arc<Host> {
        &self.host
    }

    pub fn runtime(&self) -> &Arc<Runtime> {
        &self.runtime
    }

    pub fn take_events(&self) -> Option<UiEvents> {
        self.events.lock().take().map(|receiver| UiEvents {
            receiver,
            overflow: self.overflow.clone(),
        })
    }

    /// Runs an OpenIDEA operation on the host's Tokio runtime. Several backend
    /// ops call `tokio::spawn_blocking` directly and assume they were invoked
    /// from the host runtime; GPUI polls its own tasks on the main thread, so
    /// this adapter moves the whole operation to the backend executor instead
    /// of installing thread-local runtime state around a UI future.
    pub async fn run<F, Fut, T>(&self, operation: F) -> T
    where
        F: FnOnce(Arc<Host>) -> Fut + Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let host = self.host.clone();
        self.runtime
            .spawn(async move { operation(host).await })
            .await
            .expect("backend operation task panicked")
    }

    pub async fn open_project(
        &self,
        root: &Path,
    ) -> IdeResult<ide_host::ops::project::ProjectInfo> {
        let root = root.to_path_buf();
        self.run(move |host| async move {
            ide_host::ops::project::open(&host, MAIN_WINDOW, &root)
                .await
                .map(|project| ide_host::ops::project::info(&project))
        })
        .await
    }

    pub async fn open_light(
        &self,
        file: Option<&Path>,
    ) -> IdeResult<ide_host::ops::project::ProjectInfo> {
        let host = self.host.clone();
        let file = file.map(Path::to_path_buf);
        self.runtime
            .spawn_blocking(move || {
                let project = match file {
                    Some(file) => host.open_light(MAIN_WINDOW, &file)?,
                    None => host.open_light_empty(MAIN_WINDOW)?,
                };
                Ok(ide_host::ops::project::info(&project))
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn set_project_trust(
        &self,
        root: &Path,
        trusted: bool,
        folder: bool,
    ) -> IdeResult<()> {
        let root = root.to_path_buf();
        self.run(move |host| async move { host.set_project_trust(&root, trusted, folder).await })
            .await
    }

    pub fn settings(&self) -> ide_core::settings::Settings {
        ide_host::ops::project::settings(&self.host, MAIN_WINDOW)
    }

    pub async fn set_setting(
        &self,
        key: String,
        value: serde_json::Value,
    ) -> IdeResult<ide_core::settings::Settings> {
        let host = self.host.clone();
        self.runtime
            .spawn_blocking(move || {
                ide_host::ops::project::set_settings(
                    &host,
                    MAIN_WINDOW,
                    ide_workspace::settings::SettingsScope::Global,
                    &key,
                    &value.to_string(),
                )
                .map(|applied| applied.settings)
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub fn close_project(&self) {
        self.host.close_project_detached(MAIN_WINDOW);
    }

    pub async fn read_dir(&self, path: &Path) -> IdeResult<Vec<FileEntry>> {
        let path = path.to_path_buf();
        self.run(
            move |host| async move { ide_host::ops::fs::read_dir(&host, MAIN_WINDOW, path).await },
        )
        .await
    }

    pub async fn create_dir_async(&self, path: PathBuf) -> IdeResult<()> {
        self.runtime
            .spawn_blocking(move || ide_vfs::create_dir(&path))
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn open_document(&self, path: &Path) -> IdeResult<DocumentInfo> {
        let path = path.to_path_buf();
        self.run(
            move |host| async move { ide_host::ops::doc::open(&host, MAIN_WINDOW, path).await },
        )
        .await
    }

    pub async fn open_untitled(&self) -> IdeResult<DocumentInfo> {
        self.run(move |host| async move {
            ide_host::ops::doc::open_untitled(&host, MAIN_WINDOW, None).await
        })
        .await
    }

    pub fn close_document(&self, doc: DocId) -> IdeResult<()> {
        ide_host::ops::doc::close(&self.host, MAIN_WINDOW, doc)
    }

    pub fn document_info(&self, doc: DocId) -> IdeResult<DocumentInfo> {
        let project = self.host.project(MAIN_WINDOW)?;
        project.documents.snapshot(doc)
    }

    pub async fn document_info_async(&self, doc: DocId) -> IdeResult<DocumentInfo> {
        self.run(move |host| async move {
            let project = host.project(MAIN_WINDOW)?;
            project.documents.snapshot(doc)
        })
        .await
    }

    pub async fn apply_document_edit_async(
        &self,
        doc: DocId,
        base_version: u32,
        changes: ide_core::ChangeSet,
    ) -> IdeResult<u32> {
        self.run(move |host| async move {
            ide_host::ops::doc::edit(&host, MAIN_WINDOW, doc, base_version, changes)
        })
        .await
    }

    pub async fn reload_document_discarding_changes(&self, doc: DocId) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::doc::reload_discarding_changes(&host, MAIN_WINDOW, doc).await
        })
        .await
    }

    pub async fn save_document(&self, doc: DocId) -> IdeResult<()> {
        self.run(move |host| async move { ide_host::ops::doc::save(&host, MAIN_WINDOW, doc).await })
            .await
    }

    pub async fn save_document_as(&self, doc: DocId, path: PathBuf) -> IdeResult<DocumentInfo> {
        self.run(move |host| async move {
            ide_host::ops::doc::save_as(&host, MAIN_WINDOW, doc, path).await
        })
        .await
    }

    pub async fn save_file_copy(&self, path: PathBuf, text: String) -> IdeResult<()> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            tokio::task::spawn_blocking(move || {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, text)?;
                Ok::<_, std::io::Error>(())
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))
        })
        .await
    }

    pub async fn highlights_async(
        &self,
        language: ide_core::LanguageId,
        text: String,
    ) -> Option<ide_language::SyntaxHighlights> {
        let host = self.host.clone();
        self.runtime
            .spawn_blocking(move || host.syntax().highlights(&language, &text))
            .await
            .ok()
            .flatten()
    }

    pub async fn document_outline(&self, doc: DocId) -> IdeResult<Vec<ide_core::OutlineItem>> {
        self.run(move |host| async move {
            ide_host::ops::doc::doc_outline(&host, MAIN_WINDOW, doc).await
        })
        .await
    }

    pub async fn run_http(
        &self,
        text: String,
        index: u32,
        file: Option<PathBuf>,
        root: PathBuf,
    ) -> IdeResult<ide_http::HttpExchange> {
        self.runtime
            .spawn(async move {
                let cancel = ide_core::CancellationToken::new();
                ide_http::run(
                    ide_http::RunInput {
                        text: &text,
                        index,
                        file: file.as_deref(),
                        root: Some(&root),
                        env: None,
                    },
                    &cancel,
                )
                .await
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub fn db_sources(&self) -> IdeResult<Vec<ide_db::DataSource>> {
        ide_host::ops::db::sources(&self.host, MAIN_WINDOW)
    }

    pub fn db_connected(&self) -> IdeResult<Vec<String>> {
        ide_host::ops::db::connected(&self.host, MAIN_WINDOW)
    }

    pub async fn db_connect(&self, id: String) -> IdeResult<()> {
        self.run(
            move |host| async move { ide_host::ops::db::connect(&host, MAIN_WINDOW, &id).await },
        )
        .await
    }

    pub async fn db_execute(
        &self,
        id: String,
        text: String,
        from: u32,
        to: u32,
    ) -> IdeResult<Vec<ide_host::ops::db::StatementOutcome>> {
        self.run(move |host| async move {
            let statements =
                ide_host::ops::db::statements(&host, MAIN_WINDOW, &id, &text, from, to)?;
            ide_host::ops::db::execute(
                &host,
                MAIN_WINDOW,
                ide_core::RequestId::next(),
                &id,
                statements,
                0,
                200,
                None,
            )
            .await
        })
        .await
    }

    pub fn run_configs(&self) -> IdeResult<Vec<ide_run::RunConfig>> {
        ide_host::ops::run::configs(&self.host, MAIN_WINDOW)
    }

    pub async fn start_run(
        &self,
        config: ide_run::RunConfig,
        on_output: ide_host::ops::Emit<ide_run::OutputChunk>,
    ) -> IdeResult<ide_run::RunInfo> {
        self.run(move |host| async move {
            ide_host::ops::run::start(&host, MAIN_WINDOW, config, on_output).await
        })
        .await
    }

    pub fn stop_run(&self, run: ide_core::RunId) -> IdeResult<()> {
        ide_host::ops::run::stop(&self.host, MAIN_WINDOW, run)
    }

    pub async fn spawn_terminal(
        &self,
        options: ide_terminal::TerminalOptions,
        output: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> IdeResult<ide_terminal::TerminalInfo> {
        let host = self.host.clone();
        self.runtime
            .spawn_blocking(move || {
                let emit: ide_host::ops::Emit<String> = Arc::new(move |chunk| {
                    let _ = output.send(chunk);
                });
                ide_host::ops::term::spawn(&host, MAIN_WINDOW, options, emit)
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub fn write_terminal(&self, id: TerminalId, data: &str) -> IdeResult<()> {
        ide_host::ops::term::write(&self.host, id, data)
    }

    pub fn resize_terminal(&self, id: TerminalId, cols: u16, rows: u16) -> IdeResult<()> {
        ide_host::ops::term::resize(&self.host, id, cols, rows)
    }

    pub fn kill_terminal(&self, id: TerminalId) -> IdeResult<()> {
        ide_host::ops::term::kill(&self.host, id)
    }

    pub fn ssh_configs(&self) -> Vec<ide_core::remote_settings::SshConfiguration> {
        self.host.ssh.configs()
    }

    pub fn plugins_snapshot(&self) -> IdeResult<ide_host::plugins::PluginsSnapshot> {
        let host = self.plugin_host()?;
        host.load_all();
        Ok(host.list(None))
    }

    fn plugin_host(&self) -> IdeResult<Arc<ide_host::plugins::PluginHost>> {
        let host = self
            .plugin_host
            .get_or_init(|| {
                let sink: Arc<dyn ide_host::plugins::PluginEventSink> =
                    Arc::new(|_: Option<&str>, _: ide_host::plugins::PluginEvent| {});
                ide_host::plugins::PluginHost::new(
                    self.plugins_dir.clone(),
                    sink,
                    Default::default(),
                )
                .map(|host| {
                    host.set_default_market_url(Some(
                        ide_host::plugins::OFFICIAL_MARKET_INDEX.to_owned(),
                    ));
                    Arc::new(host)
                })
                .map_err(|error| error.message)
            })
            .as_ref()
            .map_err(|message| ide_core::IdeError::internal(message.clone()))?
            .clone();
        Ok(host)
    }

    pub async fn plugin_market(&self) -> IdeResult<Vec<ide_host::plugins::MarketPlugin>> {
        let host = self.plugin_host()?;
        self.runtime
            .spawn_blocking(move || host.market_listing(true))
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn plugin_preview(&self, id: String) -> IdeResult<ide_host::plugins::InstallPreview> {
        let host = self.plugin_host()?;
        self.runtime
            .spawn_blocking(move || {
                let dir = host.download_market_plugin(&id)?;
                host.preview_install(&dir)
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn plugin_install(
        &self,
        id: String,
        optional: Vec<String>,
        signer_change: bool,
    ) -> IdeResult<ide_host::plugins::PluginInfo> {
        let host = self.plugin_host()?;
        self.runtime
            .spawn_blocking(move || {
                let dir = host.download_market_plugin(&id)?;
                let consent = ide_host::plugins::Consent {
                    optional,
                    signer_change,
                };
                host.install_from_dir(&dir, &consent)
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub fn agent_providers(&self) -> IdeResult<Vec<ide_agent::AgentProvider>> {
        ide_host::ops::agent::providers(&self.host, MAIN_WINDOW)
    }

    pub fn agent_subscribe(
        &self,
        output: tokio::sync::mpsc::UnboundedSender<ide_agent::AgentEvent>,
    ) -> IdeResult<Vec<ide_agent::AgentTask>> {
        let emit: ide_host::ops::Emit<ide_agent::AgentEvent> = Arc::new(move |event| {
            let _ = output.send(event);
        });
        ide_host::ops::agent::subscribe(&self.host, MAIN_WINDOW, emit)
    }

    pub async fn agent_create(
        &self,
        provider: String,
        prompt: String,
    ) -> IdeResult<ide_agent::AgentTask> {
        self.run(move |host| async move {
            ide_host::ops::agent::create(
                &host,
                MAIN_WINDOW,
                ide_agent::NewAgentTask {
                    project_root: None,
                    prompt: ide_agent::AgentPrompt {
                        text: prompt,
                        files: Vec::new(),
                        images: Vec::new(),
                    },
                    provider,
                    config: Vec::new(),
                    isolation: ide_agent::Isolation::Local,
                    base: None,
                    mcp_overrides: Default::default(),
                },
            )
            .await
        })
        .await
    }

    pub fn mcp_list(&self) -> IdeResult<Vec<ide_agent::McpServer>> {
        ide_host::ops::agent::mcp_list(&self.host, MAIN_WINDOW)
    }

    pub fn mcp_set_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> IdeResult<Vec<ide_agent::McpServer>> {
        ide_host::ops::agent::mcp_set_enabled(&self.host, MAIN_WINDOW, id, enabled)
    }

    pub fn mcp_remove(&self, id: String) -> IdeResult<Vec<ide_agent::McpServer>> {
        ide_host::ops::agent::mcp_remove(&self.host, MAIN_WINDOW, id)
    }

    pub fn mcp_save(
        &self,
        input: ide_agent::McpServerInput,
    ) -> IdeResult<Vec<ide_agent::McpServer>> {
        ide_host::ops::agent::mcp_save(&self.host, MAIN_WINDOW, input)
    }

    pub async fn agent_prompt(&self, task: String, prompt: String) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::agent::prompt(
                &host,
                MAIN_WINDOW,
                task,
                ide_agent::AgentPrompt {
                    text: prompt,
                    files: Vec::new(),
                    images: Vec::new(),
                },
                false,
            )
            .await
        })
        .await
    }

    pub fn agent_cancel(&self, task: String) -> IdeResult<()> {
        ide_host::ops::agent::cancel(&self.host, MAIN_WINDOW, task, None)
    }

    pub fn agent_respond_permission(
        &self,
        task: String,
        permission: u32,
        option: Option<String>,
    ) -> IdeResult<()> {
        ide_host::ops::agent::respond(&self.host, MAIN_WINDOW, task, permission, option)
    }

    pub async fn tools_list(&self) -> IdeResult<Vec<ide_tools::ToolInfo>> {
        self.run(
            move |host| async move { ide_host::ops::tools::tools_list(&host, MAIN_WINDOW).await },
        )
        .await
    }

    pub async fn tools_install(&self, name: String) -> IdeResult<ide_tools::ToolInfo> {
        self.run(move |host| async move {
            let channel = ide_host::ops::Channel::new(|_event: ide_tools::InstallEvent| {});
            ide_host::ops::tools::tools_install(
                &host,
                MAIN_WINDOW,
                name,
                ide_core::RequestId::next(),
                channel,
            )
            .await
        })
        .await
    }

    pub async fn build_model(&self) -> IdeResult<ide_build::BuildModel> {
        self.run(
            move |host| async move { ide_host::ops::build::build_model(&host, MAIN_WINDOW).await },
        )
        .await
    }

    pub async fn coverage_import(&self, path: PathBuf) -> IdeResult<ide_coverage::CoverageSuite> {
        self.run(move |host| async move {
            ide_host::ops::coverage::coverage_import(&host, MAIN_WINDOW, path.into()).await
        })
        .await
    }

    pub async fn build_run(
        &self,
        kind: ide_build::compile::BuildKind,
        output: tokio::sync::mpsc::UnboundedSender<ide_build::compile::BuildEvent>,
    ) -> IdeResult<ide_build::compile::BuildResult> {
        self.run(move |host| async move {
            let channel =
                ide_host::ops::Channel::new(move |event: ide_build::compile::BuildEvent| {
                    let _ = output.send(event);
                });
            ide_host::ops::build::build_run(
                &host,
                MAIN_WINDOW,
                kind,
                None,
                None,
                ide_core::RequestId::next(),
                Vec::new(),
                channel,
            )
            .await
        })
        .await
    }

    pub async fn debug_attachables(&self) -> IdeResult<Vec<ide_dap::attach::AttachableProcess>> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_attachable_processes(&host, MAIN_WINDOW).await
        })
        .await
    }

    pub async fn debug_start(
        &self,
        config: ide_dap::LaunchConfig,
    ) -> IdeResult<ide_core::DebugSessionId> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_start(&host, MAIN_WINDOW, config).await
        })
        .await
    }

    pub fn debug_sessions(&self) -> IdeResult<Vec<ide_dap::SessionInfo>> {
        ide_host::ops::debug::debug_sessions(&self.host, MAIN_WINDOW)
    }

    pub async fn debug_stop(&self, session: ide_core::DebugSessionId) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_stop(&host, MAIN_WINDOW, session).await
        })
        .await
    }

    pub async fn debug_set_breakpoints(
        &self,
        path: PathBuf,
        breakpoints: Vec<ide_dap::Breakpoint>,
    ) -> IdeResult<Vec<ide_dap::BreakpointState>> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_set_breakpoints(&host, MAIN_WINDOW, path, breakpoints).await
        })
        .await
    }

    pub fn debug_breakpoint_states(
        &self,
        path: PathBuf,
    ) -> IdeResult<Vec<ide_dap::BreakpointState>> {
        ide_host::ops::debug::debug_breakpoint_states(&self.host, MAIN_WINDOW, path)
    }

    pub async fn debug_stack_trace(
        &self,
        session: ide_core::DebugSessionId,
        thread_id: i64,
    ) -> IdeResult<Vec<ide_dap::StackFrame>> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_stack_trace(&host, MAIN_WINDOW, session, thread_id).await
        })
        .await
    }

    pub async fn debug_scopes(
        &self,
        session: ide_core::DebugSessionId,
        frame_id: i64,
    ) -> IdeResult<Vec<ide_dap::Scope>> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_scopes(&host, MAIN_WINDOW, session, frame_id).await
        })
        .await
    }

    pub async fn debug_variables(
        &self,
        session: ide_core::DebugSessionId,
        reference: i64,
    ) -> IdeResult<Vec<ide_dap::Variable>> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_variables(&host, MAIN_WINDOW, session, reference).await
        })
        .await
    }

    pub async fn debug_evaluate(
        &self,
        session: ide_core::DebugSessionId,
        frame_id: Option<i64>,
        expression: String,
    ) -> IdeResult<ide_dap::Variable> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_evaluate(&host, MAIN_WINDOW, session, frame_id, expression)
                .await
        })
        .await
    }

    pub async fn debug_resume(
        &self,
        session: ide_core::DebugSessionId,
        thread_id: i64,
    ) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_resume(&host, MAIN_WINDOW, session, thread_id).await
        })
        .await
    }

    pub async fn debug_pause(
        &self,
        session: ide_core::DebugSessionId,
        thread_id: i64,
    ) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_pause(&host, MAIN_WINDOW, session, thread_id).await
        })
        .await
    }

    pub async fn debug_step(
        &self,
        session: ide_core::DebugSessionId,
        thread_id: i64,
        kind: ide_dap::StepKind,
    ) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::debug::debug_step(&host, MAIN_WINDOW, session, thread_id, kind).await
        })
        .await
    }

    pub async fn spawn_ssh_terminal(
        &self,
        id: String,
        cols: u16,
        rows: u16,
        output: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> IdeResult<ide_terminal::TerminalInfo> {
        let host = self.host.clone();
        self.runtime
            .spawn(async move {
                let emit: ide_host::ops::Emit<String> = Arc::new(move |chunk| {
                    let _ = output.send(chunk);
                });
                ide_host::ops::term::spawn_ssh(&host, MAIN_WINDOW, &id, cols, rows, emit).await
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn remote_list(
        &self,
        id: String,
        dir: Option<String>,
    ) -> IdeResult<(String, Vec<ide_ssh::sftp::RemoteEntry>)> {
        let host = self.host.clone();
        self.runtime
            .spawn(async move {
                let session = host.ssh.session(&id).await?;
                let fs = ide_ssh::filesystem::RemoteFs::new(session.sftp().await?);
                let dir = match dir {
                    Some(dir) => fs.resolve(&dir).await?,
                    None => fs.home().await?,
                };
                let entries = fs.list(&dir).await?;
                Ok((dir, entries))
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn remote_read(&self, id: String, path: String) -> IdeResult<String> {
        let host = self.host.clone();
        self.runtime
            .spawn(async move {
                let session = host.ssh.session(&id).await?;
                let fs = ide_ssh::filesystem::RemoteFs::new(session.sftp().await?);
                let bytes = fs.read(&path).await?;
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn remote_write(&self, id: String, path: String, data: String) -> IdeResult<()> {
        let host = self.host.clone();
        self.runtime
            .spawn(async move {
                let session = host.ssh.session(&id).await?;
                let fs = ide_ssh::filesystem::RemoteFs::new(session.sftp().await?);
                fs.write(&path, data.as_bytes(), None).await
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn notebook_open(&self, path: String) -> IdeResult<ide_notebook::NotebookSnapshot> {
        self.run(move |host| async move {
            ide_host::ops::notebook::open(&host, MAIN_WINDOW, path).await
        })
        .await
    }

    pub async fn notebook_execute(
        &self,
        id: String,
        cells: Vec<String>,
    ) -> IdeResult<ide_notebook::NotebookSnapshot> {
        self.run(move |host| async move {
            let channel = ide_host::ops::Channel::new(
                |_event: ide_host::ops::notebook::NotebookOutputEvent| {},
            );
            ide_host::ops::notebook::execute(
                &host,
                MAIN_WINDOW,
                id,
                cells,
                None,
                ide_core::RequestId::next(),
                channel,
            )
            .await
        })
        .await
    }

    pub async fn notebook_edit(
        &self,
        id: String,
        version: u32,
        edit: ide_notebook::NotebookEdit,
    ) -> IdeResult<ide_notebook::NotebookSnapshot> {
        self.run(move |host| async move {
            ide_host::ops::notebook::edit(&host, MAIN_WINDOW, id, version, edit).await
        })
        .await
    }

    pub async fn notebook_save(&self, id: String) -> IdeResult<ide_notebook::NotebookSnapshot> {
        self.run(
            move |host| async move { ide_host::ops::notebook::save(&host, MAIN_WINDOW, id).await },
        )
        .await
    }

    pub async fn start_search(
        &self,
        query: ide_search::SearchQuery,
        on_event: ide_host::ops::Emit<ide_host::ops::search::SearchEvent>,
    ) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::search::start(
                &host,
                MAIN_WINDOW,
                ide_core::RequestId::next(),
                query,
                on_event,
            )
            .await
        })
        .await
    }

    pub async fn replace_in_files(
        &self,
        query: ide_search::SearchQuery,
        replacement: String,
        preserve_case: bool,
        targets: Vec<ide_search::ReplaceTarget>,
        on_event: ide_host::ops::Channel<ide_search::ReplaceEvent>,
    ) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::search::replace_in_files(
                &host,
                MAIN_WINDOW,
                ide_core::RequestId::next(),
                query,
                replacement,
                preserve_case,
                targets,
                on_event,
            )
            .await
        })
        .await
    }

    pub async fn find_files(
        &self,
        query: String,
        limit: u32,
    ) -> IdeResult<Vec<ide_index::FileMatch>> {
        self.run(move |host| async move {
            ide_host::ops::nav::find_files(&host, MAIN_WINDOW, query, limit).await
        })
        .await
    }

    pub async fn find_symbols(
        &self,
        query: String,
        filter: ide_index::SymbolFilter,
        limit: u32,
    ) -> IdeResult<Vec<ide_index::SymbolMatch>> {
        self.run(move |host| async move {
            ide_host::ops::nav::find_symbols(&host, MAIN_WINDOW, query, filter, limit).await
        })
        .await
    }

    pub async fn lsp_references(
        &self,
        doc: DocId,
        pos: ide_core::Pos,
    ) -> IdeResult<Vec<ide_core::Location>> {
        self.run(move |host| async move {
            let partial = ide_host::ops::Channel::new(|_| {});
            ide_host::ops::lsp::lsp_references(
                &host,
                MAIN_WINDOW,
                ide_core::RequestId::next(),
                doc,
                pos,
                true,
                false,
                false,
                partial,
            )
            .await
            .map(|references| references.locations)
        })
        .await
    }

    pub async fn git_status(&self) -> IdeResult<Vec<ide_vcs::FileStatus>> {
        self.run(move |host| async move { ide_host::ops::vcs::status(&host, MAIN_WINDOW).await })
            .await
    }

    pub async fn git_branch(&self) -> IdeResult<ide_vcs::BranchInfo> {
        self.run(move |host| async move {
            ide_host::ops::vcs::vcs_branch_info(&host, MAIN_WINDOW, None).await
        })
        .await
    }

    pub async fn git_stage(&self, paths: Vec<PathBuf>) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::vcs::vcs_stage(&host, MAIN_WINDOW, paths).await
        })
        .await
    }

    pub async fn git_unstage(&self, paths: Vec<PathBuf>) -> IdeResult<()> {
        self.run(move |host| async move {
            ide_host::ops::vcs::vcs_unstage(&host, MAIN_WINDOW, paths).await
        })
        .await
    }

    pub async fn git_commit(&self, message: String) -> IdeResult<String> {
        self.run(move |host| async move {
            ide_host::ops::vcs::vcs_commit(
                &host,
                MAIN_WINDOW,
                None,
                message,
                ide_vcs::CommitOptions {
                    run_hooks: true,
                    ..Default::default()
                },
            )
            .await
        })
        .await
    }

    pub async fn git_log(&self, query: ide_vcs::LogQuery) -> IdeResult<Vec<ide_vcs::Commit>> {
        self.run(move |host| async move {
            let project = host.project(MAIN_WINDOW)?;
            project.with_repo_at(None, |repo| repo.log(&query))
        })
        .await
    }

    pub async fn docker_snapshot(&self) -> IdeResult<ide_docker::DockerSnapshot> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            tokio::task::spawn_blocking(|| {
                ide_docker::snapshot(&ide_docker::DockerEndpoint::Default)
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))
        })
        .await
    }

    pub async fn docker_container_op(
        &self,
        id: String,
        op: ide_docker::ContainerOp,
    ) -> IdeResult<()> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            tokio::task::spawn_blocking(move || {
                ide_docker::container_op(&ide_docker::DockerEndpoint::Default, &[id], op)
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
        })
        .await
    }

    pub async fn github_availability(
        &self,
        root: PathBuf,
    ) -> IdeResult<ide_github::GhAvailability> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            tokio::task::spawn_blocking(move || ide_github::availability(&root))
                .await
                .map_err(|error| ide_core::IdeError::internal(error.to_string()))
        })
        .await
    }

    pub async fn github_pr_list(
        &self,
        root: PathBuf,
        query: ide_github::PrListQuery,
    ) -> IdeResult<Vec<ide_github::PullRequestSummary>> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            tokio::task::spawn_blocking(move || ide_github::list(&root, &query))
                .await
                .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
        })
        .await
    }

    pub async fn github_pr_view(
        &self,
        root: PathBuf,
        number: u32,
    ) -> IdeResult<ide_github::PullRequestDetails> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            tokio::task::spawn_blocking(move || ide_github::view(&root, number))
                .await
                .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
        })
        .await
    }

    pub async fn github_review_comments(
        &self,
        root: PathBuf,
        number: u32,
    ) -> IdeResult<Vec<ide_github::PrReviewComment>> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            tokio::task::spawn_blocking(move || ide_github::review_comments(&root, number))
                .await
                .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
        })
        .await
    }

    pub async fn heap_open(&self, path: PathBuf) -> IdeResult<ide_host::ops::heap::HeapOpenResult> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            ide_host::ops::heap::debug_heap_open(
                &host,
                MAIN_WINDOW,
                path.into(),
                ide_core::RequestId::next(),
                ide_host::ops::Channel::new(|_| {}),
            )
            .await
        })
        .await
    }

    pub async fn heap_classes(
        &self,
        snapshot: ide_host::ops::heap::HeapSnapshotId,
    ) -> IdeResult<Vec<ide_host::ops::heap::HeapClassRow>> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            ide_host::ops::heap::debug_heap_classes(&host, MAIN_WINDOW, snapshot).await
        })
        .await
    }

    pub async fn heap_dominators(
        &self,
        snapshot: ide_host::ops::heap::HeapSnapshotId,
        class_index: u32,
    ) -> IdeResult<ide_host::ops::heap::HeapObjectPage> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            ide_host::ops::heap::debug_heap_dominators(
                &host,
                MAIN_WINDOW,
                snapshot,
                None,
                Some(class_index),
                0,
                200,
            )
            .await
        })
        .await
    }

    pub async fn heap_path(
        &self,
        snapshot: ide_host::ops::heap::HeapSnapshotId,
        object: ide_host::ops::heap::HeapObjectRef,
    ) -> IdeResult<Option<ide_host::ops::heap::HeapRootPath>> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            ide_host::ops::heap::debug_heap_path(&host, MAIN_WINDOW, snapshot, object).await
        })
        .await
    }

    pub async fn heap_close(&self, snapshot: ide_host::ops::heap::HeapSnapshotId) -> IdeResult<()> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            ide_host::ops::heap::debug_heap_close(&host, MAIN_WINDOW, snapshot).await
        })
        .await
    }

    pub async fn profile_collect(
        &self,
        spec: ide_profiler::ProfileSpec,
    ) -> IdeResult<ide_profiler::Profile> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            tokio::task::spawn_blocking(move || ide_profiler::collect(&spec))
                .await
                .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
        })
        .await
    }

    pub async fn analysis_duplicates(
        &self,
        options: ide_analysis::DuplicateOptions,
    ) -> IdeResult<ide_analysis::DuplicatesReport> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            ide_host::ops::analysis::analysis_locate_duplicates(
                &host,
                MAIN_WINDOW,
                ide_core::RequestId::next(),
                ide_search::SearchScope::Project,
                options,
            )
            .await
        })
        .await
    }

    pub async fn structural_search(
        &self,
        query: ide_analysis::StructuralQuery,
        on_event: ide_host::ops::Channel<ide_host::ops::analysis::StructuralEvent>,
    ) -> IdeResult<()> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            ide_host::ops::analysis::structural_search(
                &host,
                MAIN_WINDOW,
                ide_core::RequestId::next(),
                ide_search::SearchScope::Project,
                query,
                on_event,
            )
            .await
        })
        .await
    }

    pub async fn inspect_code(
        &self,
        on_event: ide_host::ops::Channel<ide_host::ops::analysis::InspectEvent>,
    ) -> IdeResult<()> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            ide_host::ops::analysis::inspect_code(
                &host,
                MAIN_WINDOW,
                ide_core::RequestId::next(),
                ide_search::SearchScope::Project,
                on_event,
            )
            .await
        })
        .await
    }

    pub async fn apply_structural_replacements(
        &self,
        replacements: Vec<StructuralReplacement>,
    ) -> IdeResult<StructuralApplySummary> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            let project = host.project(MAIN_WINDOW)?;
            let documents = project.documents.clone();
            tokio::task::spawn_blocking(move || {
                let mut summary = StructuralApplySummary::default();
                for replacement in replacements {
                    let mut edits = replacement.edits;
                    if edits.is_empty() {
                        continue;
                    }
                    edits.sort_by_key(|left| std::cmp::Reverse(left.range.start));
                    let (doc, transient) = match documents.by_path(&replacement.path) {
                        Some(doc) => (doc, false),
                        None => {
                            let Ok(info) = documents.open(&replacement.path) else {
                                summary.failed += 1;
                                continue;
                            };
                            (info.id, true)
                        }
                    };
                    match documents.apply_edits(doc, &edits) {
                        Ok(count) => {
                            summary.files += 1;
                            summary.replacements += count;
                            if transient {
                                if let Err(error) = documents.save(doc) {
                                    summary.failed += 1;
                                    tracing::debug!(
                                        "structural replacement save {}: {}",
                                        replacement.path.display(),
                                        error.message
                                    );
                                }
                                documents.close(doc);
                            }
                        }
                        Err(error) => {
                            summary.failed += 1;
                            tracing::debug!(
                                "structural replacement {}: {}",
                                replacement.path.display(),
                                error.message
                            );
                            if transient {
                                documents.close(doc);
                            }
                        }
                    }
                }
                Ok(summary)
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
        })
        .await
    }

    pub async fn logcat_devices(&self) -> IdeResult<Vec<crate::logcat::LogcatDevice>> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            tokio::task::spawn_blocking(crate::logcat::list_devices)
                .await
                .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
                .map_err(ide_core::IdeError::unavailable)
        })
        .await
    }

    pub async fn logcat_stream(
        &self,
        serial: String,
        on_line: ide_host::ops::Emit<crate::logcat::LogcatLine>,
        cancel: ide_core::CancellationToken,
    ) -> IdeResult<()> {
        self.run(move |host| async move {
            host.require_trust(MAIN_WINDOW)?;
            crate::logcat::stream_device(serial, move |line| on_line(line), cancel)
                .await
                .map_err(ide_core::IdeError::unavailable)
        })
        .await
    }
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        self.host.close_project_detached(MAIN_WINDOW);
        self.host.stop_processes_on_exit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ide_core::EventSink as _;
    use ide_core::IpcPath;

    fn paths(paths: &[&str]) -> Vec<IpcPath> {
        paths
            .iter()
            .map(|path| IpcPath(PathBuf::from(path)))
            .collect()
    }

    #[test]
    fn full_queue_coalesces_progress_but_keeps_recovery_flags() {
        let (sender, mut receiver) = channel(1);
        let overflow = Arc::new(EventOverflow::new());
        let sink = UiEventSink {
            sender,
            overflow: overflow.clone(),
        };
        sink.emit(IdeEvent::IndexState {
            indexing: true,
            files: 1,
            symbols: 0,
        });
        sink.emit(IdeEvent::IndexState {
            indexing: true,
            files: 2,
            symbols: 3,
        });
        sink.emit(IdeEvent::DocPatched {
            doc: DocId(1),
            base_version: 1,
            version: 2,
            changes: Vec::new(),
            workspace_edit: false,
        });
        assert!(receiver.try_recv().is_ok());
        let report = overflow.take_report();
        assert_eq!(report.dropped, 2);
        assert!(report.document_patches);
        assert_eq!(report.coalesced.len(), 1);
        assert!(matches!(
            &report.coalesced[0],
            IdeEvent::IndexState {
                files: 2,
                symbols: 3,
                ..
            }
        ));
    }

    #[test]
    fn filesystem_overflow_merges_every_path() {
        let overflow = EventOverflow::new();
        overflow.record(IdeEvent::FsChanged {
            created: paths(&["/p/a"]),
            modified: paths(&["/p/b"]),
            removed: Vec::new(),
            ignore_rules: Vec::new(),
        });
        overflow.record(IdeEvent::FsChanged {
            created: paths(&["/p/c"]),
            modified: paths(&["/p/b"]),
            removed: paths(&["/p/d"]),
            ignore_rules: paths(&["/p"]),
        });
        let report = overflow.take_report();
        let Some(IdeEvent::FsChanged {
            created,
            modified,
            removed,
            ignore_rules,
        }) = report.file_changes
        else {
            panic!("missing merged fs event");
        };
        assert_eq!(created.len(), 2);
        assert_eq!(modified.len(), 1, "duplicates coalesce");
        assert_eq!(removed.len(), 1);
        assert_eq!(ignore_rules.len(), 1);
        assert_eq!(report.dropped, 2);
    }
}
