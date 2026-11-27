# RustIdea Zed tg-tenant-fork acceptance

Date: 2026-11-27

Status: backend and workbench integration acceptance passed for the checks
listed below. This is not a completed P4 or a claim of full OpenIDEA parity.
The macOS console was locked (`CGSSessionScreenIsLocked=Yes`) and the
AgentSpace worker was not running, so real keyboard, IME, clipboard,
screenshots and visual menu acceptance remain unverified.

## Environment

- macOS 27.2.1 on Apple Silicon.
- RustIdea `main` at `9ff2a2de9b9f99e1855e32b0b5675f93d4a40d78`
  for the original run; the Save As re-key evidence below was added on the
  follow-up working tree before the `v0.3.3-zed-preview.1` release.
- Project: `/Users/guofeng/Code/cscec3b/tg-tenant-fork` at
  `7f92ab5317194fb3615f3b2be54bba54e8c7b8a7`.
- The test used a detached Git worktree so the project's uncommitted
  `tg-tenant-server/src/main/resources/bootstrap.yml` change was left alone.
- Primary file: `tg-tenant-server/src/main/java/com/cscec3b/iti/tg/user/security/IpUtils.java`.
- Runner: `experiments/zed-poc/target/debug/rustidea-zed-workbench`,
  using the real OpenIDEA Host, Java language server, Maven model, PTY,
  document service and file watcher.

## Results

| P4 check | Result | Evidence |
|---|---|---|
| Open project | pass | `WORKBENCH_PROJECT_OPEN trusted=true tree_rows=17 initial_doc=true` |
| Module model | pass | Dependencies reported `4 modules · 76 dependencies`; first module `tg-tenant`, first library `cn.dev33:sa-token-core:1.33.0` |
| Project tree | partial | 17 top-level entries loaded; lazy child expansion was not driven visually |
| Open Java file | pass | `WORKBENCH_DOC_OPEN ... language=java tabs=1`; the Zed buffer had `language=true` |
| Edit and save | pass | Marker was inserted through the Zed buffer and saved through OpenIDEA; `status=Saved through OpenIDEA` |
| Save As re-keys the open document | pass at backend and smoke level | `WORKBENCH_SAVE_AS ok`; `WORKBENCH_TABS count=1` with `active_path=.../IpUtilsSavedAs.java`; the source file was unchanged and both files had the same SHA-256 |
| Clean reload after disk change | partial | The external-change path was exercised; a clean automatic reload in an interactive window was not |
| Maven compile | pass | Build panel: `0 errors · 0 warnings · 27.0 s`, final line `Process finished with exit code 0` |
| Maven tests | pass | Direct run reported `Tests run: 2, Failures: 0, Errors: 0` and `BUILD SUCCESS`; the embedded terminal ran the same command and printed `WORKBENCH_MAVEN_TEST_OK` |
| LSP completion | pass at backend level | `WORKBENCH_COMPLETION count=2` from the real JDT.LS request |
| LSP hover | pass at backend level | `WORKBENCH_LSP hover_chars=265` |
| LSP go to definition | pass at backend level | `definitions=1` at `IpUtils.java` line 19, column 15 |
| LSP rename | pass at backend level | `WORKBENCH_LSP_RENAME ... status=ok`; all 15 `ipAddress` occurrences became `clientIp` in the detached copy |
| Multi-file rename through the UI command path | pass | `WORKBENCH_RENAME_READY references=2 min=2`; `WORKBENCH_RENAME files=2 paths=2 new_name=getClientIp status=ok`; two tabs were open, both on-disk files changed and both Zed buffers contained the new name (`text_expect_docs=2`) |
| Multiple tabs | pass | Second Java file opened with `tabs=2`; final `WORKBENCH_TABS count=2` |
| Split | pass at entity level | `editors=2` after `RUSTIDEA_ZED_SMOKE_SPLIT=1`; visual split layout remains unverified |
| Terminal | pass | Real zsh PTY spawned, printed `WORKBENCH_TERMINAL_OK`, and its final text contained `WORKBENCH_MAVEN_TEST_OK` |
| External file conflict | pass | `WORKBENCH_EXTERNAL conflict="File changed on disk since it was opened"` |
| Chinese IME | blocked | Requires an unlocked interactive macOS session |
| Clipboard, undo and redo | blocked | Requires an unlocked interactive macOS session |
| Screenshot | blocked | `screencapture` is unavailable while the console is locked |

The workbench smoke tail now waits for a running build to finish, stops the
PTY, shuts down the OpenIDEA Host and exits cleanly. The build run above
returned `exit=0` without the Tokio shutdown panic seen in earlier smoke
attempts.

## Preliminary resource snapshot

A 25-second process-tree snapshot on the same project was taken. It is not a
workload-equivalent performance comparison: the stable 0.2.0 app did not start
a Java language server in this headless launch, while the Zed workbench did.

| Process | RSS | CPU |
|---|---:|---:|
| Zed Preview UI (`rustidea-zed-workbench`, release) | 161.1 MB | 0.0% |
| JDT.LS child started by the workbench | 1185.5 MB | 0.1% |
| Zed Preview process tree | 1346.6 MB | 0.1% |
| Stable RustIdea 0.2.0 UI | 115.4 MB | 0.0% |

These numbers are only a preliminary regression snapshot, not proof that the
Zed path uses more or less memory under equivalent language-server workloads.

## Repetition

The acceptance knobs added to the workbench are:

- `RUSTIDEA_ZED_SMOKE_MARKER=<text>`: activates the scripted smoke path. It
  is required even when the marker edit itself is disabled.
- `RUSTIDEA_ZED_SMOKE_NO_MARKER=1`: skip inserting the marker edit; used by
  read-only and Save As checks.
- `RUSTIDEA_ZED_SMOKE_WAIT_SECS`: maximum wait, used by the build adaptive
  wait and by fixed-delay stages.
- `RUSTIDEA_ZED_SMOKE_SAVE_AS=<path>`: Save As the active document to a path,
  reporting `WORKBENCH_SAVE_AS ok` after the OpenIDEA document is re-keyed.
- `RUSTIDEA_ZED_SMOKE_LSP_POS` / `RUSTIDEA_ZED_SMOKE_LSP_RENAME`: request
  hover, definition and rename at a document position.
- `RUSTIDEA_ZED_SMOKE_RENAME` / `..._POS` / `..._MIN_REFS`: run the workbench
  rename command path and wait until at least the requested number of
  references is visible before applying it.
- `RUSTIDEA_ZED_SMOKE_EXPECT_TEXT`: report whether the active Zed buffer
  contains a string after a workspace edit, and how many open Zed buffers
  contain it.
- `RUSTIDEA_ZED_SMOKE_ACTIVE_TAB`: select a tab by index before the next
  scripted action.
- `RUSTIDEA_ZED_SMOKE_SECOND_FILE`: open a second document and report the tab
  count.
- `RUSTIDEA_ZED_SMOKE_TERMINAL_COMMAND`: run a command in the real PTY instead
  of the fixed terminal marker.

## Remaining gates

- Replace the stable RustIdea editor entry with the Zed workbench; the Preview
  is still an isolated binary and the stable entry remains the old editor.
- Unlock an interactive macOS session for keyboard, Chinese IME, clipboard,
  undo/redo, visual hover/completion menus, split layout and screenshots.
- Verify a clean external-change reload, not only the conflict banner.
- Run the full Maven test suite or the project's approved subset; only one
  standalone test class was executed during this acceptance.
- Verify class or package renames that move files; the current multi-file
  rename evidence edits existing files across modules.
- Close the remaining parity items in `RUSTIDEA_FEATURE_PARITY.md`, including
  plugin live UI, structured settings/run editors and the other listed gaps.
