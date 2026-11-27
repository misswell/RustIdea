//! Semantic names for the token values the workbench and editor paint with.
//!
//! Components must ask this layer for roles (`semantic.surface.island`) rather
//! than reaching for raw CSS variable names. That keeps the OpenIDEA mapping in
//! one place and makes a missing upstream token a compile/test failure instead
//! of an invisible default.

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::metrics::Metrics;
use crate::tokens::{Contrast, Density, Mode, TokenSet};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Surfaces {
    pub app: Color,
    pub frame: Color,
    pub island: Color,
    pub elevated: Color,
    pub popup: Color,
    pub input: Color,
    pub control: Color,
    pub control_hover: Color,
    pub active: Color,
    pub hover: Color,
    pub selected: Color,
    pub selected_strong: Color,
    pub toolbar_hover: Color,
    pub toolbar_pressed: Color,
    pub toolbar_selected: Color,
    pub stripe_hover: Color,
    pub stripe_selected: Color,
    pub stripe_selected_active: Color,
    pub tab_selected: Color,
    pub tab_selected_inactive: Color,
    pub tab_hover: Color,
    pub tree_hover: Color,
    pub tree_selected: Color,
    pub tree_selected_inactive: Color,
    pub backdrop: Color,
    pub scrim: Color,
    pub current_line: Color,
    pub debug_line: Color,
    pub breakpoint_line: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub fg: Color,
    pub strong: Color,
    pub emphasis: Color,
    pub secondary: Color,
    pub muted: Color,
    pub subtle: Color,
    pub faint: Color,
    pub icon: Color,
    pub gutter: Color,
    pub on_accent: Color,
    pub link: Color,
    pub link_hover: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lines {
    pub border: Color,
    pub popup: Color,
    pub strong: Color,
    pub window: Color,
    pub tab_selected: Color,
    pub editor_tabs: Color,
    pub focus_ring: Color,
    pub indent_guide: Color,
    pub indent_guide_active: Color,
    pub gutter: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_soft: Color,
    pub danger: Color,
    pub warning: Color,
    pub weak_warning: Color,
    pub success: Color,
    pub info: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vcs {
    pub modified: Color,
    pub added: Color,
    pub deleted: Color,
    pub untracked: Color,
    pub ignored: Color,
    pub conflict: Color,
    pub gutter_modified: Color,
    pub gutter_added: Color,
    pub gutter_deleted: Color,
    pub diff_modified: Color,
    pub diff_inserted: Color,
    pub diff_deleted: Color,
    pub diff_conflict_stripe: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Editor {
    pub background: Color,
    pub foreground: Color,
    pub gutter_background: Color,
    /// The gutter column in the editor, not only its background.
    pub line_number: Color,
    pub line_number_current: Color,
    pub caret: Color,
    pub selection: Color,
    pub selection_inactive: Color,
    pub current_line: Color,
    pub line_highlight: Color,
    pub debug_line: Color,
    pub breakpoint_line: Color,
    pub search_match: Color,
    pub search_match_current: Color,
    pub search_result: Color,
    pub fold_placeholder: Color,
    pub fold_marker: Color,
    pub inlay_background: Color,
    pub inlay_foreground: Color,
    pub whitespace: Color,
    pub indent_guide: Color,
    pub indent_guide_active: Color,
    pub whitespace_indent_guide: Color,
    pub error_decoration: Color,
    pub warning_background: Color,
    pub warning_decoration: Color,
    pub wave_error: Color,
    pub wave_warning: Color,
    pub wave_typo: Color,
    pub matched_brace: Color,
    pub identifier_under_caret: Color,
    pub identifier_under_caret_write: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Syntax {
    pub keyword: Color,
    pub string: Color,
    pub docstring: Color,
    pub type_name: Color,
    pub namespace: Color,
    pub function: Color,
    pub function_call: Color,
    pub method_call: Color,
    pub number: Color,
    pub constant: Color,
    pub property: Color,
    pub variable: Color,
    pub parameter: Color,
    pub self_name: Color,
    pub comment: Color,
    pub doc_comment: Color,
    pub annotation: Color,
    pub tag: Color,
    pub attribute: Color,
    pub escape: Color,
    pub regexp: Color,
    pub builtin: Color,
    pub predefined: Color,
    pub punctuation: Color,
    pub macro_name: Color,
    pub annotation_name: Color,
    pub inline_value: Color,
    pub invalid: Color,
    pub todo: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalColors {
    pub black: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub white: Color,
    pub bright_black: Color,
    pub bright_red: Color,
    pub bright_green: Color,
    pub bright_yellow: Color,
    pub bright_blue: Color,
    pub bright_magenta: Color,
    pub bright_cyan: Color,
    pub bright_white: Color,
}

/// The complete semantic theme consumed by the workbench and the editor
/// adapter. It is copied into entities so painting never needs a global lookup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Semantics {
    pub mode: Mode,
    pub contrast: Contrast,
    pub density: Density,
    pub surface: Surfaces,
    pub text: Text,
    pub line: Lines,
    pub status: Status,
    pub vcs: Vcs,
    pub editor: Editor,
    pub syntax: Syntax,
    pub terminal: TerminalColors,
    pub metrics: Metrics,
}

impl Default for Semantics {
    fn default() -> Self {
        Self::from_tokens(TokenSet::default())
    }
}

impl Semantics {
    pub fn from_tokens(tokens: TokenSet) -> Self {
        let c = |name: &str| tokens.require_color(name);
        Self {
            mode: tokens.mode,
            contrast: tokens.contrast,
            density: tokens.density,
            surface: Surfaces {
                app: c("--bg-app"),
                frame: c("--bg-frame"),
                island: c("--bg-island"),
                elevated: c("--bg-elev"),
                popup: c("--bg-popup"),
                input: c("--bg-input"),
                control: c("--bg-control"),
                control_hover: c("--bg-control-hover"),
                active: c("--bg-active"),
                hover: c("--bg-hover"),
                selected: c("--bg-selected"),
                selected_strong: c("--bg-selected-strong"),
                toolbar_hover: c("--bg-toolbar-hover"),
                toolbar_pressed: c("--bg-toolbar-pressed"),
                toolbar_selected: c("--bg-toolbar-selected"),
                stripe_hover: c("--bg-stripe-hover"),
                stripe_selected: c("--bg-stripe-selected"),
                stripe_selected_active: c("--bg-stripe-selected-active"),
                tab_selected: c("--bg-tab-selected"),
                tab_selected_inactive: c("--bg-tab-selected-inactive"),
                tab_hover: c("--bg-tab-hover"),
                tree_hover: c("--bg-tree-hover"),
                tree_selected: c("--bg-tree-selected"),
                tree_selected_inactive: c("--bg-tree-selected-inactive"),
                backdrop: c("--bg-backdrop"),
                scrim: c("--bg-scrim"),
                current_line: c("--bg-current-line"),
                debug_line: c("--bg-debug-line"),
                breakpoint_line: c("--bg-breakpoint-line"),
            },
            text: Text {
                fg: c("--fg"),
                strong: c("--fg-strong"),
                emphasis: c("--fg-emphasis"),
                secondary: c("--fg-secondary"),
                muted: c("--fg-muted"),
                subtle: c("--fg-subtle"),
                faint: c("--fg-faint"),
                icon: c("--fg-icon"),
                gutter: c("--fg-gutter"),
                on_accent: c("--fg-on-accent"),
                link: c("--link"),
                link_hover: c("--link-hover"),
            },
            line: Lines {
                border: c("--border"),
                popup: c("--border-popup"),
                strong: c("--border-strong"),
                window: c("--border-window"),
                tab_selected: c("--border-tab-selected"),
                editor_tabs: c("--border-editor-tabs"),
                focus_ring: c("--focus-ring"),
                indent_guide: c("--border-indent-guide"),
                indent_guide_active: c("--border-indent-guide-active"),
                gutter: c("--border-gutter"),
            },
            status: Status {
                accent: c("--accent"),
                accent_hover: c("--accent-hover"),
                accent_soft: c("--accent-soft"),
                danger: c("--danger"),
                warning: c("--warning"),
                weak_warning: c("--fg-weak-warning"),
                success: c("--success"),
                info: c("--info"),
            },
            vcs: Vcs {
                modified: c("--vcs-modified"),
                added: c("--vcs-added"),
                deleted: c("--vcs-deleted"),
                untracked: c("--vcs-untracked"),
                ignored: c("--vcs-ignored"),
                conflict: c("--vcs-conflict"),
                gutter_modified: c("--gutter-modified"),
                gutter_added: c("--gutter-added"),
                gutter_deleted: c("--gutter-deleted"),
                diff_modified: c("--diff-modified"),
                diff_inserted: c("--diff-inserted"),
                diff_deleted: c("--diff-deleted"),
                diff_conflict_stripe: c("--diff-conflict-stripe"),
            },
            editor: Editor {
                background: c("--bg-editor"),
                foreground: c("--fg-editor"),
                gutter_background: c("--bg-editor-gutter"),
                line_number: c("--fg-line-number"),
                line_number_current: c("--fg-line-number-current"),
                caret: c("--caret"),
                selection: c("--bg-editor-selection"),
                selection_inactive: c("--bg-editor-selection-inactive"),
                current_line: c("--bg-current-line"),
                line_highlight: c("--bg-current-line"),
                debug_line: c("--bg-debug-line"),
                breakpoint_line: c("--bg-breakpoint-line"),
                search_match: c("--bg-search-match"),
                search_match_current: c("--bg-search-match-current"),
                search_result: c("--bg-search-result"),
                fold_placeholder: c("--bg-fold-placeholder"),
                fold_marker: c("--fg-fold-marker"),
                inlay_background: c("--bg-inlay"),
                inlay_foreground: c("--fg-inlay"),
                whitespace: c("--fg-whitespace"),
                indent_guide: c("--border-indent-guide"),
                indent_guide_active: c("--border-indent-guide-active"),
                whitespace_indent_guide: c("--border-indent-guide"),
                error_decoration: c("--wave-error"),
                warning_background: c("--editor-warning-background"),
                warning_decoration: c("--wave-warning"),
                wave_error: c("--wave-error"),
                wave_warning: c("--wave-warning"),
                wave_typo: c("--wave-typo"),
                matched_brace: c("--bg-matched-brace"),
                identifier_under_caret: c("--bg-identifier-under-caret"),
                identifier_under_caret_write: c("--bg-identifier-under-caret-write"),
            },
            syntax: Syntax {
                keyword: c("--syn-keyword"),
                string: c("--syn-string"),
                docstring: c("--syn-docstring"),
                type_name: c("--syn-type"),
                namespace: c("--syn-namespace"),
                function: c("--syn-function"),
                function_call: c("--syn-function-call"),
                method_call: c("--syn-function-call"),
                number: c("--syn-number"),
                constant: c("--syn-constant"),
                property: c("--syn-property"),
                variable: c("--syn-variable"),
                parameter: c("--syn-param"),
                self_name: c("--syn-self"),
                comment: c("--syn-comment"),
                doc_comment: c("--syn-doc-comment"),
                annotation: c("--syn-annotation"),
                tag: c("--syn-tag"),
                attribute: c("--syn-attribute"),
                escape: c("--syn-escape"),
                regexp: c("--syn-regexp"),
                builtin: c("--syn-builtin"),
                predefined: c("--syn-predefined"),
                punctuation: c("--syn-punct"),
                macro_name: c("--syn-keyword"),
                annotation_name: c("--syn-annotation"),
                inline_value: c("--syn-inline-value"),
                invalid: c("--syn-invalid"),
                todo: c("--syn-todo"),
            },
            terminal: TerminalColors {
                black: c("--terminal-ansi-black"),
                red: c("--terminal-ansi-red"),
                green: c("--terminal-ansi-green"),
                yellow: c("--terminal-ansi-yellow"),
                blue: c("--terminal-ansi-blue"),
                magenta: c("--terminal-ansi-magenta"),
                cyan: c("--terminal-ansi-cyan"),
                white: c("--terminal-ansi-white"),
                bright_black: c("--terminal-ansi-bright-black"),
                bright_red: c("--terminal-ansi-bright-red"),
                bright_green: c("--terminal-ansi-bright-green"),
                bright_yellow: c("--terminal-ansi-bright-yellow"),
                bright_blue: c("--terminal-ansi-bright-blue"),
                bright_magenta: c("--terminal-ansi-bright-magenta"),
                bright_cyan: c("--terminal-ansi-bright-cyan"),
                bright_white: c("--terminal-ansi-bright-white"),
            },
            metrics: Metrics::from_tokens(&tokens),
        }
    }

    pub fn tokens(&self) -> TokenSet {
        TokenSet::new(self.mode, self.contrast, self.density)
    }

    pub fn is_dark(&self) -> bool {
        self.mode == Mode::Dark
    }

    /// Editor selection with the inactive variant for an unfocused editor.
    pub fn selection(&self, focused: bool) -> Color {
        if focused {
            self.editor.selection
        } else {
            self.editor.selection_inactive
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_semantics_are_the_openidea_light_islands_theme() {
        let semantic = Semantics::default();
        assert_eq!(semantic.mode, Mode::Light);
        assert_eq!(semantic.surface.frame, Color::rgb(0xe9, 0xea, 0xee));
        assert_eq!(semantic.surface.island, Color::rgb(0xff, 0xff, 0xff));
        assert_eq!(semantic.status.accent, Color::rgb(0x38, 0x71, 0xe1));
        assert_eq!(semantic.metrics.titlebar, 40.0);
        assert_eq!(semantic.metrics.island_radius, 10.0);
    }

    #[test]
    fn dark_surfaces_and_syntax_switch_with_the_theme() {
        let dark = Semantics::from_tokens(TokenSet::new(
            Mode::Dark,
            Contrast::Normal,
            Density::Regular,
        ));
        assert_eq!(dark.surface.frame, Color::rgb(0x26, 0x28, 0x2c));
        assert_eq!(dark.surface.island, Color::rgb(0x19, 0x1a, 0x1c));
        assert_eq!(dark.syntax.keyword, Color::rgb(0xcf, 0x8e, 0x6d));
        assert!(dark.is_dark());
    }

    #[test]
    fn every_semantic_colour_round_trips_through_the_token_table() {
        let tokens = TokenSet::default();
        for (name, value) in tokens.iter() {
            if name.starts_with("--bg-")
                || name.starts_with("--fg-")
                || name.starts_with("--border")
                || name.starts_with("--syn-")
                || name.starts_with("--vcs-")
                || name.starts_with("--diff-")
                || name.starts_with("--gutter-")
                || name.starts_with("--terminal-ansi-")
            {
                if value.starts_with('#') || value.starts_with("rgba(") || value.starts_with("rgb(") {
                    assert!(
                        Color::parse(value).is_some(),
                        "generated colour token {name} = {value:?} did not parse"
                    );
                }
            }
        }
    }
}
