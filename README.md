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
- Phase 1: standalone GPUI application shell, theme, menus, logging, services.
- Phase 2: native editor minimum viable version.

The old OpenIDEA tree is a read-only migration reference, not a runtime
dependency. Backend crates are vendored here as they are adopted.

## Build

```sh
cargo run -p rustidea
```

## Test

```sh
cargo test --workspace
```

Licensing details are in `THIRD_PARTY_LICENSES.md`,
`THIRD_PARTY_RUST_LICENSES.md`, `THIRD_PARTY_SOURCES/option-ext-0.2.0/`, and
`RUSTIDEA_LICENSE_AUDIT.md`.
