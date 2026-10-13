//! Native FSM policy (ADR-074 Layer 2 fallback).
//!
//! Used when no WASM module is registered, or when the host fails to
//! instantiate. Thresholds come from [`PolicyRegistry`] (DATA), never
//! from adapter code.

use da_domain::fsm_policy::{
    ArticleFeatures, DecisionPoint, LearnedDecisionTable, PolicyDecision, PolicyRegistry,
    PolicySource, batch_admission_native, grammar_route_native, profile_route_native,
};
use da_ports::fsm_policy::{FsmPolicyError, FsmPolicyPort, FsmPolicyResult};

#[derive(Debug, Clone)]
pub struct NativeFallbackPolicy {
    registry: PolicyRegistry,
    /// Optional candidate learned decision table (ADR-074 candidate-only).
    /// When present and it covers the feature bucket, it answers; otherwise the
    /// consult fail-opens to the native DATA rules. A `Learned` stamp is never
    /// production-active: promotion stays explicit.
    learned: Option<LearnedDecisionTable>,
}

impl NativeFallbackPolicy {
    pub fn new(registry: PolicyRegistry) -> Self {
        Self {
            registry,
            learned: None,
        }
    }

    /// Attach a candidate learned decision table. An invalid table is refused
    /// so a corrupted artifact can never silently answer consults.
    pub fn with_learned_table(
        mut self,
        table: LearnedDecisionTable,
    ) -> Result<Self, FsmPolicyError> {
        table
            .validate()
            .map_err(|e| FsmPolicyError::Load(format!("learned table invalid: {e}")))?;
        self.learned = Some(table);
        Ok(self)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, FsmPolicyError> {
        let registry =
            PolicyRegistry::from_yaml_str(yaml).map_err(|e| FsmPolicyError::Load(e.to_string()))?;
        Ok(Self {
            registry,
            learned: None,
        })
    }

    pub fn empty() -> Self {
        Self {
            registry: PolicyRegistry {
                schema_version: "v2".into(),
                adr_ref: "ADR-074".into(),
                decision_points: Default::default(),
            },
            learned: None,
        }
    }
}

impl FsmPolicyPort for NativeFallbackPolicy {
    fn consult(
        &self,
        point: DecisionPoint,
        features: &ArticleFeatures,
    ) -> FsmPolicyResult<PolicyDecision> {
        match point {
            DecisionPoint::GrammarRoute => {
                if let Some(table) = self.learned_grammar_table() {
                    if let Some(route) = table.grammar_for(features) {
                        return Ok(PolicyDecision::grammar(table.source(), route));
                    }
                }
                let route = grammar_route_native(features, &self.registry.grammar_native());
                Ok(PolicyDecision::grammar(PolicySource::Native, route))
            }
            DecisionPoint::BatchAdmission => {
                if let Some(table) = self.learned_admission_table() {
                    if let Some(admission) = table.admission_for(features) {
                        return Ok(PolicyDecision::admission(table.source(), admission));
                    }
                }
                let admission = batch_admission_native(features, &self.registry.admission_native());
                Ok(PolicyDecision::admission(PolicySource::Native, admission))
            }
            DecisionPoint::ProfileRoute => {
                if let Some(table) = self.learned_profile_table() {
                    if let Some(profile) = table.profile_for(features) {
                        return Ok(PolicyDecision::profile(table.source(), profile));
                    }
                }
                let profile = profile_route_native(features, &self.registry.profile_route_native());
                Ok(PolicyDecision::profile(PolicySource::Native, profile))
            }
            other => Err(FsmPolicyError::UnknownPoint(other.as_str().into())),
        }
    }

    fn policy_hash(&self) -> String {
        "native".into()
    }
}

impl NativeFallbackPolicy {
    fn learned_grammar_table(&self) -> Option<&LearnedDecisionTable> {
        self.learned
            .as_ref()
            .filter(|t| t.decision_point == DecisionPoint::GrammarRoute.as_str())
    }

    fn learned_admission_table(&self) -> Option<&LearnedDecisionTable> {
        self.learned
            .as_ref()
            .filter(|t| t.decision_point == DecisionPoint::BatchAdmission.as_str())
    }

    fn learned_profile_table(&self) -> Option<&LearnedDecisionTable> {
        self.learned
            .as_ref()
            .filter(|t| t.decision_point == DecisionPoint::ProfileRoute.as_str())
    }

    /// The learned stamp carried by an attached table, if any.
    pub fn learned_stamp(&self) -> Option<&str> {
        self.learned.as_ref().map(|t| t.table_sha256.as_str())
    }

    /// Consult with the full detail (ADR-079 D505): the decision plus the
    /// bucket key, the matched cell (code/support/level), or `None` when the
    /// native DATA rules answered. `consult` delegates here and drops detail.
    pub fn consult_detail(
        &self,
        point: DecisionPoint,
        features: &ArticleFeatures,
    ) -> FsmPolicyResult<PolicyConsultDetail> {
        let decision = self.consult(point, features)?;
        let (bucket_key, cell) = match &self.learned {
            Some(table) if table.decision_point == point.as_str() => (
                da_domain::fsm_policy::feature_bucket(point, features),
                table.lookup_with_fallback(features),
            ),
            _ => (da_domain::fsm_policy::feature_bucket(point, features), None),
        };
        Ok(PolicyConsultDetail {
            decision,
            bucket_key,
            cell,
        })
    }
}

/// A consult result plus the WHY (ADR-079 D505): the exact bucket key of the
/// query and, when a learned cell answered, the cell with its support and
/// generalization level. `cell == None` means the native DATA rules answered.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyConsultDetail {
    pub decision: PolicyDecision,
    pub bucket_key: String,
    pub cell: Option<da_domain::fsm_policy::TableCell>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feat(pages: u32, bookmarks: u32, quality: f32) -> ArticleFeatures {
        ArticleFeatures {
            page_count: pages,
            toc_bookmarks: bookmarks,
            text_layer_quality: quality,
            domain: String::new(),
        }
    }

    #[test]
    fn missing_module_falls_back_native() {
        let p = NativeFallbackPolicy::empty();
        let d = p
            .consult(DecisionPoint::GrammarRoute, &feat(18, 0, 0.9))
            .unwrap();
        assert_eq!(d.id.source, PolicySource::Native);
        assert_eq!(d.route.unwrap().as_str(), "xy_cut");
        assert_eq!(p.policy_hash(), "native");
    }

    #[test]
    fn yaml_thresholds_override_defaults() {
        let yaml = r#"
decision_points:
  grammar_route:
    native:
      page_outline_min_pages: 10
      page_outline_min_bookmarks: 1
      grobid_max_text_quality: 0.4
"#;
        let p = NativeFallbackPolicy::from_yaml(yaml).unwrap();
        let d = p
            .consult(DecisionPoint::GrammarRoute, &feat(12, 3, 0.9))
            .unwrap();
        assert_eq!(d.route.unwrap().as_str(), "page_outline");
    }

    #[test]
    fn unimplemented_point_is_error_not_panic() {
        let p = NativeFallbackPolicy::empty();
        let err = p
            .consult(DecisionPoint::ConcurrencyTune, &feat(1, 0, 1.0))
            .unwrap_err();
        assert!(matches!(err, FsmPolicyError::UnknownPoint(_)));
    }

    #[test]
    fn profile_route_native_consult_returns_structural() {
        let yaml = r#"
decision_points:
  profile_route:
    native:
      default: structural
"#;
        let p = NativeFallbackPolicy::from_yaml(yaml).unwrap();
        let d = p
            .consult(DecisionPoint::ProfileRoute, &feat(18, 0, 0.85))
            .unwrap();
        assert_eq!(d.id.decision_point, "profile_route");
        assert_eq!(d.id.source, PolicySource::Native);
        assert_eq!(
            d.profile,
            Some(da_domain::fsm_policy::ProfileRoute::Structural)
        );
        assert!(
            d.route.is_none(),
            "profile_route must not reuse grammar_route"
        );
    }

    #[test]
    fn profile_route_missing_yaml_still_defaults_structural() {
        let p = NativeFallbackPolicy::empty();
        let d = p
            .consult(DecisionPoint::ProfileRoute, &feat(1, 0, 1.0))
            .unwrap();
        assert_eq!(
            d.profile,
            Some(da_domain::fsm_policy::ProfileRoute::Structural),
            "missing DATA must fail-open to D204 structural, not UnknownPoint"
        );
    }

    fn learned_grammar_table() -> LearnedDecisionTable {
        use da_domain::fsm_policy::{LearnedDecisionTable, feature_bucket};
        use std::collections::BTreeMap;
        let mut cells = BTreeMap::new();
        // The 18-page article bucket learns xy_cut; nothing else is covered.
        cells.insert(
            feature_bucket(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85)),
            "xy_cut".to_string(),
        );
        LearnedDecisionTable::seal(DecisionPoint::GrammarRoute, cells)
    }

    #[test]
    fn learned_table_answers_covered_bucket_and_stamps_learned() {
        let p = NativeFallbackPolicy::empty()
            .with_learned_table(learned_grammar_table())
            .unwrap();
        let d = p
            .consult(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85))
            .unwrap();
        match &d.id.source {
            PolicySource::Learned { table_sha256 } => {
                assert_eq!(table_sha256, p.learned_stamp().unwrap())
            }
            other => panic!("expected Learned source, got {other:?}"),
        }
        assert_eq!(d.route.unwrap().as_str(), "xy_cut");
    }

    #[test]
    fn learned_table_fail_opens_to_native_for_uncovered_bucket() {
        let p = NativeFallbackPolicy::empty()
            .with_learned_table(learned_grammar_table())
            .unwrap();
        // 700-page monograph is not covered: native answers, stamp is Native.
        let d = p
            .consult(DecisionPoint::GrammarRoute, &feat(700, 40, 0.9))
            .unwrap();
        assert_eq!(d.id.source, PolicySource::Native);
        assert_eq!(d.route.unwrap().as_str(), "page_outline");
    }

    #[test]
    fn learned_table_does_not_answer_other_points() {
        let p = NativeFallbackPolicy::empty()
            .with_learned_table(learned_grammar_table())
            .unwrap();
        // A grammar table must not answer admission or profile consults.
        let a = p
            .consult(DecisionPoint::BatchAdmission, &feat(18, 0, 0.85))
            .unwrap();
        assert_eq!(a.id.source, PolicySource::Native);
        let pr = p
            .consult(DecisionPoint::ProfileRoute, &feat(18, 0, 0.85))
            .unwrap();
        assert_eq!(pr.id.source, PolicySource::Native);
    }

    #[test]
    fn learned_table_overrides_native_when_covered() {
        // Native would say page_outline for 700 pages, but a learned cell may
        // deliberately differ; the learned stamp proves which producer answered.
        use da_domain::fsm_policy::{LearnedDecisionTable, feature_bucket};
        use std::collections::BTreeMap;
        let mut cells = BTreeMap::new();
        cells.insert(
            feature_bucket(DecisionPoint::GrammarRoute, &feat(700, 40, 0.9)),
            "grobid_structural".to_string(),
        );
        let table = LearnedDecisionTable::seal(DecisionPoint::GrammarRoute, cells);
        let p = NativeFallbackPolicy::empty()
            .with_learned_table(table)
            .unwrap();
        let d = p
            .consult(DecisionPoint::GrammarRoute, &feat(700, 40, 0.9))
            .unwrap();
        assert!(matches!(d.id.source, PolicySource::Learned { .. }));
        assert_eq!(d.route.unwrap().as_str(), "grobid_structural");
    }

    #[test]
    fn invalid_learned_table_is_refused_not_silently_attached() {
        use da_domain::fsm_policy::{LearnedDecisionTable, feature_bucket};
        use std::collections::BTreeMap;
        let mut cells = BTreeMap::new();
        cells.insert(
            feature_bucket(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85)),
            "xy_cut".to_string(),
        );
        let mut table = LearnedDecisionTable::seal(DecisionPoint::GrammarRoute, cells);
        table.table_sha256 = "deadbeef".repeat(8);
        let err = NativeFallbackPolicy::empty()
            .with_learned_table(table)
            .unwrap_err();
        assert!(matches!(err, FsmPolicyError::Load(_)));
    }

    #[test]
    fn consult_detail_reports_bucket_and_cell_with_support() {
        use da_domain::fsm_policy::{BucketLevel, LearnedDecisionTable, feature_bucket};
        use std::collections::BTreeMap;
        let features = feat(18, 0, 0.85);
        let mut cells = BTreeMap::new();
        let mut support = BTreeMap::new();
        let key = feature_bucket(DecisionPoint::GrammarRoute, &features);
        cells.insert(key.clone(), "grobid_structural".to_string());
        support.insert(key.clone(), 7u32);
        let table =
            LearnedDecisionTable::seal_with_support(DecisionPoint::GrammarRoute, cells, support);
        let p = NativeFallbackPolicy::empty()
            .with_learned_table(table)
            .unwrap();
        let detail = p
            .consult_detail(DecisionPoint::GrammarRoute, &features)
            .unwrap();
        assert_eq!(detail.bucket_key, key);
        let cell = detail.cell.expect("learned cell answered");
        assert_eq!(cell.code, "grobid_structural");
        assert_eq!(cell.support, Some(7));
        assert_eq!(cell.level, BucketLevel::Exact);
        assert!(matches!(
            detail.decision.id.source,
            PolicySource::Learned { .. }
        ));
    }

    #[test]
    fn consult_detail_reports_none_cell_when_native_answered() {
        let p = NativeFallbackPolicy::empty();
        let detail = p
            .consult_detail(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85))
            .unwrap();
        assert!(detail.cell.is_none());
        assert_eq!(detail.decision.id.source, PolicySource::Native);
        assert_eq!(
            detail.bucket_key,
            da_domain::fsm_policy::feature_bucket(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85))
        );
    }

    #[test]
    fn consult_detail_marks_the_fallback_level_that_answered() {
        use da_domain::fsm_policy::{BucketLevel, LearnedDecisionTable, feature_bucket_at_level};
        use std::collections::BTreeMap;
        // Only the level-2 (domain+bookmarks dropped) cell exists.
        let mut cells = BTreeMap::new();
        let mut support = BTreeMap::new();
        let broad = feature_bucket_at_level(DecisionPoint::GrammarRoute, &feat(18, 0, 0.85), 2);
        cells.insert(broad.clone(), "page_outline".to_string());
        support.insert(broad, 3);
        let table =
            LearnedDecisionTable::seal_with_support(DecisionPoint::GrammarRoute, cells, support);
        let p = NativeFallbackPolicy::empty()
            .with_learned_table(table)
            .unwrap();
        let detail = p
            .consult_detail(DecisionPoint::GrammarRoute, &feat(18, 5, 0.9))
            .unwrap();
        let cell = detail.cell.expect("broad cell answered");
        assert_eq!(cell.level, BucketLevel::DropDomainAndBookmarks);
        assert_eq!(cell.support, Some(3));
        assert_eq!(cell.code, "page_outline");
    }
}
