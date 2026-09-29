//! Independent consumer for the pure one-lane promotion rehearsal primitive.
//!
//! This test imports the standalone example as a module: it never registers the
//! example in the library, never launches Cargo or another process, and never
//! takes a boolean out of the report as proof — the executed-rollback claim is
//! asserted on the emitted bytes.

// The example compiles its live-substrate path only into `main`, which this
// module build deliberately excludes (`#[cfg(not(test))]`), so the consumer test
// links just the hermetic surface.  The unused live-path items are allowed here,
// in the consumer, instead of weakening the example's own lint surface.
#[path = "../examples/native_family_promotion_rehearsal.rs"]
#[allow(dead_code)]
mod rehearsal;

use rehearsal::{
    AdmittedLaneOutcome, DATA_COMPARISON, DATA_COMPOSITION, DEFAULT_REPORT_LIVE,
    DEFAULT_REPORT_STUB, DeclaredRouteTable, EmissionOutcome,
    FORBIDDEN_REPORT_TOKENS as EXAMPLE_FORBIDDEN_REPORT_TOKENS, LaneState, NativeSideRefusal,
    REHEARSAL_EPISODE, REHEARSAL_LANE, RehearsalDocument, RehearsalIdentity, RehearsalLane,
    RehearsalRefusal, RehearsalRun, SubstrateDescriptor, SwitchRefusal, carrier_sha256,
    default_root, digest, fixture, fixture_header_outcome, fixture_java_tei, identity_for,
    locate_repo_root, read_declared_route_table, rehearsal_report, run_rehearsal_triad, self_check,
    switch_lane,
};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    let from_manifest = locate_repo_root(Path::new(env!("CARGO_MANIFEST_DIR")));
    // `default_root` is the example's own resolution used by `main`; the two must
    // name the same checkout whenever the process cwd is inside one.
    if let (Some(manifest), Some(cwd)) = (from_manifest.as_deref(), default_root()) {
        assert_eq!(
            manifest,
            cwd.as_path(),
            "manifest-dir and cwd must resolve one checkout root"
        );
    }
    from_manifest.expect("the declared route carriers are tracked files of this checkout")
}

fn declared() -> (PathBuf, DeclaredRouteTable) {
    let root = repo_root();
    let table =
        read_declared_route_table(&root).expect("the declared route table must be switchable");
    (root, table)
}

fn admitted_triad(table: &DeclaredRouteTable) -> rehearsal::TriadOutcome {
    let declared = fixture();
    let identity = identity_for(&declared);
    let java_tei = fixture_java_tei(&declared);
    run_rehearsal_triad(
        &java_tei,
        &identity,
        &identity.profile_fingerprint,
        table,
        AdmittedLaneOutcome::Admitted(Box::new(fixture_header_outcome())),
    )
}

#[test]
fn fixture_is_the_declared_hermetic_oracle() {
    let declared = fixture();
    assert_eq!(
        declared.schema_version,
        "da-parse.native-fulltext-composition-oracle/v1"
    );
    assert_eq!(declared.case_id, "frozen-hybrid-fixture");
    assert_eq!(declared.profile_fingerprint, "profile=fixture");
    // `fixture_java_tei` asserts the declared sha256 against the bytes it returns.
    let java_tei = fixture_java_tei(&declared);
    assert_eq!(java_tei.len(), declared.source_xml.len());
}

#[test]
fn selectable_lanes_are_the_declared_pair_in_order() {
    let (_, table) = declared();
    let vocabulary: Vec<&str> = RehearsalLane::ALL.iter().map(|lane| lane.token()).collect();
    assert_eq!(
        vocabulary,
        ["header", "references"],
        "R125: exactly these two"
    );
    let table_lanes: Vec<&str> = table.lanes.iter().map(|row| row.lane.token()).collect();
    assert_eq!(table_lanes, vocabulary);
    let states = table.states();
    assert_eq!(states.state(RehearsalLane::Header), LaneState::Java);
    assert_eq!(states.state(RehearsalLane::References), LaneState::Java);
}

#[test]
fn declared_route_table_reads_both_carriers_and_locks_the_candidate() {
    let (root, table) = declared();

    assert_eq!(
        table.candidate_id, "da-parse.native_header_reference_composition",
        "the rehearsed family is the declared candidate, not a free name"
    );
    assert_eq!(table.candidate_version, "1.0.0");
    assert_eq!(table.observation_mode, "shadow");
    assert_eq!(table.promotion_state, "candidate");
    assert_eq!(table.auto_promote, "forbidden");
    assert_eq!(table.pooling, "forbidden");
    assert_eq!(table.lane_closure, "own_receipt_only");
    assert!(!table.terminal_receipt_is_pass);
    assert!(table.rust_run_may_not_cancel_fallback);
    assert_eq!(table.fallback_target, "java-grobid-http@0.9.0-crf");
    assert_eq!(table.states(), rehearsal::LaneStates::java());

    let lanes: Vec<&str> = table.lanes.iter().map(|row| row.lane.token()).collect();
    assert_eq!(lanes, ["header", "references"], "R125: exactly these two");
    for row in &table.lanes {
        assert_eq!(row.state, LaneState::Java, "the declared route is Java");
        assert!(row.independently_selectable);
        assert_eq!(row.rollback_target, "java-grobid-http@0.9.0-crf");
    }
    assert_eq!(
        table.lane(RehearsalLane::Header).unwrap().admission_state,
        "passed"
    );

    // The carrier digests are read from disk, once per carrier, and the map has
    // exactly the two declared carriers.
    let keys: Vec<String> = table.carrier_sha256.keys().cloned().collect();
    assert_eq!(
        keys,
        [DATA_COMPARISON.to_owned(), DATA_COMPOSITION.to_owned()]
    );
    for rel in [DATA_COMPARISON, DATA_COMPOSITION] {
        assert_eq!(
            table.carrier_sha256.get(rel).unwrap(),
            &carrier_sha256(&root, rel),
            "carrier digest must come from the bytes on disk"
        );
        assert!(table.carrier_sha256[rel].starts_with("sha256:"));
        assert_eq!(table.carrier_sha256[rel].len(), "sha256:".len() + 64);
    }
}

#[test]
fn switch_moves_exactly_one_lane_and_never_mutates_the_declared_table() {
    let (_, table) = declared();
    let original = table.clone();

    let switched = switch_lane(&table, RehearsalLane::Header, LaneState::Native)
        .expect("the declared table admits exactly this switch");

    assert_eq!(
        switched.lane(RehearsalLane::Header).unwrap().state,
        LaneState::Native
    );
    assert_eq!(
        switched.lane(RehearsalLane::References).unwrap().state,
        LaneState::Java,
        "lane closure is own_receipt_only: the twin lane must not move"
    );
    assert_eq!(switched.states().header, LaneState::Native);
    assert_eq!(switched.states().references, LaneState::Java);
    assert_ne!(switched, table, "a switch must produce a different table");
    assert_eq!(table, original, "the declared table must not be mutated");

    // The return is the same primitive, and it restores the declared table.
    let returned = switch_lane(&switched, RehearsalLane::Header, LaneState::Java)
        .expect("the return to Java is always available");
    assert_eq!(returned, original);
    assert_eq!(returned.states(), rehearsal::LaneStates::java());

    // Switching a lane that is already Java to Java is a legal no-op.
    assert_eq!(
        switch_lane(&table, RehearsalLane::References, LaneState::Java).unwrap(),
        original
    );
}

#[test]
fn returned_emission_is_byte_identical_to_baseline() {
    let (_, table) = declared();
    let declared = fixture();
    let java_tei = fixture_java_tei(&declared);
    let triad = admitted_triad(&table);

    // baseline is the lane on Java, so the composer returns the source bytes
    // verbatim: that is the foundation of the rollback claim.
    assert_eq!(
        triad.baseline.bytes, java_tei,
        "a fallback-only emission must be the byte-identical Java envelope"
    );
    // Assert on bytes, never on the report's boolean.
    assert_eq!(triad.returned.bytes, triad.baseline.bytes);
    assert_eq!(triad.returned.sha256, triad.baseline.sha256);
    assert!(triad.rollback.byte_identity);
    assert!(triad.rollback.returned_equals_baseline);
    assert!(triad.rollback.table_returned_to_declared);
    assert_eq!(triad.returned_table, table);
    assert!(triad.switch_refusal.is_none());
    assert_eq!(triad.returned.lane_states, rehearsal::LaneStates::java());
    assert_eq!(triad.baseline.outcome_token(), "accepted");
    assert_eq!(triad.switched.outcome_token(), "accepted");
    assert_eq!(triad.returned.outcome_token(), "accepted");
}

#[test]
fn switched_emission_moves_only_the_header_band() {
    let (_, table) = declared();
    let triad = admitted_triad(&table);

    assert!(
        triad.switched_performed(),
        "an admitted document must actually switch"
    );
    assert_ne!(
        triad.switched.bytes, triad.baseline.bytes,
        "the switched emission must differ from the Java baseline"
    );
    assert_eq!(triad.switched.outcome, rehearsal::EmissionOutcome::Accepted);
    assert!(triad.switched.error_token.is_none());
    assert_eq!(
        triad.switched.lane_states.header,
        LaneState::Native,
        "the switched emission must report the rehearsed lane as native"
    );
    assert_eq!(triad.switched.lane_states.references, LaneState::Java);

    assert!(triad.lane_movement.header_changed);
    assert!(
        !triad.lane_movement.references_changed,
        "the twin lane must not move in any switched document"
    );
    // The byte delta is the native header band, and it is bounded by the schema's
    // component cap rather than being an arbitrary rewrite of the document.
    assert!(triad.byte_delta() > 0);
    assert!(triad.byte_delta() < 4 * 1024 * 1024);
}

#[test]
fn every_refusal_class_fires_on_its_own_violation() {
    let (_, table) = declared();
    let lane = RehearsalLane::Header;
    let target = LaneState::Native;

    let mut cases: Vec<(RehearsalRefusal, DeclaredRouteTable)> = Vec::new();

    let mut unknown_lane = table.clone();
    unknown_lane.lanes.retain(|row| row.lane != lane);
    cases.push((RehearsalRefusal::UnknownLane, unknown_lane));

    let mut not_selectable = table.clone();
    for row in &mut not_selectable.lanes {
        if row.lane == lane {
            row.independently_selectable = false;
        }
    }
    cases.push((
        RehearsalRefusal::LaneNotIndependentlySelectable,
        not_selectable,
    ));

    let mut pooling = table.clone();
    pooling.pooling = "allowed".to_owned();
    cases.push((RehearsalRefusal::PoolingForbidden, pooling));

    let mut cancels_fallback = table.clone();
    cancels_fallback.rust_run_may_not_cancel_fallback = false;
    cases.push((
        RehearsalRefusal::RustRunMayNotCancelFallback,
        cancels_fallback,
    ));

    let mut missing_rollback = table.clone();
    for row in &mut missing_rollback.lanes {
        if row.lane == lane {
            row.rollback_target = String::new();
        }
    }
    cases.push((RehearsalRefusal::MissingRollbackTarget, missing_rollback));

    let mut drifted_rollback = table.clone();
    for row in &mut drifted_rollback.lanes {
        if row.lane == lane {
            row.rollback_target = "java-grobid-http@0.9.1".to_owned();
        }
    }
    cases.push((RehearsalRefusal::RollbackTargetDrift, drifted_rollback));

    let mut auto_promote = table.clone();
    auto_promote.auto_promote = "allowed".to_owned();
    cases.push((RehearsalRefusal::AutoPromoteNotForbidden, auto_promote));

    let mut second_promotion = table.clone();
    for row in &mut second_promotion.lanes {
        if row.lane == lane {
            row.state = LaneState::Native;
        }
    }
    cases.push((
        RehearsalRefusal::SecondPromotionWithoutRollback,
        second_promotion,
    ));

    let mut not_shadow = table.clone();
    not_shadow.observation_mode = "live".to_owned();
    cases.push((RehearsalRefusal::ObservationModeNotShadow, not_shadow));

    let mut not_candidate = table.clone();
    not_candidate.promotion_state = "promoted".to_owned();
    cases.push((RehearsalRefusal::PromotionStateNotCandidate, not_candidate));

    // Anti-no-op: every mutation fires exactly its own class, and the observed
    // errors in case order equal the whole closed vocabulary — so no class is
    // shadowed by an earlier check and none collapsed into another.
    let mut observed = Vec::with_capacity(cases.len());
    for (expected, mutated) in &cases {
        let error = switch_lane(mutated, lane, target)
            .expect_err("a violated declared table must not be switchable");
        assert_eq!(
            error, *expected,
            "the mutation for {expected:?} fired {error:?} instead"
        );
        observed.push(error);
    }
    assert_eq!(observed, RehearsalRefusal::ALL);
    assert_eq!(cases.len(), RehearsalRefusal::ALL.len());
}

#[test]
fn refusal_wire_tokens_are_the_closed_snake_case_vocabulary() {
    let wire: Vec<String> = RehearsalRefusal::ALL
        .iter()
        .map(|refusal| {
            serde_json::to_value(refusal)
                .expect("a refusal serializes")
                .as_str()
                .expect("a refusal uses a string wire value")
                .to_owned()
        })
        .collect();
    let tokens: Vec<&str> = RehearsalRefusal::ALL.iter().map(|r| r.token()).collect();
    assert_eq!(wire, tokens, "serde rename and token() must not drift");
    assert_eq!(
        tokens,
        [
            "unknown_lane",
            "lane_not_independently_selectable",
            "pooling_forbidden",
            "rust_run_may_not_cancel_fallback",
            "missing_rollback_target",
            "rollback_target_drift",
            "auto_promote_not_forbidden",
            "second_promotion_without_rollback",
            "observation_mode_not_shadow",
            "promotion_state_not_candidate",
        ]
    );
}

#[test]
fn a_refused_native_side_reports_java_and_refuses_typed() {
    let (_, table) = declared();
    let declared = fixture();
    let identity = identity_for(&declared);
    let java_tei = fixture_java_tei(&declared);

    for (refusal, token) in [
        (
            NativeSideRefusal::NativeOutcomeRejected,
            "native_outcome_rejected",
        ),
        (
            NativeSideRefusal::NativeOutcomeAbsent,
            "native_outcome_absent",
        ),
    ] {
        let triad = run_rehearsal_triad(
            &java_tei,
            &identity,
            &identity.profile_fingerprint,
            &table,
            AdmittedLaneOutcome::Refused(refusal),
        );
        assert_eq!(triad.switched.outcome, EmissionOutcome::Refused);
        assert_eq!(triad.switched.outcome_token(), "refused");
        assert_eq!(triad.switched.error_token.as_deref(), Some(token));
        // A refused native side is the Java baseline, byte for byte.
        assert_eq!(triad.switched.bytes, triad.baseline.bytes);
        assert_eq!(triad.switched.sha256, triad.baseline.sha256);
        assert_eq!(triad.switched.lane_states, rehearsal::LaneStates::java());
        assert!(!triad.lane_movement.header_changed);
        assert!(!triad.lane_movement.references_changed);
        assert!(!triad.switched_performed());
        // The policy allowed the switch, so the table returns unchanged rather
        // than being left on the native route.
        assert_eq!(triad.rehearsal_table, table);
        assert_eq!(triad.returned_table, table);
        assert!(triad.rollback.byte_identity);
        assert_eq!(
            triad.switch_refusal,
            Some(SwitchRefusal::NativeSide(refusal))
        );
    }
}

fn rehearsal_run() -> RehearsalRun {
    let (root, table_before) = declared();
    let declared = fixture();
    let identity = identity_for(&declared);
    let java_tei = fixture_java_tei(&declared);
    let triad = run_rehearsal_triad(
        &java_tei,
        &identity,
        &identity.profile_fingerprint,
        &table_before,
        AdmittedLaneOutcome::Admitted(Box::new(fixture_header_outcome())),
    );
    let table_after =
        read_declared_route_table(&root).expect("the declared route table after the run");
    RehearsalRun {
        identity: RehearsalIdentity {
            candidate_id: table_before.candidate_id.clone(),
            candidate_version: table_before.candidate_version.clone(),
            rehearsed_lane: REHEARSAL_LANE,
            episode: REHEARSAL_EPISODE.to_owned(),
        },
        substrate: SubstrateDescriptor::fixture_stub(&identity.profile_fingerprint),
        table_before,
        table_after,
        documents: vec![RehearsalDocument {
            document_id: declared.case_id.clone(),
            source_pdf_sha256: None,
            source_pdf_sha256_absent_reason: Some(rehearsal::STUB_PDF_ABSENT_REASON.to_owned()),
            java_tei_sha256: digest(&java_tei),
            native_side: AdmittedLaneOutcome::Admitted(Box::new(fixture_header_outcome())),
            triad,
        }],
    }
}

#[test]
fn report_is_deterministic_and_carries_the_declared_locks() {
    let run = rehearsal_run();
    let first = serde_json::to_string(&rehearsal_report(&run)).expect("report serializes");
    let second = serde_json::to_string(&rehearsal_report(&run)).expect("report serializes");
    assert_eq!(first, second, "two calls must give byte-equal JSON");

    let report: serde_json::Value = serde_json::from_str(&first).expect("report is JSON");
    assert_eq!(
        report["schema_version"],
        "native-family-promotion-rehearsal.v1"
    );
    assert_eq!(report["claim_scope"], "rehearsal_only");
    assert_eq!(report["pooled_quality_claim"], "forbidden");
    assert_eq!(
        report["identity"]["candidate_id"],
        "da-parse.native_header_reference_composition"
    );
    assert_eq!(report["identity"]["rehearsed_lane"], "header");
    assert_eq!(report["identity"]["episode"], "rehearsal");

    // The rehearsal table is the in-memory switch; the declared table after the
    // run is the carriers as re-read from disk.
    assert_eq!(report["rehearsal_table"]["lanes"][0]["state"], "native");
    assert_eq!(report["rehearsal_table"]["lanes"][1]["state"], "java");
    assert_eq!(
        report["declared_route_table_before"], report["declared_route_table_after"],
        "the declared table must be identical before and after"
    );
    assert_eq!(
        report["declared_route_table_after"]["lanes"][0]["state"],
        "java"
    );
    assert_eq!(
        report["declared_route_table_after"]["carrier_sha256"][DATA_COMPARISON],
        report["declared_route_table_before"]["carrier_sha256"][DATA_COMPARISON]
    );
    assert_eq!(
        report["declared_route_table_after"]["carrier_sha256"][DATA_COMPOSITION],
        report["declared_route_table_before"]["carrier_sha256"][DATA_COMPOSITION]
    );

    assert_eq!(report["rehearsal"]["documents_total"], 1);
    assert_eq!(report["rehearsal"]["documents_switched"], 1);
    assert_eq!(report["rehearsal"]["documents_refused"], 0);
    assert_eq!(report["rehearsal"]["rollback_byte_identical_all"], true);
    assert_eq!(report["rehearsal"]["references_never_moved"], true);
    assert_eq!(report["rehearsal"]["switch_and_return_non_vacuous"], true);
    assert_eq!(report["rehearsal"]["carriers_unchanged"], true);

    assert_eq!(report["named_gaps"]["java_header_raw"], 21);
    assert_eq!(report["named_gaps"]["native_accepted_header"], 20);
    assert_eq!(report["named_gaps"]["pooling"], "forbidden");

    let locks = &report["locks"];
    assert_eq!(locks["deployed"], false);
    assert_eq!(locks["import_eligible"], false);
    assert_eq!(locks["production_route"], "forbidden");
    assert_eq!(locks["graph_write"], "forbidden");
    assert_eq!(locks["auto_promote"], "forbidden");
    assert!(locks["promotion_receipt"].is_null());
    assert_eq!(locks["this_wave_promotes"], "none");
    assert_eq!(locks["observation_mode"], "shadow");
    assert_eq!(locks["promotion_state"], "candidate");
    assert!(locks["explicit_decision"].is_null());
    assert_eq!(report["redaction"]["physical_paths_published"], false);
    assert_eq!(report["redaction"]["raw_tei_published"], false);

    // Per-document byte identity travels in the report and agrees with the
    // confirmed bytes, so a reader does not have to trust one summary flag.
    let document = &report["documents"][0];
    assert_eq!(
        document["baseline"]["sha256"],
        document["returned"]["sha256"]
    );
    assert_eq!(document["rollback"]["byte_identity"], true);
    assert_eq!(document["lane_movement"]["references_changed"], false);
    assert_eq!(document["lane_movement"]["header_changed"], true);
}

#[test]
fn rehearsal_never_rewrites_a_declared_carrier() {
    let root = repo_root();
    let before: Vec<Vec<u8>> = [DATA_COMPARISON, DATA_COMPOSITION]
        .iter()
        .map(|rel| std::fs::read(root.join(rel)).expect("carrier is readable"))
        .collect();
    let before_digests: Vec<String> = [DATA_COMPARISON, DATA_COMPOSITION]
        .iter()
        .map(|rel| carrier_sha256(&root, rel))
        .collect();

    let run = rehearsal_run();
    let report = rehearsal_report(&run);
    assert_eq!(report["rehearsal"]["carriers_unchanged"], true);

    let after: Vec<Vec<u8>> = [DATA_COMPARISON, DATA_COMPOSITION]
        .iter()
        .map(|rel| std::fs::read(root.join(rel)).expect("carrier is readable"))
        .collect();
    let after_digests: Vec<String> = [DATA_COMPARISON, DATA_COMPOSITION]
        .iter()
        .map(|rel| carrier_sha256(&root, rel))
        .collect();

    assert_eq!(before, after, "a rehearsal must not touch either carrier");
    assert_eq!(before_digests, after_digests);
    assert_eq!(
        run.table_before.carrier_sha256,
        run.table_after.carrier_sha256
    );
}

// ---------------------------------------------------------------------------
// S06/T02 — the declared-substrate harness: scope, denylist, guard vocabulary
// ---------------------------------------------------------------------------

/// Mirror of the example's redaction denylist.  The assertion is on the SET, so
/// a drift in either list fails (S04 history: two parallel lists, 11 vs 10).
const FORBIDDEN_REPORT_TOKENS: [&str; 15] = [
    "<TEI",
    "<tei",
    "%PDF-",
    "data/sources",
    "file://",
    "/root/",
    "/home/",
    "target/debug",
    "target/release",
    "executable",
    "hostname",
    "/sys/",
    "docker-",
    ".scope",
    "daily-archive-grobid",
];

/// The guards `self_check()` must prove it can fire.  Deliberately a LITERAL
/// copy rather than an import, so the example's behaviour and this expectation
/// cannot drift together unnoticed.
const REQUIRED_SELF_CHECK_GUARDS: [&str; 15] = [
    "rehearsed-lane-not-independently-selectable",
    "pooling-in-rehearsal",
    "rust-run-cancelled-fallback",
    "missing-rollback-target",
    "returned-emission-differs-from-baseline",
    "lane-movement-outside-rehearsed-lane",
    "zero-switched-documents-is-vacuous",
    "policy-lock-lifted",
    "carrier-sha256-changed-after-run",
    "promotion-receipt-present",
    "explicit-decision-present",
    "forbidden-token-in-report",
    "native-side-admitted-without-switched-difference",
    "native-side-refused-without-reason",
    "measurement-scope-absent",
];

fn snapshot(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|bytes| digest(&bytes))
}

#[test]
fn denylist_matches_the_example_as_a_set() {
    let example: std::collections::BTreeSet<&str> =
        EXAMPLE_FORBIDDEN_REPORT_TOKENS.iter().copied().collect();
    let mirrored: std::collections::BTreeSet<&str> =
        FORBIDDEN_REPORT_TOKENS.iter().copied().collect();
    assert_eq!(
        example, mirrored,
        "the example and test denylists must be the same set"
    );
    assert_eq!(
        mirrored.len(),
        FORBIDDEN_REPORT_TOKENS.len(),
        "a denylist must not repeat a token"
    );
}

#[test]
fn self_check_fires_exactly_the_declared_guard_set() {
    let fired = self_check().expect("the fail-closed self-check must pass");
    let fired: std::collections::BTreeSet<&str> = fired.iter().copied().collect();
    let required: std::collections::BTreeSet<&str> =
        REQUIRED_SELF_CHECK_GUARDS.iter().copied().collect();
    assert_eq!(
        fired, required,
        "the fired guard set must EQUAL the declared vocabulary"
    );
    assert_eq!(
        required.len(),
        REQUIRED_SELF_CHECK_GUARDS.len(),
        "the guard vocabulary must not repeat a name"
    );
}

#[test]
fn self_check_creates_and_modifies_no_file() {
    let root = repo_root();
    let reports = [
        root.join(DEFAULT_REPORT_STUB),
        root.join(DEFAULT_REPORT_LIVE),
    ];
    let before: Vec<Option<String>> = reports.iter().map(|path| snapshot(path)).collect();
    let carriers_before: Vec<String> = [DATA_COMPARISON, DATA_COMPOSITION]
        .iter()
        .map(|rel| carrier_sha256(&root, rel))
        .collect();

    let fired = self_check().expect("the fail-closed self-check must pass");
    assert_eq!(fired.len(), REQUIRED_SELF_CHECK_GUARDS.len());

    let after: Vec<Option<String>> = reports.iter().map(|path| snapshot(path)).collect();
    let carriers_after: Vec<String> = [DATA_COMPARISON, DATA_COMPOSITION]
        .iter()
        .map(|rel| carrier_sha256(&root, rel))
        .collect();
    assert_eq!(
        before, after,
        "self_check must not create or modify a report"
    );
    assert_eq!(
        carriers_before, carriers_after,
        "self_check must not touch a declared carrier"
    );
}

#[test]
fn stub_report_declares_the_measurement_scope() {
    let run = rehearsal_run();
    let report = rehearsal_report(&run);
    assert_eq!(report["claim_scope"], "rehearsal_only");
    assert_eq!(report["pooled_quality_claim"], "forbidden");
    assert_eq!(report["substrate"]["kind"], "local_deterministic_stub");
    assert_eq!(report["substrate"]["live_grobid_participated"], false);
    assert_eq!(
        report["documents"][0]["source_pdf_sha256"],
        serde_json::Value::Null,
        "the stub sends no PDF, so no PDF digest may be published"
    );
    assert_eq!(
        report["documents"][0]["java_tei_sha256"],
        report["documents"][0]["baseline"]["sha256"]
    );
    assert_eq!(report["rehearsal"]["carriers_unchanged"], true);
    assert_eq!(
        report["rehearsal_table"]["equality_with_declared_after"],
        true
    );
    assert_eq!(report["rehearsal_table"]["lanes"][0]["state"], "native");
    assert_eq!(report["rehearsal_table"]["lanes"][1]["state"], "java");
    // The report publishes the closed fail-closed guard vocabulary, and it must
    // EQUAL the test's independent literal copy (set equality, not just names).
    let published: std::collections::BTreeSet<&str> = report["guards"]["fail_closed"]
        .as_array()
        .expect("guards.fail_closed is an array")
        .iter()
        .map(|value| value.as_str().expect("a guard name is a string"))
        .collect();
    let required: std::collections::BTreeSet<&str> =
        REQUIRED_SELF_CHECK_GUARDS.iter().copied().collect();
    assert_eq!(
        published, required,
        "the report must publish exactly the closed guard vocabulary"
    );

    // The declared route tables carry no explicit promotion decision, on either
    // side: the switch is in-memory and undeclared to production.
    assert!(report["declared_route_table_before"]["explicit_decision"].is_null());
    assert!(report["declared_route_table_after"]["explicit_decision"].is_null());

    // The scope carries the whole point: these numbers are a rehearsal, and a
    // pooled quality claim is forbidden rather than merely absent.
    rehearsal::validate_report(&report).expect("the stub report must be admissible");
}
