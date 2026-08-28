//! Native FSM policy (ADR-074 Layer 2 fallback).
//!
//! Used when no WASM module is registered, or when the host fails to
//! instantiate. Thresholds come from [`PolicyRegistry`] (DATA), never
//! from adapter code.

use da_domain::fsm_policy::{
    ArticleFeatures, DecisionPoint, PolicyDecision, PolicyRegistry, PolicySource,
    batch_admission_native, grammar_route_native,
};
use da_ports::fsm_policy::{FsmPolicyError, FsmPolicyPort, FsmPolicyResult};

#[derive(Debug, Clone)]
pub struct NativeFallbackPolicy {
    registry: PolicyRegistry,
}

impl NativeFallbackPolicy {
    pub fn new(registry: PolicyRegistry) -> Self {
        Self { registry }
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, FsmPolicyError> {
        let registry =
            PolicyRegistry::from_yaml_str(yaml).map_err(|e| FsmPolicyError::Load(e.to_string()))?;
        Ok(Self { registry })
    }

    pub fn empty() -> Self {
        Self {
            registry: PolicyRegistry {
                schema_version: "v2".into(),
                adr_ref: "ADR-074".into(),
                decision_points: Default::default(),
            },
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
                let route = grammar_route_native(features, &self.registry.grammar_native());
                Ok(PolicyDecision::grammar(PolicySource::Native, route))
            }
            DecisionPoint::BatchAdmission => {
                let admission = batch_admission_native(features, &self.registry.admission_native());
                Ok(PolicyDecision::admission(PolicySource::Native, admission))
            }
            other => Err(FsmPolicyError::UnknownPoint(other.as_str().into())),
        }
    }

    fn policy_hash(&self) -> String {
        "native".into()
    }
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
}
