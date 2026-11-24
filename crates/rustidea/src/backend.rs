use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

const MAX_TEXT_FILE_BYTES: u64 = 256 * 1024 * 1024;

pub trait RustIdeaBackend: Send + Sync {
    fn read_text(&self, path: &Path) -> Result<String>;
    fn write_text(&self, path: &Path, text: &str) -> Result<()>;
    fn list_directory(&self, path: &Path) -> Result<Vec<DirectoryEntry>>;
}

#[derive(Clone, Debug)]
pub struct DirectoryEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_directory: bool,
}

pub struct LocalBackend;

impl RustIdeaBackend for LocalBackend {
    fn read_text(&self, path: &Path) -> Result<String> {
        let metadata = fs::metadata(path)
            .with_context(|| format!("reading metadata for {}", path.display()))?;
        if !metadata.is_file() {
            bail!("{} is not a file", path.display());
        }
        if metadata.len() > MAX_TEXT_FILE_BYTES {
            bail!(
                "{} is too large ({:.1} MiB); the current native editor limit is {} MiB",
                path.display(),
                metadata.len() as f64 / (1024.0 * 1024.0),
                MAX_TEXT_FILE_BYTES / (1024 * 1024)
            );
        }
        let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        if bytes.iter().take(8192).any(|byte| *byte == 0) {
            bail!("{} appears to be a binary file", path.display());
        }
        String::from_utf8(bytes).with_context(|| format!("{} is not valid UTF-8", path.display()))
    }

    fn write_text(&self, path: &Path, text: &str) -> Result<()> {
        let resolved = if path.is_symlink() {
            fs::canonicalize(path).with_context(|| format!("resolving {}", path.display()))?
        } else {
            path.to_path_buf()
        };
        let path = resolved.as_path();
        let parent = path.parent().context("document has no parent directory")?;
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("creating temporary file in {}", parent.display()))?;
        if let Ok(metadata) = fs::metadata(path) {
            temporary
                .as_file()
                .set_permissions(metadata.permissions())?;
        }
        temporary.write_all(text.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(path)
            .with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
    }

    fn list_directory(&self, path: &Path) -> Result<Vec<DirectoryEntry>> {
        let mut entries = Vec::new();
        for entry in
            fs::read_dir(path).with_context(|| format!("reading directory {}", path.display()))?
        {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_ignored_name(&name) {
                continue;
            }
            entries.push(DirectoryEntry {
                path: entry.path(),
                name,
                is_directory: file_type.is_dir() && !file_type.is_symlink(),
            });
        }
        entries.sort_by(|left, right| {
            right
                .is_directory
                .cmp(&left.is_directory)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(entries)
    }
}

pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn is_ignored_name(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | ".DS_Store"
            | "target"
            | "node_modules"
            | ".next"
            | ".turbo"
            | "DerivedData"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested").join("sample.txt");
        let backend = LocalBackend;
        backend.write_text(&path, "first").unwrap();
        backend.write_text(&path, "second").unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "second");
    }

    #[cfg(unix)]
    #[test]
    fn write_preserves_permissions_and_symlink() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.txt");
        let link = directory.path().join("link.txt");
        fs::write(&target, "old").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
        symlink(&target, &link).unwrap();
        LocalBackend.write_text(&link, "new").unwrap();
        assert!(link.is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }

    #[test]
    fn directory_listing_ignores_vendor_folders_and_sorts_dirs_first() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("src")).unwrap();
        fs::create_dir(directory.path().join("target")).unwrap();
        fs::write(directory.path().join("Cargo.toml"), "").unwrap();
        let entries = LocalBackend.list_directory(directory.path()).unwrap();
        assert_eq!(entries[0].name, "src");
        assert_eq!(entries[1].name, "Cargo.toml");
        assert!(!entries.iter().any(|entry| entry.name == "target"));
    }
}
