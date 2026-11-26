use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentStore {
    #[serde(default)]
    projects: BTreeMap<String, Vec<PathBuf>>,
}

impl RecentStore {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, text)?;
        std::fs::rename(temp, path)
    }

    pub fn project(&self, root: &Path) -> Vec<PathBuf> {
        self.projects
            .get(&crate::bookmarks::project_key(root))
            .cloned()
            .unwrap_or_default()
    }

    pub fn record(&mut self, root: &Path, path: &Path, limit: usize) {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let entries = self
            .projects
            .entry(crate::bookmarks::project_key(root))
            .or_default();
        entries.retain(|entry| entry != &canonical);
        entries.insert(0, canonical);
        entries.truncate(limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_moves_a_file_to_the_front_and_caps_the_list() {
        let mut store = RecentStore::default();
        for index in 0..3 {
            store.record(
                Path::new("/repo"),
                Path::new(&format!("/repo/{index}.rs")),
                2,
            );
        }
        store.record(Path::new("/repo"), Path::new("/repo/1.rs"), 2);
        let entries = store.project(Path::new("/repo"));
        assert_eq!(entries.len(), 2);
        assert!(entries[0].ends_with("1.rs"));
        assert!(entries[1].ends_with("2.rs"));
    }
}
