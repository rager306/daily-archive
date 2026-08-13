//! Classify ArticleLocator records without inventing implicit v1 (M383 S01).

use crate::revision_materialization::{RevisionContentContext, RevisionManifestationInput};
use da_domain::{
    ArticleLocator, RevisionClass, RevisionClassification, RevisionClassificationPolicy,
    classify_revision,
};

#[derive(Debug, thiserror::Error)]
pub enum RevisionClassifyError {
    #[error("{0}")]
    Policy(String),
}

/// Application facade over bundled revision classification policy.
pub struct RevisionClassifyUseCase {
    policy: RevisionClassificationPolicy,
}

impl RevisionClassifyUseCase {
    pub fn bundled() -> Result<Self, RevisionClassifyError> {
        Ok(Self {
            policy: RevisionClassificationPolicy::bundled()
                .map_err(RevisionClassifyError::Policy)?,
        })
    }

    pub fn new(policy: RevisionClassificationPolicy) -> Self {
        Self { policy }
    }

    /// Classify an arXiv locator. Other locator kinds are VersionlessPolicy
    /// only when the policy allows versionless; they never become Exact.
    pub fn classify_locator(
        &self,
        locator: &ArticleLocator,
        has_manifestation: bool,
    ) -> Result<RevisionClassification, RevisionClassifyError> {
        match locator {
            ArticleLocator::Arxiv { id, revision } => {
                classify_revision(&self.policy, id, *revision, has_manifestation)
                    .map_err(RevisionClassifyError::Policy)
            }
            other => classify_revision(&self.policy, &other.source_key(), None, has_manifestation)
                .map_err(RevisionClassifyError::Policy),
        }
    }

    /// Exact + manifestations → revision context. Versionless → None. Blocked → error.
    /// Never invents revision=1.
    pub fn revision_context_for_classified_load(
        &self,
        locator: &ArticleLocator,
        source_code: &str,
        paper_id: &str,
        manifestations: Vec<RevisionManifestationInput>,
    ) -> Result<Option<RevisionContentContext>, RevisionClassifyError> {
        let has_manifestation = !manifestations.is_empty();
        let classified = self.classify_locator(locator, has_manifestation)?;
        match classified.class {
            RevisionClass::VersionlessPolicy => Ok(None),
            RevisionClass::Blocked => Err(RevisionClassifyError::Policy(classified.reason)),
            RevisionClass::Exact => {
                let revision = classified.explicit_revision.ok_or_else(|| {
                    RevisionClassifyError::Policy("exact class missing explicit_revision".into())
                })?;
                if manifestations.is_empty() {
                    return Err(RevisionClassifyError::Policy(
                        "exact revision lacks checksum-bound manifestation".into(),
                    ));
                }
                Ok(Some(RevisionContentContext {
                    source_code: source_code.to_string(),
                    canonical_id: paper_id.to_string(),
                    revision,
                    manifestations,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::RevisionClass;

    #[test]
    fn explicit_vn_locator_is_exact() {
        let uc = RevisionClassifyUseCase::bundled().unwrap();
        let loc = ArticleLocator::parse("https://arxiv.org/html/2506.07853v5").unwrap();
        let c = uc.classify_locator(&loc, true).unwrap();
        assert_eq!(c.class, RevisionClass::Exact);
        assert_eq!(c.explicit_revision, Some(5));
        assert_eq!(c.canonical_id, "2506.07853");
    }

    #[test]
    fn versionless_locator_is_not_implicit_v1() {
        let uc = RevisionClassifyUseCase::bundled().unwrap();
        let loc = ArticleLocator::parse("2506.07853").unwrap();
        let c = uc.classify_locator(&loc, true).unwrap();
        assert_eq!(c.class, RevisionClass::VersionlessPolicy);
        assert_eq!(c.explicit_revision, None);
        assert_ne!(c.explicit_revision, Some(1));
    }

    #[test]
    fn exact_without_manifestation_is_blocked() {
        let uc = RevisionClassifyUseCase::bundled().unwrap();
        let loc = ArticleLocator::parse("2506.07853v5").unwrap();
        let c = uc.classify_locator(&loc, false).unwrap();
        assert_eq!(c.class, RevisionClass::Blocked);
    }

    fn pdf_manifestation() -> RevisionManifestationInput {
        RevisionManifestationInput {
            format: "pdf".into(),
            sha256: "a".repeat(64),
            bytes: 12,
            uri: "catalog:x.pdf".into(),
            path: "x.pdf".into(),
            capture_status: "captured_local".into(),
        }
    }

    #[test]
    fn exact_with_manifestation_builds_context() {
        let uc = RevisionClassifyUseCase::bundled().unwrap();
        let loc = ArticleLocator::parse("2506.07853v5").unwrap();
        let ctx = uc
            .revision_context_for_classified_load(
                &loc,
                "arxiv",
                "2506.07853",
                vec![pdf_manifestation()],
            )
            .unwrap()
            .expect("exact context");
        assert_eq!(ctx.revision, 5);
        assert_eq!(ctx.canonical_id, "2506.07853");
    }

    #[test]
    fn versionless_does_not_invent_v1_context() {
        let uc = RevisionClassifyUseCase::bundled().unwrap();
        let loc = ArticleLocator::parse("2506.07853").unwrap();
        let ctx = uc
            .revision_context_for_classified_load(
                &loc,
                "arxiv",
                "2506.07853",
                vec![pdf_manifestation()],
            )
            .unwrap();
        assert!(ctx.is_none());
    }

    #[test]
    fn exact_without_manifestation_context_errors() {
        let uc = RevisionClassifyUseCase::bundled().unwrap();
        let loc = ArticleLocator::parse("2506.07853v5").unwrap();
        let err = uc
            .revision_context_for_classified_load(&loc, "arxiv", "2506.07853", vec![])
            .unwrap_err();
        assert!(err.to_string().contains("manifestation"));
    }
}
