# RustIdea UI parity with OpenIDEA

> Specification: OpenIDEA `fe3e647abb804cffbfff6bd8de42e01762d6de40` (0.1.54).
> RustIdea baseline: `34f59cdfc5b3e6af9f28d0c9dce1c8eee9718386`.
> Zed: `2664a0aa1eae5dabf8f8273c03f88d53061ca7dc`.

This document is the migration ledger. It records the OpenIDEA source file, the
RustIdea implementation file, the original geometry and behaviour, the data
source, the screenshot evidence and every unresolved difference. A component is
not "done" because it renders: it is done only when the evidence columns are
filled and the two real applications match at the same state.

## Status vocabulary

- **Foundation**: Rust model/tokens exist and are tested. No pixels yet.
- **Porting**: a native renderer exists but has not passed a real screenshot.
- **Parity**: same state as OpenIDEA, screenshot/test evidence recorded.
- **Gap**: deliberately not implemented or blocked; the reason is written down.

No component is marked **Parity** in this revision. The current workbench is
still the old experiment (`app.rs`, one growing `SidebarMode` enum, a fixed
250 px sidebar, a permanent bottom pane and a 24 px status bar), not the
OpenIDEA Islands frame. What has changed is that the reference side is now
complete: 20 of 21 states have real OpenIDEA 0.1.54 captures, the token table is
generated from the pinned CSS, and the GPUI `theme.rs` adapter derives every
colour from `Semantics` instead of a hand-tuned palette.

## Component ledger

| OpenIDEA source | RustIdea implementation | Original geometry / behaviour | Data source | Screenshot | Tests | Status / unresolved |
|---|---|---|---|---|---|---|
| `src/styles/tokens.css` | `crates/rustidea-design/src/tokens.rs`, `tokens_generated.rs`, `semantic.rs`, `metrics.rs`; converter `tools/openidea-tokens/generate.py` | 428 dark tokens; light/high-contrast/compact tables; `--h-titlebar 40`, `--h-header 40`, `--h-tab 28`, `--h-row 24`, `--h-statusbar 30`, `--stripe-icon-button 30`, `--island-gap 6`, `--island-radius 10`, `--fs-ui 13`; compact 32/32/24/22/24/24/4/8 | `third_party/openidea/tokens.css` at the pinned commit | `tests/visual-baselines/openidea/01-welcome.png` … `21-light-theme.png` (20 real captures at the pinned build) | `cargo test -p rustidea-design` (28 tests); token hash check in CI | **Foundation.** Full token table generated and tested; `theme.rs` now adapts `Semantics` and must not grow its own palette. The editor scheme still needs to be installed into Zed. |
| `src/workbench/Workbench.tsx` | `experiments/zed-poc/crates/workbench/src/app.rs` (current); planned `.../src/workbench/mod.rs` | Frame background, project colour wash, titlebar + islands + statusbar; welcome replaces the frame without a project | `ProjectInfo`, contributions, layout state | Not captured with the native frame | Workbench tests only cover data paths | **Porting.** No titlebar/stripes/islands in the current render; `app.rs` is 21 038 lines and must be split by feature. |
| `src/workbench/TitleBar.tsx`, `TitleBar.module.css`, `ContributedToolbar.tsx`, `mainToolbar.ts` | planned `.../workbench/title_bar.rs`; `TitleItemRegistry` in `rustidea-design/src/workbench/registry.rs` | 40 px bar, `padding 0 8px 0 10px`; macOS traffic-light inset `max(82px, 82px/zoom)`; project widget (avatar, name, chevron), VCS branch, run configuration, Run, Debug, search, gear; drag region; compact 30 px header | Project, VCS, run/config, actions | None yet | Registry unit tests | **Gap.** Current workbench has no titlebar widgets or macOS traffic-light-aware padding. |
| `src/workbench/EditorArea.tsx`, `EditorArea.module.css` | `app.rs` editor/split render; planned `.../workbench/editor_area.rs` | Editor island fills the centre track; tabs live above it; empty-editor hints use the OpenIDEA key hints | `Editor` entities, document store | None | Existing Zed integration tests | **Porting.** Editor engine is Zed; the surrounding island, tab strip, breadcrumb and split chrome are not ported. |
| `src/workbench/StatusBar.tsx`, `StatusBar.module.css` | `app.rs` bottom status row; planned `.../workbench/status_bar.rs`; `StatusItemRegistry` | Height `--h-statusbar + 4px` = 34 px; 10/12 px padding; left/right widget groups; breadcrumbs and background tasks shrink first | Status item contributions | None | Registry unit tests | **Gap.** Current bar is 24 px with ad-hoc text. |
| `src/workbench/toolwindows/ToolWindows.tsx`, `ToolWindows.module.css`, `store.ts`, `layouts.ts` | `crates/rustidea-design/src/workbench/layout.rs` (state machine and tests); renderer still in `app.rs` | Left/right stripes; editor track `minmax(200px,1fr)`; bottom track `minmax(0,280px)`; min 200×60 editor; primary + split areas per side; 6 px gaps; 10 px island radius; undock/overlay; hide/maximize/restore; per-root persistence | `ToolWindowRegistry` + project root | None | 14 layout tests covering replacement, split areas, maximize, hide-all, move-to, sanitize, session filtering, distraction-free | **Foundation + porting.** Model matches OpenIDEA; renderer still uses one 250 px sidebar and a permanent bottom pane. |
| `src/workbench/contributions.ts` | `crates/rustidea-design/src/workbench/registry.rs` | Tool window / title item / status item / overlay / editor kind / settings page / action registries; ordered by `order` then title; `stripe: onUse`; `secondary`; `sessionContent` | Feature modules | N/A | Registry and builtin-descriptor tests | **Foundation.** No more `SidebarMode` additions allowed; the native renderer must consume this registry. |
| `src/ui/Tabs.tsx`, `Tabs.module.css` | planned `.../ui/tabs.rs`; tab strip in `app.rs` | 38 px tab bar; 28 px tab (22–28 px); 6 px radius; 8 px horizontal tab padding; 6 px gap; selected pill `bg-tab-selected` with 1 px inset border; 20 px close slot; 6 px dirty dot; overflow scroll; drag reorder; pinned tabs; context menu | Document store, editor kinds | None | None yet | **Gap.** Current tab strip is custom and not measured against OpenIDEA. |
| `src/ui/Tree.tsx`, `Tree.module.css`, `filePresentation.ts`, `folderPresentation.ts`, `iconNode.ts` | `app.rs` `render_project`; `project.rs`; planned `.../ui/tree.rs`, `.../ui/file_presentation.rs` | 24 px rows; 5 px selection radius; `--tree-inset 6px`; 16 px chevron; 2 px icon gap; file tint bands; joined adjacent selection; virtualised rows; keyboard navigation; multi-select; drag/drop; context menu; expansion/scroll restore | `ide_vfs`, `ide_index`, VCS, diagnostics | None | Project tests | **Porting.** Loading/empty/error states exist but the tree is not the OpenIDEA tree; text abbreviations are still used in the stripe. |
| `src/ui/Button.tsx`, `Button.module.css` | planned `.../ui/button.rs` | 28 px high, min 72 px, 16 px padding, 4 px radius; small 24 px; primary/secondary/ghost/icon variants; 2 px focus ring | Actions | None | None yet | **Gap.** Current workbench builds ad-hoc `div()` buttons. |
| `src/ui/Input.tsx`, `Input.module.css` | planned `.../ui/input.rs`; `text_input.rs` currently | 28 px field; 8 px padding; 4 px radius; inset border; focus ring; search field with leading icon | Text input entities | None | Existing text input tests | **Porting.** Behaviour exists; OpenIDEA visual frame and focus ring are not applied. |
| `src/ui/Menu.tsx`, `Menu.module.css`, `menuAim.ts` | planned `.../ui/menu.rs` | 8 px popup radius; 28 px menu rows; section separators; submenu aim; keyboard mnemonic/navigation; checked/disabled states | Action registry, menus | None | None yet | **Gap.** Menus are not ported. |
| `src/ui/Dialog.tsx`, `Dialog.module.css` | planned `.../ui/dialog.rs` | 10 px dialog radius; native window shadow; title bar; footer buttons; focus trap; Escape/Enter semantics | Overlays | None | None yet | **Gap.** Save As/Rename use bare input overlays. |
| `src/ui/Popup.tsx`, `Popup.module.css`, `popupState.ts` | planned `.../ui/popup.rs` | 8/10 px popup radii; anchor placement; shadows; outside-click/focus dismissal; one popup owns the keyboard | Overlay registry | None | None yet | **Gap.** |
| `src/ui/Tooltip.tsx`, `Tooltip.module.css` | planned `.../ui/tooltip.rs` | 6 px radius; 4×12 px shadow; delay; optional shortcut label; keyboard focus shows it | Action registry | None | None yet | **Gap.** |
| `src/ui/icons.tsx`, `icons.module.css`, `expui/**`, `openidea/**`, `brands/**` | planned `.../ui/icons.rs`; vendor target `third_party/openidea/icons/`; custom `AssetSource` | 16 px normal / 20 px stripe icons; monochrome uses `currentColor`; light/dark variants; JetBrains expui (Apache-2.0/MIT) and OpenIDEA icons | Icon name in contribution | None | Icon existence/format tests | **Gap.** Current stripe uses `PRJ`, `SRC`, `DEP`, `SET` text. This is explicitly forbidden by the task and must be replaced by the original SVG assets. |
| `src/ui/filePresentation.ts` | planned `.../ui/file_presentation.rs` | Extension/name classification and colour/icon; Java class/interface/enum/test; package; Maven/Gradle; resources; VCS overlay; error overlay; decoration priority | Project tree entry + VCS + diagnostics | None | None yet | **Gap.** Current tree uses simple text/angle brackets and no file-type icons. |
| `src/workbench/Welcome.tsx`, `Welcome.module.css` | `app.rs` empty state; planned `.../workbench/welcome.rs` | Welcome window with project mark, recent projects, Open/New actions | Recent store | None | None | **Gap.** |
| `src/workbench/WindowGroupTabs.tsx`, `WindowControls.tsx`, `WindowResizeHandles.tsx` | not implemented | macOS window group tabs, traffic-light aware controls, resize handles | Platform | None | None | **Gap.** |
| `src/platform/keymap.ts`, IntelliJ/VS Code keymaps | Zed keymap bridge in `main.rs`, 603 default bindings; product action map not migrated | Multi-segment shortcuts, Double Shift, context conditions, collision check, IntelliJ and VS Code presets | Keymap registry/action registry | N/A | Zed keymap load test | **Porting.** Zed defaults are not a substitute for OpenIDEA's action IDs. |
| OpenIDEA editor scheme (`--syn-*`, `--bg-editor*`, `--gutter*`) | `crates/rustidea-design/src/editor_theme.rs` | Editor background, current line, selection, caret, gutter, active line number, indent guides, fold markers, whitespace, inlay hints, search match, diagnostics, VCS gutter, font/line height | Semantic tokens | None | Editor theme mapping tests | **Foundation.** The workbench still initialises Zed's bundled theme (`theme_settings::init(...)`); the adapter must install this scheme. |

## Design-token migration

`tools/openidea-tokens/generate.py` is the build-time converter. It:

- reads the pinned `third_party/openidea/tokens.css` (not a browser runtime);
- applies the `:root`, `[data-theme="light"]`, `[data-theme="dark-hc"]` and
  `[data-compact]` cascades;
- resolves `var()` and `color-mix(in srgb, ...)`;
- emits six complete sorted tables (dark/light/high-contrast × regular/compact)
  plus a source SHA-256;
- is checked in CI with `--check`.

The token tables contain **428** resolved tokens. The key metric assertions are
in `tokens.rs`; compact values are read from the compact table rather than
recomputed or hardcoded by the renderer. `Metrics` is derived from the token
set, including `stripe_width` and `stripe_button_gap`.

### Tokens still context-dependent

These are intentionally kept as raw expressions because they depend on runtime
state, not on the theme: project colour (`--project-color-*`),
`--background-image-*`, platform fonts, and gradient/shadow composition. The
renderer must resolve them from the same inputs OpenIDEA uses, not invent
defaults.

## Tool window mapping (OpenIDEA registration -> RustIdea descriptor)

The builtin registry in `registry.rs` uses the real OpenIDEA ids/icons/anchors:

| Area | Windows |
|---|---|
| Left primary | `project`, `structure`, `bookmarks`, `commit` |
| Left on use | `compare` |
| Right primary | `maven`, `database`, `agents`, `coverage`, `hierarchy` |
| Bottom primary | `run`, `debug`, `problems`, `git`, `terminal`, `todo`, `services` |
| Bottom on use | `find`, `build`, `inspection`, `duplicates`, `usages`, `profiler`, `logcat` |

This is not the final list: every feature module must register its own
descriptor. The rule is that a new window adds a descriptor and a feature
entity, not a new arm in a render `match`.

## Visual evidence

Reference screenshots live under `tests/visual-baselines/openidea/`; native
screenshots use the same scenario id under `tests/visual-baselines/rustidea/`.
`tests/visual/run-visual-regression.sh` writes `openidea.png`, `rustidea.png`,
`diff.png` and `metrics.json` per scenario and reports missing real captures as
`SKIP`, never as a pass.

Current evidence (all reference frames 1600×900 at the same window origin,
except Settings at 980×720):

| Scenario | OpenIDEA real capture | RustIdea real capture | Result |
|---|---|---|---|
| `01-welcome` | `openidea/01-welcome.png` | none | **Not run.** |
| `02-project-main` | `openidea/02-project-main.png` | `rustidea/00-before-zed-workbench.png` (1281×853, old dark empty shell) | **Fail / not comparable**: wrong size, wrong app state, no titlebar/islands. This is the "before" evidence. |
| `03-project-tree` … `18-context-menu` | `openidea/<id>.png` | none | **Not run.** |
| `19-save-conflict` | pending: the conflict toast needs an unlocked display | none | **Not run.** No placeholder is shipped. |
| `20-dark-theme`, `21-light-theme` | `openidea/<id>.png` | none | **Not run.** |

`python3 tests/visual/validate-manifest.py` enforces the contract: contiguous
ids, real PNGs matching the declared geometry, no duplicated images, no image
behind a pending status, and matching OpenIDEA commits in the manifest and the
environment record.

The installed OpenIDEA 0.1.54 is real and signed, but it does not embed its
source SHA. The definitive reference must be rebuilt from `fe3e647` or a build
with recorded provenance before release. The pinned source is vendored for
tokens only; it is not a runtime dependency.

## Pixel rules to enforce in the next renderer

- Container positions within 2 logical px.
- Control sizes within 2 logical px.
- Design tokens identical (the generated table is the source).
- Tool window area and stripe side identical.
- Same icons, labels and menu hierarchy.
- SSIM ≥ 0.98 with only documented masks for system font antialiasing and
  native traffic lights. Large masks or a uniform background are not accepted.

## Immediate next changes (in order)

1. Install `rustidea_design::EditorTheme` into Zed's `theme_settings` instead
   of Zed's default family. The GPUI adapter (`theme.rs`) already derives all
   colours from `Semantics`; the editor scheme is the remaining seam.
2. Add the renderer modules `workbench/{title_bar,status_bar,tool_windows,editor_area}.rs`,
   `ui/{tabs,tree,button,input,menu,dialog,popup,tooltip,icons,file_presentation}.rs`.
3. Make `app.rs` consume `Contributions`/`LayoutState`; build only visible
   tool-window entities and drop the `SidebarMode` enum.
4. Vendor the expui/OpenIDEA SVGs with their licenses and add the custom
   `AssetSource`; remove every text abbreviation from the stripe.
5. Capture the same scenarios in both apps (19 of 21 reference frames exist;
   `19-save-conflict` is blocked on an unlocked display), then turn the visual
   job into a required CI gate.

