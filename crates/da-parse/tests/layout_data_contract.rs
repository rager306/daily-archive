//! DATA contract for the S13 same-input PDF layout bakeoff.
//! The oracle is deliberately independent of da-parse donor implementations:
//! it validates only redacted identities, bounded geometry and denominators.

use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST: &str = "data/parse/layout_bakeoff.yaml";
const ORACLE: &str = "crates/da-parse/tests/fixtures/layout_oracle.yaml";
const FORBIDDEN: &[&str] = &[
    "raw_text",
    "pdf_bytes",
    "tei_xml",
    "page_image",
    "physical_path",
    "body_text",
    "raw_document_content",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn yaml(path: &str) -> Value {
    let text = fs::read_to_string(repo_root().join(path)).expect("DATA file readable");
    serde_yaml::from_str(&text).expect("DATA file is valid YAML")
}

fn str_at<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    path.iter()
        .fold(value, |current, key| &current[*key])
        .as_str()
        .expect("DATA string")
}

fn sha256(path: &Path) -> String {
    let bytes = fs::read(path).expect("source PDF readable");
    hex::encode(Sha256::digest(bytes))
}

fn reject_forbidden(value: &Value, path: &str) {
    match value {
        Value::Mapping(map) => {
            for (key, child) in map {
                if let Some(key) = key.as_str() {
                    // The top-level `forbidden_fields` list is policy metadata;
                    // the same names are forbidden only as payload keys.
                    let is_policy_declaration = path == "oracle" && key == "forbidden_fields";
                    assert!(
                        is_policy_declaration || !FORBIDDEN.contains(&key),
                        "forbidden raw field `{key}` at {path}"
                    );
                    if !is_policy_declaration {
                        reject_forbidden(child, &format!("{path}.{key}"));
                    }
                }
            }
        }
        Value::Sequence(items) => {
            for (index, child) in items.iter().enumerate() {
                reject_forbidden(child, &format!("{path}[{index}]"));
            }
        }
        _ => {}
    }
}

#[test]
fn same_input_manifest_binds_three_pdf_sources_and_exact_pages() {
    let manifest = yaml(MANIFEST);
    assert_eq!(str_at(&manifest, &["schema_version"]), "layout-bakeoff.v1");
    assert_eq!(manifest["corpus"]["corpus_n"].as_u64(), Some(3));
    assert_eq!(manifest["corpus"]["page_denominator"].as_u64(), Some(202));
    assert_eq!(manifest["lanes"]["same_input"].as_bool(), Some(true));
    assert_eq!(
        manifest["lanes"]["all_lanes_required"].as_bool(),
        Some(true)
    );
    assert_eq!(
        manifest["acceptance"]["no_winner_encoded"].as_bool(),
        Some(true)
    );
    assert_eq!(
        manifest["acceptance"]["selected_state"].as_str(),
        Some("forbidden")
    );
    assert_eq!(
        manifest["acceptance"]["promoted_state"].as_str(),
        Some("forbidden")
    );

    let expected_lanes: BTreeSet<_> = [
        "pdfalto",
        "pdf_inspector_positioned",
        "raw_positioned",
        "xy_cut_positioned",
    ]
    .into_iter()
    .collect();
    let lanes: BTreeSet<_> = manifest["lanes"]["required"]
        .as_sequence()
        .expect("required lanes")
        .iter()
        .map(|v| v.as_str().expect("lane name"))
        .collect();
    assert_eq!(lanes, expected_lanes);

    let docs = manifest["corpus"]["documents"]
        .as_sequence()
        .expect("documents");
    let mut pages = 0;
    let mut ids = BTreeSet::new();
    for doc in docs {
        let id = doc["id"].as_str().expect("document id");
        assert!(ids.insert(id), "duplicate document identity: {id}");
        let path = repo_root().join(doc["source_pdf"].as_str().expect("source path"));
        assert!(
            path.is_file(),
            "missing source PDF for {id}: {}",
            path.display()
        );
        let expected = doc["sha256"].as_str().expect("source checksum");
        assert_eq!(expected.len(), 64, "checksum shape for {id}");
        assert_eq!(sha256(&path), expected, "checksum drift for {id}");
        let count = doc["pages"].as_u64().expect("page count");
        assert!(count > 0, "empty page denominator for {id}");
        pages += count;
        assert!(
            doc["page_classes"]
                .as_sequence()
                .is_some_and(|v| v.len() >= 6),
            "document {id} lacks required layout-risk classes"
        );
        assert!(doc["supporting_s07_case"].as_str().is_some());
    }
    assert_eq!(pages, 202, "manifest page denominator drift");
}

#[test]
fn independent_oracle_is_redacted_and_has_complete_page_denominators() {
    let manifest = yaml(MANIFEST);
    let oracle = yaml(ORACLE);
    reject_forbidden(&oracle, "oracle");
    assert_eq!(oracle["schema_version"].as_str(), Some("layout-oracle.v1"));
    assert_eq!(
        oracle["oracle_id"].as_str(),
        Some("independent-layout-receipts-v1")
    );
    assert_eq!(manifest["oracle"]["independent"].as_bool(), Some(true));
    assert_eq!(manifest["oracle"]["no_donor_helpers"].as_bool(), Some(true));
    assert_eq!(manifest["oracle"]["no_tei_lifter"].as_bool(), Some(true));

    let docs = manifest["corpus"]["documents"].as_sequence().unwrap();
    let receipts = oracle["receipts"].as_sequence().expect("oracle receipts");
    assert_eq!(
        receipts.len(),
        docs.len(),
        "one oracle receipt per document"
    );
    for doc in docs {
        let id = doc["id"].as_str().unwrap();
        let receipt = receipts
            .iter()
            .find(|r| r["document_id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("missing oracle receipt for {id}"));
        assert_eq!(receipt["source_sha256"], doc["sha256"]);
        assert_eq!(receipt["page_count"], doc["pages"]);
        let pages = receipt["page_ordinals"]
            .as_sequence()
            .expect("page ordinals");
        let expected: Vec<_> = (0..doc["pages"].as_u64().unwrap()).collect();
        let actual: Vec<_> = pages.iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!(actual, expected, "page ordinal loss for {id}");
        let tokens = receipt["token_receipts"].as_sequence().expect("token rows");
        let mut token_ordinals = BTreeSet::new();
        for token in tokens {
            assert!(token_ordinals.insert(token["token_ordinal"].as_u64().unwrap()));
            let bbox = token["bbox_72dpi_top_left"].as_sequence().unwrap();
            assert_eq!(bbox.len(), 4);
            let values: Vec<_> = bbox
                .iter()
                .map(|v| {
                    v.as_f64()
                        .or_else(|| v.as_u64().map(|n| n as f64))
                        .expect("finite bbox number")
                })
                .collect();
            assert!(values.iter().all(|v| v.is_finite()));
            assert!(
                values[0] >= 0.0
                    && values[1] >= 0.0
                    && values[2] > values[0]
                    && values[3] > values[1]
            );
            assert_eq!(token["token_digest"].as_str().unwrap().len(), 64);
            assert!(token["role"].as_str().is_some());
        }
    }
}

#[test]
fn failure_taxonomy_is_closed_and_no_selection_is_encoded() {
    let manifest = yaml(MANIFEST);
    let allowed: BTreeSet<_> = [
        "missing",
        "checksum_mismatch",
        "malformed",
        "unsupported",
        "timeout",
        "nonzero",
        "oversize",
        "encrypted",
        "truncated",
    ]
    .into_iter()
    .collect();
    let actual: BTreeSet<_> = manifest["lanes"]["unavailable_outcomes"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(actual, allowed);
    assert_eq!(
        manifest["acceptance"]["no_silent_skips"].as_bool(),
        Some(true)
    );
    assert_eq!(
        manifest["acceptance"]["promotion_state"].as_str(),
        Some("candidate")
    );
}
