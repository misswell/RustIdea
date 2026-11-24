use gpui::{
    ClickEvent, MouseButton, MouseMoveEvent, MouseUpEvent, SharedString, Window, div, prelude::*,
    px, uniform_list,
};

use super::*;

impl RustIdea {
    fn render_titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let project_name = self
            .project
            .as_ref()
            .map(|project| project.info.display_name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "No project".into());
        let project_path = self
            .project
            .as_ref()
            .map(|project| project.root.display().to_string())
            .unwrap_or_default();
        let mut run_label = "Run";
        if let Some(tab) = self.active_tab() {
            let _ = tab;
        }
        let _ = &mut run_label;
        let mut recent_projects = Vec::new();
        for (index, path) in self.session.recent_projects.iter().take(5).enumerate() {
            let path = path.clone();
            let open_path = path.clone();
            recent_projects.push(
                div()
                    .id(("recent-project", index))
                    .w_full()
                    .px_2()
                    .py(px(5.))
                    .rounded_sm()
                    .text_color(theme.muted)
                    .text_size(px(12.))
                    .truncate()
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.open_project(open_path.clone(), cx);
                        window.focus(&this.focus_handle.clone());
                    }))
                    .child(path.display().to_string()),
            );
        }
        let _ = recent_projects;

        div()
            .h(px(44.))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
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
                    .child(
                        div()
                            .h(px(24.))
                            .w(px(24.))
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(theme.accent_muted)
                            .text_color(theme.accent)
                            .child("R"),
                    )
                    .child("RustIdea")
                    .child(
                        div()
                            .px_1()
                            .rounded_sm()
                            .bg(theme.accent_muted)
                            .text_color(theme.accent)
                            .text_size(px(9.))
                            .child("NATIVE"),
                    ),
            )
            .child(
                div()
                    .ml_2()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(theme.faint)
                    .text_size(px(12.))
                    .child(if project_path.is_empty() {
                        project_name
                    } else {
                        project_path.clone()
                    }),
            )
            .child(self.title_button("Open", theme, cx, |this, window, cx| {
                this.open_file_prompt(window, cx)
            }))
            .child(self.title_button("Folder", theme, cx, |this, window, cx| {
                this.open_folder_prompt(window, cx)
            }))
            .child(self.title_button("Run", theme, cx, |this, window, cx| {
                this.show_bottom(BottomKind::Run, cx);
                window.focus(&this.focus_handle.clone());
            }))
            .child(self.title_button("Debug", theme, cx, |this, window, cx| {
                this.show_bottom(BottomKind::Debug, cx);
                window.focus(&this.focus_handle.clone());
            }))
            .child(self.title_button("Search", theme, cx, |this, window, cx| {
                this.show_sidebar(SidebarKind::Search, cx);
                let focus = this.search_input.read(cx).focus_handle(cx);
                window.focus(&focus);
            }))
            .child(
                self.title_button("Settings", theme, cx, |this, window, cx| {
                    this.show_sidebar(SidebarKind::Settings, cx);
                    window.focus(&this.focus_handle.clone());
                }),
            )
            .child(
                self.title_button(theme.mode.name(), theme, cx, |this, _window, cx| {
                    this.toggle_theme(cx)
                }),
            )
    }

    fn title_button(
        &self,
        label: &'static str,
        theme: Theme,
        cx: &mut Context<Self>,
        handler: fn(&mut RustIdea, &mut Window, &mut Context<RustIdea>),
    ) -> impl IntoElement + use<> {
        div()
            .id(SharedString::from(format!("title-button-{label}")))
            .h(px(26.))
            .px_2()
            .rounded_md()
            .flex()
            .items_center()
            .text_size(px(12.))
            .text_color(theme.muted)
            .cursor_pointer()
            .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
            .on_click(
                cx.listener(move |this, _: &ClickEvent, window, cx| handler(this, window, cx)),
            )
            .child(label)
    }

    fn render_activity_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let items = [
            (SidebarKind::Project, "▤", "Project"),
            (SidebarKind::Search, "⌕", "Find in Files"),
            (SidebarKind::Git, "⑂", "Version Control"),
            (SidebarKind::Structure, "◇", "Structure"),
            (SidebarKind::Problems, "!", "Problems"),
            (SidebarKind::Database, "▦", "Database"),
            (SidebarKind::Http, "↗", "HTTP Client"),
            (SidebarKind::Agents, "✦", "Agents"),
            (SidebarKind::Remote, "⇄", "Remote Host"),
            (SidebarKind::Plugins, "⬡", "Plugins"),
            (SidebarKind::Tools, "⚒", "Tools"),
        ];
        let mut buttons = Vec::with_capacity(items.len() + 1);
        for (kind, icon, _tooltip) in items {
            let active = self.sidebar_kind == kind && self.sidebar_visible;
            let problems = kind == SidebarKind::Problems && !self.problems.is_empty();
            buttons.push(
                div()
                    .id(SharedString::from(format!("rail-{}", kind.title())))
                    .relative()
                    .h(px(38.))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(16.))
                    .text_color(if active { theme.accent } else { theme.muted })
                    .bg(if active { theme.panel_alt } else { theme.panel })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if this.sidebar_kind == kind && this.sidebar_visible {
                            this.sidebar_visible = false;
                            cx.notify();
                        } else {
                            this.show_sidebar(kind, cx);
                            let focus = match kind {
                                SidebarKind::Search => {
                                    Some(this.search_input.read(cx).focus_handle(cx))
                                }
                                SidebarKind::Git => {
                                    Some(this.git_message_input.read(cx).focus_handle(cx))
                                }
                                SidebarKind::Settings => {
                                    Some(this.settings_input.read(cx).focus_handle(cx))
                                }
                                _ => None,
                            };
                            if let Some(focus) = focus {
                                window.focus(&focus);
                            } else {
                                window.focus(&this.focus_handle.clone());
                            }
                        }
                    }))
                    .child(icon)
                    .when(problems, |element| {
                        element.child(
                            div()
                                .absolute()
                                .top(px(5.))
                                .right(px(10.))
                                .w(px(7.))
                                .h(px(7.))
                                .rounded_full()
                                .bg(theme.error),
                        )
                    }),
            );
        }
        buttons.push(
            div()
                .id("rail-settings")
                .mt_auto()
                .h(px(38.))
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(15.))
                .text_color(theme.muted)
                .cursor_pointer()
                .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.show_sidebar(SidebarKind::Settings, cx);
                    let focus = this.settings_input.read(cx).focus_handle(cx);
                    window.focus(&focus);
                }))
                .child("⚙"),
        );

        div()
            .w(px(46.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .py_1()
            .gap_1()
            .bg(theme.panel)
            .border_r_1()
            .border_color(theme.border)
            .children(buttons)
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let header = self.sidebar_header(cx);
        let body = match self.sidebar_kind {
            SidebarKind::Project => self.render_project_panel(cx),
            SidebarKind::Search => self.render_search_panel(cx),
            SidebarKind::Git => self.render_git_panel(cx),
            SidebarKind::Structure => self.render_structure_panel(cx),
            SidebarKind::Problems => self.render_problems_panel(cx),
            SidebarKind::Settings => self.render_settings_panel(cx),
            SidebarKind::Database => self.render_db_panel(theme, cx),
            SidebarKind::Http => self.render_http_panel(theme, cx),
            SidebarKind::Agents => self.render_placeholder_panel(
                "Agents",
                "Run coding agents against the open project, stream replies and review their changes.",
                "New Session",
                theme,
                cx,
                |this, _window, cx| this.show_sidebar(SidebarKind::Agents, cx),
            ),
            SidebarKind::Remote => self.render_placeholder_panel(
                "Remote Host",
                "Open SSH connections, browse remote projects and deploy build outputs.",
                "Add Host",
                theme,
                cx,
                |this, _window, cx| this.show_sidebar(SidebarKind::Remote, cx),
            ),
            SidebarKind::Plugins => self.render_placeholder_panel(
                "Plugins",
                "Install, enable and configure extensions that contribute commands, languages and tools.",
                "Browse Plugins",
                theme,
                cx,
                |this, _window, cx| this.show_sidebar(SidebarKind::Plugins, cx),
            ),
            SidebarKind::Tools => self.render_placeholder_panel(
                "Tools",
                "Run searches, format, coverage, profiling and other project-wide operations.",
                "Refresh",
                theme,
                cx,
                |this, _window, cx| this.show_sidebar(SidebarKind::Tools, cx),
            ),
        };
        div()
            .w(px(self.sidebar_width))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_r_1()
            .border_color(theme.border)
            .overflow_hidden()
            .child(header)
            .child(div().flex_1().min_h(px(0.)).overflow_hidden().child(body))
    }

    fn sidebar_header(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let title = self.sidebar_kind.title();
        let actions = match self.sidebar_kind {
            SidebarKind::Project => vec![("+", "New file"), ("⊞", "New folder"), ("↻", "Refresh")],
            SidebarKind::Git => vec![("↻", "Refresh")],
            SidebarKind::Search => vec![("↻", "Search again")],
            _ => vec![("✕", "Hide")],
        };
        let mut action_elements = Vec::new();
        for (index, (label, _tooltip)) in actions.into_iter().enumerate() {
            action_elements.push(
                div()
                    .id(("sidebar-action", index))
                    .px_1()
                    .rounded_sm()
                    .text_color(theme.faint)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        match (this.sidebar_kind, index) {
                            (SidebarKind::Project, 0) => this.new_project_file(window, false, cx),
                            (SidebarKind::Project, 1) => this.new_project_file(window, true, cx),
                            (SidebarKind::Project, 2) => this.refresh_project(cx),
                            (SidebarKind::Git, 0) => this.refresh_git(cx),
                            (SidebarKind::Search, 0) => this.run_search(cx),
                            _ => {
                                this.sidebar_visible = false;
                                cx.notify();
                            }
                        }
                    }))
                    .child(label),
            );
        }
        div()
            .h(px(34.))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .border_b_1()
            .border_color(theme.border)
            .text_size(px(11.))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme.faint)
            .child(title.to_uppercase())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(action_elements),
            )
            .into_any_element()
    }

    fn render_project_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let Some(project) = self.project.as_ref() else {
            let recent = self.session.recent_projects.clone();
            let mut recent_elements = Vec::new();
            for (index, path) in recent.into_iter().take(8).enumerate() {
                let open_path = path.clone();
                recent_elements.push(
                    div()
                        .id(("empty-recent", index))
                        .w_full()
                        .px_2()
                        .py_2()
                        .rounded_md()
                        .text_color(theme.muted)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.open_project(open_path.clone(), cx);
                            window.focus(&this.focus_handle.clone());
                        }))
                        .child(path.display().to_string()),
                );
            }
            return div()
                .size_full()
                .flex()
                .flex_col()
                .p_3()
                .gap_3()
                .child(
                    div().text_color(theme.muted).text_size(px(12.)).child(
                        "Open a folder to start working. RustIdea keeps recent projects here.",
                    ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            self.dialog_button("Open Folder", theme, cx, |this, window, cx| {
                                this.open_folder_prompt(window, cx)
                            }),
                        )
                        .child(
                            self.dialog_button("Open File", theme, cx, |this, window, cx| {
                                this.open_file_prompt(window, cx)
                            }),
                        ),
                )
                .children(recent_elements)
                .into_any_element();
        };
        let nodes = project.tree.nodes();
        let row_count = nodes.len();
        let loading = project.tree.is_loading();
        let error = project.tree.error().map(str::to_string);
        let scroll = project.scroll.clone();
        let app = cx.entity();
        let list = uniform_list(
            "project-tree-rows",
            row_count,
            move |range, _window, _cx| {
                let nodes = nodes.clone();
                let app = app.clone();
                range
                    .map(|index| {
                        let node = nodes[index].clone();
                        let path = node.path.clone();
                        let is_directory = node.is_directory;
                        let expanded = node.expanded;
                        let depth = node.depth;
                        let ignored = node.ignored;
                        let excluded = node.excluded;
                        let name = node.name.clone();
                        let color = if ignored {
                            theme.faint
                        } else if excluded || is_directory {
                            theme.muted
                        } else {
                            theme.text
                        };
                        let click_app = app.clone();
                        let menu_app = app.clone();
                        let menu_path = path.clone();
                        div()
                            .id(("tree-node", index))
                            .h(px(23.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .pl(px(6.0 + depth as f32 * 13.0))
                            .pr_2()
                            .text_color(color)
                            .text_size(px(12.5))
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.panel_alt))
                            .on_click(move |_: &ClickEvent, window, cx| {
                                click_app.update(cx, |app, cx| {
                                    app.toggle_project_node(index, Some(window), cx);
                                });
                            })
                            .on_mouse_down(
                                MouseButton::Right,
                                move |event: &gpui::MouseDownEvent, _window, cx| {
                                    let path = menu_path.clone();
                                    let is_directory = is_directory;
                                    menu_app.update(cx, |app, cx| {
                                        app.context_menu = Some(ContextMenuState {
                                            position: event.position,
                                            path,
                                            is_directory,
                                        });
                                        cx.notify();
                                    });
                                },
                            )
                            .child(div().w(px(13.)).flex_none().text_color(theme.faint).child(
                                if is_directory {
                                    if expanded { "▾" } else { "▸" }
                                } else {
                                    match path
                                        .extension()
                                        .and_then(|extension| extension.to_str())
                                        .unwrap_or_default()
                                    {
                                        "rs" => "R",
                                        "md" => "M",
                                        "toml" | "json" | "yaml" | "yml" => "{}",
                                        _ => "·",
                                    }
                                },
                            ))
                            .child(div().flex_1().min_w(px(0.)).truncate().child(name))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(scroll);
        let mut body = div()
            .id("project-tree")
            .size_full()
            .flex()
            .flex_col()
            .child(list);
        if row_count == 0 && loading {
            body = body.child(
                div()
                    .px_3()
                    .py_2()
                    .text_color(theme.faint)
                    .text_size(px(11.))
                    .child("Loading project tree…"),
            );
        }
        if loading {
            body = body.child(
                div()
                    .px_3()
                    .py_2()
                    .text_color(theme.faint)
                    .text_size(px(11.))
                    .child("Loading project tree…"),
            );
        }
        if let Some(error) = error {
            body = body.child(
                div()
                    .px_3()
                    .py_2()
                    .text_color(theme.error)
                    .text_size(px(11.))
                    .child(error),
            );
        }
        body.into_any_element()
    }

    fn render_search_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut results = Vec::new();
        for (file_index, file) in self.search.results.iter().take(800).enumerate() {
            results.push(
                div()
                    .id(("search-file", file_index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(theme.muted)
                    .text_size(px(11.5))
                    .border_b_1()
                    .border_color(theme.border)
                    .child("▸")
                    .child(div().flex_1().truncate().child(file.rel_path.clone()))
                    .child(
                        div()
                            .text_color(theme.faint)
                            .child(file.matches.len().to_string()),
                    ),
            );
            for (match_index, line) in file.matches.iter().take(30).enumerate() {
                let path = file.path.0.clone();
                let line_number = line.line + 1;
                results.push(
                    div()
                        .id(("search-match", file_index * 100 + match_index))
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .pl_5()
                        .pr_2()
                        .text_size(px(11.5))
                        .text_color(theme.text)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.open_path(path.clone(), Some(line_number), Some(window), cx);
                        }))
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
        let summary = self
            .search
            .summary
            .as_ref()
            .map(|summary| {
                format!(
                    "{} matches in {} files · {} ms{}",
                    summary.matches,
                    summary.files,
                    summary.elapsed_ms,
                    if summary.limit_hit { " · limit" } else { "" }
                )
            })
            .unwrap_or_else(|| {
                if self.search.running {
                    "Searching…".into()
                } else {
                    "Search project files".into()
                }
            });
        let error = self.search.error.clone();
        let mut content = div().size_full().flex().flex_col();
        content = content.child(
            div()
                .p_2()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .h(px(30.))
                        .w_full()
                        .px_2()
                        .rounded_md()
                        .bg(theme.window)
                        .border_1()
                        .border_color(theme.border)
                        .child(self.search_input.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(self.toggle_chip(
                            "Aa",
                            self.search.case_sensitive,
                            theme,
                            cx,
                            |this, cx| {
                                this.search.case_sensitive = !this.search.case_sensitive;
                                this.schedule_search(cx);
                            },
                        ))
                        .child(
                            self.toggle_chip(".*", self.search.regex, theme, cx, |this, cx| {
                                this.search.regex = !this.search.regex;
                                this.schedule_search(cx);
                            }),
                        )
                        .child(self.toggle_chip(
                            "W",
                            self.search.whole_word,
                            theme,
                            cx,
                            |this, cx| {
                                this.search.whole_word = !this.search.whole_word;
                                this.schedule_search(cx);
                            },
                        ))
                        .child(self.toggle_chip(
                            "⇄",
                            self.search.replace_mode,
                            theme,
                            cx,
                            |this, _cx| {
                                this.search.replace_mode = !this.search.replace_mode;
                            },
                        ))
                        .child(div().flex_1())
                        .child(
                            self.dialog_button("Replace", theme, cx, |this, _window, cx| {
                                this.replace_all(cx)
                            }),
                        ),
                )
                .when(self.search.replace_mode, |element| {
                    element.child(
                        div()
                            .h(px(30.))
                            .w_full()
                            .px_2()
                            .rounded_md()
                            .bg(theme.window)
                            .border_1()
                            .border_color(theme.border)
                            .child(self.search_replace_input.clone()),
                    )
                }),
        );
        if let Some(error) = error {
            content = content.child(
                div()
                    .px_3()
                    .py_1()
                    .text_color(theme.error)
                    .text_size(px(11.))
                    .child(error),
            );
        }
        content
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_color(theme.faint)
                    .text_size(px(10.5))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(summary),
            )
            .child(
                div()
                    .id("search-results")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(results),
            )
            .into_any_element()
    }

    fn render_db_panel(&self, theme: Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut sources = Vec::new();
        for (index, source) in self.db_sources.iter().enumerate() {
            let selected = self.db_selected.as_deref() == Some(source.id.as_str());
            let id = source.id.clone();
            sources.push(
                div()
                    .id(("db-source", index))
                    .h(px(24.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(if selected {
                        theme.accent_muted
                    } else {
                        theme.panel
                    })
                    .text_size(px(11.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.db_selected = Some(id.clone());
                        this.db_tables.clear();
                        this.db_schema = None;
                        this.db_result.clear();
                        cx.notify();
                    }))
                    .child("▤")
                    .child(div().flex_1().truncate().child(source.name.clone()))
                    .child(
                        div()
                            .text_color(theme.faint)
                            .text_size(px(10.))
                            .child(format!("{:?}", source.driver).to_lowercase()),
                    ),
            );
        }
        let mut tables = Vec::new();
        for (index, table) in self.db_tables.iter().take(500).enumerate() {
            let name = table.clone();
            let display_name = name.clone();
            tables.push(
                div()
                    .id(("db-table", index))
                    .h(px(21.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .text_color(theme.muted)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        let query = format!(
                            "SELECT * FROM {}{} LIMIT 200",
                            if let Some(schema) = this.db_schema.as_deref() {
                                if schema == "main" || schema == "public" {
                                    String::new()
                                } else {
                                    format!("{schema}.")
                                }
                            } else {
                                String::new()
                            },
                            name
                        );
                        this.db_query_input
                            .update(cx, |input, cx| input.set_content(query, cx));
                        this.run_db_query(cx);
                    }))
                    .child("▸")
                    .child(div().flex_1().truncate().child(display_name)),
            );
        }
        let result = terminal_display_lines(&self.db_result);
        let rows: Vec<gpui::AnyElement> = result
            .into_iter()
            .take(2000)
            .map(|line| {
                div()
                    .h(px(16.))
                    .text_size(px(10.5))
                    .text_color(theme.text)
                    .whitespace_nowrap()
                    .child(line)
                    .into_any_element()
            })
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(self.dialog_button(
                                "Add SQLite",
                                theme,
                                cx,
                                |this, window, cx| this.add_sqlite_source(window, cx),
                            ))
                            .child(
                                self.dialog_button("Connect", theme, cx, |this, _window, cx| {
                                    this.connect_selected_db(cx)
                                }),
                            )
                            .child(self.dialog_button(
                                "Refresh",
                                theme,
                                cx,
                                |this, _window, cx| this.load_db_sources(cx),
                            )),
                    )
                    .child(
                        div()
                            .h(px(74.))
                            .id("db-sources")
                            .flex_none()
                            .overflow_y_scroll()
                            .children(sources),
                    )
                    .when_some(self.db_status.clone(), |element, status| {
                        element.child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme.faint)
                                .child(status),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(130.))
                    .id("db-tables")
                    .flex_none()
                    .border_t_1()
                    .border_color(theme.border)
                    .overflow_y_scroll()
                    .children(tables),
            )
            .child(
                div()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .h(px(28.))
                            .px_2()
                            .rounded_md()
                            .bg(theme.window)
                            .border_1()
                            .border_color(theme.border)
                            .child(self.db_query_input.clone()),
                    )
                    .child(
                        self.dialog_button("Run Query", theme, cx, |this, _window, cx| {
                            this.run_db_query(cx)
                        }),
                    ),
            )
            .child(
                div()
                    .id("db-result")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p_2()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_http_panel(&self, theme: Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"];
        let mut method_buttons = Vec::new();
        for method in methods {
            let active = self.http_method == method;
            method_buttons.push(
                div()
                    .id(SharedString::from(format!("http-method-{method}")))
                    .h(px(22.))
                    .px_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(if active { theme.accent } else { theme.border })
                    .bg(if active {
                        theme.accent_muted
                    } else {
                        theme.panel_alt
                    })
                    .text_color(if active { theme.accent } else { theme.faint })
                    .text_size(px(10.5))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.http_method = method.into();
                        cx.notify();
                    }))
                    .child(method),
            );
        }
        let response = terminal_display_lines(&self.http_response);
        let rows: Vec<gpui::AnyElement> = response
            .into_iter()
            .take(2000)
            .map(|line| {
                div()
                    .h(px(16.))
                    .text_size(px(10.5))
                    .text_color(theme.text)
                    .whitespace_nowrap()
                    .child(line)
                    .into_any_element()
            })
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().flex().items_center().gap_1().children(method_buttons))
                    .child(
                        div()
                            .h(px(30.))
                            .px_2()
                            .rounded_md()
                            .bg(theme.window)
                            .border_1()
                            .border_color(theme.border)
                            .child(self.http_url_input.clone()),
                    )
                    .child(
                        div()
                            .h(px(30.))
                            .px_2()
                            .rounded_md()
                            .bg(theme.window)
                            .border_1()
                            .border_color(theme.border)
                            .child(self.http_headers_input.clone()),
                    )
                    .child(
                        div()
                            .h(px(30.))
                            .px_2()
                            .rounded_md()
                            .bg(theme.window)
                            .border_1()
                            .border_color(theme.border)
                            .child(self.http_body_input.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.dialog_button("Send", theme, cx, |this, _window, cx| {
                                this.send_http_request(cx)
                            }))
                            .child(div().flex_1())
                            .when_some(self.http_status.clone(), |element, status| {
                                element.child(
                                    div()
                                        .text_size(px(10.5))
                                        .text_color(theme.accent)
                                        .child(status),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(px(10.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.faint)
                    .child("RESPONSE"),
            )
            .child(
                div()
                    .id("http-response")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p_2()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_git_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let branch = self
            .git
            .branch
            .as_ref()
            .and_then(|branch| branch.head.clone())
            .unwrap_or_else(|| "(detached HEAD)".into());
        let upstream = self
            .git
            .branch
            .as_ref()
            .map(|branch| {
                format!(
                    "{}{}",
                    branch
                        .upstream
                        .as_ref()
                        .map(|upstream| format!(" ⇢ {upstream}"))
                        .unwrap_or_default(),
                    if branch.ahead + branch.behind > 0 {
                        format!(" ↑{} ↓{}", branch.ahead, branch.behind)
                    } else {
                        String::new()
                    }
                )
            })
            .unwrap_or_default();
        let mut changes = Vec::new();
        for (index, status) in self.git.status.iter().take(2000).enumerate() {
            let path = status.path.0.clone();
            let staged = status.staged.is_some();
            let subject = self.git.selected.as_ref() == Some(&path);
            let code = status
                .unstaged
                .or(status.staged)
                .map(|kind| format!("{kind:?}"))
                .unwrap_or_default();
            changes.push(
                div()
                    .id(("git-change", index))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .text_size(px(11.5))
                    .text_color(if subject { theme.text } else { theme.muted })
                    .bg(if subject {
                        theme.panel_alt
                    } else {
                        theme.panel
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.git.selected = Some(path.clone());
                        this.open_git_diff(path.clone(), staged, cx);
                        window.focus(&this.focus_handle.clone());
                    }))
                    .child(
                        div()
                            .w(px(16.))
                            .text_color(if staged { theme.accent } else { theme.warning })
                            .child(code.chars().next().unwrap_or('·').to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(status.rel_path.clone()),
                    ),
            );
        }
        let mut logs = Vec::new();
        for (index, commit) in self.git.log.iter().take(20).enumerate() {
            logs.push(
                div()
                    .id(("git-log", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(px(11.))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_color(theme.accent)
                                    .child(commit.short_id.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .truncate()
                                    .text_color(theme.text)
                                    .child(commit.summary.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_color(theme.faint)
                            .child(format!("{} · {}", commit.author, commit.short_id)),
                    ),
            );
        }
        let error = self.git.error.clone();
        let diff = self.git.diff_preview.clone();
        let mut panel = div().size_full().flex().flex_col().child(
            div()
                .p_2()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(12.))
                        .child(div().text_color(theme.accent).child("⑂"))
                        .child(
                            div()
                                .flex_1()
                                .truncate()
                                .text_color(theme.text)
                                .child(branch),
                        )
                        .child(div().text_color(theme.faint).child(upstream)),
                )
                .child(
                    div()
                        .h(px(52.))
                        .w_full()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(theme.window)
                        .border_1()
                        .border_color(theme.border)
                        .child(self.git_message_input.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            self.dialog_button("Stage All", theme, cx, |this, _window, cx| {
                                this.stage_all(cx)
                            }),
                        )
                        .child(
                            self.dialog_button("Unstage", theme, cx, |this, _window, cx| {
                                this.unstage_all(cx)
                            }),
                        )
                        .child(
                            self.dialog_button("Discard", theme, cx, |this, window, cx| {
                                this.discard_all(window, cx)
                            }),
                        )
                        .child(
                            self.dialog_button("Commit", theme, cx, |this, _window, cx| {
                                this.commit_all(cx)
                            }),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_color(theme.faint)
                                .text_size(px(10.5))
                                .child(format!("{} changes", self.git.status.len())),
                        ),
                ),
        );
        if let Some(error) = error {
            panel = panel.child(
                div()
                    .px_3()
                    .text_color(theme.error)
                    .text_size(px(11.))
                    .child(error),
            );
        }
        if let Some((path, text)) = diff {
            panel = panel.child(
                div()
                    .px_2()
                    .py_1()
                    .bg(theme.window)
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted)
                            .child(path.display().to_string()),
                    )
                    .child(
                        div()
                            .id("git-diff")
                            .h(px(180.))
                            .overflow_y_scroll()
                            .text_size(px(10.5))
                            .children(text.lines().take(400).map(|line| {
                                let color = if line.starts_with('+') && !line.starts_with("+++") {
                                    theme.accent
                                } else if line.starts_with('-') && !line.starts_with("---") {
                                    theme.error
                                } else if line.starts_with("@@") {
                                    theme.faint
                                } else {
                                    theme.muted
                                };
                                div().text_color(color).child(line.to_string())
                            })),
                    ),
            );
        }
        panel
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_size(px(10.5))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.faint)
                    .child("CHANGES"),
            )
            .child(
                div()
                    .id("git-changes")
                    .h(px(220.))
                    .overflow_y_scroll()
                    .children(changes),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_size(px(10.5))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.faint)
                    .child("LOG"),
            )
            .child(
                div()
                    .id("git-log-list")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(logs),
            )
            .into_any_element()
    }

    fn render_structure_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = Vec::new();
        if let Some(tab) = self.active_tab() {
            let buffer = tab.buffer.read(cx);
            let path = buffer.path().map(|path| path.display().to_string());
            if let Some(path) = path {
                rows.push(
                    div()
                        .id("structure-path")
                        .px_2()
                        .py_1()
                        .text_size(px(11.))
                        .text_color(theme.faint)
                        .child(path),
                );
            }
            for (line_index, line) in buffer.text().lines().enumerate() {
                let trimmed = line.trim_start();
                let indentation = line.len() - trimmed.len();
                let interesting = trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("struct ")
                    || trimmed.starts_with("pub struct ")
                    || trimmed.starts_with("enum ")
                    || trimmed.starts_with("pub enum ")
                    || trimmed.starts_with("trait ")
                    || trimmed.starts_with("impl ")
                    || trimmed.starts_with("class ")
                    || trimmed.starts_with("interface ")
                    || trimmed.starts_with("function ")
                    || trimmed.starts_with("def ")
                    || trimmed.starts_with("export ")
                    || trimmed.starts_with("const ")
                    || trimmed.starts_with("static ")
                    || trimmed.starts_with("type ")
                    || trimmed.starts_with("mod ")
                    || (trimmed.ends_with('{') && !trimmed.starts_with("//"));
                if !interesting || trimmed.is_empty() || trimmed.starts_with("//") {
                    continue;
                }
                if rows.len() > 1200 {
                    break;
                }
                let label: String = trimmed.chars().take(80).collect();
                let line_number = (line_index + 1) as u32;
                rows.push(
                    div()
                        .id(("structure", line_index))
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .pl(px(8.0 + (indentation.min(40) as f32) * 0.5))
                        .pr_2()
                        .text_size(px(11.5))
                        .text_color(theme.text)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.panel_alt))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            if let Some(tab) = this.active_tab() {
                                tab.editor
                                    .update(cx, |editor, cx| editor.go_to_line(line_number, cx));
                                let focus = this.groups[this.active_group].tabs
                                    [this.groups[this.active_group].active.unwrap_or(0)]
                                .editor
                                .read(cx)
                                .focus_handle(cx);
                                window.focus(&focus);
                            }
                        }))
                        .child(
                            div()
                                .w(px(30.))
                                .text_color(theme.faint)
                                .child(line_number.to_string()),
                        )
                        .child(div().flex_1().truncate().child(label)),
                );
            }
        }
        if rows.is_empty() {
            return div()
                .p_3()
                .text_size(px(12.))
                .text_color(theme.faint)
                .child("Open a source file to see its structure.")
                .into_any_element();
        }
        div()
            .id("structure-list")
            .size_full()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }

    fn render_problems_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = Vec::new();
        for (index, problem) in self.problems.iter().take(2000).enumerate() {
            let severity = problem.diagnostic.severity;
            let color = match severity {
                Severity::Error => theme.error,
                Severity::Warning => theme.warning,
                Severity::Info => theme.accent,
                Severity::Hint => theme.faint,
            };
            let icon = match severity {
                Severity::Error => "✖",
                Severity::Warning => "⚠",
                Severity::Info => "ℹ",
                Severity::Hint => "·",
            };
            let name = problem
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| problem.path.display().to_string());
            let line = problem.diagnostic.range.start.line + 1;
            rows.push(
                div()
                    .id(("problem", index))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_start()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(px(11.))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.open_problem(index, window, cx);
                    }))
                    .child(div().text_color(color).child(icon))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(
                                div()
                                    .text_color(theme.text)
                                    .child(problem.diagnostic.message.clone()),
                            )
                            .child(
                                div()
                                    .text_color(theme.faint)
                                    .child(format!("{name}:{line} · {}", problem.source)),
                            ),
                    ),
            );
        }
        if rows.is_empty() {
            return div()
                .p_3()
                .text_size(px(12.))
                .text_color(theme.faint)
                .child("No problems reported for this project.")
                .into_any_element();
        }
        div()
            .id("problems-list")
            .size_full()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }

    fn render_settings_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let filter = self.settings_filter.to_lowercase();
        let mut leaves = Vec::new();
        if let Ok(value) = serde_json::to_value(&self.settings) {
            flatten_json("", &value, &mut leaves);
        }
        let mut rows = Vec::new();
        for (key, value) in leaves {
            if !filter.is_empty() && !key.to_lowercase().contains(&filter) {
                continue;
            }
            if rows.len() > 1500 {
                break;
            }
            let key_for_toggle = key.clone();
            let bool_value = value.as_bool();
            rows.push(
                div()
                    .id(SharedString::from(format!("setting-{key}")))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(px(11.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_color(theme.muted)
                            .child(key),
                    )
                    .child(match bool_value {
                        Some(enabled) => div()
                            .id(SharedString::from(format!(
                                "setting-toggle-{key_for_toggle}"
                            )))
                            .w(px(26.))
                            .h(px(14.))
                            .rounded_full()
                            .bg(if enabled {
                                theme.accent_muted
                            } else {
                                theme.panel_alt
                            })
                            .border_1()
                            .border_color(if enabled { theme.accent } else { theme.border })
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                this.set_setting(
                                    key_for_toggle.clone(),
                                    serde_json::Value::Bool(!enabled),
                                    cx,
                                );
                            }))
                            .child(
                                div()
                                    .m(px(1.))
                                    .w(px(11.))
                                    .h(px(11.))
                                    .rounded_full()
                                    .bg(if enabled { theme.accent } else { theme.faint }),
                            )
                            .into_any_element(),
                        None => div()
                            .max_w(px(180.))
                            .truncate()
                            .text_color(theme.faint)
                            .child(display_json_value(&value))
                            .into_any_element(),
                    }),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div().p_2().child(
                    div()
                        .h(px(30.))
                        .w_full()
                        .px_2()
                        .rounded_md()
                        .bg(theme.window)
                        .border_1()
                        .border_color(theme.border)
                        .child(self.settings_input.clone()),
                ),
            )
            .child(
                div()
                    .id("settings-list")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_placeholder_panel(
        &self,
        title: &'static str,
        description: &'static str,
        button: &'static str,
        theme: Theme,
        cx: &mut Context<Self>,
        action: fn(&mut RustIdea, &mut Window, &mut Context<RustIdea>),
    ) -> gpui::AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .p_3()
            .gap_3()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.muted)
                    .child(description),
            )
            .child(
                div()
                    .id("placeholder-action")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme.accent_muted)
                    .text_color(theme.accent)
                    .text_size(px(12.))
                    .cursor_pointer()
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, window, cx| {
                            action(this, window, cx)
                        }),
                    )
                    .child(button),
            )
            .into_any_element()
    }

    fn dialog_button(
        &self,
        label: &'static str,
        theme: Theme,
        cx: &mut Context<Self>,
        handler: fn(&mut RustIdea, &mut Window, &mut Context<RustIdea>),
    ) -> impl IntoElement + use<> {
        div()
            .id(SharedString::from(format!("dialog-button-{label}")))
            .h(px(26.))
            .px_2()
            .rounded_md()
            .bg(theme.panel_alt)
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .text_size(px(11.5))
            .text_color(theme.muted)
            .cursor_pointer()
            .hover(|style| style.bg(theme.accent_muted).text_color(theme.text))
            .on_click(
                cx.listener(move |this, _: &ClickEvent, window, cx| handler(this, window, cx)),
            )
            .child(label)
    }

    fn toggle_chip(
        &self,
        label: &'static str,
        active: bool,
        theme: Theme,
        cx: &mut Context<Self>,
        handler: fn(&mut RustIdea, &mut Context<RustIdea>),
    ) -> impl IntoElement + use<> {
        div()
            .id(SharedString::from(format!("chip-{label}")))
            .h(px(22.))
            .px_1()
            .rounded_sm()
            .border_1()
            .border_color(if active { theme.accent } else { theme.border })
            .bg(if active {
                theme.accent_muted
            } else {
                theme.panel_alt
            })
            .text_color(if active { theme.accent } else { theme.faint })
            .text_size(px(11.))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| handler(this, cx)))
            .child(label)
    }

    fn render_editor_area(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut groups = Vec::new();
        for (group_index, group) in self.groups.iter().enumerate() {
            let active_group = group_index == self.active_group;
            let mut tabs = Vec::new();
            for (tab_index, tab) in group.tabs.iter().enumerate() {
                let active = group.active == Some(tab_index);
                let dirty = tab.buffer.read(cx).is_dirty();
                let saving = tab.buffer.read(cx).is_saving();
                let label = tab.buffer.read(cx).title();
                tabs.push(
                    div()
                        .id(("editor-tab", group_index * 1000 + tab_index))
                        .h_full()
                        .max_w(px(240.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .border_r_1()
                        .border_color(theme.border)
                        .bg(if active {
                            theme.tab_active
                        } else {
                            theme.panel
                        })
                        .text_color(if active { theme.text } else { theme.muted })
                        .text_size(px(12.))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.active_group = group_index;
                            this.activate_tab(tab_index, window, cx);
                        }))
                        .child(
                            div()
                                .w(px(12.))
                                .flex_none()
                                .text_size(px(10.))
                                .text_color(if dirty { theme.accent } else { theme.faint })
                                .child(if saving {
                                    "◌".to_string()
                                } else if dirty {
                                    "●".to_string()
                                } else {
                                    String::new()
                                }),
                        )
                        .child(div().flex_1().min_w(px(0.)).truncate().child(label))
                        .child(
                            div()
                                .id(("close-tab", group_index * 1000 + tab_index))
                                .w(px(16.))
                                .h(px(16.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .text_color(theme.faint)
                                .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    this.active_group = group_index;
                                    this.close_tab(tab_index, window, cx);
                                }))
                                .child("×"),
                        ),
                );
            }
            let mut content = div().flex_1().min_h(px(0.)).overflow_hidden();
            if let Some(active) = group.active {
                if let Some(tab) = group.tabs.get(active) {
                    if let Some(message) = tab.buffer.read(cx).external_change_message() {
                        let buffer = tab.buffer.clone();
                        let reload_buffer = buffer.clone();
                        let keep_buffer = buffer.clone();
                        content = content
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex_none()
                                    .px_3()
                                    .py_2()
                                    .bg(theme.warning.opacity(0.16))
                                    .border_b_1()
                                    .border_color(theme.warning)
                                    .text_color(theme.warning)
                                    .text_size(px(12.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .child(div().flex_1().child(message))
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id("reload-external")
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_sm()
                                                    .bg(theme.panel_alt)
                                                    .text_color(theme.text)
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, _window, cx| {
                                                            reload_buffer.update(
                                                                cx,
                                                                |buffer, cx| {
                                                                    buffer.reload_after_conflict(cx)
                                                                },
                                                            );
                                                            this.status =
                                                                "Reloaded the disk version".into();
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .child("Reload from disk"),
                                            )
                                            .child(
                                                div()
                                                    .id("keep-external")
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_sm()
                                                    .bg(theme.panel_alt)
                                                    .text_color(theme.text)
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, _window, cx| {
                                                            keep_buffer.update(cx, |buffer, cx| {
                                                                buffer.keep_local_after_conflict(cx)
                                                            });
                                                            this.status =
                                                                "Keeping the editor version".into();
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .child("Keep my version"),
                                            ),
                                    ),
                            )
                            .child(tab.editor.clone());
                    } else {
                        content = content.child(tab.editor.clone());
                    }
                } else {
                    content = content.child(self.render_empty_editor(cx));
                }
            } else {
                content = content.child(self.render_empty_editor(cx));
            }
            groups.push(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .border_r_1()
                    .border_color(theme.border)
                    .when(!active_group, |element| element)
                    .child(
                        div()
                            .id("group-tab-strip")
                            .h(px(32.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .bg(theme.panel)
                            .border_b_1()
                            .border_color(theme.border)
                            .overflow_x_scroll()
                            .child(div().flex().h_full().min_w(px(0.)).children(tabs))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id(("group-menu", group_index))
                                    .px_2()
                                    .text_color(theme.faint)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.panel_alt).text_color(theme.text))
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            this.active_group = group_index;
                                            this.close_group(cx);
                                            window.focus(&this.focus_handle.clone());
                                        },
                                    ))
                                    .child("✕"),
                            ),
                    )
                    .child(content),
            );
        }
        let stacked = self.split_axis == SplitAxis::Vertical && self.groups.len() > 1;
        let mut container = div().flex_1().min_w(px(0.)).h_full().bg(theme.window);
        container = if stacked {
            container.flex().flex_col()
        } else {
            container.flex()
        };
        container.children(groups)
    }

    fn render_empty_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .text_color(theme.muted)
            .child(div().text_size(px(38.)).text_color(theme.faint).child("◈"))
            .child(
                div()
                    .text_size(px(13.))
                    .child("Open a file or folder to start editing"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        self.dialog_button("Open File", theme, cx, |this, window, cx| {
                            this.open_file_prompt(window, cx)
                        }),
                    )
                    .child(
                        self.dialog_button("Open Folder", theme, cx, |this, window, cx| {
                            this.open_folder_prompt(window, cx)
                        }),
                    ),
            )
    }

    fn render_bottom_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let kinds = [
            BottomKind::Terminal,
            BottomKind::Problems,
            BottomKind::Run,
            BottomKind::Debug,
            BottomKind::Build,
            BottomKind::VersionControl,
            BottomKind::Output,
        ];
        let mut tabs = Vec::new();
        for kind in kinds {
            let active = self.bottom_kind == kind;
            let label = match kind {
                BottomKind::Problems => {
                    format!("{} ({})", kind.title(), self.problems.len())
                }
                _ => kind.title().to_string(),
            };
            tabs.push(
                div()
                    .id(SharedString::from(format!("bottom-tab-{}", kind.title())))
                    .h_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(if active { theme.accent } else { theme.panel })
                    .text_color(if active { theme.text } else { theme.muted })
                    .text_size(px(11.5))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.show_bottom(kind, cx);
                        window.focus(&this.focus_handle.clone());
                    }))
                    .child(label),
            );
        }
        let content: gpui::AnyElement = match self.bottom_kind {
            BottomKind::Problems => self.render_bottom_problems(cx),
            BottomKind::Terminal => self.render_bottom_terminal(cx),
            BottomKind::Run => self.render_bottom_run(cx),
            BottomKind::Debug => self.render_bottom_debug(cx),
            BottomKind::Build => self.render_bottom_build(cx),
            BottomKind::VersionControl => self.render_bottom_vcs(cx),
            BottomKind::Output => self.render_bottom_output(cx),
        };
        div()
            .h(px(self.bottom_height))
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .h(px(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().flex().h_full().children(tabs))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("bottom-close")
                            .px_2()
                            .text_color(theme.faint)
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.toggle_bottom(cx)
                            }))
                            .child("×"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .child(content),
            )
    }

    fn render_bottom_problems(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut rows = Vec::new();
        for (index, problem) in self.problems.iter().take(1000).enumerate() {
            let severity = problem.diagnostic.severity;
            let color = match severity {
                Severity::Error => theme.error,
                Severity::Warning => theme.warning,
                Severity::Info => theme.accent,
                Severity::Hint => theme.faint,
            };
            rows.push(
                div()
                    .id(("bottom-problem", index))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .text_size(px(11.5))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.open_problem(index, window, cx);
                    }))
                    .child(
                        div()
                            .text_color(color)
                            .child(format!("{:?}", severity).to_uppercase()),
                    )
                    .child(
                        div()
                            .text_color(theme.text)
                            .child(problem.diagnostic.message.clone()),
                    )
                    .child(
                        div()
                            .text_color(theme.faint)
                            .child(problem.path.display().to_string()),
                    ),
            );
        }
        if rows.is_empty() {
            return div()
                .p_3()
                .text_color(theme.faint)
                .text_size(px(12.))
                .child("No diagnostics from language servers or the syntax engine.")
                .into_any_element();
        }
        div()
            .id("bottom-problems-list")
            .size_full()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }

    fn render_bottom_terminal(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        if self.terminals.is_empty() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme.faint)
                        .child("No terminal session is running."),
                )
                .child(
                    self.dialog_button("New Terminal", theme, cx, |this, window, cx| {
                        this.new_terminal(window, cx)
                    }),
                )
                .into_any_element();
        }
        let mut tabs = Vec::new();
        for (index, terminal) in self.terminals.iter().enumerate() {
            let active = index == self.active_terminal;
            let title = terminal.info.title.clone();
            tabs.push(
                div()
                    .id(("terminal-tab", index))
                    .h_full()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_r_1()
                    .border_color(theme.border)
                    .bg(if active { theme.window } else { theme.panel })
                    .text_color(if active { theme.text } else { theme.muted })
                    .text_size(px(11.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.active_terminal = index;
                        cx.notify();
                    }))
                    .child(title)
                    .child(
                        div()
                            .id(("terminal-close", index))
                            .w(px(14.))
                            .text_color(theme.faint)
                            .hover(|style| style.text_color(theme.error))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                this.close_terminal(index, cx);
                            }))
                            .child("×"),
                    ),
            );
        }
        let active = self.terminals.get(self.active_terminal);
        let output = active
            .map(|terminal| terminal.output.as_str())
            .unwrap_or_default();
        let lines = terminal_display_lines(output);
        let output_rows: Vec<gpui::AnyElement> = lines
            .into_iter()
            .map(|line| {
                div()
                    .h(px(17.))
                    .text_color(theme.text)
                    .text_size(px(11.5))
                    .whitespace_nowrap()
                    .child(line)
                    .into_any_element()
            })
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(px(11.))
                    .text_color(theme.faint)
                    .child(div().flex().h_full().overflow_hidden().children(tabs))
                    .child(div().flex_1())
                    .child(
                        self.dialog_button("New Terminal", theme, cx, |this, window, cx| {
                            this.new_terminal(window, cx)
                        }),
                    ),
            )
            .child(
                div()
                    .id("terminal-output")
                    .flex_1()
                    .min_h(px(0.))
                    .bg(theme.window)
                    .p_2()
                    .text_size(px(11.5))
                    .overflow_y_scroll()
                    .children(output_rows),
            )
            .child(
                div()
                    .h(px(28.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.window)
                    .text_color(theme.accent)
                    .text_size(px(12.))
                    .child("❯")
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .child(self.terminal_input.clone()),
                    ),
            )
            .into_any_element()
    }

    fn render_bottom_run(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut configs = Vec::new();
        for (index, config) in self.run_configs.iter().take(40).enumerate() {
            let selected = index == self.selected_run;
            configs.push(
                div()
                    .id(("run-config", index))
                    .h(px(23.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(if selected {
                        theme.accent_muted
                    } else {
                        theme.panel
                    })
                    .text_size(px(11.5))
                    .text_color(if selected { theme.text } else { theme.muted })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.panel_alt))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.selected_run = index;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(14.))
                            .text_color(if selected { theme.accent } else { theme.faint })
                            .child(if selected { "▶" } else { "·" }),
                    )
                    .child(div().flex_1().truncate().child(config.name.clone()))
                    .child(div().text_color(theme.faint).child(config.kind.clone())),
            );
        }
        let output = terminal_display_lines(&self.run_output);
        let rows: Vec<gpui::AnyElement> = output
            .into_iter()
            .map(|line| {
                div()
                    .h(px(16.))
                    .text_size(px(11.))
                    .text_color(theme.muted)
                    .whitespace_nowrap()
                    .child(line)
                    .into_any_element()
            })
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        self.dialog_button("Refresh", theme, cx, |this, _window, cx| {
                            this.load_run_configs(cx)
                        }),
                    )
                    .child(self.dialog_button("Run", theme, cx, |this, window, cx| {
                        this.run_active_config(window, cx)
                    }))
                    .child(
                        self.dialog_button("Stop", theme, cx, |this, _window, cx| {
                            this.stop_run(cx)
                        }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_color(theme.faint)
                            .text_size(px(10.5))
                            .child(format!("{} configurations", self.run_configs.len())),
                    ),
            )
            .child(
                div()
                    .id("run-config-list")
                    .h(px(96.))
                    .flex_none()
                    .overflow_y_scroll()
                    .children(configs),
            )
            .child(
                div()
                    .id("run-output")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .bg(theme.window)
                    .p_2()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_bottom_debug(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let mut processes = Vec::new();
        for (index, process) in self.debug_attachables.iter().take(80).enumerate() {
            processes.push(
                div()
                    .id(("debug-process", index))
                    .h(px(22.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .text_color(theme.muted)
                    .border_b_1()
                    .border_color(theme.border)
                    .child("●")
                    .child(div().flex_1().truncate().child(process.clone())),
            );
        }
        let configs: Vec<gpui::AnyElement> = self
            .run_configs
            .iter()
            .filter(|config| config.debug_adapter.is_some())
            .take(30)
            .map(|config| {
                div()
                    .h(px(22.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .text_color(theme.text)
                    .child("◇")
                    .child(config.name.clone())
                    .into_any_element()
            })
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.dialog_button(
                        "Refresh Sessions",
                        theme,
                        cx,
                        |this, _window, cx| this.refresh_debug(cx),
                    ))
                    .child(
                        self.dialog_button("Load Configs", theme, cx, |this, _window, cx| {
                            this.load_run_configs(cx)
                        }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_color(theme.faint)
                            .text_size(px(10.5))
                            .child(format!("{} attachable", self.debug_attachables.len())),
                    ),
            )
            .child(
                div()
                    .id("debug-list")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(configs)
                    .children(processes),
            )
            .into_any_element()
    }

    fn render_bottom_build(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let output = terminal_display_lines(&self.build_output);
        let rows: Vec<gpui::AnyElement> = output
            .into_iter()
            .map(|line| {
                div()
                    .h(px(16.))
                    .text_size(px(11.))
                    .text_color(theme.muted)
                    .whitespace_nowrap()
                    .child(line)
                    .into_any_element()
            })
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        self.dialog_button("Build Project", theme, cx, |this, window, cx| {
                            this.build_project(window, cx)
                        }),
                    )
                    .child(div().flex_1())
                    .child(
                        div().text_color(theme.faint).text_size(px(10.5)).child(
                            self.build_summary
                                .clone()
                                .unwrap_or_else(|| "Build model not loaded".into()),
                        ),
                    ),
            )
            .child(
                div()
                    .id("build-output")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p_2()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_bottom_vcs(&self, _cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let statuses: Vec<String> = self
            .git
            .status
            .iter()
            .take(300)
            .map(|status| {
                format!(
                    "{:?}  {}",
                    status.staged.or(status.unstaged),
                    status.rel_path
                )
            })
            .collect();
        div()
            .size_full()
            .p_2()
            .flex()
            .flex_col()
            .gap(px(1.))
            .text_size(px(11.))
            .text_color(theme.muted)
            .children(statuses.into_iter().map(|status| div().child(status)))
            .into_any_element()
    }

    fn render_bottom_output(&self, _cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        div()
            .size_full()
            .p_2()
            .text_size(px(11.))
            .text_color(theme.muted)
            .child(self.status.clone())
            .into_any_element()
    }

    fn render_right_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .w(px(self.right_width))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_l_1()
            .border_color(theme.border)
            .child(
                div()
                    .h(px(34.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.faint)
                    .child("AGENTS & TOOLS")
                    .child(
                        div()
                            .id("right-close")
                            .cursor_pointer()
                            .hover(|style| style.text_color(theme.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| this.toggle_right(cx)))
                            .child("×"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .text_size(px(12.))
                    .text_color(theme.muted)
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("Agent workspace"),
                    )
                    .child(
                        div()
                                    .italic()
                                    .child("Select Agents in the activity rail to create a session, attach the project and stream replies."),
                    )
                    .child(
                        div()
                            .pt_2()
                            .text_size(px(11.))
                            .text_color(theme.faint)
                            .child(format!(
                                "{} open documents · {} indexes",
                                self.groups.iter().map(|group| group.tabs.len()).sum::<usize>(),
                                self.server_statuses.len()
                            )),
                    ),
            )
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let active = self.active_tab().map(|tab| {
            let editor = tab.editor.read(cx);
            let buffer = tab.buffer.read(cx);
            (
                editor.cursor_position(cx),
                buffer.line_count(),
                buffer.language().to_string(),
                buffer.is_dirty(),
                buffer.error().map(str::to_owned),
                buffer.path().map(|path| path.display().to_string()),
                buffer.is_large(),
            )
        });
        let (line, column, language, dirty, error, path, large) = active
            .map(|active| {
                (
                    active.0.0,
                    active.0.1,
                    active.1.to_string(),
                    active.3,
                    active.4,
                    active.5,
                    active.6,
                )
            })
            .unwrap_or((1, 1, "Plain text".into(), false, None, None, false));
        let _ = language;
        let branch = self
            .git
            .branch
            .as_ref()
            .and_then(|branch| branch.head.clone())
            .unwrap_or_else(|| "no git".into());
        let trust = self
            .project
            .as_ref()
            .map(|project| {
                if project.info.trusted {
                    "Trusted"
                } else {
                    "Restricted"
                }
            })
            .unwrap_or("No project");
        let lsp = if self.server_statuses.is_empty() {
            "LSP: idle".to_string()
        } else {
            let ready = self
                .server_statuses
                .values()
                .filter(|(status, _)| matches!(status, ServerStatus::Ready))
                .count();
            format!("LSP: {ready}/{} ready", self.server_statuses.len())
        };
        let document_language = self
            .active_tab()
            .map(|tab| tab.buffer.read(cx).language().to_string())
            .unwrap_or_else(|| "Plain text".into());
        let error_text = error.unwrap_or_default();
        div()
            .h(px(26.))
            .flex_none()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .bg(theme.titlebar)
            .border_t_1()
            .border_color(theme.border)
            .text_size(px(10.5))
            .text_color(theme.faint)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(format!("Ln {line}, Col {column}"))
                    .child(format!(
                        "{} lines",
                        self.active_tab()
                            .map(|tab| tab.buffer.read(cx).line_count())
                            .unwrap_or(0)
                    ))
                    .when(dirty, |element| {
                        element.child(div().text_color(theme.accent).child("● modified"))
                    })
                    .when(large, |element| {
                        element.child(div().text_color(theme.warning).child("large file mode"))
                    }),
            )
            .child(div().flex_1())
            .when(!error_text.is_empty(), |element| {
                element.child(div().text_color(theme.error).truncate().child(error_text))
            })
            .child(div().child(format!("{} · {}", document_language, self.status)))
            .child(div().child(branch))
            .child(div().child(trust))
            .child(div().child(lsp))
            .when(path.is_some(), |element| {
                element.child(
                    div()
                        .max_w(px(300.))
                        .truncate()
                        .child(path.unwrap_or_default()),
                )
            })
    }

    fn render_overlay(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = self.theme;
        let Some(overlay) = self.overlay.as_ref() else {
            return div().into_any_element();
        };
        let mut items = Vec::new();
        for (index, item) in overlay.items.iter().take(12).enumerate() {
            let selected = index == overlay.selected;
            let label = item.title.clone();
            let detail = item.detail.clone();
            let item_index = index;
            items.push(
                div()
                    .id(("overlay-item", index))
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .bg(if selected {
                        theme.accent_muted
                    } else {
                        theme.panel
                    })
                    .text_color(if selected { theme.text } else { theme.muted })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if let Some(overlay) = this.overlay.as_mut() {
                            overlay.selected = item_index;
                        }
                        this.accept_overlay(cx);
                        window.focus(&this.focus_handle.clone());
                    }))
                    .child(div().flex_1().min_w(px(0.)).truncate().child(label))
                    .child(
                        div()
                            .max_w(px(260.))
                            .truncate()
                            .text_size(px(10.5))
                            .child(detail),
                    ),
            );
        }
        if items.is_empty() {
            items.push(
                div()
                    .id("overlay-empty")
                    .px_3()
                    .py_3()
                    .text_color(theme.faint)
                    .child("No matches"),
            );
        }
        let kind_label = match overlay.kind {
            OverlayKind::CommandPalette => "COMMANDS",
            OverlayKind::QuickOpen => "OPEN FILE",
            OverlayKind::SearchEverywhere => "SEARCH EVERYWHERE",
            OverlayKind::GoToLine => "GO TO LINE",
        };
        div()
            .absolute()
            .top(px(76.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .w(px(640.))
                    .max_h(px(440.))
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .bg(theme.panel)
                    .border_1()
                    .border_color(theme.accent)
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(38.))
                            .px_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.accent)
                                    .child(kind_label),
                            )
                            .child(div().flex_1().min_w(px(0.)).child(overlay.input.clone())),
                    )
                    .child(
                        div()
                            .id("overlay-items")
                            .max_h(px(360.))
                            .overflow_y_scroll()
                            .children(items),
                    )
                    .when_some(overlay.error.clone(), |element, error| {
                        element.child(
                            div()
                                .px_3()
                                .py_2()
                                .text_color(theme.error)
                                .text_size(px(11.))
                                .child(error),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_sizes(&self) -> (f32, f32, f32) {
        (self.sidebar_width, self.right_width, self.bottom_height)
    }

    fn render_context_menu(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        #[derive(Clone, Copy)]
        enum ContextAction {
            Open,
            NewFile,
            NewFolder,
            Rename,
            MoveToTrash,
            Reveal,
            CopyPath,
        }

        let theme = self.theme;
        let Some(menu) = self.context_menu.clone() else {
            return div().into_any_element();
        };
        let path = menu.path.clone();
        let directory = if menu.is_directory {
            path.clone()
        } else {
            path.parent().unwrap_or(Path::new(".")).to_path_buf()
        };
        let is_directory = menu.is_directory;
        let mut items = Vec::new();
        let mut add = |label: &'static str, enabled: bool, action: ContextAction| {
            let path = path.clone();
            let directory = directory.clone();
            items.push(
                div()
                    .id(SharedString::from(format!("ctx-{label}")))
                    .h(px(25.))
                    .px_3()
                    .flex()
                    .items_center()
                    .text_size(px(11.5))
                    .text_color(if enabled { theme.text } else { theme.faint })
                    .when(enabled, |element| {
                        element
                            .cursor_pointer()
                            .hover(|style| style.bg(theme.accent_muted))
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if !enabled {
                            return;
                        }
                        this.context_menu = None;
                        match action {
                            ContextAction::Open => {
                                if is_directory {
                                    let mut load = None;
                                    if let Some(project) = this.project.as_mut()
                                        && !project.tree.collapse(&path)
                                        && project.tree.expand(&path)
                                    {
                                        load = Some(path.clone());
                                    }
                                    match load {
                                        Some(directory) => {
                                            this.load_project_directory(directory, cx)
                                        }
                                        None => cx.notify(),
                                    }
                                } else {
                                    this.open_path(path.clone(), None, Some(window), cx);
                                }
                            }
                            ContextAction::NewFile => {
                                this.create_child_path(directory.clone(), false, window, cx)
                            }
                            ContextAction::NewFolder => {
                                this.create_child_path(directory.clone(), true, window, cx)
                            }
                            ContextAction::Rename => this.rename_path(path.clone(), window, cx),
                            ContextAction::MoveToTrash => {
                                this.delete_path(path.clone(), window, cx)
                            }
                            ContextAction::Reveal => this.reveal_path(path.clone(), cx),
                            ContextAction::CopyPath => {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    path.display().to_string(),
                                ));
                                this.status = "Copied path".into();
                            }
                        }
                        cx.notify();
                    }))
                    .child(label),
            );
        };
        add(
            if is_directory { "Open" } else { "Open File" },
            true,
            ContextAction::Open,
        );
        add("New File", true, ContextAction::NewFile);
        add("New Folder", true, ContextAction::NewFolder);
        add("Rename", true, ContextAction::Rename);
        add("Move to Trash", true, ContextAction::MoveToTrash);
        add("Reveal in Finder", true, ContextAction::Reveal);
        add("Copy Path", true, ContextAction::CopyPath);
        div()
            .absolute()
            .left(menu.position.x)
            .top(menu.position.y)
            .w(px(210.))
            .rounded_md()
            .bg(theme.panel)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .p_1()
            .flex()
            .flex_col()
            .children(items)
            .into_any_element()
    }
}

impl Render for RustIdea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let viewport = window.viewport_size();
        let viewport_width = f32::from(viewport.width);
        let viewport_height = f32::from(viewport.height);
        let (sidebar_width, right_width, bottom_height) = self.render_sizes();
        let _ = (sidebar_width, right_width, bottom_height);
        let body = div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .child(self.render_activity_rail(cx))
            .when(self.sidebar_visible, |element| {
                element.child(self.render_sidebar(cx)).child(
                    div()
                        .id("sidebar-resize")
                        .w(px(4.))
                        .flex_none()
                        .h_full()
                        .cursor(gpui::CursorStyle::ResizeLeftRight)
                        .bg(theme.panel)
                        .hover(|style| style.bg(theme.accent))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _event: &gpui::MouseDownEvent, _window, cx| {
                                this.drag = Some(DragState {
                                    left_width: None,
                                    right_width: None,
                                    bottom_height: None,
                                });
                                cx.notify();
                            }),
                        )
                        .on_mouse_move(cx.listener(
                            move |this, event: &MouseMoveEvent, _window, cx| {
                                if let Some(drag) = this.drag.as_mut() {
                                    drag.left_width = Some(event.position.x - px(46.0));
                                    this.sidebar_width =
                                        (f32::from(event.position.x) - 46.0).clamp(180.0, 520.0);
                                    cx.notify();
                                }
                            },
                        ))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                this.drag = None;
                                cx.notify();
                            }),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(self.render_editor_area(window, cx))
                    .when(self.bottom_visible, |element| {
                        element
                            .child(
                                div()
                                    .id("bottom-resize")
                                    .h(px(4.))
                                    .flex_none()
                                    .w_full()
                                    .cursor(gpui::CursorStyle::ResizeUpDown)
                                    .bg(theme.panel)
                                    .hover(|style| style.bg(theme.accent))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            |this, _event: &gpui::MouseDownEvent, _window, cx| {
                                                this.drag = Some(DragState {
                                                    left_width: None,
                                                    right_width: None,
                                                    bottom_height: None,
                                                });
                                                cx.notify();
                                            },
                                        ),
                                    )
                                    .on_mouse_move(cx.listener(
                                        move |this, event: &MouseMoveEvent, window, cx| {
                                            if let Some(drag) = this.drag.as_mut() {
                                                let height = viewport_height
                                                    - f32::from(event.position.y)
                                                    - 26.0;
                                                drag.bottom_height = Some(px(height.max(80.0)));
                                                this.bottom_height =
                                                    height.clamp(80.0, viewport_height - 160.0);
                                                let _ = window;
                                                cx.notify();
                                            }
                                        },
                                    ))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                            this.drag = None;
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(self.render_bottom_panel(cx))
                    }),
            )
            .when(self.right_visible, |element| {
                element
                    .child(
                        div()
                            .id("right-resize")
                            .w(px(4.))
                            .flex_none()
                            .h_full()
                            .cursor(gpui::CursorStyle::ResizeLeftRight)
                            .bg(theme.panel)
                            .hover(|style| style.bg(theme.accent))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _event: &gpui::MouseDownEvent, _window, cx| {
                                    this.drag = Some(DragState {
                                        left_width: None,
                                        right_width: None,
                                        bottom_height: None,
                                    });
                                    cx.notify();
                                }),
                            )
                            .on_mouse_move(cx.listener(
                                move |this, event: &MouseMoveEvent, _window, cx| {
                                    if let Some(drag) = this.drag.as_mut() {
                                        let width = viewport_width - f32::from(event.position.x);
                                        drag.right_width = Some(px(width));
                                        this.right_width = width.clamp(220.0, 560.0);
                                        cx.notify();
                                    }
                                },
                            ))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                    this.drag = None;
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(self.render_right_panel(cx))
            });

        let context_menu = self
            .context_menu
            .as_ref()
            .map(|_| self.render_context_menu(cx));
        let mut root = div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.window)
            .text_color(theme.text)
            .text_size(px(13.))
            .key_context("RustIdea")
            .track_focus(&self.focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseDownEvent, _window, cx| {
                    if this.context_menu.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_action(cx.listener(|_this, _: &Quit, _window, cx| cx.quit()))
            .on_action(cx.listener(|this, _: &NewFile, window, cx| this.new_file(window, cx)))
            .on_action(
                cx.listener(|this, _: &OpenFile, window, cx| this.open_file_prompt(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &OpenFolder, window, cx| this.open_folder_prompt(window, cx)),
            )
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save_active(window, cx)))
            .on_action(cx.listener(|this, _: &SaveAll, _window, cx| this.save_all(cx)))
            .on_action(cx.listener(|this, _: &SaveAs, window, cx| this.save_active_as(window, cx)))
            .on_action(
                cx.listener(|this, _: &CloseTab, window, cx| this.close_active_tab(window, cx)),
            )
            .on_action(cx.listener(|this, _: &NextTab, window, cx| this.next_tab(window, cx)))
            .on_action(
                cx.listener(|this, _: &PreviousTab, window, cx| this.previous_tab(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleTheme, _window, cx| this.toggle_theme(cx)))
            .on_action(
                cx.listener(|this, _: &ToggleAutosave, _window, cx| this.toggle_autosave(cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleLeftPanel, _window, cx| this.toggle_sidebar(cx)),
            )
            .on_action(cx.listener(|this, _: &ToggleRightPanel, _window, cx| this.toggle_right(cx)))
            .on_action(
                cx.listener(|this, _: &ToggleBottomPanel, _window, cx| this.toggle_bottom(cx)),
            )
            .on_action(cx.listener(|this, _: &ShowProject, _window, cx| {
                this.show_sidebar(SidebarKind::Project, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowSearch, _window, cx| {
                this.show_sidebar(SidebarKind::Search, cx)
            }))
            .on_action(
                cx.listener(|this, _: &ShowGit, _window, cx| {
                    this.show_sidebar(SidebarKind::Git, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ShowStructure, _window, cx| {
                this.show_sidebar(SidebarKind::Structure, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowProblems, _window, cx| {
                this.show_sidebar(SidebarKind::Problems, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowSettings, _window, cx| {
                this.show_sidebar(SidebarKind::Settings, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowTerminal, _window, cx| {
                this.show_bottom(BottomKind::Terminal, cx)
            }))
            .on_action(
                cx.listener(|this, _: &ShowRun, _window, cx| this.show_bottom(BottomKind::Run, cx)),
            )
            .on_action(cx.listener(|this, _: &ShowDebug, _window, cx| {
                this.show_bottom(BottomKind::Debug, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowDatabase, _window, cx| {
                this.show_sidebar(SidebarKind::Database, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowAgents, _window, cx| {
                this.show_sidebar(SidebarKind::Agents, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowPlugins, _window, cx| {
                this.show_sidebar(SidebarKind::Plugins, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowRemote, _window, cx| {
                this.show_sidebar(SidebarKind::Remote, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowTools, _window, cx| {
                this.show_sidebar(SidebarKind::Tools, cx)
            }))
            .on_action(cx.listener(|this, _: &CommandPalette, window, cx| {
                this.open_command_palette(window, OverlayKind::CommandPalette, cx)
            }))
            .on_action(cx.listener(|this, _: &QuickOpen, window, cx| {
                this.open_command_palette(window, OverlayKind::QuickOpen, cx)
            }))
            .on_action(cx.listener(|this, _: &SearchEverywhere, window, cx| {
                this.open_command_palette(window, OverlayKind::SearchEverywhere, cx)
            }))
            .on_action(cx.listener(|this, _: &SplitRight, window, cx| {
                this.split_active(SplitAxis::Horizontal, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SplitDown, window, cx| {
                this.split_active(SplitAxis::Vertical, window, cx)
            }))
            .on_action(cx.listener(|this, _: &CloseGroup, _window, cx| this.close_group(cx)))
            .on_action(
                cx.listener(|this, _: &RefreshProject, _window, cx| this.refresh_project(cx)),
            )
            .on_action(cx.listener(|this, _: &TrustProject, _window, cx| this.trust_project(cx)))
            .on_action(cx.listener(|this, _: &GitRefresh, _window, cx| this.refresh_git(cx)))
            .on_action(cx.listener(|this, _: &GitCommit, _window, cx| this.commit_all(cx)))
            .on_action(
                cx.listener(|this, _: &NewTerminal, window, cx| this.new_terminal(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &GoToDefinition, window, cx| {
                    this.go_to_definition(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &GoToLine, window, cx| this.go_to_line_prompt(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FormatDocument, _window, cx| {
                    this.format_active_document(cx)
                }),
            )
            .child(self.render_titlebar(cx))
            .child(body)
            .child(self.render_status_bar(cx));
        if let Some(menu) = context_menu {
            root = root.child(menu);
        }
        let overlay = self.overlay.as_ref().map(|_| self.render_overlay(cx));
        if let Some(overlay) = overlay {
            root = root.child(overlay);
        }
        root
    }
}

fn flatten_json(
    prefix: &str,
    value: &serde_json::Value,
    output: &mut Vec<(String, serde_json::Value)>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_json(&path, value, output);
            }
        }
        serde_json::Value::Array(values) if values.len() <= 4 => {
            output.push((prefix.to_string(), value.clone()));
        }
        serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_)
        | serde_json::Value::Array(_) => {
            output.push((prefix.to_string(), value.clone()));
        }
        _ => {}
    }
}

fn display_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => {
            if value.is_empty() {
                "(empty)".into()
            } else {
                value.clone()
            }
        }
        serde_json::Value::Null => "null".into(),
        value => value.to_string(),
    }
}

/// Minimal terminal screen model for shell output. Escape sequences are
/// consumed instead of printed; carriage returns, backspaces and the common
/// erase-line/screen commands are applied. Full VT colour is intentionally
/// not faked with literal escape text.
fn terminal_display_lines(output: &str) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut chars = output.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\x1b' => match chars.next() {
                Some('[') => {
                    let mut last = '\0';
                    for code in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&code) {
                            last = code;
                            break;
                        }
                    }
                    match last {
                        'K' => {
                            if let Some(line) = lines.last_mut() {
                                line.clear();
                            }
                        }
                        'J' => {
                            lines.clear();
                            lines.push(String::new());
                        }
                        _ => {}
                    }
                }
                Some(']') => {
                    let mut previous = '\0';
                    for code in chars.by_ref() {
                        if code == '\x07' || (previous == '\x1b' && code == '\\') {
                            break;
                        }
                        previous = code;
                    }
                }
                Some(_) | None => {}
            },
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                    lines.push(String::new());
                } else if let Some(line) = lines.last_mut() {
                    line.clear();
                }
            }
            '\n' => lines.push(String::new()),
            '\u{8}' => {
                if let Some(line) = lines.last_mut() {
                    line.pop();
                }
            }
            '\t' => {
                if let Some(line) = lines.last_mut() {
                    line.push_str("    ");
                }
            }
            '\x07' | '\x00'..='\x08' | '\x0b'..='\x1f' => {}
            character => {
                if let Some(line) = lines.last_mut() {
                    line.push(character);
                }
            }
        }
    }
    if lines.len() > 5000 {
        lines.drain(..lines.len() - 5000);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::terminal_display_lines;

    #[test]
    fn terminal_consumes_ansi_and_applies_carriage_returns() {
        assert_eq!(
            terminal_display_lines("\x1b]0;title\x07\x1b[31mred\x1b[0m\rnext\n"),
            ["next", ""]
        );
        assert_eq!(
            terminal_display_lines("progress 10%\rprogress 20%"),
            ["progress 20%"]
        );
    }

    #[test]
    fn terminal_applies_backspace_erase_line_and_tabs() {
        assert_eq!(terminal_display_lines("abc\u{8}d"), ["abd"]);
        assert_eq!(terminal_display_lines("old\x1b[2Knew"), ["new"]);
        assert_eq!(terminal_display_lines("a\tb"), ["a    b"]);
    }

    #[test]
    fn terminal_output_keeps_the_last_five_thousand_lines() {
        let output = (0..5_010)
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        let lines = terminal_display_lines(&output);
        assert_eq!(lines.len(), 5_000);
        assert_eq!(lines.first().map(String::as_str), Some("11"));
    }
}
