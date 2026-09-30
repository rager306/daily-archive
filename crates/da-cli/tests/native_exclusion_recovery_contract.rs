//! S04/T05 (M514-1lss2s) — contract tests for the exclusion-recovery receipt.
//!
//! These tests are hermetic: they build the receipt from inline fixtures only,
//! so they need no PDF, no layout feature and no external corpus. They pin the
//! invariants the runner relies on: the frozen 72/5/2 denominator stays the
//! denominator, a smoke run can never claim completeness, a field state that
//! contradicts its disposition is refused, and a typed refusal always carries a
//! reason.
//!
//! They also pin the D502 honesty rule this task exists to enforce: atom
//! recognition is **not** field attribution, so no producer may claim a field
//! recovered, `recognition_only` is explicitly *not* a recovery (it never adds
//! to an aggregate's `recovered` count), and the negative cross-recognition
//! control must be well formed and bound to the field's own atom.

use da_cli::native_family_receipt::{
    AffiliationReference, CLAIM_SCOPE_DIAGNOSTIC_ONLY, EXCLUSION_REASON_ORDER, ExclusionReason,
    ExclusionRecoveryRow, ExclusionRefusalReason, FieldDisposition, FieldNotRecoveredCause,
    FieldRecognition, FieldRecovery, GuardProvenance, NativeExclusionRecoveryReceipt,
    NativeFamilyReceiptError, PRODUCT_COVERAGE_EFFECT_NONE, PopulationId, PopulationKind,
    REQUIRED_LOCKS, ReceiptKind, RecoveryDisposition, RecoveryField, RecoveryFieldAtom,
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

// ─────────────────────────── field fixtures ───────────────────────────

/// A field whose atom recognized lines: `recognition_only`, with the typed
/// attribution-absent cause. This is the state the runner emits for a Date or
/// Author field whose atom recognized at least one native Header line.
fn recognition_only(field: RecoveryField, count: u64) -> FieldRecovery {
    let atom = field.native_atom().expect("this fixture field has an atom");
    FieldRecovery {
        field,
        disposition: FieldDisposition::RecognitionOnly,
        cause: Some(FieldNotRecoveredCause::RecognitionWithoutFieldAttribution),
        recognition: Some(FieldRecognition {
            atom,
            recognized_count: count,
            evidence_digest: sha256_prefixed(field.as_str().as_bytes()),
            co_firing_atoms: Vec::new(),
            same_line_set_atoms: Vec::new(),
        }),
    }
}

/// A field whose atom recognized nothing in the run.
fn no_recognition(field: RecoveryField) -> FieldRecovery {
    FieldRecovery {
        field,
        disposition: FieldDisposition::NotRecovered,
        cause: Some(FieldNotRecoveredCause::NoRecognition),
        recognition: None,
    }
}

/// Title is always out of scope: the T02 header band has no Title atom.
fn title_out_of_scope() -> FieldRecovery {
    FieldRecovery {
        field: RecoveryField::Title,
        disposition: FieldDisposition::OutOfScope,
        cause: Some(FieldNotRecoveredCause::NoNativeAtom),
        recognition: None,
    }
}

/// A block of fields in canonical order (date, title, author).
fn fields(date: FieldRecovery, title: FieldRecovery, author: FieldRecovery) -> Vec<FieldRecovery> {
    vec![date, title, author]
}

/// The field matrix of a row whose band refused: no field can carry recognition.
fn refusal_fields(cause: FieldNotRecoveredCause) -> Vec<FieldRecovery> {
    let not_recovered = |field| FieldRecovery {
        field,
        disposition: FieldDisposition::NotRecovered,
        cause: Some(cause),
        recognition: None,
    };
    fields(
        not_recovered(RecoveryField::Date),
        title_out_of_scope(),
        not_recovered(RecoveryField::Author),
    )
}

#[allow(clippy::too_many_arguments)]
fn row_with(
    id: &str,
    reason: ExclusionReason,
    fields: Vec<FieldRecovery>,
    disposition: RecoveryDisposition,
    band_refusal: Option<RefusalReason>,
    refusal: Option<ExclusionRefusalReason>,
) -> ExclusionRecoveryRow {
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
        fields,
        band_typed_absence: false,
        band_refusal,
        refusal,
    }
}

fn date_row(id: &str, recognized: bool) -> ExclusionRecoveryRow {
    let date = if recognized {
        recognition_only(RecoveryField::Date, 1)
    } else {
        no_recognition(RecoveryField::Date)
    };
    row_with(
        id,
        ExclusionReason::HeaderDateAbsent,
        fields(
            date,
            title_out_of_scope(),
            no_recognition(RecoveryField::Author),
        ),
        RecoveryDisposition::NotRecovered,
        None,
        None,
    )
}

fn title_row(id: &str) -> ExclusionRecoveryRow {
    row_with(
        id,
        ExclusionReason::HeaderTitleAbsent,
        fields(
            no_recognition(RecoveryField::Date),
            title_out_of_scope(),
            no_recognition(RecoveryField::Author),
        ),
        RecoveryDisposition::OutOfScope,
        None,
        None,
    )
}

fn author_row(id: &str) -> ExclusionRecoveryRow {
    row_with(
        id,
        ExclusionReason::HeaderAuthorAbsent,
        fields(
            no_recognition(RecoveryField::Date),
            title_out_of_scope(),
            no_recognition(RecoveryField::Author),
        ),
        RecoveryDisposition::NotRecovered,
        None,
        None,
    )
}

/// A typed-refusal row for a date exclusion: every attributed field is
/// `not_recovered`, none carries recognition.
fn refusal_reason_date_row(id: &str) -> ExclusionRecoveryRow {
    row_with(
        id,
        ExclusionReason::HeaderDateAbsent,
        refusal_fields(FieldNotRecoveredCause::BandRefused),
        RecoveryDisposition::TypedRefusal,
        Some(RefusalReason::MissingFontSize),
        None,
    )
}

fn full_rows() -> Vec<ExclusionRecoveryRow> {
    let mut rows = Vec::new();
    for index in 0..36 {
        rows.push(date_row(&format!("date-rec-{index}"), true));
    }
    for index in 0..36 {
        rows.push(date_row(&format!("date-none-{index}"), false));
    }
    for index in 0..5 {
        rows.push(title_row(&format!("title-{index}")));
    }
    for index in 0..2 {
        rows.push(author_row(&format!("author-{index}")));
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

// ─────────────────────────── denominator ───────────────────────────

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
    // D502: recognition is not attribution, so no date field is `recovered`.
    assert_eq!(date.recovered, 0);
    assert_eq!(date.not_recovered, 72);
    assert_eq!(date.typed_refusal, 0);

    let title = &receipt.aggregates[1];
    assert_eq!(title.denominator, 5);
    assert_eq!(title.out_of_scope, 5);

    let author = &receipt.aggregates[2];
    assert_eq!(author.denominator, 2);
    assert_eq!(author.not_recovered, 2);

    assert_eq!(receipt.affiliation_reference.recovered, 3055);
    assert_eq!(receipt.affiliation_reference.denominator, 7077);
    assert!(receipt.population.assumed_complete);

    // The frozen per-reason denominator is 72/5/2 and sums to the 79 exclusion.
    let reasons: BTreeMap<_, _> = receipt
        .population
        .reasons
        .iter()
        .map(|(reason, count)| (reason.as_str(), *count))
        .collect();
    assert_eq!(reasons.get("header_date_absent"), Some(&72));
    assert_eq!(reasons.get("header_title_absent"), Some(&5));
    assert_eq!(reasons.get("header_author_absent"), Some(&2));
    assert_eq!(receipt.population.excluded, 79);
}

#[test]
fn recognition_only_is_not_counted_as_recovery() {
    let rows = full_rows();
    let receipt = build(&population(RecoveryMode::Full, 79), &rows).expect("full receipt");

    // Every non-title attributed field is either `recognition_only` (the atom
    // recognized lines) or `not_recovered`; none is `recovered`, and the
    // aggregate's `recovered` column stays zero.
    let mut recognition_only_fields = 0usize;
    for row in &receipt.recoveries {
        for field in &row.fields {
            assert_ne!(field.disposition, FieldDisposition::Recovered);
            if field.disposition == FieldDisposition::RecognitionOnly {
                recognition_only_fields += 1;
                assert_eq!(
                    field.cause,
                    Some(FieldNotRecoveredCause::RecognitionWithoutFieldAttribution)
                );
                assert!(
                    field
                        .recognition
                        .as_ref()
                        .is_some_and(|r| r.recognized_count > 0)
                );
            }
        }
    }
    assert_eq!(recognition_only_fields, 36);
    assert_eq!(receipt.aggregates[0].recovered, 0);
}

#[test]
fn a_row_whose_recognition_only_field_mismatches_its_disposition_is_refused() {
    let mut rows = full_rows();
    // A date row's reason field is `recognition_only`, so the row cannot be
    // `out_of_scope`.
    rows[0].disposition = RecoveryDisposition::OutOfScope;
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionDispositionMismatch { .. }
    ));
}

// ─────────────────────────── D502 honesty ───────────────────────────

#[test]
fn a_field_that_claims_recovery_is_refused() {
    let mut rows = full_rows();
    rows[0].fields[0] = FieldRecovery {
        field: RecoveryField::Date,
        disposition: FieldDisposition::Recovered,
        cause: None,
        recognition: None,
    };
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecoveryUnattributed { .. }
    ));
}

#[test]
fn a_row_that_claims_recovery_is_refused() {
    let mut rows = full_rows();
    rows[0].disposition = RecoveryDisposition::Recovered;
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecoveryUnattributed { .. }
    ));
}

#[test]
fn recognition_only_without_evidence_is_refused() {
    let mut rows = full_rows();
    rows[0].fields[0].recognition = None;
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent { .. }
    ));
}

#[test]
fn recognition_only_with_a_zero_count_is_refused() {
    let mut rows = full_rows();
    rows[0].fields[0]
        .recognition
        .as_mut()
        .expect("recognition evidence")
        .recognized_count = 0;
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent { .. }
    ));
}

#[test]
fn recognition_only_requires_the_attribution_cause() {
    let mut rows = full_rows();
    rows[0].fields[0].cause = Some(FieldNotRecoveredCause::NoRecognition);
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldCauseInconsistent { .. }
    ));
}

#[test]
fn recognition_only_requires_a_valid_digest() {
    let mut rows = full_rows();
    rows[0].fields[0]
        .recognition
        .as_mut()
        .expect("recognition evidence")
        .evidence_digest = "not-a-digest".to_owned();
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::InvalidDigest { .. }
    ));
}

#[test]
fn recognition_evidence_is_bound_to_the_fields_own_atom() {
    let mut rows = full_rows();
    rows[0].fields[0]
        .recognition
        .as_mut()
        .expect("recognition evidence")
        .atom = RecoveryFieldAtom::Affiliation;
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent { .. }
    ));
}

#[test]
fn a_cross_recognition_control_that_references_its_own_atom_is_refused() {
    let mut rows = full_rows();
    rows[0].fields[0]
        .recognition
        .as_mut()
        .expect("recognition evidence")
        .co_firing_atoms = vec![RecoveryFieldAtom::Date];
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent { .. }
    ));
}

#[test]
fn a_same_line_set_control_outside_the_co_firing_set_is_refused() {
    let mut rows = full_rows();
    let recognition = rows[0].fields[0]
        .recognition
        .as_mut()
        .expect("recognition evidence");
    recognition.co_firing_atoms = vec![RecoveryFieldAtom::PersonalName];
    recognition.same_line_set_atoms = vec![RecoveryFieldAtom::Affiliation];
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent { .. }
    ));
}

#[test]
fn a_well_formed_cross_recognition_control_still_claims_no_recovery() {
    // The control documents that the atom's recognition is not field-specific.
    // Even when it fires, the field stays `recognition_only` and the aggregate
    // `recovered` column stays zero: the control is diagnostic, never a proof.
    let mut rows = full_rows();
    {
        let recognition = rows[0].fields[0]
            .recognition
            .as_mut()
            .expect("recognition evidence");
        recognition.co_firing_atoms = vec![
            RecoveryFieldAtom::PersonalName,
            RecoveryFieldAtom::Affiliation,
        ];
        recognition.same_line_set_atoms = vec![RecoveryFieldAtom::PersonalName];
    }
    let receipt = build(&population(RecoveryMode::Full, 79), &rows).expect("receipt");
    assert_eq!(
        receipt.recoveries[0].fields[0].disposition,
        FieldDisposition::RecognitionOnly
    );
    assert_eq!(receipt.aggregates[0].recovered, 0);
}

#[test]
fn not_recovered_with_positive_recognition_is_refused() {
    let mut rows = full_rows();
    rows[0].fields[0].disposition = FieldDisposition::NotRecovered;
    rows[0].fields[0].cause = Some(FieldNotRecoveredCause::BandRefused);
    // the recognition evidence is left in place (count > 0)
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent { .. }
    ));
}

#[test]
fn not_recovered_requires_a_typed_cause() {
    let mut rows = full_rows();
    rows[0].fields[0].cause = None;
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldCauseInconsistent { .. }
    ));
}

#[test]
fn not_recovered_must_not_carry_the_attribution_cause() {
    let mut rows = full_rows();
    // `not_recovered` with the recognition-only cause would be a contradiction:
    // the attribution-absent cause belongs to `recognition_only`.
    rows[0].fields[0].disposition = FieldDisposition::NotRecovered;
    rows[0].fields[0].cause = Some(FieldNotRecoveredCause::RecognitionWithoutFieldAttribution);
    rows[0].fields[0].recognition = None;
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldCauseInconsistent { .. }
    ));
}

#[test]
fn out_of_scope_requires_the_no_native_atom_cause() {
    let mut rows = full_rows();
    rows[72].fields[1].cause = Some(FieldNotRecoveredCause::BandRefused);
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldCauseInconsistent { .. }
    ));
}

#[test]
fn out_of_scope_must_not_carry_recognition() {
    let mut rows = full_rows();
    rows[72].fields[1].recognition = Some(FieldRecognition {
        atom: RecoveryFieldAtom::PersonalName,
        recognized_count: 1,
        evidence_digest: sha256_prefixed(b"title"),
        co_firing_atoms: Vec::new(),
        same_line_set_atoms: Vec::new(),
    });
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent { .. }
    ));
}

#[test]
fn a_typed_refusal_row_must_not_carry_recognition() {
    let mut rows = full_rows();
    rows[0].disposition = RecoveryDisposition::TypedRefusal;
    rows[0].band_refusal = Some(RefusalReason::MissingFontSize);
    // fields[0] is still `recognition_only`
    let error = build(&population(RecoveryMode::Full, 79), &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionFieldRecognitionInconsistent { .. }
    ));
}

// ─────────────────────────── population ───────────────────────────

#[test]
fn smoke_run_is_not_assumed_complete() {
    let rows = vec![
        date_row("date-rec-0", true),
        title_row("title-0"),
        author_row("author-0"),
    ];
    let receipt = build(&population(RecoveryMode::Smoke, 3), &rows).expect("smoke receipt");
    assert!(!receipt.population.assumed_complete);
    assert_eq!(receipt.population.mode, RecoveryMode::Smoke);
    assert_eq!(receipt.recoveries.len(), 3);
    assert_eq!(receipt.aggregates[0].denominator, 1);
    assert_eq!(receipt.aggregates[0].not_recovered, 1);
    assert_eq!(receipt.aggregates[1].out_of_scope, 1);
    assert_eq!(receipt.aggregates[2].not_recovered, 1);
}

#[test]
fn smoke_receipt_that_claims_completeness_is_refused() {
    let rows = vec![date_row("date-rec-0", true)];
    let mut population = population(RecoveryMode::Smoke, 1);
    population.assumed_complete = true;
    let error = build(&population, &rows).unwrap_err();
    assert!(matches!(
        error,
        NativeFamilyReceiptError::ExclusionPopulationCoverage { .. }
    ));
}

#[test]
fn a_duplicate_document_is_refused() {
    let rows = vec![
        date_row("date-rec-0", true),
        date_row("date-rec-1", true),
        title_row("title-0"),
        date_row("date-rec-0", true),
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
    rows[0] = refusal_reason_date_row("date-rec-0");
    let receipt = build(&population(RecoveryMode::Full, 79), &rows).expect("refusal row");
    let date = &receipt.aggregates[0];
    assert_eq!(date.recovered, 0);
    assert_eq!(date.not_recovered, 71);
    assert_eq!(date.typed_refusal, 1);
}

#[test]
fn a_typed_refusal_must_carry_a_refusal_reason() {
    let mut rows = full_rows();
    rows[0] = row_with(
        "date-rec-0",
        ExclusionReason::HeaderDateAbsent,
        refusal_fields(FieldNotRecoveredCause::BandRefused),
        RecoveryDisposition::TypedRefusal,
        None,
        None,
    );
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

// ─────────────────────────── boundary ───────────────────────────

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

    assert_eq!(
        RecoveryField::Date.native_atom(),
        Some(RecoveryFieldAtom::Date)
    );
    assert_eq!(
        RecoveryField::Author.native_atom(),
        Some(RecoveryFieldAtom::PersonalName)
    );
    assert_eq!(RecoveryField::Title.native_atom(), None);

    assert_eq!(
        FieldNotRecoveredCause::RecognitionWithoutFieldAttribution.as_str(),
        "recognition_without_field_attribution"
    );
    assert_eq!(
        FieldNotRecoveredCause::NoRecognition.as_str(),
        "no_recognition"
    );
    assert_eq!(
        FieldNotRecoveredCause::NoNativeAtom.as_str(),
        "no_native_atom"
    );
    assert_eq!(
        FieldDisposition::RecognitionOnly.as_str(),
        "recognition_only"
    );
    assert_eq!(RecoveryFieldAtom::PersonalName.as_str(), "personal_name");
}

#[test]
fn the_artifact_round_trips_and_is_redacted() {
    let rows = full_rows();
    let receipt = build(&population(RecoveryMode::Full, 79), &rows).expect("full receipt");
    let bytes = receipt.artifact_json_bytes().expect("serializable");
    let text = std::str::from_utf8(&bytes).expect("utf8");
    assert_redacted(text).expect("no payload marker leaked");
    assert!(!text.contains("<TEI"));
    assert!(text.contains("recognition_only"));
    let parsed = NativeExclusionRecoveryReceipt::parse_json_bytes(&bytes).expect("replay");
    assert_eq!(parsed, receipt);
}
