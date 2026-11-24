use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ide_vfs::{EntryKind, FileEntry};

/// One flattened, visible project-tree row. The model below materializes this
/// list only from cached directory listings, so expanding a previously loaded
/// directory never touches the filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectNode {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_directory: bool,
    pub expanded: bool,
    pub ignored: bool,
    pub excluded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Toggle {
    OpenFile(PathBuf),
    LoadDirectory(PathBuf),
    Rebuilt,
    Missing,
}

#[derive(Debug)]
pub struct ProjectTree {
    root: PathBuf,
    entries: HashMap<PathBuf, Vec<FileEntry>>,
    expanded: HashSet<PathBuf>,
    loading: HashSet<PathBuf>,
    nodes: Arc<Vec<ProjectNode>>,
    error: Option<String>,
}

impl ProjectTree {
    pub fn new(root: PathBuf) -> Self {
        let mut expanded = HashSet::new();
        expanded.insert(root.clone());
        Self {
            root,
            entries: HashMap::new(),
            expanded,
            loading: HashSet::new(),
            nodes: Arc::new(Vec::new()),
            error: None,
        }
    }

    pub fn nodes(&self) -> Arc<Vec<ProjectNode>> {
        self.nodes.clone()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_loading(&self) -> bool {
        !self.loading.is_empty()
    }

    pub fn mark_loading(&mut self, dir: PathBuf) {
        self.loading.insert(dir);
    }

    pub fn loaded_dirs(&self) -> Vec<PathBuf> {
        self.entries.keys().cloned().collect()
    }

    /// Applies a background directory read. If the directory was collapsed
    /// while the read was in flight the listing stays cached but is not
    /// materialized until it is expanded again.
    pub fn apply_directory(&mut self, dir: PathBuf, listing: Vec<FileEntry>) {
        self.loading.remove(&dir);
        self.entries.insert(dir, listing);
        self.error = None;
        self.rebuild();
    }

    pub fn apply_error(&mut self, dir: PathBuf, message: String) {
        self.loading.remove(&dir);
        if self.expanded.contains(&dir) {
            self.error = Some(format!("{}: {message}", dir.display()));
        }
    }

    /// Toggles a row and returns whether a background directory read is
    /// needed. Cached directories rebuild synchronously with zero I/O.
    pub fn toggle(&mut self, index: usize) -> Toggle {
        let Some(node) = self.nodes.get(index).cloned() else {
            return Toggle::Missing;
        };
        if !node.is_directory {
            return Toggle::OpenFile(node.path);
        }
        if self.expanded.remove(&node.path) {
            self.rebuild();
            return Toggle::Rebuilt;
        }
        self.expanded.insert(node.path.clone());
        self.rebuild();
        if self.entries.contains_key(&node.path) {
            Toggle::Rebuilt
        } else {
            self.loading.insert(node.path.clone());
            Toggle::LoadDirectory(node.path)
        }
    }

    pub fn collapse(&mut self, path: &Path) -> bool {
        let removed = self.expanded.remove(path);
        if removed {
            self.rebuild();
        }
        removed
    }

    /// Expands a directory by path. Returns `true` when its listing is not
    /// cached and a background read is required.
    pub fn expand(&mut self, path: &Path) -> bool {
        let inserted = self.expanded.insert(path.to_path_buf());
        self.rebuild();
        inserted && !self.entries.contains_key(path)
    }

    /// Expands every ancestor of `path` that belongs to this project and
    /// returns the directories whose listings still need to be loaded.
    pub fn reveal_path(&mut self, path: &Path) -> Vec<PathBuf> {
        let mut chain = Vec::new();
        let mut ancestor = path.parent();
        while let Some(directory) = ancestor {
            if !directory.starts_with(&self.root) {
                break;
            }
            chain.push(directory.to_path_buf());
            if directory == self.root {
                break;
            }
            ancestor = directory.parent();
        }
        chain.reverse();
        let mut missing = Vec::new();
        for directory in chain {
            self.expanded.insert(directory.clone());
            if !self.entries.contains_key(&directory) {
                self.loading.insert(directory.clone());
                missing.push(directory);
            }
        }
        self.rebuild();
        missing
    }

    /// Drops cached listings for `dir` and all descendants. Returns the
    /// directories that were loaded and should be re-read by a refresh.
    pub fn invalidate(&mut self, dir: &Path) -> Vec<PathBuf> {
        let removed: Vec<PathBuf> = self
            .entries
            .keys()
            .filter(|loaded| loaded.starts_with(dir))
            .cloned()
            .collect();
        for loaded in &removed {
            self.entries.remove(loaded);
        }
        if !removed.is_empty() {
            self.rebuild();
        }
        removed
    }

    /// Invalidates only the directories that are already loaded. Manual
    /// refresh uses this so it never walks unexpanded subtrees.
    pub fn invalidate_loaded(&mut self) -> Vec<PathBuf> {
        let loaded = self.loaded_dirs();
        self.entries.clear();
        self.rebuild();
        loaded
    }

    pub fn note_external_changes(
        &mut self,
        changed: &[PathBuf],
        removed: &[PathBuf],
    ) -> Vec<PathBuf> {
        let mut parents = HashSet::new();
        let mut reload = Vec::new();
        for path in changed.iter().chain(removed) {
            let parent = path.parent().unwrap_or(&self.root);
            if parent.starts_with(&self.root) {
                parents.insert(parent.to_path_buf());
            }
            // A directory event also invalidates that directory's cached
            // subtree; its parent row is the one that must be re-read.
            if self.entries.contains_key(path) {
                let invalidated = self.invalidate(path);
                for directory in invalidated {
                    self.loading.insert(directory.clone());
                    if self.expanded.contains(&directory)
                        && !removed.iter().any(|path| path == &directory)
                    {
                        reload.push(directory);
                    }
                }
            }
        }
        for parent in parents {
            if self.entries.contains_key(&parent) && !self.loading.contains(&parent) {
                self.entries.remove(&parent);
                reload.push(parent);
            }
        }
        if !reload.is_empty() {
            self.rebuild();
        }
        reload.sort();
        reload.dedup();
        reload
    }

    pub fn rebuild(&mut self) {
        let mut nodes = Vec::new();
        self.visit(&self.root.clone(), 0, &mut nodes);
        self.nodes = Arc::new(nodes);
    }

    fn visit(&self, directory: &Path, depth: usize, nodes: &mut Vec<ProjectNode>) {
        let Some(entries) = self.entries.get(directory) else {
            return;
        };
        for entry in entries {
            let path = entry.path.0.clone();
            let is_directory = matches!(entry.kind, EntryKind::Dir)
                || matches!(entry.target_kind, Some(EntryKind::Dir));
            nodes.push(ProjectNode {
                path: path.clone(),
                name: entry.name.clone(),
                depth,
                is_directory,
                expanded: is_directory && self.expanded.contains(&path),
                ignored: entry.ignored,
                excluded: entry.excluded,
            });
            if is_directory && self.expanded.contains(&path) {
                self.visit(&path, depth + 1, nodes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, name: &str, kind: EntryKind) -> FileEntry {
        FileEntry {
            name: name.into(),
            path: ide_core::IpcPath(PathBuf::from(path)),
            kind,
            target_kind: None,
            size: 0,
            mtime_ms: 0.0,
            hidden: false,
            ignored: false,
            excluded: false,
        }
    }

    fn fixture() -> ProjectTree {
        let root = PathBuf::from("/project");
        let mut tree = ProjectTree::new(root.clone());
        tree.apply_directory(
            root.clone(),
            vec![
                entry("/project/src", "src", EntryKind::Dir),
                entry("/project/README.md", "README.md", EntryKind::File),
            ],
        );
        tree.apply_directory(
            PathBuf::from("/project/src"),
            vec![entry("/project/src/main.rs", "main.rs", EntryKind::File)],
        );
        tree
    }

    #[test]
    fn cached_expand_and_collapse_do_no_io() {
        let mut tree = fixture();
        let src_index = tree
            .nodes()
            .iter()
            .position(|node| node.name == "src")
            .unwrap();
        assert_eq!(
            tree.toggle(src_index),
            Toggle::Rebuilt,
            "a cached directory must not schedule another read"
        );
        assert!(tree.nodes().iter().any(|node| node.name == "main.rs"));
        assert_eq!(
            tree.toggle(src_index),
            Toggle::Rebuilt,
            "collapsing a cached subtree must be a CPU-only operation"
        );
        assert!(!tree.nodes().iter().any(|node| node.name == "main.rs"));
    }

    #[test]
    fn unloaded_expand_requests_one_directory() {
        let mut tree = ProjectTree::new(PathBuf::from("/project"));
        tree.apply_directory(
            PathBuf::from("/project"),
            vec![entry("/project/src", "src", EntryKind::Dir)],
        );
        let src = tree.nodes()[0].clone();
        assert_eq!(tree.toggle(0), Toggle::LoadDirectory(src.path));
        assert!(tree.is_loading());
    }

    #[test]
    fn invalidate_drops_only_loaded_descendants() {
        let mut tree = fixture();
        tree.apply_directory(
            PathBuf::from("/project/src"),
            vec![
                entry("/project/src/main.rs", "main.rs", EntryKind::File),
                entry("/project/src/lib", "lib", EntryKind::Dir),
            ],
        );
        tree.apply_directory(
            PathBuf::from("/project/src/lib"),
            vec![entry("/project/src/lib.rs", "lib.rs", EntryKind::File)],
        );
        let removed = tree.invalidate(Path::new("/project/src"));
        assert_eq!(removed.len(), 2);
        assert!(tree.nodes().iter().all(|node| node.name != "main.rs"));
        assert!(tree.nodes().iter().all(|node| node.name != "lib.rs"));
    }

    #[test]
    fn reveal_returns_only_missing_ancestors() {
        let mut tree = fixture();
        let root = PathBuf::from("/project");
        let missing = tree.reveal_path(Path::new("/project/src/generated/deep/file.rs"));
        assert_eq!(
            missing,
            vec![root.join("src/generated"), root.join("src/generated/deep")]
        );
    }

    #[test]
    fn file_change_reloads_only_its_loaded_parent() {
        let mut tree = fixture();
        let src = tree
            .nodes()
            .iter()
            .position(|node| node.name == "src")
            .unwrap();
        tree.toggle(src);
        let reload = tree.note_external_changes(&[PathBuf::from("/project/src/main.rs")], &[]);
        assert_eq!(reload, vec![PathBuf::from("/project/src")]);
        assert!(tree.nodes().iter().all(|node| node.name != "main.rs"));
    }

    #[test]
    fn removed_directory_is_not_rescanned() {
        let mut tree = fixture();
        let src = tree
            .nodes()
            .iter()
            .position(|node| node.name == "src")
            .unwrap();
        tree.toggle(src);
        let reload = tree.note_external_changes(&[], &[PathBuf::from("/project/src")]);
        assert_eq!(reload, vec![PathBuf::from("/project")]);
        assert!(tree.nodes().iter().all(|node| node.name != "src"));
    }
}
