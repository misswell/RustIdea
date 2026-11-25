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
  completion menu.
- `OpenIdeaSemanticsProvider`, installed with
  `Editor::set_semantics_provider`, which maps `ide_host::ops::lsp::lsp_hover`
  results into Zed's hover blocks and same-file `lsp_goto` results into Zed
  `LocationLink`s. Cross-file definitions, highlights, rename, inlay hints and
  semantic tokens are explicit empty stubs in this POC.
- An OpenIdea event pump that converts `IdeEvent::Diagnostics` into a Zed
  `DiagnosticSet` and calls `Buffer::update_diagnostics`.
- Two `Editor` views can be constructed over the same `language::Buffer`
  (`Editor::for_buffer`), so the same-file multi-view requirement is structurally
  satisfied.

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
macOS console is currently screen-locked, so no screenshot or keyboard/IME
interaction is claimed.

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

Cross-file definitions are currently dropped because the POC only owns one
Zed buffer.

Diagnostics bridge test with an intentional TypeScript type error:

```text
POC_DIAGNOSTICS source=typescript-language-server count=2
```

Two OpenIdea diagnostics were converted and applied to the Zed buffer. Visual
underline rendering still needs GUI inspection; the POC currently maps every
source to one synthetic language-server id, so multiple diagnostic producers
will overwrite rather than merge until the final DiagnosticsBridge keeps a
source-to-id map.

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

## Not yet done

- License confirmation and the repository license change to
  GPL-3.0-or-later. No GPL release is produced from this branch.
- Definition/highlights/rename provider wiring and the diagnostics bridge.
  Cross-file definitions and rename are still stubs; hover, same-file
  definitions and completion are wired.
- Completion documentation, `additionalTextEdits`, commit characters and
  completion resolve are not mapped yet.
- Multi-source diagnostic merging (the POC uses one synthetic server id).
- Replacing the existing RustIdea `Editor`/`DocumentBuffer` workbench entry with
  the Zed entity across tabs and splits.
- Real window keyboard, Chinese IME, clipboard and visual verification on an
  unlocked macOS session.
- External-change reload/conflict handling in the Zed Buffer path (the POC only
  ignores backend patches for the document it owns).
- Process-tree memory/performance comparison and three-platform CI.

This document is a POC report, not a claim of OpenIdea parity.
