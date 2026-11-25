# RustIdea

RustIdea is a native Rust IDE built on [GPUI](https://gpui.rs). It reuses the
Apache-2.0 Rust backend from [OpenIDEA](https://github.com/l0ng-ai/openidea)
while replacing the Tauri/React/CodeMirror workbench with a native Rust UI.

The shipped application does not link a WebView, a browser runtime, React,
CodeMirror, or Tauri.

## Status

This repository is following the migration plan in the root `RUSTIDEA_*.md` files:

- Phase 0: migration audit, feature inventory, architecture and license
  inventory are documented; comparative performance measurements remain open.
- Phase 1: standalone GPUI application shell, native menus, themes, workbench
  panels, session persistence, logging and reused Rust services. OpenIDEA
  operations run on the host Tokio runtime; UI events use a bounded coalescing
  queue.
- Phase 2: native editor with UTF-16 document buffers, undo/redo, selection,
  clipboard, line numbers, virtualized scrolling, syntax highlighting,
  diagnostics, save/autosave and IME marked-text handling. Document sync
  coalesces local edits, rebases disjoint backend patches, preserves local
  text on conflict, and verifies disk content with BLAKE3 before saves.

This is not yet complete OpenIDEA parity. Native project search, in-file
find-next/find-previous, VCS, terminal, run/build, database and HTTP panels are
implemented at MVP depth. LSP completion, hover documentation, rename, code
actions, go-to-definition and formatting are wired to the reused OpenIDEA
language-server service. The Remote Host panel opens real SSH shells through the
reused OpenIDEA SSH client. Multi-cursor/folding, full terminal emulation, real
debugger sessions, plugins/MCP, agents, remote filesystem mounts, notebooks,
viewers and several language tool windows remain incomplete or placeholders.
See `RUSTIDEA_FEATURE_PARITY.md` for the item-by-item status.

The old OpenIDEA tree is a read-only migration reference, not a runtime
dependency. Backend crates are vendored here as they are adopted.

## Build

```sh
cargo run -p rustidea
```

Create a signed and notarized macOS universal app:

```sh
scripts/release-macos.sh
```

## Test

```sh
cargo test --workspace
```

The targeted release-gate test set is:

```sh
cargo test -p rustidea -p rustidea-editor -p ide-text -p ide-workspace -p ide-host
cargo clippy -p rustidea -p rustidea-editor -p ide-workspace -p ide-host \
  --all-targets -- -D warnings
```

Licensing details are in `THIRD_PARTY_LICENSES.md`,
`THIRD_PARTY_RUST_LICENSES.md`, `THIRD_PARTY_SOURCES/option-ext-0.2.0/`, and
`RUSTIDEA_LICENSE_AUDIT.md`.

Verification evidence and remaining acceptance work are tracked in
`RUSTIDEA_REGRESSION_REPORT.md` and `RUSTIDEA_PERFORMANCE_REPORT.md`.
