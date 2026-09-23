//! S16 DATA contract: three independent visual heads and a digest-only PDF oracle.
//! Expected rows live in a reviewed fixture and are deliberately not produced by
//! da-parse or any layout donor helper.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use da_parse::{
    NATIVE_VISUAL_SCHEMA_VERSION, NATIVE_VISUAL_TYPED_ABSENCE_ADR_REF,
    NATIVE_VISUAL_TYPED_ABSENCE_CONTRACT_ID, NATIVE_VISUAL_TYPED_ABSENCE_DECLARED_DENOMINATOR,
    NATIVE_VISUAL_TYPED_ABSENCE_DENOMINATOR_FIELD, NATIVE_VISUAL_TYPED_ABSENCE_EMPTY_POPULATION,
    NATIVE_VISUAL_TYPED_ABSENCE_MISSING_DENOMINATOR,
    NATIVE_VISUAL_TYPED_ABSENCE_OBSERVED_POPULATION, NATIVE_VISUAL_TYPED_ABSENCE_POOLING,
    NATIVE_VISUAL_TYPED_ABSENCE_REASON_FIELD, NATIVE_VISUAL_TYPED_ABSENCE_TERMINAL_IS_PASS,
    NATIVE_VISUAL_TYPED_ABSENCE_UNITY_DENOMINATOR, NATIVE_VISUAL_TYPED_ABSENCE_UNTYPED_EMPTY_SHAPE,
    NATIVE_VISUAL_TYPED_ABSENCE_USED_BY, NATIVE_VISUAL_TYPED_ABSENCE_VOCABULARY,
    NativeVisualAbsence, NativeVisualConfig, NativeVisualHeadOutcome, NativeVisualKind,
    NativeVisualObservation, NativeVisualPrimitive, NativeVisualProvider, TYPED_ABSENCE_TAXONOMY,
    detect_figure_heads, detect_formula_heads, detect_table_heads,
};
use serde_yaml::Value;

const MANIFEST: &str = "data/parse/native_visual_heads.yaml";
const ORACLE: &str = "crates/da-parse/tests/fixtures/native_visual_oracle.yaml";
const S14: &str = "data/parse/layout_bakeoff.yaml";
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const HEADS: [&str; 3] = ["figure", "table", "formula"];
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
const FORBIDDEN: [&str; 10] = [
    "raw_text",
    "raw_pdf",
    "pdf_bytes",
    "tei_xml",
    "image",
    "page_image",
    "assets",
    "physical_path",
    "hostname",
    "executable",
];
/// The closed absence vocabulary, taken from the detector's own taxonomy rather
/// than re-typed here: renaming or adding a variant fails
/// `native_visual_typed_absence` and this DATA gate together.
fn absence_tokens() -> Vec<String> {
    TYPED_ABSENCE_TAXONOMY
        .iter()
        .map(|reason| reason.token().to_owned())
        .collect()
}

fn is_absence(status: &str) -> bool {
    TYPED_ABSENCE_TAXONOMY
        .iter()
        .any(|reason| reason.token() == status)
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn yaml(path: &str) -> Value {
    serde_yaml::from_str(&fs::read_to_string(root().join(path)).expect(path)).expect(path)
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(value, |current, key| {
        current
            .get(*key)
            .unwrap_or_else(|| panic!("missing DATA key {key}"))
    })
}

fn strings(value: &Value, path: &[&str]) -> Vec<String> {
    at(value, path)
        .as_sequence()
        .expect("DATA sequence")
        .iter()
        .map(|item| item.as_str().expect("DATA string").to_owned())
        .collect()
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

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        && value.bytes().collect::<BTreeSet<_>>().len() > 1
}

fn validate(manifest: &Value, oracle: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if at(manifest, &["schema_version"]).as_str() != Some("native-visual-heads.v1") {
        errors.push("[schema] unknown native visual schema".into());
    }
    if at(manifest, &["source_authority", "manifest"]).as_str() != Some(S14)
        || at(manifest, &["source_authority", "exact_sha256_binding"]).as_str() != Some("required")
    {
        errors.push("[source-authority] S14 identity/topology authority drifted".into());
    }
    if strings(manifest, &["lanes", "required_ids"])
        != LANES
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>()
    {
        errors.push("[unknown-lane] lane vocabulary changed".into());
    }
    if strings(manifest, &["heads", "closed_labels"])
        != HEADS
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>()
    {
        errors.push("[unknown-head] head vocabulary changed".into());
    }
    for (path, expected) in [
        (&["source_authority", "exact_document_count"][..], 3),
        (&["source_authority", "page_denominator"][..], 202),
        (&["source_authority", "lane_page_denominator"][..], 606),
        (&["lanes", "pages_per_lane"][..], 202),
        (&["lanes", "total_lane_pages"][..], 606),
    ] {
        if at(manifest, path).as_i64() != Some(expected) {
            errors.push(format!("[denominator] {path:?} must remain {expected}"));
        }
    }
    if at(manifest, &["heads", "pooled_score"]).as_str() != Some("forbidden")
        || at(manifest, &["metrics", "pooled_metrics"]).as_str() != Some("forbidden")
        || at(manifest, &["metrics", "missing_head_denominator"]).as_str() != Some("hard_failure")
    {
        errors.push("[independence] pooled or missing-head scoring was enabled".into());
    }
    for (path, expected) in [
        (&["gates", "state"][..], "shadow"),
        (&["gates", "promotion_state"][..], "candidate"),
        (&["gates", "auto_promote"][..], "forbidden"),
        (&["gates", "donor_selection"][..], "forbidden"),
        (&["gates", "graph_mutation"][..], "forbidden"),
        (&["gates", "fulltext_composition"][..], "forbidden"),
    ] {
        if at(manifest, path).as_str() != Some(expected) {
            errors.push(format!("[policy-lock] {path:?} changed"));
        }
    }
    if at(manifest, &["gates", "import_eligible"]).as_bool() != Some(false) {
        errors.push("[policy-lock] import_eligible must remain false".into());
    }
    let docs = at(manifest, &["source_documents"])
        .as_sequence()
        .expect("source documents");
    if docs.len() != DOCUMENTS.len() {
        errors.push("[source-binding] document denominator eroded".into());
    }
    for ((id, sha, pages), doc) in DOCUMENTS.iter().zip(docs) {
        if doc["id"].as_str() != Some(*id)
            || doc["sha256"].as_str() != Some(*sha)
            || doc["pages"].as_i64() != Some(*pages)
        {
            errors.push(format!("[source-binding] {id} SHA or pages drifted"));
        }
    }
    validate_oracle(oracle, &mut errors);
    errors.extend(typed_absence_semantics_errors(manifest));
    errors.extend(typed_absence_rule_errors());
    errors
}

fn validate_oracle(oracle: &Value, errors: &mut Vec<String>) {
    if at(oracle, &["schema_version"]).as_str() != Some("native-visual-oracle.v1")
        || at(oracle, &["review_status"]).as_str() != Some("independently_reviewed")
        || at(oracle, &["representation"]).as_str() != Some("digest_only")
        || at(oracle, &["coordinate_dialect"]).as_str() != Some("top_left_72dpi_points")
    {
        errors.push("[oracle-schema] review, representation, or coordinate dialect drifted".into());
    }
    for field in FORBIDDEN {
        if !strings(oracle, &["forbidden_fields"])
            .iter()
            .any(|item| item == field)
            || contains_key(oracle, field)
        {
            errors.push(format!("[redaction] forbidden oracle field {field} leaked"));
        }
    }
    let receipts = at(oracle, &["receipts"])
        .as_sequence()
        .expect("oracle receipts");
    if receipts.len() != DOCUMENTS.len() * LANES.len() {
        errors.push("[oracle-coverage] expected 9 document/lane receipts".into());
    }
    let mut keys = BTreeSet::new();
    for (receipt_index, receipt) in receipts.iter().enumerate() {
        let id = receipt["document_id"].as_str().unwrap_or("");
        let lane = receipt["lane_id"].as_str().unwrap_or("");
        let key = format!("{id}/{lane}");
        if !keys.insert(key.clone()) {
            errors.push(format!("[duplicate-receipt] {key}"));
        }
        let expected_sha = DOCUMENTS
            .iter()
            .find(|(expected_id, _, _)| *expected_id == id)
            .map(|(_, sha, _)| *sha);
        let expected_pages = DOCUMENTS
            .iter()
            .find(|(expected_id, _, _)| *expected_id == id)
            .map(|(_, _, pages)| *pages);
        let expected_document_index = DOCUMENTS
            .iter()
            .position(|(expected_id, _, _)| *expected_id == id);
        let expected_lane_index = LANES.iter().position(|candidate| *candidate == lane);
        if expected_sha != receipt["source_sha256"].as_str()
            || !LANES.contains(&lane)
            || expected_pages != receipt["page_denominator"].as_i64()
            || expected_document_index
                .zip(expected_lane_index)
                .map(|(document, lane)| document * LANES.len() + lane)
                != Some(receipt_index)
        {
            errors.push(format!(
                "[source-binding] invalid receipt binding or order {key}"
            ));
        }
        let range = receipt["page_range"].as_sequence().expect("page range");
        if range.len() != 2 || range[0].as_i64() != Some(1) || expected_pages != range[1].as_i64() {
            errors.push(format!("[topology] invalid document page range for {key}"));
        }
        let heads = receipt["heads"].as_sequence().expect("head receipts");
        if heads.len() != HEADS.len() {
            errors.push(format!("[head-denominator] missing head row for {key}"));
        }
        let mut seen_heads = BTreeSet::new();
        for head in heads {
            let label = head["head"].as_str().unwrap_or("");
            if !HEADS.contains(&label) || !seen_heads.insert(label) {
                errors.push(format!("[head-label] unknown or duplicate head in {key}"));
            }
            if head["head_ordinal"].as_i64() != Some(0) {
                errors.push(format!("[ordinal] invalid head ordinal in {key}"));
            }
            let status = head["status"].as_str().unwrap_or("");
            if !is_absence(status) {
                errors.push(format!("[absence] status is not closed for {key}/{label}"));
            }
            if head["head_ordinal"].as_i64() != Some(0) {
                errors.push(format!(
                    "[ordinal] head ordinal is not bounded for {key}/{label}"
                ));
            }
            for field in [
                "bbox_72dpi",
                "caption_digest",
                "content_digest",
                "callout_digest",
            ] {
                let value = head[field].as_str().unwrap_or("");
                let absent = value == "absent";
                if !absent && !valid_digest(value) {
                    errors.push(format!(
                        "[digest] {field} is not lowercase SHA-256 for {key}/{label}"
                    ));
                }
                if !absent && is_absence(status) {
                    errors.push(format!(
                        "[typed-absence] {field} must be absent for {key}/{label}"
                    ));
                }
            }
        }
        for sibling in ["table_cells", "formula_labels"] {
            let item = &receipt[sibling];
            let count = item["oracle_n"].as_i64();
            let status = item["status"].as_str().unwrap_or("");
            if count.is_none() || count.unwrap() < 0 {
                errors.push(format!("[sibling-denominator] invalid {key}/{sibling}"));
            }
            if count == Some(0) && status != "not_observed" {
                errors.push(format!("[sibling-absence] silent zero {key}/{sibling}"));
            }
            if count.unwrap_or(0) > 0 && status == "not_observed" {
                errors.push(format!(
                    "[sibling-conservation] nonzero absent {key}/{sibling}"
                ));
            }
        }
    }
}

/// Every reason the `typed_absence_semantics` DATA block is unacceptable.
///
/// The scalar tokens are compared against the detector's own constants
/// (`NATIVE_VISUAL_TYPED_ABSENCE_*`) and the vocabulary against
/// [`TYPED_ABSENCE_TAXONOMY`], never against a re-typed literal, so a rename on
/// either side fails this gate instead of silently drifting. A whole-block
/// absence is reported as `typed-absence-semantics:missing`, not as drift.
fn typed_absence_semantics_errors(v: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(block) = v.get("typed_absence_semantics") else {
        errors.push("typed-absence-semantics:missing".into());
        return errors;
    };
    for (field, expected) in [
        ("contract_id", NATIVE_VISUAL_TYPED_ABSENCE_CONTRACT_ID),
        ("adr_ref", NATIVE_VISUAL_TYPED_ABSENCE_ADR_REF),
        ("vocabulary", NATIVE_VISUAL_TYPED_ABSENCE_VOCABULARY),
        ("reason_field", NATIVE_VISUAL_TYPED_ABSENCE_REASON_FIELD),
        (
            "denominator_field",
            NATIVE_VISUAL_TYPED_ABSENCE_DENOMINATOR_FIELD,
        ),
        (
            "declared_denominator",
            NATIVE_VISUAL_TYPED_ABSENCE_DECLARED_DENOMINATOR,
        ),
        ("pooling", NATIVE_VISUAL_TYPED_ABSENCE_POOLING),
        (
            "missing_head_denominator",
            NATIVE_VISUAL_TYPED_ABSENCE_MISSING_DENOMINATOR,
        ),
        (
            "unity_denominator",
            NATIVE_VISUAL_TYPED_ABSENCE_UNITY_DENOMINATOR,
        ),
        (
            "empty_population",
            NATIVE_VISUAL_TYPED_ABSENCE_EMPTY_POPULATION,
        ),
        (
            "observed_population",
            NATIVE_VISUAL_TYPED_ABSENCE_OBSERVED_POPULATION,
        ),
        (
            "untyped_empty_shape",
            NATIVE_VISUAL_TYPED_ABSENCE_UNTYPED_EMPTY_SHAPE,
        ),
    ] {
        if block.get(field).and_then(Value::as_str) != Some(expected) {
            errors.push(format!("typed-absence-semantics:{field}"));
        }
    }
    if block.get("terminal_is_pass").and_then(Value::as_bool)
        != Some(NATIVE_VISUAL_TYPED_ABSENCE_TERMINAL_IS_PASS)
    {
        errors.push("typed-absence-semantics:terminal_is_pass".into());
    }
    let used_by: Vec<&str> = block
        .get("used_by")
        .and_then(Value::as_sequence)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if used_by != NATIVE_VISUAL_TYPED_ABSENCE_USED_BY {
        errors.push("typed-absence-semantics:used_by".into());
    }
    // Closed dictionary: the sanctioned vocabulary is `failure_taxonomy.values`
    // and it must be exactly the Rust taxonomy in declaration order.
    if dictionary_tokens(v) != absence_tokens() {
        errors.push("typed-absence-semantics:failure_taxonomy".into());
    }
    errors
}

/// The `failure_taxonomy.values` vocabulary as tokens, or empty when absent or
/// not a sequence.
fn dictionary_tokens(v: &Value) -> Vec<String> {
    v.get("failure_taxonomy")
        .and_then(|taxonomy| taxonomy.get("values"))
        .and_then(Value::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

const DECLARED_DOCUMENT_ID: &str = "typed-absence-document";
const DECLARED_SOURCE_SHA: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// A schema-legal observation whose `count` candidates all declare the same
/// typed absence on distinct pages.
fn declared_absence_observation(kind: NativeVisualKind, count: u32) -> NativeVisualObservation {
    let provider = NativeVisualProvider {
        id: "typed-absence-provider".to_owned(),
        version: "1.0".to_owned(),
    };
    let primitives = (1..=count)
        .map(|page| NativeVisualPrimitive {
            document_id: DECLARED_DOCUMENT_ID.to_owned(),
            source_sha256: DECLARED_SOURCE_SHA.to_owned(),
            provider: provider.clone(),
            page_ordinal: page,
            source_ordinal: 0,
            kind,
            bbox: None,
            content_digest: None,
            absence: Some(NativeVisualAbsence::MalformedBbox),
        })
        .collect();
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.to_owned(),
        document_id: DECLARED_DOCUMENT_ID.to_owned(),
        source_sha256: DECLARED_SOURCE_SHA.to_owned(),
        provider,
        primitives,
        figure_captions: Vec::new(),
        figure_callouts: Vec::new(),
        table_captions: Vec::new(),
        table_callouts: Vec::new(),
        table_cells: Vec::new(),
        formula_labels: Vec::new(),
        formula_callouts: Vec::new(),
        lane_absence: None,
    }
}

/// The DATA-declared semantics, executed against the detector: a non-empty
/// declared population must report a typed terminal whose `terminal`
/// denominator equals the declared count and never a pooled `1`; an empty
/// population stays a legitimate `NotObserved`. This is the behavioural half of
/// the DATA block and is independent of `native_visual_typed_absence`.
fn typed_absence_rule_errors() -> Vec<String> {
    let mut errors = Vec::new();
    let declared_count = 3u32;
    for (label, kind, detector) in [
        (
            "figure",
            NativeVisualKind::Image,
            detect_figure_heads as fn(&NativeVisualObservation, NativeVisualConfig) -> _,
        ),
        ("table", NativeVisualKind::Table, detect_table_heads),
        ("formula", NativeVisualKind::Formula, detect_formula_heads),
    ] {
        let declared = declared_absence_observation(kind, declared_count);
        let declared_outcome = detector(&declared, NativeVisualConfig::default());
        // `declared_count == 3` also proves the denominator was not pooled into
        // the constant `1` (H17-N2).
        let typed_and_counted = matches!(
            &declared_outcome,
            Ok(NativeVisualHeadOutcome::Terminal { reason, metrics })
                if *reason == NativeVisualAbsence::MalformedBbox
                    && metrics.terminal == declared_count
        );
        if !typed_and_counted {
            errors.push(format!(
                "typed-absence-semantics:declared-population:{label}:{declared_outcome:?}"
            ));
        }

        let empty = declared_absence_observation(kind, 0);
        let empty_outcome = detector(&empty, NativeVisualConfig::default());
        let true_absence = matches!(
            &empty_outcome,
            Ok(NativeVisualHeadOutcome::Terminal {
                reason: NativeVisualAbsence::NotObserved,
                metrics,
            }) if metrics.terminal == 1
        );
        if !true_absence {
            errors.push(format!(
                "typed-absence-semantics:empty-population:{label}:{empty_outcome:?}"
            ));
        }
    }
    errors
}

/// Assert one mutation of the `typed_absence_semantics` block (or of the
/// vocabulary it sanctions) is reported by name.
fn rejects_typed_absence(mutate: impl FnOnce(&mut Value), marker: &str) {
    let mut manifest = yaml(MANIFEST);
    mutate(&mut manifest);
    let errors = typed_absence_semantics_errors(&manifest);
    assert!(
        errors.iter().any(|error| error.contains(marker)),
        "typed absence mutation `{marker}` was accepted: {errors:?}"
    );
}

#[test]
fn typed_absence_semantics_is_bound_to_the_rust_taxonomy() {
    let manifest = yaml(MANIFEST);
    let errors = typed_absence_semantics_errors(&manifest);
    assert!(
        errors.is_empty(),
        "typed absence semantics violations:\n{}",
        errors.join("\n")
    );
    // The dictionary has exactly one authority: the Rust taxonomy.
    assert_eq!(
        dictionary_tokens(&manifest),
        absence_tokens(),
        "failure_taxonomy.values must be exactly the NativeVisualAbsence taxonomy"
    );
    assert_eq!(
        TYPED_ABSENCE_TAXONOMY.len(),
        10,
        "the closed absence vocabulary changed size"
    );
    // A whole-block absence is an absence, not scalar drift.
    let missing = typed_absence_semantics_errors(&Value::Null);
    assert!(
        missing
            .iter()
            .any(|error| error.contains("typed-absence-semantics:missing")),
        "a missing typed absence block must be reported as missing: {missing:?}"
    );
    assert_eq!(
        missing.len(),
        1,
        "a missing block must not also be reported as scalar drift: {missing:?}"
    );
    // The DATA-declared semantics hold in the code.
    assert!(
        typed_absence_rule_errors().is_empty(),
        "declared-absence semantics do not hold in the detector: {:?}",
        typed_absence_rule_errors()
    );
}

#[test]
fn typed_absence_mutations_fail_closed() {
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["contract_id"] = Value::String("other.v1".into()),
        "typed-absence-semantics:contract_id",
    );
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["adr_ref"] = Value::String("ADR-999".into()),
        "typed-absence-semantics:adr_ref",
    );
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["vocabulary"] = Value::String("other.values".into()),
        "typed-absence-semantics:vocabulary",
    );
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["reason_field"] = Value::String("status".into()),
        "typed-absence-semantics:reason_field",
    );
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["denominator_field"] = Value::String("na".into()),
        "typed-absence-semantics:denominator_field",
    );
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["declared_denominator"] = Value::String("one".into()),
        "typed-absence-semantics:declared_denominator",
    );
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["pooling"] = Value::String("allowed".into()),
        "typed-absence-semantics:pooling",
    );
    rejects_typed_absence(
        |m| {
            m["typed_absence_semantics"]["missing_head_denominator"] =
                Value::String("tolerated".into())
        },
        "typed-absence-semantics:missing_head_denominator",
    );
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["unity_denominator"] = Value::String("allowed".into()),
        "typed-absence-semantics:unity_denominator",
    );
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["terminal_is_pass"] = Value::Bool(true),
        "typed-absence-semantics:terminal_is_pass",
    );
    rejects_typed_absence(
        |m| m["typed_absence_semantics"]["untyped_empty_shape"] = Value::Null,
        "typed-absence-semantics:untyped_empty_shape",
    );
    rejects_typed_absence(
        |m| {
            m["typed_absence_semantics"]["used_by"] =
                Value::Sequence(vec![Value::String("S04-T01".into())])
        },
        "typed-absence-semantics:used_by",
    );
    // Dictionary drift in either direction is reported by the same marker.
    rejects_typed_absence(
        |m| {
            m["failure_taxonomy"]["values"]
                .as_sequence_mut()
                .unwrap()
                .push(Value::String("fabricated".into()))
        },
        "typed-absence-semantics:failure_taxonomy",
    );
    rejects_typed_absence(
        |m| {
            m["failure_taxonomy"]["values"] =
                Value::Sequence(vec![Value::String("not_observed".into())])
        },
        "typed-absence-semantics:failure_taxonomy",
    );
    rejects_typed_absence(
        |m| {
            m["failure_taxonomy"]
                .as_mapping_mut()
                .unwrap()
                .remove("values");
        },
        "typed-absence-semantics:failure_taxonomy",
    );
}

#[test]
fn native_visual_data_contract_is_source_bound_and_independent() {
    let errors = validate(&yaml(MANIFEST), &yaml(ORACLE));
    assert!(
        errors.is_empty(),
        "S16 DATA violations:\n{}",
        errors.join("\n")
    );
}

#[test]
fn rejects_pooling_missing_denominator_unknown_label_and_checksum_drift() {
    let oracle = yaml(ORACLE);
    let mut manifest = yaml(MANIFEST);
    manifest["heads"]["pooled_score"] = Value::String("allowed".into());
    assert!(!validate(&manifest, &oracle).is_empty());

    let mut manifest = yaml(MANIFEST);
    manifest["source_documents"]
        .as_sequence_mut()
        .unwrap()
        .pop();
    assert!(!validate(&manifest, &oracle).is_empty());

    let mut oracle = yaml(ORACLE);
    oracle["receipts"][0]["heads"][0]["head"] = Value::String("chart".into());
    assert!(!validate(&yaml(MANIFEST), &oracle).is_empty());

    let mut oracle = yaml(ORACLE);
    oracle["receipts"][0]["source_sha256"] = Value::String("0".repeat(64));
    assert!(!validate(&yaml(MANIFEST), &oracle).is_empty());
}

#[test]
fn rejects_payload_leakage_promotion_and_untyped_absence() {
    let mut oracle = yaml(ORACLE);
    oracle["raw_text"] = Value::String("must never be stored".into());
    assert!(!validate(&yaml(MANIFEST), &oracle).is_empty());

    let mut manifest = yaml(MANIFEST);
    manifest["gates"]["promotion_state"] = Value::String("promoted".into());
    assert!(!validate(&manifest, &yaml(ORACLE)).is_empty());

    let mut oracle = yaml(ORACLE);
    oracle["receipts"][0]["heads"][0]["status"] = Value::String("".into());
    assert!(!validate(&yaml(MANIFEST), &oracle).is_empty());
}

#[test]
fn preserves_s14_authority_and_all_three_head_denominators() {
    let s14 = yaml(S14);
    assert_eq!(s14["schema_version"].as_str(), Some("layout-bakeoff.v1"));
    assert_eq!(s14["corpus"]["page_denominator"].as_i64(), Some(202));
    assert_eq!(s14["lanes"]["required"].as_sequence().unwrap().len(), 3);
    let manifest = yaml(MANIFEST);
    assert_eq!(strings(&manifest, &["heads", "closed_labels"]).len(), 3);
    assert_eq!(
        manifest["heads"]["independent_authorities"].as_str(),
        Some("required")
    );
    assert_eq!(
        manifest["gates"]["s14_durable_evidence_mutation"].as_str(),
        Some("forbidden")
    );
}

#[test]
fn typed_absence_rule_pin_distinguishes_declared_absence_from_true_absence() {
    // The behavioural rule is non-vacuous: a declared population and an empty
    // population must not be reported as the same thing.
    let declared = declared_absence_observation(NativeVisualKind::Image, 2);
    let empty = declared_absence_observation(NativeVisualKind::Image, 0);
    let declared_outcome = detect_figure_heads(&declared, NativeVisualConfig::default())
        .expect("declared population is schema-legal");
    let empty_outcome = detect_figure_heads(&empty, NativeVisualConfig::default())
        .expect("empty population is schema-legal");
    assert_ne!(
        declared_outcome, empty_outcome,
        "a declared absence must not be flattened into a true absence"
    );
    assert!(matches!(
        declared_outcome,
        NativeVisualHeadOutcome::Terminal {
            reason: NativeVisualAbsence::MalformedBbox,
            metrics,
        } if metrics.terminal == 2
    ));
    assert!(matches!(
        empty_outcome,
        NativeVisualHeadOutcome::Terminal {
            reason: NativeVisualAbsence::NotObserved,
            metrics,
        } if metrics.terminal == 1
    ));
}
