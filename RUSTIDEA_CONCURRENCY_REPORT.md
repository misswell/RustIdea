# Concurrency and event queue report

## NativeHost event queue

`NativeHost` now uses `tokio::sync::mpsc::channel(2048)` instead of an unbounded
channel. The event sink uses `try_send`; overflow is classified instead of
silently discarded:

| Event class | Overflow behavior |
|---|---|
| `DocPatched` | sets a recovery flag; UI resyncs all open documents |
| `FsChanged` | merges created/modified/removed/ignore-rule path sets |
| Diagnostics | keeps latest set per `(path, source)` |
| Task/Index/Server/VCS/Settings/Notify | keeps latest per logical key |
| Lifecycle/other | sets a general recovery flag; UI refreshes project tree/Git |

The UI pump select-waits on the bounded receiver and an overflow notification.
After draining an overflow report, document patch loss triggers
`DocumentBuffer::force_resync`, and filesystem batches are applied through the
normal incremental tree handler.

Tests:

- `native_host::tests::full_queue_coalesces_progress_but_keeps_recovery_flags`
- `native_host::tests::filesystem_overflow_merges_every_path`
- `tests/native_event_regression.rs` (3 tests)

## Project tree

- one load request per directory (`loading` set + per-directory generation);
- stale generations cannot replace newer listings;
- project generation (`last_tree_epoch`) discards requests from an old project;
- cached expansion/collapse performs no filesystem reads;
- watcher events invalidate only affected loaded parents;
- 2,000-row pages with an explicit **Show N more…** row preserve access to the
  full directory without building a 100,000-row element array at once.

Tests:

- cached expand/collapse;
- duplicate request coalescing;
- stale generation rejection;
- incremental parent-only invalidation;
- deleted directory is not rescanned;
- 100,000-file pagination (integration test).

## Background I/O

The ordinary open/edit/save paths use the backend runtime for:

- directory reads and project descriptors;
- document open/save/edit/snapshot;
- BLAKE3 disk hashing and save verification;
- path classification at startup;
- directory creation and reveal-process launch;
- session persistence;
- syntax highlighting and coalesced document diff.

See `RUSTIDEA_MAIN_THREAD_AUDIT.md` for the remaining synchronous boundaries.

## Remaining limits

- Queue capacity and event rates have not been stress-profiled on a real large
  project.
- `LanguageServerRemoved` currently has no explicit removal handling in the
  native server-status map beyond the coalesced event.
- Conflict-path three-way merge/diff still runs on the UI thread; large
  overlapping conflicts should be moved to a worker in a later pass.
- Long-running memory/backlog growth has not been measured with a GUI session.
