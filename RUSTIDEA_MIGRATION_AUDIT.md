# OpenIDEA migration audit

## Audited snapshot

The upstream reference snapshot is `l0ng-ai/openidea` commit
`50cd478773e3641bc1b636dea2fe29c6868b07d7`, version 0.1.50. It contains
47 crate directories, 49 feature directories, 1,098 Rust files and 3,552
TS/TSX/CSS files as counted from its Git tree. Upstream changes frequently;
these counts describe this snapshot, not the current head.

Upstream targets macOS, Windows and Linux. Its desktop entry point is Tauri;
the frontend uses React and CodeMirror. The desktop command catalogue derives
project-host adapters from `ide-host`, while also retaining desktop-specific
commands. Moving the UI requires replacing both the IPC transport and frontend
business logic, not merely calling backend crates.

## Editor and document

`src/editor/core/model/document.ts` owns one `DocModel` per open backend
document. Tabs and auxiliary attachments refer to it. `slot.ts` holds each
tab's selection, fold, history and scroll state and keeps at most eight detached
views; older views are destroyed while state is retained. `attachment.ts`
connects other editable views and may share history with a primary slot.

`sync/DocSync.ts` tracks confirmed text, one in-flight edit and a local buffer.
It rebases remote changes and retries stale versions; a hard resync can discard
unacknowledged changes. `core/backend.ts` exposes versioned document operations
through Tauri commands. `ide-text` uses Ropey, UTF-16 offsets and normalized
line endings. The original ending is restored on save.

Language and editor contributions in `core/cm/setup.ts`, `languages.ts` and
`src/editor/api.ts` include folds, highlighting, selection matches, indentation,
sticky scroll and contributed extensions. Heavy extensions are disabled above
8,000,000 characters. LSP and syntax integration require their existing host
services and new native UI adapters.

The older local checkout `048b87e4aea38924e2adf5f5f8ca4806640cb401` also
registers contributions for Maven/version-catalog navigation and completion,
LSP markers, Docker, coverage, run gutters, Markdown, macros, templates,
GraphQL, search, databases, TODO, debugger, VCS and plugins. These are examples
of frontend behavior that must be audited individually; they are not evidence
that those features already work in the native editor. This checkout differs
from the snapshot above and should not be treated as its exhaustive inventory.

## Evidence gaps

The snapshot's full build and tests were not run for this audit. Its CI runs
observed at the time were cancelled; they do not prove a passing build. The
feature inventory and backend coupling need continued item-by-item verification.
Original performance measurements remain uncollected; see the baseline document.
