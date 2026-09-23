//! H17 / S04-T02 — GREEN contract: a declared visual absence keeps its typed
//! reason and its full declared denominator.
//!
//! T01 (`S04-T01-SUMMARY.md`) measured the defect: a non-empty population whose
//! primitives all declare a typed absence (`NativeVisualPrimitive.absence =
//! Some(reason)`, `bbox = None`, `content_digest = None`) was flattened to
//! `Terminal { reason: NotObserved, metrics.terminal: 1 }`, losing both the
//! reason and the denominator. T02 fixes the detector half of GAR-003 and this
//! target replaces the T01 RED pins with the GREEN contract:
//!
//! * a declared absence is reported with its own typed reason, never
//!   `NotObserved` (`H17` / `H17-N1`);
//! * `metrics.terminal` equals the number of declared candidates — three
//!   declared absences on three pages report `3`, never a pooled `1`
//!   (`H17-N2`);
//! * a mixed admitted/declared-absent population stays a **partial head** with
//!   `metrics.terminal == items.len() - receipts.len()`; it is not a typed
//!   absence, and its per-element reasons remain unobservable (documented
//!   residual, tracked in T05 / ADR-076);
//! * an empty population is still a legitimate true absence
//!   (`Terminal { reason: NotObserved, metrics.terminal: 1 }`);
//! * a candidate that is neither observed nor declared absent stays fail-closed
//!   (`NativeVisualError::InvalidGeometry`), so a zero count cannot be produced
//!   without a type.
//!
//! The vocabulary is the closed [`da_parse::NativeVisualAbsence`] taxonomy and
//! the truth is asserted through the public API only. The DATA half of the
//! contract lives in `native_visual_data_contract`; the run with `--
//! --nocapture` prints one line per probe.

use da_parse::{
    LayoutBBox, NATIVE_VISUAL_SCHEMA_VERSION, NativeVisualAbsence, NativeVisualConfig,
    NativeVisualError, NativeVisualHeadOutcome, NativeVisualKind, NativeVisualMetrics,
    NativeVisualObservation, NativeVisualPrimitive, NativeVisualProvider, TYPED_ABSENCE_TAXONOMY,
    detect_figure_heads, detect_formula_heads, detect_table_heads,
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

fn head_metrics(outcome: &NativeVisualHeadOutcome) -> &NativeVisualMetrics {
    match outcome {
        NativeVisualHeadOutcome::Figure { metrics, .. }
        | NativeVisualHeadOutcome::Table { metrics, .. }
        | NativeVisualHeadOutcome::Formula { metrics, .. }
        | NativeVisualHeadOutcome::Terminal { metrics, .. } => metrics,
    }
}

fn assert_no_admitted_conservation(metrics: &NativeVisualMetrics) {
    assert_eq!(
        (
            metrics.admitted,
            metrics.caption_conservation,
            metrics.content_conservation,
            metrics.callout_conservation,
            metrics.geometry_conservation,
        ),
        (0, 0, 0, 0, 0),
        "a typed absence admits nothing and conserves nothing"
    );
}

fn report(
    probe: Probe,
    declared_reason: NativeVisualAbsence,
    declared_count: usize,
    observed_reason: Option<NativeVisualAbsence>,
    observed_terminal: u32,
) {
    println!(
        "[H17-T02 GREEN] head={} declared_reason={:?} declared_count={} observed_reason={} observed_terminal={}",
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

/// H17 core: one declared-absent candidate carries its own typed reason and the
/// declared denominator (one), and never collapses to `NotObserved`.
#[test]
fn declared_absence_keeps_its_typed_reason_and_declared_denominator() {
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
            (Some(NativeVisualAbsence::MalformedBbox), 1),
            "H17 ({}): a declared MalformedBbox must stay typed, not flatten to NotObserved",
            probe.label
        );
        assert_no_admitted_conservation(head_metrics(&outcome));

        // The typed reason and the denominator survive the serde roundtrip that
        // every shadow report performs.
        let replay: NativeVisualHeadOutcome =
            serde_json::from_slice(&serde_json::to_vec(&outcome).unwrap()).unwrap();
        assert_eq!(replay, outcome, "serde replay ({})", probe.label);
        assert!(
            serde_json::to_string(&outcome)
                .unwrap()
                .contains("malformed_bbox"),
            "the typed reason must be observable in the serialized terminal ({})",
            probe.label
        );
    }
}

/// H17-N2: the denominator is the number of declared candidates, never pooled to
/// one. Three declared absences on three pages report `3`.
#[test]
fn declared_denominator_counts_every_declared_element_and_is_not_pooled() {
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
        let declared_count = primitives.len();
        let input = observation(primitives);
        assert_legal_shape(&input, probe.label);
        let outcome = (probe.detector)(&input, NativeVisualConfig::default())
            .expect("detector admits three declared-absent primitives");
        let (observed_reason, observed_terminal) = observed_profile(&outcome);
        report(
            probe,
            NativeVisualAbsence::MalformedBbox,
            declared_count,
            observed_reason,
            observed_terminal,
        );
        assert_eq!(
            observed_terminal as usize, declared_count,
            "H17-N2 ({}): the declared denominator must count every declared element",
            probe.label
        );
        assert_ne!(
            observed_terminal, 1,
            "H17-N2 ({}): three declared elements on three pages must not be pooled into terminal=1",
            probe.label
        );
        assert_eq!(
            observed_reason,
            Some(NativeVisualAbsence::MalformedBbox),
            "H17-N2 ({}): the shared declared reason must be carried",
            probe.label
        );
        assert_no_admitted_conservation(head_metrics(&outcome));
    }
}

/// H17-N2 (determinism): a population with two different declared reasons keeps
/// the first reason in `(page_ordinal, source_ordinal)` order, independently of
/// the order the primitives were supplied in, and repeated calls agree.
#[test]
fn first_declared_reason_is_deterministic_and_input_order_independent() {
    for probe in probes() {
        let ordered = vec![
            declared_absent_primitive(
                probe.primitive_kind,
                1,
                0,
                NativeVisualAbsence::MalformedBbox,
            ),
            declared_absent_primitive(
                probe.primitive_kind,
                2,
                0,
                NativeVisualAbsence::ChecksumMismatch,
            ),
        ];
        // Same population, supplied in the reverse order: the detector must
        // re-order by (page_ordinal, source_ordinal) and pick the same reason.
        let mut shuffled = ordered.clone();
        shuffled.reverse();

        let first = (probe.detector)(&observation(ordered.clone()), NativeVisualConfig::default())
            .expect("detector admits a mixed-reason declared population");
        let second = (probe.detector)(&observation(shuffled), NativeVisualConfig::default())
            .expect("detector admits the reversed declared population");
        let repeat = (probe.detector)(&observation(ordered.clone()), NativeVisualConfig::default())
            .expect("detector is callable more than once");

        let (reason, terminal) = observed_profile(&first);
        report(
            probe,
            NativeVisualAbsence::MalformedBbox,
            ordered.len(),
            reason,
            terminal,
        );
        assert_eq!(
            (reason, terminal),
            (Some(NativeVisualAbsence::MalformedBbox), 2),
            "H17-N2 ({}): the first declared reason in page order must win with the full denominator",
            probe.label
        );
        assert_eq!(
            first, second,
            "H17-N2 ({}): input order must not change the typed outcome",
            probe.label
        );
        assert_eq!(
            first, repeat,
            "H17-N2 ({}): repeated calls must be byte-stable",
            probe.label
        );
    }
}

/// Documented residual (T05 / ADR-076): a mixed admitted/declared-absent
/// population is a **partial head**, not a typed absence. Its unadmitted element
/// stays in the denominator only, and its per-element reason is not observable
/// here. This pin exists so a future fix that starts carrying per-element
/// reasons must update this contract deliberately.
#[test]
fn mixed_valid_and_absent_stays_a_partial_head_not_a_typed_absence() {
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
        "[H17-T02 GREEN] head=figure declared_reason={:?} declared_count=1 observed_reason={} observed_terminal={} (partial head residual)",
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
        "the absent element counts only in the terminal denominator of the partial head"
    );
    assert_eq!(
        observed_reason, None,
        "a partial head outcome carries no typed reason: the residual is per-element reasons"
    );
    let serialized = serde_json::to_string(&outcome).expect("outcome serializes");
    assert!(
        !serialized.contains("malformed_bbox"),
        "residual pin: the declared MalformedBbox reason is not observable inside a partial head outcome"
    );
}

/// H17-N1: a candidate that is neither observed nor declared absent is not a
/// legal shape. It stays fail-closed with `InvalidGeometry` and can never be
/// reported as a zero count or as `NotObserved`.
#[test]
fn untyped_empty_shape_fails_closed_and_never_becomes_not_observed() {
    for probe in probes() {
        let mut untyped = observation(vec![observed_primitive(probe.primitive_kind, 1, 0)]);
        untyped.primitives[0].bbox = None;
        untyped.primitives[0].content_digest = None;
        assert!(
            untyped.primitives[0].absence.is_none(),
            "the untyped fixture must carry no declared absence"
        );

        assert_eq!(
            untyped.validate(NativeVisualConfig::default()),
            Err(NativeVisualError::InvalidGeometry),
            "H17-N1 ({}): bbox/digest/absence all empty is not a legal primitive form",
            probe.label
        );
        assert_eq!(
            (probe.detector)(&untyped, NativeVisualConfig::default()),
            Err(NativeVisualError::InvalidGeometry),
            "H17-N1 ({}): the detector must reject the untyped shape, never report NotObserved for it",
            probe.label
        );
    }
}

/// Boundary pin: an empty population is a legitimate *true* absence, while a
/// non-empty declared population is a *typed* terminal. The two are not
/// interchangeable — the reason differs and so does the denominator.
#[test]
fn empty_population_stays_a_true_absence_while_declared_population_does_not() {
    for probe in probes() {
        let empty = observation(Vec::new());
        assert_legal_shape(&empty, probe.label);
        let empty_outcome = (probe.detector)(&empty, NativeVisualConfig::default())
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
            "oracle ({}): an empty population is a legitimate true absence and must stay NotObserved",
            probe.label
        );

        let declared = observation(vec![
            declared_absent_primitive(
                probe.primitive_kind,
                1,
                0,
                NativeVisualAbsence::MalformedBbox,
            ),
            declared_absent_primitive(
                probe.primitive_kind,
                2,
                0,
                NativeVisualAbsence::MalformedBbox,
            ),
        ]);
        assert_legal_shape(&declared, probe.label);
        let declared_outcome = (probe.detector)(&declared, NativeVisualConfig::default())
            .expect("detector admits the declared population");
        let (declared_reason, declared_terminal) = observed_profile(&declared_outcome);
        assert_eq!(
            (declared_reason, declared_terminal),
            (Some(NativeVisualAbsence::MalformedBbox), 2),
            "boundary ({}): a declared population must not be reported as a true absence",
            probe.label
        );
        assert_ne!(
            empty_reason, declared_reason,
            "boundary ({}): true absence and declared absence must be distinguishable by reason",
            probe.label
        );
    }
}

/// `lane_absence` is a lane-level typed absence and is intentionally unchanged:
/// it is already typed through `terminal_if_absent` and reports the lane-level
/// denominator `1` (there is no per-candidate population to count).
#[test]
fn lane_absence_stays_typed_and_unchanged() {
    for probe in probes() {
        for reason in [
            NativeVisualAbsence::Unsupported,
            NativeVisualAbsence::MalformedBbox,
        ] {
            let mut input = observation(Vec::new());
            input.lane_absence = Some(reason);
            assert_legal_shape(&input, probe.label);
            let outcome = (probe.detector)(&input, NativeVisualConfig::default())
                .expect("detector admits a lane-level absence");
            let (observed_reason, observed_terminal) = observed_profile(&outcome);
            report(probe, reason, 0, observed_reason, observed_terminal);
            assert_eq!(
                (observed_reason, observed_terminal),
                (Some(reason), 1),
                "lane absence ({}/{reason:?}) is already typed and must stay unchanged",
                probe.label
            );
        }
    }
}

/// The taxonomy is a closed value, and its `token()` must agree with the serde
/// wire form the shadow reports use. The DATA half is checked in
/// `native_visual_data_contract`.
#[test]
fn typed_absence_taxonomy_is_closed_and_matches_its_serde_tokens() {
    assert_eq!(
        TYPED_ABSENCE_TAXONOMY.len(),
        10,
        "the absence vocabulary must carry exactly ten variants"
    );
    let tokens: Vec<&str> = TYPED_ABSENCE_TAXONOMY
        .iter()
        .map(|reason| reason.token())
        .collect();
    let unique: std::collections::BTreeSet<&str> = tokens.iter().copied().collect();
    assert_eq!(unique.len(), tokens.len(), "tokens must be unique");
    for reason in TYPED_ABSENCE_TAXONOMY {
        assert_eq!(
            serde_json::to_string(reason).expect("taxonomy serializes"),
            format!("\"{}\"", reason.token()),
            "token() must match the serde snake_case form for {reason:?}"
        );
    }
}
