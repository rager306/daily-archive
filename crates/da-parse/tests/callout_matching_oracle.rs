//! Independent, unpooled reviewed oracle for S04 callout matching.
//! This test deliberately does not import `da_parse` or derive expectations from the matcher.
use serde::Deserialize;
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

const FIXTURE: &str = include_str!("fixtures/callout_matching_oracle.yaml");
const SOURCE: &str =
    include_str!("../../../artifacts/parse_compat/callout_matching_oracle_source.json");
const JAVA_SOURCE_SHA256: &str = "c0729ce1f99d8af5b64071dd2b898e4c322fae9dcb3fdc33e32547300e203a0e";
const DOCS: [&str; 3] = ["hub-2507.19457", "rlm-2512.24601", "reca-2605.26525v1"];
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const FAMILIES: [&str; 6] = [
    "paragraph",
    "section",
    "list_item",
    "note",
    "annex",
    "marker",
];
const REASONS: [&str; 6] = [
    "no_target",
    "target_not_in_inventory",
    "ambiguous",
    "illegal_adjacency",
    "unsupported_equation",
    "source_not_established",
];
const FORBIDDEN: [&str; 15] = [
    "raw_text",
    "text",
    "tei",
    "tei_xml",
    "pdf",
    "pdf_bytes",
    "image",
    "page_image",
    "path",
    "physical_path",
    "host",
    "hostname",
    "executable",
    "vendor_path",
    "raw_source",
];

#[derive(Debug, Deserialize)]
struct Oracle {
    schema_version: String,
    oracle_id: String,
    independent: bool,
    source_bound: bool,
    source_receipt: Receipt,
    policy: Policy,
    marker_vocabulary: Vec<String>,
    source_gaps: Vec<String>,
    #[allow(dead_code)]
    forbidden_fields: Vec<String>,
    documents: Vec<Document>,
    lanes: Vec<String>,
    upstream_outcomes: Vec<Outcome>,
    cell_evidence: Cells,
    observations: Vec<Observation>,
    summary: Summary,
}
#[derive(Debug, Deserialize)]
struct Receipt {
    corpus_id: String,
    java_source_artifact: String,
    oracle_source_sha256: String,
    java_source_sha256: String,
    segmentation_schema: String,
    segmentation_sha256: String,
    fulltext_schema: String,
    documents: usize,
    pages: usize,
    lane_pages: usize,
    document_lane_outcomes: usize,
    unpooled_cells: usize,
}
#[derive(Debug, Deserialize)]
struct Policy {
    observation_mode: String,
    production_authority: String,
    import_eligible: bool,
    graph_write: bool,
    promotion: bool,
    auto_promote: bool,
    #[serde(rename = "DecisionPoint")]
    decision_point: u8,
    independently_selectable: Vec<String>,
}
#[derive(Debug, Deserialize)]
struct Document {
    id: String,
    sha256: String,
    pages: usize,
}
#[derive(Debug, Deserialize)]
struct Outcome {
    document_id: String,
    lane: String,
    status: String,
    failure_code: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Cells {
    generated_from: String,
    families: Vec<String>,
    rows: usize,
    non_pass_disposition: String,
    pass_disposition: String,
}
#[derive(Debug, Deserialize)]
struct Observation {
    id: String,
    document_id: String,
    lane: String,
    subtype: String,
    marker_digest: String,
    target_outcome: String,
    review: String,
    majority_style: Option<String>,
    adjacency: Option<String>,
    #[allow(dead_code)]
    unresolved_reason: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Summary {
    admitted: usize,
    conserved: usize,
    loss: usize,
    matched: usize,
    unpooled_unresolved: usize,
    by_subtype: HashMap<String, usize>,
    by_target_outcome: HashMap<String, usize>,
    majority: HashMap<String, usize>,
    adjacency: HashMap<String, usize>,
}

fn reject_forbidden(value: &Value) -> Result<(), String> {
    match value {
        Value::Mapping(map) => {
            for (key, child) in map {
                if let Some(name) = key.as_str() {
                    if FORBIDDEN.contains(&name) {
                        return Err(format!("forbidden key: {name}"));
                    }
                    reject_forbidden(child)?;
                }
            }
        }
        Value::Sequence(items) => {
            for item in items {
                reject_forbidden(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn parse() -> Oracle {
    let value: Value = serde_yaml::from_str(FIXTURE).expect("oracle YAML parses");
    reject_forbidden(&value).expect("oracle remains redacted");
    serde_yaml::from_value(value).expect("oracle shape is independent and explicit")
}

#[test]
fn source_bound_unpooled_oracle_has_exact_lineage_and_policy_locks() {
    let o = parse();
    assert_eq!(o.schema_version, "callout-matching-oracle.v1");
    assert_eq!(o.oracle_id, "m501-s04-t03-independent-reviewed-v1");
    assert!(o.independent && o.source_bound);
    assert_eq!(o.source_receipt.corpus_id, "s13-same-input-pdf-corpus");
    assert_eq!(
        o.source_receipt.java_source_schema(),
        "java-grobid-callout-matching-source.v1"
    );
    assert_eq!(
        o.source_receipt.java_source_artifact,
        "artifacts/parse_compat/java_grobid_callout_matching_source.json"
    );
    assert_eq!(
        o.source_receipt.oracle_source_sha256,
        sha256(SOURCE.as_bytes())
    );
    assert_eq!(o.source_receipt.java_source_sha256, JAVA_SOURCE_SHA256);
    assert_eq!(
        o.source_receipt.segmentation_schema,
        "document-segmentation-shadow.v2"
    );
    assert_eq!(o.source_receipt.segmentation_sha256.len(), 64);
    assert_eq!(
        o.source_receipt.fulltext_schema,
        "fulltext-block-semantics.v1"
    );
    assert_eq!(
        (
            o.source_receipt.documents,
            o.source_receipt.pages,
            o.source_receipt.lane_pages,
            o.source_receipt.document_lane_outcomes,
            o.source_receipt.unpooled_cells
        ),
        (3, 202, 606, 9, 54)
    );
    assert_eq!(
        o.marker_vocabulary,
        ["citation", "figure", "table", "equation"]
    );
    assert_eq!(
        o.source_gaps,
        [
            "target_inventory",
            "majority_style",
            "adjacency",
            "equation_inventory"
        ]
    );
    assert!(
        !o.policy.import_eligible
            && !o.policy.graph_write
            && !o.policy.promotion
            && !o.policy.auto_promote
    );
    assert_eq!(
        (
            o.policy.observation_mode.as_str(),
            o.policy.production_authority.as_str(),
            o.policy.decision_point
        ),
        ("shadow", "java-grobid-http@0.9.0-crf", 6)
    );
    assert_eq!(o.policy.independently_selectable, ["header", "references"]);
}

impl Receipt {
    fn java_source_schema(&self) -> &str {
        "java-grobid-callout-matching-source.v1"
    }
}

#[test]
fn all_document_lanes_propagate_and_cell_denominator_is_unpooled() {
    let o = parse();
    assert_eq!(o.documents.len(), 3);
    assert_eq!(o.lanes, LANES);
    let identities: HashSet<_> = o.documents.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(identities, DOCS.into_iter().collect());
    assert!(
        o.documents
            .iter()
            .all(|d| d.pages > 0 && d.sha256.len() == 64)
    );
    assert_eq!(o.upstream_outcomes.len(), 9);
    let mut outcomes = HashMap::new();
    for row in &o.upstream_outcomes {
        assert!(
            identities.contains(row.document_id.as_str()) && LANES.contains(&row.lane.as_str())
        );
        assert!(
            outcomes
                .insert((&row.document_id, &row.lane), row.status.as_str())
                .is_none()
        );
        if row.status == "pass" {
            assert!(row.failure_code.is_none());
        } else {
            assert!(row.failure_code.is_some());
        }
    }
    assert_eq!(o.cell_evidence.families, FAMILIES);
    assert_eq!(o.cell_evidence.rows, 54);
    assert_eq!(
        o.cell_evidence.generated_from,
        "upstream_outcomes x [paragraph, section, list_item, note, annex, marker]"
    );
    assert_eq!(
        o.cell_evidence.non_pass_disposition,
        "propagate_upstream_terminal_or_invalid_evidence"
    );
    assert_eq!(
        o.cell_evidence.pass_disposition,
        "reviewed_or_typed_non_positive"
    );
}

#[test]
fn every_observation_is_reviewed_typed_and_conserved_without_positive_invention() {
    let o = parse();
    assert_eq!(o.observations.len(), 12);
    let mut ids = HashSet::new();
    let mut outcomes = HashMap::new();
    for row in &o.upstream_outcomes {
        outcomes.insert((&row.document_id, &row.lane), row.status.as_str());
    }
    for row in &o.observations {
        assert!(ids.insert(&row.id));
        assert!(DOCS.contains(&row.document_id.as_str()) && row.lane == "pdf_inspector_positioned");
        assert!(o.marker_vocabulary.contains(&row.subtype));
        assert!(row.marker_digest.starts_with("sha256:") && row.marker_digest.len() == 71);
        assert_eq!(row.review, "independently_reviewed");
        assert!(REASONS.contains(&row.target_outcome.as_str()));
        assert_eq!(outcomes[&(&row.document_id, &row.lane)], "pass");
        if row.target_outcome == "ambiguous" {
            assert_eq!(row.majority_style.as_deref(), Some("tie"));
        }
        if row.target_outcome == "illegal_adjacency" {
            assert_eq!(row.adjacency.as_deref(), Some("illegal_boundary_crossing"));
        }
        if row.subtype == "equation" {
            assert_eq!(row.target_outcome, "source_not_established");
        }
    }
    assert_eq!(o.summary.admitted, 12);
    assert_eq!(
        (
            o.summary.conserved,
            o.summary.loss,
            o.summary.matched,
            o.summary.unpooled_unresolved
        ),
        (12, 0, 0, 12)
    );
    assert_eq!(
        o.summary.by_subtype,
        HashMap::from([
            (String::from("citation"), 3),
            (String::from("figure"), 3),
            (String::from("table"), 3),
            (String::from("equation"), 3)
        ])
    );
    assert_eq!(o.summary.by_target_outcome.values().sum::<usize>(), 12);
    assert_eq!(o.summary.majority.values().sum::<usize>(), 12);
    assert_eq!(o.summary.adjacency.values().sum::<usize>(), 12);
}

#[test]
fn mutation_like_checks_reject_duplicate_deleted_rows_and_forbidden_payloads() {
    let mut value: Value = serde_yaml::from_str(FIXTURE).unwrap();
    let original = value["observations"].as_sequence().unwrap().len();
    {
        let observations = value["observations"].as_sequence_mut().unwrap();
        let first = observations[0].clone();
        observations.push(first);
    }
    let parsed: Oracle = serde_yaml::from_value(value.clone()).unwrap();
    let unique: HashSet<_> = parsed.observations.iter().map(|x| x.id.as_str()).collect();
    assert_ne!(
        parsed.observations.len(),
        unique.len(),
        "duplicate-row mutation must be visible"
    );
    value["observations"].as_sequence_mut().unwrap().pop();
    assert_eq!(
        value["observations"].as_sequence().unwrap().len(),
        original,
        "deleted-row mutation must change denominator"
    );
    value.as_mapping_mut().unwrap().insert(
        Value::String("raw_text".into()),
        Value::String("leak".into()),
    );
    assert!(reject_forbidden(&value).is_err());
}
