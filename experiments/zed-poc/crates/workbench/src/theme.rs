//! GPUI adapter for the canonical OpenIDEA design system.
//!
//! There is no hand-tuned palette in this file. Every colour comes from
//! `rustidea-design`, which is generated from the pinned OpenIDEA tokens.css.
//! The temporary `Theme` shape is kept so the existing workbench can be
//! migrated feature by feature; new code should use `Semantics` directly.

use gpui::{Hsla, hsla};
use rustidea_design::{Color, Contrast, Density, Mode, Semantics, TokenSet};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    #[default]
    Dark,
}

impl ThemeMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }

    pub fn design_mode(self) -> Mode {
        match self {
            Self::Dark => Mode::Dark,
            Self::Light => Mode::Light,
        }
    }
}

fn ink(color: Color) -> Hsla {
    let (h, s, l, a) = color.to_hsla();
    hsla(h, s, l, a)
}

/// Syntax colours from the OpenIDEA editor scheme. Kept `Copy` because the
/// existing render functions copy the theme once per frame.
#[derive(Clone, Copy, Debug)]
pub struct SyntaxTheme {
    pub keyword: Hsla,
    pub string: Hsla,
    pub string_special: Hsla,
    pub escape: Hsla,
    pub regexp: Hsla,
    pub atom: Hsla,
    pub constant: Hsla,
    pub number: Hsla,
    pub comment: Hsla,
    pub doc_comment: Hsla,
    pub function: Hsla,
    pub function_call: Hsla,
    pub builtin: Hsla,
    pub command: Hsla,
    pub type_name: Hsla,
    pub namespace: Hsla,
    pub variable: Hsla,
    pub parameter: Hsla,
    pub self_name: Hsla,
    pub property: Hsla,
    pub attribute: Hsla,
    pub macro_name: Hsla,
    pub label: Hsla,
    pub tag: Hsla,
    pub operator: Hsla,
    pub punctuation: Hsla,
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub mode: ThemeMode,
    pub window: Hsla,
    pub titlebar: Hsla,
    pub panel: Hsla,
    pub panel_alt: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub accent: Hsla,
    pub accent_muted: Hsla,
    pub selection: Hsla,
    pub current_line: Hsla,
    pub gutter: Hsla,
    pub cursor: Hsla,
    pub tab_active: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
    /// Editor island background. `panel` is a tool-window island; the editor is
    /// a separate island in OpenIDEA even when they share a token value.
    pub editor_background: Hsla,
    pub success: Hsla,
    pub stripe_hover: Hsla,
    pub stripe_selected: Hsla,
    pub tab_hover: Hsla,
    pub tree_hover: Hsla,
    pub tree_selected: Hsla,
    pub statusbar_text: Hsla,
    pub syntax: SyntaxTheme,
}

impl Default for Theme {
    fn default() -> Self {
        Self::for_mode(ThemeMode::Light)
    }
}

impl Theme {
    pub fn for_mode(mode: ThemeMode) -> Self {
        let semantics = Semantics::from_tokens(TokenSet::new(
            mode.design_mode(),
            Contrast::Normal,
            Density::Regular,
        ));
        Self::from_semantics(mode, &semantics)
    }

    pub fn from_semantics(mode: ThemeMode, semantics: &Semantics) -> Self {
        let syntax = SyntaxTheme {
            keyword: ink(semantics.syntax.keyword),
            string: ink(semantics.syntax.string),
            string_special: ink(semantics.syntax.string),
            escape: ink(semantics.syntax.escape),
            regexp: ink(semantics.syntax.regexp),
            atom: ink(semantics.syntax.constant),
            constant: ink(semantics.syntax.constant),
            number: ink(semantics.syntax.number),
            comment: ink(semantics.syntax.comment),
            doc_comment: ink(semantics.syntax.doc_comment),
            function: ink(semantics.syntax.function),
            function_call: ink(semantics.syntax.function_call),
            builtin: ink(semantics.syntax.builtin),
            command: ink(semantics.syntax.builtin),
            type_name: ink(semantics.syntax.type_name),
            namespace: ink(semantics.syntax.namespace),
            variable: ink(semantics.syntax.variable),
            parameter: ink(semantics.syntax.parameter),
            self_name: ink(semantics.syntax.self_name),
            property: ink(semantics.syntax.property),
            attribute: ink(semantics.syntax.attribute),
            macro_name: ink(semantics.syntax.macro_name),
            label: ink(semantics.syntax.tag),
            tag: ink(semantics.syntax.tag),
            operator: ink(semantics.syntax.punctuation),
            punctuation: ink(semantics.syntax.punctuation),
        };
        Self {
            mode,
            window: ink(semantics.surface.frame),
            titlebar: ink(semantics.surface.frame),
            panel: ink(semantics.surface.island),
            panel_alt: ink(semantics.surface.elevated),
            border: ink(semantics.line.border),
            text: ink(semantics.text.fg),
            muted: ink(semantics.text.muted),
            faint: ink(semantics.text.faint),
            accent: ink(semantics.status.accent),
            accent_muted: ink(semantics.status.accent_soft),
            selection: ink(semantics.editor.selection),
            current_line: ink(semantics.editor.current_line),
            gutter: ink(semantics.editor.gutter_background),
            cursor: ink(semantics.editor.caret),
            tab_active: ink(semantics.surface.tab_selected),
            warning: ink(semantics.status.warning),
            error: ink(semantics.status.danger),
            editor_background: ink(semantics.editor.background),
            success: ink(semantics.status.success),
            stripe_hover: ink(semantics.surface.stripe_hover),
            stripe_selected: ink(semantics.surface.stripe_selected),
            tab_hover: ink(semantics.surface.tab_hover),
            tree_hover: ink(semantics.surface.tree_hover),
            tree_selected: ink(semantics.surface.tree_selected),
            statusbar_text: ink(semantics.text.muted),
            syntax,
        }
    }

    /// The OpenIDEA semantics behind this palette, for new renderers.
    pub fn semantics(&self) -> Semantics {
        Semantics::from_tokens(TokenSet::new(
            self.mode.design_mode(),
            Contrast::Normal,
            Density::Regular,
        ))
    }

    /// Editor syntax colours, using the OpenIDEA scheme rather than Zed's
    /// bundled theme.
    pub fn syntax(&self, kind: ide_language::SyntaxHighlightKind) -> Hsla {
        use ide_language::SyntaxHighlightKind as Kind;
        let s = self.syntax;
        match kind {
            Kind::Keyword => s.keyword,
            Kind::String => s.string,
            Kind::StringSpecial => s.string_special,
            Kind::Escape => s.escape,
            Kind::Regexp => s.regexp,
            Kind::Atom => s.atom,
            Kind::Constant => s.constant,
            Kind::Number => s.number,
            Kind::Comment => s.comment,
            Kind::DocComment => s.doc_comment,
            Kind::Function => s.function,
            Kind::FunctionCall => s.function_call,
            Kind::Builtin => s.builtin,
            Kind::Command => s.command,
            Kind::Type => s.type_name,
            Kind::Namespace => s.namespace,
            Kind::Variable => s.variable,
            Kind::Parameter => s.parameter,
            Kind::SelfVariable => s.self_name,
            Kind::Property => s.property,
            Kind::Attribute => s.attribute,
            Kind::Macro => s.macro_name,
            Kind::Label => s.label,
            Kind::Tag => s.tag,
            Kind::Operator => s.operator,
            Kind::Punctuation => s.punctuation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_comes_from_the_openidea_tokens() {
        let light = Theme::for_mode(ThemeMode::Light);
        let dark = Theme::for_mode(ThemeMode::Dark);
        assert_eq!(light.mode, ThemeMode::Light);
        assert_ne!(light.window, dark.window);
        assert_ne!(light.editor_background, dark.editor_background);
        assert_ne!(light.syntax.keyword, dark.syntax.keyword);
    }

    #[test]
    fn default_is_light_like_openidea() {
        assert_eq!(Theme::default().mode, ThemeMode::Light);
    }
}
