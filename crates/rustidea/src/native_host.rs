use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{Context as _, Result};
use ide_core::{DocId, IdeEvent, IdeResult, SharedSink};
use ide_host::{Host, HostConfig};
use ide_language::{LanguageRegistry, Syntax};
use ide_vfs::FileEntry;
use ide_workspace::DocumentInfo;
use parking_lot::Mutex;
use std::future::Future;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub const MAIN_WINDOW: &str = "main";
const IDENTIFIER: &str = "com.guofeng.rustidea";

struct UiEventSink {
    sender: UnboundedSender<IdeEvent>,
}

impl ide_core::EventSink for UiEventSink {
    fn emit(&self, event: IdeEvent) {
        let _ = self.sender.send(event);
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
    events: Mutex<Option<UnboundedReceiver<IdeEvent>>>,
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
        let (sender, receiver) = unbounded_channel();
        let sink_sender = sender.clone();
        let host = Arc::new(Host::new(HostConfig {
            config_dir,
            data_dir,
            local_data_dir,
            languages,
            syntax,
            sink_for: Box::new(move |_| {
                Arc::new(UiEventSink {
                    sender: sink_sender.clone(),
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
        }))
    }

    pub fn host(&self) -> &Arc<Host> {
        &self.host
    }

    pub fn runtime(&self) -> &Arc<Runtime> {
        &self.runtime
    }

    pub fn take_events(&self) -> Option<UnboundedReceiver<IdeEvent>> {
        self.events.lock().take()
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

    pub fn apply_document_edit(
        &self,
        doc: DocId,
        base_version: u32,
        changes: ide_core::ChangeSet,
    ) -> IdeResult<u32> {
        ide_host::ops::doc::edit(&self.host, MAIN_WINDOW, doc, base_version, changes)
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

    pub fn highlights(
        &self,
        language: &ide_core::LanguageId,
        text: &str,
    ) -> Option<ide_language::SyntaxHighlights> {
        self.host.syntax().highlights(language, text)
    }
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        self.host.close_project_detached(MAIN_WINDOW);
        self.host.stop_processes_on_exit();
    }
}
