//! Strict DATA loader for the single ACAP admission policy declaration.
//!
//! The runtime ACAP admission semantics live in exactly one place:
//! `scripts/governor/checks/article_context_assurance.yaml` — the ordered
//! `fsm.states` list plus the top-level `admission` block. This module is the
//! infrastructure seam that reads that declaration into the pure
//! [`da_domain::AcapAdmissionPolicy`] value the runtime executes; the governor
//! reads the same file in `_acap_admission_violations`. The shared parity
//! oracle (`governor/fixtures/acap_runtime_control_plane/admission_parity.vectors.json`)
//! pins that both readers agree.
//!
//! Mirrors [`crate::pipeline_ledger::load_pipeline_stage_ledger`]: stat and
//! bound the file, read it, parse it, then bind the domain value. Validation is
//! fail-closed — a declaration that omits or weakens the `admission` block, or
//! names an unknown `forward_rule` / `same_layer_repeat`, is an error, never a
//! defaulted policy. R138 onion: infrastructure loads, `da-domain` decides.

use std::collections::BTreeSet;
use std::path::Path;

use da_domain::{AcapAdmissionPolicy, AcapForwardRule, ArticleContextState};
use serde::Deserialize;

/// Bound on the declaration we are willing to read. The real file is ~10 KiB;
/// this mirrors the pipeline-ledger loader's 1 MiB ceiling.
const MAX_ACAP_POLICY_BYTES: u64 = 1024 * 1024;

/// Top level of the ACAP declaration. Only the two blocks this loader binds are
/// captured; the many sibling keys (`schema_version`, `principles`,
/// `metric_definitions`, ...) are intentionally ignored so unrelated edits to
/// the declaration do not break admission loading.
#[derive(Debug, Deserialize)]
struct RawAcapPolicy {
    #[serde(default)]
    fsm: Option<RawFsm>,
    #[serde(default)]
    admission: Option<RawAdmission>,
}

/// The `fsm` block. `deny_unknown_fields` keeps the reader closed: a typo'd key
/// in the block the runtime depends on must fail loudly rather than be ignored.
/// Fields the admission policy does not need are still captured so the block
/// stays strict; `allow(dead_code)` documents that they exist for closedness,
/// not for binding.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct RawFsm {
    initial_state: ArticleContextState,
    terminal_states: Vec<ArticleContextState>,
    /// Ordered state list: the single declaration of ACAP layer order/rank.
    states: Vec<ArticleContextState>,
    transitions: serde_yaml::Value,
    failure_transition_from: serde_yaml::Value,
    failure_transition_to: ArticleContextState,
    retry_requires_same_semantic_key: bool,
}

/// The `admission` block: the one declaration of runtime admission semantics.
/// `semantics_version` is captured for closedness (a future version gate can
/// read it) but is not bound by this loader today.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct RawAdmission {
    #[serde(default)]
    semantics_version: Option<String>,
    entry_states: Vec<ArticleContextState>,
    /// DATA token resolved to [`AcapForwardRule`]; an unknown token fails closed.
    forward_rule: String,
    /// `allowed`/`forbidden`; any other token fails closed.
    same_layer_repeat: String,
    failure_sink: ArticleContextState,
    evidence_required: bool,
    skip_requires_typed_reason: bool,
}

/// Load the single declared ACAP admission policy from DATA.
///
/// Fail-closed: a non-file or oversized file, an absent `fsm` or `admission`
/// block, an unknown `forward_rule` or `same_layer_repeat` token,
/// `evidence_required != true`, a `failure_sink` that disagrees with
/// `fsm.failure_transition_to`, an `entry_states` member absent from
/// `fsm.states`, or a `states` list that does not cover every
/// [`ArticleContextState`] all return `Err`. There is no default policy
/// fallback: the runtime must read the declaration or refuse to admit.
pub fn load_acap_admission_policy(path: &Path) -> Result<AcapAdmissionPolicy, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("stat ACAP admission policy {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > MAX_ACAP_POLICY_BYTES {
        return Err(format!(
            "ACAP admission policy {} is not a bounded file",
            path.display()
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read ACAP admission policy {}: {error}", path.display()))?;
    let raw: RawAcapPolicy = serde_yaml::from_slice(&bytes)
        .map_err(|error| format!("parse ACAP admission policy {}: {error}", path.display()))?;
    admission_policy_from_raw(raw, path)
}

/// Bind the parsed declaration to the pure policy value, validating closed.
fn admission_policy_from_raw(
    raw: RawAcapPolicy,
    path: &Path,
) -> Result<AcapAdmissionPolicy, String> {
    let fsm = raw
        .fsm
        .ok_or_else(|| format!("ACAP admission policy {} has no fsm block", path.display()))?;
    let admission = raw.admission.ok_or_else(|| {
        format!(
            "ACAP admission policy {} has no admission block: the runtime has nothing to read",
            path.display()
        )
    })?;

    // The failure sink must be the one the fsm block declares, so the runtime
    // and the governor cannot disagree about where a failure lands.
    if admission.failure_sink != fsm.failure_transition_to {
        return Err(format!(
            "ACAP admission policy {} declares admission.failure_sink {:?} but fsm.failure_transition_to is {:?}",
            path.display(),
            admission.failure_sink,
            fsm.failure_transition_to
        ));
    }

    // Durability is not negotiable at runtime; a declaration that relaxes it is
    // refused rather than loaded as a weaker gate.
    if !admission.evidence_required {
        return Err(format!(
            "ACAP admission policy {} declares admission.evidence_required != true; the runtime admission gate requires a durable evidence reference",
            path.display()
        ));
    }

    let forward_rule = match admission.forward_rule.as_str() {
        "monotone_subset" => AcapForwardRule::MonotoneSubset,
        other => {
            return Err(format!(
                "ACAP admission policy {} declares unknown admission.forward_rule {other:?}",
                path.display()
            ));
        }
    };
    let same_layer_repeat = match admission.same_layer_repeat.as_str() {
        "allowed" => true,
        "forbidden" => false,
        other => {
            return Err(format!(
                "ACAP admission policy {} declares unknown admission.same_layer_repeat {other:?}",
                path.display()
            ));
        }
    };

    if admission.entry_states.is_empty() {
        return Err(format!(
            "ACAP admission policy {} declares an empty admission.entry_states",
            path.display()
        ));
    }
    let declared: BTreeSet<ArticleContextState> = fsm.states.iter().copied().collect();
    for entry in &admission.entry_states {
        if !declared.contains(entry) {
            return Err(format!(
                "ACAP admission policy {} admission entry state {entry:?} is not a declared fsm state",
                path.display()
            ));
        }
    }

    AcapAdmissionPolicy::new(
        fsm.states,
        admission.entry_states.into_iter().collect(),
        admission.failure_sink,
        forward_rule,
        same_layer_repeat,
        admission.evidence_required,
        admission.skip_requires_typed_reason,
    )
    .map_err(|error| format!("ACAP admission policy {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn project_acap_admission_policy_is_valid() {
        let path = repo_root().join("scripts/governor/checks/article_context_assurance.yaml");
        let policy = load_acap_admission_policy(&path).unwrap();
        assert_eq!(policy.order().len(), 15);
        assert_eq!(policy.rank(ArticleContextState::Parsed), Some(2));
        assert_eq!(policy.failure_sink(), ArticleContextState::FailedVisible);
        assert_eq!(policy.forward_rule(), AcapForwardRule::MonotoneSubset);
        assert!(policy.same_layer_repeat());
        assert!(policy.evidence_required());
        assert!(policy.skip_requires_typed_reason());
    }

    #[test]
    fn loader_rejects_unknown_forward_rule() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("acap.yaml");
        std::fs::write(
            &path,
            "fsm:\n  initial_state: source_registered\n  terminal_states: [accepted_checkpoint, failed_visible]\n  states: [source_registered, source_verified, parsed, structured, extracted, evidence_linked, embedded, materialized, enriched, resolved, summarized, finalized, task_accepted, accepted_checkpoint, failed_visible]\n  transitions: []\n  failure_transition_from: \"*\"\n  failure_transition_to: failed_visible\n  retry_requires_same_semantic_key: true\nadmission:\n  entry_states: [parsed]\n  forward_rule: strict_adjacent\n  same_layer_repeat: allowed\n  failure_sink: failed_visible\n  evidence_required: true\n  skip_requires_typed_reason: true\n",
        )
        .unwrap();
        let error = load_acap_admission_policy(&path).unwrap_err();
        assert!(error.contains("unknown admission.forward_rule"), "{error}");
    }

    #[test]
    fn loader_rejects_missing_admission_block() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("acap.yaml");
        std::fs::write(
            &path,
            "fsm:\n  initial_state: source_registered\n  terminal_states: [accepted_checkpoint, failed_visible]\n  states: [source_registered, source_verified, parsed, structured, extracted, evidence_linked, embedded, materialized, enriched, resolved, summarized, finalized, task_accepted, accepted_checkpoint, failed_visible]\n  transitions: []\n  failure_transition_from: \"*\"\n  failure_transition_to: failed_visible\n  retry_requires_same_semantic_key: true\n",
        )
        .unwrap();
        let error = load_acap_admission_policy(&path).unwrap_err();
        assert!(error.contains("has no admission block"), "{error}");
    }

    #[test]
    fn loader_rejects_missing_file() {
        let error = load_acap_admission_policy(Path::new("/nonexistent/acap.yaml")).unwrap_err();
        assert!(error.contains("stat ACAP admission policy"), "{error}");
    }
}
