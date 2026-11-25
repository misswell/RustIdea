use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context as _, Result};
use ide_core::{ChangeSet, DocId, IdeEvent, IdeResult, SharedSink};
use ide_host::{Host, HostConfig};
use ide_language::{LanguageRegistry, Syntax};
use ide_workspace::DocumentInfo;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

pub const WINDOW: &str = "main";
const IDENTIFIER: &str = "com.guofeng.rustidea.zedpoc";

fn same_window(window: &str) -> &str {
    window
}

/// The OpenIdea half of the bridge: one backend host and one document, with no
/// Tauri/WebView and no second language-server/index runtime.
pub struct OpenIdeaBridge {
    runtime: Arc<Runtime>,
    host: Arc<Host>,
    doc: DocId,
    pub path: PathBuf,
    pub text: String,
    pub version: u32,
    pub language: String,
    events: Mutex<Option<UnboundedReceiver<IdeEvent>>>,
}

impl OpenIdeaBridge {
    pub fn start(root: &Path, file: &Path) -> Result<Self> {
        let config_dir = ide_host::data_dirs::config_dir(IDENTIFIER)
            .map_err(anyhow::Error::msg)
            .context("resolve config directory")?;
        let data_dir = ide_host::data_dirs::data_dir(IDENTIFIER)
            .map_err(anyhow::Error::msg)
            .context("resolve data directory")?;
        let local_data_dir = ide_host::data_dirs::startup_local_data_dir(IDENTIFIER)
            .map_err(anyhow::Error::msg)
            .context("resolve local data directory")?;
        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(&local_data_dir)?;

        let languages = Arc::new(
            LanguageRegistry::load(Some(&config_dir.join("languages")))
                .map_err(anyhow::Error::msg)
                .context("load OpenIdea languages")?,
        );
        let syntax = Arc::new(OnceLock::new());
        syntax.get_or_init(|| Arc::new(Syntax::new()));

        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("rustidea-zed-backend")
                .enable_all()
                .build()?,
        );
        let (event_tx, event_rx) = unbounded_channel();
        let host = Arc::new(Host::new(HostConfig {
            config_dir,
            data_dir,
            local_data_dir,
            languages,
            syntax,
            sink_for: Box::new(move |_| {
                let event_tx = event_tx.clone();
                let sink = Arc::new(move |event: IdeEvent| {
                    if let IdeEvent::LanguageServer {
                        name,
                        status,
                        message,
                        ..
                    } = &event
                    {
                        eprintln!("OPENIDEA_LSP {name} {status:?} {message:?}");
                    }
                    let _ = event_tx.send(event);
                }) as SharedSink;
                sink
            }),
            owner: same_window,
            runtime: runtime.handle().clone(),
            project_environment: false,
        }));

        let info: DocumentInfo = runtime.block_on(async {
            ide_host::ops::project::open(&host, WINDOW, root).await?;
            if std::env::var_os("RUSTIDEA_POC_TRUST").is_some() {
                host.set_project_trust(root, true, false).await?;
            }
            ide_host::ops::doc::open(&host, WINDOW, file.to_path_buf()).await
        })?;

        Ok(Self {
            runtime,
            host,
            doc: info.id,
            path: info.path.0,
            text: info.text,
            version: info.version,
            language: info.language.to_string(),
            events: Mutex::new(Some(event_rx)),
        })
    }

    pub fn take_events(&self) -> Option<UnboundedReceiver<IdeEvent>> {
        self.events.lock().ok()?.take()
    }

    pub fn doc(&self) -> DocId {
        self.doc
    }

    pub async fn edit(&self, base_version: u32, changes: ChangeSet) -> IdeResult<u32> {
        let host = self.host.clone();
        let doc = self.doc;
        self.runtime
            .spawn(async move { ide_host::ops::doc::edit(&host, WINDOW, doc, base_version, changes) })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn save(&self) -> IdeResult<()> {
        let host = self.host.clone();
        let doc = self.doc;
        self.runtime
            .spawn(async move { ide_host::ops::doc::save(&host, WINDOW, doc).await })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn completion(
        &self,
        line: u32,
        column: u32,
        trigger: Option<String>,
    ) -> IdeResult<ide_lsp::types::CompletionList> {
        let host = self.host.clone();
        let doc = self.doc;
        self.runtime
            .spawn(async move {
                ide_host::ops::lsp::lsp_completion(
                    &host,
                    WINDOW,
                    ide_core::RequestId::next(),
                    doc,
                    ide_core::Pos::new(line, column),
                    trigger,
                )
                .await
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub fn completion_blocking(
        &self,
        line: u32,
        column: u32,
        trigger: Option<String>,
    ) -> IdeResult<ide_lsp::types::CompletionList> {
        self.runtime
            .block_on(self.completion(line, column, trigger))
    }

    pub fn servers(&self) -> IdeResult<Vec<ide_lsp::types::ServerInfo>> {
        let project = self.host.project(WINDOW)?;
        Ok(project.lsp.servers())
    }

    pub fn hover_blocking(
        &self,
        line: u32,
        column: u32,
    ) -> IdeResult<Option<ide_lsp::types::Hover>> {
        self.runtime.block_on(self.hover(line, column))
    }

    pub async fn hover(
        &self,
        line: u32,
        column: u32,
    ) -> IdeResult<Option<ide_lsp::types::Hover>> {
        let host = self.host.clone();
        let doc = self.doc;
        self.runtime
            .spawn(async move {
                ide_host::ops::lsp::lsp_hover(
                    &host,
                    WINDOW,
                    ide_core::RequestId::next(),
                    doc,
                    ide_core::Pos::new(line, column),
                )
                .await
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn definition(
        &self,
        line: u32,
        column: u32,
        kind: ide_lsp::types::GotoKind,
    ) -> IdeResult<Vec<ide_core::Location>> {
        let host = self.host.clone();
        let doc = self.doc;
        self.runtime
            .spawn(async move {
                ide_host::ops::lsp::lsp_goto(
                    &host,
                    WINDOW,
                    doc,
                    ide_core::Pos::new(line, column),
                    kind,
                )
                .await
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub fn definition_blocking(
        &self,
        line: u32,
        column: u32,
        kind: ide_lsp::types::GotoKind,
    ) -> IdeResult<Vec<ide_core::Location>> {
        self.runtime.block_on(self.definition(line, column, kind))
    }

    pub async fn prepare_rename(
        &self,
        line: u32,
        column: u32,
    ) -> IdeResult<Option<ide_core::Range>> {
        let host = self.host.clone();
        let doc = self.doc;
        self.runtime
            .spawn(async move {
                ide_host::ops::lsp::lsp_prepare_rename(
                    &host,
                    WINDOW,
                    doc,
                    ide_core::Pos::new(line, column),
                )
                .await
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub fn prepare_rename_blocking(
        &self,
        line: u32,
        column: u32,
    ) -> IdeResult<Option<ide_core::Range>> {
        self.runtime.block_on(self.prepare_rename(line, column))
    }

    pub async fn rename(
        &self,
        line: u32,
        column: u32,
        new_name: String,
    ) -> IdeResult<()> {
        let host = self.host.clone();
        let doc = self.doc;
        self.runtime
            .spawn(async move {
                let renamed = ide_host::ops::lsp::lsp_rename(
                    &host,
                    WINDOW,
                    ide_core::RequestId::next(),
                    doc,
                    ide_core::Pos::new(line, column),
                    new_name,
                )
                .await?;
                ide_host::ops::lsp::apply_workspace_edit(&host, WINDOW, renamed.edit).await
            })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub async fn open_target(&self, path: PathBuf) -> IdeResult<DocumentInfo> {
        let host = self.host.clone();
        self.runtime
            .spawn(async move { ide_host::ops::doc::open(&host, WINDOW, path).await })
            .await
            .map_err(|error| ide_core::IdeError::internal(error.to_string()))?
    }

    pub fn rename_blocking(
        &self,
        line: u32,
        column: u32,
        new_name: String,
    ) -> IdeResult<()> {
        self.runtime.block_on(self.rename(line, column, new_name))
    }

    pub fn reopen_blocking(&mut self) -> IdeResult<DocumentInfo> {
        let host = self.host.clone();
        let path = self.path.clone();
        let doc = self.doc;
        let info = self.runtime.block_on(async move {
            ide_host::ops::doc::close(&host, WINDOW, doc)?;
            ide_host::ops::doc::open(&host, WINDOW, path).await
        })?;
        self.doc = info.id;
        self.path = info.path.0.clone();
        self.text = info.text.clone();
        self.version = info.version;
        self.language = info.language.to_string();
        Ok(info)
    }

    pub fn close(&self) {
        let _ = ide_host::ops::doc::close(&self.host, WINDOW, self.doc);
        self.host.close_project_detached(WINDOW);
    }
}
