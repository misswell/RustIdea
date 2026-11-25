mod backend;
mod completion;
mod semantics;

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use backend::OpenIdeaBridge;
use completion::OpenIdeaCompletionProvider;
use semantics::OpenIdeaSemanticsProvider;
use editor::Editor;
use ide_core::IdeEvent;
use gpui::{
    App, AppContext as _, Bounds, Context, Focusable as _, KeyBinding, Subscription, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_platform::application;
use language::{
    Buffer, BufferEvent, Diagnostic, DiagnosticEntry, DiagnosticMessage, DiagnosticSet,
};
use settings::SettingsStore;
use theme::LoadThemes;

actions!(rustidea_poc, [SaveDocument]);

struct PocApp {
    editor: gpui::Entity<Editor>,
    second_editor: Option<gpui::Entity<Editor>>,
    buffer: gpui::Entity<Buffer>,
    backend: Arc<OpenIdeaBridge>,
    backend_version: u32,
    last_clock: clock::Global,
    confirmed_clock: clock::Global,
    syncing: bool,
    applying_backend_patch: bool,
    save_requested: bool,
    status: gpui::SharedString,
    _subscriptions: Vec<Subscription>,
}

impl PocApp {
    fn new(
        backend: Arc<OpenIdeaBridge>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = backend.text.clone();
        let buffer = cx.new(|cx| Buffer::local(text, cx));
        let version = buffer.read(cx).snapshot().version().clone();
        let editor = cx.new(|cx| Editor::for_buffer(buffer.clone(), None, window, cx));
        let provider = OpenIdeaCompletionProvider::new(backend.clone());
        let semantics = OpenIdeaSemanticsProvider::new(backend.clone());
        editor.update(cx, |editor, _| {
            editor.set_completion_provider(Some(Rc::new(provider)));
            editor.set_semantics_provider(Some(Rc::new(semantics)));
        });
        let second_editor = std::env::var_os("RUSTIDEA_POC_MULTIVIEW").map(|_| {
            let editor = cx.new(|cx| Editor::for_buffer(buffer.clone(), None, window, cx));
            let provider = OpenIdeaCompletionProvider::new(backend.clone());
            let semantics = OpenIdeaSemanticsProvider::new(backend.clone());
            editor.update(cx, |editor, _| {
                editor.set_completion_provider(Some(Rc::new(provider)));
                editor.set_semantics_provider(Some(Rc::new(semantics)));
            });
            editor
        });
        if let Some(mut events) = backend.take_events() {
            cx.spawn(async move |this, cx| {
                while let Some(event) = events.recv().await {
                    this.update(cx, |app, cx| app.handle_event(event, cx)).ok();
                }
            })
            .detach();
        }
        let subscription = cx.subscribe(&buffer, |app, _, event: &BufferEvent, cx| {
            if matches!(event, BufferEvent::Edited { .. }) && !app.applying_backend_patch {
                app.sync_edits(cx);
            }
        });
        window.focus(&editor.read(cx).focus_handle(cx), cx);
        let status = format!(
            "Zed buffer connected to OpenIDEA document {}",
            backend.path.display()
        );
        Self {
            editor,
            second_editor,
            buffer,
            backend_version: backend.version,
            last_clock: version.clone(),
            confirmed_clock: version,
            backend,
            syncing: false,
            applying_backend_patch: false,
            save_requested: false,
            status: status.into(),
            _subscriptions: vec![subscription],
        }
    }

    fn sync_edits(&mut self, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let snapshot = self.buffer.read(cx).snapshot();
        let edits: Vec<text::Edit<text::OffsetUtf16>> = snapshot
            .edits_since::<text::OffsetUtf16>(&self.last_clock)
            .collect();
        if edits.is_empty() {
            return;
        }
        let changes = edits
            .iter()
            .map(|edit| ide_core::TextChange {
                from: edit.old.start.0 as u32,
                to: edit.old.end.0 as u32,
                insert: snapshot
                    .text_for_range(
                        snapshot.offset_utf16_to_offset(edit.new.start)
                            ..snapshot.offset_utf16_to_offset(edit.new.end),
                    )
                    .collect(),
            })
            .collect();
        let request_clock = snapshot.version().clone();
        self.last_clock = request_clock.clone();
        self.syncing = true;
        let base_version = self.backend_version;
        let backend = self.backend.clone();
        cx.spawn(async move |this, cx| {
            let result = backend
                .edit(base_version, ide_core::ChangeSet { changes })
                .await;
            this.update(cx, |app, cx| {
                app.syncing = false;
                match result {
                    Ok(version) => {
                        app.backend_version = version;
                        app.confirmed_clock = request_clock.clone();
                        let current = app.buffer.read(cx).snapshot().version().clone();
                        if current != request_clock {
                            app.sync_edits(cx);
                        } else if app.save_requested {
                            app.save_requested = false;
                            app.save(cx);
                        }
                    }
                    Err(error) => {
                        app.status = format!("OpenIDEA edit failed: {}", error.message).into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn handle_event(&mut self, event: IdeEvent, cx: &mut Context<Self>) {
        if let IdeEvent::DocPatched {
            doc,
            base_version: _,
            version,
            changes,
            ..
        } = event
        {
            if doc != self.backend.doc() {
                return;
            }
            let snapshot = self.buffer.read(cx).snapshot();
            if snapshot.version() != &self.confirmed_clock {
                eprintln!("POC_EXTERNAL_CONFLICT local edits kept");
                self.status = "External change arrived while editing; local text kept".into();
                cx.notify();
                return;
            }
            let edits: Vec<(std::ops::Range<usize>, String)> = changes
                .iter()
                .map(|change| {
                    (
                        snapshot.offset_utf16_to_offset(text::OffsetUtf16(change.from as usize))
                            ..snapshot
                                .offset_utf16_to_offset(text::OffsetUtf16(change.to as usize)),
                        change.insert.clone(),
                    )
                })
                .collect();
            self.applying_backend_patch = true;
            self.buffer.update(cx, |buffer, cx| {
                buffer.edit(edits, None, cx);
            });
            self.applying_backend_patch = false;
            self.backend_version = version;
            self.confirmed_clock = self.buffer.read(cx).snapshot().version().clone();
            self.last_clock = self.confirmed_clock.clone();
            self.status = "Applied external OpenIDEA document patch".into();
            eprintln!("POC_EXTERNAL_PATCH version={version}");
            cx.notify();
            return;
        }
        let IdeEvent::Diagnostics {
            path,
            source,
            items,
        } = event
        else {
            return;
        };
        if path.0 != self.backend.path {
            return;
        }
        let snapshot = self.buffer.read(cx).snapshot();
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
                let mut zed = Diagnostic::default();
                zed.source = Some(source.clone());
                zed.severity = match diagnostic.severity {
                    ide_core::Severity::Error => lsp::DiagnosticSeverity::ERROR,
                    ide_core::Severity::Warning => lsp::DiagnosticSeverity::WARNING,
                    ide_core::Severity::Info => lsp::DiagnosticSeverity::INFORMATION,
                    ide_core::Severity::Hint => lsp::DiagnosticSeverity::HINT,
                };
                zed.message = DiagnosticMessage::plain(diagnostic.message);
                DiagnosticEntry::new(range, zed)
            })
            .collect();
        let set = DiagnosticSet::new(entries, &snapshot);
        eprintln!(
            "POC_DIAGNOSTICS source={} count={}",
            source,
            set.len()
        );
        self.buffer.update(cx, |buffer, cx| {
            buffer.update_diagnostics(lsp::LanguageServerId(1), set, cx);
        });
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if self.syncing {
            self.save_requested = true;
            self.status = "Waiting for pending edits before save…".into();
            cx.notify();
            return;
        }
        let version = self.buffer.read(cx).snapshot().version().clone();
        let backend = self.backend.clone();
        let buffer = self.buffer.clone();
        self.status = "Saving through the OpenIDEA Host…".into();
        cx.spawn(async move |this, cx| {
            let result = backend.save().await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(()) => {
                        buffer.update(cx, |buffer, cx| {
                            buffer.did_save(version, None, cx);
                        });
                        app.status = "Saved through the OpenIDEA Host".into();
                    }
                    Err(error) => {
                        app.status = format!("OpenIDEA save failed: {}", error.message).into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

impl Drop for PocApp {
    fn drop(&mut self) {
        self.backend.close();
    }
}

impl Render for PocApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_action(cx.listener(|this, _: &SaveDocument, _window, cx| {
                this.save(cx);
            }))
            .child(
                div()
                    .h(px(24.))
                    .px_3()
                    .flex()
                    .items_center()
                    .text_size(px(11.))
                    .text_color(gpui::rgb(0x9b9ea9))
                    .child(self.status.clone()),
            )
            .child(if let Some(second) = self.second_editor.clone() {
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .child(div().flex_1().h_full().child(self.editor.clone()))
                    .child(div().flex_1().h_full().child(second))
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.editor.clone())
                    .into_any_element()
            })
    }
}

fn main() {
    if std::env::var_os("RUSTIDEA_POC_LSP_TEST").is_some() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(std::io::stderr)
            .try_init();
    }
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.."));
    let file = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("README.md"));
    let mut backend = OpenIdeaBridge::start(&root, &file)
        .unwrap_or_else(|error| panic!("open {} through OpenIDEA: {error:#}", file.display()));
    if std::env::var_os("RUSTIDEA_POC_LSP_TEST").is_some() {
        let text = backend.text.clone();
        eprintln!(
            "POC_LSP_DOC language={} version={}",
            backend.language, backend.version
        );
        let (line, column) = std::env::var("RUSTIDEA_POC_LSP_POS")
            .ok()
            .and_then(|position| {
                let (line, column) = position.split_once(':')?;
                Some((line.parse().ok()?, column.parse().ok()?))
            })
            .unwrap_or_else(|| {
                let last_line = text.lines().last().unwrap_or_default();
                (
                    text.lines().count().saturating_sub(1) as u32,
                    last_line.encode_utf16().count() as u32,
                )
            });
        eprintln!("POC_LSP request line={line} column={column}");
        let mut stable_ready = 0;
        for attempt in 0..90 {
            if let Ok(servers) = backend.servers() {
                if attempt % 5 == 0 {
                    for server in &servers {
                        eprintln!("POC_LSP_SERVER {:?}", server);
                    }
                }
                let ready = !servers.is_empty()
                    && servers.iter().all(|server| {
                        server.status == ide_core::event::ServerStatus::Ready
                    });
                if ready {
                    stable_ready += 1;
                    if stable_ready >= 3 {
                        break;
                    }
                } else {
                    stable_ready = 0;
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        std::thread::sleep(std::time::Duration::from_secs(20));
        match backend.reopen_blocking() {
            Ok(info) => eprintln!(
                "POC_LSP_REOPEN version={} language={}",
                info.version, info.language
            ),
            Err(error) => eprintln!("POC_LSP_REOPEN_ERROR {}", error.message),
        }
        let hover_position = std::env::var("RUSTIDEA_POC_HOVER_POS")
            .ok()
            .and_then(|position| {
                let (line, column) = position.split_once(':')?;
                Some((line.parse().ok()?, column.parse().ok()?))
            })
            .unwrap_or((1, 16));
        match backend.hover_blocking(hover_position.0, hover_position.1) {
            Ok(Some(hover)) => eprintln!("POC_LSP_HOVER {:?}", hover.contents.value),
            Ok(None) => eprintln!("POC_LSP_HOVER none"),
            Err(error) => eprintln!("POC_LSP_HOVER_ERROR {}", error.message),
        }
        match backend.definition_blocking(
            line,
            column.saturating_sub(5),
            ide_lsp::types::GotoKind::Definition,
        ) {
            Ok(locations) => {
                eprintln!("POC_LSP_DEFINITIONS count={}", locations.len());
                for location in locations.iter().take(4) {
                    eprintln!(
                        "POC_LSP_DEFINITION {} {}:{}",
                        location.path.0.display(),
                        location.range.start.line,
                        location.range.start.col
                    );
                }
            }
            Err(error) => eprintln!("POC_LSP_DEFINITION_ERROR {}", error.message),
        }
        match backend.prepare_rename_blocking(line, 0) {
            Ok(range) => eprintln!("POC_LSP_PREPARE_RENAME {:?}", range),
            Err(error) => eprintln!("POC_LSP_PREPARE_RENAME_ERROR {}", error.message),
        }
        if let Ok(new_name) = std::env::var("RUSTIDEA_POC_RENAME_TO") {
            match backend.rename_blocking(line, 0, new_name.clone()) {
                Ok(()) => eprintln!("POC_LSP_RENAME_OK {new_name}"),
                Err(error) => eprintln!("POC_LSP_RENAME_ERROR {}", error.message),
            }
        }
        for _ in 0..30 {
            match backend.completion_blocking(line, column, Some(".".into())) {
                Ok(list) if !list.items.is_empty() => {
                    eprintln!("POC_LSP_OK count={}", list.items.len());
                    for item in list.items.iter().take(8) {
                        eprintln!("POC_LSP_ITEM {}", item.label);
                    }
                    return;
                }
                Ok(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
                Err(error) => {
                    eprintln!("POC_LSP_FAILED {}", error.message);
                    return;
                }
            }
        }
        eprintln!("POC_LSP_FAILED no completions after retries");
        return;
    }
    let backend = Arc::new(backend);

    application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            if let Err(error) = assets::Assets.load_fonts(cx) {
                panic!("load editor fonts: {error:#}");
            }
            let settings_store = SettingsStore::new(cx, &settings::default_settings());
            cx.set_global(settings_store);
            theme_settings::init(LoadThemes::All(Box::new(assets::Assets)), cx);
            release_channel::init(semver::Version::new(0, 0, 0), cx);
            editor::init(cx);
            cx.bind_keys([KeyBinding::new("cmd-s", SaveDocument, None)]);

            let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    |window, cx| {
                        let backend = backend.clone();
                        cx.new(|cx| PocApp::new(backend, window, cx))
                    },
                )
                .expect("open Zed editor window");
            if std::env::var_os("RUSTIDEA_POC_COMPLETION_DEMO").is_some() {
                window
                    .update(cx, |_app, window, cx| {
                        cx.spawn_in(window, async move |this, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(15))
                                .await;
                            this.update_in(cx, |app, window, cx| {
                                let len = app.buffer.read(cx).snapshot().len();
                                app.editor.update(cx, |editor, cx| {
                                    editor.change_selections(
                                        editor::SelectionEffects::no_scroll(),
                                        window,
                                        cx,
                                        |selections| {
                                            selections.select_ranges([
                                                multi_buffer::MultiBufferOffset(len)
                                                    ..multi_buffer::MultiBufferOffset(len),
                                            ])
                                        },
                                    );
                                    editor.open_completions_menu(window, cx);
                                });
                            })
                            .ok();
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(10))
                                .await;
                            this.update(cx, |_app, cx| cx.quit()).ok();
                        })
                        .detach();
                    })
                    .expect("run POC completion demo");
            }
            if let Ok(marker) = std::env::var("RUSTIDEA_POC_DEMO_EDIT") {
                window
                    .update(cx, |app, window, cx| {
                        app.buffer.update(cx, |buffer, cx| {
                            buffer.edit([(0..0, format!("{marker}\n"))], None, cx);
                        });
                        let marker = marker.clone();
                        cx.spawn_in(window, async move |this, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(2))
                                .await;
                            this.update(cx, |app, cx| app.save(cx)).ok();
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(2))
                                .await;
                            this.update(cx, |app, cx| {
                                if app.status.contains("Saved through the OpenIDEA Host") {
                                    eprintln!("POC_SAVE_OK {marker}");
                                } else {
                                    eprintln!("POC_SAVE_FAILED {}", app.status);
                                }
                                if let Some(second) = app.second_editor.clone() {
                                    let first_text = app
                                        .editor
                                        .read(cx)
                                        .buffer()
                                        .read(cx)
                                        .snapshot(cx)
                                        .text();
                                    let second_text = second
                                        .read(cx)
                                        .buffer()
                                        .read(cx)
                                        .snapshot(cx)
                                        .text();
                                    eprintln!(
                                        "POC_MULTIVIEW equal={} first_len={} second_len={}",
                                        first_text == second_text,
                                        first_text.len(),
                                        second_text.len()
                                    );
                                }
                                cx.quit();
                            })
                            .ok();
                        })
                        .detach();
                    })
                    .expect("run POC demo edit");
            }
            window
                .update(cx, |_, window, _| window.activate_window())
                .expect("activate window");
            cx.activate(true);
        });
}
