#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Hunk,
    Context,
    Added,
    Removed,
    Meta,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub added: u32,
    pub removed: u32,
    pub binary: bool,
    pub lines: Vec<DiffLine>,
}

pub fn parse(patch: &str) -> Vec<DiffFile> {
    let mut files = Vec::new();
    let mut current: Option<DiffFile> = None;
    let mut old_path: Option<String> = None;
    let mut old_line = 0u32;
    let mut new_line = 0u32;
    let mut in_hunk = false;

    for raw in patch.lines() {
        if let Some(rest) = raw.strip_prefix("diff --git ") {
            finish(&mut files, &mut current);
            old_path = None;
            in_hunk = false;
            let mut fields = rest.split_whitespace();
            let fallback_old = fields.next().and_then(diff_path);
            let fallback_new = fields.next().and_then(diff_path);
            let path = fallback_new
                .clone()
                .or_else(|| fallback_old.clone())
                .unwrap_or_default();
            current = Some(DiffFile {
                path,
                old_path: fallback_old,
                added: 0,
                removed: 0,
                binary: false,
                lines: Vec::new(),
            });
            continue;
        }
        let Some(file) = current.as_mut() else {
            continue;
        };
        if let Some(rest) = raw.strip_prefix("--- ") {
            in_hunk = false;
            old_path = diff_path(rest);
            file.old_path = old_path.clone();
            continue;
        }
        if let Some(rest) = raw.strip_prefix("+++ ") {
            in_hunk = false;
            if let Some(path) = diff_path(rest) {
                file.path = path.clone();
            } else if let Some(path) = &old_path {
                file.path = path.clone();
            }
            continue;
        }
        if let Some(rest) = raw.strip_prefix("@@ ") {
            in_hunk = true;
            if let Some((old_start, new_start)) = hunk_starts(rest) {
                old_line = old_start;
                new_line = new_start;
                file.lines.push(DiffLine {
                    kind: DiffLineKind::Hunk,
                    text: raw.to_owned(),
                    old_line: Some(old_start),
                    new_line: Some(new_start),
                });
            }
            continue;
        }
        if !in_hunk {
            if let Some(path) = raw.strip_prefix("rename from ") {
                file.old_path = Some(path.to_owned());
            } else if let Some(path) = raw.strip_prefix("rename to ") {
                file.path = path.to_owned();
            } else if raw.starts_with("Binary files ") && raw.ends_with(" differ") {
                file.binary = true;
            }
            continue;
        }
        if raw.starts_with('\\') {
            file.lines.push(DiffLine {
                kind: DiffLineKind::Meta,
                text: raw.to_owned(),
                old_line: None,
                new_line: None,
            });
            continue;
        }
        let Some(prefix) = raw.chars().next() else {
            file.lines.push(DiffLine {
                kind: DiffLineKind::Context,
                text: String::new(),
                old_line: Some(old_line),
                new_line: Some(new_line),
            });
            old_line += 1;
            new_line += 1;
            continue;
        };
        let text = raw[prefix.len_utf8()..].to_owned();
        match prefix {
            '+' => {
                file.added += 1;
                file.lines.push(DiffLine {
                    kind: DiffLineKind::Added,
                    text,
                    old_line: None,
                    new_line: Some(new_line),
                });
                new_line += 1;
            }
            '-' => {
                file.removed += 1;
                file.lines.push(DiffLine {
                    kind: DiffLineKind::Removed,
                    text,
                    old_line: Some(old_line),
                    new_line: None,
                });
                old_line += 1;
            }
            ' ' => {
                file.lines.push(DiffLine {
                    kind: DiffLineKind::Context,
                    text,
                    old_line: Some(old_line),
                    new_line: Some(new_line),
                });
                old_line += 1;
                new_line += 1;
            }
            _ => {
                in_hunk = false;
            }
        }
    }
    finish(&mut files, &mut current);
    files
}

fn finish(files: &mut Vec<DiffFile>, current: &mut Option<DiffFile>) {
    if let Some(mut file) = current.take() {
        if file.path.is_empty()
            && let Some(old_path) = &file.old_path
        {
            file.path = old_path.clone();
        }
        if file.path == "/dev/null" {
            file.path = file.old_path.clone().unwrap_or_default();
        }
        files.push(file);
    }
}

fn diff_path(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw == "/dev/null" {
        return None;
    }
    let path = raw
        .strip_prefix("a/")
        .or_else(|| raw.strip_prefix("b/"))
        .unwrap_or(raw);
    if path.starts_with('"') && path.ends_with('"') && path.len() >= 2 {
        Some(path[1..path.len() - 1].to_owned())
    } else {
        Some(path.to_owned())
    }
}

fn hunk_starts(rest: &str) -> Option<(u32, u32)> {
    let (old, _new) = rest.split_once("@@")?;
    let mut fields = old.split_whitespace();
    let old_start = fields.next()?.strip_prefix('-')?;
    let new_start = fields.next()?.strip_prefix('+')?;
    let old_start = old_start.split(',').next()?.parse().ok()?;
    let new_start = new_start.split(',').next()?.parse().ok()?;
    Some((old_start, new_start))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,5 @@
 fn one() {}
-fn two() {}
+fn two() { changed(); }
 fn three() {}
 fn four() {}
+fn five() {}
";

    #[test]
    fn parses_modified_file_lines_and_numbers() {
        let files = parse(PATCH);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, "src/lib.rs");
        assert_eq!(file.old_path.as_deref(), Some("src/lib.rs"));
        assert_eq!(file.added, 2);
        assert_eq!(file.removed, 1);
        assert!(!file.binary);
        let lines: Vec<_> = file
            .lines
            .iter()
            .filter(|line| line.kind != DiffLineKind::Hunk)
            .collect();
        assert_eq!(lines[0].kind, DiffLineKind::Context);
        assert_eq!((lines[0].old_line, lines[0].new_line), (Some(1), Some(1)));
        assert_eq!(lines[1].kind, DiffLineKind::Removed);
        assert_eq!((lines[1].old_line, lines[1].new_line), (Some(2), None));
        assert_eq!(lines[2].kind, DiffLineKind::Added);
        assert_eq!((lines[2].old_line, lines[2].new_line), (None, Some(2)));
        assert_eq!(lines[4].kind, DiffLineKind::Context);
        assert_eq!((lines[4].old_line, lines[4].new_line), (Some(4), Some(4)));
        assert_eq!(lines[5].kind, DiffLineKind::Added);
        assert_eq!((lines[5].old_line, lines[5].new_line), (None, Some(5)));
    }

    #[test]
    fn tracks_new_and_deleted_file_paths() {
        let patch = "\
diff --git a/new.txt b/new.txt
new file mode 100644
--- /dev/null
+++ b/new.txt
@@ -0,0 +1 @@
+hello
diff --git a/old.txt b/old.txt
deleted file mode 100644
--- a/old.txt
+++ /dev/null
@@ -1 +0,0 @@
-bye
";
        let files = parse(patch);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "new.txt");
        assert_eq!(files[0].old_path, None);
        assert_eq!(files[0].added, 1);
        assert_eq!(files[1].path, "old.txt");
        assert_eq!(files[1].removed, 1);
    }

    #[test]
    fn marks_binary_files() {
        let patch = "\
diff --git a/logo.png b/logo.png
index 1111111..2222222 100644
Binary files a/logo.png and b/logo.png differ
";
        let files = parse(patch);
        assert_eq!(files.len(), 1);
        assert!(files[0].binary);
        assert_eq!(files[0].path, "logo.png");
        assert_eq!(files[0].lines.len(), 0);
    }
}
