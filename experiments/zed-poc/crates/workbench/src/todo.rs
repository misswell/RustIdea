use ide_search::{FileMatches, SearchContext, SearchQuery, SearchScope};

pub const TODO_PATTERN: &str = r"\b(TODO|FIXME|HACK|XXX)\b.*";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoItem {
    pub path: std::path::PathBuf,
    pub rel_path: String,
    pub line: u32,
    pub column: u32,
    pub keyword: String,
    pub text: String,
}

pub fn build_todo_query(case_sensitive: bool, comments_only: bool) -> SearchQuery {
    SearchQuery {
        pattern: TODO_PATTERN.into(),
        is_regex: true,
        case_sensitive,
        whole_word: false,
        include: Vec::new(),
        exclude: Vec::new(),
        scope: SearchScope::Project,
        max_matches: 10_000,
        context: if comments_only {
            SearchContext::InComments
        } else {
            SearchContext::Any
        },
    }
}

pub fn todo_items_from_file(file: &FileMatches) -> Vec<TodoItem> {
    let mut items = Vec::new();
    for line in &file.matches {
        for &(start, end) in &line.ranges {
            let start = usize::try_from(start).unwrap_or(usize::MAX);
            let end = usize::try_from(end).unwrap_or(usize::MAX);
            if start >= line.text.len()
                || end > line.text.len()
                || !line.text.is_char_boundary(start)
                || !line.text.is_char_boundary(end)
            {
                continue;
            }
            let matched = &line.text[start..end];
            let keyword = matched
                .split_whitespace()
                .next()
                .unwrap_or(matched)
                .trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_uppercase();
            if !matches!(keyword.as_str(), "TODO" | "FIXME" | "HACK" | "XXX") {
                continue;
            }
            items.push(TodoItem {
                path: file.path.0.clone(),
                rel_path: file.rel_path.clone(),
                line: line.line,
                column: line.text_offset + u32::try_from(start).unwrap_or_default(),
                keyword,
                text: clean_comment_text(matched),
            });
        }
    }
    items
}

fn clean_comment_text(text: &str) -> String {
    text.trim()
        .trim_end_matches("*/")
        .trim_end_matches("-->")
        .trim_end_matches("-}")
        .trim_end_matches("*)")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ide_core::IpcPath;
    use ide_search::LineMatch;

    fn file_with(ranges: Vec<(u32, u32)>, text: &str) -> FileMatches {
        FileMatches {
            path: IpcPath("/tmp/App.java".into()),
            rel_path: "src/App.java".into(),
            matches: vec![LineMatch {
                line: 4,
                text: text.into(),
                ranges,
                text_offset: 0,
            }],
        }
    }

    #[test]
    fn query_uses_regex_and_comment_context() {
        let query = build_todo_query(true, true);
        assert!(query.is_regex);
        assert!(query.case_sensitive);
        assert_eq!(query.context, SearchContext::InComments);
        assert_eq!(query.pattern, TODO_PATTERN);
        assert_eq!(build_todo_query(false, false).context, SearchContext::Any);
    }

    #[test]
    fn converts_each_match_and_strips_comment_terminators() {
        let file = file_with(vec![(3, 23)], "// TODO: remove this */");
        let items = todo_items_from_file(&file);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].keyword, "TODO");
        assert_eq!(items[0].line, 4);
        assert_eq!(items[0].column, 3);
        assert_eq!(items[0].text, "TODO: remove this");
        assert_eq!(items[0].rel_path, "src/App.java");
    }

    #[test]
    fn ignores_non_todo_and_out_of_bounds_ranges() {
        let file = file_with(vec![(0, 8), (0, 999), (0, 3), (2, 10)], "NOTE: not a todo");
        assert!(todo_items_from_file(&file).is_empty());
    }
}
