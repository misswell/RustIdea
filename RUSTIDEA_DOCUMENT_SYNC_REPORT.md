# Document synchronization report

## Implemented protocol

`DocumentBuffer` maintains a local editable mirror and a confirmed backend
version. The backend OpenIDEA `Documents` registry remains authoritative.

State retained per document:

- confirmed `backend_version`;
- `base_text` captured before the first unconfirmed local transaction;
- at most one `inflight` backend edit;
- `SyncStatus` (`Synced`, `Dirty`, `Syncing`, `Conflict`, `Error`);
- an explicit conflict snapshot containing backend version/text;
- `ignore_patches_through` for patches caused by our own acknowledged edits;
- an optional `pending_save` operation.

Normal flow:

1. Local edit is applied to the single shared `DocumentBuffer`.
2. A 50 ms debounce coalesces edits.
3. A background task computes one UTF-16 diff from `base_text` to local text.
4. Only one edit request is submitted at `backend_version`.
5. Success advances the confirmed version; edits typed during the request
   remain pending and trigger the next request.
6. Backend patches for our own submitted version are ignored; older/newer
   external patches are processed in version order.

External/backend patch while local edits are pending:

- a pure three-way merge is attempted using `base_text`, local text and backend
  text;
- disjoint changed ranges are rebased and applied as `EditSource::External`,
  which does not add a local undo entry;
- overlapping/ambiguous ranges stay local and enter explicit `Conflict`;
- version gaps trigger a backend snapshot resync rather than a full reload that
  discards edits.

Conflict resolution:

- **Keep local**: records the backend snapshot as the new base and submits the
  local text as the next coalesced edit.
- **Reload backend**: explicit user choice; replaces the local mirror with the
  backend snapshot.

Save protection:

- a save waits for pending synchronization;
- if a conflict or unresolved external disk change exists, the save is refused;
- after synchronization, a background BLAKE3 content check compares current
  disk bytes with the confirmed disk version;
- a changed disk file becomes an explicit external conflict before any write.

Undo/redo:

- external edits rebase existing history ranges through the external change;
- overlapping history entries are dropped rather than applied at stale offsets;
- local history remains capped at 1,000 transactions.

## Tests

- `crates/rustidea/tests/document_sync_regression.rs`: 12 tests, including
  10,000 sequential inputs, disjoint rebase and conflict preservation.
- `crates/rustidea/tests/document_conflict_regression.rs`: conflict/merge cases.
- `crates/rustidea/tests/document_multiview_regression.rs`: one-handle sharing,
  view counts, Save As rekey and last-view release.
- `crates/rustidea/tests/editor_undo_regression.rs`: external edit shifts undo
  history without corrupting it.
- `crates/rustidea-editor/src/document.rs` tests: bounded history, delta undo,
  external history rebase.
- `crates/ide-host/src/ops/doc/tests.rs`: explicit dirty-buffer
  `reload_discarding_changes`.

## Remaining limits

- The merge is conservative: overlapping but semantically mergeable edits still
  become a conflict.
- The UI does not yet have a side-by-side **Compare Changes** view; the conflict
  banner exposes keep/reload.
- Interactive IME/clipboard/split-editing acceptance is still blocked by the
  locked macOS console.
- The backend `Documents` registry still holds a synchronized text mirror, so
  strict single-copy memory is not implemented. It is one UI mirror plus the
  authoritative backend mirror, not one copy per tab.
