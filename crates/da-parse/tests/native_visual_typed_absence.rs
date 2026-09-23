//! H17 / S04-T01 — RED measurement: a declared, typed visual absence is
//! flattened to a bare `NotObserved`.
//!
//! This target is a MEASUREMENT, not a fix. It pins the profile observed on
//! 2026-09-23 for `detect_figure_heads` / `detect_table_heads` /
//! `detect_formula_heads`: a non-empty population whose primitives declare a
//! typed absence (`NativeVisualPrimitive.absence = Some(reason)`, `bbox = None`,
//! `content_digest = None`) is dropped by the detector and, when nothing else is
//! admitted, reported as `Terminal { reason: NotObserved, metrics.terminal: 1 }`.
//! The typed reason is lost and the denominator collapses to 1.
//!
//! The adapter half of GAR-003 (`crates/da-cli/src/visual_adapter.rs`) already
//! preserves the typed reason into `NativeVisualPrimitive.absence`; the detector
//! half does not.
//!
//! Every probe asserts the measured profile EXACTLY, so this target fails in
//! both directions: it fails if the profile regresses further, and it fails once
//! T02 preserves the typed reason and the declared denominator — at which point
//! T02 replaces these pins with the GREEN contract. Run with `-- --nocapture` to
//! see the per-probe report.
//!
//! Vocabulary is the closed `NativeVisualAbsence` dictionary, so T02 can reuse
//! the same names. No `crates/da-parse/src` file is touched: this target uses
//! only the crate's public API.

use da_parse::{
    LayoutBBox, NATIVE_VISUAL_SCHEMA_VERSION, NativeVisualAbsence, NativeVisualConfig,
    NativeVisualError, NativeVisualHeadOutcome, NativeVisualKind, NativeVisualObservation,
    NativeVisualPrimitive, NativeVisualProvider, detect_figure_heads, detect_formula_heads,
    detect_table_heads,
};

const DOCUMENT_ID: &str = "h17-document";
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Signature shared by the three independently callable heads.
type Detector = fn(
    &NativeVisualObservation,
    NativeVisualConfig,
) -> Result<NativeVisualHeadOutcome, NativeVisualError>;

#[derive(Clone, Copy)]
struct Probe {
    label: &'static str,
    detector: Detector,
    primitive_kind: NativeVisualKind,
}

fn probes() -> [Probe; 3] {
    [
        Probe {
            label: "figure",
            detector: detect_figure_heads,
            primitive_kind: NativeVisualKind::Image,
        },
        Probe {
            label: "table",
            detector: detect_table_heads,
            primitive_kind: NativeVisualKind::Table,
        },
        Probe {
            label: "formula",
            detector: detect_formula_heads,
            primitive_kind: NativeVisualKind::Formula,
        },
    ]
}

fn provider() -> NativeVisualProvider {
    NativeVisualProvider {
        id: "h17-provider".to_owned(),
        version: "1.0".to_owned(),
    }
}

/// An admitted primitive: geometry plus content digest, no declared absence.
fn observed_primitive(kind: NativeVisualKind, page: u32, source: u32) -> NativeVisualPrimitive {
    NativeVisualPrimitive {
        document_id: DOCUMENT_ID.to_owned(),
        source_sha256: SHA.to_owned(),
        provider: provider(),
        page_ordinal: page,
        source_ordinal: source,
        kind,
        bbox: Some(LayoutBBox::new(20.0, 30.0, 120.0, 130.0).expect("valid bbox")),
        content_digest: Some(SHA.to_owned()),
        absence: None,
    }
}

/// The shape produced by `visual_adapter::native_visual_payload` for a donor
/// primitive whose state is `Absent { reason }`: no geometry, no digest, and the
/// reason carried in the typed `absence` field.
fn declared_absent_primitive(
    kind: NativeVisualKind,
    page: u32,
    source: u32,
    reason: NativeVisualAbsence,
) -> NativeVisualPrimitive {
    NativeVisualPrimitive {
        document_id: DOCUMENT_ID.to_owned(),
        source_sha256: SHA.to_owned(),
        provider: provider(),
        page_ordinal: page,
        source_ordinal: source,
        kind,
        bbox: None,
        content_digest: None,
        absence: Some(reason),
    }
}

fn observation(primitives: Vec<NativeVisualPrimitive>) -> NativeVisualObservation {
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.to_owned(),
        document_id: DOCUMENT_ID.to_owned(),
        source_sha256: SHA.to_owned(),
        provider: provider(),
        primitives,
        figure_captions: Vec::new(),
        figure_callouts: Vec::new(),
        table_captions: Vec::new(),
        table_callouts: Vec::new(),
        table_cells: Vec::new(),
        formula_labels: Vec::new(),
        formula_callouts: Vec::new(),
        lane_absence: None,
    }
}

/// Measured profile of a head outcome: carried typed reason (if any) and the
/// terminal denominator.
fn observed_profile(outcome: &NativeVisualHeadOutcome) -> (Option<NativeVisualAbsence>, u32) {
    match outcome {
        NativeVisualHeadOutcome::Terminal { reason, metrics } => (Some(*reason), metrics.terminal),
        NativeVisualHeadOutcome::Figure { metrics, .. }
        | NativeVisualHeadOutcome::Table { metrics, .. }
        | NativeVisualHeadOutcome::Formula { metrics, .. } => (None, metrics.terminal),
    }
}

fn report(
    probe: Probe,
    declared_reason: NativeVisualAbsence,
    declared_count: usize,
    observed_reason: Option<NativeVisualAbsence>,
    observed_terminal: u32,
) {
    println!(
        "[H17-T01 RED] head={} declared_reason={:?} declared_count={} observed_reason={} observed_terminal={}",
        probe.label,
        declared_reason,
        declared_count,
        match observed_reason {
            Some(reason) => format!("{reason:?}"),
            None => "none(not-carried)".to_owned(),
        },
        observed_terminal,
    );
}

/// FATAL on input-shape drift: the declared-absence form must stay a legal
/// primitive shape, otherwise these probes measure a rejected input rather than
/// detector behaviour.
fn assert_legal_shape(input: &NativeVisualObservation, label: &str) {
    if let Err(error) = input.validate(NativeVisualConfig::default()) {
        panic!(
            "FATAL input-shape drift: declared absence for {label} is no longer a legal primitive form: {error:?}"
        );
    }
}

/// Probe A (H17 core): one declared-absent primitive per kind is flattened to a
/// bare `NotObserved` with `terminal = 1`, even though the declared reason is
/// `MalformedBbox`.
#[test]
fn declared_malformed_absence_is_flattened_to_bare_not_observed() {
    for probe in probes() {
        let input = observation(vec![declared_absent_primitive(
            probe.primitive_kind,
            1,
            0,
            NativeVisualAbsence::MalformedBbox,
        )]);
        assert_legal_shape(&input, probe.label);
        let outcome = (probe.detector)(&input, NativeVisualConfig::default())
            .expect("detector admits the declared-absence population");
        let (observed_reason, observed_terminal) = observed_profile(&outcome);
        report(
            probe,
            NativeVisualAbsence::MalformedBbox,
            1,
            observed_reason,
            observed_terminal,
        );
        assert_eq!(
            (observed_reason, observed_terminal),
            (Some(NativeVisualAbsence::NotObserved), 1),
            "H17-T01 RED pin ({}): 1 declared MalformedBbox primitive measured as reason/terminal drifted",
            probe.label
        );
    }
}

/// Probe B (H17-N2 seed): three declared-absent primitives on three different
/// pages collapse into the same `terminal = 1` as a single declared absence —
/// the denominator is pooled to one instead of counting the declared elements.
#[test]
fn declared_absence_denominator_collapses_across_pages() {
    for probe in probes() {
        let primitives = (1..=3u32)
            .map(|page| {
                declared_absent_primitive(
                    probe.primitive_kind,
                    page,
                    0,
                    NativeVisualAbsence::MalformedBbox,
                )
            })
            .collect::<Vec<_>>();
        let input = observation(primitives);
        assert_legal_shape(&input, probe.label);
        let outcome = (probe.detector)(&input, NativeVisualConfig::default())
            .expect("detector admits three declared-absent primitives");
        let (observed_reason, observed_terminal) = observed_profile(&outcome);
        report(
            probe,
            NativeVisualAbsence::MalformedBbox,
            3,
            observed_reason,
            observed_terminal,
        );
        assert_eq!(
            (observed_reason, observed_terminal),
            (Some(NativeVisualAbsence::NotObserved), 1),
            "H17-T01 RED pin ({}): 3 declared absences on distinct pages must measure terminal=1 (pooled) and reason=NotObserved today; drift detected",
            probe.label
        );
    }
}

/// Probe C (reason loss in a partial head): a valid primitive plus one
/// declared-absent primitive yields a head outcome that carries receipts, so the
/// typed reason of the absent element is nowhere observable.
#[test]
fn mixed_valid_and_absent_hides_typed_reason_behind_receipts() {
    let input = observation(vec![
        declared_absent_primitive(
            NativeVisualKind::Image,
            1,
            0,
            NativeVisualAbsence::MalformedBbox,
        ),
        observed_primitive(NativeVisualKind::Image, 2, 0),
    ]);
    assert_legal_shape(&input, "figure");
    let outcome = detect_figure_heads(&input, NativeVisualConfig::default())
        .expect("detector admits a mixed valid plus declared-absent population");
    let (observed_reason, observed_terminal) = observed_profile(&outcome);
    println!(
        "[H17-T01 RED] head=figure declared_reason={:?} declared_count=1 observed_reason={} observed_terminal={}",
        NativeVisualAbsence::MalformedBbox,
        match observed_reason {
            Some(reason) => format!("{reason:?}"),
            None => "none(not-carried)".to_owned(),
        },
        observed_terminal,
    );

    let NativeVisualHeadOutcome::Figure { receipts, metrics } = &outcome else {
        panic!("mixed population must still produce a Figure head outcome");
    };
    assert_eq!(
        (receipts.len(), metrics.admitted, metrics.terminal),
        (1, 1, 1),
        "H17-T01 RED pin: the absent element must count only in the terminal denominator today"
    );
    assert_eq!(
        observed_reason, None,
        "H17-T01 RED pin: a partial head outcome must carry no typed reason today"
    );
    let serialized = serde_json::to_string(&outcome).expect("outcome serializes");
    assert!(
        !serialized.contains("malformed_bbox"),
        "H17-T01 RED pin: the declared MalformedBbox reason must not be observable in a partial head outcome today; if it is, the defect is already fixed"
    );
}

/// Probe D (non-vacuity oracle): the detector must distinguish an admitted
/// population from a genuinely empty one. An admitted primitive yields a head
/// outcome with receipts; an empty population still yields a legitimate
/// `NotObserved`. T02 must preserve both.
#[test]
fn detector_distinguishes_admitted_population_from_true_absence() {
    for probe in probes() {
        let valid_input = observation(vec![observed_primitive(probe.primitive_kind, 1, 0)]);
        assert_legal_shape(&valid_input, probe.label);
        let valid_outcome = (probe.detector)(&valid_input, NativeVisualConfig::default())
            .expect("detector admits a valid primitive");
        let (valid_reason, valid_terminal) = observed_profile(&valid_outcome);
        report(
            probe,
            NativeVisualAbsence::NotObserved,
            0,
            valid_reason,
            valid_terminal,
        );
        assert_eq!(
            (valid_reason, valid_terminal),
            (None, 0),
            "oracle ({0}): an admitted primitive must yield a head outcome with no terminal reason",
            probe.label
        );
        assert_eq!(
            head_metrics(&valid_outcome).admitted,
            1,
            "oracle ({0}): the admitted count must be 1",
            probe.label
        );

        let empty_input = observation(Vec::new());
        assert_legal_shape(&empty_input, probe.label);
        let empty_outcome = (probe.detector)(&empty_input, NativeVisualConfig::default())
            .expect("detector admits an empty population");
        let (empty_reason, empty_terminal) = observed_profile(&empty_outcome);
        report(
            probe,
            NativeVisualAbsence::NotObserved,
            0,
            empty_reason,
            empty_terminal,
        );
        assert_eq!(
            (empty_reason, empty_terminal),
            (Some(NativeVisualAbsence::NotObserved), 1),
            "oracle ({0}): an empty population is a legitimate true absence and must stay NotObserved",
            probe.label
        );
    }
}

/// Probe E (input-shape guard + fail-closed negative): the declared-absence form
/// is legal, while a primitive that is neither observed nor declared absent must
/// stay fail-closed and must never be reported as `NotObserved`.
#[test]
fn declared_absence_shape_is_legal_and_untyped_empty_shape_fails_closed() {
    for probe in probes() {
        let input = observation(vec![declared_absent_primitive(
            probe.primitive_kind,
            1,
            0,
            NativeVisualAbsence::MalformedBbox,
        )]);
        assert_legal_shape(&input, probe.label);
    }

    let mut untyped = observation(vec![observed_primitive(NativeVisualKind::Image, 1, 0)]);
    untyped.primitives[0].bbox = None;
    untyped.primitives[0].content_digest = None;
    assert!(untyped.primitives[0].absence.is_none());
    assert_eq!(
        untyped.validate(NativeVisualConfig::default()),
        Err(NativeVisualError::InvalidGeometry),
        "H17-N1 guard: a primitive with no bbox, no digest and no typed absence is not a legal form"
    );
    assert_eq!(
        detect_figure_heads(&untyped, NativeVisualConfig::default()),
        Err(NativeVisualError::InvalidGeometry),
        "H17-N1 guard: the detector must not turn an untyped malformed primitive into NotObserved"
    );
}

fn head_metrics(outcome: &NativeVisualHeadOutcome) -> &da_parse::NativeVisualMetrics {
    match outcome {
        NativeVisualHeadOutcome::Figure { metrics, .. }
        | NativeVisualHeadOutcome::Table { metrics, .. }
        | NativeVisualHeadOutcome::Formula { metrics, .. } => metrics,
        NativeVisualHeadOutcome::Terminal { metrics, .. } => metrics,
    }
}
