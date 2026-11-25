# RustIdea 0.2.0 baseline report

## Frozen code and toolchain

| Item | Value |
|---|---|
| RustIdea baseline commit | `4e4ba7a722e2719261d9fc8dd331dc49652a7fb9` (`main`) |
| RustIdea release baseline | `v0.1.2` tag, commit `47d95a0d0d04e410020ed628f7e96dfbccbc3cfa` |
| OpenIDEA upstream | `048b87e4aea38924e2adf5f5f8ca4806640cb401` (2026-11-17) |
| Rust toolchain | `rustc 1.98.1`, `cargo 1.98.1` |
| GPUI | crates.io `gpui 0.2.2` (pinned with `=0.2.2`) |
| Plugin runtime target | `wasm32-wasip2` |
| macOS | 27.2.1, build 26B131, Apple M5 arm64 |
| Linux CI | Ubuntu 24.04, GTK/GLib native dependencies |
| Windows CI | Windows Server 2025 |

This report records the state before the Phase 1–7 stability changes in the
current working tree. Product source and installed release binaries are not
interchangeable evidence.

## Existing feature mapping

| OpenIDEA capability | Authoritative implementation | RustIdea adapter/UI | Status |
|---|---|---|---|
| Host/runtime | `ide-host` `Host` | `NativeHost`, Tokio runtime task adapter | Reused, integrated |
| Documents | `ide-workspace::Documents` | `DocumentBuffer` + `DocumentStore` | Mirrored, sync protocol under repair |
| VFS/project listing | `ide-vfs`, `ide-host::ops::fs` | async `ProjectTree`, GPUI `uniform_list` | Reused, partly complete |
| Language/syntax | `ide-language`, `ide-syntax` | background highlight request, diagnostics underlines | Partial |
| LSP | `ide-lsp`, `ide-host::ops::lsp` | diagnostics, definition, format | Partial |
| VCS | `ide-vcs`, `ide-host::ops::vcs` | native status/stage/commit/log/diff panels | Partial |
| Terminal | `ide-terminal`, `ide-host::ops::term` | PTY text stream panel | Partial, no VT screen model |
| DAP | `ide-dap`, `ide-jdwp` | attachable-process listing | Partial, no live session UI |
| Search | `ide-search` | native project search/replace | Partly complete |
| Database | `ide-db`, `ide-host::ops::db` | native SQLite/query panel | Partial |
| Plugin/WASM | `ide-plugin` | placeholder panel | Not integrated |
| Agent/MCP | `ide-agent`, `ide-mcp` | placeholder panel | Not integrated |
| Remote/SSH | `ide-ssh`, `ide-remote` | placeholder panel | Not integrated |
| Settings/user data | `ide-workspace`, `ide-core` | native settings browser and local session | Partial |

## Performance baseline status

The engine benchmark and 1M-line document stress test are recorded in
`RUSTIDEA_PERFORMANCE_REPORT.md`. Full process-tree comparison with OpenIDEA,
cold/warm startup, 100-file memory, scroll FPS, input P50/P95/P99 and
long-running memory are **not measured**. No comparative performance claim is
made.

## Environment constraints

- The current macOS console was screen-locked during interactive acceptance.
- AgentSpace `AgentUse` still has no logged-in GUI session.
- Therefore window, real IME, clipboard, scroll FPS and live memory evidence
remain open even when compile/tests are green.
