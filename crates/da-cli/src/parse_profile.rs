//! Corpus GROBID profile consult (ADR-076 profile_route).
//!
//! Maps FSM [`ProfileRoute`] onto [`GrobidProfile`] so da-domain never
//! depends on da-parse. Fail-open to DATA default (structural / D204).
//! [`GrobidParser::from_env`] stays historical rich — production PDF
//! callers must consult this helper instead of `from_env().profile()`.

use da_application::parse_grammar::{ParseProfileDecision, resolve_parse_profile};
use da_domain::fsm_policy::{ArticleFeatures, PolicySource, ProfileRoute};
use da_parse::GrobidProfile;
use da_ports::fsm_policy::FsmPolicyPort;
use std::path::PathBuf;

/// Map FSM ProfileRoute onto the da-parse GROBID REST profile.
pub fn grobid_profile_from_route(route: ProfileRoute) -> GrobidProfile {
    match route {
        ProfileRoute::Structural => GrobidProfile::structural(),
        ProfileRoute::Rich => GrobidProfile::rich(),
    }
}

/// Env-gated candidate learned table loader (ADR-074).
///
/// `DA_LEARNED_POLICY_TABLE` names a candidate table artifact. When set and
/// valid, the consult answers covered buckets with a `Learned` stamp; when
/// unset, missing, or invalid, the consult fail-opens to the native DATA rules.
/// This is deliberately an env opt-in: a learned table is never production by
/// default, matching the candidate-only contract.
fn load_learned_table_env() -> Option<da_domain::fsm_policy::LearnedDecisionTable> {
    let path = std::env::var("DA_LEARNED_POLICY_TABLE").ok()?;
    if path.trim().is_empty() {
        return None;
    }
    da_application::parse_policy_artifact::load_learned_table(std::path::Path::new(&path)).ok()
}

fn policy_registry_path() -> PathBuf {
    std::env::var("DA_POLICY_REGISTRY")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/policies/registry.yaml"))
}

fn corpus_features() -> ArticleFeatures {
    ArticleFeatures {
        page_count: 0,
        toc_bookmarks: 0,
        text_layer_quality: 0.85,
        domain: String::new(),
    }
}

/// Consult DATA `profile_route`. Fail-open to structural (D204).
///
/// Native only this cut. WasmPolicyHost maps i32 through GrammarRoute;
/// do not attach grammar_route.wat to this point.
pub fn consult_corpus_grobid_profile() -> GrobidProfile {
    let yaml = std::fs::read_to_string(policy_registry_path()).unwrap_or_default();
    consult_corpus_grobid_profile_from_yaml(&yaml)
}

/// Production PDF parser: historical `from_env` URL plus DATA profile.
/// Health probes may keep [`GrobidParser::from_env`] — liveness is profile-free.
pub fn corpus_grobid_parser() -> da_parse::GrobidParser {
    da_parse::GrobidParser::from_env().with_profile(consult_corpus_grobid_profile())
}

/// YAML-in consult used by tests and by [`consult_corpus_grobid_profile`].
pub fn consult_corpus_grobid_profile_from_yaml(yaml: &str) -> GrobidProfile {
    let host: Option<&dyn FsmPolicyPort> = None;
    match resolve_parse_profile_with_learned(yaml, host, &corpus_features(), load_learned_table_env())
    {
        Ok(d) => grobid_profile_from_route(d.profile),
        Err(_) => GrobidProfile::structural(),
    }
}

/// Consult profile_route with an optional candidate learned table. A covered
/// bucket answers with a `Learned` stamp; otherwise the native path answers.
/// Kept beside [`resolve_parse_profile`] so the pure application helper stays
/// table-agnostic and this CLI boundary owns the artifact.
pub fn resolve_parse_profile_with_learned(
    yaml: &str,
    host: Option<&dyn FsmPolicyPort>,
    features: &ArticleFeatures,
    learned: Option<da_domain::fsm_policy::LearnedDecisionTable>,
) -> Result<ParseProfileDecision, da_ports::fsm_policy::FsmPolicyError> {
    use da_application::fsm_policy_consult::consult_with_fallback;
    use da_domain::fsm_policy::DecisionPoint;
    let registry = da_domain::fsm_policy::PolicyRegistry::from_yaml_str(yaml)
        .map_err(|e| da_ports::fsm_policy::FsmPolicyError::Load(e.to_string()))?;
    let mut native = da_application::fsm_policy_fallback::NativeFallbackPolicy::new(registry);
    if let Some(table) = learned {
        native = native
            .with_learned_table(table)
            .map_err(|e| da_ports::fsm_policy::FsmPolicyError::Load(e.to_string()))?;
    }
    let decision = consult_with_fallback(Some(&native), yaml, DecisionPoint::ProfileRoute, features)?;
    Ok(ParseProfileDecision {
        profile: decision.profile.unwrap_or(ProfileRoute::Structural),
        policy_hash: decision.id.source.hash_stamp().to_string(),
        source: decision.id.source,
    })
}

/// Full profile_route decision (profile + policy hash + source), not just the
/// mapped GROBID profile. Thin wrapper over [`resolve_parse_profile`]; fails
/// open exactly like [`consult_corpus_grobid_profile`] so a missing or invalid
/// registry names the DATA native structural default instead of an invented
/// consult. The governed-process record consumes this decision (S10).
pub fn corpus_grobid_profile_decision() -> ParseProfileDecision {
    let yaml = std::fs::read_to_string(policy_registry_path()).unwrap_or_default();
    corpus_grobid_profile_decision_from_yaml(&yaml)
}

/// YAML-in decision consult used by tests and by
/// [`corpus_grobid_profile_decision`].
pub fn corpus_grobid_profile_decision_from_yaml(yaml: &str) -> ParseProfileDecision {
    let host: Option<&dyn FsmPolicyPort> = None;
    resolve_parse_profile(yaml, host, &corpus_features()).unwrap_or_else(|_| ParseProfileDecision {
        profile: ProfileRoute::Structural,
        policy_hash: PolicySource::Native.hash_stamp().to_string(),
        source: PolicySource::Native,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE_YAML: &str = r#"
decision_points:
  profile_route:
    native:
      default: structural
"#;

    const RICH_YAML: &str = r#"
decision_points:
  profile_route:
    native:
      default: rich
"#;

    #[test]
    fn grobid_profile_from_route_maps_closed_set() {
        assert_eq!(
            grobid_profile_from_route(ProfileRoute::Structural).name(),
            "structural"
        );
        assert_eq!(grobid_profile_from_route(ProfileRoute::Rich).name(), "rich");
        assert_ne!(
            grobid_profile_from_route(ProfileRoute::Structural).configuration_fingerprint(),
            grobid_profile_from_route(ProfileRoute::Rich).configuration_fingerprint(),
            "D204 fingerprints must not collide"
        );
    }

    #[test]
    fn consult_corpus_grobid_profile_from_yaml_defaults_structural() {
        let p = consult_corpus_grobid_profile_from_yaml(PROFILE_YAML);
        assert_eq!(p.name(), "structural");
    }

    #[test]
    fn consult_corpus_grobid_profile_from_yaml_honors_named_rich() {
        let p = consult_corpus_grobid_profile_from_yaml(RICH_YAML);
        assert_eq!(
            p.name(),
            "rich",
            "DATA may name rich; corpus default must not"
        );
    }

    #[test]
    fn consult_corpus_grobid_profile_missing_yaml_fails_open_structural() {
        let p = consult_corpus_grobid_profile_from_yaml("decision_points: {}");
        assert_eq!(p.name(), "structural");
        let p2 = consult_corpus_grobid_profile_from_yaml("not: yaml: [");
        assert_eq!(p2.name(), "structural");
    }

    #[test]
    fn real_registry_yaml_consult_is_structural() {
        let yaml = include_str!("../../../data/policies/registry.yaml");
        let p = consult_corpus_grobid_profile_from_yaml(yaml);
        assert_eq!(p.name(), "structural");
    }

    #[test]
    fn corpus_grobid_profile_decision_from_yaml_defaults_structural_native() {
        let d = corpus_grobid_profile_decision_from_yaml(PROFILE_YAML);
        assert_eq!(d.profile, ProfileRoute::Structural);
        assert_eq!(d.source, PolicySource::Native);
        assert_eq!(d.policy_hash, "native");
    }

    #[test]
    fn corpus_grobid_profile_decision_from_yaml_honors_named_rich() {
        let d = corpus_grobid_profile_decision_from_yaml(RICH_YAML);
        assert_eq!(d.profile, ProfileRoute::Rich);
        assert_eq!(d.source, PolicySource::Native);
    }

    #[test]
    fn corpus_grobid_profile_decision_invalid_yaml_fails_open_structural_native() {
        for yaml in ["decision_points: {}", "not: yaml: ["] {
            let d = corpus_grobid_profile_decision_from_yaml(yaml);
            assert_eq!(d.profile, ProfileRoute::Structural);
            assert_eq!(d.source, PolicySource::Native);
            assert_eq!(d.policy_hash, "native");
        }
    }

    #[test]
    fn real_registry_yaml_decision_matches_profile_consult() {
        let yaml = include_str!("../../../data/policies/registry.yaml");
        let d = corpus_grobid_profile_decision_from_yaml(yaml);
        assert_eq!(d.profile, ProfileRoute::Structural);
        assert_eq!(d.source, PolicySource::Native);
        assert_eq!(d.policy_hash, "native");
        assert_eq!(
            grobid_profile_from_route(d.profile).name(),
            consult_corpus_grobid_profile_from_yaml(yaml).name(),
            "decision wrapper and profile consult must resolve the same route"
        );
    }

    #[test]
    fn from_env_facade_stays_historical_rich() {
        assert_eq!(
            da_parse::GrobidParser::from_env().profile().name(),
            "rich",
            "from_env remains historical rich; PDF callers must consult"
        );
        let yaml = include_str!("../../../data/policies/registry.yaml");
        assert_ne!(
            consult_corpus_grobid_profile_from_yaml(yaml).name(),
            da_parse::GrobidParser::from_env().profile().name(),
            "corpus consult must not silently equal the historical from_env default"
        );
        assert_eq!(
            corpus_grobid_parser().profile().name(),
            "structural",
            "production PDF constructor must follow DATA default"
        );
    }
}
