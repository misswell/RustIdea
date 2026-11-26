# RustIdea regression report

## Scope

Version: `0.2.0` (release commit `0678891`).  
Host: macOS 27.2.1, Apple M5, arm64.  
Rust: 1.98.1, edition 2024.  
Architecture: GPUI native application with reused Apache-2.0 `ide-*` backend
crates.

## Automated checks completed in this run

| Check | Result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo check -p rustidea` | Pass |
| `cargo test -p rustidea -p rustidea-editor -p ide-text -p ide-workspace -p ide-host --lib` | Pass, 685 tests |
| `cargo test -p rustidea` (binary tests) | Pass, 17 tests |
| `cargo clippy -p rustidea -p rustidea-editor -p ide-workspace -p ide-host --all-targets -- -D warnings` | Pass |
| `cargo fmt --all --check` | Pass |
| `cargo about ... --target aarch64-apple-darwin --locked --offline --fail` | Pass, 919 crate records, 12 SPDX identifiers |
| Dependency search for Tauri/Wry/WebView/WebKit/React/CodeMirror/Monaco/xterm | No runtime matches |

The 685 library tests include 491 `ide-host` tests, 165 `ide-workspace` tests,
13 `ide-text` tests and 16 versioned editor/document tests. The 17 RustIdea
binary tests cover canonical document identity/rekey/ref-counting, cached tree
expansion, incremental watcher invalidation, external disk stamps, UTF-16/IME
run conversion and terminal ANSI handling.

The 1M-line engine stress test inserts at line 500,000, verifies the new line,
undoes it and verifies the original line count. It completes as part of the
editor suite. The optimized Criterion baseline is recorded in
`RUSTIDEA_PERFORMANCE_REPORT.md`.

### Architecture correction evidence

| Requirement | Evidence |
|---|---|
| No temporary/duplicate backend | `backend.rs` and `project.rs` are absent; project/document/VCS/search/terminal/run/build/db/LSP operations call `ide_host::ops::*` through `NativeHost`. |
| GPUI futures run on the backend runtime | `NativeHost::run` moves operations to the host Tokio runtime. This fixed two launch-time panics: missing reactor in `vcs_branch_info` and out-of-order `EnterGuard` drops. The app process now stays alive in a PTY session. |
| Async lazy project tree | `ProjectTree` caches per-directory `ide_vfs` listings, loads only root/missing directories, rebuilds cached expansions without I/O and uses `uniform_list`. |
| Incremental file watcher updates | `FsChanged` invalidates only affected loaded parents; deleted directories are not re-read. Unit tests cover cached expansion, incremental invalidation and removal. |
| No workbench repaint per keystroke | The app no longer has `cx.observe(&editor)` subscriptions. `BufferEvent::Changed` returns before `cx.notify`; dirty/title/save/error are the only workbench notifications. |
| DocumentStore | Canonical path/symlink identity, one handle per document, view ref-counting, Save As rekey and last-view release are tested. |
| External change protection | Disk-stamp detection plus dirty-conflict banner and save guard; backend `reload_discarding_changes` has an automated dirty-buffer reload test. |
| Bounded editor work | Visible-line virtualization, bounded shaped-line cache, version/epoch invalidation, 1,000-entry history, delta undo/redo, coalesced autosave and large-file syntax cutoff. |
| No WebView runtime | Static dependency search and `Cargo.lock` contain no Tauri/Wry/WebKit/WebView/CodeMirror/Monaco/xterm runtime. |

The dangerous upstream `trust_vcs_sweep` fixture was changed to use a
non-recursive marker script for `remote.origin.uploadpack`; the prior fixture
could recursively spawn `git-upload-pack` when git itself honored the config.
This is a test-harness safety fix, not a product behavior change.

## Release build, signature and notarization

Status: **passed** for commit `0678891`.

- Universal executable: `x86_64 arm64`.
- Signature: Developer ID Application `Guofeng Liu (U8U443D7ZL)`, hardened
  runtime, secure timestamp `2026-11-24 18:29:55`.
- Notary submission `0ee3c7d4-1dfb-417d-a8cd-3d2e1b5681cf`: `Accepted`.
- Stapler validation and `spctl --assess --type execute`: accepted
  (`source=Notarized Developer ID`).
- `otool -L` on the universal executable: no WebKit, WebView, JavaScriptCore,
  Tauri or Wry framework.
- Universal zip SHA-256:
  `02c1da12a7ff53d290988afed16c6d7b6a47ba834d2aefa9336f68471a4d97e5`.

The three-platform GitHub Actions matrix was stopped at the release owner's
request before completion; it is not claimed as green for this release. Local
formatting, targeted Clippy, release build, signature, notarization and
Gatekeeper checks passed.

## Interactive acceptance still open

The macOS console is screen-locked (`CGSSessionScreenIsLocked=Yes`). AgentSpace's
AgentUse account still has no logged-in GUI session. The native process starts,
opens the OpenIDEA project path through the backend and remains alive, but no
usable window or screenshot is available while the console is locked. These
task-book checks are not claimed as passing:

- Open, edit, save, quit and reopen a real project file.
- Window lifecycle, menu actions, light/dark theme, project tree and context
  menu interactions.
- Clipboard, undo/redo, autosave and actual Chinese IME composition.
- 100,000- and 1,000,000-line editing and scrolling in the GPUI window (engine
  stress tests pass, but UI input/scroll timings are not measured).
- Four splits of one document and simultaneous edits.
- Long-running stability, recovery and data compatibility.
- GUI process-tree inspection for hidden WebView/browser processes.
- CPU/GPU and memory measurements.

The engine-level 1M-line and document-store/tree/watcher automated checks are
green. They are not a substitute for the locked-session GUI acceptance items.

The absence of WebView is verified statically from the dependency graph and
will be checked again against the release executable with `otool -L`.

## Follow-up gate

Log in to the isolated AgentSpace user, then repeat the interactive checks and
compare the same fixtures with OpenIDEA. Record at least five samples per
performance scenario in `RUSTIDEA_PERFORMANCE_REPORT.md`. Do not declare Phase 2
interactive acceptance, full OpenIDEA parity or performance targets passed
until that evidence exists.

## 0.2.0 stability pass (working tree after 4e4ba7a)

### Automated evidence

| Check | Result |
|---|---|
| `cargo test -p rustidea` | Pass: 34 binary tests + 68 integration tests |
| document sync regression | Pass: 10,000 sequential inputs, disjoint rebase, explicit conflict |
| multiview regression | Pass: shared handle, tab/split view counts, Save As rekey, last-view release |
| event overflow regression | Pass: bounded queue coalescing and document/fs recovery flags |
| project tree regression | Pass: 100,000-file paging, generation dedup, stale result rejection |
| large-file regression | Pass: 1M-line edit/undo, 2M-character single line |
| filesystem regression | Pass: `.gitignore`/`.ignore`, symlink reporting |
| terminal regression | Pass: ANSI stripping, CR/backspace/tab, 5,000-line cap |
| editor input regression | Pass: 10,000 UTF-16 inserts and CRLF normalization |
| editor undo regression | Pass: external edit rebases history, multibyte undo/redo |
| LSP/find helpers | Pass: UTF-16 identifier replacement ranges (including CJK and astral text) and in-file find wrapping across multibyte text |
| remote host panel | Static/state path only: configuration loading and connect call the reused `SshManager` and `spawn_ssh`; no SSH server was contacted in this run |

### LSP and in-file find (0.2.0 pass)

The native workbench now requests the reused OpenIDEA language-server service
on the host Tokio runtime for completion, hover, rename and code actions. The
ready results are installed into GPUI overlays only after the request finishes.
Completion supports label/detail filtering, UTF-16 fallback word ranges,
snippet-default insertion and additional edits. Rename and code actions apply
their workspace edits through `ide_host::ops::lsp::apply_workspace_edit`; code
actions that only carry a client command are reported as unavailable rather
than silently ignored. In-file find-next/find-previous selects a match and
wraps around the document.

These are implementation and helper-level checks. A real language server, the
overlay key handling and IME interaction still need an unlocked interactive
session before they can be marked accepted.

### Important caveats

- GUI interaction remains blocked by the locked macOS console; tests above are
  engine/state-level evidence, not IME/window/clipboard acceptance.
- Completion, hover, rename and code-action overlays have not been exercised
  against a live language server or an unlocked window.
- The three-platform CI run for the final commit is the authoritative CI gate.
  Earlier runs on `47d95a0` and `4e4ba7a` exposed missing CI native libraries,
  missing plugin fixtures and missing `src/bindings.ts`; fixes are in the
  working tree. Do not claim green until the final run succeeds.
- Queue capacity, scroll FPS, input latency and long-running memory are not
  measured with a GUI session.
