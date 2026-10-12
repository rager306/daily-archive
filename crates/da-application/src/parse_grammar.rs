//! Resolve FSM `grammar_profile` from a grammar_route policy consult.
//!
//! Pure: YAML + optional [`FsmPolicyPort`] + features → profile + hash.
//! IO (file load, GROBID) stays in CLI. Fail-open to DATA `default_profile`.

use da_domain::fsm_policy::{
    ArticleFeatures, DecisionPoint, GrammarRoute, PolicyRegistry, PolicySource, ProfileRoute,
};
use da_ports::fsm_policy::{FsmPolicyError, FsmPolicyPort, FsmPolicyResult};

use crate::fsm_policy_consult::consult_with_fallback;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseGrammarDecision {
    pub grammar_profile: String,
    pub route: GrammarRoute,
    pub policy_hash: String,
    pub source: PolicySource,
    /// ADR-079 D505: the exact query bucket, so a reader sees WHY (which
    /// condition) the decision was made. Always present.
    pub bucket_key: String,
    /// ADR-079 D505: the learned cell that answered, when one did. `None`
    /// means the native DATA rules answered (or no table was attached).
    pub cell: Option<da_domain::fsm_policy::TableCell>,
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
        bucket_key: da_domain::fsm_policy::feature_bucket(DecisionPoint::GrammarRoute, features),
        cell: None,
    })
}

/// Consult grammar_route with an optional candidate learned table
/// (ADR-078/R145). Precedence: the explicit host (WASM artifact) is consulted
/// first and fail-opens; the learned-attached native is the fallback layer, so
/// a covered bucket answers with a `Learned` stamp and an uncovered bucket
/// falls open to the native DATA rules.
pub fn resolve_parse_grammar_with_learned(
    yaml: &str,
    host: Option<&dyn FsmPolicyPort>,
    features: &ArticleFeatures,
    learned: Option<&da_domain::fsm_policy::LearnedDecisionTable>,
) -> FsmPolicyResult<ParseGrammarDecision> {
    let registry =
        PolicyRegistry::from_yaml_str(yaml).map_err(|e| FsmPolicyError::Load(e.to_string()))?;
    let default = registry
        .default_grammar_profile()
        .unwrap_or("arxiv_pdf_dot")
        .to_string();

    if let Some(h) = host {
        match h.consult(DecisionPoint::GrammarRoute, features) {
            Ok(d) => {
                let route = d.route.unwrap_or(GrammarRoute::XyCut);
                let grammar_profile = registry
                    .profile_for(route)
                    .unwrap_or(default.as_str())
                    .to_string();
                return Ok(ParseGrammarDecision {
                    grammar_profile,
                    route,
                    policy_hash: d.id.source.hash_stamp().to_string(),
                    source: d.id.source,
                    bucket_key: da_domain::fsm_policy::feature_bucket(
                        DecisionPoint::GrammarRoute,
                        features,
                    ),
                    cell: None,
                });
            }
            Err(FsmPolicyError::UnknownPoint(_)) => {}
            Err(
                FsmPolicyError::Load(_) | FsmPolicyError::Instantiate(_) | FsmPolicyError::Io(_),
            ) => {}
            Err(e) => return Err(e),
        }
    }

    let mut native = crate::fsm_policy_fallback::NativeFallbackPolicy::new(registry.clone());
    if let Some(table) = learned {
        native = native.with_learned_table(table.clone())?;
    }
    let detail = native.consult_detail(DecisionPoint::GrammarRoute, features)?;
    let route = detail.decision.route.unwrap_or(GrammarRoute::XyCut);
    let grammar_profile = registry
        .profile_for(route)
        .unwrap_or(default.as_str())
        .to_string();
    Ok(ParseGrammarDecision {
        grammar_profile,
        route,
        policy_hash: detail.decision.id.source.hash_stamp().to_string(),
        source: detail.decision.id.source,
        bucket_key: detail.bucket_key,
        cell: detail.cell,
    })
}

/// Consulted Java GROBID REST profile (ADR-076 profile_route).
/// Not FSM grammar_profile. Corpus default is structural (D204).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseProfileDecision {
    pub profile: ProfileRoute,
    pub policy_hash: String,
    pub source: PolicySource,
}

/// Consult profile_route. Fail-open to DATA native default (structural).
///
/// Hosts that only serve grammar_route return UnknownPoint and fall open
/// to native. Do not attach a grammar_route wat to this point: WasmPolicyHost
/// maps i32 through GrammarRoute::from_code.
pub fn resolve_parse_profile(
    yaml: &str,
    host: Option<&dyn FsmPolicyPort>,
    features: &ArticleFeatures,
) -> FsmPolicyResult<ParseProfileDecision> {
    let decision = consult_with_fallback(host, yaml, DecisionPoint::ProfileRoute, features)?;
    let profile = decision.profile.unwrap_or(ProfileRoute::Structural);
    Ok(ParseProfileDecision {
        profile,
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

    struct ProfileHost(da_domain::fsm_policy::ProfileRoute);

    impl FsmPolicyPort for ProfileHost {
        fn consult(
            &self,
            point: DecisionPoint,
            _features: &ArticleFeatures,
        ) -> FsmPolicyResult<PolicyDecision> {
            if point != DecisionPoint::ProfileRoute {
                return Err(FsmPolicyError::UnknownPoint(point.as_str().into()));
            }
            Ok(PolicyDecision::profile(
                PolicySource::Wasm {
                    sha256: "cafe".into(),
                },
                self.0,
            ))
        }
    }

    const PROFILE_YAML: &str = r#"
decision_points:
  grammar_route:
    default_profile: arxiv_pdf_dot
  profile_route:
    native:
      default: structural
"#;

    #[test]
    fn resolve_parse_profile_native_default_is_structural() {
        let d = resolve_parse_profile(PROFILE_YAML, None, &feat(18, 0, 0.85)).unwrap();
        assert_eq!(d.profile, da_domain::fsm_policy::ProfileRoute::Structural);
        assert_eq!(d.policy_hash, "native");
        assert_eq!(d.source, PolicySource::Native);
    }

    #[test]
    fn resolve_parse_profile_host_can_name_rich_without_changing_grammar() {
        let d = resolve_parse_profile(
            PROFILE_YAML,
            Some(&ProfileHost(da_domain::fsm_policy::ProfileRoute::Rich)),
            &feat(18, 0, 0.85),
        )
        .unwrap();
        assert_eq!(d.profile, da_domain::fsm_policy::ProfileRoute::Rich);
        assert_eq!(d.policy_hash, "cafe");
        let g = resolve_parse_grammar(PROFILE_YAML, None, &feat(18, 0, 0.85)).unwrap();
        assert_eq!(g.grammar_profile, "arxiv_pdf_dot");
        assert_ne!(
            d.profile.as_str(),
            g.route.as_str(),
            "profile_route and grammar_route stay separate consults"
        );
    }

    #[test]
    fn resolve_parse_profile_missing_yaml_fails_open_structural() {
        let d = resolve_parse_profile("decision_points: {}", None, &feat(1, 0, 1.0)).unwrap();
        assert_eq!(d.profile, da_domain::fsm_policy::ProfileRoute::Structural);
    }

    #[test]
    fn real_registry_yaml_profile_route_is_structural() {
        let yaml = include_str!("../../../data/policies/registry.yaml");
        let d = resolve_parse_profile(yaml, None, &feat(18, 0, 0.85)).unwrap();
        assert_eq!(d.profile, da_domain::fsm_policy::ProfileRoute::Structural);
    }

    #[test]
    fn grammar_consult_with_learned_table_stamps_learned() {
        use da_domain::fsm_policy::{
            ArticleFeatures, DecisionPoint, LearnedDecisionTable, PolicySource, feature_bucket,
        };
        use std::collections::BTreeMap;
        let features = ArticleFeatures {
            page_count: 18,
            toc_bookmarks: 0,
            text_layer_quality: 0.85,
            domain: String::new(),
        };
        let mut cells = BTreeMap::new();
        cells.insert(
            feature_bucket(DecisionPoint::GrammarRoute, &features),
            "grobid_structural".to_string(),
        );
        let table = LearnedDecisionTable::seal(DecisionPoint::GrammarRoute, cells);
        // Covered bucket: Learned stamp, table's code, not native's xy_cut.
        let d = resolve_parse_grammar_with_learned(YAML, None, &features, Some(&table)).unwrap();
        assert!(matches!(d.source, PolicySource::Learned { .. }));
        assert_eq!(d.route.as_str(), "grobid_structural");
        // Uncovered bucket: native fail-open with a Native stamp.
        let other = ArticleFeatures {
            page_count: 700,
            toc_bookmarks: 40,
            text_layer_quality: 0.9,
            domain: String::new(),
        };
        let d2 = resolve_parse_grammar_with_learned(YAML, None, &other, Some(&table)).unwrap();
        assert_eq!(d2.source, PolicySource::Native);
        assert_eq!(d2.route.as_str(), "page_outline");
        // No table at all: pure native, identical to resolve_parse_grammar.
        let d3 = resolve_parse_grammar_with_learned(YAML, None, &features, None).unwrap();
        let d4 = resolve_parse_grammar(YAML, None, &features).unwrap();
        assert_eq!(d3, d4);
    }
}
