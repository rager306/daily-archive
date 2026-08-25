//! Critic repair orchestrator (M454).
//!
//! Domain `route_repair` was a library with no process. This stage loads
//! reject_triggers from an agent manifest (DATA), calls `route_repair`,
//! and returns a typed [`RepairOutcome`]. No LLM.

use da_domain::agent_manifest::AgentManifest;
use da_domain::critic_repair::{
    can_repair, repair_outcome, route_repair, CriticVerdict, RepairOutcome, Verdict,
};

#[derive(Debug, thiserror::Error)]
pub enum CriticRepairError {
    #[error("critic repair: manifest: {0}")]
    Manifest(String),
    #[error("critic repair: no repair contract on agent {0}")]
    NoContract(String),
}

/// Run one repair cycle from a critic verdict + DATA manifest.
pub fn run_critic_repair(
    manifest_yaml: &str,
    verdict: &CriticVerdict,
    attempts_used: u32,
) -> Result<Option<RepairOutcome>, CriticRepairError> {
    let manifest =
        AgentManifest::from_yaml_str(manifest_yaml).map_err(CriticRepairError::Manifest)?;
    let repair = manifest
        .repair
        .as_ref()
        .ok_or_else(|| CriticRepairError::NoContract(manifest.agent_id.clone()))?;
    let max = repair.mechanism.max_repair_attempts;
    let Some(action) = route_repair(verdict, &repair.mechanism.reject_triggers) else {
        return Ok(None);
    };
    let next_attempts = attempts_used.saturating_add(1);
    let resolved = can_repair(attempts_used, max);
    Ok(Some(repair_outcome(
        &action.stage,
        &action.action,
        next_attempts,
        resolved && verdict.verdict == Verdict::Reject,
        max,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reject_figure() -> CriticVerdict {
        CriticVerdict {
            id: "fig-1".into(),
            verdict: Verdict::Reject,
            reason: "schema violation".into(),
            failure_pattern: Some("schema violation".into()),
        }
    }

    #[test]
    fn critic_manifest_routes_figure_analysis() {
        let yaml = include_str!("../../../data/agents/critic/manifest.yaml");
        let out = run_critic_repair(yaml, &reject_figure(), 0)
            .unwrap()
            .expect("reject must route");
        assert_eq!(out.stage, "figure_analysis");
        assert_eq!(out.action, "reanalyze_with_diagnostics");
        assert!(out.resolved);
        assert!(out.exhausted_reason.is_none());
    }

    #[test]
    fn confirm_does_not_repair() {
        let yaml = include_str!("../../../data/agents/critic/manifest.yaml");
        let v = CriticVerdict {
            id: "ok".into(),
            verdict: Verdict::Confirm,
            reason: "fine".into(),
            failure_pattern: None,
        };
        assert!(run_critic_repair(yaml, &v, 0).unwrap().is_none());
    }

    #[test]
    fn exhausted_after_max_attempts() {
        let yaml = include_str!("../../../data/agents/critic/manifest.yaml");
        let out = run_critic_repair(yaml, &reject_figure(), 2)
            .unwrap()
            .expect("still routes but exhausted");
        assert!(!out.resolved);
        let reason = out.exhausted_reason.expect("typed exhaustion");
        assert!(reason.contains("repair_exhausted"), "{reason}");
    }

    #[test]
    fn summarizer_without_repair_is_typed_error() {
        let yaml = include_str!("../../../data/agents/summarizer/manifest.yaml");
        let err = run_critic_repair(yaml, &reject_figure(), 0).unwrap_err();
        assert!(matches!(err, CriticRepairError::NoContract(_)));
    }
}
