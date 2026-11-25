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
- a native Run tool window lists OpenIDEA run configurations, starts/stops a
  run, streams stdout/stderr/system output and shows the exit status;
- a native Find in Files panel uses the shared GPUI `TextInput` and the
  OpenIDEA `ops::search` streaming engine, with clickable file/line results;
- a native Git tool window shows the branch, changed files, stage/unstage
  actions and a commit-message field backed by `ide-vcs`;
- a bottom-panel Terminal uses the Zed `terminal` crate's Alacritty/VT screen
  model with OpenIDEA's `ops::term` PTY ownership. The OpenIDEA PTY output is
  fed into `terminal::Terminal::write_output`; the workbench renders the
  screen with GPUI text runs, ANSI/256-colour mapping, inverse/bold/italic
  attributes, a cursor block/bar/underline and scrollback. Key events are
  translated with Zed's own `to_esc_str` mapping and written back to the
  OpenIDEA terminal session; paste honours bracketed-paste mode. It does not
  use the old text-only terminal stub or xterm.js; an `EntityInputHandler`
  marked-range path now forwards committed IME text and reports cursor bounds;
- the sidebar is now a compact multi-panel workbench: Project, Search, Git,
  Outline, Problems, Database, HTTP, Remote, Agents, Plugins, Tools and
  Settings;
- Outline calls OpenIDEA's LSP/tree-sitter `doc_outline` and navigates
  selection; Problems aggregates real backend diagnostics and navigates to
  file/line;
- Database lists OpenIDEA data sources, connects them, and runs the statement
  at the caret from the active SQL document through `ops::db::execute`;
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
  lists MCP status, and the dedicated MCP panel manages stdio servers through
  `mcp_list`/`mcp_save`/`mcp_remove`/`mcp_set_enabled`; permission entries
  render their option buttons and cancel action through
  `ops::agent::respond`; subagent entries open their own timeline with a back
  action; AskUserQuestion/elicitation forms open a JSON-answer modal and reply
  through the same `respond` call; Tools lists
  language-server/debug-adapter status through `ops::tools::tools_list` and can
  install a selected tool (86 tools on this machine);
- Settings shows the effective OpenIDEA settings as a filterable, flattened
  list; boolean values are live toggles, and string/number/other JSON values
  open an inline value editor, all written through
  `ops::project::set_settings` (verified below);
- Cmd-P opens a native quick-open overlay backed by
  `ops::nav::find_files`; typing re-queries the project index and clicking or
  submitting opens the selected file;
- Cmd-Shift-P opens a native command palette with 33 commands covering file
  save/close, splits, every sidebar and bottom panel, run/build/rebuild and
  backend refreshes; filtering is local and execution uses the same methods as
  the buttons;
- Cmd-Shift-O opens Go to Symbol, backed by `ops::nav::find_symbols`; results
  show container/path and navigate to the symbol's file/line;
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
  run configurations. The panel is a process/browse surface; starting a session
  and the variables/stack/frames UI are still pending;
- Notebook opens the active `.ipynb` through `ops::notebook::open`, renders
  code/markdown cells and stored outputs, and runs all code cells through
  `ops::notebook::execute`; each cell opens a Zed-Editor modal for multi-line
  source editing, and Commit uses `NotebookEdit::SetSource` followed by
  `ops::notebook::save`; explicit Save and Run all are in the panel header;
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
| Command Palette (Cmd-Shift-P) | working (33 commands) | local dispatch |
| Go to Symbol (Cmd-Shift-O) | working | `ops::nav::find_symbols` |
| Find in Files | working | `ops::search` |
| Git / VCS | working | `ide-vcs` + `ops::vcs` |
| Outline | working | `ops::doc::doc_outline` |
| Problems | working (list/navigation) | `IdeEvent::Diagnostics` |
| Database | working (source/connect/query) | `ops::db` |
| HTTP Client | working (parse/run/response) | `ide_http::run` |
| Remote Host | SSH terminal, SFTP browse/preview/edit | `ops::term::spawn_ssh`, `SshManager` |
| Notebook | viewer + cell editor + save/run all | `ops::notebook` |
| Agents | working (providers/tasks/timeline/prompt/cancel) | `ops::agent` |
| MCP | list/toggle/add/update/remove stdio servers | `ops::agent::mcp_*` |
| Tools | list/install | `ops::tools` |
| Plugins | installed list + marketplace/preview/install + optional-capability consent | `PluginHost` |
| Settings | filter + boolean toggles + JSON value editor | `ops::project::settings` / `set_settings` |
| Debug | attachable-process browse only | `ops::debug::debug_attachable_processes` |
| Terminal | working VT screen | Zed `terminal` + `ops::term` |
| Run | working | `ops::run` |
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

Quick-open smoke:

```text
quick_open=1 quick_open_status=1 files
```

Command-palette smoke:

```text
commands=1 command_first="Show Problems" sidebar=Problems
```

Go-to-symbol smoke:

```text
symbols=1 symbol_first="App · src/main/java/com/example/App.java"
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

Notebook cell-edit smoke (fresh `.ipynb`, committed then saved):

```text
notebook_cells=1 notebook_first="# rustidea notebook edit"
disk: source = ["# rustidea notebook edit\n", "print('NB EDIT OK')\n"]
```

MCP round-trip smoke in an isolated config/data root (add `/bin/echo` server,
then remove it):

```text
WORKBENCH_MCP_ROUNDTRIP added=1 removed=0 status=New stdio MCP server
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
documents/project/sync/host: 24 passed; 0 failed
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
- Plugin live UI contributions, full debugger
  session UI, notebook insert/delete/rich-output editing, live remote-write
  verification (UI path exists), structured question/form widgets and live
  subagent/form verification
  are not yet integrated. MCP add/update/remove is wired for stdio servers; HTTP/SSE editing
  still lives in settings. Settings edits booleans and JSON scalars; structured
  list/map editors are missing. Database/HTTP have working execution paths but no
  source/request editors of their own. The Terminal now has a real VT screen
  and OpenIDEA PTY, but its mouse selection, hyperlinks, IME composition and
  resize path are not yet verified or complete (the IME input handler is
  wired, but locked-screen visual/IME verification is still unavailable).

This document is a POC report, not a claim of OpenIdea parity.
