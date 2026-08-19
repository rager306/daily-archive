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
        assert_eq!(ledger.stages.len(), 19);
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
        assert_eq!(
            registry.acap_layer("parse").unwrap(),
            Some(da_domain::ArticleContextState::Parsed)
        );
        assert_eq!(
            registry.acap_layer("grobid_parse").unwrap(),
            Some(da_domain::ArticleContextState::Parsed)
        );
        assert_eq!(
            registry.acap_layer("candidate_materialization").unwrap(),
            None
        );
        assert_eq!(
            registry.acap_layer("revision_materialization").unwrap(),
            Some(da_domain::ArticleContextState::Structured)
        );
        assert_eq!(
            registry.acap_layer("extraction_materialization").unwrap(),
            Some(da_domain::ArticleContextState::EvidenceLinked)
        );
        // Detection stays in the parse plane (resolved via alias); the
        // multimodal stages are canonical identities, never aliases of
        // parse/extraction_compute.
        assert_eq!(registry.resolve("figure_detect").unwrap().as_str(), "parse");
        assert_eq!(
            registry
                .resolve("figure_context_artifact")
                .unwrap()
                .as_str(),
            "figure_context"
        );
        for canonical in [
            "figure_context",
            "figure_analysis",
            "table_fidelity",
            "equation_extraction",
            "candidate_materialization",
        ] {
            assert_eq!(
                registry.parse_canonical(canonical).unwrap().as_str(),
                canonical
            );
        }
        assert_ne!(
            registry.resolve("figure_analysis").unwrap().as_str(),
            "extraction_compute"
        );
        // Candidate materialization declares the candidate-only graph truth
        // boundary: graph_materialize cache mode, never graph truth (D127).
        let candidate = ledger
            .stage(
                &registry
                    .parse_canonical("candidate_materialization")
                    .unwrap(),
            )
            .unwrap();
        assert!(candidate.candidate_only);
        assert!(!candidate.graph_truth_boundary);
        assert_eq!(
            candidate.cache_mode,
            da_domain::StageCacheMode::GraphMaterialize
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
