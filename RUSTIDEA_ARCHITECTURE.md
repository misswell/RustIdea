# Native architecture

`rustidea` owns the GPUI application, workbench, editor view and a local service
interface. `rustidea-editor` owns the versioned document and undo history;
`ide-text` and Ropey store text. `ide-core` provides shared text types. The
current application opens files through `RustIdeaBackend`; it does not load the
OpenIDEA frontend or Tauri runtime.

This is an early implementation, not full OpenIDEA parity. A view currently
owns its document; a shared document registry and independent view state are
required before multi-split editing. The local filesystem backend is temporary:
future migration must reuse the upstream host document services, external-change
conflict handling, language services and events rather than maintaining parallel
implementations. Project tree traversal and document construction also need
large-project responsiveness validation.

Future UI components should consume Rust services, with blocking filesystem and
language operations off the UI thread. OpenIDEA remains a reference and
regression baseline, not a second production editor.
