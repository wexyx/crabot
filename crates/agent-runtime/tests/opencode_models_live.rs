//! Exercises the real model discovery against the installed OpenCode CLI.
//! Not part of the default suite: it needs `opencode` on PATH and a signed-in
//! account. Set `CRABOT_OPENCODE_MODELS=1` to run it.
use agent_runtime::config::{OpenCodeCatalogSource, OpenCodeConfig, opencode_models};

#[tokio::test]
async fn discovers_the_installed_catalog_with_prices() {
    if std::env::var("CRABOT_OPENCODE_MODELS").as_deref() != Ok("1") {
        eprintln!("skipped: set CRABOT_OPENCODE_MODELS=1 to exercise the real OpenCode CLI");
        return;
    }
    let config = OpenCodeConfig::from_env();
    let catalog = opencode_models(&config).await.expect("model discovery");
    println!(
        "source={} models={}",
        catalog.source.as_str(),
        catalog.models.len()
    );
    for model in catalog.models.iter().take(5) {
        println!(
            "{} free={} in={:?} context={:?}",
            model.id, model.free, model.input, model.context
        );
    }
    if catalog.source == OpenCodeCatalogSource::Catalog {
        // Free must be a strict subset of priced, and never the other way round.
        for model in &catalog.models {
            assert!(
                model.priced,
                "{} has no price but claims a source",
                model.id
            );
            if !model.free {
                assert!(model.input.unwrap_or_default() > 0.0, "{}", model.id);
            }
        }
        assert!(catalog.models.iter().any(|m| m.free), "no free model found");
    }
}
