//! Filesystem/bundled adapter for ontology-aligned Section summary role DATA.

use da_domain::section_summary_policy::{SectionRolePolicy, SectionRolePolicyError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const BUNDLED_POLICY_YAML: &str = include_str!("../../../data/ontology/section_summary_roles.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionRolePolicyIdentity {
    pub schema_version: String,
    pub configuration_sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SectionRolePolicyLoadError {
    #[error("read Section summary role policy {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("decode Section summary role policy: {0}")]
    Decode(#[from] serde_yaml::Error),
    #[error(transparent)]
    Invalid(#[from] SectionRolePolicyError),
    #[error("serialize validated Section summary role policy: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct LoadedSectionRolePolicy {
    policy: SectionRolePolicy,
    identity: SectionRolePolicyIdentity,
}

impl LoadedSectionRolePolicy {
    pub fn bundled() -> Result<Self, SectionRolePolicyLoadError> {
        Self::from_yaml(BUNDLED_POLICY_YAML)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, SectionRolePolicyLoadError> {
        let path = path.as_ref();
        let yaml =
            std::fs::read_to_string(path).map_err(|source| SectionRolePolicyLoadError::Read {
                path: path.display().to_string(),
                source,
            })?;
        Self::from_yaml(&yaml)
    }

    pub fn load_project() -> Result<Self, SectionRolePolicyLoadError> {
        for path in [
            "data/ontology/section_summary_roles.yaml",
            "../../data/ontology/section_summary_roles.yaml",
            "../../../data/ontology/section_summary_roles.yaml",
        ] {
            if Path::new(path).exists() {
                return Self::from_file(path);
            }
        }
        Self::bundled()
    }

    pub fn policy(&self) -> &SectionRolePolicy {
        &self.policy
    }

    pub fn identity(&self) -> &SectionRolePolicyIdentity {
        &self.identity
    }

    fn from_yaml(yaml: &str) -> Result<Self, SectionRolePolicyLoadError> {
        let policy: SectionRolePolicy = serde_yaml::from_str(yaml)?;
        policy.validate()?;
        let canonical = serde_json::to_vec(&policy)?;
        let identity = SectionRolePolicyIdentity {
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
        let bundled = LoadedSectionRolePolicy::bundled().unwrap();
        let project = LoadedSectionRolePolicy::load_project().unwrap();
        assert_eq!(bundled.policy(), project.policy());
        assert_eq!(bundled.identity(), project.identity());
        assert_eq!(
            project
                .policy()
                .classify("Methods")
                .unwrap()
                .summary_type
                .as_str(),
            "methods"
        );
        assert_eq!(
            project
                .policy()
                .classify("Results")
                .unwrap()
                .summary_type
                .as_str(),
            "results"
        );
    }

    #[test]
    fn rule_mutation_invalidates_only_configuration_hash() {
        let baseline = LoadedSectionRolePolicy::bundled().unwrap();
        let mutated = LoadedSectionRolePolicy::from_yaml(
            &BUNDLED_POLICY_YAML.replace("      - empirical evaluation\n", ""),
        )
        .unwrap();
        assert_eq!(
            baseline.identity().schema_version,
            mutated.identity().schema_version
        );
        assert_ne!(
            baseline.identity().configuration_sha256,
            mutated.identity().configuration_sha256
        );
    }

    #[test]
    fn invalid_duplicate_alias_fails_closed() {
        let invalid = BUNDLED_POLICY_YAML.replace("      - result\n", "      - methods\n");
        assert!(matches!(
            LoadedSectionRolePolicy::from_yaml(&invalid),
            Err(SectionRolePolicyLoadError::Invalid(
                SectionRolePolicyError::DuplicateAlias(_)
            ))
        ));
    }
}
