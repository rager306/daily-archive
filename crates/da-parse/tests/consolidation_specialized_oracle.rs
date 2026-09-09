//! Independent reviewed oracle validation for the specialized metadata shadow plane.
//!
//! This test intentionally uses only the YAML representation.  It does not import
//! `da_parse`, the accountant, segmentation, fulltext blocks, or the companion's
//! digest code: the fixture is the independent expected-value authority.

use serde_yaml::{Mapping, Value};
use std::collections::BTreeSet;

const FIXTURE: &str = "tests/fixtures/consolidation_specialized_oracle.yaml";
const DOCS: [(&str, &str, usize); 3] = [
    (
        "hub-2507.19457",
        "sha256:ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec",
        96,
    ),
    (
        "rlm-2512.24601",
        "sha256:8567362c22768d9b50d4a4a8d63bb28dda2c2b2051be30d67f70f645170429ca",
        43,
    ),
    (
        "reca-2605.26525v1",
        "sha256:a5672497671518d3b3d79698becfd5a4bbdc512bb02a002a473db501c3b097c8",
        63,
    ),
];
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const FAMILIES: [&str; 6] = [
    "funding",
    "acknowledgement",
    "licence",
    "copyright",
    "software",
    "consolidation",
];
const FORBIDDEN: [&str; 15] = [
    "raw_pdf",
    "pdf_bytes",
    "raw_text",
    "text",
    "tei",
    "tei_xml",
    "image",
    "page_image",
    "path",
    "physical_path",
    "host",
    "hostname",
    "executable",
    "vendor_path",
    "credentials",
];

fn fixture() -> Value {
    serde_yaml::from_str(&std::fs::read_to_string(FIXTURE).expect("fixture exists"))
        .expect("fixture is YAML")
}

fn map(value: &Value) -> &Mapping {
    value.as_mapping().expect("oracle object")
}
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    map(value)
        .get(Value::String(name.into()))
        .unwrap_or_else(|| panic!("missing field {name}"))
}
fn string(value: &Value, name: &str) -> String {
    field(value, name)
        .as_str()
        .unwrap_or_else(|| panic!("{name} must be a string"))
        .to_owned()
}
fn usize_field(value: &Value, name: &str) -> usize {
    field(value, name)
        .as_u64()
        .unwrap_or_else(|| panic!("{name} must be an unsigned integer")) as usize
}
fn strings(value: &Value, name: &str) -> Vec<String> {
    field(value, name)
        .as_sequence()
        .unwrap_or_else(|| panic!("{name} must be a sequence"))
        .iter()
        .map(|item| item.as_str().expect("string list item").to_owned())
        .collect()
}

fn contains_forbidden_key(value: &Value) -> bool {
    match value {
        Value::Mapping(entries) => entries.iter().any(|(key, value)| {
            let key_name = key.as_str();
            // The deny-list itself is metadata, not a raw payload field.
            if key_name == Some("forbidden_fields") {
                return false;
            }
            key_name.is_some_and(|key| FORBIDDEN.contains(&key)) || contains_forbidden_key(value)
        }),
        Value::Sequence(items) => items.iter().any(contains_forbidden_key),
        _ => false,
    }
}

fn upstream_status(root: &Value, document: &str, lane: &str) -> String {
    field(root, "upstream_outcomes")
        .as_sequence()
        .expect("upstream outcomes")
        .iter()
        .find(|outcome| {
            string(outcome, "document_id") == document && string(outcome, "lane") == lane
        })
        .map(|outcome| string(outcome, "status"))
        .expect("every cell has an upstream outcome")
}

/// Independent structural validator. It returns errors rather than normalizing
/// malformed input, so a mutation cannot silently become an empty result.
fn validate(root: &Value) -> Result<(), String> {
    if string(root, "schema_version") != "consolidation-specialized-oracle.v1"
        || string(root, "oracle_id") != "m501-s05-t03-independent-reviewed-v1"
        || !field(root, "independent").as_bool().unwrap_or(false)
        || !field(root, "source_bound").as_bool().unwrap_or(false)
        || string(root, "redaction") != "digest_only"
    {
        return Err("oracle identity or independence changed".into());
    }

    let contract = field(root, "source_contract");
    if string(contract, "corpus_id") != "s13-same-input-pdf-corpus"
        || usize_field(contract, "documents") != 3
        || usize_field(contract, "pages_per_lane") != 202
        || usize_field(contract, "lane_pages") != 606
        || usize_field(contract, "document_lane_outcomes") != 9
        || usize_field(contract, "unpooled_cells") != 54
    {
        return Err("source denominator drift".into());
    }

    let documents = field(root, "documents")
        .as_sequence()
        .ok_or("documents is not a sequence")?;
    if documents.len() != DOCS.len() {
        return Err("document count drift".into());
    }
    let mut document_ids = BTreeSet::new();
    let mut page_sum = 0;
    for (expected_id, expected_sha, expected_pages) in DOCS {
        let document = documents
            .iter()
            .find(|item| string(item, "id") == expected_id)
            .ok_or_else(|| format!("missing document {expected_id}"))?;
        if string(document, "source_sha256") != expected_sha
            || usize_field(document, "pages") != expected_pages
            || !document_ids.insert(expected_id)
        {
            return Err(format!("source identity drift for {expected_id}"));
        }
        page_sum += expected_pages;
    }
    if page_sum != 202 {
        return Err("page denominator does not sum to 202".into());
    }
    if strings(root, "lanes") != LANES || strings(root, "families") != FAMILIES {
        return Err("closed lane or family vocabulary changed".into());
    }

    let outcomes = field(root, "upstream_outcomes")
        .as_sequence()
        .ok_or("upstream outcomes is not a sequence")?;
    if outcomes.len() != 9 {
        return Err("document-lane outcome denominator drift".into());
    }
    let mut outcome_keys = BTreeSet::new();
    for outcome in outcomes {
        let key = (string(outcome, "document_id"), string(outcome, "lane"));
        if !document_ids.contains(key.0.as_str())
            || !LANES.contains(&key.1.as_str())
            || !outcome_keys.insert(key.clone())
        {
            return Err("missing, duplicate, or unknown upstream outcome".into());
        }
        let status = string(outcome, "status");
        if !["pass", "invalid_evidence", "terminal_failure"].contains(&status.as_str()) {
            return Err("unknown upstream status".into());
        }
    }

    let receipts = field(root, "range_receipts")
        .as_sequence()
        .ok_or("range receipts is not a sequence")?;
    if receipts.len() != 9 {
        return Err("range receipt denominator drift".into());
    }
    let mut receipt_keys = BTreeSet::new();
    for receipt in receipts {
        let document = string(receipt, "document_id");
        let lane = string(receipt, "lane");
        let pages = DOCS
            .iter()
            .find(|(id, _, _)| *id == document)
            .map(|(_, _, pages)| *pages)
            .ok_or("range receipt has unknown document")?;
        if !LANES.contains(&lane.as_str())
            || !receipt_keys.insert((document.clone(), lane.clone()))
            || usize_field(receipt, "start_page") != 1
            || usize_field(receipt, "end_page") != pages
            || !string(receipt, "receipt_digest").starts_with("sha256:")
        {
            return Err("incomplete, duplicate, or identity-drifting range receipt".into());
        }
    }

    let cells = field(field(root, "cell_evidence"), "cells")
        .as_sequence()
        .ok_or("cells is not a sequence")?;
    if cells.len() != 54 {
        return Err("specialized cell denominator is not exactly 54".into());
    }
    let mut keys = BTreeSet::new();
    let mut admitted = 0;
    for cell in cells {
        let document = string(cell, "document_id");
        let lane = string(cell, "lane");
        let family = string(cell, "family");
        if !document_ids.contains(document.as_str())
            || !LANES.contains(&lane.as_str())
            || !FAMILIES.contains(&family.as_str())
            || !keys.insert((document.clone(), lane.clone(), family.clone()))
        {
            return Err("missing, duplicate, or unknown unpooled cell".into());
        }
        let status = string(cell, "status");
        if ![
            "source_not_established",
            "enrichment_not_required",
            "propagated_terminal",
            "propagated_invalid",
        ]
        .contains(&status.as_str())
        {
            return Err("unknown typed cell disposition".into());
        }
        let count = map(cell)
            .get(Value::String("count".into()))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if count != 0 {
            return Err("positive specialized count has no source proof".into());
        }
        let upstream = upstream_status(root, &document, &lane);
        if upstream != "pass" && !status.starts_with("propagated_") {
            return Err("non-pass upstream was hidden or coerced".into());
        }
        if upstream == "pass"
            && status.starts_with("propagated_")
            && status != "propagated_terminal"
        {
            return Err("propagation disposition does not match reviewed pass lane".into());
        }
        admitted += 1;
    }
    let evidence = field(root, "cell_evidence");
    if usize_field(evidence, "admitted") != admitted
        || usize_field(evidence, "conserved") != admitted
        || usize_field(evidence, "loss") != 0
        || usize_field(evidence, "positive_count") != 0
        || keys.len() != 54
    {
        return Err("cell conservation or exact-key invariant failed".into());
    }

    let policy = field(root, "policy");
    if string(policy, "observation_mode") != "shadow"
        || string(policy, "production_authority") != "java-grobid-http@0.9.0-crf"
        || field(policy, "import_eligible").as_bool() != Some(false)
        || field(policy, "graph_write").as_bool() != Some(false)
        || field(policy, "promotion").as_bool() != Some(false)
        || field(policy, "auto_promote").as_bool() != Some(false)
        || field(policy, "production_route").as_bool() != Some(false)
        || usize_field(policy, "decision_point") != 6
        || field(policy, "pooled_summary").as_bool() != Some(false)
        || strings(policy, "independently_selectable") != ["header", "references"]
    {
        return Err("production or selection policy unlocked".into());
    }
    if contains_forbidden_key(root) {
        return Err("redaction boundary violated".into());
    }
    Ok(())
}

#[test]
fn independent_oracle_has_exact_54_unpooled_cells_and_complete_ranges() {
    let root = fixture();
    validate(&root).unwrap();
    assert_eq!(
        field(&root, "cell_evidence")["cells"]
            .as_sequence()
            .unwrap()
            .len(),
        54
    );
    assert_eq!(
        field(&root, "range_receipts").as_sequence().unwrap().len(),
        9
    );
}

#[test]
fn all_six_non_pass_document_lane_outcomes_propagate_to_every_family() {
    let root = fixture();
    let cells = field(field(&root, "cell_evidence"), "cells")
        .as_sequence()
        .unwrap();
    let non_pass: BTreeSet<_> = field(&root, "upstream_outcomes")
        .as_sequence()
        .unwrap()
        .iter()
        .filter(|item| string(item, "status") != "pass")
        .map(|item| (string(item, "document_id"), string(item, "lane")))
        .collect();
    assert_eq!(non_pass.len(), 6);
    for (document, lane) in non_pass {
        let matching: Vec<_> = cells
            .iter()
            .filter(|cell| string(cell, "document_id") == document && string(cell, "lane") == lane)
            .collect();
        assert_eq!(matching.len(), 6);
        assert!(matching.iter().all(|cell| {
            ["propagated_terminal", "propagated_invalid"].contains(&string(cell, "status").as_str())
        }));
    }
}

#[test]
fn mutation_suite_rejects_denominator_identity_positive_and_policy_drift() {
    let original = fixture();
    type Mutation = (&'static str, Box<dyn Fn(&mut Value)>);
    let mut mutations: Vec<Mutation> = Vec::new();
    mutations.push((
        "remove cell",
        Box::new(|root| {
            field_mut(root, "cell_evidence")["cells"]
                .as_sequence_mut()
                .unwrap()
                .pop();
        }),
    ));
    mutations.push((
        "duplicate cell",
        Box::new(|root| {
            let cells = field_mut(root, "cell_evidence")["cells"]
                .as_sequence_mut()
                .unwrap();
            let copy = cells[0].clone();
            cells.push(copy);
        }),
    ));
    mutations.push((
        "change source sha",
        Box::new(|root| {
            field_mut(root, "documents")[0]["source_sha256"] = Value::String("sha256:drift".into());
        }),
    ));
    mutations.push((
        "change denominator",
        Box::new(|root| {
            field_mut(root, "source_contract")["lane_pages"] = Value::Number(605.into());
        }),
    ));
    mutations.push((
        "invent positive",
        Box::new(|root| {
            field_mut(root, "cell_evidence")["cells"][0]["count"] = Value::Number(1.into());
        }),
    ));
    mutations.push((
        "hide terminal",
        Box::new(|root| {
            field_mut(root, "cell_evidence")["cells"][0]["status"] =
                Value::String("source_not_established".into());
        }),
    ));
    mutations.push((
        "unlock production",
        Box::new(|root| {
            field_mut(root, "policy")["import_eligible"] = Value::Bool(true);
        }),
    ));
    mutations.push((
        "add raw path",
        Box::new(|root| {
            field_mut(root, "policy")["physical_path"] = Value::String("/secret/input.pdf".into());
        }),
    ));
    for (name, mutate) in mutations {
        let mut changed = original.clone();
        mutate(&mut changed);
        assert!(validate(&changed).is_err(), "mutation {name} was accepted");
    }
}

fn field_mut<'a>(value: &'a mut Value, name: &str) -> &'a mut Value {
    value
        .as_mapping_mut()
        .unwrap()
        .get_mut(Value::String(name.into()))
        .unwrap_or_else(|| panic!("missing mutable field {name}"))
}

#[test]
fn oracle_is_digest_only_and_has_no_sut_or_raw_payload_dependency() {
    let root = fixture();
    assert!(!contains_forbidden_key(&root));
    let serialized = serde_yaml::to_string(&root).unwrap();
    assert!(!serialized.contains("NativeBodyAccountant"));
    assert!(!serialized.contains("evaluate_native_consolidation_specialized"));
    assert!(!contains_forbidden_key(&root));
}
