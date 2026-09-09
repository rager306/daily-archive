//! S02 DATA contract gate: source-bound full segmentation family + redacted oracle.
//! This test validates DATA only; it deliberately never calls production segmentation.
use serde_json::Value as Json;
use serde_yaml::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const MANIFEST: &str = "data/parse/document_segmentation.yaml";
const ORACLE: &str = "crates/da-parse/tests/fixtures/segmentation_oracle.yaml";
const JAVA: &str = "artifacts/parse_compat/java_grobid_body_taxonomy_source.json";
const LATTICE: &str = "data/parse/layout_lattice_parity.yaml";
const LAYOUT: &str = "artifacts/parse_compat/java_grobid_layout_source.json";
const REGIONS: [&str; 9] = [
    "header",
    "body",
    "references",
    "annex",
    "acknowledgement",
    "toc",
    "funding",
    "availability",
    "page_furniture",
];
const FURNITURE: [&str; 5] = [
    "page_number",
    "cover",
    "footnote",
    "headnote",
    "margin_note",
];
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const DOCUMENTS: [(&str, &str, i64); 3] = [
    (
        "hub-2507.19457",
        "ab3a5139bac83f192ad67529368d77b84b0d807e95a8e4fd0daa8d45fd046bec",
        96,
    ),
    (
        "rlm-2512.24601",
        "8567362c22768d9b50d4a4a8d63bb28dda2c2b2051be30d67f70f645170429ca",
        43,
    ),
    (
        "reca-2605.26525v1",
        "a5672497671518d3b3d79698becfd5a4bbdc512bb02a002a473db501c3b097c8",
        63,
    ),
];
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn yaml(path: &str) -> Value {
    serde_yaml::from_str(&fs::read_to_string(root().join(path)).expect(path)).expect(path)
}
fn json(path: &str) -> Json {
    serde_json::from_str(&fs::read_to_string(root().join(path)).expect(path)).expect(path)
}
fn at<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(v, |c, k| {
        c.get(*k).unwrap_or_else(|| panic!("missing DATA key {k}"))
    })
}
fn jat<'a>(v: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(v, |c, k| {
        c.get(*k).unwrap_or_else(|| panic!("missing JSON key {k}"))
    })
}
fn strings(v: &Value, path: &[&str]) -> Vec<String> {
    at(v, path)
        .as_sequence()
        .expect("DATA sequence")
        .iter()
        .map(|x| x.as_str().expect("DATA string").to_owned())
        .collect()
}
fn bad(errors: &mut Vec<String>, condition: bool, message: &str) {
    if !condition {
        errors.push(message.into());
    }
}

fn validate_manifest(
    m: &Value,
    oracle: &Value,
    java: &Json,
    lattice: &Value,
    layout: &Json,
) -> Vec<String> {
    let mut e = Vec::new();
    bad(
        &mut e,
        at(m, &["schema_version"]).as_str() == Some("document-segmentation.v2"),
        "[schema] unknown segmentation schema",
    );
    bad(
        &mut e,
        at(m, &["compatible_with"]).as_str() == Some("document-segmentation.v1"),
        "[schema] compatibility lost",
    );
    for (p, x) in [
        (vec!["policy_locks", "observation_mode"], "shadow"),
        (vec!["policy_locks", "promotion_state"], "candidate"),
        (
            vec!["policy_locks", "production_authority"],
            "java-grobid-http@0.9.0-crf",
        ),
        (vec!["policy_locks", "auto_promote"], "forbidden"),
        (vec!["policy_locks", "winner_selection"], "forbidden"),
        (vec!["policy_locks", "parser_port_routing"], "forbidden"),
    ] {
        bad(
            &mut e,
            at(m, &p).as_str() == Some(x),
            "[policy-lock] production lock changed",
        );
    }
    for p in [
        ["policy_locks", "import_eligible"],
        ["policy_locks", "graph_mutation"],
        ["policy_locks", "promotion"],
    ] {
        let v = at(m, &p).as_str();
        bad(
            &mut e,
            v == Some("forbidden")
                || (p[1] == "import_eligible" && at(m, &p).as_bool() == Some(false)),
            "[policy-lock] import/graph/promotion lock changed",
        );
    }
    bad(
        &mut e,
        strings(m, &["regions", "closed_labels"]) == REGIONS,
        "[unknown-region] closed region vocabulary changed",
    );
    bad(
        &mut e,
        strings(m, &["furniture", "closed_subtypes"]) == FURNITURE,
        "[unknown-furniture-subtype] furniture vocabulary changed",
    );
    bad(
        &mut e,
        strings(m, &["lanes", "required_ids"]) == LANES,
        "[unknown-lane] lane vocabulary changed",
    );
    for (p, x) in [
        (vec!["source_corpus", "corpus_n"], 3),
        (vec!["source_corpus", "page_sum"], 202),
        (vec!["lanes", "pages_per_lane"], 202),
        (vec!["lanes", "total_lane_pages"], 606),
        (vec!["evaluation", "required_rows"], 81),
    ] {
        bad(
            &mut e,
            at(m, &p).as_i64() == Some(x),
            "[denominator-erosion] exact denominator changed",
        );
    }
    bad(
        &mut e,
        strings(m, &["evaluation", "matrix_axes"])
            == ["document", "lane", "source_bound_genre", "region"],
        "[matrix] axes are incomplete or reordered",
    );
    bad(
        &mut e,
        at(m, &["evaluation", "row_formula"]).as_str()
            == Some("3_documents_x_3_lanes_x_1_reviewed_genre_x_9_regions"),
        "[matrix] row formula changed",
    );
    bad(
        &mut e,
        at(m, &["evaluation", "pooled_metrics"]).as_str() == Some("forbidden"),
        "[pooled-metrics] pooling must be forbidden",
    );
    bad(
        &mut e,
        at(m, &["source_bound_genres", "stratification_only"]).as_bool() == Some(true)
            && at(m, &["source_bound_genres", "classifier_branching"]).as_str()
                == Some("forbidden")
            && at(m, &["source_bound_genres", "reviewed_annotation_required"]).as_bool()
                == Some(true),
        "[genre-policy] genre is not stratification-only",
    );
    bad(
        &mut e,
        strings(m, &["regions", "lexical_token_kinds"])
            == ["Word", "Whitespace", "Punctuation", "Symbol"],
        "[lexical-kind] lexical vocabulary changed",
    );
    bad(
        &mut e,
        strings(m, &["regions", "visual_evidence_kinds"]) == ["Image", "Formula"],
        "[visual-kind] visual evidence vocabulary changed",
    );
    bad(
        &mut e,
        at(m, &["regions", "visual_lexical_assignment"]).as_str() == Some("forbidden")
            && at(m, &["regions", "no_classifier_branch_by_genre"]).as_bool() == Some(true),
        "[mapping] implicit/default mapping allowed",
    );
    bad(
        &mut e,
        at(m, &["furniture", "excluded_from_body_reading_order"]).as_bool() == Some(true)
            && at(m, &["furniture", "sequence"]).as_str()
                == Some("ordered_annotation_separate_from_body_reading_order"),
        "[furniture-in-body] furniture order is not separate",
    );
    bad(
        &mut e,
        strings(m, &["annotations", "identity_fields"]).contains(&"lattice_fingerprint".into())
            && strings(m, &["annotations", "boundary_fields"]).contains(&"boundary_digest".into()),
        "[annotation-schema] identity/boundary fields missing",
    );
    bad(
        &mut e,
        at(m, &["source_contract", "lattice_authority"]).as_str() == Some(LATTICE)
            && at(m, &["source_contract", "layout_authority"]).as_str() == Some(LAYOUT),
        "[coordinate-binding] S01 authorities changed",
    );
    bad(
        &mut e,
        at(m, &["source_contract", "java_revision"]).as_str()
            == Some("0b0ab53b159b32792a01fca1d9d04fd02cf8b24b"),
        "[source-identity] Java revision changed",
    );
    let docs = at(m, &["source_corpus", "documents"])
        .as_sequence()
        .unwrap();
    bad(
        &mut e,
        docs.len() == 3,
        "[source-binding] document denominator eroded",
    );
    for ((id, sha, pages), d) in DOCUMENTS.iter().zip(docs) {
        bad(
            &mut e,
            d["id"].as_str() == Some(*id)
                && d["source_sha256"].as_str() == Some(*sha)
                && d["pages"].as_i64() == Some(*pages),
            "[source-binding] document identity drifted",
        );
    }
    bad(
        &mut e,
        jat(java, &["schema_version"]).as_str() == Some("java-grobid-body-taxonomy-source.v2")
            && jat(java, &["vendor_revision"]).as_str()
                == Some("0b0ab53b159b32792a01fca1d9d04fd02cf8b24b"),
        "[java-source] taxonomy identity changed",
    );
    bad(
        &mut e,
        jat(java, &["s02_output_mapping", "closed_outputs"])
            .as_array()
            .map(|x| x.len())
            == Some(9),
        "[java-source] output family incomplete",
    );
    for name in REGIONS {
        bad(
            &mut e,
            java["s02_output_mapping"]["closed_outputs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x == name),
            "[java-source] closed output missing",
        );
    }
    bad(
        &mut e,
        jat(
            java,
            &[
                "s02_output_mapping",
                "deferred",
                "conflict",
                "implicit_mapping",
            ],
        )
        .as_str()
            == Some("forbidden")
            && jat(
                java,
                &[
                    "s02_output_mapping",
                    "deferred",
                    "contribution",
                    "implicit_mapping",
                ],
            )
            .as_str()
                == Some("forbidden"),
        "[deferred] conflict/contribution may map implicitly",
    );
    bad(
        &mut e,
        at(lattice, &["schema_version"]).as_str() == Some("layout-lattice-parity.v1")
            && jat(layout, &["schema_version"]).as_str() == Some("java-grobid-layout-source.v1"),
        "[S01] lattice/layout authority mismatch",
    );
    validate_oracle(oracle, &mut e);
    e
}
fn validate_oracle(o: &Value, e: &mut Vec<String>) {
    bad(
        e,
        at(o, &["schema_version"]).as_str() == Some("segmentation-oracle.v1"),
        "[oracle-schema] wrong oracle schema",
    );
    let f = strings(o, &["forbidden_fields"]);
    for x in [
        "raw_text",
        "pdf_bytes",
        "tei_xml",
        "page_image",
        "physical_path",
        "hostname",
        "executable",
    ] {
        bad(
            e,
            f.iter().any(|v| v == x),
            "[redaction] forbidden raw field removed",
        );
    }
    let mut keys = BTreeSet::new();
    let receipts = at(o, &["receipts"]).as_sequence().unwrap();
    bad(
        e,
        receipts.len() == 9,
        "[oracle-coverage] expected nine document/lane receipts",
    );
    for r in receipts {
        let key = format!(
            "{}/{}",
            r["document_id"].as_str().unwrap_or(""),
            r["lane_id"].as_str().unwrap_or("")
        );
        bad(
            e,
            keys.insert(key),
            "[duplicate-oracle-key] duplicate receipt",
        );
        let t = &r["token_receipts"][0];
        bad(
            e,
            REGIONS.iter().any(|x| t["region"].as_str() == Some(*x)),
            "[unknown-region] oracle region not closed",
        );
    }
}
#[test]
fn s02_data_contract_is_complete_source_bound_and_redacted() {
    let m = yaml(MANIFEST);
    let errors = validate_manifest(
        &m,
        &yaml(ORACLE),
        &json(JAVA),
        &yaml(LATTICE),
        &json(LAYOUT),
    );
    assert!(
        errors.is_empty(),
        "S02 DATA violations:\n{}",
        errors.join("\n")
    );
}
#[test]
fn rejects_every_closed_vocabulary_and_identity_mutation() {
    for (path, key) in [
        ("regions/closed_labels", "regions"),
        ("furniture/closed_subtypes", "furniture"),
        ("regions/lexical_token_kinds", "lexical"),
    ] {
        let _ = (path, key);
        let mut m = yaml(MANIFEST);
        let target = if key == "regions" {
            &mut m["regions"]["closed_labels"]
        } else if key == "furniture" {
            &mut m["furniture"]["closed_subtypes"]
        } else {
            &mut m["regions"]["lexical_token_kinds"]
        };
        target.as_sequence_mut().unwrap().pop();
        assert!(
            !validate_manifest(
                &m,
                &yaml(ORACLE),
                &json(JAVA),
                &yaml(LATTICE),
                &json(LAYOUT)
            )
            .is_empty()
        );
    }
    let mut m = yaml(MANIFEST);
    m["source_contract"]["java_revision"] = Value::String("drift".into());
    assert!(
        !validate_manifest(
            &m,
            &yaml(ORACLE),
            &json(JAVA),
            &yaml(LATTICE),
            &json(LAYOUT)
        )
        .is_empty()
    );
}
#[test]
fn rejects_default_body_furniture_visual_genre_denominator_and_policy_unlocks() {
    let mut m = yaml(MANIFEST);
    m["regions"]["visual_lexical_assignment"] = Value::String("body".into());
    assert!(
        !validate_manifest(
            &m,
            &yaml(ORACLE),
            &json(JAVA),
            &yaml(LATTICE),
            &json(LAYOUT)
        )
        .is_empty()
    );
    let mut m = yaml(MANIFEST);
    m["furniture"]["excluded_from_body_reading_order"] = Value::Bool(false);
    assert!(
        !validate_manifest(
            &m,
            &yaml(ORACLE),
            &json(JAVA),
            &yaml(LATTICE),
            &json(LAYOUT)
        )
        .is_empty()
    );
    let mut m = yaml(MANIFEST);
    m["evaluation"]["required_rows"] = Value::Number(80.into());
    assert!(
        !validate_manifest(
            &m,
            &yaml(ORACLE),
            &json(JAVA),
            &yaml(LATTICE),
            &json(LAYOUT)
        )
        .is_empty()
    );
    let mut m = yaml(MANIFEST);
    m["policy_locks"]["import_eligible"] = Value::Bool(true);
    assert!(
        !validate_manifest(
            &m,
            &yaml(ORACLE),
            &json(JAVA),
            &yaml(LATTICE),
            &json(LAYOUT)
        )
        .is_empty()
    );
    let mut o = yaml(ORACLE);
    o["forbidden_fields"]
        .as_sequence_mut()
        .unwrap()
        .retain(|x| x.as_str() != Some("raw_text"));
    assert!(
        !validate_manifest(
            &yaml(MANIFEST),
            &o,
            &json(JAVA),
            &yaml(LATTICE),
            &json(LAYOUT)
        )
        .is_empty()
    );
}
