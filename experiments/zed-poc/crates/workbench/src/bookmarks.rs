use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkStore {
    #[serde(default)]
    projects: BTreeMap<String, Vec<Bookmark>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub path: String,
    pub line: u32,
    #[serde(default)]
    pub text: String,
}

impl BookmarkStore {
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

    pub fn project(&self, root: &Path) -> &[Bookmark] {
        self.projects
            .get(&project_key(root))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn set_project(&mut self, root: &Path, mut bookmarks: Vec<Bookmark>) {
        bookmarks.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.line.cmp(&right.line))
        });
        bookmarks.dedup_by(|left, right| left.path == right.path && left.line == right.line);
        self.projects.insert(project_key(root), bookmarks);
    }
}

pub fn project_key(root: &Path) -> String {
    std::fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

pub fn toggle(bookmarks: &mut Vec<Bookmark>, path: PathBuf, line: u32, text: String) -> bool {
    let path = std::fs::canonicalize(&path).unwrap_or(path);
    let path = path.to_string_lossy().into_owned();
    if let Some(index) = bookmarks
        .iter()
        .position(|bookmark| bookmark.path == path && bookmark.line == line)
    {
        bookmarks.remove(index);
        false
    } else {
        bookmarks.push(Bookmark { path, line, text });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_adds_and_removes_the_same_line_and_sorts_them() {
        let mut bookmarks = Vec::new();
        assert!(toggle(
            &mut bookmarks,
            PathBuf::from("/repo/b.rs"),
            2,
            "b".into()
        ));
        assert!(toggle(
            &mut bookmarks,
            PathBuf::from("/repo/a.rs"),
            5,
            "a".into()
        ));
        assert!(!toggle(
            &mut bookmarks,
            PathBuf::from("/repo/b.rs"),
            2,
            "b".into()
        ));
        assert_eq!(bookmarks.len(), 1);
        let mut store = BookmarkStore::default();
        store.set_project(Path::new("/repo"), bookmarks);
        assert_eq!(store.project(Path::new("/repo")).len(), 1);
    }

    #[test]
    fn store_survives_a_missing_or_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bookmarks.json");
        assert!(BookmarkStore::load(&path).projects.is_empty());
        std::fs::write(&path, "{not json").unwrap();
        assert!(BookmarkStore::load(&path).projects.is_empty());
        let mut store = BookmarkStore::default();
        store.set_project(
            Path::new("/repo"),
            vec![Bookmark {
                path: "/repo/a.rs".into(),
                line: 3,
                text: "fn a()".into(),
            }],
        );
        store.save(&path).unwrap();
        let loaded = BookmarkStore::load(&path);
        assert_eq!(loaded.project(Path::new("/repo")).len(), 1);
        assert_eq!(loaded.project(Path::new("/repo"))[0].text, "fn a()");
    }
}
