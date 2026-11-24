# RustIdea regression report

## Scope and build

Verified source revision: `07d1e293fa8d9a83cd03920e7ea1729176ecc980`.
Host: macOS 27.2.1, Apple M5, arm64. App build: Rust 1.98.1,
`aarch64-apple-darwin`, version 0.1.1.

The automated checks completed successfully:

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` | Pass |
| `cargo test --workspace` | Pass, 156 tests |
| `cargo fmt --all --check` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `cargo build --release -p rustidea` | Pass |
| `cargo build --release -p rustidea --target x86_64-apple-darwin` | Pass |
| `cargo about ... --target aarch64-apple-darwin --locked --offline --fail` | Pass |
| Dependency and executable linkage search for Tauri/WebKit/React/CodeMirror | No matches |

The arm64 release app was signed with Developer ID, accepted by Apple Notary
Service, stapled, and accepted by Gatekeeper. An arm64/x86_64 universal app was
also built and signed; its notarization submission is still in progress. These
checks do not replace interactive app testing.

## Interactive acceptance still open

AgentSpace `AgentUse` has no logged-in GUI session, so the app was not launched
in an isolated desktop. These task-book checks have not been performed and are
not claimed as passing:

- Open and edit a real project file; save, quit, and reopen it.
- Light/dark theme changes, tab/window lifecycle and project-tree interaction.
- Clipboard, undo/redo and real Chinese IME composition.
- 100,000- and 1,000,000-line files; responsive scrolling and editing.
- Four splits of one document and multiple open documents.
- Long-running stability, crash recovery and data-compatibility behavior.
- GUI process-tree inspection for hidden WebView/browser processes.
- Interactive CPU/GPU checks and memory-growth checks.
- Linux and Windows builds.

Cross-target `cargo check --all-targets` attempts were made for Linux x86_64 and
Windows GNU. Both stopped in the `ring` build script because the host lacks the
target C compilers (`x86_64-linux-gnu-gcc` and `x86_64-w64-mingw32-gcc`). These
are environment-blocked checks, not Rust source failures and not passing
cross-platform builds.

## Required follow-up

Log into the `AgentUse` account in AgentSpace, then repeat the interactive
checks above. Use the same host, fixture and steps for the OpenIDEA comparison;
record each performance scenario at least five times and report medians in
`RUSTIDEA_PERFORMANCE_REPORT.md`. Do not declare Phase 2 accepted or the
performance targets met until the missing evidence is recorded.
