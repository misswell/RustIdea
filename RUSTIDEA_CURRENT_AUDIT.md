# RustIdea current audit

Monday, 2026-11-28. This is the P0 audit required before rebuilding the
workbench. It records what exists, what runs, and what is still missing.

## Baselines

| Repository | Commit | Notes |
|---|---|---|
| RustIdea (this tree) | `9fdfb25339cf8b3d4f7e3125e1e527fc3e875392` | local `main`; six commits ahead of `origin/main` (`34f59cdfc5b3e6af9f28d0c9dce1c8eee9718386`), not pushed yet |
| RustIdea `origin/main` | `34f59cdfc5b3e6af9f28d0c9dce1c8eee9718386` | requested review baseline |
| OpenIDEA source | `cc5d9d95e4f97a033c97b4a4753047db3b3d1a73` | `v0.1.53-325-gcc5d9d95e`, 2026-11-28 05:55 |
| OpenIDEA installed app | 0.1.54, built 2026-11-27 | older than the pinned commit; must not be used as the new visual baseline |
| Zed | `2664a0aa1eae5dabf8f8273c03f88d53061ca7dc` | `third_party/zed`, shallow checkout + `experiments/zed-poc/patches/zed-tree-sitter-0.25.patch` |

The OpenIDEA source worktree at `/Users/guofeng/Code/solo/openidea` is on
`048b87e4aea38924e2adf5f5f8ca4806640cb401` with only untracked `artifacts/`.
A detached worktree of the pinned commit lives at `/tmp/openidea-cc5d9d95`.

## Build and run entry points

- `scripts/build-native.sh` builds the official native app
  (`experiments/zed-poc/crates/workbench`, release by default, `--locked`).
- `scripts/run-native.sh` runs the built binary, optionally with `--trust`.
- `scripts/package-native-macos.sh` packages `RustIdea Native Preview.app`.
- `scripts/release-macos.sh` builds, signs, notarizes and publishes.
- `crates/rustidea` and `crates/rustidea-editor` remain as the frozen legacy
  editor and historical regression baseline; the README already names the Zed
  workbench as the single official application.

## Workspace map

- `crates/ide-*`: 48 reusable OpenIDEA backend crates (host, project, VFS,
  index, LSP, build, run, DAP/JDWP, terminal, VCS, search, DB, HTTP, SSH,
  remote, agents, MCP, plugins, notebook, coverage, profiler, heap and more).
- `crates/rustidea-design`: generated OpenIDEA design system (428 dark tokens,
  light/high-contrast/compact tables), editor theme mapping, tool-window
  registry and a 995-line Islands layout state machine with tests.
- `experiments/zed-poc/crates/workbench`: the native workbench. `app.rs` is
  21,056 lines; all other workbench modules add up to 29,795 lines.
- `third_party/zed`: pinned Zed editor, buffer, language, theme and GPUI
  crates, consumed by path from the experiment workspace.

## What is wired today

- Zed `Editor`/`Buffer`/language engine with OpenIDEA document sync, completion,
  semantics and code actions providers.
- `NativeHost` bridge to the `ide-*` backend; project open, trust, directory
  reads, run configurations, Git, terminal, LSP and the feature panels.
- Workbench UI: flat 250 px left panel, 27 text-abbreviation sidebar buttons,
  editor tabs, permanent bottom pane, 24 px status row, ad-hoc popups.
- Design tokens: `theme.rs` adapts `rustidea_design::Semantics`; no independent
  palette.
- Visual pipeline: real-window text and tree rows now render (see below).

## What is missing or wrong

- No OpenIDEA title bar, main toolbar, navigation bar, window-group tabs or
  traffic-light-aware layout.
- No Islands frame: no left/right/bottom stripes, no gaps, no 10 px radii, no
  draggable splitters, no primary/secondary slots, no maximize/undock/restore,
  no layout persistence.
- No dynamic tool-window registry in the renderer; `SidebarMode` is a 27-value
  enum and the frame builds every panel before choosing one.
- No OpenIDEA icon system or file presentation; stripe and tree use text and
  Unicode marks.
- Tabs, status bar and buttons are custom approximations, not ports of
  `Tabs.tsx`, `StatusBar.tsx`, `Button.tsx`.
- The editor still initialises Zed's bundled theme; the OpenIDEA editor scheme
  is not installed.
- Action/keymap registry, menus, dialogs and command palette are not ported.

## Bugs fixed on 2026-11-28

1. **Invisible text and icons in the real window.** `gpui_platform` was used
   without the `font-kit` feature, so Metal painted quads, borders and radii but
   no glyph or SVG pixels. The headless test passed because it uses the
   software renderer. Fixed in `experiments/zed-poc/Cargo.toml`; guarded by
   `tests/visual/window-render-smoke.sh` and a CI feature check.
2. **Blank project tree.** The `uniform_list` used GPUI's default `Auto` sizing
   behaviour, which gives it no height in this layout. Now uses
   `ListSizingBehavior::Infer` and an explicit empty/loading/error message.

## Evidence

- `tests/visual-reports/2026-11-28-renderer-unblock/rustidea-project-root-1600x900.png`
  is a real capture of the current native app at 1600×900 with text and tree
  rows visible.
- `cargo test --release -p rustidea-zed-workbench`: 69 unit tests and 2 headless
  render tests pass.

## Next implementation order

1. Capture the pinned OpenIDEA `cc5d9d95` build and refresh
   `tests/visual-baselines/openidea/`.
2. Port the icon assets and file presentation rules, then the Islands frame
   (title bar, both stripes, editor island, bottom island, status bar).
3. Replace `SidebarMode` with the `rustidea-design` tool-window registry and
   render only visible islands.
4. Port tabs, tree interactions, buttons/inputs/menus; install the OpenIDEA
   editor theme into Zed.
5. Re-run real screenshots and functional parity before touching features.
