use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use editor::Editor;
use gpui::{
    ClickEvent, Context, Entity, Focusable as _, Subscription, UniformListScrollHandle, Window,
    div, prelude::*, px, uniform_list,
};
use ide_core::{DocId, IdeEvent};
use language::{BufferEvent, Diagnostic, DiagnosticEntry, DiagnosticMessage, DiagnosticSet};
use lsp::LanguageServerId;

use crate::bridge::{ZedDocument, is_buffer_edited};
use crate::code_actions::OpenIdeaCodeActionProvider;
use crate::completion::OpenIdeaCompletionProvider;
use crate::documents::{DocumentKey, DocumentStore};
use crate::host::NativeHost;
use crate::lsp::DocumentLsp;
use crate::project::{ProjectTree, Toggle};
use crate::semantics::OpenIdeaSemanticsProvider;
use crate::theme::{Theme, ThemeMode};

fn refresh_window(handle: Option<gpui::WindowHandle<Workbench>>, cx: &mut gpui::AsyncApp) {
    if let Some(handle) = handle
        && let Err(error) = handle.update(cx, |_, window, _| window.refresh())
    {
        eprintln!("WORKBENCH_REFRESH_ERROR {error:#}");
    }
}

pub struct InitialState {
    pub project: ide_host::ops::project::ProjectInfo,
    pub root_listing: Vec<ide_vfs::FileEntry>,
    pub document: Option<ide_workspace::DocumentInfo>,
}

pub struct Workbench {
    host: Arc<NativeHost>,
    language_registry: Arc<language::LanguageRegistry>,
    root: PathBuf,
    tree: ProjectTree,
    tree_scroll: UniformListScrollHandle,
    documents: HashMap<DocId, ZedDocument>,
    store: DocumentStore<DocId>,
    editors: HashMap<(DocId, bool), Entity<Editor>>,
    split: bool,
    order: Vec<DocId>,
    active: Option<DocId>,
    subscriptions: HashMap<DocId, Subscription>,
    diagnostic_servers: HashMap<String, LanguageServerId>,
    next_diagnostic_server: usize,
    theme: Theme,
    status: gpui::SharedString,
    diagnostics: HashMap<PathBuf, usize>,
    window_handle: Option<gpui::WindowHandle<Workbench>>,
}

impl Workbench {
    pub fn new(
        host: Arc<NativeHost>,
        language_registry: Arc<language::LanguageRegistry>,
        root: PathBuf,
        initial: InitialState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let InitialState {
            project,
            root_listing,
            document,
        } = initial;
        let window_handle = window.window_handle().downcast::<Workbench>();
        let mut tree = ProjectTree::new(root.clone());
        tree.apply_directory(root.clone(), root_listing);
        tree.rebuild();
        let registry_for_initial = language_registry.clone();
        let split = std::env::var_os("RUSTIDEA_ZED_SMOKE_SPLIT").is_some();
        let status = format!("{} · {}", project.name, root.display());
        let mut app = Self {
            host,
            language_registry,
            root,
            tree,
            tree_scroll: UniformListScrollHandle::new(),
            documents: HashMap::default(),
            store: DocumentStore::default(),
            editors: HashMap::default(),
            split,
            order: Vec::new(),
            active: None,
            subscriptions: HashMap::default(),
            diagnostic_servers: HashMap::default(),
            next_diagnostic_server: 1,
            theme: Theme::for_mode(ThemeMode::Dark),
            status: status.into(),
            diagnostics: HashMap::default(),
            window_handle,
        };
        if let Some(document) = document {
            app.insert_document(document, &registry_for_initial, cx);
        }
        let ui_events = app.host.take_events();
        if let Some(mut events) = ui_events {
            cx.spawn(async move |this, cx| {
                while let Some(event) = events.receiver.recv().await {
                    let handle = this
                        .update(cx, |app, cx| {
                            app.handle_event(event, cx);
                            app.window_handle
                        })
                        .ok()
                        .flatten();
                    refresh_window(handle, cx);
                }
            })
            .detach();
        }
        app
    }

    pub fn set_window_handle(&mut self, window: gpui::WindowHandle<Workbench>) {
        self.window_handle = Some(window);
    }

    fn load_root(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let root = self.root.clone();
        cx.spawn(async move |this, cx| {
            let listing = host.read_dir(&root).await;
            let handle = this
                .update(cx, |app, cx| {
                    match listing {
                        Ok(listing) => {
                            app.tree.apply_directory(root, listing);
                            eprintln!("WORKBENCH_TREE rows={}", app.tree.nodes().len());
                        }
                        Err(error) => app.tree.apply_error(root, error.message),
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn load_directory(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let listing = host.read_dir(&path).await;
            let handle = this
                .update(cx, |app, cx| {
                    match listing {
                        Ok(listing) => app.tree.apply_directory(path, listing),
                        Err(error) => app.tree.apply_error(path, error.message),
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn activate(&mut self, doc: DocId, cx: &mut Context<Self>) {
        self.active = Some(doc);
        cx.notify();
    }

    fn ensure_editor(
        &mut self,
        doc: DocId,
        secondary: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<Editor>> {
        if let Some(editor) = self.editors.get(&(doc, secondary)) {
            return Some(editor.clone());
        }
        let document = self.documents.get(&doc)?;
        let buffer = document.buffer.clone();
        let editor = cx.new(|cx| Editor::for_buffer(buffer, None, window, cx));
        let lsp = Arc::new(DocumentLsp::new(
            self.host.clone(),
            doc,
            document.path.clone(),
        ));
        editor.update(cx, |editor, _| {
            editor.set_completion_provider(Some(Rc::new(OpenIdeaCompletionProvider::new(
                lsp.clone(),
            ))));
            editor
                .set_semantics_provider(Some(Rc::new(OpenIdeaSemanticsProvider::new(lsp.clone()))));
            editor.set_code_action_provider(Some(Rc::new(OpenIdeaCodeActionProvider::new(lsp))));
        });
        self.editors.insert((doc, secondary), editor.clone());
        Some(editor)
    }

    fn close_tab(&mut self, doc: DocId, cx: &mut Context<Self>) {
        let Some(position) = self.order.iter().position(|candidate| *candidate == doc) else {
            return;
        };
        let path = self
            .documents
            .get(&doc)
            .map(|document| document.path.clone());
        let was_active = self.active == Some(doc);
        self.order.remove(position);
        self.subscriptions.remove(&doc);
        self.documents.remove(&doc);
        self.editors.remove(&(doc, false));
        self.editors.remove(&(doc, true));
        if let Some(path) = path {
            self.store.release(&DocumentKey::Path(path.clone()));
            let _ = self.host.close_document(doc);
            if self.split && self.order.is_empty() {
                self.split = false;
            }
        }
        if was_active {
            self.active = self
                .order
                .get(position.min(self.order.len().saturating_sub(1)))
                .copied();
        }
        cx.notify();
    }

    fn toggle_split(&mut self, cx: &mut Context<Self>) {
        self.split = !self.split;
        cx.notify();
    }

    fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(doc) = self.store.get(&DocumentKey::for_ui_path(&path)).copied() {
            self.activate(doc, cx);
            return;
        }
        if self
            .documents
            .values()
            .any(|document| document.path == path)
        {
            if let Some((doc, _)) = self.documents.iter().find(|(_, d)| d.path == path) {
                self.activate(*doc, cx);
            }
            return;
        }
        self.status = format!("Opening {}…", path.display()).into();
        let host = self.host.clone();
        let registry = self.language_registry.clone();
        cx.spawn(async move |this, cx| {
            let info = host.open_document(&path).await;
            let handle = this
                .update(cx, |app, cx| {
                    match info {
                        Ok(info) => app.insert_document(info, &registry, cx),
                        Err(error) => {
                            app.status = format!("Open failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn insert_document(
        &mut self,
        info: ide_workspace::DocumentInfo,
        registry: &Arc<language::LanguageRegistry>,
        cx: &mut Context<Self>,
    ) {
        let key = DocumentKey::Path(info.path.0.clone());
        if let Some(existing) = self.store.get(&key).copied() {
            self.activate(existing, cx);
            return;
        }
        let doc = ZedDocument::open(info.clone(), registry.clone(), cx);
        let buffer = doc.buffer.clone();
        let doc_id = doc.doc;
        let subscription = cx.subscribe(&buffer, move |app, _, event: &BufferEvent, cx| {
            if is_buffer_edited(event) {
                app.sync_document(doc_id, cx);
            }
        });
        self.documents.insert(doc_id, doc);
        eprintln!(
            "WORKBENCH_DOC_OPEN doc={} path={} language={} tabs={}",
            doc_id.0,
            info.path.0.display(),
            info.language,
            self.order.len() + 1
        );
        self.subscriptions.insert(doc_id, subscription);
        self.order.push(doc_id);
        if self.store.insert_view(key, doc_id, doc_id) {
            self.active = Some(doc_id);
            self.status = format!("{} · {}", info.path.0.display(), info.language).into();
        }
        cx.notify();
    }

    fn sync_document(&mut self, doc_id: DocId, cx: &mut Context<Self>) {
        let Some(document) = self.documents.get_mut(&doc_id) else {
            return;
        };
        if document.syncing || document.applying_backend_patch {
            return;
        }
        let Some((request_clock, changes)) = document.pending_changes(cx) else {
            return;
        };
        let host = self.host.clone();
        let base_version = document.backend_version;
        cx.spawn(async move |this, cx| {
            let result = host
                .apply_document_edit_async(doc_id, base_version, changes)
                .await;
            let handle = this
                .update(cx, |app, cx| {
                    let Some(document) = app.documents.get_mut(&doc_id) else {
                        return app.window_handle;
                    };
                    document.syncing = false;
                    match result {
                        Ok(version) => {
                            document.backend_version = version;
                            document.confirmed_clock = request_clock.clone();
                            let current = document.buffer.read(cx).snapshot().version().clone();
                            if current != request_clock {
                                app.sync_document(doc_id, cx);
                            } else if document.save_requested {
                                document.save_requested = false;
                                app.save_document(doc_id, cx);
                            }
                        }
                        Err(error) => {
                            document.conflict =
                                Some(format!("OpenIDEA edit failed: {}", error.message));
                            app.status = document.conflict.clone().unwrap().into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    fn save_document(&mut self, doc_id: DocId, cx: &mut Context<Self>) {
        let Some(document) = self.documents.get(&doc_id) else {
            return;
        };
        if document.syncing {
            if let Some(document) = self.documents.get_mut(&doc_id) {
                document.save_requested = true;
            }
            return;
        }
        let version = document.buffer.read(cx).snapshot().version().clone();
        let buffer = document.buffer.clone();
        let host = self.host.clone();
        cx.spawn(async move |this, cx| {
            let result = host.save_document(doc_id).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(()) => {
                            buffer.update(cx, |buffer, cx| buffer.did_save(version, None, cx));
                            app.status = "Saved through OpenIDEA".into();
                        }
                        Err(error) => {
                            app.status = format!("Save failed: {}", error.message).into();
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
    }

    pub fn smoke_insert_marker(&mut self, marker: &str, cx: &mut Context<Self>) -> bool {
        let Some(doc) = self.active else {
            return false;
        };
        let Some(document) = self.documents.get(&doc) else {
            return false;
        };
        let buffer = document.buffer.clone();
        buffer.update(cx, |buffer, cx| {
            buffer.edit([(0..0, format!("{marker}\n"))], None, cx);
        });
        true
    }

    pub fn smoke_save_active(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(doc) = self.active else {
            return false;
        };
        self.save_document(doc, cx);
        true
    }

    pub fn smoke_request_completion(&mut self, line: u32, column: u32, cx: &mut Context<Self>) {
        let Some(doc) = self.active else {
            return;
        };
        let Some(document) = self.documents.get(&doc) else {
            return;
        };
        let lsp = Arc::new(DocumentLsp::new(
            self.host.clone(),
            doc,
            document.path.clone(),
        ));
        cx.spawn(async move |_this, cx| {
            let mut count = 0;
            for attempt in 0..30 {
                match lsp.completion(line, column, Some(".".into())).await {
                    Ok(list) => {
                        count = list.items.len();
                        if count > 0 {
                            break;
                        }
                    }
                    Err(error) if attempt == 29 => {
                        eprintln!("WORKBENCH_COMPLETION_ERROR {}", error.message);
                    }
                    Err(_) => {}
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
            }
            eprintln!("WORKBENCH_COMPLETION count={count}");
        })
        .detach();
    }

    pub fn smoke_active_text(&self, cx: &gpui::App) -> String {
        self.active
            .and_then(|doc| self.documents.get(&doc))
            .map(|document| document.text(cx))
            .unwrap_or_default()
    }

    pub fn smoke_status(&self) -> String {
        self.status.to_string()
    }

    pub fn smoke_enable_split(&mut self, cx: &mut Context<Self>) {
        if !self.split {
            self.toggle_split(cx);
        }
    }

    pub fn smoke_editor_count(&self) -> usize {
        self.editors.len()
    }

    pub fn smoke_has_language(&self, cx: &gpui::App) -> bool {
        self.active
            .and_then(|doc| self.documents.get(&doc))
            .is_some_and(|document| document.buffer.read(cx).language().is_some())
    }

    fn handle_event(&mut self, event: IdeEvent, cx: &mut Context<Self>) {
        match event {
            IdeEvent::DocPatched {
                doc,
                version,
                changes,
                ..
            } => {
                if let Some(document) = self.documents.get_mut(&doc)
                    && let Err(error) = document.apply_backend_patch(version, &changes, cx)
                {
                    self.status = format!("Document patch failed: {error:#}").into();
                }
            }
            IdeEvent::Diagnostics {
                path,
                source,
                items,
            } => {
                let Some((doc_id, _)) = self
                    .documents
                    .iter()
                    .find(|(_, document)| document.path == path.0)
                else {
                    return;
                };
                let doc_id = *doc_id;
                let server_id = *self
                    .diagnostic_servers
                    .entry(source.clone())
                    .or_insert_with(|| {
                        let id = LanguageServerId(self.next_diagnostic_server);
                        self.next_diagnostic_server += 1;
                        id
                    });
                let Some(document) = self.documents.get(&doc_id) else {
                    return;
                };
                let snapshot = document.buffer.read(cx).snapshot();
                let entries: Vec<DiagnosticEntry<text::PointUtf16>> = items
                    .into_iter()
                    .map(|diagnostic| {
                        let range = text::PointUtf16::new(
                            diagnostic.range.start.line,
                            diagnostic.range.start.col,
                        )
                            ..text::PointUtf16::new(
                                diagnostic.range.end.line,
                                diagnostic.range.end.col,
                            );
                        let zed = Diagnostic {
                            source: Some(source.clone()),
                            severity: match diagnostic.severity {
                                ide_core::Severity::Error => lsp::DiagnosticSeverity::ERROR,
                                ide_core::Severity::Warning => lsp::DiagnosticSeverity::WARNING,
                                ide_core::Severity::Info => lsp::DiagnosticSeverity::INFORMATION,
                                ide_core::Severity::Hint => lsp::DiagnosticSeverity::HINT,
                            },
                            message: DiagnosticMessage::plain(diagnostic.message),
                            ..Default::default()
                        };
                        DiagnosticEntry::new(range, zed)
                    })
                    .collect();
                let set = DiagnosticSet::new(entries, &snapshot);
                let count = set.len();
                document.buffer.update(cx, |buffer, cx| {
                    buffer.update_diagnostics(server_id, set, cx);
                });
                self.diagnostics.insert(path.0, count);
            }
            IdeEvent::LanguageServer { name, status, .. } => {
                self.status = format!("{name}: {status:?}").into();
            }
            IdeEvent::FsChanged {
                modified, created, ..
            } => {
                let changed: Vec<PathBuf> = created
                    .into_iter()
                    .chain(modified)
                    .map(|path| path.0)
                    .collect();
                let reload = self.tree.note_external_changes(&changed, &[]);
                for directory in reload {
                    self.load_directory(directory, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut row = div()
            .h(px(34.))
            .flex()
            .items_center()
            .bg(theme.titlebar)
            .border_b_1()
            .border_color(theme.border);
        for doc_id in &self.order {
            let Some(document) = self.documents.get(doc_id) else {
                continue;
            };
            let active = self.active == Some(*doc_id);
            let title = document
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| document.path.display().to_string());
            let dirty = document.is_dirty(cx);
            let id = *doc_id;
            row = row.child(
                div()
                    .id(("tab", id.0))
                    .h_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_r_1()
                    .border_color(theme.border)
                    .bg(if active {
                        theme.tab_active
                    } else {
                        theme.titlebar
                    })
                    .text_color(if active { theme.text } else { theme.muted })
                    .text_size(px(12.))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.activate(id, cx);
                        if let Some(editor) = this.editors.get(&(id, false)) {
                            let focus = editor.read(cx).focus_handle(cx);
                            window.focus(&focus, cx);
                        }
                    }))
                    .child(title)
                    .when(dirty, |tab| {
                        tab.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(theme.accent))
                    })
                    .child(
                        div()
                            .id(("close-tab", id.0))
                            .px_1()
                            .text_color(theme.faint)
                            .hover(|style| style.text_color(theme.error))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.close_tab(id, cx);
                            }))
                            .child("×"),
                    ),
            );
        }
        row = row.child(div().flex_1()).child(
            div()
                .id("toggle-split")
                .px_3()
                .text_color(theme.muted)
                .text_size(px(11.))
                .cursor_pointer()
                .hover(|style| style.text_color(theme.text))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_split(cx);
                }))
                .child(if self.split { "Unsplit" } else { "Split" }),
        );
        row
    }

    fn render_project(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let nodes = self.tree.nodes();
        let row_count = nodes.len();
        let app = cx.entity();
        uniform_list("project-tree", row_count, move |range, _window, _cx| {
            let nodes = nodes.clone();
            let app = app.clone();
            range
                .map(|index| {
                    let node = nodes[index].clone();
                    let name = node.name.clone();
                    let depth = node.depth;
                    let is_directory = node.is_directory;
                    let expanded = node.expanded;
                    let is_more = node.more;
                    let color = if is_more {
                        theme.accent
                    } else if node.ignored {
                        theme.faint
                    } else if is_directory {
                        theme.muted
                    } else {
                        theme.text
                    };
                    let click_app = app.clone();
                    div()
                        .id(("tree-row", index))
                        .h(px(23.))
                        .pl(px(6.0 + depth as f32 * 13.0))
                        .pr_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(12.5))
                        .text_color(color)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(move |_: &ClickEvent, window, cx| {
                            click_app.update(cx, |app, cx| {
                                app.toggle_tree_row(index, Some(window), cx);
                            });
                        })
                        .child(div().w(px(13.)).flex_none().text_color(theme.faint).child(
                            if is_more {
                                "＋".to_string()
                            } else if is_directory {
                                if expanded { "▾" } else { "▸" }.to_string()
                            } else {
                                "·".to_string()
                            },
                        ))
                        .child(div().flex_1().min_w(px(0.)).truncate().child(name))
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.tree_scroll)
    }

    fn toggle_tree_row(
        &mut self,
        index: usize,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let toggle = self.tree.toggle(index);
        match toggle {
            Toggle::OpenFile(path) => self.open_path(path, cx),
            Toggle::LoadDirectory(path) => self.load_directory(path, cx),
            Toggle::Symlink => {
                self.status = "Symlinked directories are not expanded".into();
            }
            Toggle::Rebuilt | Toggle::Missing => {}
        }
        if let Some(window) = window
            && let Some(doc) = self.active
            && let Some(editor) = self.editors.get(&(doc, false))
        {
            window.focus(&editor.read(cx).focus_handle(cx), cx);
        }
        cx.notify();
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut focus_primary = false;
        if let Some(doc) = self.active {
            focus_primary = !self.editors.contains_key(&(doc, false));
            self.ensure_editor(doc, false, window, cx);
            if self.split {
                self.ensure_editor(doc, true, window, cx);
            }
        }
        if focus_primary
            && let Some(doc) = self.active
            && let Some(editor) = self.editors.get(&(doc, false))
        {
            let focus = editor.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        }
        let active_editor = self
            .active
            .and_then(|doc| self.editors.get(&(doc, false)).cloned());
        let split_editor = self
            .active
            .filter(|_| self.split)
            .and_then(|doc| self.editors.get(&(doc, true)).cloned());
        let project = self.render_project(cx).into_any_element();
        let tabs = self.render_tabs(cx).into_any_element();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.window)
            .text_color(theme.text)
            .on_action(cx.listener(|this, _: &crate::SaveDocument, _, cx| {
                if let Some(doc) = this.active {
                    this.save_document(doc, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &crate::CloseTab, _, cx| {
                if let Some(doc) = this.active {
                    this.close_tab(doc, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &crate::ToggleSplit, _, cx| {
                this.toggle_split(cx);
            }))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .child(
                        div()
                            .w(px(250.))
                            .h_full()
                            .flex_none()
                            .bg(theme.panel)
                            .border_r_1()
                            .border_color(theme.border)
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .h(px(30.))
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .text_size(px(11.))
                                    .text_color(theme.muted)
                                    .child("PROJECT"),
                            )
                            .child(div().flex_1().min_h(px(0.)).child(project)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .child(tabs)
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .flex()
                                    .child(
                                        div().flex_1().min_w(px(0.)).h_full().child(
                                            match active_editor {
                                                Some(editor) => editor.into_any_element(),
                                                None => div()
                                                    .size_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(theme.faint)
                                                    .child("Open a file from the project tree")
                                                    .into_any_element(),
                                            },
                                        ),
                                    )
                                    .when_some(split_editor, |row, editor| {
                                        row.child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .h_full()
                                                .border_l_1()
                                                .border_color(theme.border)
                                                .child(editor),
                                        )
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .h(px(24.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .bg(theme.titlebar)
                    .border_t_1()
                    .border_color(theme.border)
                    .text_size(px(11.))
                    .text_color(theme.muted)
                    .child(self.status.clone())
                    .child(div().flex_1())
                    .child(format!(
                        "{} tabs · {} diagnostics",
                        self.order.len(),
                        self.diagnostics.values().sum::<usize>()
                    )),
            )
    }
}
