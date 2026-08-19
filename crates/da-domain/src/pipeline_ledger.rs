//! Provider-neutral semantic contract for the production article pipeline.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::assurance::ArticleContextState;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StageId(String);

impl StageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

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
    #[serde(default)]
    pub aliases: Vec<String>,
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
    /// Candidate-only graph truth boundary: the stage materializes candidate
    /// evidence into the graph, but every written node is hard-locked
    /// non-authoritative (`authoritative=false`, `import_eligible=false`) —
    /// never graph truth (D127 / ADR-053). Requires
    /// `cache_mode == GraphMaterialize` and `graph_truth_boundary == false`.
    #[serde(default)]
    pub candidate_only: bool,
    /// Optional ACAP FSM layer this stage establishes when admitted.
    /// `None` means the stage is outside the FSM (typed skip). `FailedVisible`
    /// is rejected by [`PipelineStageLedger::validate`] — failure is a sink,
    /// not a declared layer. Unknown tokens fail serde because the field is
    /// a closed [`ArticleContextState`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acap_layer: Option<ArticleContextState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineStageLedger {
    pub schema_version: String,
    pub stages: Vec<PipelineStageContract>,
    pub import_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageIdRegistry {
    canonical: BTreeSet<StageId>,
    resolved: BTreeMap<String, StageId>,
    acap_layers: BTreeMap<StageId, ArticleContextState>,
}

impl StageIdRegistry {
    pub fn from_ledger(ledger: &PipelineStageLedger) -> Result<Self, String> {
        ledger.validate()?;
        let canonical = ledger
            .stages
            .iter()
            .map(|stage| StageId(stage.id.clone()))
            .collect::<BTreeSet<_>>();
        let mut resolved = canonical
            .iter()
            .map(|id| (id.as_str().to_owned(), id.clone()))
            .collect::<BTreeMap<_, _>>();
        for stage in &ledger.stages {
            let target = StageId(stage.id.clone());
            for alias in &stage.aliases {
                if !safe_stage_component(alias) {
                    return Err(format!(
                        "pipeline stage alias is empty or unsafe: {} -> {alias}",
                        stage.id
                    ));
                }
                if let Some(existing) = resolved.insert(alias.clone(), target.clone()) {
                    return Err(format!(
                        "pipeline stage alias collision: {alias} resolves to {existing} and {}",
                        stage.id
                    ));
                }
            }
        }
        let mut acap_layers = BTreeMap::new();
        for stage in &ledger.stages {
            if let Some(layer) = stage.acap_layer {
                acap_layers.insert(StageId(stage.id.clone()), layer);
            }
        }
        Ok(Self {
            canonical,
            resolved,
            acap_layers,
        })
    }

    pub fn parse_canonical(&self, raw: &str) -> Result<StageId, String> {
        let candidate = StageId(raw.to_owned());
        self.canonical
            .get(&candidate)
            .cloned()
            .ok_or_else(|| format!("unknown canonical pipeline stage id: {raw}"))
    }

    pub fn resolve(&self, raw: &str) -> Result<StageId, String> {
        self.resolved
            .get(raw)
            .cloned()
            .ok_or_else(|| format!("unknown pipeline stage id or alias: {raw}"))
    }

    pub fn contains(&self, raw: &str) -> bool {
        self.resolved.contains_key(raw)
    }

    /// ACAP layer declared for `raw` after alias resolve.
    ///
    /// Unknown ids error. A known stage with no `acap_layer` returns `Ok(None)`
    /// (ungated helper / typed skip).
    pub fn acap_layer(&self, raw: &str) -> Result<Option<ArticleContextState>, String> {
        let id = self.resolve(raw)?;
        Ok(self.acap_layers.get(&id).copied())
    }
}

fn safe_stage_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

impl PipelineStageLedger {
    pub fn stage(&self, id: &StageId) -> Option<&PipelineStageContract> {
        self.stages.iter().find(|stage| stage.id == id.as_str())
    }

    pub fn registry(&self) -> Result<StageIdRegistry, String> {
        StageIdRegistry::from_ledger(self)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "pipeline-stage-contracts.v1" || self.import_eligible {
            return Err("pipeline ledger schema/D127 invalid".into());
        }
        if self.stages.is_empty() {
            return Err("pipeline ledger has no stages".into());
        }
        let mut by_id = BTreeMap::new();
        for stage in &self.stages {
            if !safe_stage_component(&stage.id) || by_id.insert(stage.id.as_str(), stage).is_some()
            {
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
            if stage.candidate_only && stage.cache_mode != StageCacheMode::GraphMaterialize {
                return Err(format!(
                    "candidate-only stage {} must be graph_materialize",
                    stage.id
                ));
            }
            if stage.candidate_only && stage.graph_truth_boundary {
                return Err(format!(
                    "candidate-only stage {} cannot declare graph truth",
                    stage.id
                ));
            }
            if stage.cache_mode == StageCacheMode::GraphMaterialize
                && !stage.graph_truth_boundary
                && !stage.candidate_only
            {
                return Err(format!(
                    "graph materialize stage {} must declare graph truth or candidate-only boundary",
                    stage.id
                ));
            }
            if stage.acap_layer == Some(ArticleContextState::FailedVisible) {
                return Err(format!(
                    "pipeline stage {} cannot declare FailedVisible as acap_layer",
                    stage.id
                ));
            }
        }
        let canonical_ids = by_id.keys().copied().collect::<BTreeSet<_>>();
        let mut aliases = BTreeSet::new();
        for stage in &self.stages {
            for alias in &stage.aliases {
                if !safe_stage_component(alias)
                    || canonical_ids.contains(alias.as_str())
                    || !aliases.insert(alias.as_str())
                {
                    return Err(format!(
                        "pipeline stage alias empty, unsafe or duplicated: {} -> {alias}",
                        stage.id
                    ));
                }
            }
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
            aliases: vec![],
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
            candidate_only: false,
            acap_layer: None,
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
    fn registry_resolves_aliases_but_keeps_canonical_parsing_strict() {
        let mut parse = stage("parse", &[], StageCacheMode::PureCompute, false);
        parse.aliases = vec!["grobid_parse".into(), "html_parse".into()];
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![parse],
            import_eligible: false,
        };
        let registry = ledger.registry().unwrap();
        assert_eq!(registry.resolve("grobid_parse").unwrap().as_str(), "parse");
        assert_eq!(registry.resolve("parse").unwrap().as_str(), "parse");
        assert!(registry.parse_canonical("grobid_parse").is_err());
        let parse_id = registry.parse_canonical("parse").unwrap();
        assert_eq!(ledger.stage(&parse_id).unwrap().id, "parse");
    }

    #[test]
    fn rejects_duplicate_colliding_and_unsafe_aliases() {
        let mut parse = stage("parse", &[], StageCacheMode::PureCompute, false);
        parse.aliases = vec!["shared".into()];
        let mut embed = stage("embedding", &["parse"], StageCacheMode::PureCompute, false);
        embed.aliases = vec!["shared".into()];
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![parse.clone(), embed],
            import_eligible: false,
        };
        assert!(ledger.validate().unwrap_err().contains("alias"));

        parse.aliases = vec!["embedding".into()];
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![
                parse.clone(),
                stage("embedding", &["parse"], StageCacheMode::PureCompute, false),
            ],
            import_eligible: false,
        };
        assert!(ledger.validate().unwrap_err().contains("alias"));

        parse.aliases = vec!["../parse".into()];
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![parse],
            import_eligible: false,
        };
        assert!(ledger.validate().unwrap_err().contains("alias"));
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

    #[test]
    fn candidate_only_materialization_is_not_graph_truth() {
        // A candidate materialization stage writes into the graph with
        // graph_materialize cache mode but declares a candidate-only boundary:
        // the materialized nodes are locked non-authoritative (D127), so the
        // stage is validated and round-trips with candidate_only preserved.
        let mut materialize = stage(
            "candidate_materialization",
            &[],
            StageCacheMode::GraphMaterialize,
            false,
        );
        materialize.candidate_only = true;
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![materialize],
            import_eligible: false,
        };
        ledger.validate().unwrap();
        let q: PipelineStageLedger =
            serde_json::from_slice(&serde_json::to_vec(&ledger).unwrap()).unwrap();
        assert!(q.stages[0].candidate_only);
        assert!(!q.stages[0].graph_truth_boundary);
    }

    #[test]
    fn candidate_only_cannot_declare_graph_truth() {
        // A stage cannot be both candidate-only and graph truth: the two
        // boundaries are mutually exclusive.
        let mut stage = stage(
            "candidate_materialization",
            &[],
            StageCacheMode::GraphMaterialize,
            true,
        );
        stage.candidate_only = true;
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![stage],
            import_eligible: false,
        };
        assert!(
            ledger
                .validate()
                .unwrap_err()
                .contains("cannot declare graph truth")
        );
    }

    #[test]
    fn candidate_only_requires_graph_materialize_cache_mode() {
        // Candidate materialization writes graph evidence, so a pure-compute
        // candidate-only stage is invalid.
        let mut stage = stage(
            "candidate_materialization",
            &[],
            StageCacheMode::PureCompute,
            false,
        );
        stage.candidate_only = true;
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![stage],
            import_eligible: false,
        };
        assert!(
            ledger
                .validate()
                .unwrap_err()
                .contains("must be graph_materialize")
        );
    }

    #[test]
    fn graph_materialize_requires_declared_boundary() {
        // Every graph materialization must declare which boundary it writes:
        // graph truth or candidate-only. An undeclared materialize is invalid.
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![stage(
                "undecided",
                &[],
                StageCacheMode::GraphMaterialize,
                false,
            )],
            import_eligible: false,
        };
        assert!(
            ledger
                .validate()
                .unwrap_err()
                .contains("graph truth or candidate-only")
        );
    }

    #[test]
    fn registry_resolves_acap_layer_after_alias_and_skips_unset() {
        let mut parse = stage("parse", &[], StageCacheMode::PureCompute, false);
        parse.aliases = vec!["grobid_parse".into()];
        parse.acap_layer = Some(ArticleContextState::Parsed);
        let readiness = stage("readiness", &["parse"], StageCacheMode::PureCompute, false);
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![parse, readiness],
            import_eligible: false,
        };
        let registry = ledger.registry().unwrap();
        assert_eq!(
            registry.acap_layer("parse").unwrap(),
            Some(ArticleContextState::Parsed)
        );
        assert_eq!(
            registry.acap_layer("grobid_parse").unwrap(),
            Some(ArticleContextState::Parsed)
        );
        assert_eq!(registry.acap_layer("readiness").unwrap(), None);
        assert!(registry.acap_layer("missing").is_err());
    }

    #[test]
    fn rejects_failed_visible_as_acap_layer() {
        let mut parse = stage("parse", &[], StageCacheMode::PureCompute, false);
        parse.acap_layer = Some(ArticleContextState::FailedVisible);
        let ledger = PipelineStageLedger {
            schema_version: "pipeline-stage-contracts.v1".into(),
            stages: vec![parse],
            import_eligible: false,
        };
        assert!(
            ledger
                .validate()
                .unwrap_err()
                .contains("cannot declare FailedVisible as acap_layer")
        );
    }

    #[test]
    fn unknown_acap_layer_token_fails_serde() {
        let err = serde_json::from_str::<PipelineStageContract>(
            r#"{
                "id": "parse",
                "owner": "application",
                "input_semantics": ["input"],
                "output_semantics": ["output"],
                "depends_on": [],
                "context_adds": ["grounding"],
                "context_losses": [],
                "hard_invariants": ["D127=false"],
                "consumers": ["next"],
                "cache_mode": "pure_compute",
                "failure_class": "terminal",
                "conditional": false,
                "graph_truth_boundary": false,
                "acap_layer": "not_a_real_state"
            }"#,
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("not_a_real_state") || err.contains("unknown variant"),
            "{err}"
        );
    }
}
