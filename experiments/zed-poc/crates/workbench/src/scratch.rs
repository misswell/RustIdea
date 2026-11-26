use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScratchFile {
    pub path: PathBuf,
    pub name: String,
    pub modified_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScratchTemplate {
    pub label: &'static str,
    pub extension: &'static str,
    pub content: &'static str,
}

pub const TEMPLATES: &[ScratchTemplate] = &[
    ScratchTemplate {
        label: "Java",
        extension: "java",
        content: "public class Scratch {\n    public static void main(String[] args) {\n        System.out.println(\"Hello, world!\");\n    }\n}\n",
    },
    ScratchTemplate {
        label: "Python",
        extension: "py",
        content: "def main():\n    print(\"Hello, world!\")\n\nif __name__ == \"__main__\":\n    main()\n",
    },
    ScratchTemplate {
        label: "Rust",
        extension: "rs",
        content: "fn main() {\n    println!(\"Hello, world!\");\n}\n",
    },
    ScratchTemplate {
        label: "JS",
        extension: "js",
        content: "console.log(\"Hello, world!\");\n",
    },
    ScratchTemplate {
        label: "Markdown",
        extension: "md",
        content: "# Scratch\n\n",
    },
];

pub fn template_for(extension: &str) -> Option<&'static ScratchTemplate> {
    TEMPLATES.iter().find(|template| {
        template.extension.eq_ignore_ascii_case(extension)
            || template.label.eq_ignore_ascii_case(extension)
    })
}

pub fn sanitize_file_name(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw == "." || raw == ".." || raw.contains('/') || raw.contains('\\') {
        return None;
    }
    if Path::new(raw).extension().is_none() {
        Some(format!("{raw}.md"))
    } else {
        Some(raw.to_owned())
    }
}

pub fn list(dir: &Path) -> Vec<ScratchFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<ScratchFile> = entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| {
            let path = entry.path();
            let modified_ms = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or_default();
            ScratchFile {
                name: entry.file_name().to_string_lossy().into_owned(),
                path,
                modified_ms,
            }
        })
        .collect();
    files.sort_by(|left, right| {
        right
            .modified_ms
            .cmp(&left.modified_ms)
            .then_with(|| left.name.cmp(&right.name))
    });
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_names_are_flat_and_default_to_markdown() {
        assert_eq!(sanitize_file_name("notes").as_deref(), Some("notes.md"));
        assert_eq!(
            sanitize_file_name("Main.java").as_deref(),
            Some("Main.java")
        );
        assert_eq!(sanitize_file_name(""), None);
        assert_eq!(sanitize_file_name("../escape"), None);
        assert_eq!(sanitize_file_name("a/b.txt"), None);
    }

    #[test]
    fn lists_files_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("old.md"), "old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(dir.path().join("new.md"), "new").unwrap();
        let files = list(dir.path());
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "new.md");
    }

    #[test]
    fn templates_cover_common_languages_with_unique_extensions() {
        assert!(template_for("java").is_some());
        assert!(template_for("RUST").is_some());
        assert!(template_for("").is_none());
        let mut extensions: Vec<_> = TEMPLATES
            .iter()
            .map(|template| template.extension)
            .collect();
        extensions.sort_unstable();
        extensions.dedup();
        assert_eq!(extensions.len(), TEMPLATES.len());
        assert!(
            TEMPLATES
                .iter()
                .all(|template| !template.content.is_empty() && !template.label.is_empty())
        );
    }
}
