use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{Context as _, Result};
use ide_core::{ChangeSet, DocId, IdeResult, NullSink, SharedSink};
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
}

impl OpenIdeaBridge {
    pub fn start(_root: &Path, file: &Path) -> Result<Self> {
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
            sink_for: Box::new(|_| Arc::new(NullSink) as SharedSink),
            owner: same_window,
            runtime: runtime.handle().clone(),
            project_environment: false,
        }));

        let info: DocumentInfo = runtime.block_on(async {
            let project = host.open_light(WINDOW, file)?;
            let _ = project;
            ide_host::ops::doc::open(&host, WINDOW, file.to_path_buf()).await
        })?;

        Ok(Self {
            runtime,
            host,
            doc: info.id,
            path: info.path.0,
            text: info.text,
            version: info.version,
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

    pub fn close(&self) {
        let _ = ide_host::ops::doc::close(&self.host, WINDOW, self.doc);
        self.host.close_project_detached(WINDOW);
    }
}
