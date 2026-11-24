# RustIdea regression report

## Scope and build

Verified application source revision: `bfadf41e47708ea3c48f887e5312584dd6e2e477`.
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
| GitHub Actions CI on macOS 15, Ubuntu 24.04 and Windows 2025 | Pass; build, 156 tests and Clippy |

CI run: <https://github.com/misswell/RustIdea/actions/runs/45312837627>.
macOS completed in 3m22s, Ubuntu in 20m56s and Windows in 3m30s.

The arm64/x86_64 universal release app was signed with Developer ID, accepted
by Apple Notary Service, stapled, and accepted by Gatekeeper. The accepted
notarization submission is `0dd04501-d29a-429e-8491-ceedc1111566`. These checks
do not replace interactive app testing.

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

Local cross-target checks for Linux x86_64 and Windows GNU were blocked because
the host lacks the target C compilers (`x86_64-linux-gnu-gcc` and
`x86_64-w64-mingw32-gcc`). Native GitHub Actions jobs independently built,
tested and linted the application on Ubuntu 24.04 and Windows 2025, so
cross-platform compile verification is complete. Interactive cross-platform
GUI acceptance remains open.

## Required follow-up

Log into the `AgentUse` account in AgentSpace, then repeat the interactive
checks above. Use the same host, fixture and steps for the OpenIDEA comparison;
record each performance scenario at least five times and report medians in
`RUSTIDEA_PERFORMANCE_REPORT.md`. Do not declare Phase 2 accepted or the
performance targets met until the missing evidence is recorded.
