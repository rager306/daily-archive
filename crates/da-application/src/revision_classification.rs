//! Classify ArticleLocator records without inventing implicit v1 (M383 S01).

use da_domain::{
    ArticleLocator, RevisionClassification, RevisionClassificationPolicy, classify_revision,
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
}
