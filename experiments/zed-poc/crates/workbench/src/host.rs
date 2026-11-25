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
}

impl NativeHost {
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
