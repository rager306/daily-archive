//! (R131) Contract for the non-CRF body steps and the native body version ledger.
//!
//! This file owns the *non-CRF step* section of the S05 body band: the sentence
//! and language steps, their three typed outcomes (`NotRequested` /
//! `MissingResource` / `Unsupported`), the structural absence of a success
//! outcome, the mapping of the `segmentSentences` request switch onto the
//! sentence step, and the body version ledger with its adjacent-surface
//! classification.
//!
//! Expectations are derived from the wave's stated contract, not from the module
//! under test: GROBID 0.9.x has no sentence CRF and no language CRF in the body
//! pipeline (`SentenceUtilities.runSentenceDetection` runs in the TEI formatter,
//! `LanguageUtilities` is not a CRF family), so a step can never report produced
//! sentences/languages. The header/body dictionary split is checked against the
//! live dictionaries, so "abstract is a header label" is asserted rather than
//! restated in prose.
//!
//! Boundary statement: every symbol exercised here is a pure data carrier or a
//! pure function. No filesystem, environment, clock, model bytes, or production
//! route is touched, and no atom of this band is spliced into the
//! Java-authoritative parse path.

use da_parse::*;

fn sentence(requested: bool, resource: Option<&str>) -> NativeSentenceStep {
    NativeSentenceStep {
        requested,
        resource: resource.map(str::to_owned),
    }
}

fn language(requested: bool, resource: Option<&str>) -> NativeLanguageStep {
    NativeLanguageStep {
        requested,
        resource: resource.map(str::to_owned),
    }
}

fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(&back, value, "roundtrip mismatch");
}

/// All three outcomes are reachable, for both steps: the gap is typed, not a
/// single blanket refusal.
#[test]
fn every_outcome_is_reachable_for_both_steps() {
    let sentence_observations = [
        sentence(false, None).run(),
        sentence(true, None).run(),
        sentence(true, Some("lexicon/sentence")).run(),
    ];
    let language_observations = [
        language(false, None).run(),
        language(true, None).run(),
        language(true, Some("models/lang")).run(),
    ];
    for observations in [&sentence_observations, &language_observations] {
        assert!(matches!(
            observations[0],
            NativeBodyStepOutcome::NotRequested
        ));
        assert!(matches!(
            observations[1],
            NativeBodyStepOutcome::MissingResource { .. }
        ));
        assert!(matches!(
            observations[2],
            NativeBodyStepOutcome::Unsupported { .. }
        ));
    }
    // The step identity is carried by the receipt, not confused between steps.
    let receipts = [
        sentence(true, None).receipt(),
        language(true, None).receipt(),
    ];
    assert_eq!(receipts[0].step, NativeBodyStep::Sentence);
    assert_eq!(receipts[1].step, NativeBodyStep::Language);
    assert_ne!(receipts[0].outcome, receipts[1].outcome);
}

/// The missing-resource outcome names the resource that must be configured —
/// never the blank id a caller happened to supply.
#[test]
fn a_missing_resource_is_reported_by_required_name() {
    for blank in [None, Some(""), Some("   "), Some("\t\n")] {
        assert_eq!(
            sentence(true, blank).run(),
            NativeBodyStepOutcome::MissingResource {
                resource: NATIVE_SENTENCE_RESOURCE_NAME.into()
            }
        );
        assert_eq!(
            language(true, blank).run(),
            NativeBodyStepOutcome::MissingResource {
                resource: NATIVE_LANGUAGE_RESOURCE_NAME.into()
            }
        );
    }
    assert_eq!(NATIVE_SENTENCE_RESOURCE_NAME, "sentence-detection");
    assert_eq!(NATIVE_LANGUAGE_RESOURCE_NAME, "language-identification");
}

/// A configured resource is refused with both the id and the named gap, so a
/// caller can see *what* was refused and *why* it cannot be run natively.
#[test]
fn a_configured_resource_is_refused_with_the_named_gap() {
    let refused = sentence(true, Some("lexicon/sentence-detection")).run();
    match refused {
        NativeBodyStepOutcome::Unsupported { resource, reason } => {
            assert_eq!(resource, "lexicon/sentence-detection");
            assert_eq!(reason, NATIVE_SENTENCE_GAP_REASON);
            assert!(reason.contains("TEI formatter"));
        }
        other => panic!("a configured resource must be refused, got {other:?}"),
    }
    match language(true, Some("models/language")).run() {
        NativeBodyStepOutcome::Unsupported { resource, reason } => {
            assert_eq!(resource, "models/language");
            assert_eq!(reason, NATIVE_LANGUAGE_GAP_REASON);
            assert!(reason.contains("not a CRF family"));
        }
        other => panic!("a configured resource must be refused, got {other:?}"),
    }
    // Unrequested wins over a configured resource: nothing is evaluated.
    assert_eq!(
        sentence(false, Some("lexicon/sentence")).run(),
        NativeBodyStepOutcome::NotRequested
    );
}

/// No input combination claims produced sentences/languages — the honest
/// structural consequence of this band having no detector.
#[test]
fn no_input_combination_claims_output() {
    let resources = [
        None,
        Some(""),
        Some("  "),
        Some("id"),
        Some("日本語/モデル"),
    ];
    for requested in [false, true] {
        for resource in resources {
            let sentence_outcome = sentence(requested, resource).run();
            let language_outcome = language(requested, resource).run();
            assert!(
                !sentence_outcome.is_productive(),
                "sentence {requested}/{resource:?} claimed output: {sentence_outcome:?}"
            );
            assert!(
                !language_outcome.is_productive(),
                "language {requested}/{resource:?} claimed output: {language_outcome:?}"
            );
            assert!(!sentence(requested, resource).receipt().is_productive());
            assert!(!language(requested, resource).receipt().is_productive());
        }
    }
}

/// Hostile ids (multibyte, very long) are data: no panic, no truncation into a
/// different id, and no successful outcome.
#[test]
fn hostile_resource_ids_do_not_panic() {
    let long = "x".repeat(64 * 1024);
    for id in ["日本語", "🙂", &long] {
        match sentence(true, Some(id)).run() {
            NativeBodyStepOutcome::Unsupported { resource, .. } => assert_eq!(resource, id),
            other => panic!("expected a typed refusal, got {other:?}"),
        }
    }
    // An empty step (the default shape) is still total.
    assert_eq!(
        sentence(false, None).run(),
        NativeBodyStepOutcome::NotRequested
    );
}

/// The `segmentSentences` switch of the V1 fulltext request dialect drives the
/// sentence step directly; the language step has no wire switch and stays
/// caller intent.
#[test]
fn the_dialect_switch_drives_the_sentence_step() {
    let default_dialect = FulltextDialectV1::legacy_rich();
    assert!(!default_dialect.segment_sentences());
    assert_eq!(
        sentence(default_dialect.segment_sentences(), None).run(),
        NativeBodyStepOutcome::NotRequested
    );
    let requesting = default_dialect.with_segment_sentences(true);
    assert!(requesting.segment_sentences());
    assert!(matches!(
        sentence(requesting.segment_sentences(), None).run(),
        NativeBodyStepOutcome::MissingResource { .. }
    ));
}

/// `abstract` is an S03 header label: it is present in the header dictionary and
/// absent from both body dictionaries, which is why the body version ledger does
/// not claim it.
#[test]
fn abstract_is_a_header_label_and_not_a_body_label() {
    assert!(NATIVE_HEADER_LABELS.contains(&"<abstract>"));
    assert!(NATIVE_HEADER_LABELS.contains(&"I-<abstract>"));
    for label in NATIVE_FULLTEXT_BASE_LABELS {
        assert!(
            !label.contains("abstract"),
            "the body base dictionary must not carry an abstract label, found {label}"
        );
    }
    for label in NATIVE_FULLTEXT_LABELS {
        assert!(
            !label.contains("abstract"),
            "the body label dictionary must not carry an abstract label, found {label}"
        );
    }
    assert!(NATIVE_BODY_ADJACENT_SURFACES.contains(&("abstract", "header-label")));
    assert!(NATIVE_BODY_ADJACENT_SURFACES.contains(&("pdf_version_vs_tei_body", "parse-envelope")));
    assert_eq!(NATIVE_BODY_ADJACENT_SURFACES.len(), 2);
}

/// The body version ledger publishes exactly the band's own surfaces, bound to
/// the live constants, and never the adjacent concerns.
#[test]
fn the_body_version_ledger_pins_the_live_schema_versions() {
    assert_eq!(
        NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION,
        "da-parse.native-fulltext-sentence/v1"
    );
    let surface = native_body_version_surface();
    let names: Vec<&str> = surface.iter().map(|entry| entry.surface.as_str()).collect();
    assert_eq!(
        names,
        ["features", "sequence", "reading_order", "sentence_language"]
    );
    assert_eq!(
        surface[0].schema_version,
        NATIVE_FULLTEXT_FEATURE_SCHEMA_VERSION
    );
    assert_eq!(
        surface[1].schema_version,
        NATIVE_FULLTEXT_SEQUENCE_SCHEMA_VERSION
    );
    assert_eq!(
        surface[2].schema_version,
        NATIVE_FULLTEXT_READING_ORDER_SCHEMA_VERSION
    );
    assert_eq!(
        surface[3].schema_version,
        NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION
    );
    assert_eq!(surface[0].kind, NativeBodySurfaceKind::Features);
    assert_eq!(surface[1].kind, NativeBodySurfaceKind::Sequence);
    assert_eq!(surface[2].kind, NativeBodySurfaceKind::ReadingOrder);
    assert_eq!(surface[3].kind, NativeBodySurfaceKind::NonCrfStep);
    // The ledger is stable and carries no adjacent concern.
    assert_eq!(native_body_version_surface(), surface);
    assert_eq!(surface.len(), 4);
    let mut versions: Vec<&str> = surface
        .iter()
        .map(|entry| entry.schema_version.as_str())
        .collect();
    versions.sort_unstable();
    versions.dedup();
    assert_eq!(
        versions.len(),
        surface.len(),
        "schema versions must be unique"
    );
    assert!(surface.iter().all(|entry| {
        entry
            .schema_version
            .starts_with("da-parse.native-fulltext-")
    }));
    assert!(
        surface
            .iter()
            .all(|entry| entry.surface != "abstract" && entry.surface != "pdf_version_vs_tei_body")
    );
}

/// Everything crossing a receipt boundary roundtrips, and the wire shape of the
/// three outcomes is pinned so a rename cannot pass unnoticed.
#[test]
fn steps_outcomes_and_receipts_roundtrip_with_a_pinned_wire_shape() {
    roundtrip(&sentence(true, Some("id")));
    roundtrip(&language(false, None));
    roundtrip(&NativeBodyStepOutcome::NotRequested);
    roundtrip(&NativeBodyStepOutcome::MissingResource {
        resource: NATIVE_SENTENCE_RESOURCE_NAME.into(),
    });
    roundtrip(&NativeBodyStepOutcome::Unsupported {
        resource: "id".into(),
        reason: NATIVE_LANGUAGE_GAP_REASON.into(),
    });
    roundtrip(&sentence(true, None).receipt());
    roundtrip(&native_body_version_surface()[0]);

    let not_requested = serde_json::to_value(NativeBodyStepOutcome::NotRequested).unwrap();
    assert_eq!(not_requested["outcome"], "not_requested");
    let missing = serde_json::to_value(NativeBodyStepOutcome::MissingResource {
        resource: NATIVE_SENTENCE_RESOURCE_NAME.into(),
    })
    .unwrap();
    assert_eq!(missing["outcome"], "missing_resource");
    assert_eq!(missing["resource"], NATIVE_SENTENCE_RESOURCE_NAME);
    let unsupported = serde_json::to_value(NativeBodyStepOutcome::Unsupported {
        resource: "id".into(),
        reason: NATIVE_SENTENCE_GAP_REASON.into(),
    })
    .unwrap();
    assert_eq!(unsupported["outcome"], "unsupported");
    assert_eq!(unsupported["resource"], "id");

    let step = serde_json::to_value(NativeBodyStep::Sentence).unwrap();
    assert_eq!(step, "sentence");
    let kind = serde_json::to_value(NativeBodySurfaceKind::NonCrfStep).unwrap();
    assert_eq!(kind, "non_crf_step");
}

/// Repeated evaluation of the same declared step is byte-identical.
#[test]
fn evaluation_is_deterministic() {
    let step = language(true, Some("models/lang"));
    assert_eq!(step.run(), step.run());
    assert_eq!(step.receipt(), step.receipt());
    assert_eq!(native_body_version_surface(), native_body_version_surface());
}
