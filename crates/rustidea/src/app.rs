use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    App, ClickEvent, Context, FocusHandle, Focusable, PathPromptOptions, PromptLevel, Subscription,
    Window, actions, div, prelude::*, px, rgb,
};

use crate::backend::{RustIdeaBackend, display_name};
use crate::editor::{Editor, create_document};
use crate::project::{ProjectTree, open_directory};
use crate::session::{Session, SessionStore};
use crate::theme::Theme;

actions!(
    rustidea,
    [
        Quit,
        NewFile,
        OpenFile,
        OpenFolder,
        Save,
        SaveAs,
        CloseTab,
        NextTab,
        PreviousTab,
        ToggleTheme,
        ToggleAutosave,
    ]
);

struct Tab {
    editor: gpui::Entity<Editor>,
    label: String,
    close_after_save: bool,
    _subscription: Subscription,
}

pub struct RustIdea {
    focus_handle: FocusHandle,
    backend: Arc<dyn RustIdeaBackend>,
    session_store: SessionStore,
    session: Session,
    theme: Theme,
    project: Option<ProjectTree>,
    tabs: Vec<Tab>,
    active_tab: Option<usize>,
    status: String,
    untitled_count: u64,
}

impl RustIdea {
    pub fn new(
        session_store: SessionStore,
        session: Session,
        backend: Arc<dyn RustIdeaBackend>,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = Theme::for_mode(session.theme);
        Self {
            focus_handle: cx.focus_handle(),
            backend,
            session_store,
            session,
            theme,
            project: None,
            tabs: Vec::new(),
            active_tab: None,
            status: "Ready".into(),
            untitled_count: 0,
        }
    }

    pub fn restore_last_project(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.session.last_project.clone() else {
            return;
        };
        if path.is_dir() {
            self.open_project(path, cx);
        } else {
            self.status = format!("Last project is unavailable: {}", path.display());
        }
    }

    pub fn open_startup_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if path.is_dir() {
            self.open_project(path, cx);
        } else if path.is_file() {
            self.request_open_path(path, window, cx);
        } else {
            self.status = format!("Startup path is unavailable: {}", path.display());
            cx.notify();
        }
    }

    fn active_editor(&self) -> Option<gpui::Entity<Editor>> {
        self.tabs
            .get(self.active_tab?)
            .map(|tab| tab.editor.clone())
    }

    fn activate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.active_tab = Some(index);
        let focus_handle = self.tabs[index].editor.read(cx).focus_handle(cx);
        window.focus(&focus_handle);
        cx.notify();
    }

    fn add_editor(
        &mut self,
        editor: gpui::Entity<Editor>,
        label: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscription = cx.observe(&editor, |app, editor, cx| {
            if let Some(tab) = app.tabs.iter_mut().find(|tab| tab.editor == editor)
                && let Some(path) = editor.read(cx).path()
            {
                tab.label = display_name(path);
            }
            if let Some(index) = app.tabs.iter().position(|tab| tab.editor == editor)
                && app.tabs[index].close_after_save
                && !editor.read(cx).is_saving()
            {
                if !editor.read(cx).is_dirty() && editor.read(cx).error().is_none() {
                    app.remove_tab(index, cx);
                } else {
                    app.tabs[index].close_after_save = false;
                    app.status = "File remains open: save failed or newer edits exist".into();
                }
            }
            cx.notify();
        });
        self.tabs.push(Tab {
            editor: editor.clone(),
            label,
            close_after_save: false,
            _subscription: subscription,
        });
        self.active_tab = Some(self.tabs.len() - 1);
        let focus_handle = editor.read(cx).focus_handle(cx);
        window.focus(&focus_handle);
        cx.notify();
    }

    fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.untitled_count += 1;
        let label = format!("Untitled {}", self.untitled_count);
        let theme = self.theme;
        let backend = self.backend.clone();
        let autosave = self.session.autosave;
        let editor = cx.new(|cx| {
            Editor::new(
                None,
                create_document(None, ""),
                backend,
                theme,
                autosave,
                cx,
            )
        });
        self.add_editor(editor, label, window, cx);
        self.status = "New file — use Save As to choose a path".into();
    }

    fn save_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.active_editor() else {
            self.status = "No active editor".into();
            cx.notify();
            return;
        };
        if editor.read(cx).path().is_none() {
            self.save_active_as(window, cx);
            return;
        }
        editor.update(cx, |editor, cx| editor.save(cx));
        cx.notify();
    }

    fn save_active_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.active_editor() else {
            self.status = "No active editor".into();
            cx.notify();
            return;
        };
        let title = editor.read(cx).title();
        let directory = editor
            .read(cx)
            .path()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .or_else(|| {
                self.project
                    .as_ref()
                    .map(|project| project.root().to_path_buf())
            })
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let receiver = cx.prompt_for_new_path(&directory, Some(&title));
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            this.update_in(cx, |app, window, cx| {
                match result {
                    Ok(Some(path)) => {
                        if editor.read(cx).is_saving() {
                            app.status = "Wait for the current save before using Save As".into();
                            cx.notify();
                            return;
                        }
                        editor.update(cx, |editor, cx| editor.save_as(path, cx));
                        app.status = "Saving…".into();
                        window.focus(&editor.read(cx).focus_handle(cx));
                    }
                    Ok(None) => {
                        app.status = "Save As cancelled".into();
                    }
                    Err(error) => {
                        app.status = format!("Save As failed: {error}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_file_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open File".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            this.update_in(cx, |app, window, cx| match result {
                Ok(Some(paths)) => {
                    for path in paths {
                        app.request_open_path(path, window, cx);
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    app.status = format!("Open File failed: {error}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn open_folder_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|error| Err(anyhow::anyhow!(error.to_string())));
            this.update_in(cx, |app, window, cx| match result {
                Ok(Some(mut paths)) if !paths.is_empty() => {
                    let path = paths.remove(0);
                    app.open_project(path, cx);
                    window.focus(&app.focus_handle.clone());
                }
                Ok(_) => {}
                Err(error) => {
                    app.status = format!("Open Folder failed: {error}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn request_open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.editor.read(cx).path() == Some(path.as_path()))
        {
            self.activate_tab(index, window, cx);
            return;
        }
        let backend = self.backend.clone();
        let read_path = path.clone();
        let task = cx.background_executor().spawn(async move {
            backend
                .read_text(&read_path)
                .map(|text| create_document(Some(&read_path), &text))
        });
        self.status = format!("Opening {}…", display_name(&path));
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |app, window, cx| {
                app.finish_open_path(path, result, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn finish_open_path(
        &mut self,
        path: PathBuf,
        result: anyhow::Result<rustidea_editor::Document>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(document) => {
                if let Some(index) = self
                    .tabs
                    .iter()
                    .position(|tab| tab.editor.read(cx).path() == Some(path.as_path()))
                {
                    self.activate_tab(index, window, cx);
                    return;
                }
                if let Some(project) = self.project.as_mut() {
                    project.reveal(&path);
                }
                let title = display_name(&path);
                let theme = self.theme;
                let backend = self.backend.clone();
                let autosave = self.session.autosave;
                let editor_path = path.clone();
                let editor = cx.new(move |cx| {
                    Editor::new(Some(editor_path), document, backend, theme, autosave, cx)
                });
                self.add_editor(editor, title, window, cx);
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => {
                self.status = format!("Open failed: {error}");
                tracing::error!(%error, path = %path.display(), "failed to open file");
            }
        }
        cx.notify();
    }

    fn open_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match open_directory(path.clone(), self.backend.clone()) {
            Ok(project) => {
                self.session.remember_project(&path);
                if let Err(error) = self.session_store.save(&self.session) {
                    tracing::warn!(%error, "failed to persist RustIdea session");
                }
                self.project = Some(project);
                self.status = format!("Project {}", path.display());
            }
            Err(error) => {
                self.status = format!("Open project failed: {error}");
                tracing::error!(%error, path = %path.display(), "failed to open project");
            }
        }
        cx.notify();
    }

    fn toggle_project_node(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let Some(node) = project.nodes().get(index).cloned() else {
            return;
        };
        if node.is_directory {
            project.toggle(index);
        } else {
            self.request_open_path(node.path, window, cx);
            return;
        }
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        let editor = tab.editor.clone();
        if editor.read(cx).is_saving() {
            self.status = "Wait for the current save before closing".into();
            cx.notify();
            return;
        }
        let is_dirty = editor.read(cx).is_dirty();
        if !is_dirty {
            self.remove_tab(index, cx);
            return;
        }

        let has_path = editor.read(cx).path().is_some();
        let buttons: &[&str] = if has_path {
            &["Save and Close", "Discard", "Cancel"]
        } else {
            &["Discard", "Cancel"]
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            "This file has unsaved changes.",
            None,
            buttons,
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let choice = answer.await.unwrap_or(buttons.len() - 1);
            this.update_in(cx, |app, window, cx| {
                if has_path {
                    match choice {
                        0 => {
                            if let Some(current) =
                                app.tabs.iter().position(|tab| tab.editor == editor)
                            {
                                app.tabs[current].close_after_save = true;
                                editor.update(cx, |editor, cx| editor.save(cx));
                            }
                        }
                        1 => {
                            if let Some(current) =
                                app.tabs.iter().position(|tab| tab.editor == editor)
                            {
                                app.remove_tab(current, cx);
                            }
                        }
                        _ => {
                            window.focus(&editor.read(cx).focus_handle(cx));
                        }
                    }
                } else if choice == 0 {
                    if let Some(current) = app.tabs.iter().position(|tab| tab.editor == editor) {
                        app.remove_tab(current, cx);
                    }
                } else {
                    window.focus(&editor.read(cx).focus_handle(cx));
                }
            })
            .ok();
        })
        .detach();
    }

    fn remove_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        self.active_tab = match self.active_tab {
            Some(_) if self.tabs.is_empty() => None,
            Some(active) if active > index => Some(active - 1),
            Some(active) if active == index => Some(index.min(self.tabs.len() - 1)),
            other => other,
        };
        self.status = "Tab closed".into();
        cx.notify();
    }

    fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }
        let next = self
            .active_tab
            .map_or(0, |index| (index + 1) % self.tabs.len());
        self.activate_tab(next, window, cx);
    }

    fn previous_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }
        let previous = self.active_tab.map_or(0, |index| {
            if index == 0 {
                self.tabs.len() - 1
            } else {
                index - 1
            }
        });
        self.activate_tab(previous, window, cx);
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.session.theme = self.session.theme.toggled();
        self.theme = Theme::for_mode(self.session.theme);
        for tab in &self.tabs {
            tab.editor
                .update(cx, |editor, _| editor.set_theme(self.theme));
        }
        if let Err(error) = self.session_store.save(&self.session) {
            tracing::warn!(%error, "failed to persist theme");
        }
        self.status = format!("{} theme", self.theme.mode.name());
        cx.notify();
    }

    fn toggle_autosave(&mut self, cx: &mut Context<Self>) {
        self.session.autosave = !self.session.autosave;
        for tab in &self.tabs {
            let enabled = self.session.autosave;
            tab.editor
                .update(cx, |editor, _| editor.set_autosave(enabled));
        }
        if let Err(error) = self.session_store.save(&self.session) {
            tracing::warn!(%error, "failed to persist autosave setting");
        }
        self.status = if self.session.autosave {
            "Autosave enabled".into()
        } else {
            "Autosave disabled".into()
        };
        cx.notify();
    }
}

impl Focusable for RustIdea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RustIdea {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let project_name = self
            .project
            .as_ref()
            .map(|project| project.root().display().to_string())
            .unwrap_or_else(|| "No project".into());
        let mut tab_elements = Vec::with_capacity(self.tabs.len());
        for (index, tab) in self.tabs.iter().enumerate() {
            let active = self.active_tab == Some(index);
            let dirty = tab.editor.read(cx).is_dirty();
            let label = tab.label.clone();
            tab_elements.push(
                div()
                    .id(("tab", index))
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .border_r_1()
                    .border_color(theme.border)
                    .bg(if active {
                        theme.tab_active
                    } else {
                        theme.panel
                    })
                    .text_color(if active { theme.text } else { theme.muted })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.activate_tab(index, window, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(if active { "●" } else { " " })
                            .child(label),
                    )
                    .child(
                        div()
                            .id(("tab-close", index))
                            .px_1()
                            .rounded_sm()
                            .text_color(theme.faint)
                            .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.close_tab(index, window, cx);
                            }))
                            .child(if dirty { "•" } else { "×" }),
                    ),
            );
        }

        let mut tree_rows = Vec::new();
        if let Some(project) = self.project.as_ref() {
            for (index, node) in project.nodes().iter().enumerate() {
                let name = node.name.clone();
                let is_directory = node.is_directory;
                let expanded = node.expanded;
                let depth = node.depth;
                tree_rows.push(
                    div()
                        .id(("tree", index))
                        .h(px(23.))
                        .flex()
                        .items_center()
                        .gap_1()
                        .pl(px(8.0 + depth as f32 * 14.0))
                        .pr_2()
                        .text_color(if is_directory {
                            theme.muted
                        } else {
                            theme.text
                        })
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.toggle_project_node(index, window, cx);
                        }))
                        .child(if is_directory {
                            if expanded { "▾" } else { "▸" }
                        } else {
                            "·"
                        })
                        .child(name),
                );
            }
        }

        let active_status = self.active_editor().map(|editor| {
            let editor = editor.read(cx);
            (
                editor.cursor_position(),
                editor.line_count(),
                editor.is_dirty(),
                editor.is_saving(),
                editor.path().map(|path| path.display().to_string()),
                editor.error().map(str::to_owned),
            )
        });

        let mut recent_projects = Vec::new();
        for (index, path) in self.session.recent_projects.iter().take(6).enumerate() {
            let path = path.clone();
            let label = path.display().to_string();
            recent_projects.push(
                div()
                    .id(("recent", index))
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .text_color(theme.muted)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.open_project(path.clone(), cx);
                        window.focus(&this.focus_handle.clone());
                    }))
                    .child(label),
            );
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.window)
            .text_color(theme.text)
            .text_size(px(13.))
            .key_context("RustIdea")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &NewFile, window, cx| this.new_file(window, cx)))
            .on_action(cx.listener(|this, _: &OpenFile, window, cx| {
                this.open_file_prompt(window, cx)
            }))
            .on_action(cx.listener(|this, _: &OpenFolder, window, cx| {
                this.open_folder_prompt(window, cx)
            }))
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save_active(window, cx)))
            .on_action(cx.listener(|this, _: &SaveAs, window, cx| {
                this.save_active_as(window, cx)
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                if let Some(index) = this.active_tab {
                    this.close_tab(index, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NextTab, window, cx| this.next_tab(window, cx)))
            .on_action(cx.listener(|this, _: &PreviousTab, window, cx| {
                this.previous_tab(window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleTheme, _window, cx| this.toggle_theme(cx)))
            .on_action(cx.listener(|this, _: &ToggleAutosave, _window, cx| {
                this.toggle_autosave(cx)
            }))
            .child(
                div()
                    .h(px(42.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .bg(theme.titlebar)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("RustIdea")
                            .child(
                                div()
                                    .px_1()
                                    .rounded_sm()
                                    .bg(theme.accent_muted)
                                    .text_color(theme.accent)
                                    .text_size(px(10.))
                                    .child("NATIVE"),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_color(theme.faint)
                            .child(project_name),
                    )
                    .child(self.toolbar_button("Open File", &theme, cx, |this, window, cx| {
                        this.open_file_prompt(window, cx)
                    }))
                    .child(self.toolbar_button("Open Folder", &theme, cx, |this, window, cx| {
                        this.open_folder_prompt(window, cx)
                    }))
                    .child(self.toolbar_button("Save", &theme, cx, |this, window, cx| {
                        this.save_active(window, cx)
                    }))
                    .child(self.toolbar_button(
                        if self.session.autosave {
                            "Autosave On"
                        } else {
                            "Autosave Off"
                        },
                        &theme,
                        cx,
                        |this, _window, cx| this.toggle_autosave(cx),
                    ))
                    .child(self.toolbar_button(theme.mode.name(), &theme, cx, |this, _window, cx| {
                        this.toggle_theme(cx)
                    })),
            )
            .child(
                div()
                    .id("tab-strip")
                    .h(px(34.))
                    .flex()
                    .items_center()
                    .bg(theme.panel)
                    .border_b_1()
                    .border_color(theme.border)
                    .overflow_x_scroll()
                    .children(tab_elements),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .child(
                        div()
                            .w(px(248.))
                            .flex_none()
                            .flex()
                            .flex_col()
                            .bg(theme.panel)
                            .border_r_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .h(px(34.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px_3()
                                    .text_size(px(10.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.faint)
                                    .child("EXPLORER")
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(
                                                div()
                                            .id("explorer-open-file")
                                                    .cursor_pointer()
                                                    .hover(|style| {
                                                        style.text_color(theme.text)
                                                    })
                                                    .on_click(cx.listener(
                                                        |this, _: &ClickEvent, window, cx| {
                                                            this.open_file_prompt(window, cx)
                                                        },
                                                    ))
                                                    .child("+"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("project-tree-scroll")
                                    .flex_1()
                                    .min_h(px(0.))
                                    .overflow_y_scroll()
                                    .children(tree_rows),
                            )
                            .when_some(
                                self.project
                                    .as_ref()
                                    .and_then(|project| project.error().map(str::to_owned)),
                                |this, error| {
                                    this.child(
                                        div()
                                            .p_2()
                                            .text_color(theme.warning)
                                            .text_size(px(11.))
                                            .child(error),
                                    )
                                },
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .when_some(self.active_editor(), |this, editor| {
                                this.child(editor)
                            })
                            .when(self.active_tab.is_none(), |this| {
                                this.child(
                                    div()
                                        .size_full()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap_4()
                                        .child(
                                            div()
                                                .text_size(px(36.))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .child("RustIdea"),
                                        )
                                        .child(
                                            div()
                                                .text_color(theme.muted)
                                                .child(
                                                    "Native GPUI editor · Ropey document engine · no WebView",
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .id("welcome-open-file")
                                                        .px_4()
                                                        .py_2()
                                                        .rounded_md()
                                                        .bg(theme.accent)
                                                        .text_color(rgb(0xffffff))
                                                        .cursor_pointer()
                                                        .hover(|style| style.opacity(0.88))
                                                        .on_click(cx.listener(
                                                            |this, _: &ClickEvent, window, cx| {
                                                                this.open_file_prompt(window, cx)
                                                            },
                                                        ))
                                                        .child("Open File"),
                                                )
                                                .child(
                                                    div()
                                                        .id("welcome-open-folder")
                                                        .px_4()
                                                        .py_2()
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(theme.border)
                                                        .cursor_pointer()
                                                        .hover(|style| {
                                                            style.bg(theme.panel_alt)
                                                        })
                                                        .on_click(cx.listener(
                                                            |this, _: &ClickEvent, window, cx| {
                                                                this.open_folder_prompt(window, cx)
                                                            },
                                                        ))
                                                        .child("Open Folder"),
                                                )
                                                .child(
                                                    div()
                                                        .id("welcome-new-file")
                                                        .px_4()
                                                        .py_2()
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(theme.border)
                                                        .cursor_pointer()
                                                        .hover(|style| {
                                                            style.bg(theme.panel_alt)
                                                        })
                                                        .on_click(cx.listener(
                                                            |this, _: &ClickEvent, window, cx| {
                                                                this.new_file(window, cx)
                                                            },
                                                        ))
                                                        .child("New File"),
                                                ),
                                        )
                                        .when(!recent_projects.is_empty(), |this| {
                                            this.child(
                                                div()
                                                    .w(px(520.))
                                                    .mt_6()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .px_3()
                                                            .text_size(px(10.))
                                                            .font_weight(
                                                                gpui::FontWeight::SEMIBOLD,
                                                            )
                                                            .text_color(theme.faint)
                                                            .child("RECENT PROJECTS"),
                                                    )
                                                    .children(recent_projects),
                                            )
                                        })
                                        .child(
                                            div()
                                                .mt_4()
                                                .text_size(px(11.))
                                                .text_color(theme.faint)
                                                .child(
                                                    "⌘O Open File   ⇧⌘O Open Folder   ⌘S Save   ⌘Z Undo",
                                                ),
                                        ),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .gap_4()
                    .px_3()
                    .bg(theme.titlebar)
                    .border_t_1()
                    .border_color(theme.border)
                    .text_size(px(11.))
                    .text_color(theme.muted)
                    .child(self.status.clone())
                    .child(div().flex_1())
                    .when_some(active_status, |this, status| {
                        let (cursor, lines, dirty, saving, path, error) = status;
                        this.child(format!("Ln {}, Col {}", cursor.0, cursor.1))
                            .child(format!("{} lines", lines))
                            .child(if saving {
                                "Saving…".to_string()
                            } else if dirty {
                                "Modified".to_string()
                            } else {
                                "Saved".to_string()
                            })
                            .when_some(path, |this, path| this.child(path))
                            .when_some(error, |this, error| {
                                this.text_color(theme.error).child(error)
                            })
                    })
                    .child(if self.session.autosave {
                        "UTF-8 · LF · Autosave"
                    } else {
                        "UTF-8 · LF"
                    }),
            )
    }
}

impl RustIdea {
    fn toolbar_button(
        &self,
        label: &'static str,
        theme: &Theme,
        cx: &mut Context<Self>,
        listener: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let on_click = cx.listener(move |this: &mut Self, _: &ClickEvent, window, cx| {
            listener(this, window, cx);
        });
        div()
            .id(label)
            .px_2()
            .py_1()
            .rounded_sm()
            .text_size(px(11.))
            .text_color(theme.muted)
            .cursor_pointer()
            .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
            .on_click(on_click)
            .child(label)
    }
}
