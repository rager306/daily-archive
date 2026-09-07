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

#[test]
fn independent_real_oracle_binds_all_nine_reviewed_receipts() {
    let fixture: Value = serde_yaml::from_str(FIXTURE).expect("oracle fixture parses");
    let source_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SOURCE);
    let source: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(source_path).expect("source oracle exists"))
            .expect("source oracle parses");
    let receipts = fixture["receipts"].as_sequence().expect("receipts list");
    assert_eq!(receipts.len(), EXPECTED_KEYS.len());
    let source_rows = source["receipts"].as_array().expect("source receipts list");
    assert_eq!(source_rows.len(), receipts.len());
    let mut keys = HashSet::new();

    for (fixture_row, source_row) in receipts.iter().zip(source_rows) {
        let row = fixture_row.as_mapping().expect("receipt mapping");
        let document_id = text(row, "document_id");
        let lane = text(row, "lane_id");
        assert!(
            keys.insert((document_id.to_owned(), lane.to_owned())),
            "duplicate receipt"
        );
        assert_eq!(source_row["document_id"].as_str(), Some(document_id));
        assert_eq!(source_row["lane"].as_str(), Some(lane));
        assert_eq!(
            source_row["source_sha256"].as_str(),
            Some(text(row, "source_sha256"))
        );
        assert_eq!(
            source_row["page_count"].as_u64(),
            row[Value::String("page_count".into())].as_u64()
        );
        assert_eq!(
            row[Value::String("admitted_token_denominator".into())].as_u64(),
            Some(1)
        );
        assert_eq!(
            row[Value::String("page_ordinals".into())]
                .as_sequence()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(source_row["status"].as_str(), Some("pass"));
        digest(text(row, "source_sha256"));
        let token = &row[Value::String("token_receipts".into())]
            .as_sequence()
            .unwrap()[0];
        let token = token.as_mapping().unwrap();
        digest(text(token, "token_digest"));
        digest(text(token, "boundary_digest"));
        let section = &row[Value::String("section_receipts".into())]
            .as_sequence()
            .unwrap()[0];
        digest(text(
            section.as_mapping().unwrap(),
            "section_boundary_digest",
        ));
    }
    let expected: HashSet<_> = EXPECTED_KEYS
        .iter()
        .map(|(document, lane)| ((*document).to_owned(), (*lane).to_owned()))
        .collect();
    assert_eq!(keys, expected);
}
