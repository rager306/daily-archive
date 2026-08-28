//! Resolve FSM `grammar_profile` from a grammar_route policy consult.
//!
//! Pure: YAML + optional [`FsmPolicyPort`] + features → profile + hash.
//! IO (file load, GROBID) stays in CLI. Fail-open to DATA `default_profile`.

use da_domain::fsm_policy::{
    ArticleFeatures, DecisionPoint, GrammarRoute, PolicyRegistry, PolicySource,
};
use da_ports::fsm_policy::{FsmPolicyError, FsmPolicyPort, FsmPolicyResult};

use crate::fsm_policy_consult::consult_with_fallback;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseGrammarDecision {
    pub grammar_profile: String,
    pub route: GrammarRoute,
    pub policy_hash: String,
    pub source: PolicySource,
}

/// Consult grammar_route and map to an FSM grammar_profile via DATA.
///
/// Fail-open: host Load/Instantiate/Io → native; unknown route → default_profile.
pub fn resolve_parse_grammar(
    yaml: &str,
    host: Option<&dyn FsmPolicyPort>,
    features: &ArticleFeatures,
) -> FsmPolicyResult<ParseGrammarDecision> {
    let registry =
        PolicyRegistry::from_yaml_str(yaml).map_err(|e| FsmPolicyError::Load(e.to_string()))?;
    let default = registry
        .default_grammar_profile()
        .unwrap_or("arxiv_pdf_dot")
        .to_string();

    let decision = consult_with_fallback(host, yaml, DecisionPoint::GrammarRoute, features)?;
    let route = decision.route.unwrap_or(GrammarRoute::XyCut);
    let grammar_profile = registry
        .profile_for(route)
        .unwrap_or(default.as_str())
        .to_string();
    Ok(ParseGrammarDecision {
        grammar_profile,
        route,
        policy_hash: decision.id.source.hash_stamp().to_string(),
        source: decision.id.source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::fsm_policy::{Admission, PolicyDecision};

    const YAML: &str = r#"
decision_points:
  grammar_route:
    default_profile: arxiv_pdf_dot
    profile_map:
      page_outline: page_outline
      xy_cut: arxiv_pdf_dot
      grobid_structural: arxiv_pdf_dot
    native:
      page_outline_min_pages: 50
      page_outline_min_bookmarks: 1
      grobid_max_text_quality: 0.4
"#;

    fn feat(pages: u32, bookmarks: u32, quality: f32) -> ArticleFeatures {
        ArticleFeatures {
            page_count: pages,
            toc_bookmarks: bookmarks,
            text_layer_quality: quality,
            domain: String::new(),
        }
    }

    struct FixedHost(GrammarRoute);

    impl FsmPolicyPort for FixedHost {
        fn consult(
            &self,
            point: DecisionPoint,
            _features: &ArticleFeatures,
        ) -> FsmPolicyResult<PolicyDecision> {
            if point != DecisionPoint::GrammarRoute {
                return Err(FsmPolicyError::UnknownPoint(point.as_str().into()));
            }
            Ok(PolicyDecision::grammar(
                PolicySource::Wasm {
                    sha256: "deadbeef".into(),
                },
                self.0,
            ))
        }
    }

    struct LoadFailHost;

    impl FsmPolicyPort for LoadFailHost {
        fn consult(
            &self,
            _point: DecisionPoint,
            _features: &ArticleFeatures,
        ) -> FsmPolicyResult<PolicyDecision> {
            Err(FsmPolicyError::Load("gone".into()))
        }
    }

    struct AdmissionOnlyHost;

    impl FsmPolicyPort for AdmissionOnlyHost {
        fn consult(
            &self,
            _point: DecisionPoint,
            _features: &ArticleFeatures,
        ) -> FsmPolicyResult<PolicyDecision> {
            Ok(PolicyDecision::admission(
                PolicySource::Native,
                Admission::Accept,
            ))
        }
    }

    #[test]
    fn page_outline_maps_to_page_outline_profile() {
        let d = resolve_parse_grammar(
            YAML,
            Some(&FixedHost(GrammarRoute::PageOutline)),
            &feat(1, 0, 1.0),
        )
        .unwrap();
        assert_eq!(d.grammar_profile, "page_outline");
        assert_eq!(d.route, GrammarRoute::PageOutline);
        assert_eq!(d.policy_hash, "deadbeef");
    }

    #[test]
    fn grobid_maps_to_arxiv_pdf_dot() {
        let d = resolve_parse_grammar(
            YAML,
            Some(&FixedHost(GrammarRoute::GrobidStructural)),
            &feat(1, 0, 1.0),
        )
        .unwrap();
        assert_eq!(d.grammar_profile, "arxiv_pdf_dot");
        assert_eq!(d.route, GrammarRoute::GrobidStructural);
    }

    #[test]
    fn xy_cut_stamps_intent_keeps_arxiv_pdf_dot_path() {
        let d = resolve_parse_grammar(
            YAML,
            Some(&FixedHost(GrammarRoute::XyCut)),
            &feat(1, 0, 1.0),
        )
        .unwrap();
        assert_eq!(d.grammar_profile, "arxiv_pdf_dot");
        assert_eq!(d.route, GrammarRoute::XyCut);
    }

    #[test]
    fn missing_host_falls_open_native_typical_article() {
        let d = resolve_parse_grammar(YAML, None, &feat(18, 0, 0.85)).unwrap();
        assert_eq!(d.grammar_profile, "arxiv_pdf_dot");
        assert_eq!(d.route, GrammarRoute::XyCut);
        assert_eq!(d.policy_hash, "native");
    }

    #[test]
    fn load_fail_host_falls_open_native() {
        let d = resolve_parse_grammar(YAML, Some(&LoadFailHost), &feat(18, 0, 0.85)).unwrap();
        assert_eq!(d.source, PolicySource::Native);
        assert_eq!(d.grammar_profile, "arxiv_pdf_dot");
    }

    #[test]
    fn native_monograph_features_select_page_outline() {
        let d = resolve_parse_grammar(YAML, None, &feat(700, 40, 0.9)).unwrap();
        assert_eq!(d.grammar_profile, "page_outline");
        assert_eq!(d.route, GrammarRoute::PageOutline);
    }

    #[test]
    fn missing_route_on_decision_uses_xy_cut_then_map() {
        let d = resolve_parse_grammar(YAML, Some(&AdmissionOnlyHost), &feat(1, 0, 1.0)).unwrap();
        assert_eq!(d.route, GrammarRoute::XyCut);
        assert_eq!(d.grammar_profile, "arxiv_pdf_dot");
    }

    #[test]
    fn real_registry_yaml_parses_profile_map() {
        let yaml = include_str!("../../../data/policies/registry.yaml");
        let d = resolve_parse_grammar(yaml, None, &feat(18, 0, 0.85)).unwrap();
        assert_eq!(d.grammar_profile, "arxiv_pdf_dot");
    }
}
