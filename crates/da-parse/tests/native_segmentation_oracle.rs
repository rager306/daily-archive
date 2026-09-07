use serde_yaml::Value;
use std::collections::HashSet;

const FIXTURE: &str = include_str!("fixtures/segmentation_oracle.yaml");
const SOURCE: &str = "artifacts/parse_compat/document_segmentation_oracle_source.json";
const EXPECTED_KEYS: [(&str, &str); 9] = [
    ("hub-2507.19457", "pdfalto"),
    ("hub-2507.19457", "pdf_inspector_positioned"),
    ("hub-2507.19457", "pdf_inspector_positioned_xy_cut"),
    ("rlm-2512.24601", "pdfalto"),
    ("rlm-2512.24601", "pdf_inspector_positioned"),
    ("rlm-2512.24601", "pdf_inspector_positioned_xy_cut"),
    ("reca-2605.26525v1", "pdfalto"),
    ("reca-2605.26525v1", "pdf_inspector_positioned"),
    ("reca-2605.26525v1", "pdf_inspector_positioned_xy_cut"),
];

fn text<'a>(map: &'a serde_yaml::Mapping, key: &str) -> &'a str {
    map.get(Value::String(key.into()))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string field {key}"))
}

fn digest(value: &str) {
    assert_eq!(value.len(), 64, "digest must be SHA-256: {value}");
    assert!(
        value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    assert!(
        value.bytes().collect::<HashSet<_>>().len() > 1,
        "placeholder digest: {value}"
    );
}

fn validate_oracle(fixture: &Value, source: &serde_json::Value) -> Result<(), String> {
    let receipts = fixture["receipts"]
        .as_sequence()
        .ok_or_else(|| "missing oracle rows".to_owned())?;
    if receipts.len() != EXPECTED_KEYS.len() {
        return Err(format!("expected nine oracle rows, got {}", receipts.len()));
    }
    let source_rows = source["receipts"]
        .as_array()
        .ok_or_else(|| "missing source rows".to_owned())?;
    if source_rows.len() != receipts.len() {
        return Err("source and fixture row counts differ".to_owned());
    }
    let expected: HashSet<_> = EXPECTED_KEYS
        .iter()
        .map(|(document, lane)| ((*document).to_owned(), (*lane).to_owned()))
        .collect();
    let mut keys = HashSet::new();
    let regions = ["header", "body", "references", "annex", "page_furniture"];

    for (fixture_row, source_row) in receipts.iter().zip(source_rows) {
        let row = fixture_row
            .as_mapping()
            .ok_or_else(|| "receipt is not a mapping".to_owned())?;
        let document_id = text(row, "document_id");
        let lane = text(row, "lane_id");
        if !keys.insert((document_id.to_owned(), lane.to_owned())) {
            return Err("duplicate oracle key".to_owned());
        }
        if !expected.contains(&(document_id.to_owned(), lane.to_owned())) {
            return Err("unknown document or lane".to_owned());
        }
        if source_row["document_id"].as_str() != Some(document_id)
            || source_row["lane"].as_str() != Some(lane)
            || source_row["source_sha256"].as_str() != Some(text(row, "source_sha256"))
            || source_row["page_count"].as_u64() != row[Value::String("page_count".into())].as_u64()
        {
            return Err("source identity or checksum mismatch".to_owned());
        }
        if source_row["status"].as_str() != Some("pass") {
            return Err("terminal or unsupported source outcome".to_owned());
        }
        let pages = row[Value::String("page_ordinals".into())]
            .as_sequence()
            .ok_or_else(|| "missing page ordinals".to_owned())?;
        let tokens = row[Value::String("token_receipts".into())]
            .as_sequence()
            .ok_or_else(|| "missing token receipts".to_owned())?;
        if row[Value::String("admitted_token_denominator".into())].as_u64()
            != Some(tokens.len() as u64)
        {
            return Err("admitted-token denominator drift".to_owned());
        }
        let mut previous = None;
        for token in tokens {
            let token = token
                .as_mapping()
                .ok_or_else(|| "token receipt is not a mapping".to_owned())?;
            let page = token[Value::String("page_ordinal".into())]
                .as_u64()
                .ok_or_else(|| "missing token page ordinal".to_owned())?;
            let ordinal = token[Value::String("token_ordinal".into())]
                .as_u64()
                .ok_or_else(|| "missing token ordinal".to_owned())?;
            if !pages.iter().any(|p| p.as_u64() == Some(page)) {
                return Err("token page is outside page denominator".to_owned());
            }
            if previous.is_some_and(|previous| (page, ordinal) <= previous) {
                return Err("non-monotonic token order".to_owned());
            }
            previous = Some((page, ordinal));
            let region = text(token, "region");
            if !regions.contains(&region) {
                return Err("unknown region".to_owned());
            }
            digest(text(token, "token_digest"));
            digest(text(token, "boundary_digest"));
        }
        let sections = row[Value::String("section_receipts".into())]
            .as_sequence()
            .ok_or_else(|| "missing section receipts".to_owned())?;
        let mut previous_section = None;
        for section in sections {
            let section = section
                .as_mapping()
                .ok_or_else(|| "section receipt is not a mapping".to_owned())?;
            let order = section[Value::String("order_ordinal".into())]
                .as_u64()
                .ok_or_else(|| "missing section order".to_owned())?;
            if previous_section.is_some_and(|previous| order <= previous) {
                return Err("non-monotonic section order".to_owned());
            }
            previous_section = Some(order);
            if !["body", "annex"].contains(&text(section, "parent_region")) {
                return Err("section has unknown parent region".to_owned());
            }
            digest(text(section, "section_boundary_digest"));
        }
    }
    if keys != expected {
        return Err("missing oracle row".to_owned());
    }
    Ok(())
}

fn load_inputs() -> (Value, serde_json::Value) {
    let fixture: Value = serde_yaml::from_str(FIXTURE).expect("oracle fixture parses");
    let source_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SOURCE);
    let source: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(source_path).expect("source oracle exists"))
            .expect("source oracle parses");
    (fixture, source)
}

#[test]
fn independent_real_oracle_binds_all_nine_reviewed_receipts() {
    let (fixture, source) = load_inputs();
    validate_oracle(&fixture, &source).unwrap();
}

#[test]
fn oracle_rejects_missing_duplicate_checksum_region_denominator_and_order_drift() {
    let (fixture, source) = load_inputs();

    let mut missing = fixture.clone();
    missing["receipts"].as_sequence_mut().unwrap().pop();
    assert!(
        validate_oracle(&missing, &source)
            .unwrap_err()
            .contains("nine")
    );

    let mut duplicate = fixture.clone();
    let first = duplicate["receipts"].as_sequence().unwrap()[0].clone();
    *duplicate["receipts"]
        .as_sequence_mut()
        .unwrap()
        .last_mut()
        .unwrap() = first;
    assert!(
        validate_oracle(&duplicate, &source)
            .unwrap_err()
            .contains("duplicate")
    );

    let mut checksum = fixture.clone();
    checksum["receipts"][0]["source_sha256"] = Value::String("f".repeat(64));
    assert!(
        validate_oracle(&checksum, &source)
            .unwrap_err()
            .contains("checksum")
    );

    let mut region = fixture.clone();
    region["receipts"][0]["token_receipts"][0]["region"] = Value::String("figure".into());
    assert!(
        validate_oracle(&region, &source)
            .unwrap_err()
            .contains("region")
    );

    let mut denominator = fixture.clone();
    denominator["receipts"][0]["admitted_token_denominator"] = Value::Number(2.into());
    assert!(
        validate_oracle(&denominator, &source)
            .unwrap_err()
            .contains("denominator")
    );

    let mut order = fixture.clone();
    order["receipts"][0]["token_receipts"][0]["token_ordinal"] = Value::Number(2.into());
    let repeated_token = order["receipts"][0]["token_receipts"][0].clone();
    order["receipts"][0]["token_receipts"]
        .as_sequence_mut()
        .unwrap()
        .push(repeated_token);
    order["receipts"][0]["admitted_token_denominator"] = Value::Number(2.into());
    assert!(
        validate_oracle(&order, &source)
            .unwrap_err()
            .contains("monotonic")
    );
}

#[test]
fn caption_like_text_is_not_a_section_boundary() {
    let (fixture, _) = load_inputs();
    let row = &fixture["receipts"][0];
    let section = row["section_receipts"].as_sequence().unwrap();
    assert!(section.iter().all(|s| s["parent_region"] != "caption"));
}
