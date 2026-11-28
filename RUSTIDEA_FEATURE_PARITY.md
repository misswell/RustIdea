# RustIdea feature parity inventory

Reference: OpenIDEA `cc5d9d95e4f97a033c97b4a4753047db3b3d1a73`.
Statuses: `NOT_STARTED`, `IN_PROGRESS`, `IMPLEMENTED`, `VERIFIED`, `BLOCKED`.
`IMPLEMENTED` means code exists; only a real run plus regression evidence may
mark `VERIFIED`.

## Workbench shell

| Feature | OpenIDEA source | RustIdea implementation | Status |
|---|---|---|---|
| Workbench frame / Islands | `src/workbench/Workbench.tsx`, `Workbench.module.css` | `experiments/zed-poc/crates/workbench/src/app.rs` (flat frame) | NOT_STARTED |
| Title bar + toolbar | `src/workbench/TitleBar.tsx`, `ContributedToolbar.tsx`, `mainToolbar.ts` | none | NOT_STARTED |
| Navigation bar / breadcrumbs | `src/workbench/EditorArea.tsx`, `src/editor/core` | none | NOT_STARTED |
| Window group tabs | `src/workbench/WindowGroupTabs.tsx` | none | NOT_STARTED |
| Tool-window layout state | `src/workbench/toolwindows/ToolWindows.tsx`, `store.ts`, `layouts.ts` | `crates/rustidea-design/src/workbench/layout.rs` (model + tests only) | IN_PROGRESS |
| Tool-window registry | `src/workbench/contributions.ts` | `crates/rustidea-design/src/workbench/registry.rs`; renderer still uses `SidebarMode` | IN_PROGRESS |
| Stripes | `toolwindows/stripes.ts`, `ToolWindows.module.css` | 27 text abbreviations in `app.rs` | NOT_STARTED |
| Status bar | `src/workbench/StatusBar.tsx` | 24 px ad-hoc row in `app.rs` | NOT_STARTED |
| Welcome | `src/workbench/Welcome.tsx` | empty editor hint only | NOT_STARTED |
| Overlays / menus / dialogs | `src/ui/Menu.tsx`, `Dialog.tsx`, `Popup.tsx` | ad-hoc popups in `app.rs` | NOT_STARTED |

## Design system

| Feature | OpenIDEA source | RustIdea implementation | Status |
|---|---|---|---|
| Tokens (dark/light/HC/compact) | `src/styles/tokens.css` | `crates/rustidea-design/src/tokens*.rs`, `semantic.rs` | IMPLEMENTED |
| Token → GPUI adapter | `src/styles/*` | `workbench/src/theme.rs` | IMPLEMENTED |
| Editor scheme | `--syn-*`, `--bg-editor*`, `--gutter*` | `crates/rustidea-design/src/editor_theme.rs`; not installed into Zed | IN_PROGRESS |
| Icons | `src/ui/icons.tsx`, `expui/**`, `openidea/**` | none | NOT_STARTED |
| File/folder presentation | `src/ui/filePresentation.ts`, `folderPresentation.ts` | none | NOT_STARTED |
| Base components | `src/ui/{Button,Input,Tabs,Tree,Menu,…}` | ad-hoc GPUI elements | NOT_STARTED |

## Project, editor and documents

| Feature | OpenIDEA source | RustIdea implementation | Status |
|---|---|---|---|
| Project open/trust/multi-module | `src/features/project-tree`, `src/platform/project` | `NativeHost` + `ProjectTree` (`workbench/src/project.rs`) | IN_PROGRESS |
| Project tree UI | `src/features/project-tree/*`, `src/ui/Tree.tsx` | `render_project` in `app.rs`; plain names/chevrons only | IN_PROGRESS |
| File operations (create/rename/move/delete) | `src/features/project-tree/*` | host APIs exist; no ported UI/menus | IN_PROGRESS |
| Zed editor integration | n/a (RustIdea-specific) | `Editor`/`Buffer`, sync, completion, semantics, code actions | IMPLEMENTED |
| Tabs / splits | `src/workbench/EditorArea.tsx`, `src/ui/Tabs.tsx` | custom tabs + one split action | IN_PROGRESS |
| DocumentStore authority | `src/editor/api`, `src/editor/core` | per-document entities in `app.rs` | IN_PROGRESS |
| LSP surface | `src/editor/lsp` | completion, hover, definition, references, rename, code actions, diagnostics | IN_PROGRESS |
| Editor theme / font parity | `src/styles/tokens.css` | Zed bundled theme today | NOT_STARTED |

## Tools and services

| Feature | OpenIDEA source | RustIdea implementation | Status |
|---|---|---|---|
| Search | `src/features/search` | `SidebarMode::Search` | IN_PROGRESS |
| Git changes/diff/history | `src/features/vcs`, `src/workbench` | Git sidebar; no diff/history UI | IN_PROGRESS |
| Build tools (Maven/Gradle) | `src/features/build-tools` | `build` pane + host build API | IN_PROGRESS |
| Run configurations | `src/features/run-anything` | bottom run configurations/actions | IN_PROGRESS |
| Debug (DAP/JDWP) | `src/features/debugger` | debug pane + `ide-dap`/`ide-jdwp` | IN_PROGRESS |
| Terminal | `src/features/terminal` | terminal pane | IN_PROGRESS |
| Problems / inspections | `src/features/code-insight`, `analysis` | `SidebarMode::{Problems,Analysis}` | IN_PROGRESS |
| Structure / outline | toolwindow `structure` | `SidebarMode::Outline` | IN_PROGRESS |
| TODO / bookmarks | `src/features/todo` | `SidebarMode::{Todo,Bookmarks}` | IN_PROGRESS |
| Database | `src/features/database` | `SidebarMode::Database` | IN_PROGRESS |
| HTTP client | `src/features/http-client` | `SidebarMode::Http` | IN_PROGRESS |
| SSH / remote dev | `src/features/ssh`, `remote-dev`, `deployment` | `SidebarMode::Remote` | IN_PROGRESS |
| Agents | `src/features/agents` | `SidebarMode::Agents` | IN_PROGRESS |
| MCP | `src/features/mcp` | `SidebarMode::Mcp` | IN_PROGRESS |
| Plugins / tools / marketplace | `src/features/plugins`, `tools` | `SidebarMode::{Plugins,Tools}` | IN_PROGRESS |
| Notebook / plots | `src/features/notebook` | `SidebarMode::Notebook` | IN_PROGRESS |
| Coverage | `src/features/coverage` | `SidebarMode::Coverage` | IN_PROGRESS |
| Profiler | `src/features/profiler`, `perf` | `SidebarMode::Profiler` | IN_PROGRESS |
| Heap analysis | `src/features/analysis` (heap) | `SidebarMode::Heap` | IN_PROGRESS |
| Logcat | `src/features/logcat` | `SidebarMode::Logcat` | IN_PROGRESS |
| Scratch / recent files | `src/features/scratch`, `welcome` | `SidebarMode::{Scratch,Recent}` | IN_PROGRESS |
| Settings | `src/features/settings` | `SidebarMode::Settings` | IN_PROGRESS |
| GitHub / pull requests | `src/features/github` | `SidebarMode::GitHub` | IN_PROGRESS |
| GraphQL / Jupyter / NuGet / Flutter / .NET | tool windows in `src/features/*` | not registered | NOT_STARTED |

## Actions, keymaps and tests

| Feature | OpenIDEA source | RustIdea implementation | Status |
|---|---|---|---|
| Action registry / menus | `src/workbench/contributions.ts`, `src/commands` | Zed keymap only (603 bindings) | NOT_STARTED |
| OpenIDEA keymaps (IntelliJ/VSCode/custom) | `src/platform/keymap.ts` | none | NOT_STARTED |
| Command palette / double shift | `src/ui/CommandPalette.tsx` | ad-hoc quick open | NOT_STARTED |
| Component tests | `src/ui/*.test.tsx` | none | NOT_STARTED |
| Real visual gate | screenshots in `tests/visual-baselines/openidea` | real capture pipeline exists; RustIdea states not captured yet | IN_PROGRESS |

## Current blockers

- The full OpenIDEA `cc5d9d95` visual baseline needs one capture session with an
  unlocked display.
- Every shell row above must be ported before any `VERIFIED` status is possible.
