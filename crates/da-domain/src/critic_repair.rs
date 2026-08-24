//! Critic repair loop (M451, AutoResearchEval F.4 adaptation):
//! «нашёл дефект → подействовал». A critic verdict of `reject` MUST
//! trigger a repair action on the affected stage — never a silent
//! annotation. This module owns the repair routing (DATA-driven from
//! the critic manifest) and the attempt bookkeeping (append-only).

use serde::{Deserialize, Serialize};

/// Critic verdict for one identity pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Confirm,
    Reject,
    Abstain,
}

/// What the critic says about one low-confidence pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriticVerdict {
    pub id: String,          // article or pair id
    pub verdict: Verdict,
    pub reason: String,
    /// failure pattern ref (agent_failure_patterns.yaml)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_pattern: Option<String>,
}

/// Repair routing: verdict → stage-specific action (DATA from the
/// critic manifest's repair.mechanism.reject_triggers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepairAction {
    pub stage: String,
    pub action: String,
    pub note: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RepairError {
    #[error("repair: {0}")]
    Config(String),
    #[error("repair: exhausted ({attempts} attempts) for {stage}")]
    Exhausted { stage: String, attempts: u32 },
}

/// One repair cycle's outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepairOutcome {
    pub stage: String,
    pub action: String,
    pub attempts_used: u32,
    pub resolved: bool,
    /// typed honest marker when max attempts exhausted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exhausted_reason: Option<String>,
}

/// Route a reject verdict to a repair action. Returns None for
/// confirm/abstain (no repair needed — abstain is a valid diagnostic).
pub fn route_repair<'a>(
    verdict: &CriticVerdict,
    triggers: &'a [RepairAction],
) -> Option<&'a RepairAction> {
    if verdict.verdict != Verdict::Reject {
        return None;
    }
    // find the trigger matching the pattern or the first generic one
    triggers.iter().find(|t| {
        verdict.failure_pattern.as_deref().map(|p| t.note.contains(p)).unwrap_or(false)
            || t.note.is_empty()
    }).or_else(|| triggers.first())
}

/// Decide whether one more repair attempt is allowed (max_repair_attempts
/// from the manifest; 0 = no limit).
pub fn can_repair(attempts_used: u32, max_attempts: u32) -> bool {
    max_attempts == 0 || attempts_used < max_attempts
}

/// Produce the final outcome for a repair cycle (resolved or exhausted).
pub fn repair_outcome(
    stage: &str,
    action: &str,
    attempts_used: u32,
    resolved: bool,
    max_attempts: u32,
) -> RepairOutcome {
    RepairOutcome {
        stage: stage.to_string(),
        action: action.to_string(),
        attempts_used,
        resolved,
        exhausted_reason: if !resolved && !can_repair(attempts_used, max_attempts) {
            Some(format!(
                "repair_exhausted: {attempts_used} attempts on {stage} (max {max_attempts})"
            ))
        } else {
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triggers() -> Vec<RepairAction> {
        vec![
            RepairAction { stage: "enriched".into(), action: "rebuild_binding".into(), note: "surname_mismatch".into() },
            RepairAction { stage: "summaries".into(), action: "regenerate".into(), note: "grounding < alarm".into() },
        ]
    }

    #[test]
    fn reject_routes_repair_confirm_does_not() {
        let reject = CriticVerdict {
            id: "pair-1".into(),
            verdict: Verdict::Reject,
            reason: "different people".into(),
            failure_pattern: Some("surname_mismatch".into()),
        };
        let confirm = CriticVerdict {
            id: "pair-2".into(),
            verdict: Verdict::Confirm,
            reason: "same".into(),
            failure_pattern: None,
        };
        let abstain = CriticVerdict {
            id: "pair-3".into(),
            verdict: Verdict::Abstain,
            reason: "not enough".into(),
            failure_pattern: None,
        };

        let t = triggers();
        let r = route_repair(&reject, &t);
        assert!(r.is_some());
        assert_eq!(r.unwrap().action, "rebuild_binding");

        assert!(route_repair(&confirm, &triggers()).is_none());
        assert!(route_repair(&abstain, &triggers()).is_none());
    }

    #[test]
    fn attempt_limit_enforced_with_typed_exhaustion() {
        assert!(can_repair(0, 2));
        assert!(can_repair(1, 2));
        assert!(!can_repair(2, 2));

        let ok = repair_outcome("enriched", "rebuild_binding", 1, true, 2);
        assert!(ok.resolved);
        assert!(ok.exhausted_reason.is_none());

        let bad = repair_outcome("enriched", "rebuild_binding", 2, false, 2);
        assert!(!bad.resolved);
        let reason = bad.exhausted_reason.unwrap();
        assert!(reason.contains("repair_exhausted"), "{reason}");
    }

    #[test]
    fn no_pattern_falls_to_first_trigger() {
        let reject = CriticVerdict {
            id: "pair-x".into(),
            verdict: Verdict::Reject,
            reason: "bad".into(),
            failure_pattern: None,
        };
        let t = triggers();
        let r = route_repair(&reject, &t);
        assert!(r.is_some());
        assert_eq!(r.unwrap().stage, "enriched"); // first trigger
    }
}
