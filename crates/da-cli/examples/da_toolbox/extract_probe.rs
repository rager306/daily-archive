//! Targeted extraction probe: run the rule extractor (DATA dictionaries)
//! over the cached parse payload of one sweep paper and print entities.
//! No gold needed — this is the non-gold counterpart of eval_extract.
//!
//! Usage: cargo run -p da-cli --example extract_probe -- 2504.16925

use anyhow::Result;
use da_adapters::{FilePipelineCache, RuleBasedExtractor};
use da_cli::catalog_materialize::load_paper_parse_and_extraction;
use da_ports::extractor::Extractor;

#[tokio::main]
pub async fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("extract_probe");
    let paper_id = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: extract_probe <paper_id>"))?;
    let cache = FilePipelineCache::new("artifacts/catalog_sweep/pipeline_cache");
    let (parsed, _cached_entities) = load_paper_parse_and_extraction(&cache, &paper_id)?;
    let sections: Vec<(String, String)> = parsed
        .sections
        .iter()
        .map(|s| (s.title.clone(), s.text.clone()))
        .collect();
    let config = da_adapters::rule_extractor::ExtractionConfig::from_yaml_file(
        "data/extraction_patterns.yaml",
    )?;
    let extractor = RuleBasedExtractor::with_config(config);
    let entities: Vec<da_ports::extractor::ExtractedEntity> = extractor.extract(&sections).await?;
    println!("paper {} → {} entities", paper_id, entities.len());
    for e in entities.iter().take(20) {
        println!("  {:?}  {}", e.entity_type, e.label);
    }
    Ok(())
}
