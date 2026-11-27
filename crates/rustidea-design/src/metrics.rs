//! Typed chrome metrics derived from the OpenIDEA tokens.

use serde::{Deserialize, Serialize};

use crate::tokens::TokenSet;

/// Logical-pixel chrome measurements. Fields are computed from the token
/// table, never chosen by the workbench; `from_tokens` is the only constructor
/// production code should use. The fallbacks mirror the pinned default theme
/// and keep the UI usable when a future upstream token is renamed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub titlebar: f32,
    pub compact_header: f32,
    pub titlebar_tool_button: f32,
    pub header: f32,
    pub tabbar: f32,
    pub tab: f32,
    pub row: f32,
    pub list_row: f32,
    pub statusbar: f32,
    pub navigationbar: f32,
    pub field: f32,
    pub button: f32,
    pub control: f32,
    pub control_small: f32,
    pub stripe_icon_button: f32,
    pub stripe_outer_padding: f32,
    pub stripe_button_width: f32,
    pub island_gap: f32,
    pub island_radius: f32,
    pub project_panel_width: f32,
    pub gutter_width: f32,
    pub tab_pad_x: f32,
    pub tree_inset: f32,
    pub space_0_5: f32,
    pub space_1: f32,
    pub space_1_5: f32,
    pub space_2: f32,
    pub space_3: f32,
    pub space_4: f32,
    pub radius_small: f32,
    pub radius_row: f32,
    pub radius_selection: f32,
    pub radius_control: f32,
    pub radius_tab: f32,
    pub radius_toolbar_button: f32,
    pub radius_popup: f32,
    pub radius_popup_large: f32,
    pub radius_dialog: f32,
    pub radius_tooltip: f32,
    pub font_ui_size: f32,
    pub font_small_size: f32,
    pub font_tiny_size: f32,
    pub font_code_size: f32,
    pub font_stripe_label_size: f32,
    pub line_height_code: f32,
    pub font_ui: String,
    pub font_mono: String,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::from_tokens(&TokenSet::default())
    }
}

impl Metrics {
    pub fn from_tokens(tokens: &TokenSet) -> Self {
        let px = |name: &str, fallback: f32| tokens.px(name).unwrap_or(fallback);
        Self {
            titlebar: px("--h-titlebar", 40.0),
            compact_header: px("--h-compact-header", 30.0),
            titlebar_tool_button: px("--titlebar-tool-button", 30.0),
            header: px("--h-header", 40.0),
            tabbar: px("--h-tabbar", 38.0),
            tab: px("--h-tab", 28.0),
            row: px("--h-row", 24.0),
            list_row: px("--h-list-row", 24.0),
            statusbar: px("--h-statusbar", 30.0),
            navigationbar: px("--h-navigationbar", 26.0),
            field: px("--h-field", 28.0),
            button: px("--h-button", 28.0),
            control: px("--h-control", 24.0),
            control_small: px("--h-control-small", 20.0),
            stripe_icon_button: px("--stripe-icon-button", 30.0),
            stripe_outer_padding: px("--stripe-outer-padding", 4.0),
            stripe_button_width: px("--stripe-button-w", 58.0),
            island_gap: px("--island-gap", 6.0),
            island_radius: px("--island-radius", 10.0),
            project_panel_width: px("--w-project-panel", 240.0),
            gutter_width: px("--w-gutter", 52.0),
            tab_pad_x: px("--tab-pad-x", 8.0),
            tree_inset: px("--tree-inset", 6.0),
            space_0_5: px("--space-0-5", 2.0),
            space_1: px("--space-1", 4.0),
            space_1_5: px("--space-1-5", 6.0),
            space_2: px("--space-2", 8.0),
            space_3: px("--space-3", 12.0),
            space_4: px("--space-4", 16.0),
            radius_small: px("--radius-small", 4.0),
            radius_row: px("--radius-row", 4.0),
            radius_selection: px("--radius-selection", 5.0),
            radius_control: px("--radius-control", 4.0),
            radius_tab: px("--radius-tab", 6.0),
            radius_toolbar_button: px("--radius-toolbar-button", 6.0),
            radius_popup: px("--radius-popup", 8.0),
            radius_popup_large: px("--radius-popup-large", 10.0),
            radius_dialog: px("--radius-dialog", 10.0),
            radius_tooltip: px("--radius-tooltip", 6.0),
            font_ui_size: px("--fs-ui", 13.0),
            font_small_size: px("--fs-small", 12.0),
            font_tiny_size: px("--fs-tiny", 11.0),
            font_code_size: px("--fs-code", 13.0),
            font_stripe_label_size: px("--fs-stripe-label", 11.0),
            line_height_code: px("--lh-code", 22.0),
            font_ui: tokens
                .get("--font-ui")
                .unwrap_or("system-ui, -apple-system, sans-serif")
                .to_owned(),
            font_mono: tokens
                .get("--font-mono")
                .unwrap_or("ui-monospace, Menlo, monospace")
                .to_owned(),
        }
    }

    /// The tab strip's total height including the tab itself.
    pub fn tab_strip_height(&self) -> f32 {
        self.tabbar
    }

    /// A stripe's width: the button, its outer padding and the island gap.
    pub fn stripe_width(&self, named: bool) -> f32 {
        let button = if named {
            self.stripe_button_width
        } else {
            self.stripe_icon_button
        };
        button + self.stripe_outer_padding + self.island_gap
    }

    /// The vertical gap between stripe buttons, as in OpenIDEA: one titlebar
    /// minus one button, so the first button lines up with the toolbar.
    pub fn stripe_button_gap(&self) -> f32 {
        (self.titlebar - self.stripe_icon_button).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_metrics_come_from_the_default_tokens() {
        let metrics = Metrics::default();
        assert_eq!(metrics.titlebar, 40.0);
        assert_eq!(metrics.tab, 28.0);
        assert_eq!(metrics.statusbar, 30.0);
        assert_eq!(metrics.island_gap, 6.0);
        assert_eq!(metrics.island_radius, 10.0);
        assert_eq!(metrics.font_ui_size, 13.0);
        assert_eq!(metrics.stripe_button_gap(), 10.0);
        assert_eq!(metrics.stripe_width(false), 40.0);
    }

    #[test]
    fn compact_metrics_are_computed_not_hardcoded() {
        let compact = Metrics::from_tokens(&TokenSet::new(
            crate::tokens::Mode::Light,
            crate::tokens::Contrast::Normal,
            crate::tokens::Density::Compact,
        ));
        assert_eq!(compact.titlebar, 32.0);
        assert_eq!(compact.statusbar, 24.0);
        assert_eq!(compact.island_gap, 4.0);
        assert_eq!(compact.stripe_button_gap(), 8.0);
    }
}
