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
use crate::text_input::{TextInput, TextInputEvent};
use crate::theme::{Theme, ThemeMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarMode {
    Project,
    Search,
    Git,
}

fn refresh_window(handle: Option<gpui::WindowHandle<Workbench>>, cx: &mut gpui::AsyncApp) {
    if let Some(handle) = handle
        && let Err(error) = handle.update(cx, |_, window, _| window.refresh())
    {
        eprintln!("WORKBENCH_REFRESH_ERROR {error:#}");
    }
}

fn change_letter(kind: Option<ide_vcs::ChangeKind>) -> &'static str {
    match kind {
        Some(ide_vcs::ChangeKind::Added) => "A",
        Some(ide_vcs::ChangeKind::Modified) => "M",
        Some(ide_vcs::ChangeKind::Deleted) => "D",
        Some(ide_vcs::ChangeKind::Renamed) => "R",
        Some(ide_vcs::ChangeKind::Untracked) => "?",
        Some(ide_vcs::ChangeKind::Ignored) => "I",
        Some(ide_vcs::ChangeKind::Conflicted) => "C",
        None => "·",
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
    run_configs: Vec<ide_run::RunConfig>,
    selected_run: usize,
    running_run: Option<ide_run::RunInfo>,
    run_output: String,
    run_status: gpui::SharedString,
    sidebar_mode: SidebarMode,
    search_input: Entity<TextInput>,
    search_pattern: String,
    search_results: Vec<ide_search::FileMatches>,
    search_status: gpui::SharedString,
    _search_subscription: Subscription,
    git_status: Vec<ide_vcs::FileStatus>,
    git_branch: Option<ide_vcs::BranchInfo>,
    git_message_input: Entity<TextInput>,
    git_status_message: gpui::SharedString,
    _git_subscription: Subscription,
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
        let run_configs = host.run_configs().unwrap_or_default();
        let run_status = format!("{} run configurations", run_configs.len()).into();
        let theme = Theme::for_mode(ThemeMode::Dark);
        let search_input = cx.new(|cx| {
            TextInput::new(
                "Search project files…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let search_subscription = cx.subscribe(&search_input, |app, _, event, cx| match event {
            TextInputEvent::Submit(pattern) => {
                app.start_search(pattern.clone(), cx);
            }
            TextInputEvent::Change(pattern) => {
                app.search_pattern = pattern.clone();
            }
            TextInputEvent::Cancel => {
                app.sidebar_mode = SidebarMode::Project;
                cx.notify();
            }
            TextInputEvent::MoveSelection(_) => {}
        });
        let git_message_input = cx.new(|cx| {
            TextInput::new(
                "Commit message…",
                theme.text,
                theme.faint,
                theme.selection,
                theme.cursor,
                cx,
            )
        });
        let git_subscription = cx.subscribe(&git_message_input, |app, _, event, cx| {
            if let TextInputEvent::Submit(message) = event {
                app.commit_git(message.clone(), cx);
            }
        });
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
            theme,
            status: status.into(),
            diagnostics: HashMap::default(),
            window_handle,
            run_configs,
            selected_run: 0,
            running_run: None,
            run_output: String::new(),
            run_status,
            sidebar_mode: SidebarMode::Project,
            search_input,
            search_pattern: String::new(),
            search_results: Vec::new(),
            search_status: "Type a pattern and press Enter".into(),
            _search_subscription: search_subscription,
            git_status: Vec::new(),
            git_branch: None,
            git_message_input,
            git_status_message: "Loading Git status…".into(),
            _git_subscription: git_subscription,
        };
        if let Some(document) = document {
            app.insert_document(document, &registry_for_initial, cx);
        }
        app.refresh_git(cx);
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

    fn start_selected_run(&mut self, cx: &mut Context<Self>) {
        let Some(config) = self.run_configs.get(self.selected_run).cloned() else {
            self.run_status = "No run configuration selected".into();
            cx.notify();
            return;
        };
        let host = self.host.clone();
        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel();
        let emit: ide_host::ops::Emit<ide_run::OutputChunk> = Arc::new(move |chunk| {
            let _ = out_tx.send(chunk);
        });
        self.run_output.clear();
        self.run_status = format!("Starting {}…", config.name).into();
        cx.spawn(async move |this, cx| {
            let result = host.start_run(config, emit).await;
            let started = result.is_ok();
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(info) => {
                            app.run_status =
                                format!("Running {} (pid {:?})", info.name, info.pid).into();
                            app.running_run = Some(info);
                        }
                        Err(error) => {
                            app.run_status = format!("Run failed: {}", error.message).into();
                            app.running_run = None;
                        }
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
            if !started {
                return;
            }
            while let Some(chunk) = out_rx.recv().await {
                let text = match chunk {
                    ide_run::OutputChunk::Stdout { text }
                    | ide_run::OutputChunk::Stderr { text }
                    | ide_run::OutputChunk::System { text } => text,
                };
                let handle = this
                    .update(cx, |app, cx| {
                        app.run_output.push_str(&text);
                        if app.run_output.len() > 2_000_000 {
                            let split = app.run_output.len().saturating_sub(1_800_000);
                            app.run_output.drain(..split);
                        }
                        cx.notify();
                        app.window_handle
                    })
                    .ok()
                    .flatten();
                refresh_window(handle, cx);
            }
            let handle = this
                .update(cx, |app, cx| {
                    app.running_run = None;
                    app.run_status = "Process finished".into();
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn stop_active_run(&mut self, cx: &mut Context<Self>) {
        let Some(run) = self.running_run.as_ref() else {
            return;
        };
        if let Err(error) = self.host.stop_run(run.id) {
            self.run_status = format!("Stop failed: {}", error.message).into();
        } else {
            self.run_status = "Stopping…".into();
        }
        cx.notify();
    }

    fn start_search(&mut self, pattern: String, cx: &mut Context<Self>) {
        let pattern = pattern.trim().to_owned();
        if pattern.is_empty() {
            self.search_results.clear();
            self.search_status = "Type a pattern and press Enter".into();
            cx.notify();
            return;
        }
        self.search_results.clear();
        self.search_status = format!("Searching for {pattern}…").into();
        let query = ide_search::SearchQuery {
            pattern,
            is_regex: false,
            case_sensitive: false,
            whole_word: false,
            include: Vec::new(),
            exclude: Vec::new(),
            scope: ide_search::SearchScope::Project,
            max_matches: 10_000,
            context: ide_search::SearchContext::Any,
        };
        let host = self.host.clone();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let emit: ide_host::ops::Emit<ide_host::ops::search::SearchEvent> =
            Arc::new(move |event| {
                let _ = event_tx.send(event);
            });
        cx.spawn(async move |this, cx| {
            let result = host.start_search(query, emit).await;
            if let Err(error) = &result {
                let handle = this
                    .update(cx, |app, cx| {
                        app.search_status = format!("Search failed: {}", error.message).into();
                        cx.notify();
                        app.window_handle
                    })
                    .ok()
                    .flatten();
                refresh_window(handle, cx);
                return;
            }
            while let Some(event) = event_rx.recv().await {
                let handle = this
                    .update(cx, |app, cx| {
                        match event {
                            ide_host::ops::search::SearchEvent::File { file } => {
                                app.search_results.push(file);
                                app.search_status = format!(
                                    "{} files, {} matches",
                                    app.search_results.len(),
                                    app.search_results
                                        .iter()
                                        .map(|file| file.matches.len())
                                        .sum::<usize>()
                                )
                                .into();
                            }
                            ide_host::ops::search::SearchEvent::Done { summary } => {
                                app.search_status = format!(
                                    "{} files, {} matches{}",
                                    summary.files,
                                    summary.matches,
                                    if summary.limit_hit {
                                        " (limit reached)"
                                    } else {
                                        ""
                                    }
                                )
                                .into();
                            }
                        }
                        cx.notify();
                        app.window_handle
                    })
                    .ok()
                    .flatten();
                refresh_window(handle, cx);
            }
        })
        .detach();
        cx.notify();
    }

    fn refresh_git(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.git_status_message = "Loading Git status…".into();
        cx.spawn(async move |this, cx| {
            let (status, branch) = tokio::join!(host.git_status(), host.git_branch());
            let handle = this
                .update(cx, |app, cx| {
                    match status {
                        Ok(status) => {
                            app.git_status_message = if status.is_empty() {
                                "No changes".into()
                            } else {
                                format!("{} changed files", status.len()).into()
                            };
                            app.git_status = status;
                        }
                        Err(error) => {
                            app.git_status.clear();
                            app.git_status_message = error.message.into();
                        }
                    }
                    if let Ok(branch) = branch {
                        app.git_branch = Some(branch);
                    }
                    cx.notify();
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn stage_all(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self
            .git_status
            .iter()
            .map(|status| status.path.0.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let host = self.host.clone();
        self.git_status_message = "Staging files…".into();
        cx.spawn(async move |this, cx| {
            let result = host.git_stage(paths).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.git_status_message = match result {
                        Ok(()) => "Staged all changes".into(),
                        Err(error) => format!("Stage failed: {}", error.message).into(),
                    };
                    app.refresh_git(cx);
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn unstage_all(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self
            .git_status
            .iter()
            .filter(|status| status.staged.is_some())
            .map(|status| status.path.0.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let host = self.host.clone();
        self.git_status_message = "Unstaging files…".into();
        cx.spawn(async move |this, cx| {
            let result = host.git_unstage(paths).await;
            let handle = this
                .update(cx, |app, cx| {
                    app.git_status_message = match result {
                        Ok(()) => "Unstaged changes".into(),
                        Err(error) => format!("Unstage failed: {}", error.message).into(),
                    };
                    app.refresh_git(cx);
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
    }

    fn commit_git(&mut self, message: String, cx: &mut Context<Self>) {
        let message = message.trim().to_owned();
        if message.is_empty() {
            self.git_status_message = "Enter a commit message".into();
            cx.notify();
            return;
        }
        let host = self.host.clone();
        self.git_status_message = "Committing…".into();
        cx.spawn(async move |this, cx| {
            let result = host.git_commit(message).await;
            let handle = this
                .update(cx, |app, cx| {
                    match result {
                        Ok(commit) => {
                            app.git_status_message =
                                format!("Created commit {}", &commit[..commit.len().min(12)])
                                    .into();
                            app.git_message_input.update(cx, |input, cx| {
                                input.set_content("", cx);
                            });
                        }
                        Err(error) => {
                            app.git_status_message =
                                format!("Commit failed: {}", error.message).into();
                        }
                    }
                    app.refresh_git(cx);
                    app.window_handle
                })
                .ok()
                .flatten();
            refresh_window(handle, cx);
        })
        .detach();
        cx.notify();
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

    pub fn smoke_start_run(&mut self, cx: &mut Context<Self>) {
        self.start_selected_run(cx);
    }

    pub fn smoke_run_status(&self) -> String {
        self.run_status.to_string()
    }

    pub fn smoke_run_output(&self) -> &str {
        &self.run_output
    }

    pub fn smoke_search(&mut self, pattern: &str, cx: &mut Context<Self>) {
        self.sidebar_mode = SidebarMode::Search;
        self.start_search(pattern.to_owned(), cx);
    }

    pub fn smoke_search_status(&self) -> String {
        self.search_status.to_string()
    }

    pub fn smoke_search_files(&self) -> usize {
        self.search_results.len()
    }

    pub fn smoke_git_files(&self) -> usize {
        self.git_status.len()
    }

    pub fn smoke_git_branch(&self) -> String {
        self.git_branch
            .as_ref()
            .and_then(|branch| branch.head.clone())
            .unwrap_or_default()
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
            IdeEvent::RunExited { run, code } => {
                if self
                    .running_run
                    .as_ref()
                    .is_some_and(|running| running.id == run)
                {
                    self.running_run = None;
                    self.run_status = format!("Exited with {:?}", code).into();
                }
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

    fn render_git_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let branch = self
            .git_branch
            .as_ref()
            .map(|branch| {
                let head = branch.head.as_deref().unwrap_or("detached");
                format!(
                    "{head}{}{}",
                    if branch.ahead > 0 {
                        format!(" ↑{}", branch.ahead)
                    } else {
                        String::new()
                    },
                    if branch.behind > 0 {
                        format!(" ↓{}", branch.behind)
                    } else {
                        String::new()
                    }
                )
            })
            .unwrap_or_else(|| "No Git repository".into());
        let mut files = div()
            .id("git-files")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, file) in self.git_status.iter().take(200).enumerate() {
            let path = file.path.0.clone();
            let open_app = cx.entity();
            let letter = change_letter(file.unstaged.or(file.staged));
            let color = if file.unstaged.is_some() {
                theme.warning
            } else {
                theme.accent
            };
            files = files.child(
                div()
                    .id(("git-file", index))
                    .h(px(22.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.5))
                    .text_color(theme.text)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(move |_: &ClickEvent, _, cx| {
                        open_app.update(cx, |app, cx| {
                            app.open_path(path.clone(), cx);
                        });
                    })
                    .child(div().w(px(12.)).text_color(color).child(letter))
                    .child(div().flex_1().truncate().child(file.rel_path.clone())),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .px_3()
                    .flex()
                    .items_center()
                    .text_size(px(11.5))
                    .text_color(theme.accent)
                    .child(branch),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .border_y_1()
                    .border_color(theme.border)
                    .child(self.git_message_input.clone()),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(10.5))
                    .text_color(theme.muted)
                    .child(self.git_status_message.clone())
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("git-stage-all")
                            .px_2()
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.stage_all(cx);
                            }))
                            .child("Stage all"),
                    )
                    .child(
                        div()
                            .id("git-unstage-all")
                            .px_2()
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.unstage_all(cx);
                            }))
                            .child("Unstage all"),
                    ),
            )
            .child(files)
            .into_any_element()
    }

    fn render_search_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut results = div()
            .id("search-results")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (file_index, file) in self.search_results.iter().take(80).enumerate() {
            let path = file.path.0.clone();
            let open_app = cx.entity();
            results = results.child(
                div()
                    .id(("search-file", file_index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.5))
                    .text_color(theme.muted)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                    .on_click(move |_: &ClickEvent, _, cx| {
                        open_app.update(cx, |app, cx| {
                            app.open_path(path.clone(), cx);
                        });
                    })
                    .child(div().flex_1().truncate().child(file.rel_path.clone()))
                    .child(
                        div()
                            .text_color(theme.faint)
                            .child(file.matches.len().to_string()),
                    ),
            );
            for (match_index, line) in file.matches.iter().take(20).enumerate() {
                let path = file.path.0.clone();
                let open_app = cx.entity();
                let line_number = line.line + 1;
                results = results.child(
                    div()
                        .id(("search-match", file_index * 100 + match_index))
                        .h(px(22.))
                        .pl_5()
                        .pr_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.5))
                        .text_color(theme.text)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(move |_: &ClickEvent, _, cx| {
                            open_app.update(cx, |app, cx| {
                                app.open_path(path.clone(), cx);
                            });
                        })
                        .child(
                            div()
                                .w(px(34.))
                                .text_color(theme.faint)
                                .child(line_number.to_string()),
                        )
                        .child(div().flex_1().truncate().child(line.text.clone())),
                );
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(34.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.search_input.clone()),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.5))
                    .text_color(theme.faint)
                    .child(self.search_status.clone()),
            )
            .child(results)
            .into_any_element()
    }

    fn render_run_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        if self.run_configs.is_empty() {
            return div().into_any_element();
        }
        let running = self.running_run.is_some();
        let mut configs = Vec::new();
        for (index, config) in self.run_configs.iter().take(6).enumerate() {
            let selected = index == self.selected_run;
            configs.push(
                div()
                    .id(("run-config", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_size(px(11.))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .bg(if selected {
                        theme.panel_alt
                    } else {
                        theme.titlebar
                    })
                    .cursor_pointer()
                    .hover(|style| style.text_color(theme.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.selected_run = index;
                        cx.notify();
                    }))
                    .child(config.name.clone()),
            );
        }
        let mut lines: Vec<String> = self.run_output.lines().map(str::to_owned).collect();
        if lines.len() > 200 {
            lines.drain(..lines.len() - 200);
        }
        let mut output = div()
            .id("run-output")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p_2()
            .font_family("Menlo")
            .text_size(px(11.5))
            .text_color(theme.text)
            .flex()
            .flex_col();
        if lines.is_empty() {
            output = output.child(
                div()
                    .text_color(theme.faint)
                    .child("Run output appears here."),
            );
        } else {
            output = output.children(lines.into_iter().map(|line| div().child(line)));
        }
        div()
            .h(px(180.))
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .h(px(32.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .px_2()
                            .text_size(px(11.))
                            .text_color(theme.muted)
                            .child("RUN"),
                    )
                    .children(configs)
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("start-run")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_size(px(11.))
                            .text_color(theme.text)
                            .bg(theme.accent_muted)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.start_selected_run(cx);
                            }))
                            .child("Run"),
                    )
                    .when(running, |row| {
                        row.child(
                            div()
                                .id("stop-run")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_size(px(11.))
                                .text_color(theme.error)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.stop_active_run(cx);
                                }))
                                .child("Stop"),
                        )
                    })
                    .child(
                        div()
                            .px_2()
                            .text_size(px(11.))
                            .text_color(theme.muted)
                            .child(self.run_status.clone()),
                    ),
            )
            .child(output)
            .into_any_element()
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
        let search = self.render_search_panel(cx);
        let git = self.render_git_panel(cx);
        let sidebar_body = match self.sidebar_mode {
            SidebarMode::Project => project,
            SidebarMode::Search => search,
            SidebarMode::Git => git,
        };
        let tabs = self.render_tabs(cx).into_any_element();
        let run_panel = self.render_run_panel(cx);
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
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_size(px(11.))
                                    .child(
                                        div()
                                            .id("sidebar-project")
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .cursor_pointer()
                                            .text_color(
                                                if self.sidebar_mode == SidebarMode::Project {
                                                    theme.text
                                                } else {
                                                    theme.muted
                                                },
                                            )
                                            .bg(if self.sidebar_mode == SidebarMode::Project {
                                                theme.panel_alt
                                            } else {
                                                theme.panel
                                            })
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.sidebar_mode = SidebarMode::Project;
                                                cx.notify();
                                            }))
                                            .child("PROJECT"),
                                    )
                                    .child(
                                        div()
                                            .id("sidebar-search")
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .cursor_pointer()
                                            .text_color(
                                                if self.sidebar_mode == SidebarMode::Search {
                                                    theme.text
                                                } else {
                                                    theme.muted
                                                },
                                            )
                                            .bg(if self.sidebar_mode == SidebarMode::Search {
                                                theme.panel_alt
                                            } else {
                                                theme.panel
                                            })
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.sidebar_mode = SidebarMode::Search;
                                                cx.notify();
                                            }))
                                            .child("SEARCH"),
                                    )
                                    .child(
                                        div()
                                            .id("sidebar-git")
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .cursor_pointer()
                                            .text_color(if self.sidebar_mode == SidebarMode::Git {
                                                theme.text
                                            } else {
                                                theme.muted
                                            })
                                            .bg(if self.sidebar_mode == SidebarMode::Git {
                                                theme.panel_alt
                                            } else {
                                                theme.panel
                                            })
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.sidebar_mode = SidebarMode::Git;
                                                this.refresh_git(cx);
                                                cx.notify();
                                            }))
                                            .child("GIT"),
                                    ),
                            )
                            .child(div().flex_1().min_h(px(0.)).child(sidebar_body)),
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
            .child(run_panel)
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
