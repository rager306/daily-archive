//! M516-th0oby / S12 / T06 — the doc contract for the human-facing
//! release-readiness decision list.
//!
//! WHY THIS TEST EXISTS: the demo says the readiness receipt lists exactly what a
//! human must decide — but a human reads prose, not JSON. The document a human
//! reads is `doc/review/M516-S12-release-readiness.md`, and the authority for
//! every number in it is the machine-readable receipt
//! `artifacts/parse_compat/grobid_retirement_release_readiness.json`.
//!
//! A prose package rots silently: a sentence can be edited to say a gate passed
//! while the receipt still carries `blocked`, or to claim the machine authorized a
//! release, and no reviewer recomputes the values by hand. This target makes the
//! document a *checked projection* of the receipt instead of a restatement:
//!
//! * [`committed_document_is_a_clean_projection`] asserts the document exists,
//!   carries the four required section headings, references both the receipt and
//!   the decision record, records the machine authorization as none by carrying
//!   the literal `s09_authorized=false`, and is linked from `doc/review/README.md`.
//! * The marker-delimited projection block is compared to the receipt by exact
//!   identity (closed key set): the envelope, the three machine-authorization
//!   flags, the five locks and the three gate rows. A re-typed value is refused by
//!   name, so a gate cannot silently become `met`.
//! * Every gate evidence binding (`path` + `pointer` + `sha256`) the receipt
//!   declares must appear verbatim in the document, so a dropped or stale binding
//!   fails.
//! * A small forbidden-token set keeps the document from drifting into an
//!   authorization claim ("the package prepares the decision, it does not make
//!   it").
//!
//! [`mutated_documents_are_refused_by_name`] clones the committed document string
//! in memory, mutates one thing, and asserts the named refusal, so the contract
//! cannot pass vacuously. Nothing here writes to disk: the document is a
//! projection of the receipt, never the other way round.
//!
//! Run:
//!
//! ```text
//! cargo test -p da-cli --test release_readiness_receipt_doc_contract
//! cargo test -p da-cli --test release_readiness_receipt_doc_contract -- --nocapture
//! ```

use da_cli::release_readiness_receipt::{
    DEFAULT_RELEASE_READINESS_ARTIFACT_REL, ReleaseReadinessReceipt,
};
use serde_json::{Map, Value};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// The human-facing document under contract.
const DOC: &str = "doc/review/M516-S12-release-readiness.md";
/// Its file name, which the review index must carry to make it reachable.
const DOC_BASENAME: &str = "M516-S12-release-readiness.md";
/// The review index the document must be linked from.
const README: &str = "doc/review/README.md";
/// The decision record the human fills in.
const DECISION_RECORD: &str = "doc/review/HUMAN-REVIEW-TEMPLATE.md";

/// The four sections the document must carry, verbatim.
const HEADING_PENDING_DECISIONS: &str = "## 1. Что решает человек — pending human decisions";
const HEADING_THREE_GATES: &str = "## 2. Три гейта — quality, cost, rights";
const HEADING_MACHINE_NONE: &str =
    "## 3. Машина не авторизует ничего — machine authorization = none";
const HEADING_DECISION_RECORD: &str = "## 4. Запись решения — decision record";

/// The markers around the checked projection block.
const PROJECTION_BEGIN: &str = "<!-- readiness-projection:begin -->";
const PROJECTION_END: &str = "<!-- readiness-projection:end -->";

/// The literal that records the machine authorization as none. The document must
/// carry it verbatim; a document that dropped it could claim a release.
const MACHINE_AUTHORIZATION_TOKEN: &str = "s09_authorized=false";

/// Closed key set of the document's projection block.
const PROJECTION_KEYS: [&str; 9] = [
    "schema_version",
    "claim_scope",
    "thresholds",
    "product_coverage_effect",
    "s09_authorized",
    "gsd_lifecycle_authorizes",
    "human_release_recorded",
    "locks",
    "gates",
];

/// Closed key set of a projection gate row.
const GATE_KEYS: [&str; 3] = ["gate_id", "status", "release_effect"];

/// Tokens a document may not carry: each would claim a decision the human has not
/// made.
const FORBIDDEN_TOKENS: [&str; 3] = [
    "release authorized",
    "decision is recorded",
    "s09_authorized=true",
];

// ───────────────────────────── helpers ─────────────────────────────

/// Resolve the repository root the way the sibling da-cli contract tests do.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the crate lives two levels under the repository root")
}

fn read_text(root: &Path, rel: &str) -> Result<String, String> {
    fs::read_to_string(root.join(rel)).map_err(|e| format!("cannot read `{rel}`: {e}"))
}

fn committed_receipt(root: &Path) -> Result<ReleaseReadinessReceipt, String> {
    let bytes = fs::read(root.join(DEFAULT_RELEASE_READINESS_ARTIFACT_REL))
        .map_err(|e| format!("cannot read `{DEFAULT_RELEASE_READINESS_ARTIFACT_REL}`: {e}"))?;
    ReleaseReadinessReceipt::parse_json_bytes(&bytes).map_err(|e| e.to_string())
}

/// The region between two markers, or a named refusal if either is absent.
fn region<'a>(text: &'a str, begin: &str, end: &str) -> Result<&'a str, String> {
    let start = text
        .find(begin)
        .ok_or_else(|| format!("document_marker_missing: {begin}"))?
        + begin.len();
    let stop = text[start..]
        .find(end)
        .ok_or_else(|| format!("document_marker_missing: {end}"))?
        + start;
    Ok(&text[start..stop])
}

/// Parse the single JSON value enclosed by the projection markers. The document
/// wraps it in a fenced code block, so the region is trimmed to the first opening
/// bracket and the last closing bracket; anything else is a named refusal.
fn parse_projection(text: &str) -> Result<Value, String> {
    let body = region(text, PROJECTION_BEGIN, PROJECTION_END)?;
    let open = body
        .find(['{', '['])
        .ok_or_else(|| "document_projection_has_no_json".to_string())?;
    let close = body
        .rfind(['}', ']'])
        .ok_or_else(|| "document_projection_has_no_json".to_string())?;
    if close < open {
        return Err("document_projection_brackets_out_of_order".to_string());
    }
    serde_json::from_str(&body[open..=close])
        .map_err(|e| format!("document_projection_json_does_not_parse: {e}"))
}

// ─────────────────────────── projection checks ───────────────────────────

/// Compare the document's projection block to the receipt as an exact, closed-key
/// projection.
fn check_projection(
    projection: &Value,
    receipt: &ReleaseReadinessReceipt,
    errors: &mut Vec<String>,
) {
    let Some(map) = projection.as_object() else {
        errors.push("document_projection_not_an_object".into());
        return;
    };
    for key in map.keys() {
        if !PROJECTION_KEYS.contains(&key.as_str()) {
            errors.push(format!("document_projection_key_unknown: key={key}"));
        }
    }
    for key in PROJECTION_KEYS {
        if !map.contains_key(key) {
            errors.push(format!("document_projection_key_missing: key={key}"));
        }
    }

    // The envelope is the preparation-only projection.
    let envelope: [(&str, &str); 4] = [
        ("schema_version", receipt.schema_version.as_str()),
        ("claim_scope", receipt.claim_scope.as_str()),
        ("thresholds", receipt.thresholds.as_str()),
        (
            "product_coverage_effect",
            receipt.product_coverage_effect.as_str(),
        ),
    ];
    for (key, expected) in envelope {
        match map.get(key).and_then(Value::as_str) {
            Some(actual) if actual == expected => {}
            Some(_) => errors.push(format!("document_projection_envelope_drift: key={key}")),
            None => errors.push(format!("document_projection_key_missing: key={key}")),
        }
    }

    // The machine authorizes nothing.
    let flags: [(&str, bool); 3] = [
        (
            "s09_authorized",
            receipt.machine_authorization.s09_authorized,
        ),
        (
            "gsd_lifecycle_authorizes",
            receipt.machine_authorization.gsd_lifecycle_authorizes,
        ),
        (
            "human_release_recorded",
            receipt.machine_authorization.human_release_recorded,
        ),
    ];
    for (key, expected) in flags {
        match map.get(key).and_then(Value::as_bool) {
            Some(actual) if actual == expected => {}
            Some(_) => errors.push(format!(
                "document_projection_authorization_drift: key={key}"
            )),
            None => errors.push(format!("document_projection_key_missing: key={key}")),
        }
    }

    // The five locks stay exactly as the receipt declares them.
    let expected_locks =
        serde_json::to_value(&receipt.machine_authorization.locks).unwrap_or(Value::Null);
    match map.get("locks") {
        Some(actual) if *actual == expected_locks => {}
        Some(_) => errors.push("document_projection_lock_drift".into()),
        None => errors.push("document_projection_key_missing: key=locks".into()),
    }

    // The three gate rows, in canonical order.
    let expected_gates: Vec<Value> = receipt
        .release_gates
        .iter()
        .map(|gate| {
            let mut row = Map::new();
            row.insert("gate_id".into(), Value::String(gate.gate_id.clone()));
            row.insert("status".into(), Value::String(gate.status.clone()));
            row.insert(
                "release_effect".into(),
                Value::String(gate.release_effect.clone()),
            );
            Value::Object(row)
        })
        .collect();
    match map.get("gates").and_then(Value::as_array) {
        Some(actual) => {
            if actual.len() != expected_gates.len() {
                errors.push(format!(
                    "document_projection_gate_count_drift: expected={} actual={}",
                    expected_gates.len(),
                    actual.len()
                ));
            }
            for (index, expected) in expected_gates.iter().enumerate() {
                let Some(actual) = actual.get(index) else {
                    continue;
                };
                let Some(actual_map) = actual.as_object() else {
                    errors.push(format!(
                        "document_projection_gate_not_an_object: index={index}"
                    ));
                    continue;
                };
                for key in actual_map.keys() {
                    if !GATE_KEYS.contains(&key.as_str()) {
                        errors.push(format!(
                            "document_projection_gate_key_unknown: index={index} key={key}"
                        ));
                    }
                }
                for key in GATE_KEYS {
                    match (actual_map.get(key), expected.get(key)) {
                        (Some(a), Some(e)) if a == e => {}
                        (Some(_), Some(_)) => errors.push(format!(
                            "document_projection_gate_drift: index={index} key={key}"
                        )),
                        (None, Some(_)) => errors.push(format!(
                            "document_projection_gate_key_missing: index={index} key={key}"
                        )),
                        (_, None) => {}
                    }
                }
            }
        }
        None => errors.push("document_projection_key_missing: key=gates".into()),
    }
}

/// The full document contract. Empty result means the document is a clean
/// projection of the committed receipt.
fn validate_document(text: &str, receipt: &ReleaseReadinessReceipt, readme: &str) -> Vec<String> {
    let mut errors = Vec::new();

    // The four required sections.
    for heading in [
        HEADING_PENDING_DECISIONS,
        HEADING_THREE_GATES,
        HEADING_MACHINE_NONE,
        HEADING_DECISION_RECORD,
    ] {
        if !text.contains(heading) {
            errors.push(format!("document_required_heading_missing: {heading}"));
        }
    }

    // Both authority pointers: the receipt and the decision record.
    for reference in [DEFAULT_RELEASE_READINESS_ARTIFACT_REL, DECISION_RECORD] {
        if !text.contains(reference) {
            errors.push(format!("document_required_reference_missing: {reference}"));
        }
    }

    // The machine authorization is recorded as none.
    if !text.contains(MACHINE_AUTHORIZATION_TOKEN) {
        errors.push(format!(
            "document_machine_authorization_token_missing: {MACHINE_AUTHORIZATION_TOKEN}"
        ));
    }

    // The document is reachable from the review index.
    if !readme.contains(DOC_BASENAME) {
        errors.push(format!("document_not_linked_from_readme: {DOC_BASENAME}"));
    }

    // The projection block, compared to the receipt by exact identity.
    match parse_projection(text) {
        Ok(projection) => check_projection(&projection, receipt, &mut errors),
        Err(error) => errors.push(error),
    }

    // Every gate evidence binding the receipt declares appears verbatim.
    for gate in &receipt.release_gates {
        for binding in &gate.evidence {
            if !text.contains(&binding.manifest) {
                errors.push(format!(
                    "document_gate_evidence_missing: {} manifest={}",
                    gate.gate_id, binding.manifest
                ));
            }
            if !text.contains(&binding.pointer) {
                errors.push(format!(
                    "document_gate_evidence_missing: {} pointer={}",
                    gate.gate_id, binding.pointer
                ));
            }
            if !text.contains(&binding.revision_sha256) {
                errors.push(format!(
                    "document_gate_evidence_missing: {} sha256={}",
                    gate.gate_id, binding.revision_sha256
                ));
            }
        }
    }

    // The document must not claim a decision the human has not made.
    for token in FORBIDDEN_TOKENS {
        if text.contains(token) {
            errors.push(format!("document_forbidden_token_present: {token}"));
        }
    }

    errors
}

// ─────────────────────────────── tests ─────────────────────────────────

/// Positive control: the committed document is a clean projection of the receipt,
/// carries the required sections and pointers, and is linked from the index.
#[test]
fn committed_document_is_a_clean_projection() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let text = read_text(&root, DOC)?;
    let readme = read_text(&root, README)?;
    let receipt = committed_receipt(&root)?;

    let errors = validate_document(&text, &receipt, &readme);
    assert!(
        errors.is_empty(),
        "document drift:\n- {}",
        errors.join("\n- ")
    );

    // Non-vacuity: the projection block really carries the three gates and the
    // five locks, not an empty shell.
    let projection = parse_projection(&text).expect("projection parses");
    assert_eq!(
        projection
            .get("gates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(receipt.release_gates.len()),
        "the projection must carry exactly the receipt's gates"
    );
    assert_eq!(
        projection
            .get("locks")
            .and_then(Value::as_object)
            .map(Map::len),
        Some(receipt.machine_authorization.locks.len()),
        "the projection must carry exactly the receipt's locks"
    );

    Ok(())
}

/// The document must be reachable from the review index.
#[test]
fn document_is_linked_from_the_review_index() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let readme = read_text(&root, README)?;
    assert!(
        readme.contains(DOC_BASENAME),
        "`{README}` must link `{DOC_BASENAME}`"
    );
    Ok(())
}

/// Negative: each in-memory mutation of the committed document is refused by
/// name. The positive control above proves a zero here would be a real zero.
#[test]
fn mutated_documents_are_refused_by_name() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let committed = read_text(&root, DOC)?;
    let readme = read_text(&root, README)?;
    let receipt = committed_receipt(&root)?;

    // A well-formed digest that is not the receipt's binding, for the evidence
    // mutation.
    let bound_sha = receipt.release_gates[0].evidence[0].revision_sha256.clone();
    let mut stale_sha = bound_sha.clone();
    stale_sha.pop();
    stale_sha.push(if bound_sha.ends_with('0') { '1' } else { '0' });

    let cases: Vec<(&str, String, String, String)> = vec![
        (
            "dropped pending-decisions heading",
            HEADING_PENDING_DECISIONS.to_owned(),
            "## 1. Решения".to_owned(),
            format!("document_required_heading_missing: {HEADING_PENDING_DECISIONS}"),
        ),
        (
            "dropped machine-authorization heading",
            HEADING_MACHINE_NONE.to_owned(),
            "## 3. Замки".to_owned(),
            format!("document_required_heading_missing: {HEADING_MACHINE_NONE}"),
        ),
        (
            "dropped receipt reference",
            DEFAULT_RELEASE_READINESS_ARTIFACT_REL.to_owned(),
            "artifacts/parse_compat/other.json".to_owned(),
            format!(
                "document_required_reference_missing: {DEFAULT_RELEASE_READINESS_ARTIFACT_REL}"
            ),
        ),
        (
            "dropped decision-record reference",
            DECISION_RECORD.to_owned(),
            "doc/review/OTHER-TEMPLATE.md".to_owned(),
            format!("document_required_reference_missing: {DECISION_RECORD}"),
        ),
        (
            "machine authorization token",
            MACHINE_AUTHORIZATION_TOKEN.to_owned(),
            "s09_authorized=true".to_owned(),
            format!("document_machine_authorization_token_missing: {MACHINE_AUTHORIZATION_TOKEN}"),
        ),
        (
            "projection marker",
            PROJECTION_BEGIN.to_owned(),
            String::new(),
            format!("document_marker_missing: {PROJECTION_BEGIN}"),
        ),
        (
            "projection gate status",
            "\"status\": \"blocked\"".to_owned(),
            "\"status\": \"met\"".to_owned(),
            "document_projection_gate_drift: index=0 key=status".to_owned(),
        ),
        (
            "projection lock value",
            "\"import_eligible\": \"false\"".to_owned(),
            "\"import_eligible\": \"true\"".to_owned(),
            "document_projection_lock_drift".to_owned(),
        ),
        (
            "projection claim scope",
            "\"claim_scope\": \"release_readiness_preparation_only\"".to_owned(),
            "\"claim_scope\": \"release_authorized\"".to_owned(),
            "document_projection_envelope_drift: key=claim_scope".to_owned(),
        ),
        (
            "projection authorization flag",
            "\"s09_authorized\": false".to_owned(),
            "\"s09_authorized\": true".to_owned(),
            "document_projection_authorization_drift: key=s09_authorized".to_owned(),
        ),
        (
            "gate evidence sha256",
            bound_sha,
            stale_sha,
            "document_gate_evidence_missing: quality sha256=".to_owned(),
        ),
    ];

    for (label, old, new, needle) in cases {
        let mutated = committed.replace(&old, &new);
        assert_ne!(
            mutated, committed,
            "mutation `{label}` did not change the document"
        );
        let errors = validate_document(&mutated, &receipt, &readme);
        assert!(
            errors.iter().any(|error| error.contains(&needle)),
            "mutation `{label}` must be refused by `{needle}`; got {errors:?}"
        );
    }

    // Dropping the index link must be refused even when the document is clean.
    let no_link = readme.replace(DOC_BASENAME, "some-other-doc.md");
    assert_ne!(
        no_link, readme,
        "the index link mutation must change the index"
    );
    let errors = validate_document(&committed, &receipt, &no_link);
    assert!(
        errors.iter().any(
            |error| error.contains(&format!("document_not_linked_from_readme: {DOC_BASENAME}"))
        ),
        "an unlinked document must be refused; got {errors:?}"
    );

    Ok(())
}

/// Negative: a whole section removal drops its heading and must be refused.
#[test]
fn section_removal_is_refused() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let committed = read_text(&root, DOC)?;
    let readme = read_text(&root, README)?;
    let receipt = committed_receipt(&root)?;

    let start = committed
        .find(HEADING_DECISION_RECORD)
        .expect("the decision-record section is present");
    let truncated = committed[..start].to_owned();
    let errors = validate_document(&truncated, &receipt, &readme);
    assert!(
        errors.iter().any(|error| error.contains(&format!(
            "document_required_heading_missing: {HEADING_DECISION_RECORD}"
        ))),
        "a removed section must be refused by its missing heading; got {errors:?}"
    );
    Ok(())
}
