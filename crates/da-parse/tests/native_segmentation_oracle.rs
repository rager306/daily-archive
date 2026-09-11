use serde_yaml::Value;
use sha2::{Digest as ShaDigest, Sha256};
use std::collections::HashSet;

const FIXTURE: &str = include_str!("fixtures/segmentation_oracle.yaml");
const SOURCE: &str = "artifacts/parse_compat/document_segmentation_oracle_source.json";
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
const EXPECTED: [(&str, &str, u64); 9] = [
    ("hub-2507.19457", "pdfalto", 96),
    ("hub-2507.19457", "pdf_inspector_positioned", 96),
    ("hub-2507.19457", "pdf_inspector_positioned_xy_cut", 96),
    ("rlm-2512.24601", "pdfalto", 43),
    ("rlm-2512.24601", "pdf_inspector_positioned", 43),
    ("rlm-2512.24601", "pdf_inspector_positioned_xy_cut", 43),
    ("reca-2605.26525v1", "pdfalto", 63),
    ("reca-2605.26525v1", "pdf_inspector_positioned", 63),
    ("reca-2605.26525v1", "pdf_inspector_positioned_xy_cut", 63),
];

fn digest(value: &Value, field: &str) -> Result<(), String> {
    let text = value.as_str().ok_or_else(|| format!("missing {field}"))?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || text.chars().all(|c| c == '0')
    {
        return Err(format!("invalid placeholder {field}"));
    }
    Ok(())
}

fn canonical_sha256(bytes: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes.as_bytes()))
}

fn source_digest(source_row: &serde_json::Value, field: &str) -> Result<String, String> {
    let artifact = source_row[field]
        .as_object()
        .ok_or_else(|| format!("missing canonical {field}"))?;
    let bytes = artifact["bytes"]
        .as_str()
        .ok_or_else(|| format!("missing canonical {field}.bytes"))?;
    let declared = artifact["sha256"]
        .as_str()
        .ok_or_else(|| format!("missing canonical {field}.sha256"))?;
    let computed = canonical_sha256(bytes);
    if declared != computed {
        return Err(format!("canonical {field} digest is not recomputable"));
    }
    Ok(computed)
}

fn validate_oracle(fixture: &Value, source: &serde_json::Value) -> Result<(), String> {
    if fixture["schema_version"].as_str() != Some("segmentation-oracle.v2")
        || fixture["redaction"].as_str() != Some("digest_only_source_bound_ordinals")
    {
        return Err("wrong oracle schema or redaction".into());
    }
    if fixture["genre"]["provenance"].as_str() != Some("source_bound_reviewed")
        || fixture["genre"]["use"].as_str() != Some("stratification_only")
    {
        return Err("genre is not reviewed stratification".into());
    }
    let rows = fixture["receipts"]
        .as_sequence()
        .ok_or("missing receipts")?;
    let source_rows = source["receipts"]
        .as_array()
        .ok_or("missing source receipts")?;
    if rows.len() != 9 || source_rows.len() != 9 {
        return Err("expected nine oracle rows".into());
    }
    let mut keys = HashSet::new();
    if source["schema_version"].as_str() != Some("document-segmentation-oracle-source.v3") {
        return Err("source does not publish canonicalization v3".into());
    }
    if source["canonicalization"]["encoding"].as_str() != Some("utf-8")
        || source["canonicalization"]["line_endings"].as_str() != Some("LF")
        || source["canonicalization"]["trailing_newline"].as_bool() != Some(false)
        || source["canonicalization"]["digest"].as_str()
            != Some("sha256 over exact canonical bytes")
    {
        return Err("canonicalization contract is incomplete".into());
    }
    for (expected, (row, source_row)) in EXPECTED.iter().zip(rows.iter().zip(source_rows)) {
        let key = (
            row["document_id"].as_str().ok_or("missing document")?,
            row["lane_id"].as_str().ok_or("missing lane")?,
        );
        if key != (expected.0, expected.1) || !keys.insert(key) {
            return Err("duplicate or unknown oracle key".into());
        }
        if row["page_count"].as_u64() != Some(expected.2)
            || source_row["document_id"].as_str() != Some(expected.0)
            || source_row["lane"].as_str() != Some(expected.1)
            || source_row["page_count"].as_u64() != Some(expected.2)
            || source_row["source_sha256"].as_str() != row["source_sha256"].as_str()
            || source_row["status"].as_str() != Some("pass")
        {
            return Err("source identity or terminal outcome mismatch".into());
        }
        let ranges = row["page_receipt_ranges"]
            .as_sequence()
            .ok_or("missing page ranges")?;
        if ranges.is_empty()
            || ranges.iter().any(|range| {
                range["start"].as_u64() != Some(1)
                    || range["end"].as_u64() != Some(expected.2)
                    || digest(&range["receipt_digest"], "receipt_digest").is_err()
            })
        {
            return Err(format!(
                "page denominator or page receipt drift for {key:?}: {ranges:?}"
            ));
        }
        let denominator = source_row["denominator"]
            .as_object()
            .ok_or("missing typed denominator")?;
        if denominator["kind"].as_str() != Some("measured")
            || denominator["source"].as_str().is_none()
            || denominator["value"].as_u64().is_none()
            || row["admitted_lexical_denominator"].as_u64() != denominator["value"].as_u64()
            || source_row["proxy_denominator"]["kind"].as_str() != Some("proxy")
            || source_row["proxy_denominator"]["formula"].as_str() != Some("page_count × 10")
            || source_row["proxy_denominator"]["value"].as_u64() != Some(expected.2 * 10)
        {
            return Err("denominator is formula-derived or unbound".into());
        }
        let page_digest = source_digest(source_row, "canonical_page_range")?;
        let annotation_digest = source_digest(source_row, "canonical_annotation")?;
        if source_row["page_range_digest"].as_str() != Some(page_digest.as_str())
            || source_row["annotation_digest"].as_str() != Some(annotation_digest.as_str())
            || row["page_receipt_ranges"][0]["receipt_digest"].as_str()
                != Some(page_digest.trim_start_matches("sha256:"))
            || row["annotation_digest"].as_str()
                != Some(annotation_digest.trim_start_matches("sha256:"))
        {
            return Err("reviewer recomputation mismatch".into());
        }
        let runs = row["region_runs"]
            .as_sequence()
            .ok_or("missing region runs")?;
        if runs.len() != REGIONS.len() {
            return Err("region annotation set incomplete".into());
        }
        let mut seen = HashSet::new();
        let mut previous_start: Option<Vec<u64>> = None;
        for run in runs {
            let region = run["region"].as_str().ok_or("missing region")?;
            if !REGIONS.contains(&region) || !seen.insert(region) {
                return Err("unknown or duplicate region".into());
            }
            if run["absence"].as_str() == Some("reviewed_absent") {
                if run["start"].is_null() || run["end"].is_null() {
                    continue;
                }
                return Err("absence has a range".into());
            }
            digest(&run["boundary_digest"], "boundary_digest")?;
            let start = run["start"].as_sequence().ok_or("missing range start")?;
            let end = run["end"].as_sequence().ok_or("missing range end")?;
            if start.len() != 4
                || end.len() != 4
                || start.iter().any(|x| x.as_u64().is_none())
                || end.iter().any(|x| x.as_u64().is_none())
            {
                return Err("invalid boundary range".into());
            }
            let start = start
                .iter()
                .map(|x| x.as_u64().unwrap())
                .collect::<Vec<_>>();
            let end = end.iter().map(|x| x.as_u64().unwrap()).collect::<Vec<_>>();
            if start > end
                || (region != "page_furniture"
                    && previous_start
                        .as_ref()
                        .is_some_and(|previous| start <= *previous))
            {
                return Err("non-monotonic region boundary".into());
            }
            previous_start = Some(start);
        }
        if seen.len() != REGIONS.len() {
            return Err("missing region annotation".into());
        }
        let furniture = row["furniture_sequence"]
            .as_sequence()
            .ok_or("missing furniture sequence")?;
        if furniture.is_empty()
            || furniture.iter().any(|item| {
                !FURNITURE.contains(&item["subtype"].as_str().unwrap_or(""))
                    || item["evidence"].as_str() != Some("reviewed_digest_bound")
            })
        {
            return Err("invalid furniture evidence".into());
        }
    }
    Ok(())
}

fn load_inputs() -> (Value, serde_json::Value) {
    let fixture = serde_yaml::from_str(FIXTURE).expect("fixture parses");
    let source_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SOURCE);
    let source =
        serde_json::from_str(&std::fs::read_to_string(source_path).expect("source exists"))
            .expect("source parses");
    (fixture, source)
}

#[test]
fn oracle_reconstruction_binds_all_nine_reviewed_receipts() {
    let (fixture, source) = load_inputs();
    validate_oracle(&fixture, &source).unwrap();
}

#[test]
fn oracle_reconstruction_keeps_page_accounting_unpooled_and_replay_stable() {
    let (fixture, source) = load_inputs();
    let pages = fixture["receipts"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|row| row["page_count"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(pages, 606);
    assert_eq!(source["receipts"].as_array().unwrap().len(), 9);
    assert_eq!(
        serde_yaml::to_string(&fixture).unwrap(),
        serde_yaml::to_string(&fixture).unwrap()
    );
    assert!(source["receipts"].as_array().unwrap().iter().all(|row| {
        row["denominator"]["kind"] == "measured"
            && row["denominator"]["value"].as_u64().unwrap() > 0
    }));
}

#[test]
fn oracle_reconstruction_keeps_reviewed_absence_and_furniture_outside_body_order() {
    let (fixture, _) = load_inputs();
    for row in fixture["receipts"].as_sequence().unwrap() {
        let runs = row["region_runs"].as_sequence().unwrap();
        assert_eq!(runs.len(), 9);
        assert!(runs.iter().any(|run| run["absence"] == "reviewed_absent"));
        assert!(!row["furniture_sequence"].as_sequence().unwrap().is_empty());
    }
}

#[test]
fn oracle_reconstruction_rejects_adversarial_mutations() {
    let (fixture, source) = load_inputs();
    let mut missing = fixture.clone();
    missing["receipts"].as_sequence_mut().unwrap().pop();
    assert!(validate_oracle(&missing, &source).is_err());

    let mut duplicate = fixture.clone();
    let first = duplicate["receipts"].as_sequence().unwrap()[0].clone();
    *duplicate["receipts"]
        .as_sequence_mut()
        .unwrap()
        .last_mut()
        .unwrap() = first;
    assert!(validate_oracle(&duplicate, &source).is_err());

    for (field, value) in [
        ("source_sha256", Value::String("f".repeat(64))),
        ("annotation_digest", Value::String("0".repeat(64))),
        ("admitted_lexical_denominator", Value::Number(1.into())),
    ] {
        let mut mutation = fixture.clone();
        mutation["receipts"][0][field] = value;
        assert!(
            validate_oracle(&mutation, &source).is_err(),
            "{field} mutation accepted"
        );
    }
    let mut unknown_region = fixture.clone();
    unknown_region["receipts"][0]["region_runs"][0]["region"] = Value::String("figure".into());
    assert!(validate_oracle(&unknown_region, &source).is_err());

    let mut overlap = fixture.clone();
    overlap["receipts"][0]["region_runs"][1]["start"] =
        Value::Sequence(vec![0.into(), 0.into(), 0.into(), 0.into()]);
    assert!(
        validate_oracle(&overlap, &source)
            .unwrap_err()
            .contains("boundary")
    );

    let mut furniture = fixture.clone();
    furniture["receipts"][0]["furniture_sequence"][0]["subtype"] = Value::String("body".into());
    assert!(validate_oracle(&furniture, &source).is_err());

    let mut canonical_bytes = source.clone();
    canonical_bytes["receipts"][0]["canonical_page_range"]["bytes"] = serde_json::Value::String(
        "document_id=hub-2507.19457\nlane=pdfalto\npage_range=1-97".into(),
    );
    assert!(
        validate_oracle(&fixture, &canonical_bytes)
            .unwrap_err()
            .contains("recomputable")
    );
}
