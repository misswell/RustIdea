# RustIdea native application: the single official entry

Status: **decision recorded; stable replacement gated on maintainer approval.**

## Decision

The official RustIdea application is the Zed Workbench:

- Zed `editor`, `Buffer`, `language`, `Provider` and the document bridge from
  `experiments/zed-poc/crates/workbench`;
- every reusable OpenIDEA `ide-*` backend crate at the repository root;
- a GPUI workbench built from the OpenIDEA design system and contributions.

The legacy application (`crates/rustidea`, `crates/rustidea-editor`) is a
**frozen historical regression baseline**. It receives no new features and is
never packaged as a release. It remains in the workspace only so its tests and
the migration reports stay reproducible.

The separate Cargo workspace at `experiments/zed-poc` is a build-graph
isolation, not a separate product. The official scripts point only at it.

## Official commands

```sh
# one-time: fetch the pinned Zed tree
scripts/fetch-zed.sh

# debug build + run; --trust is the explicit Trust Project action
scripts/run-native.sh --trust /path/to/project

# release binary
PROFILE=release scripts/build-native.sh

# package, sign and notarize (native binary only)
scripts/release-macos.sh
```

`scripts/build-native.sh` is the only release build entry. It refuses to run
without the pinned Zed checkout and always builds `rustidea-zed-workbench`.

## Gates before replacing the stable app

The release script defaults to **RustIdea Native Preview** with bundle id
`com.guofeng.rustidea.native-preview`. Replacing the stable `RustIdea.app`
(`com.guofeng.rustidea`) requires:

1. Maintainer approval of the GPL-3.0-or-later combined-work distribution.
2. Maintainer approval of the final bundle id and signing identity.
3. Maintainer approval of the data migration below.
4. The visual parity gate in `RUSTIDEA_UI_PARITY.md` passing.

After approval, set `RUSTIDEA_STABLE_REPLACEMENT_APPROVED=1` for
`scripts/release-macos.sh`. This is a deliberate switch, not an accidental
side effect of a build.

## User-data migration

Legacy RustIdea stores a JSON session at:

```text
~/Library/Application Support/RustIdea/session.json
```

It contains: `theme` (`dark` default), `autosave`, `recent_projects` (max 12),
`last_project`.

Native RustIdea will use:

```text
~/Library/Application Support/RustIdea/native/
  session.json        # migration marker, theme/density, recent projects
  layouts/<scope>.json # LayoutSnapshot per project root
  state.json          # window bounds/zoom (non-critical)
```

Migration rules:

- On first launch, **read but never modify** the legacy session. Import
  `recent_projects`/`last_project`, `autosave`, and the theme choice
  (`dark` -> Islands Dark, `light` -> Islands Light). The native default is
  OpenIDEA's light theme; a legacy user keeps dark.
- Write `migrated_from_legacy: 1` and the source path. Never import twice.
- Keep the legacy file for at least two releases so a rollback to the frozen
  app still works.
- Tool-window layouts are new: start from OpenIDEA's default layout
  (`project` on the left, everything else closed). Do not invent a layout from
  the legacy sidebar enum.
- OpenIDEA project settings under `.openidea/` and the OpenIDEA application data
  directory remain authoritative for backend behaviour; the native shell reads
  the same files. It must not fork settings semantics.
- No migration deletes files, rewrites user source, or clears unsaved data.
  A failed migration leaves the legacy session untouched and starts with
  defaults.

## Product identity

- Window/menu title: **RustIdea** (the experiment's "RustIdea Zed" title is a
  migration bug).
- Binary: `rustidea-native` in the app bundle; Cargo target can stay
  `rustidea-zed-workbench` until the workspace is renamed.
- Preview bundle id: `com.guofeng.rustidea.native-preview`.
- Stable bundle id, after approval: `com.guofeng.rustidea`.
- No WebView/WebKit/JavaScriptCore/Tauri/wry dependency; the release script
  checks the linked libraries and fails if one appears.

## Data and state ownership

| State | Owner |
|---|---|
| Document text, versions, disk stamps | OpenIDEA `ide-host::ops::doc` + Zed Buffer bridge |
| Project/workspace data, LSP, terminals, run/debug, VCS | OpenIDEA backend crates |
| Editor layout, selections, folds | Zed Editor |
| Tool-window visibility/sizes/placements | `rustidea-design::workbench::LayoutState` |
| Theme/tokens | `rustidea-design` generated from OpenIDEA tokens |
| Window bounds/zoom | Native shell |
| Legacy session | Read-only migration input |

## CI

The native job must contain, separately:

- **Backend Regression**: root workspace tests.
- **Zed Integration**: workbench/check/test/clippy.
- **Design/UI Model**: `cargo test -p rustidea-design` and token `--check`.
- **Native Release Smoke**: build the native binary, launch it with a real
  project, verify the process stays alive and the window is titled RustIdea.
- **Visual Parity**: real OpenIDEA vs real RustIdea screenshots with
  `tests/visual/run-visual-regression.sh`.
- **License Compliance**: GPL bundle and source offer.
- **Performance**: native vs OpenIDEA on the same project and JDT.LS state.

A cargo check/test/clippy pass is not a GUI acceptance result.

## Current blockers (this revision)

- The macOS session locked during capture. Only one real OpenIDEA reference
  screenshot was collected; the remaining scenarios are pending.
- The native workbench still renders the old experiment shell. Its window title
  and UI are not release quality.
- The installed OpenIDEA 0.1.54 lacks embedded source provenance. Rebuild the
  reference from `fe3e647` (or add provenance to the build) before final visual
  sign-off.
- GPL distribution, bundle id and migration approval are explicitly pending.
