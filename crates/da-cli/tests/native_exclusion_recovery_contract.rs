//! S04/T05 (M514-1lss2s) — contract tests for the exclusion-recovery receipt.
//!
//! These tests are hermetic: they build the receipt from inline fixtures only,
//! so they need no PDF, no layout feature and no external corpus. They pin the
//! invariants the runner relies on: the frozen 72/5/2 denominator stays the
//! denominator, a smoke run can never claim completeness, a field state that
//! contradicts its disposition is refused, and a typed refusal always carries a
//! reason.

use da_cli::native_family_receipt::{
    AffiliationReference, CLAIM_SCOPE_DIAGNOSTIC_ONLY, EXCLUSION_REASON_ORDER, ExclusionReason,
    ExclusionRecoveryRow, ExclusionRefusalReason, FieldDisposition, FieldRecovery, GuardProvenance,
    NativeExclusionRecoveryReceipt, NativeFamilyReceiptError, PRODUCT_COVERAGE_EFFECT_NONE,
    PopulationId, PopulationKind, REQUIRED_LOCKS, ReceiptKind, RecoveryDisposition, RecoveryField,
    RecoveryMode, RecoveryPopulation, RefusalReason, THRESHOLDS_NOT_ACCEPTED, assert_redacted,
    build_exclusion_recovery_receipt, sha256_prefixed,
};
use std::collections::BTreeMap;

fn locks() -> BTreeMap<String, String> {
    REQUIRED_LOCKS
        .iter()
        .map(|(lock, value)| ((*lock).to_owned(), (*value).to_owned()))
        .collect()
}

fn guard() -> GuardProvenance {
    GuardProvenance {
        installed: true,
        identity_isolated: true,
        observed_java_calls: 0,
    }
}

fn reference() -> AffiliationReference {
    AffiliationReference {
        metric_id: "extraction.native_capability_recall".to_owned(),
        recovered: 3055,
        denominator: 7077,
    }
}

fn population(mode: RecoveryMode, targeted: u64) -> RecoveryPopulation {
    let mut reasons = BTreeMap::new();
    reasons.insert("header_date_absent".to_owned(), 72);
    reasons.insert("header_title_absent".to_owned(), 5);
    reasons.insert("header_author_absent".to_owned(), 2);
    RecoveryPopulation {
        population_id: PopulationId::ExclusionBaseline,
        population_kind: PopulationKind::Exclusion,
        mode,
        assumed_complete: mode == RecoveryMode::Full,
        assigned: 610,
        excluded: 79,
        measurable: 531,
        targeted,
        reasons,
    }
}

fn make_row(
    id: &str,
    reason: ExclusionReason,
    disposition: RecoveryDisposition,
    refusal: Option<ExclusionRefusalReason>,
) -> ExclusionRecoveryRow {
    let field_state = |target: RecoveryField| -> FieldDisposition {
        if disposition == RecoveryDisposition::TypedRefusal {
            return FieldDisposition::NotRecovered;
        }
        if reason.field() == target {
            match disposition {
                RecoveryDisposition::Recovered => FieldDisposition::Recovered,
                RecoveryDisposition::NotRecovered => FieldDisposition::NotRecovered,
                RecoveryDisposition::OutOfScope => FieldDisposition::OutOfScope,
                RecoveryDisposition::TypedRefusal => FieldDisposition::NotRecovered,
            }
        } else if target == RecoveryField::Title {
            FieldDisposition::OutOfScope
        } else {
            FieldDisposition::NotRecovered
        }
    };
    let band_refusal = if disposition == RecoveryDisposition::TypedRefusal {
        Some(RefusalReason::MissingFontSize)
    } else {
        None
    };
    ExclusionRecoveryRow {
        document_id: id.to_owned(),
        reason,
        probe_identity_sha256: "0".repeat(64),
        probe_size_bytes: 1,
        source_pdf: Some("data/sources/arxiv/cs-cl/1206.6423/source/1206.6423.pdf".to_owned()),
        pdf_sha256: Some("0".repeat(64)),
        pdf_sha256_pinned: Some("0".repeat(64)),
        pdf_size_bytes: Some(1),
        pipeline_digest: Some(sha256_prefixed(b"pipeline")),
        disposition,
        fields: vec![
            FieldRecovery {
                field: RecoveryField::Date,
                disposition: field_state(RecoveryField::Date),
            },
            FieldRecovery {
                field: RecoveryField::Title,
                disposition: field_state(RecoveryField::Title),
            },
            FieldRecovery {
                field: RecoveryField::Author,
                disposition: field_state(RecoveryField::Author),
            },
        ],
        band_typed_absence: false,
        band_refusal,
        refusal,
    }
}

fn full_rows() -> Vec<ExclusionRecoveryRow> {
    let mut rows = Vec::new();
    for index in 0..72 {
        rows.push(make_row(
            &format!("date-{index}"),
            ExclusionReason::HeaderDateAbsent,
            RecoveryDisposition::Recovered,
            None,
        ));
    }
    for index in 0..5 {
        rows.push(make_row(
            &format!("title-{index}"),
            ExclusionReason::HeaderTitleAbsent,
            RecoveryDisposition::OutOfScope,
            None,
        ));
    }
    for index in 0..2 {
        rows.push(make_row(
            &format!("author-{index}"),
            ExclusionReason::HeaderAuthorAbsent,
            RecoveryDisposition::NotRecovered,
            None,
        ));
    }
    rows
}

fn build(
    population: &RecoveryPopulation,
    rows: &[ExclusionRecoveryRow],
) -> Result<NativeExclusionRecoveryReceipt, NativeFamilyReceiptError> {
    build_exclusion_recovery_receipt(
        &sha256_prefixed(b"manifest"),
        &sha256_prefixed(b"probe"),
        &sha256_prefixed(b"baseline"),
        &locks(),
        CLAIM_SCOPE_DIAGNOSTIC_ONLY,
        PRODUCT_COVERAGE_EFFECT_NONE,
        guard(),
        population,
        &reference(),
        rows,
        &[],
    )
}

#[test]
fn full_run_over_the_frozen_denominator_is_accepted() {
    let rows = full_rows();
    let receipt = build(&population(RecoveryMode::Full, 79), &rows).expect("full receipt");
    assert_eq!(receipt.receipt_kind, ReceiptKind::ExclusionRecoveryBaseline);
    assert_eq!(receipt.thresholds, THRESHOLDS_NOT_ACCEPTED);
    assert_eq!(
        receipt.product_coverage_effect,
        PRODUCT_COVERAGE_EFFECT_NONE
    );
    assert_eq!(receipt.recoveries.len(), 79);
    assert_eq!(receipt.aggregates.len(), 3);
    let date = &receipt.aggregates[0];
    assert_eq!(date.reason, ExclusionReason::HeaderDateAbsent);
    assert_eq!(date.denominator, 72);
    assert_eq!(date.recovered, 72);
    let title = &receipt.aggregates[1];
    assert_eq!(title.denominator, 5);
    assert_eq!(title.out_of_scope, 5);
    let author = &receipt.aggregates[2];
    assert_eq!(author.denominator, 2);
    assert_eq!(author.not_recovered, 2);
    assert_eq!(receipt.affiliation_reference.denominator, 7077);
    assert!(receipt.population.assumed_complete);
}

#[test]
fn smoke_run_is_not_assumed_complete() {
    let rows = vec![
        make_row(
            "date-0",
            ExclusionReason::HeaderDateAbsent,
            RecoveryDisposition::Recovered,
            None,
        ),
        make_row(
            "title-0",
            ExclusionReason::HeaderTitleAbsent,
            RecoveryDisposition::OutOfScope,
            None,
        ),
        make_row(
            "author-0",
            ExclusionReason::HeaderAuthorAbsent,
            RecoveryDisposition::NotRecovered,
            None,
        ),
    ];
    let receipt = build(&population(RecoveryMode::Smoke, 3), &rows).expect("smoke receipt");
    assert!(!receipt.population.assumed_complete);
    assert_eq!(receipt.population.mode, RecoveryMode::Smoke);
    assert_eq!(receipt.recoveries.len(), 3);
    assert_eq!(receipt.aggregates[0].denominator, 1);
    assert_eq!(receipt.aggregates[1].out_of_scope, 1);
    assert_eq!(receipt.aggregates[2].not_recovered, 1);
}

#[test]
fn smoke_receipt_that_claims_completeness_is_refused() {
    let rows = vec![make_row(
        "date-0",
        ExclusionReason::HeaderDateAbsent,
        RecoveryDisposition::Recovered,
        None,
    )];
    let mut population = population(RecoveryMode::Smoke, 1);
    population.assumed_complete = true;
    let error = build(&population, &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionPopulationCoverage { .. }
    ));
}

#[test]
fn a_field_that_contradicts_its_disposition_is_refused() {
    let mut rows = full_rows();
    rows[0].fields[0].disposition = FieldDisposition::NotRecovered;
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionDispositionMismatch { .. }
    ));
}

#[test]
fn a_typed_refusal_must_carry_a_refusal_reason() {
    let mut rows = full_rows();
    rows[0].disposition = RecoveryDisposition::TypedRefusal;
    rows[0].band_refusal = None;
    rows[0].refusal = None;
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionRefusalMissing { .. }
    ));
}

#[test]
fn a_non_refusal_must_not_carry_a_refusal_reason() {
    let mut rows = full_rows();
    rows[0].refusal = Some(ExclusionRefusalReason::PdfShaDrift);
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionRefusalInconsistent { .. }
    ));
}

#[test]
fn a_duplicate_document_is_refused() {
    let rows = vec![
        make_row(
            "date-0",
            ExclusionReason::HeaderDateAbsent,
            RecoveryDisposition::Recovered,
            None,
        ),
        make_row(
            "date-1",
            ExclusionReason::HeaderDateAbsent,
            RecoveryDisposition::Recovered,
            None,
        ),
        make_row(
            "title-0",
            ExclusionReason::HeaderTitleAbsent,
            RecoveryDisposition::OutOfScope,
            None,
        ),
        make_row(
            "date-0",
            ExclusionReason::HeaderDateAbsent,
            RecoveryDisposition::Recovered,
            None,
        ),
    ];
    let error = build(&population(RecoveryMode::Smoke, 4), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionDuplicateDocument { .. }
    ));
}

#[test]
fn a_typed_refusal_row_is_accepted_with_a_reason() {
    let mut rows = full_rows();
    rows[71].disposition = RecoveryDisposition::TypedRefusal;
    rows[71].refusal = Some(ExclusionRefusalReason::PdfShaDrift);
    rows[71].band_refusal = Some(RefusalReason::MissingFontSize);
    let receipt = build(&population(RecoveryMode::Full, 79), &rows).expect("refusal row");
    let date = &receipt.aggregates[0];
    assert_eq!(date.recovered, 71);
    assert_eq!(date.typed_refusal, 1);
}

#[test]
fn a_java_call_on_the_seam_is_refused() {
    let rows = full_rows();
    let error = build_exclusion_recovery_receipt(
        &sha256_prefixed(b"manifest"),
        &sha256_prefixed(b"probe"),
        &sha256_prefixed(b"baseline"),
        &locks(),
        CLAIM_SCOPE_DIAGNOSTIC_ONLY,
        PRODUCT_COVERAGE_EFFECT_NONE,
        GuardProvenance {
            installed: true,
            identity_isolated: true,
            observed_java_calls: 3,
        },
        &population(RecoveryMode::Full, 79),
        &reference(),
        &rows,
        &[],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ObservedJavaOnSlice { attempts: 3 }
    ));
}

#[test]
fn an_unknown_reason_token_is_refused() {
    let rows = full_rows();
    let mut population = population(RecoveryMode::Full, 79);
    population.reasons.insert("header_bogus".to_owned(), 0);
    let error = build(&population, &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionUnknownReason { .. }
    ));
}

#[test]
fn a_denominator_that_does_not_sum_is_refused() {
    let rows = full_rows();
    let mut population = population(RecoveryMode::Full, 79);
    population.excluded = 80;
    let error = build(&population, &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionDenominatorDrift { .. }
    ));
}

#[test]
fn a_non_diagnostic_claim_scope_is_refused() {
    let rows = full_rows();
    let error = build_exclusion_recovery_receipt(
        &sha256_prefixed(b"manifest"),
        &sha256_prefixed(b"probe"),
        &sha256_prefixed(b"baseline"),
        &locks(),
        "product",
        PRODUCT_COVERAGE_EFFECT_NONE,
        guard(),
        &population(RecoveryMode::Full, 79),
        &reference(),
        &rows,
        &[],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ClaimScopeDrift { .. }
    ));
}

#[test]
fn a_missing_affiliation_reference_is_refused() {
    let rows = full_rows();
    let error = build_exclusion_recovery_receipt(
        &sha256_prefixed(b"manifest"),
        &sha256_prefixed(b"probe"),
        &sha256_prefixed(b"baseline"),
        &locks(),
        CLAIM_SCOPE_DIAGNOSTIC_ONLY,
        PRODUCT_COVERAGE_EFFECT_NONE,
        guard(),
        &population(RecoveryMode::Full, 79),
        &AffiliationReference {
            metric_id: String::new(),
            recovered: 3055,
            denominator: 7077,
        },
        &rows,
        &[],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::AffiliationReferenceMissing
    ));
}

#[test]
fn reason_and_field_vocabularies_round_trip() {
    for reason in EXCLUSION_REASON_ORDER {
        assert_eq!(ExclusionReason::parse(reason.as_str()), Some(reason));
        assert!(!reason.field().as_str().is_empty());
    }
    assert!(ExclusionReason::parse("header_bogus").is_none());
    assert_eq!(
        ExclusionReason::HeaderDateAbsent.field(),
        RecoveryField::Date
    );
    assert_eq!(
        ExclusionReason::HeaderTitleAbsent.field(),
        RecoveryField::Title
    );
    assert_eq!(
        ExclusionReason::HeaderAuthorAbsent.field(),
        RecoveryField::Author
    );
}

#[test]
fn the_artifact_round_trips_and_is_redacted() {
    let rows = full_rows();
    let receipt = build(&population(RecoveryMode::Full, 79), &rows).expect("full receipt");
    let bytes = receipt.artifact_json_bytes().expect("serializable");
    let text = std::str::from_utf8(&bytes).expect("utf8");
    assert_redacted(text).expect("no payload marker leaked");
    assert!(!text.contains("<TEI"));
    let parsed = NativeExclusionRecoveryReceipt::parse_json_bytes(&bytes).expect("replay");
    assert_eq!(parsed, receipt);
}
