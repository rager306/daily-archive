//! Provider-neutral semantic contract for the production article pipeline.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageCacheMode {
    None,
    PureCompute,
    DurableArtifact,
    GraphMaterialize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageOwner {
    Domain,
    Application,
    Port,
    Adapter,
    Cli,
    Storage,
    Mcp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineStageContract {
    pub id: String,
    pub owner: StageOwner,
    pub input_semantics: Vec<String>,
    pub output_semantics: Vec<String>,
    pub depends_on: Vec<String>,
    pub context_adds: Vec<String>,
    pub context_losses: Vec<String>,
    pub hard_invariants: Vec<String>,
    pub consumers: Vec<String>,
    pub cache_mode: StageCacheMode,
    pub failure_class: String,
    pub conditional: bool,
    pub graph_truth_boundary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineStageLedger {
    pub schema_version: String,
    pub stages: Vec<PipelineStageContract>,
    pub import_eligible: bool,
}

impl PipelineStageLedger {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "pipeline-stage-contracts.v1" || self.import_eligible {
            return Err("pipeline ledger schema/D127 invalid".into());
        }
        if self.stages.is_empty() {
            return Err("pipeline ledger has no stages".into());
        }
        let mut by_id = BTreeMap::new();
        for stage in &self.stages {
            if stage.id.trim().is_empty() || by_id.insert(stage.id.as_str(), stage).is_some() {
                return Err(format!(
                    "pipeline stage id empty or duplicated: {}",
                    stage.id
                ));
            }
            if stage.input_semantics.is_empty()
                || stage.output_semantics.is_empty()
                || stage.context_adds.is_empty()
                || stage.hard_invariants.is_empty()
                || stage.consumers.is_empty()
                || stage.failure_class.trim().is_empty()
            {
                return Err(format!(
                    "pipeline stage {} has incomplete semantics",
                    stage.id
                ));
            }
            if stage.graph_truth_boundary && stage.cache_mode != StageCacheMode::GraphMaterialize {
                return Err(format!(
                    "graph truth stage {} must be graph_materialize",
                    stage.id
                ));
            }
            if stage.cache_mode == StageCacheMode::GraphMaterialize && !stage.graph_truth_boundary {
                return Err(format!(
                    "graph materialize stage {} must declare truth boundary",
                    stage.id
                ));
            }
        }
        for stage in &self.stages {
            for dependency in &stage.depends_on {
                if dependency == &stage.id || !by_id.contains_key(dependency.as_str()) {
                    return Err(format!(
                        "stage {} has invalid dependency {dependency}",
                        stage.id
                    ));
                }
            }
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for stage in &self.stages {
            visit(stage.id.as_str(), &by_id, &mut visiting, &mut visited)?;
        }
        Ok(())
    }
}

fn visit<'a>(
    id: &'a str,
    by_id: &BTreeMap<&'a str, &'a PipelineStageContract>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id) {
        return Err(format!("pipeline stage cycle at {id}"));
    }
    for dependency in &by_id[id].depends_on {
        visit(dependency, by_id, visiting, visited)?;
    }
    visiting.remove(id);
    visited.insert(id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stage(id: &str, deps: &[&str], mode: StageCacheMode, truth: bool) -> PipelineStageContract {
        PipelineStageContract {
            id: id.into(),
            owner: StageOwner::Application,
            input_semantics: vec!["input".into()],
            output_semantics: vec!["output".into()],
            depends_on: deps.iter().map(|x| x.to_string()).collect(),
            context_adds: vec!["grounding".into()],
            context_losses: vec![],
            hard_invariants: vec!["D127=false".into()],
            consumers: vec!["next".into()],
            cache_mode: mode,
            failure_class: "terminal".into(),
            conditional: false,
            graph_truth_boundary: truth,
        }
    }
    #[test]
    fn valid_ledger_roundtrips() {
        let l = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![
                stage("parse", &[], StageCacheMode::PureCompute, false),
                stage(
                    "materialize",
                    &["parse"],
                    StageCacheMode::GraphMaterialize,
                    true,
                ),
            ],
            import_eligible: false,
        };
        l.validate().unwrap();
        let q: PipelineStageLedger =
            serde_json::from_slice(&serde_json::to_vec(&l).unwrap()).unwrap();
        assert_eq!(q, l);
    }
    #[test]
    fn rejects_cycle_and_cache_truth_mismatch() {
        let l = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![
                stage("a", &["b"], StageCacheMode::PureCompute, false),
                stage("b", &["a"], StageCacheMode::PureCompute, false),
            ],
            import_eligible: false,
        };
        assert!(l.validate().unwrap_err().contains("cycle"));
        let l = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![stage("bad", &[], StageCacheMode::PureCompute, true)],
            import_eligible: false,
        };
        assert!(l.validate().is_err());
    }
}
