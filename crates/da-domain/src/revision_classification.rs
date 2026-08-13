//! Revision classification for corpus inventory (M383 S01, ADR-058).
//!
//! Classifies a source key as Exact, VersionlessPolicy, or Blocked.
//! Never invents revision=1 for a versionless locator.

use serde::{Deserialize, Serialize};

const BUNDLED_POLICY: &str =
    include_str!("../../../data/ontology/revision_classification_policy.yaml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionClass {
    /// Explicit vN and (when required) a manifestation is present.
    Exact,
    /// Versionless locator allowed by DATA policy. Not implicit v1.
    VersionlessPolicy,
    /// Missing explicit revision when forbidden, or Exact without required manifestation.
    Blocked,
}

impl RevisionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::VersionlessPolicy => "versionless_policy",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionClassificationPolicy {
    pub schema_version: String,
    pub import_eligible: bool,
    pub allow_versionless: bool,
    pub require_manifestation_for_exact: bool,
}

impl RevisionClassificationPolicy {
    pub fn bundled() -> Result<Self, String> {
        Self::from_yaml(BUNDLED_POLICY)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        let policy: Self = serde_yaml::from_str(yaml).map_err(|e| e.to_string())?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "revision-classification-policy.v1" {
            return Err(format!(
                "unsupported revision classification policy: {}",
                self.schema_version
            ));
        }
        if self.import_eligible {
            return Err("revision classification policy must keep import_eligible=false".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionClassification {
    pub class: RevisionClass,
    pub canonical_id: String,
    pub explicit_revision: Option<u32>,
    pub has_manifestation: bool,
    pub reason: String,
    pub import_eligible: bool,
}

impl RevisionClassification {
    pub fn validate(&self) -> Result<(), String> {
        if self.canonical_id.trim().is_empty() {
            return Err("canonical_id must be non-empty".into());
        }
        if self.import_eligible {
            return Err("classification must keep import_eligible=false".into());
        }
        if self.class == RevisionClass::Exact && self.explicit_revision.is_none() {
            return Err("exact class requires explicit_revision".into());
        }
        Ok(())
    }
}

/// Classify a catalog/source record. `explicit_revision` is Some only when
/// the locator or article_key carried an explicit vN. Never default to 1.
pub fn classify_revision(
    policy: &RevisionClassificationPolicy,
    canonical_id: &str,
    explicit_revision: Option<u32>,
    has_manifestation: bool,
) -> Result<RevisionClassification, String> {
    policy.validate()?;
    if canonical_id.trim().is_empty() {
        return Err("canonical_id must be non-empty".into());
    }
    if let Some(rev) = explicit_revision {
        if rev == 0 {
            return Err("explicit_revision must be >= 1 when present".into());
        }
        let (class, reason) = if policy.require_manifestation_for_exact && !has_manifestation {
            (
                RevisionClass::Blocked,
                "exact revision lacks checksum-bound manifestation".to_string(),
            )
        } else {
            (RevisionClass::Exact, format!("explicit revision v{rev}"))
        };
        let out = RevisionClassification {
            class,
            canonical_id: canonical_id.to_string(),
            explicit_revision: Some(rev),
            has_manifestation,
            reason,
            import_eligible: false,
        };
        out.validate()?;
        return Ok(out);
    }

    let (class, reason) = if policy.allow_versionless {
        (
            RevisionClass::VersionlessPolicy,
            "versionless locator is an explicit policy case, not implicit v1".to_string(),
        )
    } else {
        (
            RevisionClass::Blocked,
            "versionless locator forbidden by policy".to_string(),
        )
    };
    let out = RevisionClassification {
        class,
        canonical_id: canonical_id.to_string(),
        explicit_revision: None,
        has_manifestation,
        reason,
        import_eligible: false,
    };
    out.validate()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_policy_allows_versionless_and_requires_manifestation() {
        let p = RevisionClassificationPolicy::bundled().expect("bundled");
        assert!(p.allow_versionless);
        assert!(p.require_manifestation_for_exact);
        assert!(!p.import_eligible);
    }

    #[test]
    fn explicit_revision_with_manifestation_is_exact() {
        let p = RevisionClassificationPolicy::bundled().unwrap();
        let c = classify_revision(&p, "2506.07853", Some(5), true).unwrap();
        assert_eq!(c.class, RevisionClass::Exact);
        assert_eq!(c.explicit_revision, Some(5));
        assert!(!c.import_eligible);
    }

    #[test]
    fn explicit_revision_without_manifestation_is_blocked() {
        let p = RevisionClassificationPolicy::bundled().unwrap();
        let c = classify_revision(&p, "2506.07853", Some(5), false).unwrap();
        assert_eq!(c.class, RevisionClass::Blocked);
        assert!(c.reason.contains("manifestation"));
    }

    #[test]
    fn versionless_is_policy_case_not_implicit_v1() {
        let p = RevisionClassificationPolicy::bundled().unwrap();
        let c = classify_revision(&p, "2506.07853", None, true).unwrap();
        assert_eq!(c.class, RevisionClass::VersionlessPolicy);
        assert_eq!(c.explicit_revision, None);
        assert_ne!(c.explicit_revision, Some(1));
    }

    #[test]
    fn versionless_blocked_when_policy_forbids() {
        let mut p = RevisionClassificationPolicy::bundled().unwrap();
        p.allow_versionless = false;
        let c = classify_revision(&p, "2506.07853", None, true).unwrap();
        assert_eq!(c.class, RevisionClass::Blocked);
    }

    #[test]
    fn never_accepts_revision_zero() {
        let p = RevisionClassificationPolicy::bundled().unwrap();
        assert!(classify_revision(&p, "2506.07853", Some(0), true).is_err());
    }
}
