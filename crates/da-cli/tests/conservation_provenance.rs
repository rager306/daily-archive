//! GAR-005 contract: a conservation verdict must declare which run produced it.
//!
//! P9 is a `candidate_lift` stamp: parse-pdf never calls the conservation
//! comparator (`engine_called=false`, `computed_this_run=false`). Before the
//! fix the row still published `tracked_verdict: conserved`, so an aggregator
//! could count a preserved-TEI reference claim as this run's conservation
//! measurement.
//!
//! The surface under test is the projection of the *same* row builder the stamp
//! merges into `cascade_observe`, read at the documented location
//! (`cascade_observe.P9`). Nothing here restates the emitted row by hand; only
//! the deliberately malformed probes are hand-built.
//!
//! Cases:
//! 1. not measured this run — the only shape parse-pdf emits today;
//! 2. measured this run — a run that ran the comparator itself, and it is
//!    distinguishable from case 1;
//! 3. the pre-fix shape: a historic claim with no current-run namespace;
//! 4. malformed provenance — absent, ill-formed, unknown or self-contradictory.

use da_cli::source_cli::{
    ConservationVerdictProvenance, P9_CURRENT_RUN_NOT_EVALUATED, P9_VERDICT_NAMESPACE_CURRENT_RUN,
    P9_VERDICT_NAMESPACE_HISTORICAL_REFERENCE, classify_conservation_verdict_provenance,
    historic_conservation_reference, p9_cascade_observe_document,
};
use serde_yaml::{Mapping, Value};

/// Build a mapping from literal wire keys/values.
fn mapping_of(entries: &[(&str, Value)]) -> Mapping {
    Mapping::from_iter(
        entries
            .iter()
            .map(|(key, value)| ((*key).into(), value.clone())),
    )
}

/// A `current_run` namespace, optionally omitting either declared field.
fn current_run(namespace: Option<&str>, verdict: Option<Value>) -> Mapping {
    let mut current = Mapping::new();
    if let Some(namespace) = namespace {
        current.insert("verdict_namespace".into(), namespace.into());
    }
    if let Some(verdict) = verdict {
        current.insert("verdict".into(), verdict);
    }
    current
}

/// Read the P9 observation at the location a consumer of the artifact reads.
fn p9_row() -> Mapping {
    p9_cascade_observe_document()
        .get("cascade_observe")
        .and_then(Value::as_mapping)
        .and_then(|observe| observe.get("P9"))
        .and_then(Value::as_mapping)
        .expect("cascade_observe.P9 must exist in the stamped document")
        .clone()
}

fn current_run_of(row: &Mapping) -> &Mapping {
    row.get("current_run")
        .and_then(Value::as_mapping)
        .expect("current_run namespace")
}

fn s(row: &Mapping, key: &str) -> Option<String> {
    row.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn sc(row: &Mapping, outer: &str, inner: &str) -> Option<String> {
    row.get(outer)
        .and_then(Value::as_mapping)
        .and_then(|nested| nested.get(inner))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

#[test]
fn stamped_p9_publishes_no_conservation_verdict_for_a_run_that_did_not_measure_it() {
    let row = p9_row();

    // The existing honesty flags stay as they are and are not repurposed.
    assert_eq!(s(&row, "stage").as_deref(), Some("P9"));
    assert_eq!(s(&row, "production_authority").as_deref(), Some("tei_lift"));
    assert_eq!(s(&row, "observe_mode").as_deref(), Some("candidate_lift"));
    assert_eq!(
        row.get("engine_called").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        row.get("computed_this_run").and_then(Value::as_bool),
        Some(false)
    );

    // The current-run namespace can therefore only say "not evaluated".
    let current = current_run_of(&row);
    assert_eq!(
        current.get("verdict_namespace").and_then(Value::as_str),
        Some("current_run")
    );
    assert_eq!(
        current.get("verdict").and_then(Value::as_str),
        Some("NotEvaluated")
    );
    assert_eq!(
        current.get("evaluation_state").and_then(Value::as_str),
        Some("unmeasured")
    );

    // The previously recorded result is relocated, not lost: it stays readable
    // under a field that names it historic.
    assert_eq!(historic_conservation_reference(&row), Some("conserved"));
    assert_eq!(
        sc(&row, "reference_evidence", "verdict_namespace").as_deref(),
        Some("historical_reference")
    );
    assert_eq!(
        sc(&row, "reference_evidence", "evidence_state").as_deref(),
        Some("historical")
    );
    assert_eq!(
        sc(&row, "reference_evidence", "artifact_identity").as_deref(),
        Some("preserved_tei")
    );

    // No conservation claim is reachable without the scoped namespace.
    assert!(row.get("verdict").is_none());
    assert!(!row.contains_key("tracked_verdict"));
    assert!(!row.contains_key("tracked_on_preserved_tei"));

    assert_eq!(
        classify_conservation_verdict_provenance(&row),
        ConservationVerdictProvenance::NotEvaluatedThisRun
    );

    // The wire shape an external consumer sees keeps the historic value behind
    // its own namespace.
    let text = serde_yaml::to_string(&row).expect("serialize P9 row");
    assert!(text.contains("historical_reference"));
    assert!(!text.contains("tracked_verdict"));

    // Two assemblies of the same run are byte-identical.
    assert_eq!(p9_row(), row);
}

#[test]
fn a_run_that_measured_conservation_is_distinguishable_from_one_that_did_not() {
    let not_measured = p9_row();
    assert_eq!(
        classify_conservation_verdict_provenance(&not_measured),
        ConservationVerdictProvenance::NotEvaluatedThisRun
    );

    // Every value of the conservation engine's closed vocabulary reads as a
    // measurement *only* when the same run declares it in the current-run
    // namespace.
    for verdict in ["conserved", "loss_detected", "malformed_source"] {
        let measured = mapping_of(&[(
            "current_run",
            Value::Mapping(current_run(Some("current_run"), Some(verdict.into()))),
        )]);
        assert_eq!(
            classify_conservation_verdict_provenance(&measured),
            ConservationVerdictProvenance::MeasuredThisRun,
            "engine verdict `{verdict}` in the current-run namespace is a measurement"
        );
        // A measured row carries no historic reference, and the not-measured
        // row carries no measured current-run value: the shapes are disjoint.
        assert_eq!(historic_conservation_reference(&measured), None);
        assert_ne!(
            classify_conservation_verdict_provenance(&measured),
            classify_conservation_verdict_provenance(&not_measured)
        );
    }
}

#[test]
fn a_historic_verdict_outside_a_current_run_namespace_is_not_a_current_measurement() {
    // The pre-fix shape: the reference claim at the top level of P9, with the
    // honesty flags already reporting "not computed".
    let legacy = mapping_of(&[
        ("stage", "P9".into()),
        ("production_authority", "tei_lift".into()),
        ("observe_mode", "candidate_lift".into()),
        ("engine_called", false.into()),
        ("computed_this_run", false.into()),
        ("tracked_on_preserved_tei", true.into()),
        ("tracked_verdict", "conserved".into()),
    ]);
    assert_eq!(
        classify_conservation_verdict_provenance(&legacy),
        ConservationVerdictProvenance::Unattributed
    );
    assert!(legacy.get("verdict").is_none());
    assert_eq!(historic_conservation_reference(&legacy), None);

    // A historic value alone never reads as a current-run value either.
    let reference_only = mapping_of(&[(
        "reference_evidence",
        Value::Mapping(mapping_of(&[
            ("verdict_namespace", "historical_reference".into()),
            ("evidence_state", "historical".into()),
            ("verdict", "conserved".into()),
        ])),
    )]);
    assert_eq!(
        classify_conservation_verdict_provenance(&reference_only),
        ConservationVerdictProvenance::Unattributed
    );
    assert_eq!(
        historic_conservation_reference(&reference_only),
        Some("conserved")
    );

    // A historic claim cannot upgrade a not-evaluated current run.
    let not_measured = p9_row();
    assert_eq!(
        classify_conservation_verdict_provenance(&not_measured),
        ConservationVerdictProvenance::NotEvaluatedThisRun
    );
    assert_eq!(
        historic_conservation_reference(&not_measured),
        Some("conserved")
    );
}

#[test]
fn absent_ill_formed_unknown_or_contradictory_provenance_fails_closed() {
    let cases: Vec<(&str, Mapping)> = vec![
        ("empty row", Mapping::new()),
        (
            "current_run is not a mapping",
            mapping_of(&[("current_run", "NotEvaluated".into())]),
        ),
        (
            "current_run without a verdict",
            mapping_of(&[(
                "current_run",
                Value::Mapping(current_run(Some("current_run"), None)),
            )]),
        ),
        (
            "verdict is not a string",
            mapping_of(&[(
                "current_run",
                Value::Mapping(current_run(Some("current_run"), Some(Value::Bool(true)))),
            )]),
        ),
        (
            "verdict outside the closed vocabulary",
            mapping_of(&[(
                "current_run",
                Value::Mapping(current_run(
                    Some("current_run"),
                    Some("probably_fine".into()),
                )),
            )]),
        ),
        (
            "namespace declaration absent",
            mapping_of(&[(
                "current_run",
                Value::Mapping(current_run(None, Some("conserved".into()))),
            )]),
        ),
        (
            "namespace contradicts the key it lives under",
            mapping_of(&[(
                "current_run",
                Value::Mapping(current_run(
                    Some("historical_reference"),
                    Some("conserved".into()),
                )),
            )]),
        ),
        (
            "historic namespace nested under the current-run key",
            mapping_of(&[(
                "reference_evidence",
                Value::Mapping(current_run(Some("current_run"), Some("conserved".into()))),
            )]),
        ),
    ];
    for (name, row) in cases {
        assert_eq!(
            classify_conservation_verdict_provenance(&row),
            ConservationVerdictProvenance::Unattributed,
            "case `{name}` must fail closed rather than read as a current measurement"
        );
    }

    // A historic namespace nested under the current-run key is not readable
    // through the historic reader either: the key and the declaration disagree.
    let contradictory = mapping_of(&[(
        "reference_evidence",
        Value::Mapping(current_run(Some("current_run"), Some("conserved".into()))),
    )]);
    assert_eq!(historic_conservation_reference(&contradictory), None);
}

#[test]
fn public_constants_bind_the_emitted_wire_values() {
    assert_eq!(P9_VERDICT_NAMESPACE_CURRENT_RUN, "current_run");
    assert_eq!(
        P9_VERDICT_NAMESPACE_HISTORICAL_REFERENCE,
        "historical_reference"
    );
    assert_eq!(P9_CURRENT_RUN_NOT_EVALUATED, "NotEvaluated");

    let row = p9_row();
    assert_eq!(
        sc(&row, "current_run", "verdict_namespace").as_deref(),
        Some(P9_VERDICT_NAMESPACE_CURRENT_RUN)
    );
    assert_eq!(
        sc(&row, "reference_evidence", "verdict_namespace").as_deref(),
        Some(P9_VERDICT_NAMESPACE_HISTORICAL_REFERENCE)
    );
    assert_eq!(
        sc(&row, "current_run", "verdict").as_deref(),
        Some(P9_CURRENT_RUN_NOT_EVALUATED)
    );
}
