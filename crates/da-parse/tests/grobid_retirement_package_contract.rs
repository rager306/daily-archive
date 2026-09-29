//! Grobid-retirement release-decision package — hermetic committed-bytes contract
//! (M513-r7dg4z / S07 / T01).
//!
//! The package (`artifacts/parse_compat/grobid_retirement_release_decision_package.json`)
//! is an **index of bindings**, not a second dictionary of measurements: it points
//! at the already-measured parity, ledger, ceiling and rollback carriers and never
//! restates a number. A package of that shape decays silently — a carrier can be
//! replaced while the index text stays put — so this target judges the committed
//! bytes against the carriers themselves:
//!
//!   * every `source_bindings[].path` is re-hashed **from disk** and compared to the
//!     declared digest, so a substituted carrier is refused by path;
//!   * the human record is a **pointer to authority** (`acceptance_manifest.yaml`),
//!     and the contract re-reads that authority and compares the declared values to
//!     the live ones, so editing the declaration alone cannot clear the gate;
//!   * every `remainders[].pointer` is resolved to a real node and the `state`
//!     token is checked against that node, so a remainder cannot be "closed" by
//!     editing the index;
//!   * GAR-015 must still be `open` and H20 `pending`/`unassigned`/`false`/`false`,
//!     so no release decision is fabricated;
//!   * the rehearsal carrier must carry `claim_scope: rehearsal_only`, so a
//!     rehearsal is never read as measured parity.
//!
//! The target imports no `da-parse` symbol and decodes no typed model: it reads the
//! committed documents with `serde_json` / `serde_yaml` and hashes with `sha2`. It
//! is deliberately independent of the Governor Python checks (ADR-051: an oracle
//! shares no code with the surface it checks).
//!
//! Negative mutations clone the parsed package in-process; each must fail **by
//! name**, and a positive control proves the validator does not refuse everything.
//!
//! Run:
//!
//! ```text
//! cargo test -p da-parse --test grobid_retirement_package_contract
//! cargo test -p da-parse --test grobid_retirement_package_contract -- --nocapture
//! ```

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const PREFIX: &str = "[grobid-retirement-package]";

const PACKAGE: &str = "artifacts/parse_compat/grobid_retirement_release_decision_package.json";
const MANIFEST: &str = "data/parse/acceptance_manifest.yaml";
const REGISTER: &str = "doc/review/grobid-absorption-review-2026-09-10.jsonl";
const FOLLOWUP: &str = "doc/review/grobid-absorption-followup.json";
const REHEARSAL: &str = "artifacts/parse_compat/native_family_promotion_rehearsal.json";

const PACKAGE_IDENTITY: &str = "grobid-retirement-release-decision-package.v1";
const MANIFEST_ANCHOR: &str = "data/parse/acceptance_manifest.yaml#release_decision_package";
const HUMAN_RECORD_FORM: &str = "doc/review/HUMAN-REVIEW-TEMPLATE.md";
const FINDING_ID: &str = "GAR-015";

/// Closed top-level key set. An extra key is refused by name (deny-unknown).
const TOP_LEVEL_KEYS: [&str; 14] = [
    "schema_version",
    "purpose",
    "package_status",
    "source_bindings",
    "human_record",
    "remainders",
    "locks",
    "claim_scope",
    "pooled_quality_claim",
    "rehearsal_claim_scope",
    "citation_requires_recorded_human_release_decision",
    "nothing_here_authorizes_a_release",
    "this_wave_promotes",
    "explicit_decision",
];

/// Closed `locks` key set — test-local constants, never a governor policy read.
const LOCK_KEYS: [&str; 6] = [
    "import_eligible",
    "deployed",
    "production_route",
    "graph_write",
    "auto_promote",
    "promotion_receipt",
];

const HUMAN_RECORD_KEYS: [&str; 6] = [
    "authority",
    "authority_sha256",
    "record_form",
    "declared",
    "authorization_requires",
    "prohibited_authorizations",
];

const DECLARED_KEYS: [&str; 4] = [
    "package_status",
    "reviewer",
    "decision",
    "prose_alone_authorizes_release",
];

const BINDING_KEYS: [&str; 4] = ["role", "path", "sha256", "identity"];

const AUTHORIZATION_REQUIRES: [&str; 5] = [
    "named_reviewer",
    "explicit_decision",
    "passing_quality_gate",
    "passing_cost_gate",
    "passing_rights_gate",
];

const PROHIBITED_ACTIONS: [&str; 5] = [
    "redistribution",
    "production_use",
    "model_distribution",
    "external_downloads",
    "graph_write_or_import",
];

/// `(role, path, identity)` for the six evidence groups, exactly.
const SOURCE_ROLES: [(&str, &str, &str); 6] = [
    (
        "parity_measurement",
        "artifacts/parse_compat/external_holdout_release_probe.json",
        "external-holdout-release-probe.v1",
    ),
    (
        "parity_readable_report",
        "doc/review/grobid-parity-report.md",
        "grobid-parity-report.md",
    ),
    (
        "parity_ledger_and_remainders",
        "data/parse/grobid_compatibility.yaml",
        "grobid-compatibility.v1",
    ),
    (
        "resource_ceilings",
        "artifacts/parse_compat/parse_batch_budget_live.json",
        "parse-batch-budget-live.v1",
    ),
    (
        "rollback_rehearsal",
        "artifacts/parse_compat/native_family_promotion_rehearsal.json",
        "native-family-promotion-rehearsal.v1",
    ),
    (
        "rollback_release_readiness",
        "doc/review/grobid-rollback-readiness.md",
        "grobid-rollback-readiness.md",
    ),
];

const REMAINDER_IDS: [&str; 6] = [
    "human_release_decision",
    "named_gap_learning_ruvllm_ablation",
    "horizon_native_learning_order",
    "absent_capability_dispositions",
    "h20_frozen_external_holdout_decision",
    "h21_revision_label_residual",
];

// ─────────────────────────────── loading ───────────────────────────────

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root above crates/da-parse")
        .to_path_buf()
}

fn read_text(root: &Path, rel: &str) -> Result<String, String> {
    fs::read_to_string(root.join(rel)).map_err(|e| format!("{PREFIX} cannot read `{rel}`: {e}"))
}

/// Read a committed document into an untyped JSON node. JSON files go through
/// `serde_json`; YAML files go through `serde_yaml` and land in the same node
/// shape, so every check below is shared.
fn read_document(root: &Path, rel: &str) -> Result<Value, String> {
    let text = read_text(root, rel)?;
    if rel.ends_with(".json") || rel.ends_with(".jsonl") {
        serde_json::from_str(&text).map_err(|e| format!("{PREFIX} `{rel}` is not valid JSON: {e}"))
    } else {
        serde_yaml::from_str(&text).map_err(|e| format!("{PREFIX} `{rel}` is not valid YAML: {e}"))
    }
}

fn sha256_file(root: &Path, rel: &str) -> Result<String, String> {
    let bytes =
        fs::read(root.join(rel)).map_err(|e| format!("{PREFIX} cannot hash `{rel}`: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

// ───────────────────────────── node helpers ────────────────────────────

fn str_field<'a>(node: &'a Value, key: &str) -> Option<&'a str> {
    node.get(key).and_then(Value::as_str)
}

fn bool_field(node: &Value, key: &str) -> Option<bool> {
    node.get(key).and_then(Value::as_bool)
}

fn string_set(node: &Value, key: &str) -> BTreeSet<String> {
    node.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn require_keys(
    node: &Value,
    allowed: &[&str],
    required: &[&str],
    label: &str,
) -> Result<(), String> {
    let map = node
        .as_object()
        .ok_or_else(|| format!("{label} is not an object"))?;
    for key in required {
        if !map.contains_key(*key) {
            return Err(format!("missing required key: {key}"));
        }
    }
    let allowed: BTreeSet<&str> = allowed.iter().copied().collect();
    for key in map.keys() {
        if !allowed.contains(key.as_str()) {
            return Err(format!("unknown key: {key}"));
        }
    }
    Ok(())
}

fn require_str_is(node: &Value, key: &str, expected: &str, token: &str) -> Result<(), String> {
    if str_field(node, key) == Some(expected) {
        Ok(())
    } else {
        Err(token.to_string())
    }
}

fn require_bool_is(node: &Value, key: &str, expected: bool, token: &str) -> Result<(), String> {
    if bool_field(node, key) == Some(expected) {
        Ok(())
    } else {
        Err(token.to_string())
    }
}

// ─────────────────────────── package validators ─────────────────────────

/// (a) closed top-level key set: every required key present, no unknown key.
fn check_closed_keys(pkg: &Value) -> Result<(), String> {
    require_keys(pkg, &TOP_LEVEL_KEYS, &TOP_LEVEL_KEYS, "package root")
}

/// (i) mandatory scope tokens carry their declared values.
fn check_scope_tokens(pkg: &Value) -> Result<(), String> {
    require_str_is(
        pkg,
        "schema_version",
        PACKAGE_IDENTITY,
        "schema_version is not grobid-retirement-release-decision-package.v1",
    )?;
    require_str_is(
        pkg,
        "purpose",
        "human_release_decision_package_index_only",
        "purpose is not human_release_decision_package_index_only",
    )?;
    require_str_is(
        pkg,
        "package_status",
        "pending_human_review",
        "package_status is not pending_human_review",
    )?;
    require_str_is(
        pkg,
        "claim_scope",
        "release_decision_package_preparation_only",
        "claim_scope is not release_decision_package_preparation_only",
    )?;
    require_str_is(
        pkg,
        "pooled_quality_claim",
        "forbidden",
        "pooled_quality_claim is not forbidden",
    )?;
    require_str_is(
        pkg,
        "rehearsal_claim_scope",
        "rehearsal_only",
        "rehearsal_claim_scope is not rehearsal_only",
    )?;
    require_bool_is(
        pkg,
        "citation_requires_recorded_human_release_decision",
        true,
        "citation_requires_recorded_human_release_decision is not true",
    )?;
    require_bool_is(
        pkg,
        "nothing_here_authorizes_a_release",
        true,
        "nothing_here_authorizes_a_release is not true",
    )?;
    require_str_is(
        pkg,
        "this_wave_promotes",
        "none",
        "this_wave_promotes is not none",
    )?;
    match pkg.get("explicit_decision") {
        Some(value) if value.is_null() => Ok(()),
        _ => Err("explicit_decision is not null".to_string()),
    }
}

/// (h) the six locks are present, exact, and hold their declared values.
fn check_locks(pkg: &Value) -> Result<(), String> {
    let locks = pkg.get("locks").ok_or("locks is missing")?;
    require_keys(locks, &LOCK_KEYS, &LOCK_KEYS, "locks")?;
    require_bool_is(
        locks,
        "import_eligible",
        false,
        "import_eligible is not false",
    )?;
    require_bool_is(locks, "deployed", false, "deployed is not false")?;
    require_str_is(
        locks,
        "production_route",
        "forbidden",
        "production_route is not forbidden",
    )?;
    require_str_is(
        locks,
        "graph_write",
        "forbidden",
        "graph_write is not forbidden",
    )?;
    require_str_is(
        locks,
        "auto_promote",
        "forbidden",
        "auto_promote is not forbidden",
    )?;
    require_str_is(
        locks,
        "promotion_receipt",
        "forbidden",
        "promotion_receipt is not forbidden",
    )?;
    Ok(())
}

/// (b) every bound carrier is re-hashed from disk and must match its declaration.
fn check_source_bindings(pkg: &Value, root: &Path) -> Result<(), String> {
    let bindings = pkg
        .get("source_bindings")
        .and_then(Value::as_array)
        .ok_or("source_bindings is not an array")?;
    if bindings.len() != SOURCE_ROLES.len() {
        return Err(format!(
            "source_bindings count is not {}: {}",
            SOURCE_ROLES.len(),
            bindings.len()
        ));
    }
    let mut seen_roles = BTreeSet::new();
    for binding in bindings {
        require_keys(binding, &BINDING_KEYS, &BINDING_KEYS, "source binding")?;
        let role = str_field(binding, "role").ok_or("source binding role is missing")?;
        let path = str_field(binding, "path").ok_or("source binding path is missing")?;
        let declared = str_field(binding, "sha256").ok_or("source binding sha256 is missing")?;
        let identity =
            str_field(binding, "identity").ok_or("source binding identity is missing")?;
        if identity.trim().is_empty() {
            return Err(format!("source binding identity is empty: {path}"));
        }
        seen_roles.insert(role.to_string());
        let actual = sha256_file(root, path).map_err(|_| format!("substituted source: {path}"))?;
        if actual != declared {
            return Err(format!("substituted source: {path}"));
        }
    }
    let expected: BTreeSet<String> = SOURCE_ROLES
        .iter()
        .map(|(role, _, _)| role.to_string())
        .collect();
    if seen_roles != expected {
        return Err("source_bindings role set is not the declared six".into());
    }
    Ok(())
}

/// (c) the human record is a pointer to authority: the committed manifest is
/// re-read and the declared values are compared to the live ones, so editing the
/// declaration alone cannot clear a gate.
fn check_human_record(pkg: &Value, manifest: &Value, root: &Path) -> Result<(), String> {
    let record = pkg.get("human_record").ok_or("human_record is missing")?;
    require_keys(
        record,
        &HUMAN_RECORD_KEYS,
        &HUMAN_RECORD_KEYS,
        "human_record",
    )?;
    require_str_is(
        record,
        "authority",
        MANIFEST_ANCHOR,
        "human_record.authority is not data/parse/acceptance_manifest.yaml#release_decision_package",
    )?;
    let declared_sha =
        str_field(record, "authority_sha256").ok_or("human_record.authority_sha256 is missing")?;
    let actual_sha = sha256_file(root, MANIFEST)?;
    if declared_sha != actual_sha {
        return Err(format!("authority sha256 substituted: {MANIFEST}"));
    }
    require_str_is(
        record,
        "record_form",
        HUMAN_RECORD_FORM,
        "human_record.record_form is not doc/review/HUMAN-REVIEW-TEMPLATE.md",
    )?;
    let record_form =
        str_field(record, "record_form").ok_or("human_record.record_form is missing")?;
    if !root.join(record_form).is_file() {
        return Err(format!("human_record form is not on disk: {record_form}"));
    }

    let declared = record
        .get("declared")
        .ok_or("human_record.declared is missing")?;
    require_keys(
        declared,
        &DECLARED_KEYS,
        &DECLARED_KEYS,
        "human_record.declared",
    )?;

    let release = manifest
        .get("release_decision_package")
        .ok_or("manifest release_decision_package is missing")?;
    require_str_is(
        release,
        "package_status",
        "pending_human_review",
        "package_status is not pending_human_review",
    )?;
    require_str_is(
        declared,
        "package_status",
        "pending_human_review",
        "declared package_status is not pending_human_review",
    )?;

    let live_review = release
        .get("human_release_review")
        .ok_or("manifest human_release_review is missing")?;
    if !live_review
        .get("reviewer")
        .map(Value::is_null)
        .unwrap_or(false)
    {
        return Err("reviewer is not null".into());
    }
    if !declared
        .get("reviewer")
        .map(Value::is_null)
        .unwrap_or(false)
    {
        return Err("reviewer is not null".into());
    }
    if str_field(live_review, "decision") != Some("pending") {
        return Err("decision is not pending".into());
    }
    if str_field(declared, "decision") != Some("pending") {
        return Err("decision is not pending".into());
    }
    if bool_field(live_review, "prose_alone_authorizes_release") != Some(false) {
        return Err("prose_alone_authorizes_release is not false".into());
    }
    if bool_field(declared, "prose_alone_authorizes_release") != Some(false) {
        return Err("prose_alone_authorizes_release is not false".into());
    }

    // Live gates and prohibitions: quality blocked, cost bounded_not_measured,
    // rights blocked, all five prohibited authorizations blocked.
    if release.pointer("/quality/status").and_then(Value::as_str) != Some("blocked") {
        return Err("quality gate is not blocked".into());
    }
    if release.pointer("/cost/status").and_then(Value::as_str) != Some("bounded_not_measured") {
        return Err("cost gate is not bounded_not_measured".into());
    }
    if release.pointer("/rights/status").and_then(Value::as_str) != Some("blocked") {
        return Err("rights gate is not blocked".into());
    }
    let required_gates: BTreeSet<String> = string_set(release, "required_gates");
    let expected_gates: BTreeSet<String> = ["quality", "cost", "rights"]
        .iter()
        .map(ToString::to_string)
        .collect();
    if required_gates != expected_gates {
        return Err("required_gates is not quality/cost/rights".into());
    }
    let prohibited = release
        .get("prohibited_authorizations")
        .and_then(Value::as_object)
        .ok_or("manifest prohibited_authorizations is missing")?;
    if prohibited.len() != PROHIBITED_ACTIONS.len()
        || PROHIBITED_ACTIONS
            .iter()
            .any(|action| prohibited.get(*action).and_then(Value::as_str) != Some("blocked"))
    {
        return Err("prohibited_authorizations is not five blocked keys".into());
    }

    let declared_gates = string_set(record, "authorization_requires");
    let expected_gates: BTreeSet<String> = AUTHORIZATION_REQUIRES
        .iter()
        .map(ToString::to_string)
        .collect();
    if declared_gates != expected_gates {
        return Err("authorization_requires is not the five declared gates".into());
    }
    let declared_prohibited = string_set(record, "prohibited_authorizations");
    let expected_prohibited: BTreeSet<String> =
        PROHIBITED_ACTIONS.iter().map(ToString::to_string).collect();
    if declared_prohibited != expected_prohibited {
        return Err("prohibited_authorizations is not the five declared prohibitions".into());
    }
    Ok(())
}

/// (f) the rehearsal is never read as measured parity: the rehearsal carrier and
/// the package both declare `rehearsal_only`, and the pooled claim is forbidden.
fn check_rehearsal_scope(pkg: &Value, rehearsal: &Value) -> Result<(), String> {
    require_str_is(
        rehearsal,
        "claim_scope",
        "rehearsal_only",
        "rehearsal artifact claim_scope is not rehearsal_only",
    )?;
    require_str_is(
        rehearsal,
        "pooled_quality_claim",
        "forbidden",
        "rehearsal artifact pooled_quality_claim is not forbidden",
    )?;
    require_str_is(
        pkg,
        "rehearsal_claim_scope",
        "rehearsal_only",
        "rehearsal_claim_scope is not rehearsal_only",
    )?;
    require_str_is(
        pkg,
        "pooled_quality_claim",
        "forbidden",
        "pooled_quality_claim is not forbidden",
    )
}

/// Resolve a `file#a.b.c` pointer against the committed tree. A sequence segment
/// selects the item whose `stage_id`/`id`/`name`/`capability` equals the segment.
fn resolve_anchor<'a>(node: &'a Value, anchor: &str) -> Option<&'a Value> {
    let mut current = node;
    for segment in anchor.split('.') {
        current = match current {
            Value::Object(map) => map.get(segment)?,
            Value::Array(items) => items
                .iter()
                .find(|item| identifier_matches(item, segment))?,
            _ => return None,
        };
    }
    Some(current)
}

fn identifier_matches(item: &Value, segment: &str) -> bool {
    ["stage_id", "id", "name", "capability"]
        .iter()
        .any(|key| item.get(*key).and_then(Value::as_str) == Some(segment))
}

fn resolve_remainder(root: &Path, pointer: &str) -> Result<(Value, String), String> {
    let (file, anchor) = match pointer.split_once('#') {
        Some((file, anchor)) => (file, Some(anchor)),
        None => (pointer, None),
    };
    let document = read_document(root, file)?;
    let node = match anchor {
        None => document,
        Some(anchor) => resolve_anchor(&document, anchor)
            .ok_or_else(|| format!("remainder pointer unresolved: {pointer}"))?
            .clone(),
    };
    Ok((node, file.to_string()))
}

/// (g) every remainder resolves and its `state` token agrees with the carrier.
fn check_remainders(pkg: &Value, root: &Path) -> Result<(), String> {
    let bindings: BTreeMap<String, String> = pkg
        .get("source_bindings")
        .and_then(Value::as_array)
        .map(|bindings| {
            bindings
                .iter()
                .filter_map(|binding| {
                    Some((
                        str_field(binding, "path")?.to_string(),
                        str_field(binding, "sha256")?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let remainders = pkg
        .get("remainders")
        .and_then(Value::as_array)
        .ok_or("remainders is not an array")?;
    let mut seen = BTreeSet::new();
    for remainder in remainders {
        require_keys(
            remainder,
            &["id", "state", "pointer", "decision_ref"],
            &["id", "state", "pointer"],
            "remainder",
        )?;
        let id = str_field(remainder, "id")
            .ok_or("remainder id is missing")?
            .to_string();
        seen.insert(id.clone());
        let pointer = str_field(remainder, "pointer")
            .ok_or("remainder pointer is missing")?
            .to_string();
        let file = pointer
            .split_once('#')
            .map(|(file, _)| file)
            .unwrap_or(pointer.as_str())
            .to_string();
        if let Some(expected) = bindings.get(&file) {
            let actual = sha256_file(root, &file)?;
            if &actual != expected {
                return Err(format!("remainder source drifted: {file}"));
            }
        }
        let (node, _) = resolve_remainder(root, &pointer)?;
        check_remainder_state(&id, remainder, &node)?;
    }
    let expected: BTreeSet<String> = REMAINDER_IDS.iter().map(ToString::to_string).collect();
    if seen != expected {
        return Err("remainders id set is not the declared six".into());
    }
    Ok(())
}

fn check_remainder_state(id: &str, remainder: &Value, node: &Value) -> Result<(), String> {
    match id {
        "human_release_decision" => {
            if str_field(remainder, "state") != Some("pending") {
                return Err("human_release_decision state is not pending".into());
            }
            if !node.get("reviewer").map(Value::is_null).unwrap_or(false) {
                return Err("human_release_decision reviewer is not null".into());
            }
            if str_field(node, "decision") != Some("pending") {
                return Err("human_release_decision decision is not pending".into());
            }
            Ok(())
        }
        "named_gap_learning_ruvllm_ablation" => {
            if str_field(remainder, "state") != Some("typed_refusal") {
                return Err("named_gap_learning_ruvllm_ablation state is not typed_refusal".into());
            }
            let gaps = node
                .as_array()
                .ok_or("named_gap_learning_ruvllm_ablation pointer is not the named_gaps list")?;
            let recorded = gaps.iter().any(|gap| {
                str_field(gap, "capability") == Some("learning.ruvllm_ablation")
                    && str_field(gap, "reason") == Some("no_java_donor")
            });
            if !recorded {
                return Err(
                    "named_gap_learning_ruvllm_ablation is not in parity_ledger.named_gaps".into(),
                );
            }
            Ok(())
        }
        "horizon_native_learning_order" => {
            if str_field(remainder, "state") != Some("in_horizon_project_native") {
                return Err(
                    "horizon_native_learning_order state is not in_horizon_project_native".into(),
                );
            }
            let order = node
                .as_array()
                .ok_or("horizon_native_learning_order pointer is not horizon_order")?;
            if order.is_empty()
                || !order
                    .iter()
                    .any(|entry| entry.as_str() == Some("learning.ruvllm_ablation"))
            {
                return Err("horizon_order is empty or does not carry the named gap".into());
            }
            Ok(())
        }
        "absent_capability_dispositions" => {
            if str_field(remainder, "state") != Some("recorded") {
                return Err("absent_capability_dispositions state is not recorded".into());
            }
            let dispositions = node
                .as_object()
                .ok_or("absent_capability_dispositions pointer is not a mapping")?;
            if dispositions.is_empty() {
                return Err("absent_capability_dispositions is empty".into());
            }
            for (capability, entry) in dispositions {
                if entry.get("disposition").and_then(Value::as_str).is_none() {
                    return Err(format!(
                        "absent disposition `{capability}` has no disposition"
                    ));
                }
            }
            Ok(())
        }
        "h20_frozen_external_holdout_decision" => {
            if str_field(remainder, "state") != Some("pending") {
                return Err("h20 state is not pending".into());
            }
            if str_field(node, "status") != Some("pending") {
                return Err("H20 status is not pending".into());
            }
            if str_field(node, "owner") != Some("unassigned") {
                return Err("H20 owner is not unassigned".into());
            }
            if bool_field(node, "implementation_pass") != Some(false) {
                return Err("H20 implementation_pass is not false".into());
            }
            if bool_field(node, "verification_pass") != Some(false) {
                return Err("H20 verification_pass is not false".into());
            }
            Ok(())
        }
        "h21_revision_label_residual" => {
            if str_field(remainder, "state") != Some("accepted_residual") {
                return Err("h21 state is not accepted_residual".into());
            }
            if node.as_str() != Some("H21") {
                return Err("label_divergence_item does not resolve to H21".into());
            }
            Ok(())
        }
        other => Err(format!("unknown remainder id: {other}")),
    }
}

/// (d) GAR-015 must still be `open`.
fn check_finding_open(register_text: &str) -> Result<(), String> {
    for line in register_text.lines().filter(|line| !line.trim().is_empty()) {
        let record: Value = serde_json::from_str(line)
            .map_err(|e| format!("{PREFIX} `{REGISTER}` is not valid JSONL: {e}"))?;
        if str_field(&record, "finding_id") == Some(FINDING_ID) {
            return require_str_is(&record, "status", "open", "GAR-015 status is not open");
        }
    }
    Err(format!(
        "{PREFIX} `{REGISTER}` must contain a {FINDING_ID} record"
    ))
}

/// (e) H20 must be `pending` / `unassigned` / `false` / `false`.
fn check_h20(followup: &Value) -> Result<(), String> {
    let stages = followup
        .get("stages")
        .and_then(Value::as_array)
        .ok_or("stages is not an array")?;
    let h20 = stages
        .iter()
        .find(|stage| str_field(stage, "stage_id") == Some("H20"))
        .ok_or("H20 stage is missing")?;
    require_str_is(h20, "status", "pending", "H20 status is not pending")?;
    require_str_is(h20, "owner", "unassigned", "H20 owner is not unassigned")?;
    require_bool_is(
        h20,
        "implementation_pass",
        false,
        "H20 implementation_pass is not false",
    )?;
    require_bool_is(
        h20,
        "verification_pass",
        false,
        "H20 verification_pass is not false",
    )?;
    Ok(())
}

fn validate_all(
    pkg: &Value,
    manifest: &Value,
    register_text: &str,
    followup: &Value,
    rehearsal: &Value,
    root: &Path,
) -> Result<(), String> {
    check_closed_keys(pkg)?;
    check_scope_tokens(pkg)?;
    check_locks(pkg)?;
    check_source_bindings(pkg, root)?;
    check_human_record(pkg, manifest, root)?;
    check_rehearsal_scope(pkg, rehearsal)?;
    check_remainders(pkg, root)?;
    check_finding_open(register_text)?;
    check_h20(followup)?;
    Ok(())
}

/// Human-readable parse-out of the index (visible with `--nocapture`).
fn print_breakdown(pkg: &Value) {
    println!("{PREFIX} release-decision package index");
    println!(
        "  schema_version : {}",
        str_field(pkg, "schema_version").unwrap_or("<missing>")
    );
    println!(
        "  package_status : {}",
        str_field(pkg, "package_status").unwrap_or("<missing>")
    );
    println!(
        "  claim_scope    : {}",
        str_field(pkg, "claim_scope").unwrap_or("<missing>")
    );
    println!("  source_bindings:");
    if let Some(bindings) = pkg.get("source_bindings").and_then(Value::as_array) {
        for binding in bindings {
            let role = str_field(binding, "role").unwrap_or("<missing>");
            let path = str_field(binding, "path").unwrap_or("<missing>");
            let sha = str_field(binding, "sha256").unwrap_or("<missing>");
            let short = &sha[..sha.len().min(12)];
            println!("    - {role}: {path} sha256={short}");
        }
    }
    println!("  locks:");
    if let Some(locks) = pkg.get("locks").and_then(Value::as_object) {
        for (key, value) in locks {
            println!("    - {key} = {value}");
        }
    }
    println!("  remainders:");
    if let Some(remainders) = pkg.get("remainders").and_then(Value::as_array) {
        for remainder in remainders {
            println!(
                "    - {} [{}] -> {}",
                str_field(remainder, "id").unwrap_or("<missing>"),
                str_field(remainder, "state").unwrap_or("<missing>"),
                str_field(remainder, "pointer").unwrap_or("<missing>")
            );
        }
    }
}

// ──────────────────────────────── tests ────────────────────────────────

#[test]
fn package_indexes_committed_bytes_and_declares_pending_human_review() {
    let root = root();
    let pkg = read_document(&root, PACKAGE).expect("package readable");
    let manifest = read_document(&root, MANIFEST).expect("manifest readable");
    let followup = read_document(&root, FOLLOWUP).expect("followup readable");
    let rehearsal = read_document(&root, REHEARSAL).expect("rehearsal readable");
    let register = read_text(&root, REGISTER).expect("register readable");

    print_breakdown(&pkg);
    validate_all(&pkg, &manifest, &register, &followup, &rehearsal, &root)
        .unwrap_or_else(|error| panic!("{PREFIX} committed package refused: {error}"));
}

// Eight inputs: the six validator inputs plus the expected refusal and the
// mutation label. Bundling them into a struct would only move the verbosity
// onto the ten call sites without removing it, so the clippy argument-count
// threshold is waived for this thin assertion wrapper.
#[allow(clippy::too_many_arguments)]
fn assert_refused(
    pkg: &Value,
    manifest: &Value,
    register: &str,
    followup: &Value,
    rehearsal: &Value,
    root: &Path,
    expected: &str,
    label: &str,
) {
    match validate_all(pkg, manifest, register, followup, rehearsal, root) {
        Ok(()) => panic!("{PREFIX} {label}: the validator accepted a mutated package"),
        Err(error) => assert_eq!(error, expected, "{PREFIX} {label}: wrong refusal"),
    }
}

#[test]
fn mutated_packages_are_refused_by_name() {
    let root = root();
    let pkg = read_document(&root, PACKAGE).expect("package readable");
    let manifest = read_document(&root, MANIFEST).expect("manifest readable");
    let followup = read_document(&root, FOLLOWUP).expect("followup readable");
    let rehearsal = read_document(&root, REHEARSAL).expect("rehearsal readable");
    let register = read_text(&root, REGISTER).expect("register readable");

    // Positive control: the validator discriminates instead of refusing all input.
    assert!(
        validate_all(&pkg, &manifest, &register, &followup, &rehearsal, &root).is_ok(),
        "{PREFIX} the positive control must validate"
    );

    // 1. A reviewer recorded in `declared` instead of `null`.
    let mut mutated = pkg.clone();
    *mutated
        .pointer_mut("/human_record/declared/reviewer")
        .expect("declared.reviewer") = Value::String("someone".into());
    assert_refused(
        &mutated,
        &manifest,
        &register,
        &followup,
        &rehearsal,
        &root,
        "reviewer is not null",
        "declared reviewer",
    );

    // 2. A recorded decision instead of `pending`.
    let mut mutated = pkg.clone();
    *mutated
        .pointer_mut("/human_record/declared/decision")
        .expect("declared.decision") = Value::String("approved".into());
    assert_refused(
        &mutated,
        &manifest,
        &register,
        &followup,
        &rehearsal,
        &root,
        "decision is not pending",
        "declared decision",
    );

    // 3. A source digest shifted by one hex character.
    let mut mutated = pkg.clone();
    {
        let node = mutated
            .pointer_mut("/source_bindings/0/sha256")
            .expect("binding sha256");
        let original = node.as_str().expect("binding sha256 string");
        let mut chars: Vec<char> = original.chars().collect();
        chars[0] = if chars[0] == 'a' { 'b' } else { 'a' };
        *node = Value::String(chars.into_iter().collect());
    }
    assert_refused(
        &mutated,
        &manifest,
        &register,
        &followup,
        &rehearsal,
        &root,
        "substituted source: artifacts/parse_compat/external_holdout_release_probe.json",
        "shifted source sha",
    );

    // 4. A removed enforcement key on the top level.
    let mut mutated = pkg.clone();
    mutated
        .as_object_mut()
        .expect("package object")
        .remove("pooled_quality_claim");
    assert_refused(
        &mutated,
        &manifest,
        &register,
        &followup,
        &rehearsal,
        &root,
        "missing required key: pooled_quality_claim",
        "removed enforcement key",
    );

    // 5. `pooled_quality_claim` relaxed away from `forbidden`.
    let mut mutated = pkg.clone();
    *mutated
        .pointer_mut("/pooled_quality_claim")
        .expect("pooled_quality_claim") = Value::String("allowed".into());
    assert_refused(
        &mutated,
        &manifest,
        &register,
        &followup,
        &rehearsal,
        &root,
        "pooled_quality_claim is not forbidden",
        "relaxed pooled claim",
    );

    // 6. A rehearsal scope elevated to production.
    let mut mutated = pkg.clone();
    *mutated
        .pointer_mut("/rehearsal_claim_scope")
        .expect("rehearsal_claim_scope") = Value::String("production".into());
    assert_refused(
        &mutated,
        &manifest,
        &register,
        &followup,
        &rehearsal,
        &root,
        "rehearsal_claim_scope is not rehearsal_only",
        "elevated rehearsal scope",
    );

    // 7. The citation gate flipped to false.
    let mut mutated = pkg.clone();
    *mutated
        .pointer_mut("/citation_requires_recorded_human_release_decision")
        .expect("citation flag") = Value::Bool(false);
    assert_refused(
        &mutated,
        &manifest,
        &register,
        &followup,
        &rehearsal,
        &root,
        "citation_requires_recorded_human_release_decision is not true",
        "disabled citation gate",
    );

    // 8. An unknown top-level key (deny-unknown).
    let mut mutated = pkg.clone();
    mutated
        .as_object_mut()
        .expect("package object")
        .insert("rogue_key".into(), Value::String("x".into()));
    assert_refused(
        &mutated,
        &manifest,
        &register,
        &followup,
        &rehearsal,
        &root,
        "unknown key: rogue_key",
        "unknown top-level key",
    );

    // 9. The live authority diverges from the declaration — the contract must
    // re-read the manifest, not only the package's `declared` copy.
    let mut live = manifest.clone();
    *live
        .pointer_mut("/release_decision_package/human_release_review/reviewer")
        .expect("live reviewer") = Value::String("someone".into());
    assert_refused(
        &pkg,
        &live,
        &register,
        &followup,
        &rehearsal,
        &root,
        "reviewer is not null",
        "live authority reviewer",
    );

    let mut live = manifest.clone();
    *live
        .pointer_mut("/release_decision_package/human_release_review/decision")
        .expect("live decision") = Value::String("approved".into());
    assert_refused(
        &pkg,
        &live,
        &register,
        &followup,
        &rehearsal,
        &root,
        "decision is not pending",
        "live authority decision",
    );
}
