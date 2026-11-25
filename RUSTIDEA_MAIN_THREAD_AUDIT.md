# GPUI main-thread blocking audit

## Ordinary user paths moved off the UI thread

| Path | Before | Current |
|---|---|---|
| Project directory listing | host future on GPUI poll with Tokio assumptions | `NativeHost::run` on host runtime; generation-checked apply |
| Project startup path classification | `Path::is_dir/is_file` on UI | `spawn_blocking` classification |
| Document open/save/edit/snapshot | mixed sync/borrowed futures | `NativeHost::run`, async document ops |
| Save disk verification | metadata only on UI | background BLAKE3 stream + compare |
| Disk baseline hash | none | background worker |
| Syntax highlighting | full parse on every render | background `highlights_async`, latest-version commit |
| Local/backend diff | full text diff on UI | background `spawn_blocking` diff |
| Directory creation | `ide_vfs::create_dir` on UI | `create_dir_async` runtime worker |
| Reveal in file manager | `Command::spawn` on UI | runtime `spawn_blocking` |
| Session persistence | `fsync`/rename on UI | runtime `spawn_blocking` |

## Path normalization

- `DocumentKey::for_ui_path` and `DocumentStore::lexical_path` are pure lexical
  operations safe on the UI thread.
- Real `canonicalize` remains only in test-only `normalize_path`.
- Backend `DocumentInfo` paths are treated as the authoritative canonical
  identity after an open completes.

## Audited remaining synchronous points

| Point | Reason / risk | Follow-up |
|---|---|---|
| `NativeHost::document_info` in explicit conflict reload | backend snapshot copy; no filesystem walk | replace with `document_info_async` in the reload path |
| Conflict three-way `merge_three_way`/`coalesced_diff` | CPU, potentially large on overlapping conflicts | move to blocking worker with generation check |
| `ide_host`/`ide_vfs` internals | some metadata calls occur inside backend worker after adapter | verify callers remain on runtime |
| Session load in `SessionStore::discover/load` | startup before interactive work | acceptable; can be moved to startup worker |
| `DiskStamp::of` unit-test-only | test code | no production UI caller |

No ordinary edit, scroll, project-tree expansion or save path contains a known
synchronous `read_dir`, `metadata`, `canonicalize`, `read`, `write`, or
`rename` call on the GPUI thread.

## Evidence

The audit is based on `rg` over `crates/rustidea/src` plus the changes above.
Slow-disk, network-filesystem and 100k-file latency measurements are still
required for full Phase 5 acceptance.
