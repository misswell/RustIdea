#[allow(dead_code)]
#[path = "../src/native_host.rs"]
mod native_host;

use ide_core::{DocId, IdeEvent, IpcPath};
use native_host::EventOverflow;
use std::path::PathBuf;

fn paths(values: &[&str]) -> Vec<IpcPath> {
    values
        .iter()
        .map(|value| IpcPath(PathBuf::from(value)))
        .collect()
}

#[test]
fn reliable_document_patch_loss_sets_a_recovery_flag() {
    let overflow = EventOverflow::new();
    overflow.record(IdeEvent::DocPatched {
        doc: DocId(3),
        base_version: 10,
        version: 11,
        changes: Vec::new(),
        workspace_edit: false,
    });
    let report = overflow.take_report();
    assert!(report.document_patches);
    assert_eq!(report.dropped, 1);
    assert!(overflow.take_report().dropped == 0, "report is drained");
}

#[test]
fn filesystem_overflow_is_batched_without_losing_paths() {
    let overflow = EventOverflow::new();
    overflow.record(IdeEvent::FsChanged {
        created: paths(&["/project/new"]),
        modified: paths(&["/project/file"]),
        removed: Vec::new(),
        ignore_rules: Vec::new(),
    });
    overflow.record(IdeEvent::FsChanged {
        created: Vec::new(),
        modified: paths(&["/project/file"]),
        removed: paths(&["/project/deleted"]),
        ignore_rules: paths(&["/project"]),
    });
    let report = overflow.take_report();
    let Some(IdeEvent::FsChanged {
        created,
        modified,
        removed,
        ignore_rules,
    }) = report.file_changes
    else {
        panic!("expected a merged FsChanged event");
    };
    assert_eq!(created.len(), 1);
    assert_eq!(modified.len(), 1);
    assert_eq!(removed.len(), 1);
    assert_eq!(ignore_rules.len(), 1);
}

#[test]
fn diagnostics_for_one_file_keep_only_the_latest_full_set() {
    let overflow = EventOverflow::new();
    overflow.record(IdeEvent::Diagnostics {
        path: IpcPath(PathBuf::from("/project/main.rs")),
        source: "rust-analyzer".into(),
        items: Vec::new(),
    });
    overflow.record(IdeEvent::Diagnostics {
        path: IpcPath(PathBuf::from("/project/main.rs")),
        source: "rust-analyzer".into(),
        items: Vec::new(),
    });
    let report = overflow.take_report();
    assert_eq!(report.coalesced.len(), 1);
    assert!(matches!(
        &report.coalesced[0],
        IdeEvent::Diagnostics { source, .. } if source == "rust-analyzer"
    ));
}
