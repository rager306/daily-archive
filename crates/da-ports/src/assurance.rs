//! Driven ports for durable ACAP evidence (ADR-055).

use da_domain::ArticleAssuranceReport;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AssuranceStoreError {
    #[error("assurance report already exists with different content for {article_id}")]
    Conflict { article_id: String },
    #[error("assurance report failed integrity validation for {article_id}")]
    Integrity { article_id: String },
    #[error("assurance storage failure: {0}")]
    Storage(String),
}

/// Durable report boundary. Implementations must be idempotent for byte-equivalent
/// reports and fail closed rather than overwrite conflicting evidence.
pub trait AssuranceReportStore: Send + Sync {
    fn load(&self, article_id: &str)
    -> Result<Option<ArticleAssuranceReport>, AssuranceStoreError>;
    fn store(&self, report: &ArticleAssuranceReport) -> Result<(), AssuranceStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::{ArticleContextState, ContentDigest};
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeStore {
        reports: Mutex<BTreeMap<String, ArticleAssuranceReport>>,
    }

    impl AssuranceReportStore for FakeStore {
        fn load(
            &self,
            article_id: &str,
        ) -> Result<Option<ArticleAssuranceReport>, AssuranceStoreError> {
            Ok(self.reports.lock().unwrap().get(article_id).cloned())
        }

        fn store(&self, report: &ArticleAssuranceReport) -> Result<(), AssuranceStoreError> {
            let mut reports = self.reports.lock().unwrap();
            match reports.get(&report.article_id) {
                Some(existing) if existing == report => Ok(()),
                Some(_) => Err(AssuranceStoreError::Conflict {
                    article_id: report.article_id.clone(),
                }),
                None => {
                    reports.insert(report.article_id.clone(), report.clone());
                    Ok(())
                }
            }
        }
    }

    fn report(article_id: &str, policy: &str) -> ArticleAssuranceReport {
        ArticleAssuranceReport::new(
            article_id,
            ContentDigest::of_text(policy),
            ArticleContextState::Materialized,
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn assurance_report_store_contract_roundtrips_and_is_idempotent() {
        let store = FakeStore::default();
        let expected = report("2507.19457", "policy-v1");
        store.store(&expected).unwrap();
        store.store(&expected).unwrap();
        assert_eq!(store.load("2507.19457").unwrap(), Some(expected));
    }

    #[test]
    fn assurance_report_store_contract_rejects_conflicting_overwrite() {
        let store = FakeStore::default();
        store.store(&report("2507.19457", "policy-v1")).unwrap();
        let error = store.store(&report("2507.19457", "policy-v2")).unwrap_err();
        assert_eq!(
            error,
            AssuranceStoreError::Conflict {
                article_id: "2507.19457".into()
            }
        );
    }
}
