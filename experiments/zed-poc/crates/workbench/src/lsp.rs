use std::path::PathBuf;
use std::sync::Arc;

use ide_core::{DocId, IdeResult, Location, RequestId};
use ide_lsp::types::{
    CodeAction, CompletionItem, CompletionList, GotoKind, Highlight, Hover, InlayHint, ServerInfo,
};
use ide_workspace::DocumentInfo;

use crate::host::{MAIN_WINDOW, NativeHost};

pub struct DocumentLsp {
    host: Arc<NativeHost>,
    doc: DocId,
    pub path: PathBuf,
}

pub struct RenameOutcome {
    pub files: usize,
    pub paths: Vec<PathBuf>,
}

impl DocumentLsp {
    pub fn new(host: Arc<NativeHost>, doc: DocId, path: PathBuf) -> Self {
        Self { host, doc, path }
    }

    pub fn doc(&self) -> DocId {
        self.doc
    }

    pub fn servers(&self) -> IdeResult<Vec<ServerInfo>> {
        let project = self.host.host().project(MAIN_WINDOW)?;
        Ok(project.lsp.servers())
    }

    pub async fn completion(
        &self,
        line: u32,
        column: u32,
        trigger: Option<String>,
    ) -> IdeResult<CompletionList> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                ide_host::ops::lsp::lsp_completion(
                    &backend,
                    MAIN_WINDOW,
                    RequestId::next(),
                    doc,
                    ide_core::Pos::new(line, column),
                    trigger,
                )
                .await
            })
            .await
    }

    pub async fn open_target(&self, path: PathBuf) -> IdeResult<DocumentInfo> {
        self.host.open_document(&path).await
    }

    pub async fn resolve_completion(&self, item: CompletionItem) -> IdeResult<CompletionItem> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                ide_host::ops::lsp::lsp_resolve_completion(&backend, MAIN_WINDOW, doc, item).await
            })
            .await
    }

    pub async fn hover(&self, line: u32, column: u32) -> IdeResult<Option<Hover>> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                ide_host::ops::lsp::lsp_hover(
                    &backend,
                    MAIN_WINDOW,
                    RequestId::next(),
                    doc,
                    ide_core::Pos::new(line, column),
                )
                .await
            })
            .await
    }

    pub async fn definition(
        &self,
        line: u32,
        column: u32,
        kind: GotoKind,
    ) -> IdeResult<Vec<Location>> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                ide_host::ops::lsp::lsp_goto(
                    &backend,
                    MAIN_WINDOW,
                    doc,
                    ide_core::Pos::new(line, column),
                    kind,
                )
                .await
            })
            .await
    }

    pub async fn document_highlights(&self, line: u32, column: u32) -> IdeResult<Vec<Highlight>> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                ide_host::ops::lsp::lsp_document_highlights(
                    &backend,
                    MAIN_WINDOW,
                    doc,
                    ide_core::Pos::new(line, column),
                )
                .await
            })
            .await
    }

    pub async fn inlay_hints(&self, range: ide_core::Range) -> IdeResult<Vec<InlayHint>> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                ide_host::ops::lsp::lsp_inlay_hints(&backend, MAIN_WINDOW, doc, range).await
            })
            .await
    }

    pub async fn prepare_rename(
        &self,
        line: u32,
        column: u32,
    ) -> IdeResult<Option<ide_core::Range>> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                ide_host::ops::lsp::lsp_prepare_rename(
                    &backend,
                    MAIN_WINDOW,
                    doc,
                    ide_core::Pos::new(line, column),
                )
                .await
            })
            .await
    }

    pub async fn rename(
        &self,
        line: u32,
        column: u32,
        new_name: String,
    ) -> IdeResult<RenameOutcome> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                let renamed = ide_host::ops::lsp::lsp_rename(
                    &backend,
                    MAIN_WINDOW,
                    RequestId::next(),
                    doc,
                    ide_core::Pos::new(line, column),
                    new_name,
                )
                .await?;
                let files = renamed.edit.files.len();
                let mut paths = Vec::new();
                for file in &renamed.edit.files {
                    match file {
                        ide_core::FileEdit::Edit { path, .. }
                        | ide_core::FileEdit::Create { path, .. }
                        | ide_core::FileEdit::Delete { path, .. } => {
                            paths.push(path.0.clone());
                        }
                        ide_core::FileEdit::Rename { from, to } => {
                            paths.push(from.0.clone());
                            paths.push(to.0.clone());
                        }
                    }
                }
                paths.sort();
                paths.dedup();
                ide_host::ops::lsp::apply_workspace_edit(&backend, MAIN_WINDOW, renamed.edit)
                    .await?;
                Ok(RenameOutcome { files, paths })
            })
            .await
    }

    pub async fn code_actions(&self, range: ide_core::Range) -> IdeResult<Vec<CodeAction>> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                ide_host::ops::lsp::lsp_code_actions(&backend, MAIN_WINDOW, doc, range, Vec::new())
                    .await
            })
            .await
    }

    pub async fn apply_code_action(&self, action: CodeAction) -> IdeResult<()> {
        let doc = self.doc;
        self.host
            .run(move |backend| async move {
                ide_host::ops::lsp::lsp_apply_code_action(&backend, MAIN_WINDOW, doc, action).await
            })
            .await
    }
}
