//! Strict DATA loader for the canonical semantic pipeline stage ledger.

use std::path::Path;

use da_domain::pipeline_ledger::PipelineStageLedger;

pub fn load_pipeline_stage_ledger(path: &Path) -> Result<PipelineStageLedger, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("stat pipeline ledger {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err(format!(
            "pipeline ledger {} is not a bounded file",
            path.display()
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read pipeline ledger {}: {error}", path.display()))?;
    let ledger: PipelineStageLedger = serde_yaml::from_slice(&bytes)
        .map_err(|error| format!("parse pipeline ledger {}: {error}", path.display()))?;
    ledger.validate()?;
    Ok(ledger)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_pipeline_ledger_is_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/ontology/pipeline_stage_contracts.yaml");
        let ledger = load_pipeline_stage_ledger(&path).unwrap();
        assert_eq!(ledger.stages.len(), 14);
        assert!(!ledger.import_eligible);
        let registry = ledger.registry().unwrap();
        assert_eq!(registry.resolve("grobid_parse").unwrap().as_str(), "parse");
        assert_eq!(registry.resolve("html_parse").unwrap().as_str(), "parse");
        assert_eq!(registry.resolve("embed").unwrap().as_str(), "embedding");
        assert_eq!(
            registry.resolve("rule_extract").unwrap().as_str(),
            "extraction_compute"
        );
        assert_eq!(
            registry.resolve("cluster_detection").unwrap().as_str(),
            "cluster"
        );
    }

    #[test]
    fn loader_rejects_unknown_fields() {
        let path =
            std::env::temp_dir().join(format!("pipeline-ledger-{}.yaml", std::process::id()));
        std::fs::write(&path, "schema_version: pipeline-stage-contracts.v1\nimport_eligible: false\nstages: []\nunknown: true\n").unwrap();
        assert!(
            load_pipeline_stage_ledger(&path)
                .unwrap_err()
                .contains("unknown field")
        );
        std::fs::remove_file(path).unwrap();
    }
}
