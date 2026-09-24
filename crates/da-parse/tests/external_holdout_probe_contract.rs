//! Frozen external release-probe lane — hermetic declaration contract
//! (M511-j7wz2f / S08 / T01).
//!
//! H20/GAR-015 has no state carrier for a probe: the existing
//! `external_holdout_contract.execution_status` is pinned `not_executed` by the
//! M510 contracts, and the external dataset rows must stay `bytes_absent`. S08
//! therefore adds a SEPARATE declared lane — `release_probe`, an additive child
//! of `external_holdout_contract` — whose declarations are frozen BEFORE any
//! measurement and whose admission can never lift the external `bytes_absent`
//! band.
//!
//! This contract is deliberately independent of both the Rust data-contract
//! validator in `crates/da-parse/tests/data_contracts.rs` and of the Governor
//! Python oracle (`scripts/governor/adapters/checks/external_holdout_package.py`):
//! ADR-051 requires an oracle to share no code with the surface it checks. It
//! imports no `da-parse` symbol, decodes no typed model, uses the generic
//! `serde_yaml` reader and performs the SHA-256 mathematics itself. Every
//! negative case mutates an in-process clone and must be caught by its own
//! named label, and a positive control proves the oracle discriminates instead
//! of refusing everything.
//!
//! The properties this contract re-derives from the declaration:
//!
//! 1. **Lane separation.** The external band stays `not_executed` / `bytes_absent`
//!    and every external row stays `bytes_absent`; flipping any of those to an
//!    executed state is `external_lane_state_flipped`, never a configurable
//!    switch.
//! 2. **Frozen identity.** The probe identity digest is recomputed from
//!    `data/parse/holdout_manifests.yaml` using exactly the declared
//!    canonicalization handles (`digest_input_fields`, `digest_input_order`,
//!    `digest_field_separator`, `digest_record_separator`); drift is
//!    `probe_identity_digest_drift` and an unfrozen lane is
//!    `probe_identity_not_frozen`.
//! 3. **One metric vocabulary, declared before measurement.** The declared
//!    metric names must equal the `acceptance_margins.margins[].metric` set (a
//!    rename is `probe_metric_vocabulary_drift`), no threshold value is
//!    duplicated (`probe_metric_threshold_duplicated`) and a zero-admitted
//!    population is never read as a vacuous 1.0
//!    (`probe_zero_admitted_population_read_as_one`, MEM1702).
//! 4. **Fail-closed citation.** While the recorded human release decision is
//!    `pending`, and while prose is offered instead of a recorded decision, the
//!    probe result is not citable (`citation_without_release_decision`,
//!    `prose_alone_authorizes_citation`). A cleared in-memory clone is citable,
//!    which is the positive control.

use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const PREFIX: &str = "[external-holdout-release-probe]";
const MANIFEST: &str = "data/parse/acceptance_manifest.yaml";
const SOURCE_MANIFEST: &str = "data/parse/holdout_manifests.yaml";

/// The closed key set of the `release_probe` lane. A lane that gains an
/// undeclared key is `probe_lane_unknown_key`: the declaration is closed DATA,
/// not an extensible bag.
const RELEASE_PROBE_KEYS: [&str; 17] = [
    "schema_version",
    "identity_input",
    "digest_is_byte_hash",
    "digest_is_authority_binding",
    "frozen_before_execution",
    "source_manifest",
    "digest_input_fields",
    "digest_input_order",
    "digest_field_separator",
    "digest_record_separator",
    "identity_digest",
    "margin_ref",
    "metrics",
    "latency",
    "cost",
    "citation_requires_recorded_human_release_decision",
    "citation_without_release_decision",
];

const RELEASE_PROBE_METRIC_KEYS: [&str; 7] = [
    "metric",
    "numerator_definition",
    "denominator_definition",
    "aggregation",
    "claim_scope",
    "zero_admitted_population_behavior",
    "margin_ref",
];

const RELEASE_PROBE_LATENCY_KEYS: [&str; 3] =
    ["unit", "measurement_source", "pooled_latency_claim"];

const RELEASE_PROBE_COST_KEYS: [&str; 3] =
    ["cost_status", "unknown_is_not_zero", "pooled_cost_claim"];

/// Threshold keys owned by `acceptance_margins`; duplicating any of them inside
/// a metric definition would be a second dictionary.
const THRESHOLD_KEYS: [&str; 4] = ["margin", "threshold", "direction", "tolerance"];

const REQUIRED_METRICS: [&str; 2] = [
    "document_agreement_ratio",
    "document_uncertainty_interval_width",
];

const RECORDED_DECISION_STATES: [&str; 2] = ["recorded", "approved"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel))
        .unwrap_or_else(|e| panic!("{PREFIX} cannot read `{rel}`: {e}"))
}

fn manifest() -> Yaml {
    serde_yaml::from_str(&read(MANIFEST))
        .unwrap_or_else(|e| panic!("{PREFIX} `{MANIFEST}` is not valid YAML: {e}"))
}

fn at<'a>(node: &'a Yaml, key: &str) -> Option<&'a Yaml> {
    node.get(key)
}

fn at_path<'a>(node: &'a Yaml, path: &str) -> Option<&'a Yaml> {
    let mut current = node;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn at_path_mut<'a>(node: &'a mut Yaml, segments: &[&str]) -> Option<&'a mut Yaml> {
    match segments.split_first() {
        Some((head, tail)) => at_path_mut(node.get_mut(*head)?, tail),
        None => Some(node),
    }
}

fn str_at<'a>(node: &'a Yaml, key: &str) -> Option<&'a str> {
    at(node, key).and_then(Yaml::as_str)
}

fn flag(node: &Yaml, key: &str) -> Option<bool> {
    at(node, key).and_then(Yaml::as_bool)
}

fn seq<'a>(node: &'a Yaml, key: &str) -> Option<&'a Vec<Yaml>> {
    at(node, key).and_then(Yaml::as_sequence)
}

/// The ordered string values of a sequence field (order matters for the
/// identity canonicalization handles).
fn strings_in_order(node: &Yaml, key: &str) -> Vec<String> {
    seq(node, key)
        .map(|values| {
            values
                .iter()
                .filter_map(Yaml::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn keys_of(node: &Yaml) -> BTreeSet<String> {
    node.as_mapping()
        .map(|map| {
            map.keys()
                .filter_map(Yaml::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn first_unknown_key(node: &Yaml, allowed: &[&str]) -> Option<String> {
    let allowed: BTreeSet<&str> = allowed.iter().copied().collect();
    node.as_mapping()?
        .keys()
        .filter_map(Yaml::as_str)
        .find(|key| !allowed.contains(key))
        .map(ToString::to_string)
}

fn put_str(doc: &mut Yaml, path: &str, value: &str) {
    let segments: Vec<&str> = path.split('.').collect();
    let target = at_path_mut(doc, &segments)
        .unwrap_or_else(|| panic!("{PREFIX} cannot resolve `{path}` for mutation"));
    *target = Yaml::String(value.to_string());
}

fn put_bool(doc: &mut Yaml, path: &str, value: bool) {
    let segments: Vec<&str> = path.split('.').collect();
    let target = at_path_mut(doc, &segments)
        .unwrap_or_else(|| panic!("{PREFIX} cannot resolve `{path}` for mutation"));
    *target = Yaml::Bool(value);
}

fn insert_key(doc: &mut Yaml, path: &str, key: &str, value: Yaml) {
    let segments: Vec<&str> = path.split('.').collect();
    let map = at_path_mut(doc, &segments)
        .and_then(Yaml::as_mapping_mut)
        .unwrap_or_else(|| panic!("{PREFIX} cannot resolve `{path}` for insertion"));
    map.insert(Yaml::String(key.to_string()), value);
}

fn remove_key(doc: &mut Yaml, path: &str, key: &str) {
    let segments: Vec<&str> = path.split('.').collect();
    let map = at_path_mut(doc, &segments)
        .and_then(Yaml::as_mapping_mut)
        .unwrap_or_else(|| panic!("{PREFIX} cannot resolve `{path}` for removal"));
    map.remove(key);
}

fn put_metric_str(doc: &mut Yaml, index: usize, key: &str, value: &str) {
    let rows = at_path_mut(
        doc,
        &["external_holdout_contract", "release_probe", "metrics"],
    )
    .and_then(Yaml::as_sequence_mut)
    .expect("release_probe.metrics");
    let row = rows
        .get_mut(index)
        .and_then(Yaml::as_mapping_mut)
        .unwrap_or_else(|| panic!("{PREFIX} metric row {index}"));
    row.insert(
        Yaml::String(key.to_string()),
        Yaml::String(value.to_string()),
    );
}

fn put_metric_node(doc: &mut Yaml, index: usize, key: &str, value: Yaml) {
    let rows = at_path_mut(
        doc,
        &["external_holdout_contract", "release_probe", "metrics"],
    )
    .and_then(Yaml::as_sequence_mut)
    .expect("release_probe.metrics");
    let row = rows
        .get_mut(index)
        .and_then(Yaml::as_mapping_mut)
        .unwrap_or_else(|| panic!("{PREFIX} metric row {index}"));
    row.insert(Yaml::String(key.to_string()), value);
}

/// External rows are `classes.<class>.rows`; a `local_proxy` block is never an
/// external row. Returns `(class, id, state)`.
fn external_rows(doc: &Yaml) -> Vec<(String, String, String)> {
    let mut rows = Vec::new();
    let Some(classes) = at(doc, "classes").and_then(Yaml::as_mapping) else {
        return rows;
    };
    for (class_name, class) in classes {
        let Some(name) = class_name.as_str() else {
            continue;
        };
        let Some(entries) = at(class, "rows").and_then(Yaml::as_sequence) else {
            continue;
        };
        for entry in entries {
            rows.push((
                name.to_string(),
                str_at(entry, "id").unwrap_or("<unnamed>").to_string(),
                str_at(entry, "state").unwrap_or("<missing>").to_string(),
            ));
        }
    }
    rows
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Recompute the probe identity digest from the declared source manifest using
/// exactly the declared canonicalization handles. The digest is never derived
/// from the already-declared value; it is always recomputed from the bytes.
fn recompute_identity_digest(lane: &Yaml) -> Result<String, String> {
    let rel = str_at(lane, "source_manifest").unwrap_or_default();
    if rel.trim().is_empty() {
        return Err("probe_identity_source_missing".into());
    }
    let Ok(text) = fs::read_to_string(root().join(rel)) else {
        return Err("probe_identity_source_missing".into());
    };
    let Ok(source) = serde_yaml::from_str::<Yaml>(&text) else {
        return Err("probe_identity_source_missing".into());
    };
    let fields = strings_in_order(lane, "digest_input_fields");
    if fields.is_empty() {
        return Err("probe_identity_source_missing".into());
    }
    let field_sep = str_at(lane, "digest_field_separator").unwrap_or_default();
    let record_sep = str_at(lane, "digest_record_separator").unwrap_or_default();
    let Some(articles) = seq(&source, "articles") else {
        return Err("probe_identity_source_missing".into());
    };
    let mut records: Vec<String> = Vec::new();
    for article in articles {
        let mut values: Vec<String> = Vec::new();
        for field in &fields {
            let Some(value) = str_at(article, field) else {
                return Err("probe_identity_source_missing".into());
            };
            values.push(value.to_string());
        }
        records.push(values.join(field_sep));
    }
    let payload = records.join(record_sep);
    Ok(format!("sha256:{}", sha256_hex(payload.as_bytes())))
}

/// How the caller claims the recorded human release decision is satisfied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeClaim<'a> {
    /// The recorded human gate is offered.
    RecordedGate,
    /// A prose note is offered instead of a recorded decision.
    ProseOnly(&'a str),
}

/// May the frozen probe result be cited? Every branch is derived from the
/// declaration itself, and a declaration that would permit an unexecuted
/// citation is refused rather than read as a switch.
fn cite_probe_result(doc: &Yaml, claim: ProbeClaim<'_>) -> Result<(), String> {
    let Some(contract) = at(doc, "external_holdout_contract") else {
        return Err("package_root_missing".into());
    };
    let Some(release) = at(doc, "release_decision_package") else {
        return Err("package_root_missing".into());
    };

    // ── lane separation: the external band must never be flipped ─────────────
    if str_at(contract, "execution_status") != Some("not_executed") {
        return Err("external_lane_state_flipped".into());
    }
    let Some(byte_policy) = at(contract, "byte_policy") else {
        return Err("external_lane_state_flipped".into());
    };
    if str_at(byte_policy, "external_bytes") != Some("bytes_absent")
        || str_at(byte_policy, "load_external_bytes") != Some("forbidden")
        || str_at(byte_policy, "download_external_bytes") != Some("forbidden")
        || str_at(byte_policy, "materialize_external_files") != Some("forbidden")
    {
        return Err("external_lane_state_flipped".into());
    }
    for (class, id, state) in external_rows(doc) {
        if state != "bytes_absent" {
            return Err("external_lane_state_flipped".into());
        }
        let _ = (class, id);
    }

    // ── the declared lane ────────────────────────────────────────────────────
    let Some(lane) = at(contract, "release_probe") else {
        return Err("probe_lane_missing".into());
    };
    if first_unknown_key(lane, &RELEASE_PROBE_KEYS).is_some() {
        return Err("probe_lane_unknown_key".into());
    }
    if str_at(lane, "schema_version") != Some("external-holdout-release-probe.v1") {
        return Err("probe_lane_schema_drift".into());
    }

    // ── frozen identity ──────────────────────────────────────────────────────
    if str_at(lane, "identity_input") != Some("canonical_dataset_id_plus_identity")
        || flag(lane, "digest_is_byte_hash") != Some(false)
        || flag(lane, "digest_is_authority_binding") != Some(true)
        || flag(lane, "frozen_before_execution") != Some(true)
        || strings_in_order(lane, "digest_input_fields") != ["article_id", "sha256"]
        || str_at(lane, "digest_input_order") != Some("as_listed")
        || str_at(lane, "digest_field_separator")
            .unwrap_or("")
            .is_empty()
        || str_at(lane, "digest_record_separator")
            .unwrap_or("")
            .is_empty()
    {
        return Err("probe_identity_not_frozen".into());
    }
    let computed = recompute_identity_digest(lane)?;
    if str_at(lane, "identity_digest") != Some(computed.as_str()) {
        return Err("probe_identity_digest_drift".into());
    }

    // ── one metric vocabulary, declared before measurement ───────────────────
    let Some(margins) = at(contract, "acceptance_margins") else {
        return Err("probe_metric_vocabulary_drift".into());
    };
    let margin_metrics: BTreeSet<String> = seq(margins, "margins")
        .map(|rows| {
            rows.iter()
                .filter_map(|row| str_at(row, "metric"))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    if str_at(lane, "margin_ref") != Some("external_holdout_contract.acceptance_margins") {
        return Err("probe_metric_margin_ref_drift".into());
    }
    let Some(metric_rows) = seq(lane, "metrics") else {
        return Err("probe_metric_vocabulary_drift".into());
    };
    let declared_metrics: Vec<String> = metric_rows
        .iter()
        .filter_map(|row| str_at(row, "metric"))
        .map(ToString::to_string)
        .collect();
    let declared_set: BTreeSet<String> = declared_metrics.iter().cloned().collect();
    if declared_metrics.len() != margin_metrics.len() || declared_set != margin_metrics {
        return Err("probe_metric_vocabulary_drift".into());
    }
    for row in metric_rows {
        for key in THRESHOLD_KEYS {
            if at(row, key).is_some() {
                return Err("probe_metric_threshold_duplicated".into());
            }
        }
        if first_unknown_key(row, &RELEASE_PROBE_METRIC_KEYS).is_some() {
            return Err("probe_lane_unknown_key".into());
        }
        if str_at(row, "numerator_definition")
            .unwrap_or("")
            .trim()
            .is_empty()
            || str_at(row, "denominator_definition")
                .unwrap_or("")
                .trim()
                .is_empty()
            || str_at(row, "aggregation") != Some("per_document")
            || str_at(row, "claim_scope") != Some("local_proxy_only")
        {
            return Err("probe_metric_definition_missing".into());
        }
        if str_at(row, "zero_admitted_population_behavior") != Some("no_denominator") {
            return Err("probe_zero_admitted_population_read_as_one".into());
        }
        if str_at(row, "margin_ref") != Some("external_holdout_contract.acceptance_margins") {
            return Err("probe_metric_margin_ref_drift".into());
        }
    }

    // ── latency / cost accounting ────────────────────────────────────────────
    let (Some(latency), Some(cost)) = (at(lane, "latency"), at(lane, "cost")) else {
        return Err("probe_accounting_missing".into());
    };
    if first_unknown_key(latency, &RELEASE_PROBE_LATENCY_KEYS).is_some()
        || first_unknown_key(cost, &RELEASE_PROBE_COST_KEYS).is_some()
    {
        return Err("probe_lane_unknown_key".into());
    }
    if str_at(latency, "pooled_latency_claim") != Some("forbidden")
        || str_at(cost, "pooled_cost_claim") != Some("forbidden")
    {
        return Err("probe_pooled_claim_leak".into());
    }
    if str_at(latency, "unit").unwrap_or("").trim().is_empty()
        || str_at(latency, "measurement_source")
            .unwrap_or("")
            .trim()
            .is_empty()
        || str_at(cost, "cost_status") != Some("bounded_not_measured")
        || flag(cost, "unknown_is_not_zero") != Some(true)
    {
        return Err("probe_accounting_missing".into());
    }

    // ── fail-closed citation ─────────────────────────────────────────────────
    if flag(lane, "citation_requires_recorded_human_release_decision") != Some(true)
        || str_at(lane, "citation_without_release_decision") != Some("forbidden")
    {
        return Err("citation_without_release_decision".into());
    }
    match claim {
        ProbeClaim::ProseOnly(_) => return Err("prose_alone_authorizes_citation".into()),
        ProbeClaim::RecordedGate => {}
    }
    let Some(human) = at(release, "human_release_review") else {
        return Err("citation_without_release_decision".into());
    };
    if !RECORDED_DECISION_STATES.contains(&str_at(human, "decision").unwrap_or_default()) {
        return Err("citation_without_release_decision".into());
    }
    if str_at(human, "reviewer").is_none_or(|reviewer| reviewer.trim().is_empty()) {
        return Err("citation_without_release_decision".into());
    }
    Ok(())
}

fn cleared_citation_document() -> Yaml {
    let mut cleared = manifest();
    put_str(
        &mut cleared,
        "release_decision_package.human_release_review.decision",
        "recorded",
    );
    put_str(
        &mut cleared,
        "release_decision_package.human_release_review.reviewer",
        "named-reviewer",
    );
    cleared
}

// ──────────────────────────────── tests ──────────────────────────────────

#[test]
fn release_probe_lane_is_declared_with_a_closed_key_set() {
    let doc = manifest();
    let lane = at_path(&doc, "external_holdout_contract.release_probe")
        .unwrap_or_else(|| panic!("{PREFIX} `{MANIFEST}` must declare the release_probe lane"));
    assert_eq!(
        str_at(lane, "schema_version"),
        Some("external-holdout-release-probe.v1")
    );
    assert_eq!(
        keys_of(lane),
        RELEASE_PROBE_KEYS
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<String>>(),
        "{PREFIX} the release_probe lane key set must be exactly the declared closed set"
    );
    assert_eq!(
        str_at(lane, "source_manifest"),
        Some(SOURCE_MANIFEST),
        "{PREFIX} the lane must pin the frozen source manifest"
    );
    assert!(
        root().join(SOURCE_MANIFEST).is_file(),
        "{PREFIX} the frozen source manifest `{SOURCE_MANIFEST}` must exist on disk"
    );
    assert_eq!(
        strings_in_order(lane, "digest_input_fields"),
        vec!["article_id".to_string(), "sha256".to_string()]
    );
    assert_eq!(str_at(lane, "digest_input_order"), Some("as_listed"));
    assert!(
        !str_at(lane, "digest_field_separator")
            .unwrap_or("")
            .is_empty()
            && !str_at(lane, "digest_record_separator")
                .unwrap_or("")
                .is_empty(),
        "{PREFIX} the canonicalization separators must be declared DATA"
    );

    // Nested key sets are closed too.
    let metrics = seq(lane, "metrics").expect("release_probe.metrics");
    assert!(!metrics.is_empty());
    for row in metrics {
        assert_eq!(
            keys_of(row),
            RELEASE_PROBE_METRIC_KEYS
                .iter()
                .map(ToString::to_string)
                .collect::<BTreeSet<String>>(),
            "{PREFIX} metric rows must carry exactly the declared definition keys"
        );
    }
    let latency = at(lane, "latency").expect("release_probe.latency");
    assert_eq!(
        keys_of(latency),
        RELEASE_PROBE_LATENCY_KEYS
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<String>>()
    );
    let cost = at(lane, "cost").expect("release_probe.cost");
    assert_eq!(
        keys_of(cost),
        RELEASE_PROBE_COST_KEYS
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<String>>()
    );
}

#[test]
fn external_band_stays_bytes_absent_and_a_lane_flip_is_refused() {
    let doc = manifest();
    let contract = at(&doc, "external_holdout_contract").expect("external_holdout_contract");
    assert_eq!(str_at(contract, "execution_status"), Some("not_executed"));
    let byte_policy = at(contract, "byte_policy").expect("byte_policy");
    assert_eq!(str_at(byte_policy, "external_bytes"), Some("bytes_absent"));
    assert_eq!(
        str_at(byte_policy, "load_external_bytes"),
        Some("forbidden")
    );
    assert_eq!(
        str_at(byte_policy, "download_external_bytes"),
        Some("forbidden")
    );
    assert_eq!(
        str_at(byte_policy, "materialize_external_files"),
        Some("forbidden")
    );

    let rows = external_rows(&doc);
    assert!(!rows.is_empty(), "{PREFIX} external rows must be declared");
    for (class, id, state) in &rows {
        assert_eq!(
            state, "bytes_absent",
            "{PREFIX} external row `{class}/{id}` must stay bytes_absent"
        );
    }

    // The lane is declared and the M510 pins are intact, so the oracle is
    // reached and refuses the pending citation instead of tripping the lock.
    assert_eq!(
        cite_probe_result(&doc, ProbeClaim::RecordedGate),
        Err("citation_without_release_decision".to_string()),
        "{PREFIX} a pending human decision must refuse the probe citation"
    );

    let mut executed_lane = doc.clone();
    put_str(
        &mut executed_lane,
        "external_holdout_contract.execution_status",
        "executed",
    );
    assert_eq!(
        cite_probe_result(&executed_lane, ProbeClaim::RecordedGate),
        Err("external_lane_state_flipped".to_string())
    );

    let mut loaded_bytes = doc.clone();
    put_str(
        &mut loaded_bytes,
        "external_holdout_contract.byte_policy.external_bytes",
        "bytes_present",
    );
    assert_eq!(
        cite_probe_result(&loaded_bytes, ProbeClaim::RecordedGate),
        Err("external_lane_state_flipped".to_string())
    );

    let mut download = doc.clone();
    put_str(
        &mut download,
        "external_holdout_contract.byte_policy.download_external_bytes",
        "allowed",
    );
    assert_eq!(
        cite_probe_result(&download, ProbeClaim::RecordedGate),
        Err("external_lane_state_flipped".to_string())
    );

    let mut executed_row = doc.clone();
    {
        let classes = at_path_mut(&mut executed_row, &["classes"])
            .and_then(Yaml::as_mapping_mut)
            .expect("classes");
        for (_name, class) in classes.iter_mut() {
            if let Some(rows) = class.get_mut("rows").and_then(Yaml::as_sequence_mut)
                && let Some(first) = rows.first_mut().and_then(Yaml::as_mapping_mut)
            {
                first.insert(
                    Yaml::String("state".into()),
                    Yaml::String("executed".into()),
                );
                break;
            }
        }
    }
    assert_eq!(
        cite_probe_result(&executed_row, ProbeClaim::RecordedGate),
        Err("external_lane_state_flipped".to_string())
    );
}

#[test]
fn frozen_identity_digest_is_recomputed_from_the_declared_handles() {
    let doc = manifest();
    let lane = at_path(&doc, "external_holdout_contract.release_probe").expect("release_probe");
    let computed = recompute_identity_digest(lane).expect("recompute identity digest");
    assert_eq!(
        str_at(lane, "identity_digest"),
        Some(computed.as_str()),
        "{PREFIX} the declared identity_digest must equal the recomputed digest"
    );

    let mut drifted = doc.clone();
    put_str(
        &mut drifted,
        "external_holdout_contract.release_probe.identity_digest",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_eq!(
        cite_probe_result(&drifted, ProbeClaim::RecordedGate),
        Err("probe_identity_digest_drift".to_string())
    );

    let mut unfrozen = doc.clone();
    put_bool(
        &mut unfrozen,
        "external_holdout_contract.release_probe.frozen_before_execution",
        false,
    );
    assert_eq!(
        cite_probe_result(&unfrozen, ProbeClaim::RecordedGate),
        Err("probe_identity_not_frozen".to_string())
    );

    let mut byte_hash = doc.clone();
    put_bool(
        &mut byte_hash,
        "external_holdout_contract.release_probe.digest_is_byte_hash",
        true,
    );
    assert_eq!(
        cite_probe_result(&byte_hash, ProbeClaim::RecordedGate),
        Err("probe_identity_not_frozen".to_string())
    );
}

#[test]
fn metric_definitions_share_one_vocabulary_with_acceptance_margins() {
    let doc = manifest();
    let lane = at_path(&doc, "external_holdout_contract.release_probe").expect("release_probe");
    let margins =
        at_path(&doc, "external_holdout_contract.acceptance_margins").expect("acceptance_margins");
    let margin_metrics: BTreeSet<String> = seq(margins, "margins")
        .expect("margins.margins")
        .iter()
        .filter_map(|row| str_at(row, "metric"))
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        margin_metrics,
        REQUIRED_METRICS
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<String>>(),
        "{PREFIX} the acceptance margins must still declare the two probe metrics"
    );
    let declared: BTreeSet<String> = seq(lane, "metrics")
        .expect("release_probe.metrics")
        .iter()
        .filter_map(|row| str_at(row, "metric"))
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        declared, margin_metrics,
        "{PREFIX} the probe metric vocabulary must be the acceptance_margins vocabulary"
    );

    let mut renamed = doc.clone();
    put_metric_str(&mut renamed, 0, "metric", "agreement_ratio");
    assert_eq!(
        cite_probe_result(&renamed, ProbeClaim::RecordedGate),
        Err("probe_metric_vocabulary_drift".to_string())
    );

    let mut vacuous = doc.clone();
    put_metric_str(
        &mut vacuous,
        0,
        "zero_admitted_population_behavior",
        "one_point_zero",
    );
    assert_eq!(
        cite_probe_result(&vacuous, ProbeClaim::RecordedGate),
        Err("probe_zero_admitted_population_read_as_one".to_string())
    );

    let mut duplicated_threshold = doc.clone();
    put_metric_node(&mut duplicated_threshold, 0, "margin", Yaml::from(0.5_f64));
    assert_eq!(
        cite_probe_result(&duplicated_threshold, ProbeClaim::RecordedGate),
        Err("probe_metric_threshold_duplicated".to_string())
    );
}

#[test]
fn citation_is_fail_closed_until_a_recorded_human_decision_exists() {
    let doc = manifest();
    assert_eq!(
        cite_probe_result(&doc, ProbeClaim::RecordedGate),
        Err("citation_without_release_decision".to_string())
    );
    assert_eq!(
        cite_probe_result(&doc, ProbeClaim::ProseOnly("the review note says approved")),
        Err("prose_alone_authorizes_citation".to_string())
    );

    let mut unbound = doc.clone();
    put_bool(
        &mut unbound,
        "external_holdout_contract.release_probe.citation_requires_recorded_human_release_decision",
        false,
    );
    assert_eq!(
        cite_probe_result(&unbound, ProbeClaim::RecordedGate),
        Err("citation_without_release_decision".to_string())
    );

    // Positive control: a cleared in-memory clone IS citable, and prose on that
    // same clone is still refused.
    let cleared = cleared_citation_document();
    assert!(
        cite_probe_result(&cleared, ProbeClaim::RecordedGate).is_ok(),
        "{PREFIX} the citation oracle must not refuse everything"
    );
    assert_eq!(
        cite_probe_result(&cleared, ProbeClaim::ProseOnly("approved")),
        Err("prose_alone_authorizes_citation".to_string()),
        "{PREFIX} prose must never authorize even with the gate clear"
    );
}

#[test]
fn negative_matrix_catches_every_named_lane_mutation() {
    let doc = manifest();

    let mut no_lane = doc.clone();
    remove_key(&mut no_lane, "external_holdout_contract", "release_probe");

    let mut unknown_key = doc.clone();
    insert_key(
        &mut unknown_key,
        "external_holdout_contract.release_probe",
        "lane_extra",
        Yaml::Bool(true),
    );

    let mut digest_drift = doc.clone();
    put_str(
        &mut digest_drift,
        "external_holdout_contract.release_probe.identity_digest",
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    );

    let mut not_frozen = doc.clone();
    put_bool(
        &mut not_frozen,
        "external_holdout_contract.release_probe.frozen_before_execution",
        false,
    );

    let mut metric_rename = doc.clone();
    put_metric_str(&mut metric_rename, 0, "metric", "document_agreement");

    let mut zero_denominator = doc.clone();
    put_metric_str(
        &mut zero_denominator,
        1,
        "zero_admitted_population_behavior",
        "one",
    );

    let mut lane_flip = doc.clone();
    put_str(
        &mut lane_flip,
        "external_holdout_contract.execution_status",
        "executed",
    );

    let mut band_flip = doc.clone();
    put_str(
        &mut band_flip,
        "external_holdout_contract.byte_policy.external_bytes",
        "bytes_present",
    );

    let mut latency_claim = doc.clone();
    put_str(
        &mut latency_claim,
        "external_holdout_contract.release_probe.latency.pooled_latency_claim",
        "allowed",
    );

    let mut cost_claim = doc.clone();
    put_str(
        &mut cost_claim,
        "external_holdout_contract.release_probe.cost.pooled_cost_claim",
        "allowed",
    );

    let mut cost_measured = doc.clone();
    put_str(
        &mut cost_measured,
        "external_holdout_contract.release_probe.cost.cost_status",
        "measured",
    );

    let mut threshold_duplicated = doc.clone();
    put_metric_node(&mut threshold_duplicated, 1, "margin", Yaml::from(0.25_f64));

    let cases: Vec<(&str, Yaml, &str)> = vec![
        ("lane removal", no_lane, "probe_lane_missing"),
        ("unknown lane key", unknown_key, "probe_lane_unknown_key"),
        (
            "identity digest drift",
            digest_drift,
            "probe_identity_digest_drift",
        ),
        (
            "identity not frozen",
            not_frozen,
            "probe_identity_not_frozen",
        ),
        (
            "metric rename",
            metric_rename,
            "probe_metric_vocabulary_drift",
        ),
        (
            "zero denominator read as one",
            zero_denominator,
            "probe_zero_admitted_population_read_as_one",
        ),
        (
            "external lane flip",
            lane_flip,
            "external_lane_state_flipped",
        ),
        (
            "external band flip",
            band_flip,
            "external_lane_state_flipped",
        ),
        (
            "pooled latency claim",
            latency_claim,
            "probe_pooled_claim_leak",
        ),
        ("pooled cost claim", cost_claim, "probe_pooled_claim_leak"),
        (
            "cost read as measured",
            cost_measured,
            "probe_accounting_missing",
        ),
        (
            "duplicated threshold",
            threshold_duplicated,
            "probe_metric_threshold_duplicated",
        ),
    ];
    assert!(
        cases.len() >= 8,
        "{PREFIX} the negative matrix must have >= 8 cases"
    );

    let mut survivors: Vec<String> = Vec::new();
    for (name, mutated, expected) in &cases {
        match cite_probe_result(mutated, ProbeClaim::RecordedGate) {
            Err(label) if label == *expected => {}
            other => survivors.push(format!("{name}: expected `{expected}`, got {other:?}")),
        }
    }
    assert!(
        survivors.is_empty(),
        "{PREFIX} negative matrix survivors (must be 0):\n{}",
        survivors.join("\n")
    );

    // The unmutated live document is itself a case: a pending human decision
    // must refuse the citation even though the lane is complete.
    assert_eq!(
        cite_probe_result(&doc, ProbeClaim::RecordedGate),
        Err("citation_without_release_decision".to_string())
    );
}
