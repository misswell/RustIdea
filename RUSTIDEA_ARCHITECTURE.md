# Native architecture

## Runtime

`crates/rustidea` is the native application. `main.rs` installs logging, a
panic hook, GPUI menus/key bindings and opens a native window. `NativeHost`
owns one `ide_host::Host`, a Tokio runtime, the main window id and the host
event channel. It exposes typed operations for project, document, filesystem,
VCS, search, terminal, run, build, database, HTTP and LSP calls.

`NativeHost::run` moves an OpenIDEA operation onto the host Tokio runtime.
Several backend operations call `tokio::spawn_blocking` directly and assume
they run inside the host runtime; GPUI polls its own tasks on the macOS main
thread. The adapter closure and event sink are the only RustIdea-specific
layer. There is no `LocalBackend`, Tauri IPC, React, CodeMirror, Monaco,
xterm.js or WebView in the shipped application.

Blocking or host-bound work is spawned on the runtime and results return to
GPUI through tasks. `IdeEvent` messages use a bounded queue (2,048 entries):
coalescible events keep the latest logical state, filesystem batches merge path
sets, document-patch loss sets a resync flag, and lifecycle/other loss sets a
general refresh flag. The UI pump drains both the queue and overflow reports.

## UI composition

The GPUI workbench in `app.rs` and `app/ui.rs` owns:

- titlebar, menus, activity rail, command palette and quick open;
- left navigation, right informational panel, bottom output/run/debug panel;
- resizable panel dimensions;
- editor groups, tabs, horizontal/vertical split axis, close/unload flow;
- project tree, search, VCS, structure, problems, settings, database and HTTP
  tools;
- status bar, dialogs, context menus and the platform character palette.

The native editor in `editor.rs` implements `Element` and
`EntityInputHandler`. It virtualizes visible lines, lays out shaped text and
gutter numbers, paints selections/caret/diagnostics, handles mouse selection,
scroll wheel and keyboard actions, and supports IME marked text through UTF-16
ranges.

## Documents

`DocumentStore` is the canonical UI registry. On the UI thread, paths use pure
lexical normalization; the backend `DocumentInfo.path` is the authoritative
canonical identity once an open completes. Untitled buffers are keyed by the
backend `DocId` until Save As assigns a path. The store counts
tab/split views, returns the same `Entity<DocumentBuffer>` to every view, rekeys
the same buffer on Save As, and releases the backend document exactly once
when the last view closes. Concurrent opens of the same path are coalesced with
the `pending_opens` set.

`DocumentBuffer` is the one editable UI mirror for that document. Views hold
selection, scroll and focus independently, while all views observe the same
buffer. Local edits are coalesced after a 50 ms debounce into one diff from the
last confirmed base text. At most one request is in flight. Successful replies
advance the confirmed version; edits made during the request stay pending.
Backend patches for our own acknowledged version are ignored by
`ignore_patches_through`. A version gap or rejected base version triggers a
backend snapshot resync, never a blind full reload that discards unsynced
input.

When a backend patch overlaps pending local text, a pure three-way merge is
attempted. Disjoint changes are rebased and applied as external edits (without
adding undo entries); overlapping edits enter an explicit conflict that
preserves local text until the user chooses **Keep local** or **Reload
backend**. Save waits for synchronization and is refused in a conflict state.

The host `ide-workspace` document registry remains authoritative for save,
external disk tracking, LSP and workspace edits. It retains a synchronized
full-text mirror because that is the existing OpenIDEA service contract. This
is a known deviation from the task book's strict single-copy ideal, but there
is no per-tab or per-split copy in RustIdea.

### External changes and conflicts

Each buffer records a `DiskVersion` containing `(exists, size, mtime, BLAKE3
content hash)`. Hashing is streamed in 64 KiB chunks on the backend runtime,
never on the GPUI thread. Metadata is a fast path; the hash catches same-length
rewrites and timestamp-preserving changes. Save verification hashes the current
file before writing. A changed clean file is resynced from the backend; a dirty
or deleted file raises an explicit conflict banner with **Reload** and **Keep
my version**. Saves are refused while a conflict is unresolved, so an external
edit is never silently overwritten.

## Project tree

The tree model caches one `ide_vfs::FileEntry` listing per loaded directory.
`ide_host::ops::fs::read_dir` supplies the listing, so `.gitignore`, `.ignore`
and configured excludes use the existing OpenIDEA filtering instead of a new
walker.

- The root is read once after project open and each child only when first
  expanded.
- Expanding/collapsing a cached directory rebuilds the flattened row list with
  zero filesystem calls.
- File watcher events invalidate only the affected loaded parent directories;
  removed directories are not re-read.
- Each directory has one in-flight generation; duplicate requests are dropped,
  and a stale result cannot replace a newer listing.
- Directory reads run on the backend runtime. Results are applied only when both
  the project generation and directory generation still match.
- Directories after the first 2,000 entries show an explicit **Show N more…**
  row; paging preserves access without building a 100,000-row element array.
- The row list is rendered with GPUI `uniform_list`, so only visible rows are
  materialized even when a loaded project has tens of thousands of entries.

## Render notification split

The workbench no longer observes every `Editor` notification. Text edits emit
`BufferEvent::Changed`, which the editor consumes for its own repaint and the
workbench deliberately ignores. Only semantic events notify the workbench:
dirty state (tab dot), title change (tab label/Save As), save state (status bar),
errors, external conflicts and project tree updates. Tab titles are read from
the shared buffer, so there is no editor observer maintaining duplicate labels.

`ide-text`/Ropey remains the text engine. Positions crossing the editor,
LSP and host boundary are UTF-16; byte offsets are converted at the editor
input boundary. Original line endings are restored by `ide-text` on save.

The editor virtualizes the visible line range. Syntax highlighting is requested
on the backend runtime, committed only for the latest document version and
discarded when a newer edit arrives. A bounded `LayoutCache` keeps shaped text
and gutter numbers for the current window, invalidated by document version, IME
marked range, theme or diagnostics epoch; scrolling reuses cached shapes instead
of calling `shape_line` again. Undo/redo mirror only the exact inverse/forward
edit set, not two full document copies; external edits rebase history ranges.
History is capped at 1,000 transactions and autosave timers are coalesced.
Documents over 4,000 lines or 512 KiB of UTF-16 text skip syntax parsing so
large-file editing stays responsive; the backend's `large` flag also disables
heavy features.

`crates/rustidea-editor/benches/document.rs` is the repeatable engine baseline:
single-line edit, undo/redo and 60-line viewport extraction on synthetic 100k-
and 1M-line documents.

## Reused backend layers

The `ide-*` crates are copied Apache-2.0 backend code. They provide language
registries, Tree-sitter, LSP/DAP, workspace/search/VCS/terminal/run/build/db,
plugin, agent, remote and tooling services. RustIdea adds native adapters and
panels, not parallel implementations of those services. Features whose panel
is still a placeholder are clearly marked in `RUSTIDEA_LEGACY_FEATURE_PARITY.md`.

## Constraints and follow-ups

- No screenshot, static text or WebView fallback may stand in for native
  editing or a native tool.
- The terminal currently renders a bounded ANSI-consumed text stream, not a
  full terminal screen; that must be upgraded before terminal parity is
  claimed.
- The GPUI application must remain launchable on macOS, Windows and Linux;
  only macOS interactive acceptance is currently targeted.
- The strict one-copy document model, LSP completion and
  plugin/agent/remote surfaces are not complete.
