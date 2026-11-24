# Phase 0 migration audit

## Audited upstream snapshot

The reference snapshot is `l0ng-ai/openidea` commit
`50cd478773e3641bc1b636dea2fe29c6868b07d7`, version 0.1.50. Its Git tree has
47 backend crate directories, 49 frontend feature directories, 1,098 Rust
files and 3,552 TS/TSX/CSS files. Counts describe that snapshot, not upstream
head. A second local checkout was also inspected for contribution wiring, but
the snapshot above remains the inventory baseline.

Upstream targets macOS, Windows and Linux. Its desktop entry point is Tauri;
the frontend is React and CodeMirror. Desktop commands combine project-host
adapters from `ide-host` with Tauri-specific commands. Replacing the UI means
replacing both the IPC transport and frontend business logic; calling the
backend crates alone does not reproduce a feature.

## Reuse boundary in RustIdea 0.1.2

The workspace now contains 46 package manifests and 909 Rust files (about
500,764 lines), including the 42 reused `ide-*` backend crates, their language,
WIT and plugin assets, and the native GPUI application. The reused source is
Apache-2.0 and remains in `crates/`. RustIdea owns the workbench and editor;
OpenIDEA remains a read-only reference and is not launched by RustIdea.

The native service boundary is `crates/rustidea/src/native_host.rs`. It owns
one `ide_host::Host`, a Tokio runtime, the window id and an event receiver.
The GPUI application calls `ide_host::ops::*` and other reused service APIs,
but never Tauri IPC. Calls that can block or touch the filesystem, language
servers, terminals, VCS, databases or debuggers run on the service runtime and
return through GPUI tasks or typed events.

## Current backend reuse paths

| UI capability | Reused service |
|---|---|
| Project open, trust, document open/save, settings | `ide-host::ops::{project,doc,fs,settings}` |
| Project tree and project files | `ops::fs::read_dir`, create, rename, delete, reveal |
| Multi-file search and replace | `ops::search` and `ide-search` |
| Diagnostics, go to definition, formatting | `ops::lsp` plus host diagnostic events |
| Syntax highlighting | `ide-language` syntax registry and `ide-host` language store |
| Version control | `ops::vcs` and `ide-vcs` |
| Terminal and shell I/O | `ops::term` and `ide-terminal` |
| Run configurations and process output | `ops::run` and `ide-run` |
| Build model | `ops::build` and `ide-build` |
| HTTP requests | `ide-http` through the host runtime |
| SQL data sources, schemas and queries | `ops::db` and `ide-db` |
| Debug attachable discovery | `ops::debug` and `ide-dap` |

Unused backend capabilities are not counted as migrated merely because their
crate compiles. The exact UI status is recorded in
`RUSTIDEA_FEATURE_PARITY.md`.

## Document and editor findings

Upstream `document.ts` owns one model per backend document. Tabs and
attachments reference it; per-view selection, folds, history and scroll live
in slots. `DocSync.ts` tracks confirmed text, one in-flight edit and a local
buffer, rebases remote edits, retries stale versions and can hard-resync.
`ide-text` stores text in Ropey with UTF-16 coordinates and normalized line
endings; the original ending is restored on save.

RustIdea's native `DocumentBuffer` is shared by all tabs and split views of a
document. It owns undo/redo, versioning, dirty state, local line-ending
normalization and applies backend patches. The host document registry remains
the save/LSP/workspace-edit source and currently retains a synchronized text
mirror. This preserves correctness and host integration, but the strict
single-copy requirement in the task book remains an architecture follow-up.

Upstream editor contributions include folds, selection matches, indentation,
sticky scroll, LSP markers, run gutters, Markdown, macros, templates, Docker,
coverage, databases, TODO, debugger, VCS and plugin extensions. Heavy
extensions are disabled above 8,000,000 characters. Those frontend behaviours
still require item-by-item native replacement; the backend crate compiling is
not evidence that a UI feature exists.

## Tauri and web-technology coupling

The RustIdea application graph resolves 853 normal dependency package records.
It contains no `tauri`, `wry`, WebKit/WebView, Chromium, Electron, React,
CodeMirror, Monaco or xterm dependency. `gpui` is pinned to `=0.2.2` and the
workbench, terminal surface, dialogs and editor are GPUI/Rust code. Web
technology remains only in the upstream reference and non-runtime assets.

## Evidence gaps

- The upstream snapshot's full build and tests were not used as proof for
  RustIdea. Its observed CI runs were cancelled at the time of this audit.
- OpenIDEA and RustIdea comparative performance has not been measured; see
  `RUSTIDEA_PERFORMANCE_BASELINE.md`.
- Interactive macOS acceptance (window lifecycle, themes, clipboard, real
  Chinese IME, large files, multi-split and process-tree inspection) remains
  open because no unlocked isolated GUI session was available.
- Full OpenIDEA interface and function parity has not been reached. Missing
  groups are enumerated in `RUSTIDEA_FEATURE_PARITY.md`.
