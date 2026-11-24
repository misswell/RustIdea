use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;

use crate::backend::RustIdeaBackend;

const MAX_VISIBLE_ENTRIES: usize = 8_000;

#[derive(Clone, Debug)]
pub struct ProjectNode {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_directory: bool,
    pub expanded: bool,
}

pub struct ProjectTree {
    root: PathBuf,
    backend: Arc<dyn RustIdeaBackend>,
    expanded: HashSet<PathBuf>,
    nodes: Vec<ProjectNode>,
    error: Option<String>,
}

impl ProjectTree {
    pub fn open(root: PathBuf, backend: Arc<dyn RustIdeaBackend>) -> Self {
        let mut tree = Self {
            expanded: HashSet::from([root.clone()]),
            root,
            backend,
            nodes: Vec::new(),
            error: None,
        };
        tree.reload();
        tree
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn nodes(&self) -> &[ProjectNode] {
        &self.nodes
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn toggle(&mut self, index: usize) {
        let Some(node) = self.nodes.get(index).cloned() else {
            return;
        };
        if !node.is_directory {
            return;
        }
        if !self.expanded.remove(&node.path) {
            self.expanded.insert(node.path);
        }
        self.reload();
    }

    pub fn reveal(&mut self, path: &Path) {
        let mut ancestor = path.parent();
        while let Some(directory) = ancestor {
            if !directory.starts_with(&self.root) {
                break;
            }
            self.expanded.insert(directory.to_path_buf());
            if directory == self.root {
                break;
            }
            ancestor = directory.parent();
        }
        self.reload();
    }

    pub fn reload(&mut self) {
        self.nodes.clear();
        self.error = None;
        self.visit_directory(&self.root.clone(), 0);
    }

    fn visit_directory(&mut self, directory: &Path, depth: usize) {
        if self.nodes.len() >= MAX_VISIBLE_ENTRIES {
            self.error = Some(format!(
                "project tree truncated at {MAX_VISIBLE_ENTRIES} visible entries"
            ));
            return;
        }
        let entries = match self.backend.list_directory(directory) {
            Ok(entries) => entries,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        for entry in entries {
            if self.nodes.len() >= MAX_VISIBLE_ENTRIES {
                self.error = Some(format!(
                    "project tree truncated at {MAX_VISIBLE_ENTRIES} visible entries"
                ));
                return;
            }
            let expanded = entry.is_directory && self.expanded.contains(&entry.path);
            let path = entry.path;
            self.nodes.push(ProjectNode {
                path: path.clone(),
                name: entry.name,
                depth,
                is_directory: entry.is_directory,
                expanded,
            });
            if expanded {
                self.visit_directory(&path, depth + 1);
            }
        }
    }
}

pub fn open_directory(path: PathBuf, backend: Arc<dyn RustIdeaBackend>) -> Result<ProjectTree> {
    if !path.is_dir() {
        anyhow::bail!("{} is not a directory", path.display());
    }
    Ok(ProjectTree::open(path, backend))
}
