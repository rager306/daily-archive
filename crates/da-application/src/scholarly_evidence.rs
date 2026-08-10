//! Application facade for multi-provider scholarly evidence reconcile (M371/S11).
//!
//! Pure: no filesystem, no network, no graph writes. Callers inject provider and
//! TeX observations already captured as durable artifacts.

use da_domain::scholarly_evidence::{
    ScholarlyEvidenceError, ScholarlyEvidenceInput, ScholarlyEvidenceReport,
    reconcile_scholarly_evidence,
};

pub struct ScholarlyEvidenceUseCase;

impl ScholarlyEvidenceUseCase {
    pub fn reconcile(
        input: ScholarlyEvidenceInput,
    ) -> Result<ScholarlyEvidenceReport, ScholarlyEvidenceError> {
        reconcile_scholarly_evidence(input)
    }
}
