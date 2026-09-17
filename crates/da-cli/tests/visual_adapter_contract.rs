#![cfg(feature = "pdf-layout")]

//! GAR-003 discriminating contract for the donor visual sidecar converter.
//!
//! The converter itself lives on the library surface in `da_cli::visual_adapter`
//! (S02/T01). This target exists to make the *failure* paths observable rather
//! than only the happy path:
//!
//! * AC1 — real donor primitives reach the native detector when extraction
//!   succeeds (`visual_adapter_preserves_donor_primitives_end_to_end`);
//! * the pre-fix drop path (`primitives: Vec::new()`) is falsified by an
//!   independent property oracle (`preservation_violations`) instead of a copy
//!   of the converter (ADR-051: no same code in SUT and oracle);
//! * AC2 — malformed donor input maps to `MalformedBbox`, never `NotObserved`;
//! * AC3 — a positive fixture proves non-empty observations and falsifies
//!   terminal-only parity; the receipt families this donor physically cannot
//!   supply are declared rather than silently empty.
//!
//! Every test keeps the stable `visual_adapter_` filter token so a zero run
//! (`0 passed; 0 filtered out`) is detectable instead of silently green.

use da_adapters::{
    LayoutDonorError, LayoutDonorLimits, LayoutShadowObservation, VisualPrimitiveKind,
    VisualPrimitiveState, parse_pdfalto_xml_shadow,
};
use da_cli::visual_adapter::{
    NON_DELIVERABLE_VISUAL_FAMILIES, VisualIdentity, donor_error_absence, empty_observation,
    native_visual_payload,
};
use da_parse::{
    LayoutError, NativeVisualAbsence, NativeVisualConfig, NativeVisualHeadOutcome,
    NativeVisualKind, NativeVisualObservation, detect_figure_heads,
};

/// Two observed primitives of different kinds on one page. No XML namespace and
/// no service URL: the fixture must stay portable and free of hardcoded hosts.
const TWO_PRIMITIVE_DONOR_XML: &[u8] = br#"<doc><page width="612" height="792"><Figure x="10" y="20" width="30" height="40" content="figure-a"/><Table x="50" y="60" width="70" height="80" content="table-a"/></page></doc>"#;

/// Non-finite donor geometry: the donor must reject it rather than report an
/// empty-but-successful lane.
const MALFORMED_DONOR_XML: &[u8] = br#"<doc><page width="612" height="792"><Figure x="nan" y="20" width="30" height="40" content="bad"/></page></doc>"#;

const FIXTURE_PDF_BYTES: &[u8] = b"fixture-pdf";
const FIXTURE_DOCUMENT_ID: &str = "fixture-doc";

fn donor(xml: &[u8]) -> LayoutShadowObservation {
    parse_pdfalto_xml_shadow(
        xml,
        FIXTURE_PDF_BYTES,
        FIXTURE_DOCUMENT_ID,
        LayoutDonorLimits::default(),
    )
    .expect("donor fixture must parse")
}

fn identity() -> VisualIdentity {
    VisualIdentity {
        document_id: FIXTURE_DOCUMENT_ID.into(),
        source_sha256: "a".repeat(64),
        provider_id: "pdfalto".into(),
        provider_version: "unknown".into(),
    }
}

/// Test-local, explicit donor→native kind table. Deliberately NOT
/// `da_cli::visual_adapter::native_kind`: the oracle must not delegate to the
/// code under test (ADR-051).
fn expected_native_kind(kind: VisualPrimitiveKind) -> NativeVisualKind {
    match kind {
        VisualPrimitiveKind::Image => NativeVisualKind::Image,
        VisualPrimitiveKind::Graphic => NativeVisualKind::Graphic,
        VisualPrimitiveKind::Table => NativeVisualKind::Table,
        VisualPrimitiveKind::Formula => NativeVisualKind::Formula,
        VisualPrimitiveKind::Rect => NativeVisualKind::Rect,
    }
}

fn push_once(violations: &mut Vec<&'static str>, violation: &'static str) {
    if !violations.contains(&violation) {
        violations.push(violation);
    }
}

/// Independent preservation oracle: it re-derives the properties GAR-003
/// demands from the donor observation alone and never calls the converter.
fn preservation_violations(
    donor: &LayoutShadowObservation,
    native: &NativeVisualObservation,
) -> Vec<&'static str> {
    let mut violations = Vec::new();
    let donor_primitives = &donor.visual.primitives;

    if donor_primitives.len() != native.primitives.len() {
        push_once(&mut violations, "donor_primitives_discarded");
    }
    if donor.visual.absence.is_none() && native.primitives.is_empty() {
        push_once(&mut violations, "empty_observation_without_lane_absence");
    }
    if native.document_id.is_empty()
        || native.source_sha256.is_empty()
        || native.provider.id.is_empty()
        || native.provider.version.is_empty()
    {
        push_once(&mut violations, "primitive_provenance_lost");
    }

    for primitive in donor_primitives {
        let Some(matched) = native.primitives.iter().find(|candidate| {
            candidate.page_ordinal == primitive.page_ordinal
                && candidate.source_ordinal == primitive.source_ordinal
        }) else {
            push_once(&mut violations, "donor_primitives_discarded");
            continue;
        };
        if matched.kind != expected_native_kind(primitive.kind)
            || matched.document_id.is_empty()
            || matched.source_sha256.is_empty()
            || matched.provider.id.is_empty()
            || matched.provider.version.is_empty()
        {
            push_once(&mut violations, "primitive_provenance_lost");
        }
        match (&primitive.state, &matched.bbox, &matched.content_digest) {
            (
                VisualPrimitiveState::Observed { content_digest },
                Some(native_bbox),
                Some(native_digest),
            ) => {
                if Some(*native_bbox) != primitive.bbox || native_digest != content_digest {
                    push_once(&mut violations, "primitive_provenance_lost");
                }
            }
            (VisualPrimitiveState::Absent { .. }, _, _) => {
                if matched.absence.is_none() || matched.content_digest.is_some() {
                    push_once(&mut violations, "primitive_provenance_lost");
                }
            }
            _ => push_once(&mut violations, "primitive_provenance_lost"),
        }
    }
    violations
}

/// Read the native observation's named family. Panics on an undeclared name so
/// a renamed/added field cannot silently pass the closure test.
fn family_observation_len(native: &NativeVisualObservation, family: &str) -> usize {
    match family {
        "figure_captions" => native.figure_captions.len(),
        "figure_callouts" => native.figure_callouts.len(),
        "table_captions" => native.table_captions.len(),
        "table_callouts" => native.table_callouts.len(),
        "table_cells" => native.table_cells.len(),
        "formula_labels" => native.formula_labels.len(),
        "formula_callouts" => native.formula_callouts.len(),
        other => panic!("undeclared receipt family {other}"),
    }
}

fn collect_keys(value: &serde_json::Value, keys: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, nested) in map {
                keys.push(key.clone());
                collect_keys(nested, keys);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_keys(item, keys);
            }
        }
        _ => {}
    }
}

#[test]
fn visual_adapter_preserves_donor_primitives_end_to_end() {
    let donor = donor(TWO_PRIMITIVE_DONOR_XML);
    assert_eq!(
        donor.visual.primitives.len(),
        2,
        "fixture must carry two donor primitives"
    );
    assert!(donor.visual.absence.is_none());

    let native = native_visual_payload(&donor).expect("donor payload must convert");
    assert_eq!(
        native.primitives.len(),
        2,
        "both donor primitives must reach the native detector"
    );
    assert!(native.lane_absence.is_none());

    let violations = preservation_violations(&donor, &native);
    assert!(
        violations.is_empty(),
        "donor primitives were not preserved: {violations:?}"
    );

    let outcome = detect_figure_heads(&native, NativeVisualConfig::default())
        .expect("converted payload must be detector-valid");
    assert!(
        matches!(outcome, NativeVisualHeadOutcome::Figure { .. }),
        "preserved figure primitive must produce a figure head, got {outcome:?}"
    );
}

#[test]
fn visual_adapter_drop_path_is_detected() {
    let donor = donor(TWO_PRIMITIVE_DONOR_XML);
    let preserved = native_visual_payload(&donor).expect("donor payload must convert");
    assert!(
        preservation_violations(&donor, &preserved).is_empty(),
        "control: the preserved shape must be violation-free"
    );

    // Pre-fix shape (`git show 209fe0bcc^:crates/da-cli/examples/native_visual_heads_shadow.rs`):
    // the Ok branch published `primitives: Vec::new()` with no lane absence.
    let mut discarded = preserved;
    discarded.primitives = Vec::new();
    discarded.lane_absence = None;

    let violations = preservation_violations(&donor, &discarded);
    assert!(violations.contains(&"donor_primitives_discarded"));
    assert!(violations.contains(&"empty_observation_without_lane_absence"));

    let outcome = detect_figure_heads(&discarded, NativeVisualConfig::default())
        .expect("pre-fix shape is schema-valid but semantically empty");
    assert!(
        matches!(
            outcome,
            NativeVisualHeadOutcome::Terminal {
                reason: NativeVisualAbsence::NotObserved,
                ..
            }
        ),
        "discarded primitives must read as terminal not_observed, got {outcome:?}"
    );
}

#[test]
fn visual_adapter_malformed_maps_to_malformed_bbox_not_not_observed() {
    let error = parse_pdfalto_xml_shadow(
        MALFORMED_DONOR_XML,
        FIXTURE_PDF_BYTES,
        FIXTURE_DOCUMENT_ID,
        LayoutDonorLimits::default(),
    )
    .expect_err("NaN donor geometry must be rejected");
    assert!(matches!(
        error,
        LayoutDonorError::Malformed { .. } | LayoutDonorError::InvalidLayout(_)
    ));

    let absence = donor_error_absence(&error);
    assert_eq!(absence, NativeVisualAbsence::MalformedBbox);
    assert_ne!(absence, NativeVisualAbsence::NotObserved);

    let empty = empty_observation(&identity(), absence);
    assert_eq!(empty.lane_absence, Some(NativeVisualAbsence::MalformedBbox));
    assert!(empty.primitives.is_empty());

    let outcome = detect_figure_heads(&empty, NativeVisualConfig::default())
        .expect("terminal observation is detector-valid");
    assert!(
        matches!(
            outcome,
            NativeVisualHeadOutcome::Terminal {
                reason: NativeVisualAbsence::MalformedBbox,
                ..
            }
        ),
        "malformed donor input must stay malformed, got {outcome:?}"
    );
}

#[test]
fn visual_adapter_donor_error_mapping_table_is_closed() {
    let table = [
        (
            LayoutDonorError::Malformed {
                detail: "geometry".into(),
            },
            NativeVisualAbsence::MalformedBbox,
        ),
        (
            LayoutDonorError::InvalidLayout(LayoutError::NonFiniteGeometry),
            NativeVisualAbsence::MalformedBbox,
        ),
        (
            LayoutDonorError::Oversize {
                resource: "pages",
                limit: 512,
            },
            NativeVisualAbsence::Oversize,
        ),
        (
            LayoutDonorError::Unavailable {
                executable: "pdfalto".into(),
            },
            NativeVisualAbsence::Unsupported,
        ),
        (
            LayoutDonorError::Io(std::io::Error::other("fixture io failure")),
            NativeVisualAbsence::Unsupported,
        ),
        (
            LayoutDonorError::Timeout { timeout_ms: 30_000 },
            NativeVisualAbsence::Interrupted,
        ),
        (
            LayoutDonorError::NonZero { status: Some(1) },
            NativeVisualAbsence::Interrupted,
        ),
        (
            LayoutDonorError::Encrypted,
            NativeVisualAbsence::Interrupted,
        ),
        (
            LayoutDonorError::Truncated,
            NativeVisualAbsence::Interrupted,
        ),
    ];

    for (error, expected) in table {
        let actual = donor_error_absence(&error);
        assert_eq!(actual, expected, "unexpected mapping for {error}");
        assert_ne!(
            actual,
            NativeVisualAbsence::NotObserved,
            "terminal donor error must never read as NotObserved: {error}"
        );
    }
}

#[test]
fn visual_adapter_non_deliverable_families_are_exactly_declared() {
    let mut declared = NON_DELIVERABLE_VISUAL_FAMILIES.to_vec();
    declared.sort_unstable();
    assert_eq!(
        declared,
        vec![
            // Lexicographic order: "figure_callouts" precedes "figure_captions".
            "figure_callouts",
            "figure_captions",
            "formula_callouts",
            "formula_labels",
            "table_callouts",
            "table_captions",
            "table_cells",
        ]
    );

    let native =
        native_visual_payload(&donor(TWO_PRIMITIVE_DONOR_XML)).expect("donor payload must convert");
    assert!(
        !native.primitives.is_empty(),
        "families are declared on a non-empty observation"
    );
    assert!(native.lane_absence.is_none());
    for family in NON_DELIVERABLE_VISUAL_FAMILIES {
        assert_eq!(
            family_observation_len(&native, family),
            0,
            "declared non-deliverable family {family} must stay empty rather than be guessed"
        );
    }
}

#[test]
fn visual_adapter_donor_sidecar_cannot_supply_receipt_families() {
    let donor = donor(TWO_PRIMITIVE_DONOR_XML);
    let sidecar = serde_json::to_value(&donor.visual).expect("donor visual sidecar serializes");
    let mut keys = Vec::new();
    collect_keys(&sidecar, &mut keys);
    assert!(
        !keys.is_empty(),
        "serialized sidecar must expose at least one key"
    );
    for family in NON_DELIVERABLE_VISUAL_FAMILIES {
        assert!(
            !keys.iter().any(|key| key == family),
            "donor sidecar must not supply receipt family {family}: {keys:?}"
        );
    }
}
