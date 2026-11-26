# RustIdea V2 Zed POC report

## Baseline

- Branch: `experiment/zed-editor`
- RustIdea base: `b4833ea` plus the native-workbench LSP/find/SSH commit
  `b4dda49`.
- Zed: `2664a0aa1eae5dabf8f8273c03f88d53061ca7dc` (2026-11-24).
- Zed toolchain: Rust `1.100.0`.
- The experiment workspace is isolated at `experiments/zed-poc`. It is excluded
  from the main RustIdea workspace so the production 0.2.0 graph does not pull
  GPL Zed crates before the license decision.

## What is actually running

The POC binary is `rustidea-zed-poc`. It builds and runs with:

- Zed `editor::Editor` over a real `language::Buffer` (no self-written editor).
- OpenIdea `ide_host::Host` and `ide_host::ops::doc` as the document authority.
- A bridge that converts Zed `BufferEvent::Edited` incremental
  `Edit<OffsetUtf16>` records into OpenIdea `ChangeSet`s, keeping the Zed clock
  and OpenIdea `u32` version separate.
- `ide_host::ops::doc::edit` for incremental synchronization and
  `ide_host::ops::doc::save` for disk writes. The save calls
  `Buffer::did_save` only for the version that was synchronized.
- `OpenIdeaCompletionProvider`, installed with
  `Editor::set_completion_provider`, which maps `ide_host::ops::lsp::lsp_completion`
  results (including snippet defaults and replacement ranges) into Zed's native
  completion menu, and maps documentation, completion resolve and
  `additionalTextEdits`.
- `OpenIdeaSemanticsProvider`, installed with
  `Editor::set_semantics_provider`, which maps `ide_host::ops::lsp::lsp_hover`
  results into Zed's hover blocks and same-file `lsp_goto` results into Zed
  `LocationLink`s. Rename uses OpenIdea `lsp_prepare_rename` plus
  `lsp_rename`/`apply_workspace_edit`; document highlights use
  `lsp_document_highlights`. Inlay-hint mapping now calls
  `lsp_inlay_hints`; semantic tokens remain an explicit empty stub.
- An OpenIdea event pump that converts `IdeEvent::Diagnostics` into a Zed
  `DiagnosticSet` and calls `Buffer::update_diagnostics`, with one stable Zed
  language-server id per OpenIdea diagnostic producer.
- Two `Editor` views can be constructed over the same `language::Buffer`
  (`Editor::for_buffer`), so the same-file multi-view requirement is structurally
  satisfied.

A second binary, `rustidea-zed-workbench`, now hosts these pieces in a first
native workbench shell with a project tree, document tabs and a status bar. It
uses the same OpenIDEA Host and Zed Editor instead of a separate editor engine.

It does **not** create a Zed Project, does not start Zed LSP/index services, and
does not contain a WebView.

## Dependency finding: tree-sitter link conflict

Zed at this SHA requires a git `tree-sitter` 0.28, while the OpenIdea backend is
built on `tree-sitter` 0.25. Both declare `links = "tree-sitter"`, so Cargo
cannot link them into one binary. The experiment pins Zed's workspace
`tree-sitter` dependency to the OpenIdea-compatible `0.25.10` and applies the
small API adaptations recorded in
`experiments/zed-poc/patches/zed-tree-sitter-0.25.patch`:

- `QueryMatch::captures()` field access.
- two `QueryCaptures`/`QueryMatches` lifetime parameters.
- `set_byte_range`/`set_point_range`.
- boolean parse-progress callback.
- a `usize` JSX child index.
- a public `Editor::open_completions_menu` integration entry point so a host
  that owns its completion source can open Zed's existing menu without
  reimplementing it.

This is a confirmed fork adaptation, not a deletion of Zed modules. The
trade-off is that Zed's newest tree-sitter query APIs are not available in this
POC; the full integration must decide whether to keep this 0.25 fork or migrate
the OpenIdea tree-sitter consumers to a Zed-compatible version.

## Evidence

Build:

```sh
scripts/fetch-zed.sh
cd experiments/zed-poc
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build -p rustidea-zed-poc
```

Binary:

`/Users/guofeng/Code/solo/RustIdea/experiments/zed-poc/target/debug/rustidea-zed-poc`

The native window was started repeatedly and the process stayed alive. The
macOS console is now unlocked, but the agent process still creates its windows
in a non-visible Space/session: `CGWindowListCopyWindowInfo` reports the POC
windows with `onscreen=false`, and accessibility sees one window while screen
capture of the user's visible desktop does not contain it. A minimal GPUI
`rustidea-zed-hello` diagnostic binary shows the same placement. Therefore no
real keyboard, Chinese IME, clipboard or visual hover/menu acceptance is
claimed yet; this is an environment/window-placement blocker, not a substitute
for passing tests.

An automated end-to-end data test used a copy of the repository README:

```sh
cp README.md /tmp/rustidea-zed-poc-e2e.md
RUSTIDEA_POC_DEMO_EDIT="// RustIdea Zed bridge E2E 1795583444" \
  experiments/zed-poc/target/debug/rustidea-zed-poc \
  /Users/guofeng/Code/solo/RustIdea /tmp/rustidea-zed-poc-e2e.md
```

Result:

```text
POC_SAVE_OK // RustIdea Zed bridge E2E 1795583444
```

and the marker was present as the first line of the file after the process
exited. This proves the real Zed Buffer edit -> OpenIdea incremental edit ->
OpenIdea Host save path, but it does not replace interactive keyboard/IME
testing.

Completion provider test (TypeScript project, `typescript-language-server`
installed by the OpenIdea language registry):

```sh
RUSTIDEA_POC_TRUST=1 RUSTIDEA_POC_COMPLETION_DEMO=1 \
  experiments/zed-poc/target/debug/rustidea-zed-poc \
  /tmp/zed-poc-lsp-ts /tmp/zed-poc-lsp-ts/test.ts
```

Output:

```text
OPENIDEA_LSP typescript-language-server Ready
POC_PROVIDER_COMPLETIONS count=48
```

This opens the real Zed completion menu code path at the end of `text.` and
shows that the provider received 48 OpenIdea completions. It does not claim
interactive selection/clipboard/IME acceptance.

Hover backend test on the same TypeScript file:

```sh
RUSTIDEA_POC_TRUST=1 RUSTIDEA_POC_LSP_TEST=1 RUSTIDEA_POC_LSP_POS=1:5 \
RUSTIDEA_POC_HOVER_POS=0:6 \
  experiments/zed-poc/target/debug/rustidea-zed-poc \
  /tmp/zed-poc-lsp-ts /tmp/zed-poc-lsp-ts/test.ts
```

Output:

```text
POC_LSP_HOVER "\n```typescript\nconst text: \"hello\"\n```\n"
... 
POC_LSP_OK count=48
```

The hover is returned by OpenIdea LSP and converted by the Zed semantics
provider; the actual mouse-hover popover still needs an unlocked GUI session.

The same run reports a same-file definition:

```text
POC_LSP_DEFINITIONS count=1
POC_LSP_DEFINITION /private/tmp/zed-poc-lsp-ts/test.ts 1:6
```

Cross-file definition test (TypeScript import):

```text
POC_LSP_HOVER "(alias) const shared: 42 ..."
POC_LSP_DEFINITIONS count=1
POC_LSP_DEFINITION /private/tmp/zed-poc-lsp-xdef/defs.ts 0:13
```

For a target outside the active editor, the provider opens the file through
OpenIdea `ops::doc::open`, creates a read-only local Zed buffer for navigation
and anchors the `LocationLink` there. It does not yet keep that target buffer
synchronized with later backend edits; the final DocumentStore must own it.

Rename test on a TypeScript copy:

```text
POC_LSP_PREPARE_RENAME Some(Range { start: Pos { line: 1, col: 0 }, end: Pos { line: 1, col: 4 } })
POC_LSP_RENAME_OK renamed
```

The file became:

```typescript
const renamed = "hello";
renamed.
```

`perform_rename` returns an empty Zed `ProjectTransaction`; the actual edits
come back through OpenIdea `DocPatched` events and are applied to the Zed buffer
by the bridge. Multi-file rename therefore depends on the final DocumentStore
keeping every affected file open/synchronized.

Diagnostics bridge test with an intentional TypeScript type error:

```text
POC_DIAGNOSTICS source=typescript-language-server count=2
```

Two OpenIdea diagnostics were converted and applied to the Zed buffer. At the
time of that first test the POC mapped every source to one synthetic
language-server id. The current code keeps a source-to-id map (see Phase 3
additions below), so separate producers have separate Zed diagnostic sets.
Visual underline rendering still needs GUI inspection.

Multi-view test with two Zed editors over one Buffer:

```text
POC_SAVE_OK // multiview marker
POC_MULTIVIEW equal=true first_len=2746 second_len=2746
```

Both views held the same post-edit text. Interactive simultaneous editing and
scroll synchronization still need an unlocked GUI session.

External-change test: with a clean Zed buffer and a file inside the OpenIDEA
project root, appending to the file on disk produced:

```text
POC_EXTERNAL_PATCH version=1
```

The OpenIdea `DocPatched` event was converted into a Zed buffer edit and the
backend/confirmed clocks were advanced. A dirty-buffer guard refuses to apply a
patch when the Zed clock differs from the confirmed clock and keeps local text;
that conflict path has not yet been exercised by an interactive test. Files
outside the project root are not watched by this POC.

## Phase 3 additions (2026-11-25)

- `OpenIdeaSemanticsProvider` now maps
  `ide_host::ops::lsp::lsp_document_highlights` into Zed `DocumentHighlight`s.
- Diagnostics now use a stable source-to-`LanguageServerId` map, so separate
  OpenIdea producers replace only their own Zed diagnostic set instead of
  overwriting one synthetic `LanguageServerId(1)` set.
- `OpenIdeaCompletionProvider` now supplies documentation, LSP source metadata,
  completion resolve through `ide_host::ops::lsp::lsp_resolve_completion`,
  `additionalTextEdits` (applied after accept through a confirm callback) and
  commit-character data. Completion resolve is proven at the backend boundary;
  the visual menu and post-accept edit path still need onscreen interaction.
- `OpenIdeaCodeActionProvider` is installed through a minimal Zed fork entry
  point, `Editor::set_code_action_provider`. It asks
  `ide_host::ops::lsp::lsp_code_actions` for actions and applies a chosen action
  through `lsp_apply_code_action`, keeping OpenIdea as the edit authority.
- `OpenIdeaSemanticsProvider::inlay_hints` maps OpenIdea label/part/tooltip/
  padding/kind data into Zed `InlayHint`s. The live TypeScript and Rust
  language-server runs below returned `count=0`, so rendering of a real inlay
  hint is not claimed; the conversion path is compiled but not visually proven.
- The POC has a headless Java/Maven end-to-end run mode using the same OpenIdea
  Host, project, document and LSP services.

Automated GPUI input test:

```text
test tests::editor_types_and_exposes_shared_buffer ... ok
POC_TEST_TEXT first="hellostart\n" second="hellostart\n" buffer="hellostart\n"
```

The test creates two real Zed `Editor` views over one `language::Buffer` in two
GPUI test windows, focuses the first editor, sends `h e l l o` through GPUI's
`simulate_keystrokes`, and verifies that both editor snapshots and the shared
buffer contain the inserted text.

Java end-to-end evidence (small Maven project, managed `jdtls` 1.63.0):

```text
OPENIDEA_LSP jdtls Ready
POC_LSP_HOVER "```java\nGreeter greeter - com.example.App.main(String[])\n```"
POC_LSP_HIGHLIGHTS count=2
POC_LSP_CODE_ACTIONS count=6
POC_LSP_OK count=20
POC_LSP_ITEM greet
POC_LSP_RESOLVED equals documentation=true
POC_LSP_RESOLVED greet documentation=false
POC_RUN_CONFIG maven:package maven mvn package
POC_RUN_OUTPUT ... Compiling 1 source file ...
POC_RUN_OUTPUT ... BUILD SUCCESS ...
POC_RUN_EXIT Some(0)
```

An unresolved `ArrayList` completion in a Java file without the import returned
through the same resolve bridge as:

```text
POC_LSP_ITEM ArrayList
POC_LSP_RESOLVED ArrayList documentation=true additional_edits=1
```

This proves that the OpenIdea completion item, its lazily resolved documentation
and its import `additionalTextEdits` survive into the Zed completion object.
Accepting the item in the visual menu and observing the post-accept import edit
still requires the onscreen GUI acceptance noted below.

The Java save path was also exercised through the actual Zed buffer:

```text
POC_SAVE_OK // rustidea java save marker 1795599999
```

The first line of
`/tmp/rustidea-java-run/src/main/java/com/example/App.java` was the marker after
the process exited, proving Zed buffer edit -> OpenIdea incremental edit ->
OpenIdea Host save for Java. The Java project was then packaged successfully
through the OpenIdea run service (`mvn package`, exit 0).

## Phase 4 workbench shell (2026-11-25)

The new `experiments/zed-poc/crates/workbench` crate is a first Phase 4 slice:

- native project tree rows, expandable directories and a real OpenIDEA
  `ops::fs::read_dir` path;
- native document tabs backed by one cached Zed `Buffer` per OpenIDEA document;
- the Zed `Editor` renders the active tab and receives OpenIDEA completion,
  semantics and code-action providers through `DocumentLsp`;
- a split view opens a second Zed `Editor` over the same `language::Buffer`;
- a native Run tool window lists OpenIDEA run configurations, creates and saves
  user shell configurations, starts/stops a run, streams
  stdout/stderr/system output and shows the exit status;
- a native Find in Files panel uses the shared GPUI `TextInput` and the
  OpenIDEA `ops::search` streaming engine, with regex/case/whole-word toggles
  and clickable file/line/column results;
- the Find in Files panel also replaces every current match through
  `replace_in_files` (one undoable patch per file, open buffers updated,
  preserve-case option, cancellable backend request) and re-runs the search;
- a native Git tool window shows the branch, changed files, stage/unstage
  actions and a commit-message field backed by `ide-vcs`;
- the editor previews `#RGB`, `#RGBA`, `#RRGGBB` and `#RRGGBBAA` literals with
  semi-transparent background highlights, refreshed after edits;
- the editor now binds core Zed actions: caret movement/selection, backspace/
  delete, page up/down, undo/redo, copy/cut/paste/select-all, comment toggle,
  indent/outdent, completion (Ctrl-Space), go-to-definition/declaration
  (F12/Shift-F12); the full macOS default Zed keymap is also loaded from
  `keymaps/default-macos.json` (603 bindings), with RustIdea bindings applied
  afterwards so Quick Open/Command Palette/Symbol Search keep their own keys;
- a bottom-panel Terminal uses the Zed `terminal` crate's Alacritty/VT screen
  model with OpenIDEA's `ops::term` PTY ownership. The OpenIDEA PTY output is
  fed into `terminal::Terminal::write_output`; the workbench renders the
  screen with GPUI text runs, ANSI/256-colour mapping, inverse/bold/italic
  attributes, a cursor block/bar/underline and scrollback. Key events are
  translated with Zed's own `to_esc_str` mapping and written back to the
  OpenIDEA terminal session; paste honours bracketed-paste mode. It does not
  use the old text-only terminal stub or xterm.js; an `EntityInputHandler`
  marked-range path now forwards committed IME text and reports cursor bounds;
- the sidebar is now a compact multi-panel workbench: Project, Search, TODO,
  Dependencies, Coverage, Bookmarks, Git, Git Log, Services, GitHub, Heap,
  Profiler, Logcat, Scratch, Recent, Analysis, Outline, Problems, Database,
  HTTP, Remote, Notebook, MCP, Agents, Plugins, Tools and Settings;
- Outline calls OpenIDEA's LSP/tree-sitter `doc_outline` and navigates
  selection; Problems aggregates real backend diagnostics and navigates to
  file/line;
- Database lists OpenIDEA data sources, adds project SQLite sources through
  `ops::db::save_sources`, connects them, browses schemas and tables from
  `ops::db::schemas`/`tables`, opens DataGrip-style query console files through
  `ops::db::console_path`/`consoles`, creates a `SELECT` console for a table,
  and runs the statement at the caret from the active SQL console through
  `ops::db::execute`;
- HTTP parses the active `.http` file, lists requests, and executes the
  selected request through `ide_http::run`, showing status, headers and body;
- Remote lists OpenIDEA SSH configurations, opens a real SSH PTY-backed
  terminal through `ops::term::spawn_ssh`, and browses/previews remote files
  over SFTP (`SshManager::session` + `RemoteFs::list/read`); the preview opens
  a Zed-Editor modal and writes changes back through `RemoteFs::write`;
- Agents lists trusted OpenIDEA ACP providers (16 on this machine), subscribes
  to `AgentEvent`, shows task rows and an incremental conversation timeline
  (`Entry`/`Append`/`Truncated`), creates a local agent task from the prompt
  field through `ops::agent::create`, sends follow-ups through
  `ops::agent::prompt`, and cancels a selected task; the Agents panel also
  lists MCP status, and the dedicated MCP panel manages stdio, HTTP and SSE
  servers through
  `mcp_list`/`mcp_save`/`mcp_remove`/`mcp_set_enabled`; permission entries
  render their option buttons and cancel action through
  `ops::agent::respond`; subagent entries open their own timeline with a back
  action; AskUserQuestion/elicitation forms open a JSON-answer modal and reply
  through the same `respond` call; Tools lists
  language-server/debug-adapter status through `ops::tools::tools_list` and can
  install a selected tool (86 tools on this machine);
- Settings shows the effective OpenIDEA settings as a filterable, flattened
  list; boolean values are live toggles, and string/number/array/object values
  open a multiline Zed-Editor JSON modal, all written through
  `ops::project::set_settings` (verified below);
- Cmd-P opens a native quick-open overlay backed by
  `ops::nav::find_files`; typing re-queries the project index and clicking or
  submitting opens the selected file;
- Cmd-Shift-P opens a native command palette with 51 commands covering file
  save/close, splits, every sidebar and bottom panel, run/build/rebuild and
  backend refreshes; filtering is local and execution uses the same methods as
  the buttons;
- Cmd-Shift-O opens Go to Symbol, backed by `ops::nav::find_symbols`; results
  show container/path and navigate to the symbol's file/line;
- Alt-F7 / "Find usages at cursor" calls `ops::lsp::lsp_references` for the
  active document position and shows a clickable usage list;
- the bottom panel also has a Build tool window backed by
  `ops::build::build_model` / `ops::build::build_run`: it streams build
  `Started`, `Step`, `Output`, `Message` and `Progress` events and shows the
  final error/warning counts and duration;
- Plugins loads the installed plugin host in a blocking task and lists installed
  `PluginInfo` rows (the current RustIdea identifier has none installed); it
  also loads the official marketplace (139 entries), previews a downloaded
  package with signature/capability metadata (`Checkstyle 0.1.0`, 4
  capabilities, signed), renders each optional capability as a checkbox, and
  installs with the selected optional grants through `install_from_dir`;
- Debug scans `ops::debug::debug_attachable_processes` and lists debug-capable
  run configurations, starts attach sessions for Node/Python/.NET/Go/native
  processes through `ops::debug::debug_start`, lists/stop sessions and streams
  `DebugEvent::Output` into a console; a Breakpoint action toggles the line at
  the active editor cursor through `debug_set_breakpoints` and shows returned
  verification states. On `Stopped`, the panel fetches stack frames, scopes
  and variables, supports frame selection, and exposes Resume/Pause/Step-Over;
  an expression field evaluates in the selected frame through
  `ops::debug::debug_evaluate`. Java/Unity attach configs and condition/logpoint
  editing are still pending.
  No attachable process was visible to the scanner in the smoke environment;
- Notebook opens the active `.ipynb` through `ops::notebook::open`, renders
  code/markdown cells and stored outputs, and runs all code cells through
  `ops::notebook::execute`; each cell opens a Zed-Editor modal for multi-line
  source editing, and Commit uses `NotebookEdit::SetSource` followed by
  `ops::notebook::save`; the panel also inserts code/markdown cells, deletes
  cells and toggles code↔markdown through `NotebookEdit`; explicit Save and Run
  all are in the panel header, and per-cell Clear Output uses
  `NotebookEdit::SetOutputs` followed by save;
- buffer edits are sent incrementally to
  `ide_host::ops::doc::edit`; saves use `ide_host::ops::doc::save`; external
  `DocPatched` events and diagnostics are applied back to Zed buffers;
- Java is registered with a local Zed `LanguageConfig`, tree-sitter-java grammar
  and the OpenIDEA Java outline query, because Zed's built-in language set does
  not include Java.

Panel coverage at this commit:

| Surface | State | Backend path |
|---|---|---|
| Project tree | working | `ops::fs::read_dir` |
| Quick Open (Cmd-P) | working | `ops::nav::find_files` |
| Zed default macOS keymap | loaded (603 bindings) | `settings::KeymapFile` |
| Command Palette (Cmd-Shift-P) | working (51 commands) | local dispatch |
| Go to Symbol (Cmd-Shift-O) | working | `ops::nav::find_symbols` |
| Find Usages (Alt-F7) | working | `ops::lsp::lsp_references` |
| Find in Files | regex/case/whole-word + navigation | `ops::search` |
| Replace in Files | working (all current matches, preserve case) | `ops::search::replace_in_files` |
| TODO | working (comment scan/navigation/filters) | `ops::search` regex + comment context |
| Dependencies | Maven/Gradle/sbt modules and libraries with filter | `ops::build::build_model` + `ide-build` |
| Coverage | Run with Coverage for the selected run configuration, import lcov/JaCoCo/Go/SimpleCov reports, sort, file/directory percentages, project-tree badges, editor gutter highlights, file navigation | `ops::coverage::coverage_prepare/collect/import`, Zed gutter highlights |
| Bookmarks | F3 toggle at caret, editor gutter marker, navigable list, remove, per-project JSON persistence | local store + navigation, Zed gutter highlights |
| Git Log | 100 newest commits, message/hash filter, refs and merge markers | `ide-vcs` `Repo::log` |
| Services | Docker availability, containers/images/volumes/networks, start/stop/restart containers, container log viewer (tail 500), pull images, prune per-object, Compose start/stop/restart/down | `ide-docker` |
| GitHub | repository/login status, PR list by state, details/files/checks/review comments/timeline, unified diff, submit review, request a reviewer, conversation and inline diff comments, close/reopen, merge (merge/squash/rebase with two-click confirmation), open in browser | `ide-github`, `ide-vcs` |
| Heap | import `.hprof`, class table, class object pages, GC-root path, class-by-class two-snapshot comparison | `ops::heap::debug_heap_open/classes/dominators/path/compare` |
| Profiler | import Node `.cpuprofile` folders, method self/total weights, call tree, source navigation | `ide-profiler` |
| Logcat | adb devices, streaming threadtime log, level/text filters, Android Studio `tag:`/`level:`/`package:`/`message:` query grammar, persisted filter history, log export | `adb` process |
| Scratch | list/create/edit scratch files and Java/Python/Rust/JS/Markdown templates in the app data directory | local workspace + `ops::doc` |
| Recent Files | per-project MRU list persisted in app config | local store |
| Save a Copy | writes the active unsaved buffer text to a chosen path | `save_file_copy` |
| Analysis | Locate Duplicates + structural search + replace + Inspect Code | `ops::analysis` duplicates/structural/inspect, `Documents` edits |
| Git / VCS | working | `ide-vcs` + `ops::vcs` |
| Outline | working | `ops::doc::doc_outline` |
| Problems | working (list/navigation) | `IdeEvent::Diagnostics` |
| Database | working (source/connect/query) | `ops::db` |
| HTTP Client | working (file parse/run/response) plus unsaved multiline request editor with Send | `ide_http::run` |
| Remote Host | SSH terminal, SFTP browse/preview/edit | `ops::term::spawn_ssh`, `SshManager` |
| Notebook | viewer + cell edit/insert/delete/type + save/run all | `ops::notebook` |
| Agents | working (providers/tasks/timeline/prompt/cancel) | `ops::agent` |
| MCP | list/toggle/add/update/remove stdio/HTTP/SSE servers | `ops::agent::mcp_*` |
| Tools | list/install | `ops::tools` |
| Plugins | installed list + marketplace/preview/install + optional-capability consent | `PluginHost` |
| Settings | filter + boolean toggles + multiline JSON editor for scalars/lists/maps | `ops::project::settings` / `set_settings` |
| Debug | attach/session console, line breakpoints, stack/scopes/variables, evaluate, resume/pause/step (session runtime unverified) | `ops::debug` |
| Terminal | working VT screen | Zed `terminal` + `ops::term` |
| Run | detected/user configurations, create/edit/delete user configurations in `.openidea/run.toml`, start/stop, output/exit status | `ops::run` |
| Build | working | `ops::build` |

Headless split/search smoke (real native window, real OpenIDEA host, Java file):

```text
WORKBENCH_PROJECT_OPEN name=rustidea-java-workbench-completion trusted=true tree_rows=3 initial_doc=true
WORKBENCH_DOC_OPEN doc=1 path=.../App.java language=java tabs=1
WORKBENCH_SMOKE status=jdtls: Ready language=true editors=2 run_status=2 run configurations run_output="" search_status=1 files, 6 matches search_files=1 text_prefix="// rustidea search smoke"
```

Run smoke on the same Java project:

```text
WORKBENCH_PROJECT_OPEN name=rustidea-java-workbench-smoke trusted=true tree_rows=3 initial_doc=true
WORKBENCH_DOC_OPEN doc=1 path=.../App.java language=java tabs=1
WORKBENCH_SMOKE status=jdtls: Ready language=true editors=2 run_status=Exited with Some(0) run_output="Process finished with exit code 0" text_prefix="// rustidea run smoke"
```

Run configuration save smoke:

```text
run_status=Saved run configuration Smoke Config run_configs=1
.openidea/run.toml: id="user:1795665278917" name="Smoke Config" command="echo" args=["hello"] env={SMOKE="1"}
```

Run configuration edit/delete smoke:

```text
run_status=Saved run configuration Edited run_configs=1
run.toml name="Edited"
run_status=Deleted run configuration DeleteMe run_configs=0
```

Git smoke in a temporary `main` repository with one modified Java file:

```text
WORKBENCH_PROJECT_OPEN name=rustidea-git-smoke trusted=false tree_rows=3 initial_doc=true
WORKBENCH_DOC_OPEN doc=1 path=.../App.java language=java tabs=1
WORKBENCH_SMOKE status=jdtls: Ready language=true editors=1 ... git_branch=main git_files=1 text_prefix="// rustidea git smoke"
```

The workbench completion smoke produced `WORKBENCH_COMPLETION count=1` in a
separate run. It is a real OpenIDEA jdtls completion request made through
the workbench's `DocumentLsp`; it is not yet an onscreen menu acceptance test.
`language=true` proves the buffer has an attached Zed tree-sitter language
(including the custom Java registration), not merely an OpenIDEA language id.
`editors=2` is the split view: two Zed editors over the same document buffer.
The search line proves the workbench Find in Files panel streamed results from
the OpenIDEA search engine. A separate run smoke produced
`run_status=Exited with Some(0)` and the final Maven output line, proving the
Run panel used the OpenIDEA Run service rather than a text-only stub.

Terminal smoke in the same Java project (real OpenIDEA PTY, real shell):

```text
WORKBENCH_TERMINAL_SPAWN id=TerminalId#1 title=zsh pid=Some(73544)
WORKBENCH_SMOKE ... terminal_status=zsh · pid Some(73544)
terminal="/private/tmp/rustidea-java-workbench-smoke $ | WORKBENCH_TERMINAL_OK
| /private/tmp/rustidea-java-workbench-smoke $ printf 'WORKBENCH_TERMINAL_OK\n'"
```

This proves the OpenIDEA PTY stream reached the Zed VT grid. It is not an
onscreen key-by-key or IME acceptance test.

HTTP smoke against a local `http.server`:

```text
http_status=200 OK · 26 bytes · 1.7 ms
http="HTTP/1.0 200 OK"
```

HTTP editor smoke against the same local server:

```text
http_status=1 requests
http_editor=200 OK · 26 bytes · 8.2 ms
http="HTTP/1.0 200 OK"
```

Outline smoke on the Java project:

```text
outline_status=2 top-level items outline_items=5
```

Agents/Tools smoke:

```text
agents=16 tools=86
```

Notebook smoke (two-cell `.ipynb`):

```text
notebook_cells=2
```

Notebook clear-output smoke:

```text
notebook_cells=2 notebook_outputs=0
```

The saved `.ipynb` had the executed cell's `outputs` emptied and
`execution_count` reset to null.

Quick-open smoke:

```text
quick_open=1 quick_open_status=1 files
```

Command-palette smoke:

```text
commands=1 command_first="Show Problems" sidebar=Problems
```

Keymap smoke:

```text
WORKBENCH_KEYMAP bindings=603
```

Breakpoint smoke:

```text
debug_status=Breakpoint toggled at line 1 breakpoints=1
```

Go-to-symbol smoke:

```text
symbols=1 symbol_first="App · src/main/java/com/example/App.java"
```

Find-usages smoke on a Java helper:

```text
references=1 references_status=1 usages
```

Settings smoke in an isolated config root:

```text
settings_rows=466 show_whitespace=true
settings.toml: showWhitespace = true
```

String-setting smoke in an isolated config root:

```text
settings_rows=466 font_family="Menlo"
settings.toml: fontFamily = "Menlo"
```

Structured JSON setting smoke (`keymapOverrides.smokeAction = ["Meta+Shift+9"]`):

```text
settings_rows=467 settings_keymap_overrides=1
```

Notebook cell-edit smoke (fresh `.ipynb`, committed then saved):

```text
notebook_cells=1 notebook_first="# rustidea notebook edit"
disk: source = ["# rustidea notebook edit\n", "print('NB EDIT OK')\n"]
```

Notebook insert-cell smoke:

```text
notebook_cells=2
disk: cells=2 types=["code", "markdown"]
```

MCP round-trip smoke in an isolated config/data root (add `/bin/echo` server,
then remove it):

```text
WORKBENCH_MCP_ROUNDTRIP added=1 removed=0 status=New MCP server
```

MCP HTTP/SSE round-trip smoke (add a URL-backed server for each transport, then
remove it):

```text
WORKBENCH_MCP_TRANSPORT name=smoke-http transport=http url=Some("https://mcp.example.test/mcp") command=None
WORKBENCH_MCP_TRANSPORT name=smoke-sse transport=sse url=Some("https://mcp.example.test/mcp") command=None
WORKBENCH_MCP_TRANSPORT remaining=0
```

TODO smoke on a Java project containing two comment TODOs, one TODO inside a
string and one outside a comment:

```text
todo=2 todo_status=2 TODO items · 1 files scanned
todo_first="src/main/java/com/example/App.java:4"
```

Replace-in-files smoke (`alpha` → `omega` in a two-line text file):

```text
WORKBENCH_REPLACE replacements=3 files=1 failed=0
disk: ["omega omega\n", "beta omega\n"]
```

Dependencies smoke on a Maven project with one direct JUnit dependency:

```text
dep_modules=1 dep_libraries=1 dep_status=1 modules · 1 dependencies
dep_first_module="dependencies-smoke"
dep_first_library="org.junit.jupiter:junit-jupiter-api:5.10.0"
```

Coverage import smoke with an lcov tracefile containing one file, three lines
and two branches:

```text
coverage_files=1 coverage_status=lcov · 1 files · 66% (2/3) lines
coverage_first="src/main/java/com/example/App.java" coverage_badges=7 coverage_gutter=2
```

Run with Coverage smoke on a Cargo project through a fake `cargo llvm-cov`
CLI (the report is written by the fake tool, then collected through OpenIDEA):

```text
coverage_files=1 coverage_status=cargo-llvm-cov · 1 files · 50% (1/2) lines
coverage_first="src/main.rs" coverage_badges=3 coverage_gutter=2 coverage_run_busy=false
```

Bookmark smoke (F3 on line 1, then a fresh process reloading the same config):

```text
bookmarks=1 bookmark_first="/private/tmp/rustidea-bookmark-smoke/src/main.rs:1" bookmark_gutter=1
bookmarks.json text="fn main() {"
```

Color-preview smoke on a CSS file with four hex literals:

```text
color_preview=4
```

Git Log smoke on this repository:

```text
git_log=74 git_log_status=74 commits
git_log_first="afaf8374 experiment: add persistent bookmark tool window"
```

Docker Services smoke through a fake `docker` CLI:

```text
docker_containers=1 docker_images=1
docker_status=Docker 27.1.1 · engine 27.1.1 · default
docker_first="shop-db-1 · postgres:16 · running"
```

Docker logs smoke through the same fake CLI:

```text
docker_logs=2 docker_logs_status=2 line(s) · tail 500
```

Docker pull smoke (`postgres:17`):

```text
docker_status=Status: Downloaded newer image for postgres:17
```

Docker prune smoke (`image prune --all --force`):

```text
docker_status=untagged: postgres@sha256:deadbeef
```

Docker Compose smoke (`compose --project-name shop start`):

```text
docker_status=Compose output: compose --project-name shop start
```

GitHub Pull Requests smoke through a fake `gh` CLI:

```text
github_prs=1 github_status=octo-org/demo · octocat · 1 pull requests
github_first="#42 Add the native GitHub panel"
```

GitHub PR detail smoke:

```text
github_detail=#14768 Refreshable tokens: bump cli/oauth to v1.3.0 and fix `gh status` column width · 2 files · 5 checks
github_comments=3 github_timeline=2 github_detail_status=2 files · 5 checks
```

GitHub review submission smoke through the fake `gh` CLI (`pr review --comment`):

```text
github_review=Review submitted for #14768
pr review 14768 --comment --body Smoke review comment
```

GitHub diff smoke on a local two-commit repository through the fake `gh` CLI:

```text
github_diff_files=1 github_diff_lines=6
github_diff_status=1 files · 6 diff lines · f326112 → dd76151
```

GitHub inline review comment smoke through the fake `gh` API:

```text
github_inline=Inline comment added to sample.txt:2
{"commit_id":"dd761512c6af29127d7a61d7a1f047d3b56d73c2","path":"sample.txt","line":2,"side":"RIGHT","body":"Inline smoke comment"}
```

GitHub close/reopen smoke through the fake `gh` CLI:

```text
github_review=Closed #42
pr close 42
github_review=Reopened #42
pr reopen 42
```

GitHub merge smoke through the fake `gh` CLI:

```text
github_review=Merged #42
pr merge 42 --squash
```

GitHub request-reviewer smoke through the fake `gh` CLI:

```text
github_review=Review requested from @octocat
pr edit 42 --add-reviewer octocat
```

The `@` prefix was stripped before invoking `gh`; empty or whitespace-containing
logins are rejected by the `ide-github` unit test.

Heap snapshot smoke with a generated `ide-heap` sample:

```text
heap_classes=6 heap_objects=8
heap_status=rustidea-heap-sample.hprof · 8 objects · 7 classes
heap_first="java.lang.Class · 7"
```

Heap comparison smoke (same sample opened as base and current):

```text
heap_classes=6 heap_status=rustidea-heap-sample2.hprof · 8 objects · 7 classes
heap_compare=6 heap_compare_status=6 classes · rustidea-heap-sample.hprof → rustidea-heap-sample2.hprof
```

Heap class object page smoke:

```text
heap_object_rows=7 heap_objects_status=7 objects
heap_object_first="java.lang.Class 0x100000001"
```

Node CPU profile smoke with one `.cpuprofile`:

```text
profile_methods=2 profile_status=node --cpu-prof · 3 frames · 2 methods · 3.5 ms
profile_first="work"
```

Call-tree smoke on the same profile:

```text
profile_methods=2 profile_tree=2
```

Logcat smoke through a fake `adb`:

```text
logcat_lines=2 logcat_status=logcat stopped · 2 lines
logcat_first="I ActivityManager: Start proc com.example"
```

Logcat query + export smoke (`tag:AndroidRuntime level:E`):

```text
logcat_lines=2 logcat_matches=1
logcat_export="…/logcat-export-1795659359.log"
logcat_status=Exported 1 line(s) to …/logcat-export-1795659359.log
```

The exported file contained only `E AndroidRuntime: java.lang.NullPointerException: boom`.

Scratch file smoke:

```text
scratch_files=1 scratch_status=1 scratch files
scratch_first="notes.md"
```

Scratch template smoke (Java):

```text
scratch_files=2 scratch_status=2 scratch files
scratch_first="scratch-1795653375.java"
```

The new file contained the Java template (`public class Scratch { … }`).

Recent files smoke after opening one file:

```text
recent_files=1 recent_status=1 recent files
recent_first="/private/tmp/rustidea-bookmark-smoke/src/main.rs"
```

Save-a-copy smoke:

```text
WORKBENCH_SAVE_COPY ok path=/tmp/rustidea-saveas-smoke/copy.txt
disk: copy.txt contains the active buffer text
```

Locate Duplicates smoke on two Java files with the same method body:

```text
duplicate_groups=1 duplicate_status=1 duplicate groups · 2 files · 374 tokens · 1 ms
duplicate_first="src/main/java/com/example/DupA.java:1"
```

Structural search smoke (Java pattern `return $A + $B;`):

```text
structural_matches=2 structural_status=2 matches in 2 files · 1 ms
structural_first="src/main/java/com/example/DupA.java:34"
```

Structural replace smoke on two Java files (pattern `return $A + $B;`,
replacement `return 0;`):

```text
structural_matches=0 structural_status=0 matches in 0 files · 0 ms
structural_apply=Applied 2 replacements in 2 files
```

Both files contained `return 0;` after the run and the re-search returned zero
matches, proving the edits went through OpenIDEA `Documents` and were saved.

Inspect Code smoke on a project with no language-server files (proves the scan
event/summary path, not a diagnostic result):

```text
inspect_problems=0 inspect_status=0 files · 0 problems · 2 skipped · 0 failed · 8 ms · scanned 0
```

Database browser and console smoke on a SQLite fixture with `users` (3 rows)
and `active_users`:

```text
WORKBENCH_DB_UI schemas=1 tables=2 status="2 tables and views"
console="…/consoles/db/smoke-sqlite/console.sql" output="["
db_status=3 row(s), 0.27 ms
```

The console file was opened in the Zed Editor and contained
`SELECT name FROM users ORDER BY id;`; the active path in the smoke report was
that console file. The query returned three rows through
`ops::db::execute`. This verifies the schema/table lists, console creation and
query path, not a visual result-grid acceptance.

Add-source smoke on a fresh project with no `.openidea/data-sources.toml`:

```text
WORKBENCH_DB_UI sources=1 schemas=0 tables=0
status="Connect to browse schemas and tables"
```

The workbench wrote a `[[data-source]]` entry with
`driver = "sqlite"` and an absolute `file = ".../app.db"`. A second run
connected that created source, listed one schema and one table, and ran
`SELECT label FROM items ORDER BY id;` from its console:

```text
WORKBENCH_DB_UI sources=1 schemas=1 tables=1 status="1 tables and views"
db_status=2 row(s), 0.19 ms
```

Build smoke on the Maven project:

```text
build_status=rustidea-java-workbench-smoke · 0 errors · 0 warnings · 0.7 s
build="Process finished with exit code 0"
```

The first line of the Java file on disk was the inserted marker after the
process exited, so the multi-document workbench path exercised:

```text
project open -> tree load -> OpenIDEA doc open -> Zed Buffer edit
-> OpenIDEA document sync -> OpenIDEA save -> disk
```

The macOS window exists and `CGWindowListCopyWindowInfo` reports it onscreen
(`RustIdea Zed`, 1280x853). Window-specific `screencapture` captures the GPUI
background but not the Metal-rendered text layer, so this screenshot is not
used as visual text/IME acceptance.

Workbench tests:

```text
documents/project/sync/host/build/coverage/todo/bookmarks/logcat/scratch/recent/pr_diff/color_preview: 62 passed; 0 failed
ide-github reviewer-login: 1 passed; 0 failed
```

## Not yet done

- License confirmation and the repository license change to
  GPL-3.0-or-later. No GPL release is produced from this branch.
- Replacing the existing RustIdea `Editor`/`DocumentBuffer` workbench entry with
  the Zed entity across tabs and splits.
- Real window keyboard, Chinese IME, clipboard and visual verification on an
  accessible/onscreen macOS Space. The current agent session creates windows
  offscreen even with an unlocked console.
- External-change reload/conflict handling in the Zed Buffer path (the POC only
  ignores backend patches for the document it owns).
- Process-tree memory/performance comparison and three-platform CI.
- Plugin live UI contributions, conditioned breakpoints/logpoints and Java/
  Unity attach, richer notebook output editing (clear is implemented), live
  remote-write verification (UI path exists), structured question/form widgets
  and live subagent/form verification
  are not yet integrated. MCP add/update/remove is wired for stdio, HTTP and
  SSE transports. Settings edits booleans and JSON scalars/lists/maps through a
  multiline editor; dedicated widgets for complex schemas are missing. HTTP has
  an unsaved request editor with Send; Database now creates SQLite sources and
  has schema/table browsing and console-file source editing, but typed
  connection editors for other drivers, result grids and saved console history
  are missing. Coverage imports existing lcov, JaCoCo,
  Go and SimpleCov reports; project-tree badges show file and directory
  coverage, and the editor applies covered/partial/uncovered gutter highlights.
  The coverage panel can launch the selected run configuration with coverage
  and collect its report; the gutter colors are smoke-verified through the Zed
  highlight API rather than a visual screenshot. Bookmarks have a navigable panel and an editor gutter marker
  (also API-smoke-verified, not visually captured). Services lists Docker objects, can
  start/stop/restart containers, shows logs, pulls images, prunes objects and
  runs Compose lifecycle actions, but custom connections are missing. GitHub lists and filters pull
  requests and shows details/checks/review comments/timeline and a unified
  diff, submits reviews, requests reviewers, conversation comments, inline
  diff comments, closes/reopens and merges pull requests (two-click
  confirmation); review dismissals are missing. Heap
  imports and lists
  classes and object pages with GC-root paths and compares two snapshots
  class-by-class, but capture from a live JVM and richer instance/field detail are missing. Profiler
  imports Node CPU profiles, lists methods and shows a call tree, but
  recording new profiles and Python/JVM/perf formats are missing. Logcat lists devices, streams, filters threadtime output,
  supports the Android Studio query grammar, persists filter history and exports logs. The
  Scratch panel lists and creates files with Java/Python/Rust/JS/Markdown
  template shortcuts; a full language chooser is still missing. Save As currently saves a copy: the OpenIDEA document
  layer only supports re-keying untitled buffers, not renaming an existing file.
  Color preview covers hex literals only; `rgb()`/`hsl()` literals and a color
  picker are not implemented.
  The Run panel can create/edit/delete shell configurations; type-specific
  configuration dialogs and templates are not implemented.
  Analysis runs Locate Duplicates, structural search, structural replacement
  and Inspect Code. Replace uses the OpenIDEA document layer and saves files
  that were opened transiently; edits in already-open buffers stay dirty until
  the user saves. A Java diagnostic-producing Inspect Code run is not yet
  smoke-verified. The Terminal now has a real VT
  screen
  and OpenIDEA PTY, but its mouse selection, hyperlinks, IME composition and
  resize path are not yet verified or complete (the IME input handler is
  wired, but locked-screen visual/IME verification is still unavailable).

This document is a POC report, not a claim of OpenIdea parity.
