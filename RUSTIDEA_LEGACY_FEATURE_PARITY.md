# Legacy feature inventory (pre-Zed workbench)

This is the Phase 0–2 inventory of the old `crates/rustidea` workbench. It is
kept for regression and migration reference. The OpenIDEA-to-Zed-native
inventory now lives in `RUSTIDEA_FEATURE_PARITY.md`.

Status legend:

- **MVP native**: a working GPUI UI and a reused backend path exist.
- **Partial**: the core path works, but important upstream interactions or
  views are absent.
- **Service only**: backend code exists; the feature has no native UI.
- **Placeholder**: only a GPUI panel or action stub exists.
- **Missing**: no native UI or adapter is present.

This table is an honest Phase 0–2 inventory. It is not a claim of complete
OpenIDEA parity. Phase 0–2 do not require every Phase 3–7 feature, and the 0.2.0
release must not be described as a complete IDE replacement.

The separate `experiment/zed-editor` V2 effort is tracked in
`RUSTIDEA_ZED_POC_REPORT.md`. It now has a first native workbench shell
(project tree, tabs, Zed Editor, multi-document OpenIDEA sync/save and
language intelligence), but it has not replaced this complete workbench or
completed the OpenIDEA UI parity migration.

## Shell and workbench

| Upstream feature area | RustIdea status | Evidence and remaining work |
|---|---|---|
| app, workbench, activity rail, panels | **MVP native** | GPUI window, menus, activity rail, left/right/bottom panels, tabs, split groups, dialogs, command palette, quick open, status bar. Multi-window and complete upstream menu parity are missing. |
| welcome, onboarding, whats-new | **Missing** | No native welcome screen, recent-project landing page or release-notes UI. |
| project trust | **Partial** | Trust state is shown and can be granted through `ops::project`; full upstream trust rationale/dialog flow and unsafe-config sweep UI are missing. |
| settings | **Partial** | Native settings browser, filter and boolean toggles use the shared backend settings API. Non-boolean values are display-only; complete typed editors, scopes and migration UX are missing. |
| themes | **MVP native** | Native dark/light token sets, toggle action and persistence. Fine-grained upstream theme catalogue is missing. |
| command palette, quick open, search everywhere | **Partial** | GPUI overlay with file, command, line and symbol modes; result ranking and full upstream command catalogue are incomplete. |
| session and recovery | **Partial** | Project, open files, theme, panel state and autosave preference are persisted. Crash recovery, backup buffers and full data migration are missing. |

## Editor and language intelligence

| Upstream feature area | RustIdea status | Evidence and remaining work |
|---|---|---|
| editor | **MVP native** | Real GPUI text editor over `ide-text`/Ropey; UTF-16 positions, virtualized visible lines, gutter line numbers, selections, caret, horizontal/vertical scrolling, clipboard, undo/redo, diagnostics underlines, Tree-sitter highlighting, save, save-as and autosave. |
| macOS IME | **Partial** | `ElementInputHandler` marked text range and UTF-16 replacement are implemented; real Chinese IME composition has not been interactively verified. |
| editor core rendering | **Partial** | Monospace text, line height, current-line/caret/selection/spans, CJK/astral text handling, horizontal scrolling and a bounded shaped-line layout cache exist. Syntax highlighting now parses in the background and commits only for the latest version. Soft wrap, RTL verification, font zoom, whitespace rendering, indent guides, relative numbers and minimap are missing. |
| editor advanced editing | **Missing** | Multi-cursor, rectangular/column selection, bracket completion/matching, automatic and smart indent, code folding, sticky scroll, code lens, inline hints, overwrite mode and drag-to-move text. |
| editor search | **Partial** | Project-wide search/replace and in-editor find-next/find-previous with wrapping are native. In-editor replace, regex options and all-match decorations are missing. |
| editor LSP, code insight | **Partial** | Diagnostics, completion (filter, snippet insertion and additional edits), hover documentation, rename through a workspace edit, code actions with edit application, go-to-definition and format-document are wired. References, signature help, inlay hints, semantic tokens, folding ranges and full navigation history are missing. Command-only code actions are reported as unavailable. |
| navigation, structure | **Partial** | Native line navigation and a line-oriented structure panel exist. The symbol index/palette is basic; breadcrumbs, inheritance navigation and refactoring are missing. |
| refactor, code actions | **Service only** | Reused crates compile; no native refactor command surface. |
| multi-view synchronization | **MVP native** | `DocumentStore` uses lexical UI identity plus backend canonical identity, keeps one `DocumentBuffer` per file and reference-counts tabs/splits; Save As rekeys the same buffer. Pending edits coalesce into one backend diff, and disjoint backend edits are rebased. Interactive split editing remains unverified. |
| external changes and conflict handling | **MVP native** | DiskVersion uses streamed BLAKE3 plus metadata; save-boundary verification blocks overwrite. Local/backend overlap uses a three-way merge or explicit Keep/Reload conflict; save waits for sync. Interactive watcher timing remains unverified. |

## Project and navigation

| Upstream feature area | RustIdea status | Evidence and remaining work |
|---|---|---|
| project tree | **MVP native** | Reused `ide-vfs` listing with `.gitignore`/`.ignore`, async root and on-demand reads, per-directory generation deduplication, 2,000-row paging, incremental watcher invalidation and GPUI `uniform_list` virtualization. Open files/folders, context menu, create file/folder, rename, trash, reveal and copy path work. Drag/drop, file icons and large-project interactive timing are unverified. |
| search | **MVP native** | Project search, case/regex/whole-word toggles, debounce/cancellation, file/line results, replace-in-files and jump-to-line. Search history, saved filters and complete encoding behavior are missing. |
| scratch, TODO | **Missing** | No scratch files or TODO tool window. |
| dependencies, libraries, dotnet, rails, cucumber, thymeleaf | **Service only** | Some language/project services are vendored; their tool windows and project-specific interactions are absent. |
| viewers, web preview | **Missing** | No native image/PDF/web viewer replacement. Adding a WebView is explicitly forbidden. |

## Source control and collaboration

| Upstream feature area | RustIdea status | Evidence and remaining work |
|---|---|---|
| VCS status, staging, commit, log, diff | **MVP native** | Native changes/log/branch panels, stage/unstage/discard/commit and textual diff preview via `ops::vcs`. Inline editable diff, blame, shelves UI, merge/rebase workflows and full hunk application are missing. |
| GitHub | **Service only** | `ide-github` is vendored; no native GitHub UI or operations. |
| remote development, SSH, deployment | **Partial** | Remote Host panel lists the reused OpenIDEA SSH configurations, shows live connected state and opens a real SSH shell through `ide_ssh`/`ops::term::spawn_ssh`; disconnect is wired. Remote filesystem mounts, port forwarding, deployment targets and configuration editing are missing. |

## Execution, debugging and tools

| Upstream feature area | RustIdea status | Evidence and remaining work |
|---|---|---|
| terminal | **Partial** | Native shell tabs, command input, PTY output stream, bounded ANSI-consumed text display, carriage returns, backspace, tabs and output cap. It is not a full VT screen emulator: alternate screen, cursor addressing, colors and mouse/paste/resize are missing. |
| run, run-anything | **Partial** | Native run configs, start/stop, streamed output. Debug-launch integration, environment editor, terminal task merging and run-anything catalogue are missing. |
| build, build-tools | **Partial** | Build model summary and build output surface exist. Native build-tool configuration and full Gradle/Maven/sbt actions are missing. |
| debugger | **Partial** | Attachable-process refresh and debug-config listing exist. There is no native DAP session UI, breakpoint editing, stack/variable/watch views or actual launch/attach control. |
| coverage, profiler, heap, logcat | **Service only** | Backend crates exist; native tool windows are absent. |
| database | **Partial** | SQLite source creation, source list, connect, schemas/tables and query execution/result rendering use `ops::db`. Typed connection editors, other drivers, saved consoles and result grids are incomplete. |
| HTTP client | **Partial** | Single native request editor (method URL, headers, body), execution and response viewer. Collections, environments, history, auth helpers and multi-request tabs are missing. |
| notebook | **Service only** | Backend crate exists; no native notebook UI. |
| analysis, color preview, perf, templates, macros, vim, python console | **Service only / Missing** | No complete native UI. Vim mode and macro editing are not implemented. |
| GraphQL, web tools | **Missing** | No native GraphQL query surface or web utility replacement. |

## Extensions and agents

| Upstream feature area | RustIdea status | Evidence and remaining work |
|---|---|---|
| plugins, MCP | **Placeholder** | Plugins sidebar explains the future surface only. No plugin host, install/update, permissions or MCP management UI. Backend crates compile. |
| agents | **Placeholder** | Agents rail and right-side explanatory panel exist. No provider setup, session creation, message streaming, approval or attachment workflow. |
| tools, services | **Placeholder** | Native stubs only; no complete tool catalogue or service process controls. |
| user data compatibility | **Partial / unverified** | Settings use shared backend APIs and a local RustIdea session; direct migration of OpenIDEA recent projects, keymaps, plugin data and project config has not been implemented or tested. |

## Required work before claiming full parity

1. Finish interactive Phase 2 evidence: window/clipboard/undo/IME, large-file timing and sustained-memory checks.
2. Finish Phase 3 editor capabilities.
3. Finish the remaining LSP surfaces: references, signature help, inlay hints,
   semantic tokens, folding ranges, command-backed code actions and complete
   diagnostics navigation.
4. Build true multi-window and crash-recovery tests; conflict tests exist.
5. Replace terminal ANSI text handling with a complete native terminal screen
   model or a non-WebView equivalent.
6. Implement actual DAP sessions and debug panels.
7. Implement plugin, MCP and agent surfaces, then finish remote filesystem
   mounts, port forwarding and deployment on top of the working SSH shell.
8. Port remaining language/tool windows without WebView fallback.
9. Complete OpenIDEA data migration and item-by-item parity tests.
10. Collect the OpenIDEA/RustIdea UI performance baseline and interactive
   acceptance evidence.
