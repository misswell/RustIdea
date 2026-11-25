use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::theme::ThemeMode;

const MAX_RECENT_PROJECTS: usize = 12;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub theme: ThemeMode,
    pub autosave: bool,
    pub recent_projects: VecDeque<PathBuf>,
    pub last_project: Option<PathBuf>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            autosave: true,
            recent_projects: VecDeque::new(),
            last_project: None,
        }
    }
}

impl Session {
    pub fn remember_project(&mut self, path: &Path) {
        self.recent_projects.retain(|recent| recent != path);
        self.recent_projects.push_front(path.to_path_buf());
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
        self.last_project = Some(path.to_path_buf());
    }
}

#[derive(Clone)]
pub struct SessionStore {
    path: Option<PathBuf>,
}

impl SessionStore {
    pub fn discover() -> Self {
        let path =
            dirs::data_dir().map(|directory| directory.join("RustIdea").join("session.json"));
        Self { path }
    }

    pub fn load(&self) -> Session {
        let Some(path) = self.path.as_ref() else {
            return Session::default();
        };
        let Ok(contents) = fs::read_to_string(path) else {
            return Session::default();
        };
        serde_json::from_str(&contents).unwrap_or_else(|error| {
            tracing::warn!(%error, path = %path.display(), "ignoring invalid RustIdea session");
            Session::default()
        })
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        let parent = path
            .parent()
            .context("session file has no parent directory")?;
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        let temporary = path.with_extension("json.tmp");
        let serialized = serde_json::to_vec_pretty(session)?;
        {
            let mut file = fs::File::create(&temporary)
                .with_context(|| format!("creating {}", temporary.display()))?;
            file.write_all(&serialized)?;
            file.sync_all()?;
        }
        fs::rename(&temporary, path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
    }
}
