//! M515/S03/T02 — contract test for the corpus-bound same-n resource artifact.
//!
//! This test is deliberately **ungated** (no `#![cfg(feature = ...)]`): the
//! `da-cli --tests` gate compiles it in the default feature set and executes it,
//! rather than compiling an empty target. It re-derives the assigned census from
//! the frozen holdout manifest and the freeze revision from the committed
//! governor carriers, binds the committed artifact and asserts its canonical
//! byte re-serialization, its non-empty native denominators, its measured
//! same-n Java reference pair (independently recomputing the pinned latency
//! maximum from `provenance.yaml`) and rejects each named mutation.
//!
//! The S03 receipt replaced the typed-absence-only reference shape with a
//! measured pair: `java_latency_ms` is `observed` against the per-document Java
//! provenance carrier, which is same-n to the 531 measurable documents, while
//! `java_model_invocations` / `java_cpu_wall_clock_ns` stay `not_observed` with
//! the closed reason `reference_row_absent` (D510).

use da_cli::native_acceptance::{
    DEFAULT_ACCEPTANCE_ARTIFACT_REL, HOLDOUT_MANIFEST_REL, NativeAcceptanceReceipt,
    REFERENCE_PROBE_REL, load_holdout_documents,
};
use da_cli::native_corpus_resource_same_n::{
    AssignedIdentity, CORPUS_RESOURCE_GOVERNOR_POINTER, CORPUS_RESOURCE_GOVERNOR_REL,
    CORPUS_RESOURCE_NATIVE_MEASUREMENT_REL, CORPUS_RESOURCE_PAIRING_POPULATION_VOCABULARY,
    CORPUS_RESOURCE_REFERENCE_METRIC_ORDER, CORPUS_RESOURCE_REFERENCE_PROVENANCE_REL,
    CORPUS_RESOURCE_REFERENCE_ROW_ABSENT, CORPUS_RESOURCE_ROUTE_RECEIPT_REL,
    CorpusResourceSameNExpectation, CorpusResourceSameNReceipt,
    DEFAULT_CORPUS_RESOURCE_SAME_N_ARTIFACT_REL, carrier_bare_digest, freeze_revision,
    validate_corpus_resource_same_n_receipt,
};
use da_cli::native_family_receipt::{CarrierBinding, sha256_hex, sha256_prefixed};
use da_cli::native_resource_same_n::{
    SAME_N_ABSENCE_REASON_VOCABULARY, SAME_N_OBSERVATION_STATE_VOCABULARY,
};
use da_cli::native_route_receipt::RouteLimitsBinding;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels under the repository root")
        .to_path_buf()
}

fn read(root: &Path, rel: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(root.join(rel))?)
}

/// Every named mutation must be refused by name; a mutation that slips through
/// fails this test.
fn expect_refusal(
    label: &str,
    receipt: &CorpusResourceSameNReceipt,
    expectation: &CorpusResourceSameNExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_corpus_resource_same_n_receipt(receipt, expectation) {
        Ok(()) => Err(format!("{label} was admitted").into()),
        Err(error) => {
            let debug = format!("{error:?}");
            if debug.starts_with(variant) {
                Ok(())
            } else {
                Err(format!("{label} refused as `{debug}`, expected `{variant}`").into())
            }
        }
    }
}

#[test]
fn native_corpus_resource_same_n_artifact_is_canonical_and_census_bound()
-> Result<(), Box<dyn Error>> {
    let root = repo_root();

    // ── re-derive the assigned census from the frozen manifest ─────────────
    let manifest_bytes = read(&root, HOLDOUT_MANIFEST_REL)?;
    let assigned: Vec<AssignedIdentity> =
        load_holdout_documents(std::str::from_utf8(&manifest_bytes)?)?
            .into_iter()
            .map(|document| AssignedIdentity {
                id: document.id,
                source_pdf: document.pdf_path,
                sha256: document.sha256,
            })
            .collect();
    assert_eq!(assigned.len(), 610, "assigned census drifted");

    // ── re-derive the freeze revision from the committed carriers ──────────
    let governor_bytes = read(&root, CORPUS_RESOURCE_GOVERNOR_REL)?;
    let value: serde_yaml::Value = serde_yaml::from_slice(&governor_bytes)?;
    let table = value
        .get(CORPUS_RESOURCE_GOVERNOR_POINTER)
        .and_then(|node| node.get("limit_policy"))
        .and_then(|node| node.get("freeze_revision"))
        .and_then(|node| node.get("carriers"))
        .and_then(|node| node.as_sequence())
        .ok_or("the governor block declares no freeze carriers")?;
    let mut carriers = Vec::with_capacity(table.len());
    for carrier in table {
        let path = carrier
            .get("path")
            .and_then(|node| node.as_str())
            .ok_or("a freeze carrier has no path")?;
        let bytes = fs::read(root.join(path))?;
        carriers.push((path.to_owned(), carrier_bare_digest(&bytes)));
    }
    let revision = freeze_revision(&carriers);
    assert!(revision.starts_with("sha256:"), "freeze revision malformed");

    // ── bind the T01 carrier and prove it parses ───────────────────────────
    let t01_bytes = read(&root, DEFAULT_ACCEPTANCE_ARTIFACT_REL)?;
    let t01 = NativeAcceptanceReceipt::parse_json_bytes(&t01_bytes)?;
    let native_measurement = CarrierBinding {
        manifest: CORPUS_RESOURCE_NATIVE_MEASUREMENT_REL.to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: sha256_prefixed(&t01_bytes),
    };

    let expectation = CorpusResourceSameNExpectation {
        freeze_revision: revision,
        native_measurement: native_measurement.clone(),
        assigned: assigned.clone(),
    };

    // ── the artifact exists, is canonical and validates ────────────────────
    let artifact_bytes = read(&root, DEFAULT_CORPUS_RESOURCE_SAME_N_ARTIFACT_REL)?;
    let receipt = CorpusResourceSameNReceipt::parse_json_bytes(&artifact_bytes)?;
    validate_corpus_resource_same_n_receipt(&receipt, &expectation)?;
    assert_eq!(
        receipt.artifact_json_bytes()?,
        artifact_bytes,
        "the artifact is not its own canonical re-serialization"
    );

    // ── the full assigned denominator is carried, never shrunken ───────────
    assert_eq!(receipt.populations.assigned, 610, "assigned drifted");
    assert_eq!(receipt.populations.measurable, 531, "measurable drifted");
    assert_eq!(receipt.populations.excluded, 79, "excluded drifted");
    assert_eq!(receipt.documents.len(), 610, "document rows drifted");

    // ── every assigned document has exactly one typed row ──────────────────
    let mut seen = BTreeSet::new();
    for document in &receipt.documents {
        assert!(
            seen.insert(document.id.as_str()),
            "duplicate document row `{}`",
            document.id
        );
        assert!(
            ["observed", "refused", "not_run"].contains(&document.plane_state.as_str()),
            "unknown plane state `{}` on `{}`",
            document.plane_state,
            document.id
        );
    }
    assert_eq!(seen.len(), 610);

    // ── native rows carry non-empty denominators (no silent zero) ──────────
    assert!(
        !receipt.native_metrics.is_empty(),
        "no native metric measured"
    );
    for row in &receipt.native_metrics {
        assert!(
            row.denominator > 0,
            "native metric `{}` carries an empty denominator",
            row.metric_id
        );
    }

    // ── the T01 carrier binding is current, not the stale pre-S03 one ──────
    assert_eq!(
        receipt.native_measurement, native_measurement,
        "the receipt names a different T01 measurement carrier"
    );

    // ── exactly one reference carrier is same-n, and it is the provenance ──
    let same_n_carriers: Vec<_> = receipt
        .reference_carriers
        .iter()
        .filter(|entry| entry.same_n)
        .collect();
    assert_eq!(
        same_n_carriers.len(),
        1,
        "exactly one carrier must be declared same-n"
    );
    let provenance_carrier = same_n_carriers[0];
    assert_eq!(
        provenance_carrier.carrier.manifest,
        CORPUS_RESOURCE_REFERENCE_PROVENANCE_REL
    );
    assert_eq!(provenance_carrier.carrier.pointer, "documents");
    assert!(
        provenance_carrier.reason.is_empty(),
        "a same-n carrier must not carry an absence reason"
    );
    assert!(
        CORPUS_RESOURCE_PAIRING_POPULATION_VOCABULARY
            .contains(&provenance_carrier.pairing_population.as_str()),
        "same-n carrier names a population outside the vocabulary"
    );
    assert_eq!(
        provenance_carrier.pairing_population, "measurable",
        "the same-n carrier must pair over the measurable population"
    );
    assert_eq!(
        provenance_carrier.covered_documents, receipt.populations.measurable,
        "the same-n carrier must cover the measurable population exactly"
    );
    assert_eq!(provenance_carrier.covered_documents, 531);

    // Every other declared carrier is typed not-same-n, covers strictly fewer
    // documents than the population it names, and carries a closed reason.
    for entry in receipt.reference_carriers.iter().filter(|e| !e.same_n) {
        let population = if entry.pairing_population == "assigned" {
            receipt.populations.assigned
        } else {
            receipt.populations.measurable
        };
        assert!(
            entry.covered_documents < population,
            "a not-same-n carrier `{}` must cover strictly fewer than its population",
            entry.carrier.manifest
        );
        assert!(
            SAME_N_ABSENCE_REASON_VOCABULARY.contains(&entry.reason.as_str()),
            "reference carrier `{}` has an untyped absence reason",
            entry.carrier.manifest
        );
    }

    // ── exactly one reference row is observed: the measured Java latency ───
    let observed_rows: Vec<_> = receipt
        .reference_metrics
        .iter()
        .filter(|row| row.is_observed())
        .collect();
    assert_eq!(
        observed_rows.len(),
        1,
        "exactly one reference row must be observed"
    );
    let observed = observed_rows[0];
    assert_eq!(observed.id, "java_latency_ms");
    assert_eq!(observed.state, "observed");
    assert!(
        observed.reason.is_none(),
        "an observed row must not carry an absence reason"
    );
    let receipt_latency = observed
        .value
        .ok_or("the observed java_latency_ms row carries no value")?;
    let source = observed
        .source
        .as_ref()
        .ok_or("the observed java_latency_ms row carries no source")?;
    assert_eq!(source.kind, "measured_reference");
    assert_eq!(
        source.manifest.as_deref(),
        Some(CORPUS_RESOURCE_REFERENCE_PROVENANCE_REL)
    );
    assert_eq!(source.pointer.as_deref(), Some("documents[].latency_ms"));
    assert_eq!(source.measurement_state.as_deref(), Some("measured"));
    assert_eq!(source.metric_id.as_deref(), Some("latency_ms"));

    // The source sha256 is the *bare* digest of the provenance file, while the
    // carrier binding carries the prefixed digest of the same bytes.
    let provenance_bytes = read(&root, CORPUS_RESOURCE_REFERENCE_PROVENANCE_REL)?;
    let provenance_bare_sha256 = sha256_hex(&provenance_bytes);
    let provenance_prefixed_sha256 = sha256_prefixed(&provenance_bytes);
    assert_eq!(
        source.sha256.as_deref(),
        Some(provenance_bare_sha256.as_str())
    );
    assert_eq!(
        provenance_carrier.carrier.revision_sha256, provenance_prefixed_sha256,
        "the same-n carrier pins a different provenance revision"
    );

    // ── the two unmeasured metrics are typed absences ──────────────────────
    for id in ["java_model_invocations", "java_cpu_wall_clock_ns"] {
        let row = receipt
            .reference_metrics
            .iter()
            .find(|row| row.id == id)
            .ok_or_else(|| format!("reference metric `{id}` is missing"))?;
        assert_eq!(row.state, "not_observed", "`{id}` must stay not_observed");
        assert!(row.value.is_none(), "`{id}` must carry no number");
        assert!(row.source.is_none(), "`{id}` must carry no observed source");
        assert_eq!(
            row.reason.as_deref(),
            Some(CORPUS_RESOURCE_REFERENCE_ROW_ABSENT),
            "`{id}` must carry the closed reference_row_absent reason"
        );
    }

    // ── every reference row is inside the corpus vocabulary and well-typed ─
    for row in &receipt.reference_metrics {
        assert!(
            CORPUS_RESOURCE_REFERENCE_METRIC_ORDER.contains(&row.id.as_str()),
            "reference metric `{}` is outside the vocabulary",
            row.id
        );
        assert!(
            SAME_N_OBSERVATION_STATE_VOCABULARY.contains(&row.state.as_str()),
            "reference metric `{}` has an unknown state",
            row.id
        );
        if !row.is_observed() {
            let reason = row.reason.as_deref().unwrap_or_default();
            assert!(
                SAME_N_ABSENCE_REASON_VOCABULARY.contains(&reason),
                "reference metric `{}` has an untyped reason",
                row.id
            );
        }
    }

    // ── independently recompute the pinned latency maximum ─────────────────
    // The provenance carrier has no state field; the measurable population is
    // the 531 rows the T01 acceptance receipt measured, read here from that
    // carrier, and the latencies are read from `provenance.yaml` by pointer
    // `documents[].latency_ms`.
    let measurable_ids: Vec<&str> = t01
        .documents
        .iter()
        .filter(|document| document.state == "measurable")
        .map(|document| document.id.as_str())
        .collect();
    assert_eq!(measurable_ids.len(), 531, "measurable census drifted");

    let provenance_value: serde_yaml::Value = serde_yaml::from_slice(&provenance_bytes)?;
    let provenance_rows = provenance_value
        .get("documents")
        .and_then(serde_yaml::Value::as_sequence)
        .ok_or("the provenance carrier declares no documents")?;
    let mut latency_by_id: BTreeMap<&str, u64> = BTreeMap::new();
    for row in provenance_rows {
        let id = row
            .get("article_id")
            .and_then(serde_yaml::Value::as_str)
            .ok_or("a provenance row has no article_id")?;
        let latency = row
            .get("latency_ms")
            .and_then(serde_yaml::Value::as_u64)
            .ok_or("a provenance row has no latency_ms")?;
        assert!(
            latency_by_id.insert(id, latency).is_none(),
            "duplicate provenance article id `{id}`"
        );
    }
    let mut recomputed_max = 0u64;
    for id in &measurable_ids {
        let latency = *latency_by_id
            .get(id)
            .ok_or_else(|| format!("measurable document `{id}` has no provenance row"))?;
        recomputed_max = recomputed_max.max(latency);
    }
    assert_eq!(
        receipt_latency, recomputed_max,
        "the receipt latency does not match the provenance recompute"
    );

    let probe_bytes = read(&root, REFERENCE_PROBE_REL)?;
    let probe_value: serde_json::Value = serde_json::from_slice(&probe_bytes)?;
    let probe_max = probe_value
        .get("latency_accounting")
        .and_then(|node| node.get("max_latency_ms"))
        .and_then(serde_json::Value::as_u64)
        .ok_or("the release probe declares no latency_accounting.max_latency_ms")?;
    assert_eq!(
        receipt_latency, probe_max,
        "the receipt latency does not match the release probe"
    );

    // ── the closure counters recompute from the carried rows ───────────────
    assert_eq!(
        receipt.residuals.observed_metrics,
        receipt.native_metrics.len() as u64 + 1,
        "observed_metrics must be the native rows plus the one observed reference row"
    );
    // The corpus envelope adds `java_latency_ms`, so the observed count is the
    // six native rows T01 measured plus the one measured reference row.
    assert_eq!(receipt.residuals.observed_metrics, 7);
    assert_eq!(receipt.residuals.absent_metrics, 2);
    assert_eq!(receipt.residuals.untyped_metrics, 0);
    assert_eq!(receipt.residuals.reference_rows_bound, 1);

    // ── the limits freeze binding matches the committed route receipt ──────
    let route_bytes = read(&root, CORPUS_RESOURCE_ROUTE_RECEIPT_REL)?;
    let route_value: serde_json::Value = serde_json::from_slice(&route_bytes)?;
    let route_limits: RouteLimitsBinding = serde_json::from_value(
        route_value
            .get("limits")
            .cloned()
            .ok_or("the route receipt carries no `limits` block")?,
    )?;
    assert_eq!(
        receipt.limits, route_limits,
        "limits freeze binding drifted"
    );

    // ── named mutations are all refused ────────────────────────────────────
    let mut m = receipt.clone();
    m.documents[0].sha256 = "sha256:0000".to_owned();
    expect_refusal("document_set_mismatch", &m, &expectation, "Mismatch")?;

    let mut m = receipt.clone();
    m.documents[0].id = "unknown-9999.00000".to_owned();
    expect_refusal("unknown_document", &m, &expectation, "Mismatch")?;

    let mut m = receipt.clone();
    m.pairing.pooled = "allowed".to_owned();
    expect_refusal("pooled_claim", &m, &expectation, "PooledClaimed")?;

    let mut m = receipt.clone();
    m.pairing.saving_claim = "allowed".to_owned();
    expect_refusal("saving_claim", &m, &expectation, "SavingClaimed")?;

    // An untyped reason on a not-observed row (the corpus envelope's observed
    // row is `java_latency_ms`, so mutate the java_model_invocations absence).
    let mut m = receipt.clone();
    let absent_index = m
        .reference_metrics
        .iter()
        .position(|row| !row.is_observed())
        .ok_or("no absent reference row to mutate")?;
    m.reference_metrics[absent_index].reason = Some("mystery".to_owned());
    expect_refusal("untyped_reason", &m, &expectation, "AbsenceReasonUnknown")?;

    let mut m = receipt.clone();
    m.native_metrics[0].denominator = 0;
    expect_refusal("empty_denominator", &m, &expectation, "ZeroMasquerade")?;

    let mut m = receipt.clone();
    m.locks
        .insert("graph_write".to_owned(), "allowed".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;

    let mut drifted = expectation.clone();
    drifted.freeze_revision = sha256_prefixed(b"drifted");
    expect_refusal(
        "freeze_revision_drift",
        &receipt,
        &drifted,
        "StaleFreezeRevision",
    )?;

    // A same-n carrier may not carry an absence reason.
    let mut m = receipt.clone();
    let same_n_index = m
        .reference_carriers
        .iter()
        .position(|entry| entry.same_n)
        .ok_or("no same-n carrier to mutate")?;
    m.reference_carriers[same_n_index].reason = CORPUS_RESOURCE_REFERENCE_ROW_ABSENT.to_owned();
    expect_refusal(
        "same_n_carrier_with_reason",
        &m,
        &expectation,
        "ReferenceSourceUnpinned",
    )?;

    // A same-n carrier whose coverage drifts off the population is refused.
    let mut m = receipt.clone();
    let same_n_index = m
        .reference_carriers
        .iter()
        .position(|entry| entry.same_n)
        .ok_or("no same-n carrier to mutate")?;
    let covered = m.reference_carriers[same_n_index].covered_documents;
    m.reference_carriers[same_n_index].covered_documents = covered - 1;
    expect_refusal(
        "same_n_carrier_partial_coverage",
        &m,
        &expectation,
        "Mismatch",
    )?;

    // An observed row with no source may not masquerade as a measurement.
    let mut m = receipt.clone();
    let observed_index = m
        .reference_metrics
        .iter()
        .position(|row| row.is_observed())
        .ok_or("no observed reference row to mutate")?;
    m.reference_metrics[observed_index].source = None;
    expect_refusal(
        "observed_row_without_source",
        &m,
        &expectation,
        "ZeroMasquerade",
    )?;

    // A redaction leak must be refused by the serializer itself.
    let mut m = receipt.clone();
    m.documents[0].source_pdf = "/tmp/leak".to_owned();
    let leak = m.artifact_json_bytes();
    assert!(
        matches!(
            leak,
            Err(da_cli::native_resource_same_n::SameNReceiptError::RedactionLeak { .. })
        ),
        "a physical path leak was not refused"
    );

    Ok(())
}
