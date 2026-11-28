# Shell comparison: OpenIDEA cc5d9d95 vs RustIdea native

Both windows: 1600×900 logical, built-in Retina display (1800×1169 logical
desktop), light theme, project `/Users/guofeng/Code/cscec3b/tg-tenant-fork`.
OpenIDEA is the source build of `cc5d9d95e4f97a033c97b4a4753047db3b3d1a73`
(0.1.54), launched from a temporary bundle, with Eclipse JDT.LS 1.63 installed.
RustIdea is `rustidea-zed-workbench` built from this worktree with Zed
`2664a0aa`, 2026-11-28.

## Captures

- `openidea-project-root-1600x900.png`: project open, tree at root, no editor.
- `rustidea-project-root-1600x900.png`: project open, tree at root, no editor.
- OpenIDEA editor, tabs and Java state live in
  `../2026-11-28-openidea-cc5d9d95/`.

## Measured geometry

OpenIDEA, from pixel runs on the capture:

| Region | Pixels |
|---|---|
| Title bar | 0–40 (40 px) |
| Left stripe background | 0–38 (~40 px) |
| Left island | 41–278 (238 px wide) |
| Island gap | 279–287 (9 px, token `--island-gap`) |
| Editor island | 288–1557 |
| Right stripe | 1560–1599 (~40 px) |
| Island bottom | 868 |
| Gap to status bar | 869–870 |
| Status bar | 871–898 (28 px + border) |
| Tree header (`项目 ⌄`) | 40–68 |
| Tree rows | 24 px |

RustIdea, same rows:

| Region | Pixels |
|---|---|
| Native title bar | 0–32 (32 px, not OpenIDEA's bar) |
| Left flat panel | 0–247 (248 px, no stripe) |
| Panel border | 248–249 |
| Editor area | 250–1598 (grey background, no island radius/gap) |
| Permanent bottom pane | 636–874 |
| Status row | 877–898 (22 px) |

## Differences that block parity

1. No 40 px OpenIDEA title bar: project widget, branch, run configuration,
   Run/Debug, search, gear and traffic-light padding are missing.
2. No stripes. RustIdea puts 27 text abbreviations inside a flat 248 px panel;
   OpenIDEA uses a ~40 px icon stripe and a separate rounded island.
3. No island gaps/radii. RustIdea's panels are edge-to-edge; OpenIDEA uses a
   9 px gap and 10 px corners on a shared frame background.
4. RustIdea has a permanent bottom pane; OpenIDEA opens bottom tool windows on
   demand and gives the editor the full height otherwise.
5. Status bar is 22 px with plain text instead of the 28–34 px contribution
   bar (breadcrumbs left, `jdtls +1`, caret, encoding, keymap, indent on the
   right).
6. Tree rows use plain chevrons/`·` and no file icons; OpenIDEA has 20×20 file
   icons, Git/annotation decorations, inline root path and hover/selection
   bands.
7. Editor background, tab strip and gutter are grey custom surfaces; OpenIDEA
   renders a white editor island with its own tab bar (28 px tabs) and
   breadcrumb row.

## Evidence for the reference editor states

- `../2026-11-28-openidea-cc5d9d95/openidea-pom-editor-1600x900.png`
- `../2026-11-28-openidea-cc5d9d95/openidea-java-editor-1600x900.png`
- `../2026-11-28-openidea-cc5d9d95/openidea-goto-file-1600x900.png`

The Java capture shows two tabs, gutter annotations, code-lens text, breadcrumb
status and `jdtls +1` / `17:14` / `LF` / `UTF-8` / `Vim` / `4 个空格` items.

## Status

This is a diagnostic report, not an acceptance pass. No RustIdea state is
marked parity yet; the shell rewrite is the next P1 step.
