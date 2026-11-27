//! The generated OpenIDEA token tables and their typed lookup.

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::tokens_generated as generated;

/// OpenIDEA's default is the light Islands theme; dark is the other side of
/// the same design language, not a separate design.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contrast {
    #[default]
    Normal,
    High,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    #[default]
    Regular,
    Compact,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl Density {
    pub fn name(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Compact => "compact",
        }
    }
}

/// A resolved (or intentionally still context-dependent) token table for one
/// theme/density combination.
#[derive(Clone, Copy, Debug)]
pub struct TokenSet {
    pub mode: Mode,
    pub contrast: Contrast,
    pub density: Density,
    table: &'static [(&'static str, &'static str)],
}

impl Default for TokenSet {
    fn default() -> Self {
        Self::new(Mode::Light, Contrast::Normal, Density::Regular)
    }
}

impl TokenSet {
    pub fn new(mode: Mode, contrast: Contrast, density: Density) -> Self {
        let table = match (mode, contrast, density) {
            (Mode::Light, _, Density::Regular) => generated::LIGHT,
            (Mode::Light, _, Density::Compact) => generated::LIGHT_COMPACT,
            (Mode::Dark, Contrast::Normal, Density::Regular) => generated::DARK,
            (Mode::Dark, Contrast::Normal, Density::Compact) => generated::DARK_COMPACT,
            (Mode::Dark, Contrast::High, Density::Regular) => generated::DARK_HC,
            (Mode::Dark, Contrast::High, Density::Compact) => generated::DARK_HC_COMPACT,
        };
        Self {
            mode,
            contrast,
            density,
            table,
        }
    }

    /// Raw token value, with CSS `var()` and `color-mix()` already resolved by
    /// the generator. Values that depend on runtime state (project colour,
    /// background image, platform font) remain as written in `tokens.css`.
    pub fn get(&self, name: &str) -> Option<&'static str> {
        self.table
            .binary_search_by_key(&name, |(key, _)| *key)
            .ok()
            .map(|index| self.table[index].1)
    }

    pub fn require(&self, name: &str) -> &'static str {
        self.get(name).unwrap_or_else(|| {
            panic!(
                "OpenIDEA token {name} is missing from {} / {} / {}",
                self.mode.name(),
                match self.contrast {
                    Contrast::Normal => "normal",
                    Contrast::High => "high",
                },
                self.density.name()
            )
        })
    }

    /// All tokens in this table, sorted by name.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.table.iter().copied()
    }

    pub fn color(&self, name: &str) -> Option<Color> {
        self.get(name).and_then(Color::parse)
    }

    pub fn require_color(&self, name: &str) -> Color {
        let raw = self.require(name);
        Color::parse(raw)
            .unwrap_or_else(|| panic!("OpenIDEA token {name} = {raw:?} is not a concrete colour"))
    }

    /// A length token in logical pixels. Handles the concrete `px` values the
    /// theme data carries and simple `calc(a - b)` expressions.
    pub fn px(&self, name: &str) -> Option<f32> {
        let raw = self.get(name)?;
        parse_px(raw)
    }

    pub fn require_px(&self, name: &str) -> f32 {
        self.px(name)
            .unwrap_or_else(|| panic!("OpenIDEA token {name} = {:?} is not a px length", self.require(name)))
    }
}

fn parse_px(raw: &str) -> Option<f32> {
    let raw = raw.trim();
    if let Some(number) = raw.strip_suffix("px") {
        return number.trim().parse().ok();
    }
    let inner = raw.strip_prefix("calc(")?.strip_suffix(')')?;
    // A tiny arithmetic reader for `calc(a ± b ± ...)`, after token generation
    // has substituted every `var()`. Multiplicative CSS expressions are left
    // to the caller.
    let mut total = 0.0f32;
    let mut sign = 1.0f32;
    let mut start = 0usize;
    let bytes = inner.as_bytes();
    let mut depth = 0usize;
    for (index, byte) in bytes.iter().copied().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b'+' | b'-' if depth == 0 && index > start => {
                let term = inner[start..index].trim();
                total += sign * parse_px(term)?;
                sign = if byte == b'-' { -1.0 } else { 1.0 };
                start = index + 1;
            }
            _ => {}
        }
    }
    total += sign * parse_px(inner[start..].trim())?;
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openidea_default_metrics_match_the_pinned_baseline() {
        let tokens = TokenSet::default();
        assert_eq!(tokens.mode, Mode::Light);
        assert_eq!(tokens.require_px("--h-titlebar"), 40.0);
        assert_eq!(tokens.require_px("--h-header"), 40.0);
        assert_eq!(tokens.require_px("--h-tab"), 28.0);
        assert_eq!(tokens.require_px("--h-row"), 24.0);
        assert_eq!(tokens.require_px("--h-statusbar"), 30.0);
        assert_eq!(tokens.require_px("--stripe-icon-button"), 30.0);
        assert_eq!(tokens.require_px("--island-gap"), 6.0);
        assert_eq!(tokens.require_px("--island-radius"), 10.0);
        assert_eq!(tokens.require_px("--fs-ui"), 13.0);
    }

    #[test]
    fn compact_density_recomputes_every_chrome_metric() {
        let compact = TokenSet::new(Mode::Light, Contrast::Normal, Density::Compact);
        assert_eq!(compact.require_px("--h-titlebar"), 32.0);
        assert_eq!(compact.require_px("--h-header"), 32.0);
        assert_eq!(compact.require_px("--h-tab"), 24.0);
        assert_eq!(compact.require_px("--h-row"), 22.0);
        assert_eq!(compact.require_px("--h-statusbar"), 24.0);
        assert_eq!(compact.require_px("--stripe-icon-button"), 24.0);
        assert_eq!(compact.require_px("--island-gap"), 4.0);
        assert_eq!(compact.require_px("--island-radius"), 8.0);
    }

    #[test]
    fn themes_have_distinct_resolved_surfaces() {
        let light = TokenSet::new(Mode::Light, Contrast::Normal, Density::Regular);
        let dark = TokenSet::new(Mode::Dark, Contrast::Normal, Density::Regular);
        let high = TokenSet::new(Mode::Dark, Contrast::High, Density::Regular);
        assert_eq!(light.require_color("--bg-frame"), Color::rgb(0xe9, 0xea, 0xee));
        assert_eq!(dark.require_color("--bg-frame"), Color::rgb(0x26, 0x28, 0x2c));
        assert_eq!(high.require_color("--bg-frame"), Color::rgb(0x1a, 0x1b, 0x1e));
    }

    #[test]
    fn every_table_is_sorted_for_binary_search() {
        for mode in [Mode::Light, Mode::Dark] {
            for contrast in [Contrast::Normal, Contrast::High] {
                for density in [Density::Regular, Density::Compact] {
                    let tokens = TokenSet::new(mode, contrast, density);
                    let mut previous = "";
                    for (name, _) in tokens.iter() {
                        assert!(previous < name, "table out of order at {name}");
                        previous = name;
                    }
                }
            }
        }
    }

    #[test]
    fn calc_lengths_are_evaluated() {
        assert_eq!(parse_px("calc(13px - 1px)"), Some(12.0));
        assert_eq!(parse_px("calc(10px + 2px - 1px)"), Some(11.0));
    }
}
