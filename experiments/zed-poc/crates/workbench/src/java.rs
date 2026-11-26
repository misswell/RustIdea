use std::borrow::Cow;
use std::sync::Arc;

use anyhow::Context as _;
use language::{Language, LanguageConfig, LanguageQueries};

pub fn register(registry: &Arc<language::LanguageRegistry>) -> anyhow::Result<()> {
    let config = LanguageConfig::from_toml(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/java/config.toml"
    )))
    .context("parse Java language config")?;
    let queries = LanguageQueries {
        highlights: Some(Cow::Borrowed(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/java/highlights.scm"
        )))),
        outline: Some(Cow::Borrowed(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/java/outline.scm"
        )))),
        ..Default::default()
    };
    let language = Arc::new(
        Language::new(config, Some(tree_sitter_java::LANGUAGE.into()))
            .with_queries(queries)
            .context("parse Java tree-sitter queries")?,
    );
    registry.add(language);
    Ok(())
}
