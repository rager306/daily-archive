//! Independently reviewed, digest-only oracle for the Java feature/training surface.
//! This test intentionally does not import the parser companion or any SUT digest helper.
use serde_yaml::Value;
use std::{collections::BTreeSet, fs, path::PathBuf};

const FIXTURE: &str = "crates/da-parse/tests/fixtures/feature_tagger_training_oracle.yaml";
const SOURCE: &str = "artifacts/parse_compat/feature_tagger_training_oracle_source.json";
const LANES: [&str; 3] = ["layout", "tei", "reference"];
const STATUSES: [&str; 3] = ["pass", "invalid_evidence", "terminal_failure"];
const FORBIDDEN: [&str; 16] = [
    "raw_pdf",
    "raw_text",
    "raw_tei",
    "pdf_bytes",
    "tei_xml",
    "page_image",
    "image_bytes",
    "model_payload",
    "weights",
    "credentials",
    "physical_path",
    "vendor_path",
    "executable",
    "hostname",
    "secret",
    "payload",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn fixture() -> Value {
    serde_yaml::from_str(&fs::read_to_string(root().join(FIXTURE)).expect("fixture exists"))
        .expect("fixture YAML is valid")
}
fn source() -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(root().join(SOURCE)).expect("source exists"))
        .expect("source JSON is valid")
}
fn digest(value: &str) -> bool {
    let raw = value.strip_prefix("sha256:").unwrap_or("");
    raw.len() == 64 && raw.bytes().all(|b| b.is_ascii_hexdigit()) && !raw.chars().all(|c| c == '0')
}
fn contains_key(value: &Value, forbidden: &str) -> bool {
    match value {
        Value::Mapping(map) => map
            .iter()
            .any(|(key, child)| key.as_str() == Some(forbidden) || contains_key(child, forbidden)),
        Value::Sequence(items) => items.iter().any(|item| contains_key(item, forbidden)),
        _ => false,
    }
}
fn source_contains_key(value: &serde_json::Value, forbidden: &str) -> bool {
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .any(|(key, child)| key == forbidden || source_contains_key(child, forbidden)),
        serde_json::Value::Array(items) => items
            .iter()
            .any(|item| source_contains_key(item, forbidden)),
        _ => false,
    }
}

/// The validator is an oracle over reviewed data, not a producer of expected values.
fn validate(f: &Value, s: &serde_json::Value) -> Vec<String> {
    let mut errors = Vec::new();
    if f["schema_version"].as_str() != Some("feature-tagger-training-oracle.v1")
        || f["review_status"].as_str() != Some("independently_reviewed")
        || f["representation"].as_str() != Some("digest_only_source_bound_ordinals")
        || f["expected_rows_not_from_sut_helpers"].as_bool() != Some(true)
        || s["schema_version"].as_str() != Some("feature-tagger-training-oracle-source.v1")
        || s["review_status"].as_str() != Some("independently_reviewed")
        || s["digest_only"].as_bool() != Some(true)
    {
        errors.push("review metadata drift".into());
    }
    for key in FORBIDDEN {
        if contains_key(f, key) || source_contains_key(s, key) {
            errors.push(format!("forbidden payload key: {key}"));
        }
    }
    if f["redaction"]["raw_content"].as_str() != Some("forbidden")
        || f["redaction"]["forbidden_payload_presence"].as_bool() != Some(false)
        || s["raw_payload_present"].as_bool() != Some(false)
    {
        errors.push("redaction contract drift".into());
    }
    if f["corpus"]["documents"].as_sequence().map_or(0, Vec::len) != 3
        || f["corpus"]["pages_per_lane"].as_u64() != Some(202)
        || f["corpus"]["lane_pages"].as_u64() != Some(606)
        || f["corpus"]["document_lane_outcomes"].as_u64() != Some(9)
        || s["corpus"]["documents"].as_u64() != Some(3)
        || s["corpus"]["pages_per_lane"].as_u64() != Some(202)
        || s["corpus"]["lane_pages"].as_u64() != Some(606)
        || s["corpus"]["document_lane_outcomes"].as_u64() != Some(9)
    {
        errors.push("corpus denominator drift".into());
    }
    let docs = f["corpus"]["documents"]
        .as_sequence()
        .cloned()
        .unwrap_or_default();
    let mut doc_ids = BTreeSet::new();
    for doc in &docs {
        let id = doc["identity"].as_str().unwrap_or("");
        if !doc_ids.insert(id) || !digest(doc["source_sha256"].as_str().unwrap_or("")) {
            errors.push(format!("invalid document identity: {id}"));
        }
    }
    let feature_units = f["inventory"]["feature_source_units"].as_u64();
    let engines = f["inventory"]["tagger_engines"]
        .as_sequence()
        .map_or(0, Vec::len);
    if feature_units != Some(14)
        || f["inventory"]["features_vector_classes"].as_u64() != Some(13)
        || f["inventory"]["feature_factory_units"].as_u64() != Some(1)
        || engines != 4
        || f["inventory"]["training_entry_points"]
            .as_sequence()
            .map_or(0, Vec::len)
            != 2
        || f["inventory"]["generator_modes"]
            .as_sequence()
            .map_or(0, Vec::len)
            != 3
        || f["inventory"]["lifecycle"]
            .as_sequence()
            .map_or(0, Vec::len)
            != 3
    {
        errors.push("inventory denominator drift".into());
    }
    let outcomes = f["outcomes"].as_sequence().cloned().unwrap_or_default();
    let source_outcomes = s["outcomes"].as_array().cloned().unwrap_or_default();
    if outcomes.len() != 9 || source_outcomes.len() != 9 {
        errors.push("outcome denominator is not 9".into());
    }
    let mut keys = BTreeSet::new();
    let mut non_pass = 0;
    for (index, outcome) in outcomes.iter().enumerate() {
        let id = outcome["document_id"].as_str().unwrap_or("");
        let lane = outcome["lane"].as_str().unwrap_or("");
        let key = format!("{id}/{lane}");
        if !keys.insert(key.clone()) || !doc_ids.contains(id) || !LANES.contains(&lane) {
            errors.push(format!("duplicate or unknown outcome: {key}"));
        }
        if outcome["ordinal"].as_u64() != Some(index as u64)
            || outcome["page_count"].as_u64() != Some(202)
            || !STATUSES.contains(&outcome["status"].as_str().unwrap_or(""))
        {
            errors.push(format!("outcome order/status drift: {key}"));
        }
        if outcome["status"].as_str() != Some("pass") {
            non_pass += 1;
        }
        let source_row = source_outcomes.get(index);
        if source_row.and_then(|r| r["ordinal"].as_u64()) != Some(index as u64)
            || source_row.and_then(|r| r["document_id"].as_str()) != Some(id)
            || source_row.and_then(|r| r["lane"].as_str()) != Some(lane)
            || source_row.and_then(|r| r["status"].as_str()) != outcome["status"].as_str()
        {
            errors.push(format!("source binding drift: {key}"));
        }
    }
    if keys.len() != 9 || non_pass != 6 {
        errors.push("six non-pass outcomes are not preserved".into());
    }

    let annotation = &f["structural_annotation"];
    let receipts = annotation["receipts"]
        .as_sequence()
        .cloned()
        .unwrap_or_default();
    if annotation["positive_reviewed_rows"].as_u64() != Some(4)
        || annotation["receipt_count"].as_u64() != Some(4)
        || receipts.len() != 4
        || annotation["denominator_is_not_pooled"].as_bool() != Some(true)
        || annotation["columns"].as_sequence().map_or(0, Vec::len) != 11
    {
        errors.push("structural receipt denominator drift".into());
    }
    let mut receipt_ordinals = BTreeSet::new();
    for (index, receipt) in receipts.iter().enumerate() {
        let ordinal = receipt["ordinal"].as_u64().unwrap_or(u64::MAX);
        if ordinal != index as u64
            || !receipt_ordinals.insert(ordinal)
            || !doc_ids.contains(receipt["document_id"].as_str().unwrap_or(""))
            || !LANES.contains(&receipt["lane"].as_str().unwrap_or(""))
            || !digest(receipt["token_digest"].as_str().unwrap_or(""))
        {
            errors.push(format!("invalid receipt {index}"));
        }
        let disposition = receipt["disposition"].as_str().unwrap_or("");
        if disposition == "structural_pass"
            && (receipt["admitted"].as_u64() != receipt["conserved"].as_u64()
                || receipt["loss"].as_u64() != Some(0))
        {
            errors.push(format!("structural conservation drift {index}"));
        }
        if disposition == "propagated_terminal"
            && (!["invalid_evidence", "terminal_failure"]
                .contains(&receipt["terminal_code"].as_str().unwrap_or(""))
                || receipt["admitted"].as_u64() != Some(0)
                || receipt["loss"].as_u64().unwrap_or(0) == 0)
        {
            errors.push(format!("terminal coercion or zero-loss drift {index}"));
        }
    }
    if annotation["unsupported"].as_sequence().map_or(0, Vec::len) != 8
        || annotation["visual_disposition"].as_str() != Some("unsupported_visual")
        || annotation["training_disposition"].as_str() != Some("source_not_established")
        || annotation["model_disposition"].as_str() != Some("forbidden_this_wave")
    {
        errors.push("typed gaps drift".into());
    }
    for claim in [
        "implementation_parity",
        "training_equivalence",
        "model_quality",
        "pooling",
        "production_splice",
    ] {
        if f["claims"][claim].as_bool() != Some(false) {
            errors.push(format!("unsupported claim: {claim}"));
        }
    }
    errors
}

#[test]
fn independent_oracle_binds_source_and_preserves_exact_denominators() {
    let f = fixture();
    let s = source();
    let errors = validate(&f, &s);
    assert!(
        errors.is_empty(),
        "oracle violations:\n{}",
        errors.join("\n")
    );
    assert_eq!(f["structural_annotation"]["receipt_count"], 4);
    assert_eq!(s["positive_reviewed_structural_rows"], 4);
}

#[test]
fn oracle_mutations_fail_closed_without_sut_or_pooling() {
    let original = fixture();
    let source_data = source();
    type Mutation = (&'static str, fn(&mut Value));
    let mutations: [Mutation; 10] = [
        ("missing receipt", |v| {
            v["structural_annotation"]["receipts"]
                .as_sequence_mut()
                .unwrap()
                .pop();
        }),
        ("duplicate receipt", |v| {
            let first = v["structural_annotation"]["receipts"][0].clone();
            v["structural_annotation"]["receipts"]
                .as_sequence_mut()
                .unwrap()[1] = first;
        }),
        ("reordered outcome", |v| {
            v["outcomes"][0]["ordinal"] = 1.into();
        }),
        ("digest substitution", |v| {
            v["structural_annotation"]["receipts"][0]["token_digest"] = "placeholder".into();
        }),
        ("denominator erosion", |v| {
            v["corpus"]["lane_pages"] = 605.into();
        }),
        ("terminal coercion", |v| {
            v["outcomes"][1]["status"] = "pass".into();
        }),
        ("invented positive", |v| {
            v["structural_annotation"]["receipts"][1]["disposition"] = "structural_pass".into();
        }),
        ("visual lexical admission", |v| {
            v["structural_annotation"]["visual_disposition"] = "structural_pass".into();
        }),
        ("placeholder metric", |v| {
            v["structural_annotation"]["positive_reviewed_rows"] = 0.into();
        }),
        ("raw payload", |v| {
            v["raw_text"] = "forbidden".into();
        }),
    ];
    for (name, mutate) in mutations {
        let mut candidate = original.clone();
        mutate(&mut candidate);
        assert!(
            !validate(&candidate, &source_data).is_empty(),
            "mutation accepted: {name}"
        );
    }
}

#[test]
fn oracle_source_is_structurally_independent_from_companion() {
    let source =
        fs::read_to_string(root().join("crates/da-parse/tests/feature_tagger_training_oracle.rs"))
            .unwrap();
    let forbidden_names = [
        ["native_", "feature_tagger"].concat(),
        ["Native", "FeatureTagger"].concat(),
        ["NativeBody", "Accountant"].concat(),
        ["segment_document_", "full"].concat(),
        ["LayoutLattice::", "digest"].concat(),
    ];
    for forbidden in forbidden_names {
        assert!(
            !source.contains(&forbidden),
            "oracle imports or calls SUT helper: {forbidden}"
        );
    }
}

#[test]
fn oracle_does_not_claim_training_or_model_quality() {
    let f = fixture();
    for key in [
        "implementation_parity",
        "training_equivalence",
        "model_quality",
    ] {
        assert_eq!(f["claims"][key], false);
    }
    assert_eq!(
        f["structural_annotation"]["training_disposition"],
        "source_not_established"
    );
    assert_eq!(
        f["structural_annotation"]["model_disposition"],
        "forbidden_this_wave"
    );
}
