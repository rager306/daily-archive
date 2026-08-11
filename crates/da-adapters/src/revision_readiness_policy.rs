//! Adapter for revision readiness ontology policy DATA and canonical identity.

use da_domain::revision_readiness::{RevisionReadinessPolicy, RevisionReadinessPolicyError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const BUNDLED_YAML: &str = include_str!("../../../data/ontology/revision_readiness_policy.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionReadinessPolicyIdentity {
    pub schema_version: String,
    pub configuration_sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RevisionReadinessPolicyLoadError {
    #[error("read revision readiness policy {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("decode revision readiness policy: {0}")]
    Decode(#[from] serde_yaml::Error),
    #[error(transparent)]
    Invalid(#[from] RevisionReadinessPolicyError),
    #[error("serialize revision readiness policy: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct LoadedRevisionReadinessPolicy {
    policy: RevisionReadinessPolicy,
    identity: RevisionReadinessPolicyIdentity,
}

impl LoadedRevisionReadinessPolicy {
    pub fn bundled() -> Result<Self, RevisionReadinessPolicyLoadError> {
        Self::from_yaml(BUNDLED_YAML)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, RevisionReadinessPolicyLoadError> {
        let path = path.as_ref();
        let yaml = std::fs::read_to_string(path).map_err(|source| {
            RevisionReadinessPolicyLoadError::Read {
                path: path.display().to_string(),
                source,
            }
        })?;
        Self::from_yaml(&yaml)
    }

    pub fn load_project() -> Result<Self, RevisionReadinessPolicyLoadError> {
        for path in [
            "data/ontology/revision_readiness_policy.yaml",
            "../../data/ontology/revision_readiness_policy.yaml",
            "../../../data/ontology/revision_readiness_policy.yaml",
        ] {
            if Path::new(path).exists() {
                return Self::from_file(path);
            }
        }
        Self::bundled()
    }

    pub fn policy(&self) -> &RevisionReadinessPolicy {
        &self.policy
    }

    pub fn identity(&self) -> &RevisionReadinessPolicyIdentity {
        &self.identity
    }

    fn from_yaml(yaml: &str) -> Result<Self, RevisionReadinessPolicyLoadError> {
        let policy: RevisionReadinessPolicy = serde_yaml::from_str(yaml)?;
        policy.validate()?;
        let canonical = serde_json::to_vec(&policy)?;
        let identity = RevisionReadinessPolicyIdentity {
            schema_version: policy.schema_version.clone(),
            configuration_sha256: hex::encode(Sha256::digest(canonical)),
        };
        Ok(Self { policy, identity })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_and_project_values_have_same_identity() {
        let bundled = LoadedRevisionReadinessPolicy::bundled().unwrap();
        let project = LoadedRevisionReadinessPolicy::load_project().unwrap();
        assert_eq!(bundled.policy(), project.policy());
        assert_eq!(bundled.identity(), project.identity());
    }

    #[test]
    fn threshold_mutation_changes_configuration_identity() {
        let baseline = LoadedRevisionReadinessPolicy::bundled().unwrap();
        let changed = LoadedRevisionReadinessPolicy::from_yaml(&BUNDLED_YAML.replace(
            "min_summary_coverage_bps: 8000",
            "min_summary_coverage_bps: 9000",
        ))
        .unwrap();
        assert_eq!(
            baseline.identity().schema_version,
            changed.identity().schema_version
        );
        assert_ne!(
            baseline.identity().configuration_sha256,
            changed.identity().configuration_sha256
        );
    }

    #[test]
    fn invalid_policy_fails_closed() {
        let invalid = BUNDLED_YAML.replace("min_sections: 1", "min_sections: 0");
        assert!(matches!(
            LoadedRevisionReadinessPolicy::from_yaml(&invalid),
            Err(RevisionReadinessPolicyLoadError::Invalid(
                RevisionReadinessPolicyError::ZeroMinSections
            ))
        ));
    }
}
