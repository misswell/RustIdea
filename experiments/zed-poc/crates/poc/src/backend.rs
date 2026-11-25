use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{Context as _, Result};
use ide_core::{ChangeSet, DocId, IdeEvent, IdeResult, SharedSink};
use ide_host::{Host, HostConfig};
use ide_language::{LanguageRegistry, Syntax};
use ide_workspace::DocumentInfo;
use tokio::runtime::Runtime;

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
        let host = Arc::new(Host::new(HostConfig {
            config_dir,
            data_dir,
            local_data_dir,
            languages,
            syntax,
            sink_for: Box::new(|_| {
                let sink = Arc::new(|event: IdeEvent| {
                    if let IdeEvent::LanguageServer {
                        name,
                        status,
                        message,
                        ..
                    } = event
                    {
                        eprintln!("OPENIDEA_LSP {name} {status:?} {message:?}");
                    }
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
        })
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
        let host = self.host.clone();
        let doc = self.doc;
        self.runtime.block_on(async move {
            ide_host::ops::lsp::lsp_hover(
                &host,
                WINDOW,
                ide_core::RequestId::next(),
                doc,
                ide_core::Pos::new(line, column),
            )
            .await
        })
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
