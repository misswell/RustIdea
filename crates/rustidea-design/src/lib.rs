//! The canonical design system shared by the native RustIdea workbench.
//!
//! The data is generated from the pinned OpenIDEA `tokens.css` by
//! `tools/openidea-tokens/generate.py`; see `tokens_generated.rs`.  This crate
//! deliberately has no GPUI dependency so the old and native workbenches can
//! use the same source of truth while the native app replaces the legacy one.

pub mod color;
pub mod editor_theme;
pub mod metrics;
pub mod semantic;
pub mod tokens;
pub mod tokens_generated;
pub mod workbench;

pub use color::Color;
pub use metrics::Metrics;
pub use semantic::Semantics;
pub use tokens::{Contrast, Density, Mode, TokenSet};

/// The OpenIDEA commit whose tokens are vendored in `third_party/openidea`.
pub const OPENIDEA_BASELINE_COMMIT: &str = "fe3e647abb804cffbfff6bd8de42e01762d6de40";
/// The OpenIDEA release version at that commit.
pub const OPENIDEA_BASELINE_VERSION: &str = "0.1.54";
