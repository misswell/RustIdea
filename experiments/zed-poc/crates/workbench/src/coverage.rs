use std::path::{Path, PathBuf};

use ide_coverage::{CoverageSuite, FileCoverage};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageRow {
    pub path: PathBuf,
    pub rel_path: String,
    pub lines_covered: u32,
    pub lines_total: u32,
    pub branches_covered: u32,
    pub branches_total: u32,
}

pub fn coverage_rows(root: &Path, suite: &CoverageSuite) -> Vec<CoverageRow> {
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let prefix = format!("{}/", canonical.to_string_lossy().trim_end_matches('/'));
    suite
        .files
        .iter()
        .map(|file| {
            let display = file.path.0.to_string_lossy();
            let rel_path = display.strip_prefix(&prefix).unwrap_or(&display).to_owned();
            CoverageRow {
                path: file.path.0.clone(),
                rel_path,
                lines_covered: file.lines_covered,
                lines_total: file.lines_total,
                branches_covered: file.branches_covered,
                branches_total: file.branches_total,
            }
        })
        .collect()
}

pub fn percent(covered: u32, total: u32) -> Option<f64> {
    (total > 0).then(|| covered as f64 / total as f64 * 100.0)
}

pub fn format_percent(covered: u32, total: u32) -> String {
    let Some(percent) = percent(covered, total) else {
        return "n/a".into();
    };
    let shown = if percent > 0.0 && percent < 1.0 {
        "<1".to_owned()
    } else {
        format!("{}", percent.floor() as u32)
    };
    format!("{shown}% ({covered}/{total})")
}

pub fn sort_rows(rows: &mut [CoverageRow], by_lines: bool) {
    if by_lines {
        rows.sort_by(|a, b| {
            let left = percent(a.lines_covered, a.lines_total);
            let right = percent(b.lines_covered, b.lines_total);
            match (left, right) {
                (Some(left), Some(right)) => left
                    .partial_cmp(&right)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.rel_path.cmp(&b.rel_path)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.rel_path.cmp(&b.rel_path),
            }
        });
    } else {
        rows.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    }
}

pub fn totals(files: &[FileCoverage]) -> (u32, u32, u32, u32) {
    (
        files.iter().map(|file| file.lines_covered).sum(),
        files.iter().map(|file| file.lines_total).sum(),
        files.iter().map(|file| file.branches_covered).sum(),
        files.iter().map(|file| file.branches_total).sum(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ide_core::IpcPath;

    fn file(path: &str, covered: u32, total: u32) -> FileCoverage {
        FileCoverage {
            path: IpcPath(path.into()),
            lines: Vec::new(),
            lines_total: total,
            lines_covered: covered,
            branches_total: 0,
            branches_covered: 0,
        }
    }

    #[test]
    fn formats_percentages_like_openidea() {
        assert_eq!(format_percent(0, 0), "n/a");
        assert_eq!(format_percent(1, 200), "<1% (1/200)");
        assert_eq!(format_percent(17, 20), "85% (17/20)");
    }

    #[test]
    fn makes_project_relative_rows_and_sorts_by_coverage() {
        let suite = CoverageSuite {
            runner: "lcov".into(),
            has_branches: false,
            files: vec![
                file("/repo/src/A.java", 10, 10),
                file("/repo/src/B.java", 2, 5),
            ],
        };
        let mut rows = coverage_rows(Path::new("/repo"), &suite);
        sort_rows(&mut rows, true);
        assert_eq!(rows[0].rel_path, "src/B.java");
        assert_eq!(
            format_percent(rows[0].lines_covered, rows[0].lines_total),
            "40% (2/5)"
        );
        assert_eq!(rows[1].rel_path, "src/A.java");
        assert_eq!(totals(&suite.files), (12, 15, 0, 0));
    }
}
