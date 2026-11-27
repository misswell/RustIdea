//! The OpenIDEA editor colour scheme, expressed independently of Zed.
//!
//! Zed owns text layout, selection, folding and input. RustIdea still has to
//! paint the same editor palette OpenIDEA uses. The native workbench translates
//! this struct into Zed's `theme::Theme`/`EditorStyle`; keeping the mapping here
//! means the editor colours are tested as design data, not buried in GPUI code.

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::semantic::Semantics;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditorTheme {
    pub background: Color,
    pub foreground: Color,
    pub current_line: Color,
    pub selection: Color,
    pub selection_inactive: Color,
    pub caret: Color,
    pub gutter_background: Color,
    pub line_number: Color,
    pub line_number_current: Color,
    pub active_line_number: Color,
    pub active_line_number_background: Color,
    pub indent_guide: Color,
    pub indent_guide_active: Color,
    pub fold_marker: Color,
    pub fold_placeholder: Color,
    pub whitespace: Color,
    pub inlay_background: Color,
    pub inlay_foreground: Color,
    pub search_match: Color,
    pub search_match_current: Color,
    pub search_result: Color,
    pub matched_brace: Color,
    pub identifier_under_caret: Color,
    pub identifier_under_caret_write: Color,
    pub debug_line: Color,
    pub breakpoint_line: Color,
    pub error: Color,
    pub warning: Color,
    pub warning_background: Color,
    pub added: Color,
    pub modified: Color,
    pub deleted: Color,
    pub added_gutter: Color,
    pub modified_gutter: Color,
    pub deleted_gutter: Color,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
}

impl EditorTheme {
    pub fn from_semantics(semantics: &Semantics) -> Self {
        let editor = semantics.editor;
        let vcs = semantics.vcs;
        let metrics = &semantics.metrics;
        Self {
            background: editor.background,
            foreground: editor.foreground,
            current_line: editor.current_line,
            selection: editor.selection,
            selection_inactive: editor.selection_inactive,
            caret: editor.caret,
            gutter_background: editor.gutter_background,
            line_number: editor.line_number,
            line_number_current: editor.line_number_current,
            active_line_number: editor.line_number_current,
            active_line_number_background: editor.current_line,
            indent_guide: editor.indent_guide,
            indent_guide_active: editor.indent_guide_active,
            fold_marker: editor.fold_marker,
            fold_placeholder: editor.fold_placeholder,
            whitespace: editor.whitespace,
            inlay_background: editor.inlay_background,
            inlay_foreground: editor.inlay_foreground,
            search_match: editor.search_match,
            search_match_current: editor.search_match_current,
            search_result: editor.search_result,
            matched_brace: editor.matched_brace,
            identifier_under_caret: editor.identifier_under_caret,
            identifier_under_caret_write: editor.identifier_under_caret_write,
            debug_line: editor.debug_line,
            breakpoint_line: editor.breakpoint_line,
            error: semantics.status.danger,
            warning: semantics.status.warning,
            warning_background: editor.warning_background,
            added: vcs.added,
            modified: vcs.modified,
            deleted: vcs.deleted,
            added_gutter: vcs.gutter_added,
            modified_gutter: vcs.gutter_modified,
            deleted_gutter: vcs.gutter_deleted,
            font_family: metrics.font_mono.clone(),
            font_size: metrics.font_code_size,
            line_height: metrics.line_height_code,
        }
    }

    /// Map a syntax capture to its OpenIDEA colour. The native provider names
    /// are the ones the OpenIDEA backend already returns; a new kind must be
    /// added here rather than silently falling back to the foreground colour.
    pub fn syntax(&self, semantics: &Semantics, kind: &str) -> Color {
        let syn = semantics.syntax;
        match kind {
            "keyword" | "keyword.control" | "modifier" => syn.keyword,
            "string" | "string.special" | "regexp" => syn.string,
            "docstring" => syn.docstring,
            "type" | "type.builtin" => syn.type_name,
            "namespace" => syn.namespace,
            "function" | "function.method" => syn.function,
            "function.call" | "function.method.call" => syn.function_call,
            "number" | "number.float" | "boolean" => syn.number,
            "constant" | "constant.builtin" => syn.constant,
            "property" | "property.definition" => syn.property,
            "variable" | "variable.builtin" => syn.variable,
            "parameter" => syn.parameter,
            "self" | "self.keyword" => syn.self_name,
            "comment" => syn.comment,
            "comment.doc" => syn.doc_comment,
            "annotation" | "attribute" => syn.annotation,
            "tag" => syn.tag,
            "escape" => syn.escape,
            "builtin" => syn.builtin,
            "predefined" => syn.predefined,
            "punctuation" | "operator" => syn.punctuation,
            "invalid" => syn.invalid,
            "todo" => syn.todo,
            _ => self.foreground,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{Contrast, Density, Mode, TokenSet};

    #[test]
    fn light_and_dark_editor_islands_use_the_theme_surface() {
        let light = EditorTheme::from_semantics(&Semantics::default());
        let dark = EditorTheme::from_semantics(&Semantics::from_tokens(TokenSet::new(
            Mode::Dark,
            Contrast::Normal,
            Density::Regular,
        )));
        assert_eq!(light.font_size, 13.0);
        assert_eq!(light.line_height, 22.0);
        assert_ne!(light.background, dark.background);
        assert_ne!(light.selection, dark.selection);
        assert_ne!(light.current_line, dark.current_line);
        assert_ne!(light.line_number, dark.line_number);
    }

    #[test]
    fn every_required_editor_role_is_mapped() {
        let semantics = Semantics::default();
        let editor = EditorTheme::from_semantics(&semantics);
        for kind in [
            "keyword",
            "string",
            "type",
            "function",
            "number",
            "constant",
            "property",
            "variable",
            "parameter",
            "comment",
            "annotation",
            "tag",
            "escape",
            "regexp",
            "builtin",
            "punctuation",
        ] {
            assert_ne!(
                editor.syntax(&semantics, kind),
                Color::TRANSPARENT,
                "{kind} has no colour"
            );
        }
    }
}
