//! S02/T06 (M516-th0oby) — hermetic contract for the committed native TEI
//! emission artifact.
//!
//! This test reads committed bytes alone (`artifacts/parse_compat/
//! native_route_tei_emission.json`, the route receipt and the slice
//! declaration).  It never reads the gitignored scholarly PDF corpus and never
//! runs the route.  It proves the committed emission envelope keeps the
//! diagnostic envelope and the five locks closed, carries an
//! installed/identity-isolated guard-owned zero, accounts for every family in
//! the closed disposition vocabulary, carries no forbidden report field
//! (`data/parse/SCHEMA.yaml#comparison_document.forbidden_report_fields`), and
//! that the identities it claims are recomputed from the committed carriers —
//! then it refuses each named mutation by name.
//!
//! The emission is held in-process by the runner (no Python, no network), so the
//! raw bytes are not committed and only the digest-bound envelope is; this test
//! audits the envelope, never the uncommitted TEI.

use da_cli::native_family_receipt::{REQUIRED_LOCKS, sha256_prefixed};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const ARTIFACT: &str =
    include_str!("../../../artifacts/parse_compat/native_route_tei_emission.json");
const ROUTE: &str = include_str!("../../../artifacts/parse_compat/native_route_e2e.json");
const LAYOUT: &str = include_str!("../../../data/parse/layout_bakeoff.yaml");
const SCHEMA: &str = include_str!("../../../data/parse/SCHEMA.yaml");

const TEI_EMISSION_SCHEMA_VERSION: &str = "native-route-tei-emission/v1";
const EMISSION_CORPUS_POINTER: &str = "data/parse/layout_bakeoff.yaml";

const NATIVE_TEI_FAMILIES: [&str; 8] = [
    "header",
    "references",
    "body",
    "figure",
    "table",
    "formula",
    "visual",
    "callout",
];
const NATIVE_TEI_DISPOSITIONS: [&str; 2] = ["native_emitted", "typed_absence"];

// ───────────────────────────── refusals ─────────────────────────────

/// One named refusal per mutation the audit owns.  Every variant is a refusal,
/// never a silent downgrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmissionAuditRefusal {
    SchemaVersion,
    ClaimScope,
    ProductCoverageEffect,
    Thresholds,
    LockOpened,
    GuardObserverDrift,
    GuardNotInstalled,
    GuardNotIdentityIsolated,
    GuardJavaCallsNonzero,
    GuardDrift,
    CorpusDrift,
    CorpusPointerDrift,
    DocumentSetDrift,
    SourceDigestDrift,
    ConfigFingerprintDrift,
    FamilyDispositionMissing,
    FamilyDispositionUnknown,
    ForbiddenReportField,
}

// ───────────────────────────── carriers ─────────────────────────────

fn artifact() -> Value {
    serde_json::from_str(ARTIFACT).expect("the committed emission artifact parses")
}

fn route_receipt() -> Value {
    serde_json::from_str(ROUTE).expect("the committed route receipt parses")
}

fn layout() -> serde_yaml::Value {
    serde_yaml::from_str(LAYOUT).expect("the slice declaration parses")
}

fn schema() -> serde_yaml::Value {
    serde_yaml::from_str(SCHEMA).expect("the parse schema parses")
}

/// The corpus the declaration pins: `(id, sha256)` per document, in declaration
/// order.  This is the committed carrier the emission identity is recomputed
/// against.
fn declared_documents(layout: &serde_yaml::Value) -> BTreeMap<String, String> {
    layout["native_route_corpus"]["documents"]
        .as_sequence()
        .expect("corpus.documents is a sequence")
        .iter()
        .map(|document| {
            (
                document["id"].as_str().expect("a declared id").to_owned(),
                document["sha256"]
                    .as_str()
                    .expect("a declared sha256")
                    .to_owned(),
            )
        })
        .collect()
}

fn forbidden_report_fields(schema: &serde_yaml::Value) -> BTreeSet<String> {
    schema["comparison_document"]["forbidden_report_fields"]
        .as_sequence()
        .expect("forbidden_report_fields is a sequence")
        .iter()
        .map(|value| value.as_str().expect("a forbidden field").to_owned())
        .collect()
}

fn collect_keys(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                out.insert(key.clone());
                collect_keys(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_keys(item, out);
            }
        }
        _ => {}
    }
}

// ───────────────────────────── audit ─────────────────────────────

/// The hermetic audit of the committed emission envelope.  It recomputes every
/// claim it can from the committed carriers — the declaration's pinned corpus
/// and document digests, the declaration bytes' own sha256, and the route
/// receipt's guard/locks — and refuses, by name, the first violation it finds.
fn audit(
    artifact: &Value,
    route: &Value,
    layout: &serde_yaml::Value,
    schema: &serde_yaml::Value,
    layout_sha: &str,
) -> Result<(), EmissionAuditRefusal> {
    // Envelope.
    if artifact["schema_version"].as_str() != Some(TEI_EMISSION_SCHEMA_VERSION) {
        return Err(EmissionAuditRefusal::SchemaVersion);
    }
    if artifact["claim_scope"].as_str() != Some("diagnostic_only") {
        return Err(EmissionAuditRefusal::ClaimScope);
    }
    if artifact["product_coverage_effect"].as_str() != Some("none") {
        return Err(EmissionAuditRefusal::ProductCoverageEffect);
    }
    if artifact["thresholds"].as_str() != Some("not_accepted") {
        return Err(EmissionAuditRefusal::Thresholds);
    }

    // The five locks, exactly closed.
    let locks = artifact["locks"]
        .as_object()
        .ok_or(EmissionAuditRefusal::LockOpened)?;
    if locks.len() != REQUIRED_LOCKS.len() {
        return Err(EmissionAuditRefusal::LockOpened);
    }
    for (lock, value) in REQUIRED_LOCKS {
        if locks.get(lock).and_then(Value::as_str) != Some(value) {
            return Err(EmissionAuditRefusal::LockOpened);
        }
    }

    // Guard: installed, identity-isolated, observing zero Java consults, and
    // exactly the guard the committed route receipt records.
    let guard = artifact["guard"]
        .as_object()
        .ok_or(EmissionAuditRefusal::GuardDrift)?;
    let route_guard = route["guard"]
        .as_object()
        .ok_or(EmissionAuditRefusal::GuardDrift)?;
    if guard.get("observer").and_then(Value::as_str)
        != route_guard.get("observer").and_then(Value::as_str)
    {
        return Err(EmissionAuditRefusal::GuardObserverDrift);
    }
    if guard.get("installed").and_then(Value::as_bool) != Some(true) {
        return Err(EmissionAuditRefusal::GuardNotInstalled);
    }
    if guard.get("identity_isolated").and_then(Value::as_bool) != Some(true) {
        return Err(EmissionAuditRefusal::GuardNotIdentityIsolated);
    }
    if guard.get("observed_java_calls").and_then(Value::as_u64) != Some(0) {
        return Err(EmissionAuditRefusal::GuardJavaCallsNonzero);
    }
    if artifact["guard"] != route["guard"] {
        return Err(EmissionAuditRefusal::GuardDrift);
    }

    // Corpus: recomputed from the declaration.
    let corpus = artifact["corpus"]
        .as_object()
        .ok_or(EmissionAuditRefusal::CorpusDrift)?;
    let declared = layout["native_route_corpus"]
        .as_mapping()
        .ok_or(EmissionAuditRefusal::CorpusDrift)?;
    let declared_id = declared
        .get(serde_yaml::Value::from("id"))
        .and_then(serde_yaml::Value::as_str);
    if corpus.get("corpus_id").and_then(Value::as_str) != declared_id {
        return Err(EmissionAuditRefusal::CorpusDrift);
    }
    let declared_n = declared
        .get(serde_yaml::Value::from("corpus_n"))
        .and_then(serde_yaml::Value::as_u64);
    if corpus.get("document_count").and_then(Value::as_u64) != declared_n {
        return Err(EmissionAuditRefusal::CorpusDrift);
    }
    let declared_pages = declared
        .get(serde_yaml::Value::from("page_denominator"))
        .and_then(serde_yaml::Value::as_u64);
    if corpus.get("page_denominator").and_then(Value::as_u64) != declared_pages {
        return Err(EmissionAuditRefusal::CorpusDrift);
    }
    if corpus.get("pointer").and_then(Value::as_str) != Some(EMISSION_CORPUS_POINTER) {
        return Err(EmissionAuditRefusal::CorpusPointerDrift);
    }

    // The document set is exactly the declared corpus.
    let declared_docs = declared_documents(layout);
    let documents = artifact["documents"]
        .as_array()
        .ok_or(EmissionAuditRefusal::DocumentSetDrift)?;
    let mut seen = BTreeSet::new();
    for document in documents {
        let id = document["document_id"]
            .as_str()
            .ok_or(EmissionAuditRefusal::DocumentSetDrift)?;
        if !declared_docs.contains_key(id) || !seen.insert(id.to_owned()) {
            return Err(EmissionAuditRefusal::DocumentSetDrift);
        }
    }
    if seen != declared_docs.keys().cloned().collect::<BTreeSet<_>>() {
        return Err(EmissionAuditRefusal::DocumentSetDrift);
    }

    // Per-document identity recomputed from the committed carriers, and a
    // non-empty, closed, per-family disposition.
    for document in documents {
        let id = document["document_id"]
            .as_str()
            .expect("the document set passed");
        let identity = &document["emission"]["identity"];
        if identity["source_sha256"].as_str()
            != Some(
                declared_docs
                    .get(id)
                    .expect("the document set passed")
                    .as_str(),
            )
        {
            return Err(EmissionAuditRefusal::SourceDigestDrift);
        }
        if identity["config_fingerprint"].as_str() != Some(layout_sha) {
            return Err(EmissionAuditRefusal::ConfigFingerprintDrift);
        }

        let families = document["emission"]["families"]
            .as_array()
            .ok_or(EmissionAuditRefusal::FamilyDispositionMissing)?;
        if families.is_empty() {
            return Err(EmissionAuditRefusal::FamilyDispositionMissing);
        }
        let mut family_names = BTreeSet::new();
        for family in families {
            let name = family["family"]
                .as_str()
                .ok_or(EmissionAuditRefusal::FamilyDispositionUnknown)?;
            let disposition = family["disposition"]
                .as_str()
                .ok_or(EmissionAuditRefusal::FamilyDispositionUnknown)?;
            if !NATIVE_TEI_FAMILIES.contains(&name)
                || !NATIVE_TEI_DISPOSITIONS.contains(&disposition)
            {
                return Err(EmissionAuditRefusal::FamilyDispositionUnknown);
            }
            if disposition == "typed_absence" && family["reason"].as_str().is_none() {
                return Err(EmissionAuditRefusal::FamilyDispositionUnknown);
            }
            if !family_names.insert(name.to_owned()) {
                return Err(EmissionAuditRefusal::FamilyDispositionMissing);
            }
        }
        if family_names != NATIVE_TEI_FAMILIES.iter().map(|f| f.to_string()).collect() {
            return Err(EmissionAuditRefusal::FamilyDispositionMissing);
        }
    }

    // No forbidden report field may appear at any depth.
    let mut keys = BTreeSet::new();
    collect_keys(artifact, &mut keys);
    for forbidden in forbidden_report_fields(schema) {
        if keys.contains(&forbidden) {
            return Err(EmissionAuditRefusal::ForbiddenReportField);
        }
    }

    Ok(())
}

fn mutated(mutation: impl FnOnce(&mut Value)) -> EmissionAuditRefusal {
    let mut artifact = artifact();
    mutation(&mut artifact);
    audit(
        &artifact,
        &route_receipt(),
        &layout(),
        &schema(),
        &sha256_prefixed(LAYOUT.as_bytes()),
    )
    .expect_err("the mutation must be refused")
}

// ───────────────────────────── clean ─────────────────────────────

#[test]
fn committed_artifact_is_redacted_and_audits_clean() {
    let artifact = artifact();
    assert!(
        !ARTIFACT.trim().is_empty(),
        "the artifact must be non-empty"
    );
    for marker in ["http://", "https://", "/root", "/home", "/tmp", "<TEI"] {
        assert!(
            !ARTIFACT.contains(marker),
            "the artifact leaked the marker `{marker}`"
        );
    }
    audit(
        &artifact,
        &route_receipt(),
        &layout(),
        &schema(),
        &sha256_prefixed(LAYOUT.as_bytes()),
    )
    .expect("the committed artifact audits clean");
}

#[test]
fn committed_artifact_recomputes_its_identity_and_guard_from_committed_carriers() {
    let artifact = artifact();
    let route = route_receipt();
    let layout = layout();
    let declared = declared_documents(&layout);
    let layout_sha = sha256_prefixed(LAYOUT.as_bytes());

    // The declaration's own digest is the config fingerprint every document
    // claims; the declaration's pinned digest is the source identity.
    assert_eq!(
        layout["native_route_corpus"]["documents"]
            .as_sequence()
            .expect("declared documents")
            .len(),
        declared.len()
    );
    for document in artifact["documents"].as_array().expect("documents") {
        let id = document["document_id"].as_str().expect("a document id");
        let identity = &document["emission"]["identity"];
        assert_eq!(
            identity["config_fingerprint"].as_str(),
            Some(layout_sha.as_str()),
            "config fingerprint must be the declaration digest"
        );
        assert_eq!(
            identity["source_sha256"].as_str(),
            Some(declared.get(id).expect("a declared document").as_str()),
            "source digest must be the declaration's pin for {id}"
        );
    }

    // The guard, the locks and the envelope are recomputed from the route
    // receipt the emission was built beside.
    assert_eq!(artifact["guard"], route["guard"]);
    assert_eq!(artifact["locks"], route["locks"]);
    assert_eq!(artifact["claim_scope"], route["claim_scope"]);
    assert_eq!(
        artifact["product_coverage_effect"],
        route["product_coverage_effect"]
    );
    assert_eq!(artifact["thresholds"], route["thresholds"]);
}

// ───────────────────────────── refusals ─────────────────────────────

#[test]
fn refuses_a_wrong_schema_version() {
    assert_eq!(
        mutated(|a| a["schema_version"] = json!("native-route-tei-emission/v2")),
        EmissionAuditRefusal::SchemaVersion
    );
}

#[test]
fn refuses_a_non_diagnostic_claim_scope() {
    assert_eq!(
        mutated(|a| a["claim_scope"] = json!("product")),
        EmissionAuditRefusal::ClaimScope
    );
}

#[test]
fn refuses_a_product_coverage_claim() {
    assert_eq!(
        mutated(|a| a["product_coverage_effect"] = json!("full")),
        EmissionAuditRefusal::ProductCoverageEffect
    );
}

#[test]
fn refuses_a_threshold_claim() {
    assert_eq!(
        mutated(|a| a["thresholds"] = json!("accepted")),
        EmissionAuditRefusal::Thresholds
    );
}

#[test]
fn refuses_an_opened_lock() {
    assert_eq!(
        mutated(|a| a["locks"]["deployed"] = json!("true")),
        EmissionAuditRefusal::LockOpened
    );
    assert_eq!(
        mutated(|a| {
            let _ = a["locks"]
                .as_object_mut()
                .expect("locks")
                .remove("graph_write");
        }),
        EmissionAuditRefusal::LockOpened
    );
}

#[test]
fn refuses_a_guard_observer_drift() {
    assert_eq!(
        mutated(|a| a["guard"]["observer"] = json!("elsewhere")),
        EmissionAuditRefusal::GuardObserverDrift
    );
}

#[test]
fn refuses_an_uninstalled_guard_for_an_observed_zero() {
    assert_eq!(
        mutated(|a| a["guard"]["installed"] = json!(false)),
        EmissionAuditRefusal::GuardNotInstalled
    );
}

#[test]
fn refuses_a_non_isolated_guard() {
    assert_eq!(
        mutated(|a| a["guard"]["identity_isolated"] = json!(false)),
        EmissionAuditRefusal::GuardNotIdentityIsolated
    );
}

/// Positive control: the guard-owned zero is only a measurement because a
/// nonzero count is refused.
#[test]
fn refuses_a_nonzero_java_count() {
    assert_eq!(
        mutated(|a| a["guard"]["observed_java_calls"] = json!(1)),
        EmissionAuditRefusal::GuardJavaCallsNonzero
    );
}

#[test]
fn refuses_a_guard_drift_past_the_individual_fields() {
    assert_eq!(
        mutated(|a| a["guard"]["extra"] = json!(true)),
        EmissionAuditRefusal::GuardDrift
    );
}

#[test]
fn refuses_a_corpus_drift_from_the_declaration() {
    assert_eq!(
        mutated(|a| a["corpus"]["document_count"] = json!(13)),
        EmissionAuditRefusal::CorpusDrift
    );
    assert_eq!(
        mutated(|a| a["corpus"]["page_denominator"] = json!(380)),
        EmissionAuditRefusal::CorpusDrift
    );
    assert_eq!(
        mutated(|a| a["corpus"]["corpus_id"] = json!("another-corpus")),
        EmissionAuditRefusal::CorpusDrift
    );
}

#[test]
fn refuses_a_corpus_pointer_drift() {
    assert_eq!(
        mutated(|a| a["corpus"]["pointer"] = json!("elsewhere.yaml")),
        EmissionAuditRefusal::CorpusPointerDrift
    );
}

#[test]
fn refuses_a_document_outside_the_declared_corpus() {
    assert_eq!(
        mutated(|a| a["documents"][0]["document_id"] = json!("ghost-0000.00000")),
        EmissionAuditRefusal::DocumentSetDrift
    );
    assert_eq!(
        mutated(|a| {
            let _ = a["documents"].as_array_mut().expect("documents").pop();
        }),
        EmissionAuditRefusal::DocumentSetDrift
    );
}

#[test]
fn refuses_a_source_digest_drift() {
    assert_eq!(
        mutated(|a| {
            a["documents"][0]["emission"]["identity"]["source_sha256"] =
                json!(format!("sha256:{}", "0".repeat(64)));
        }),
        EmissionAuditRefusal::SourceDigestDrift
    );
}

#[test]
fn refuses_a_config_fingerprint_drift() {
    assert_eq!(
        mutated(|a| {
            a["documents"][0]["emission"]["identity"]["config_fingerprint"] =
                json!(format!("sha256:{}", "0".repeat(64)));
        }),
        EmissionAuditRefusal::ConfigFingerprintDrift
    );
}

#[test]
fn refuses_an_empty_or_partial_family_set() {
    assert_eq!(
        mutated(|a| a["documents"][0]["emission"]["families"] = json!([])),
        EmissionAuditRefusal::FamilyDispositionMissing
    );
    assert_eq!(
        mutated(|a| {
            let _ = a["documents"][0]["emission"]["families"]
                .as_array_mut()
                .expect("families")
                .pop();
        }),
        EmissionAuditRefusal::FamilyDispositionMissing
    );
}

#[test]
fn refuses_an_unknown_family_disposition() {
    assert_eq!(
        mutated(|a| a["documents"][0]["emission"]["families"][0]["disposition"] = json!("bogus")),
        EmissionAuditRefusal::FamilyDispositionUnknown
    );
    assert_eq!(
        mutated(|a| a["documents"][0]["emission"]["families"][0]["family"] = json!("bogus")),
        EmissionAuditRefusal::FamilyDispositionUnknown
    );
}

#[test]
fn refuses_an_unreasoned_typed_absence() {
    assert_eq!(
        mutated(|a| {
            let families = a["documents"][0]["emission"]["families"]
                .as_array_mut()
                .expect("families");
            let typed = families
                .iter_mut()
                .find(|family| family["disposition"] == "typed_absence")
                .expect("a typed absence");
            let _ = typed.as_object_mut().expect("an object").remove("reason");
        }),
        EmissionAuditRefusal::FamilyDispositionUnknown
    );
}

#[test]
fn refuses_a_forbidden_report_field() {
    assert_eq!(
        mutated(|a| a["tei_xml"] = json!("<TEI/>")),
        EmissionAuditRefusal::ForbiddenReportField
    );
    assert_eq!(
        mutated(|a| a["documents"][0]["raw_tei"] = json!("<TEI/>")),
        EmissionAuditRefusal::ForbiddenReportField
    );
}

/// The forbidden set is read from the committed schema, so the audit cannot
/// silently drop a field the schema forbids.
#[test]
fn the_forbidden_field_set_comes_from_the_committed_schema() {
    let fields = forbidden_report_fields(&schema());
    for expected in ["tei_xml", "raw_tei", "body_text", "pdf_bytes"] {
        assert!(
            fields.contains(expected),
            "the schema must forbid `{expected}`"
        );
    }
}
