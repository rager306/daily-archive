//! Corpus GROBID profile consult (ADR-076 profile_route).
//!
//! Maps FSM [`ProfileRoute`] onto [`GrobidProfile`] so da-domain never
//! depends on da-parse. Fail-open to DATA default (structural / D204).
//! [`GrobidParser::from_env`] stays historical rich — production PDF
//! callers must consult this helper instead of `from_env().profile()`.

use da_application::parse_grammar::resolve_parse_profile;
use da_domain::fsm_policy::{ArticleFeatures, ProfileRoute};
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

/// YAML-in consult used by tests and by [`consult_corpus_grobid_profile`].
pub fn consult_corpus_grobid_profile_from_yaml(yaml: &str) -> GrobidProfile {
    let host: Option<&dyn FsmPolicyPort> = None;
    match resolve_parse_profile(yaml, host, &corpus_features()) {
        Ok(d) => grobid_profile_from_route(d.profile),
        Err(_) => GrobidProfile::structural(),
    }
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
    }
}
