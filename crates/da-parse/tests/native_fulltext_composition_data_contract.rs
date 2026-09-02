//! S18/T01 DATA admission contract.
//! The fixture is intentionally parsed as generic YAML so this gate remains
//! independent from the future composer implementation.
use serde_yaml::{Mapping, Value};
use std::path::PathBuf;

const AUTHORITY: &str = "data/parse/native_fulltext_composition.yaml";
const COMPARISON: &str = "data/parse/comparison_contract.yaml";
const LEDGER: &str = "data/parse/api_compatibility_ledger.yaml";
const ROLLBACK: &str = "java-grobid-http@0.9.0-crf";
const COMPONENTS: [&str; 7] = [
    "header",
    "references",
    "document_segmentation",
    "figure",
    "table",
    "formula",
    "learning_policy",
];
const STATES: [&str; 7] = [
    "passed",
    "fallback",
    "rejected",
    "unsupported",
    "not_observed",
    "invalid_evidence",
    "stale",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn yaml(path: &str) -> Value {
    serde_yaml::from_slice(&std::fs::read(root().join(path)).unwrap()).unwrap()
}
fn text(v: &Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(str::to_owned)
}
fn seq(v: &Value, path: &[&str]) -> Vec<String> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key).unwrap();
    }
    cur.as_sequence()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}
fn authority_errors(v: &Value) -> Vec<String> {
    let mut e = Vec::new();
    let required = [
        "schema_version",
        "adr_ref",
        "authority_id",
        "component_ids",
        "dispositions",
        "strata",
        "admission",
        "source_evidence",
        "component_receipts",
        "rollback_target",
        "redaction",
    ];
    for k in required {
        if v.get(k).is_none() {
            e.push(format!("missing:{k}"));
        }
    }
    if text(v, &["schema_version"]).as_deref() != Some("native-fulltext-composition.v1") {
        e.push("schema drift".into());
    }
    for (k, expected) in [
        ("observation_mode", "shadow"),
        ("promotion_state", "candidate"),
        ("auto_promote", "forbidden"),
        ("production_route", "forbidden"),
        ("graph_mutation", "forbidden"),
    ] {
        if text(v, &[k]).as_deref() != Some(expected) {
            e.push(format!("policy:{k}"));
        }
    }
    if v.get("import_eligible").and_then(Value::as_bool) != Some(false) {
        e.push("policy:import".into());
    }
    if text(v, &["rollback_target"]).as_deref() != Some(ROLLBACK) {
        e.push("rollback".into());
    }
    if seq(v, &["component_ids"]) != COMPONENTS {
        e.push("component ids".into());
    }
    if seq(v, &["dispositions"]) != STATES {
        e.push("disposition vocabulary".into());
    }
    if v.get("strata")
        .and_then(|x| x.get("closed"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        e.push("strata open".into());
    }
    let req = seq(v, &["admission", "native_substitution_requires"]);
    if req
        != [
            "exact_source_identity",
            "exact_schema_version",
            "exact_artifact_sha256",
            "passing_receipt",
        ]
    {
        e.push("admission identity binding".into());
    }
    if v.get("admission")
        .and_then(|x| x.get("terminal_receipt_is_pass"))
        .and_then(Value::as_bool)
        != Some(false)
    {
        e.push("terminal-as-pass".into());
    }
    if text(v, &["admission", "pooled_winner"]).as_deref() != Some("forbidden") {
        e.push("pooling".into());
    }
    if text(v, &["admission", "fallback_target"]).as_deref() != Some(ROLLBACK) {
        e.push("fallback".into());
    }
    let matrix = v.get("admission").and_then(|x| x.get("current_matrix"));
    for (k, expected) in [
        ("header", "passed"),
        ("references", "passed"),
        ("document_segmentation", "fallback"),
        ("figure", "fallback"),
        ("table", "fallback"),
        ("formula", "fallback"),
        ("learning_policy", "not_observed"),
    ] {
        if matrix.and_then(|x| x.get(k)).and_then(Value::as_str) != Some(expected) {
            e.push(format!("matrix:{k}"));
        }
    }
    for (k, producer, disposition, evidence, schema) in [
        (
            "header",
            "S12",
            "passed",
            "passing",
            "native-composition.outcome/v1",
        ),
        (
            "references",
            "S12",
            "passed",
            "passing",
            "native-composition.outcome/v1",
        ),
        (
            "document_segmentation",
            "S15",
            "fallback",
            "invalid_evidence",
            "native-segmentation.v1",
        ),
        (
            "figure",
            "S16",
            "fallback",
            "unsupported",
            "native-visual-heads.v1",
        ),
        (
            "table",
            "S16",
            "fallback",
            "not_observed",
            "native-visual-heads.v1",
        ),
        (
            "formula",
            "S16",
            "fallback",
            "unsupported",
            "native-visual-heads.v1",
        ),
        (
            "learning_policy",
            "S17",
            "not_observed",
            "not_observed",
            "candidate-lineage.v1",
        ),
    ] {
        let r = v.get("component_receipts").and_then(|x| x.get(k));
        if r.and_then(|x| x.get("producer")).and_then(Value::as_str) != Some(producer) {
            e.push(format!("receipt producer:{k}"));
        }
        if r.and_then(|x| x.get("disposition")).and_then(Value::as_str) != Some(disposition) {
            e.push(format!("receipt disposition:{k}"));
        }
        if r.and_then(|x| x.get("evidence_state"))
            .and_then(Value::as_str)
            != Some(evidence)
        {
            e.push(format!("receipt evidence:{k}"));
        }
        if r.and_then(|x| x.get("schema_version"))
            .and_then(Value::as_str)
            != Some(schema)
        {
            e.push(format!("receipt schema:{k}"));
        }
        if r.and_then(|x| x.get("source_hash")).and_then(Value::as_str) != Some("required_sha256") {
            e.push(format!("receipt source hash:{k}"));
        }
        if r.and_then(|x| x.get("artifact_sha256"))
            .and_then(Value::as_str)
            != Some("required_sha256")
        {
            e.push(format!("receipt artifact hash:{k}"));
        }
        for field in [
            "source_hash",
            "artifact_sha256",
            "schema_version",
            "fallback_target",
        ] {
            if r.and_then(|x| x.get(field)).is_none() {
                e.push(format!("receipt:{k}:{field}"));
            }
        }
    }
    let d = v
        .get("source_evidence")
        .and_then(|x| x.get("s12_named_denominators"))
        .unwrap();
    for (k, n) in [
        ("date_total", 173),
        ("header_raw_total", 36),
        ("header_accepted_total", 35),
        ("header_rejected_or_incomplete", 1),
        ("citation_names_total", 2198),
        ("affiliation_recognition_total", 13),
        ("affiliation_linking_total", 13),
        ("references_total", 207),
        ("anchors_total", 380),
        ("anchors_targeted", 217),
        ("anchors_targetless", 163),
        ("anchors_fragmented_adjacency", 168),
    ] {
        if d.get(k).and_then(Value::as_i64) != Some(n) {
            e.push(format!("denominator:{k}"));
        }
    }
    for (section, expected) in [
        ("s15_shared_intersection", "immutable_s15_manifest"),
        ("s16_shared_intersection", "immutable_s16_manifest"),
        ("s17_lineage", "immutable_s17_candidate_lineage"),
    ] {
        if text(v, &["source_evidence", section, "source"]).as_deref() != Some(expected) {
            e.push(format!("source:{section}"));
        }
    }
    if v.get("source_evidence")
        .and_then(|x| x.get("s15_shared_intersection"))
        .and_then(|x| x.get("terminal_lane_receipts"))
        .and_then(Value::as_i64)
        != Some(9)
    {
        e.push("S15 receipts".into());
    }
    if v.get("source_evidence")
        .and_then(|x| x.get("s16_shared_intersection"))
        .and_then(|x| x.get("visual_receipts"))
        .and_then(Value::as_i64)
        != Some(27)
    {
        e.push("S16 receipts".into());
    }
    let forbidden = seq(v, &["redaction", "forbidden"]);
    for x in [
        "raw_tei",
        "tei_xml",
        "pdf_bytes",
        "body_text",
        "physical_path",
        "hostname",
        "executable",
        "secret",
    ] {
        if !forbidden.contains(&x.to_owned()) {
            e.push(format!("redaction:{x}"));
        }
    }
    e
}
fn mutate(mut v: Value, path: &[&str], value: Value) -> Value {
    let mut cur = &mut v;
    for key in &path[..path.len() - 1] {
        cur = cur.get_mut(*key).unwrap();
    }
    let map: &mut Mapping = cur.as_mapping_mut().unwrap();
    map.insert(Value::String(path[path.len() - 1].into()), value);
    v
}
fn rejects(v: Value, marker: &str) {
    let errors = authority_errors(&v);
    assert!(
        errors.iter().any(|x| x.contains(marker)),
        "mutation `{marker}` was accepted: {errors:?}"
    );
}

#[test]
fn native_fulltext_authority_is_closed_and_honest() {
    let a = yaml(AUTHORITY);
    assert!(
        authority_errors(&a).is_empty(),
        "authority errors: {:?}",
        authority_errors(&a)
    );
    let c = yaml(COMPARISON);
    assert_eq!(
        text(
            &c,
            &["native_fulltext_composition_shadow", "schema_version"]
        )
        .as_deref(),
        Some("native-fulltext-composition.v1")
    );
    let l = yaml(LEDGER);
    assert!(
        l.get("entries")
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .any(|entry| {
                entry.get("id").and_then(Value::as_str)
                    == Some("parse.component.native_fulltext_composition_shadow")
            })
    );
}

#[test]
fn native_fulltext_mutations_fail_closed() {
    let a = yaml(AUTHORITY);
    rejects(
        mutate(
            a.clone(),
            &["admission", "terminal_receipt_is_pass"],
            Value::Bool(true),
        ),
        "terminal-as-pass",
    );
    rejects(
        mutate(
            a.clone(),
            &[
                "source_evidence",
                "s12_named_denominators",
                "references_total",
            ],
            Value::Number(206.into()),
        ),
        "denominator:references_total",
    );
    rejects(
        mutate(
            a.clone(),
            &["dispositions"],
            Value::Sequence(vec![Value::String("mystery".into())]),
        ),
        "disposition vocabulary",
    );
    rejects(
        mutate(
            a.clone(),
            &["component_receipts", "header", "artifact_sha256"],
            Value::Null,
        ),
        "receipt artifact hash:header",
    );
    rejects(
        mutate(
            a.clone(),
            &[
                "component_receipts",
                "document_segmentation",
                "evidence_state",
            ],
            Value::String("passing".into()),
        ),
        "receipt evidence:document_segmentation",
    );
    rejects(
        mutate(
            a.clone(),
            &["component_receipts", "figure", "schema_version"],
            Value::String("native-visual-heads.v2".into()),
        ),
        "receipt schema:figure",
    );
    rejects(
        mutate(
            a.clone(),
            &["component_receipts", "table", "source_hash"],
            Value::String("stale".into()),
        ),
        "receipt source hash:table",
    );
    rejects(
        mutate(
            a.clone(),
            &["admission", "pooled_winner"],
            Value::String("best".into()),
        ),
        "pooling",
    );
    rejects(
        mutate(
            a.clone(),
            &["rollback_target"],
            Value::String("none".into()),
        ),
        "rollback",
    );
    rejects(
        mutate(
            a.clone(),
            &["promotion_state"],
            Value::String("promoted".into()),
        ),
        "policy:promotion_state",
    );
    rejects(
        mutate(a.clone(), &["import_eligible"], Value::Bool(true)),
        "policy:import",
    );
    rejects(
        mutate(a, &["redaction", "forbidden"], Value::Sequence(vec![])),
        "redaction:raw_tei",
    );
}
