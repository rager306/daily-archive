//! Application consult helper: registry YAML in, decision out.
//!
//! IO (reading the YAML file) stays in CLI/adapters. This module is
//! pure: given YAML + features, return a native decision. The WASM
//! host is injected via [`FsmPolicyPort`] when present.

use da_domain::fsm_policy::{ArticleFeatures, DecisionPoint, PolicyDecision};
use da_ports::fsm_policy::{FsmPolicyError, FsmPolicyPort, FsmPolicyResult};

use crate::fsm_policy_fallback::NativeFallbackPolicy;

/// Prefer `host` when it can serve the point; otherwise native fallback.
pub fn consult_with_fallback(
    host: Option<&dyn FsmPolicyPort>,
    yaml: &str,
    point: DecisionPoint,
    features: &ArticleFeatures,
) -> FsmPolicyResult<PolicyDecision> {
    if let Some(h) = host {
        match h.consult(point, features) {
            Ok(d) => return Ok(d),
            Err(FsmPolicyError::UnknownPoint(_)) => {}
            Err(FsmPolicyError::Load(_) | FsmPolicyError::Instantiate(_) | FsmPolicyError::Io(_)) => {
                // fail-open to native; trap/invalid-code stay errors
            }
            Err(e) => return Err(e),
        }
    }
    NativeFallbackPolicy::from_yaml(yaml)?.consult(point, features)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::fsm_policy::{GrammarRoute, PolicySource};

    struct BrokenHost;

    impl FsmPolicyPort for BrokenHost {
        fn consult(
            &self,
            _point: DecisionPoint,
            _features: &ArticleFeatures,
        ) -> FsmPolicyResult<PolicyDecision> {
            Err(FsmPolicyError::Load("missing wasm".into()))
        }
    }

    #[test]
    fn broken_host_falls_open_to_native() {
        let feat = ArticleFeatures {
            page_count: 18,
            toc_bookmarks: 0,
            text_layer_quality: 0.9,
            domain: String::new(),
        };
        let d = consult_with_fallback(
            Some(&BrokenHost),
            "decision_points: {}",
            DecisionPoint::GrammarRoute,
            &feat,
        )
        .unwrap();
        assert_eq!(d.id.source, PolicySource::Native);
        assert_eq!(d.route, Some(GrammarRoute::XyCut));
    }
}
