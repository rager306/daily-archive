//! Independent reviewed-oracle evaluation for the native section / reading-order
//! surface (S05/T02).
//!
//! The reviewed segmentation oracle is committed twice: the digest-only fixture
//! `crates/da-parse/tests/fixtures/segmentation_oracle.yaml` and the canonical
//! source artifact `artifacts/parse_compat/document_segmentation_oracle_source.json`
//! whose sha256 is pinned in `data/parse/document_segmentation.yaml#oracle`. This
//! test:
//!
//! 1. recomputes the source artifact's sha256 **from its bytes** and compares it
//!    to the declared pin (a stale or substituted source cannot pass);
//! 2. re-derives the per-document × per-lane × per-region expected boundary/count
//!    rows from the committed fixture alone, normalising the reviewed one-based
//!    page origin to the native zero-based location;
//! 3. evaluates native-shaped outcomes with the typed vocabulary
//!    (`Agreement` / `BoundaryDisplaced` / `CountMismatch` / `NotAssessable`);
//! 4. refuses a same-generator oracle, and keeps an absent region a *typed
//!    absence* rather than a zero;
//! 5. proves the structural separation from the T01 character-coverage oracle:
//!    the section rows are boundary/count only, and the verdict vocabulary
//!    carries no character-level term.
//!
//! The **corpus** native measurement (the real `pdf_inspector_positioned` plane)
//! runs in the gated runner of S05/T06 and is written to the corpus-bound
//! receipt; a `da-parse` unit test cannot read the gitignored PDFs. The native
//! outcomes here are therefore *conformance* inputs transposed from the reviewed
//! native-lane rows, and this test's claim scope is limited to the oracle
//! re-read and the verdict machinery.

use da_parse::*;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const FIXTURE: &str = include_str!("fixtures/segmentation_oracle.yaml");
const SEGMENTATION_MANIFEST: &str = "data/parse/document_segmentation.yaml";
const ORACLE_SOURCE: &str = "artifacts/parse_compat/document_segmentation_oracle_source.json";

const OS_LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];

/// The declared corpus, with the reviewed page count.
const DOCUMENTS: [(&str, u64); 3] = [
    ("hub-2507.19457", 96),
    ("rlm-2512.24601", 43),
    ("reca-2605.26525v1", 63),
];
fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fixture_value() -> serde_yaml::Value {
    serde_yaml::from_str(FIXTURE).expect("the reviewed fixture parses")
}

fn manifest_value() -> serde_yaml::Value {
    let bytes = std::fs::read(workspace_path(SEGMENTATION_MANIFEST)).expect("manifest exists");
    serde_yaml::from_slice(&bytes).expect("the manifest parses")
}

fn source_bytes() -> Vec<u8> {
    std::fs::read(workspace_path(ORACLE_SOURCE)).expect("the oracle source exists")
}

fn page_origin(manifest: &serde_yaml::Value) -> String {
    manifest["evaluation"]["coordinate_binding"]["page_origin"]
        .as_str()
        .expect("the manifest declares a page origin")
        .to_owned()
}

/// Normalise one reviewed coordinate to the native zero-based location.
fn normalize(location: &serde_yaml::Value, page_origin: &str) -> Option<FullLocation> {
    let values = location.as_sequence()?;
    let page = values.first()?.as_u64()? as u32;
    let page = match page_origin {
        "one_based" => page.checked_sub(1)?,
        "zero_based" => page,
        _ => return None,
    };
    Some(FullLocation {
        page_ordinal: page,
        block_ordinal: values.get(1)?.as_u64()? as u32,
        line_ordinal: values.get(2)?.as_u64()? as u32,
        token_ordinal: values.get(3)?.as_u64()? as u32,
    })
}

/// Re-derive the expected rows from the committed fixture alone.
fn derived_rows() -> Vec<NativeSectionOracleRow> {
    let fixture = fixture_value();
    let manifest = manifest_value();
    let origin = page_origin(&manifest);
    let mut rows = Vec::new();
    for receipt in fixture["receipts"].as_sequence().expect("receipts") {
        let document_id = receipt["document_id"].as_str().expect("document_id");
        let lane_id = receipt["lane_id"].as_str().expect("lane_id");
        let runs = receipt["region_runs"].as_sequence().expect("region_runs");
        for run in runs {
            let region = run["region"].as_str().expect("region");
            let boundary = if run["absence"].as_str() == Some("reviewed_absent") {
                None
            } else {
                Some(NativeSectionOracleBoundary {
                    start: normalize(&run["start"], &origin).expect("start"),
                    end: normalize(&run["end"], &origin).expect("end"),
                })
            };
            rows.push(NativeSectionOracleRow {
                document_id: document_id.to_owned(),
                lane_id: lane_id.to_owned(),
                region: parse_region(region),
                boundary,
            });
        }
    }
    rows
}

fn parse_region(name: &str) -> FullSemanticRegion {
    native_section_regions()
        .into_iter()
        .find(|region| serde_json::to_value(region).unwrap() == name)
        .unwrap_or_else(|| panic!("unknown region {name}"))
}

fn region_rows_from_oracle(
    rows: &[NativeSectionOracleRow],
    document_id: &str,
    lane: &str,
) -> Vec<NativeSectionRegionRow> {
    rows.iter()
        .filter(|row| row.document_id == document_id && row.lane_id == lane)
        .filter_map(|row| {
            row.boundary
                .as_ref()
                .map(|boundary| NativeSectionRegionRow {
                    region: row.region,
                    start: boundary.start.clone(),
                    end: boundary.end.clone(),
                    admitted_tokens: 1,
                    boundary_digest: format!(
                        "sha256:{}",
                        hex_sha256(format!("{document_id}:{lane}").as_bytes())
                    ),
                })
        })
        .collect()
}

fn native_outcome(
    document_id: &str,
    region_rows: Vec<NativeSectionRegionRow>,
) -> NativeSectionOutcome {
    let admitted_token_count = region_rows.iter().map(|row| row.admitted_tokens).sum();
    let region_sequence = region_rows.iter().map(|row| row.region).collect();
    NativeSectionOutcome {
        schema_version: NATIVE_SECTION_READING_ORDER_SCHEMA_VERSION.to_owned(),
        document_id: document_id.to_owned(),
        provider_id: "conformance-fixture".to_owned(),
        source_pdf_sha256: "0".repeat(64),
        lattice_fingerprint: "sha256:conformance".to_owned(),
        section_parent_regions: SECTION_PARENT_REGIONS.to_vec(),
        sections: Vec::new(),
        absences: Vec::new(),
        reading_order: NativeReadingOrderOutcome {
            schema_version: NATIVE_SECTION_READING_ORDER_SCHEMA_VERSION.to_owned(),
            admitted_order_digest: format!("sha256:{}", hex_sha256(document_id.as_bytes())),
            admitted_token_count,
            region_rows,
            region_sequence,
            page_spans: Vec::new(),
            column_shifts: Vec::new(),
        },
    }
}

// ───────────────────────────── oracle identity ─────────────────────────────

#[test]
fn the_oracle_source_sha256_recomputes_to_the_declared_pin() {
    let manifest = manifest_value();
    let declared = manifest["oracle"]["source_artifact_sha256"]
        .as_str()
        .expect("the manifest pins the source artifact");
    let recomputed = hex_sha256(&source_bytes());
    assert_eq!(
        recomputed, declared,
        "the committed oracle bytes must hash to the declared pin"
    );
    assert_eq!(
        manifest["oracle"]["source_artifact"].as_str(),
        Some(ORACLE_SOURCE)
    );
    let source: serde_json::Value =
        serde_json::from_slice(&source_bytes()).expect("the source parses");
    assert_eq!(
        source["schema_version"].as_str(),
        Some("document-segmentation-oracle-source.v3")
    );
    assert_eq!(source["receipts"].as_array().map(Vec::len), Some(9));
}

#[test]
fn the_reviewed_fixture_and_the_canonical_source_agree_on_every_boundary() {
    let fixture = fixture_value();
    let source: serde_json::Value =
        serde_json::from_slice(&source_bytes()).expect("the source parses");
    let source_receipts = source["receipts"].as_array().expect("source receipts");
    for receipt in fixture["receipts"].as_sequence().expect("fixture receipts") {
        let document_id = receipt["document_id"].as_str().unwrap();
        let lane = receipt["lane_id"].as_str().unwrap();
        let source_receipt = source_receipts
            .iter()
            .find(|row| {
                row["document_id"].as_str() == Some(document_id)
                    && row["lane"].as_str() == Some(lane)
            })
            .expect("a source receipt exists");
        let canonical = source_receipt["canonical_boundary"]
            .as_array()
            .expect("canonical boundaries");
        for run in receipt["region_runs"].as_sequence().unwrap() {
            let region = run["region"].as_str().unwrap();
            if run["absence"].as_str() == Some("reviewed_absent") {
                continue;
            }
            let declared = run["boundary_digest"].as_str().expect("fixture digest");
            let published = canonical
                .iter()
                .find(|row| row["region"].as_str() == Some(region))
                .and_then(|row| row["sha256"].as_str())
                .map(|value| value.trim_start_matches("sha256:"))
                .expect("a canonical boundary exists");
            assert_eq!(
                declared, published,
                "{document_id}/{lane}/{region} digest drift"
            );
        }
    }
}

// ───────────────────────────── expected rows ─────────────────────────────

#[test]
fn expected_rows_are_derived_per_document_lane_region() {
    let rows = derived_rows();
    assert_eq!(rows.len(), 81, "3 documents x 3 lanes x 9 regions");
    let present = rows.iter().filter(|row| row.boundary.is_some()).count();
    let absent = rows.iter().filter(|row| row.boundary.is_none()).count();
    assert_eq!(present, 45, "five present regions per document-lane");
    assert_eq!(absent, 36, "four reviewed-absent regions per document-lane");
    for (document_id, pages) in DOCUMENTS {
        let lane: Vec<&NativeSectionOracleRow> = rows
            .iter()
            .filter(|row| row.document_id == document_id)
            .collect();
        assert_eq!(lane.len(), 27, "{document_id} has 3 lanes x 9 regions");
        let fixture = fixture_value();
        for receipt in fixture["receipts"].as_sequence().unwrap() {
            if receipt["document_id"].as_str() == Some(document_id) {
                assert_eq!(receipt["page_count"].as_u64(), Some(pages));
                assert!(OS_LANES.contains(&receipt["lane_id"].as_str().unwrap()));
            }
        }
    }
    // The reviewed one-based page origin is normalised to the native origin.
    for row in &rows {
        if let Some(boundary) = &row.boundary {
            assert!(
                boundary.start.page_ordinal < 96,
                "the reviewed page 1 becomes native page 0"
            );
        }
    }
}

// ─────────────────────────── typed verdicts ───────────────────────────

#[test]
fn a_native_outcome_matching_the_reviewed_native_lane_agrees() {
    let rows = derived_rows();
    let native_rows = region_rows_from_oracle(&rows, "hub-2507.19457", NATIVE_MEASURED_LANE_ID);
    let outcome = native_outcome("hub-2507.19457", native_rows);
    let verdicts =
        evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &rows, true).expect("runs");
    assert_eq!(verdicts.len(), NATIVE_SECTION_REGION_COUNT);
    for verdict in &verdicts {
        assert_eq!(
            verdict.verdict,
            NativeSectionRowVerdict::Agreement,
            "conformance row {region:?}",
            region = verdict.region
        );
    }
}

#[test]
fn a_boundary_displacement_is_typed_not_prose() {
    let mut rows = derived_rows();
    let body = rows
        .iter_mut()
        .find(|row| {
            row.document_id == "hub-2507.19457"
                && row.lane_id == NATIVE_MEASURED_LANE_ID
                && row.region == FullSemanticRegion::Body
        })
        .expect("the body row exists");
    let boundary = body.boundary.as_mut().expect("body is present");
    boundary.end.page_ordinal += 1;
    let native_rows =
        region_rows_from_oracle(&derived_rows(), "hub-2507.19457", NATIVE_MEASURED_LANE_ID);
    let outcome = native_outcome("hub-2507.19457", native_rows);
    let verdicts =
        evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &rows, true).expect("runs");
    let body_verdict = verdicts
        .iter()
        .find(|row| row.region == FullSemanticRegion::Body)
        .expect("body verdict");
    assert!(matches!(
        body_verdict.verdict,
        NativeSectionRowVerdict::BoundaryDisplaced { .. }
    ));
}

#[test]
fn a_region_presence_disagreement_is_a_count_mismatch() {
    let rows = derived_rows();
    // An empty native plane has no body run while the oracle declares one.
    let outcome = native_outcome("hub-2507.19457", Vec::new());
    let mut scoped: Vec<NativeSectionOracleRow> = rows
        .iter()
        .filter(|row| row.document_id == "hub-2507.19457" && row.lane_id == NATIVE_MEASURED_LANE_ID)
        .cloned()
        .collect();
    scoped.retain(|row| row.region == FullSemanticRegion::Body);
    let verdicts = evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &scoped, true)
        .expect("runs");
    let body = verdicts
        .iter()
        .find(|row| row.region == FullSemanticRegion::Body)
        .unwrap();
    assert_eq!(
        body.verdict,
        NativeSectionRowVerdict::CountMismatch {
            oracle_runs: 1,
            native_runs: 0,
        }
    );
}

#[test]
fn an_absent_reviewed_region_is_a_typed_absence_not_a_zero() {
    let fixture = fixture_value();
    for receipt in fixture["receipts"].as_sequence().unwrap() {
        for run in receipt["region_runs"].as_sequence().unwrap() {
            if run["absence"].as_str() == Some("reviewed_absent") {
                assert!(
                    run["start"].is_null() && run["end"].is_null(),
                    "an absent region carries no zero-valued boundary"
                );
            }
        }
    }
    let rows = derived_rows();
    let absent: Vec<&NativeSectionOracleRow> = rows
        .iter()
        .filter(|row| {
            row.document_id == "hub-2507.19457"
                && row.lane_id == NATIVE_MEASURED_LANE_ID
                && row.boundary.is_none()
        })
        .collect();
    assert_eq!(absent.len(), 4, "acknowledgement/toc/funding/availability");
    for row in absent {
        let value = serde_json::to_value(row).unwrap();
        assert!(
            value["boundary"].is_null(),
            "absence is typed as a missing boundary, not a zero"
        );
    }
    // An absent native region against an absent oracle row agrees; it is not
    // silently dropped.
    let outcome = native_outcome("hub-2507.19457", Vec::new());
    let scoped: Vec<NativeSectionOracleRow> = rows
        .iter()
        .filter(|row| {
            row.document_id == "hub-2507.19457"
                && row.lane_id == NATIVE_MEASURED_LANE_ID
                && row.region == FullSemanticRegion::Funding
        })
        .cloned()
        .collect();
    let verdicts = evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &scoped, true)
        .expect("runs");
    let funding = verdicts
        .iter()
        .find(|row| row.region == FullSemanticRegion::Funding)
        .unwrap();
    assert_eq!(funding.verdict, NativeSectionRowVerdict::Agreement);
}

#[test]
fn a_non_measured_lane_is_not_assessable_by_name() {
    let rows = derived_rows();
    let native_rows = region_rows_from_oracle(&rows, "hub-2507.19457", NATIVE_MEASURED_LANE_ID);
    let outcome = native_outcome("hub-2507.19457", native_rows);

    let rejected =
        evaluate_native_section_rows(&outcome, SEGMENTER_REJECTED_LANE_IDS[0], &rows, true)
            .expect("runs");
    assert!(rejected.iter().all(|row| row.verdict
        == NativeSectionRowVerdict::NotAssessable {
            reason: NativeSectionNotAssessable::LaneNotMeasuredBySegmenter,
        }));

    let java =
        evaluate_native_section_rows(&outcome, JAVA_AUTHORITY_LANE_ID, &rows, true).expect("runs");
    assert!(java.iter().all(|row| row.verdict
        == NativeSectionRowVerdict::NotAssessable {
            reason: NativeSectionNotAssessable::LaneIsJavaAuthority,
        }));

    let unknown =
        evaluate_native_section_rows(&outcome, "unlisted-lane", &rows, true).expect("runs");
    assert!(unknown.iter().all(|row| row.verdict
        == NativeSectionRowVerdict::NotAssessable {
            reason: NativeSectionNotAssessable::UnknownLane,
        }));

    let missing =
        evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &[], true).expect("runs");
    assert!(missing.iter().all(|row| row.verdict
        == NativeSectionRowVerdict::NotAssessable {
            reason: NativeSectionNotAssessable::OracleRowMissing,
        }));
}

#[test]
fn a_same_generator_oracle_is_refused_by_name() {
    assert_eq!(
        require_independent_section_oracle(NATIVE_SECTION_SUT_GENERATOR_ID),
        Err(NativeSectionReadingOrderError::NotIndependent)
    );
    assert_eq!(
        require_independent_section_oracle("preserved-java-tei-lexical-scanner"),
        Ok(())
    );

    let rows = derived_rows();
    let native_rows = region_rows_from_oracle(&rows, "hub-2507.19457", NATIVE_MEASURED_LANE_ID);
    let outcome = native_outcome("hub-2507.19457", native_rows);
    let verdicts = evaluate_native_section_rows(&outcome, NATIVE_MEASURED_LANE_ID, &rows, false)
        .expect("runs");
    assert!(verdicts.iter().all(|row| row.verdict
        == NativeSectionRowVerdict::NotAssessable {
            reason: NativeSectionNotAssessable::OracleNotIndependent,
        }));
}

// ───────────────────── structural separation from T01 ─────────────────────

#[test]
fn the_section_oracle_is_boundary_and_count_based_not_character_based() {
    let rows = derived_rows();
    let row = serde_json::to_value(&rows[0]).unwrap();
    let object = row.as_object().expect("a row is an object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["boundary", "document_id", "lane_id", "region"],
        "the section row is boundary/count only"
    );
    for forbidden in [
        "admitted_lexical_denominator",
        "characters",
        "character_counts",
        "annotation_digest",
    ] {
        assert!(
            !object.contains_key(forbidden),
            "the section row must not carry {forbidden}"
        );
    }

    // The verdict vocabulary is closed and carries no character-level term.
    assert_eq!(NATIVE_SECTION_ROW_VERDICT_VOCABULARY.len(), 4);
    for forbidden in ["content_loss", "granularity_only", "character_recall"] {
        assert!(!NATIVE_SECTION_ROW_VERDICT_VOCABULARY.contains(&forbidden));
    }

    // The manifest binds the oracle to page/block/line/token, not to character
    // offsets, so a character-level body verdict cannot be produced from it.
    let manifest = manifest_value();
    assert_eq!(
        manifest["evaluation"]["coordinate_binding"]["unit"].as_str(),
        Some("page_block_line_token")
    );
    assert!(manifest["oracle"]["key_fields"].as_sequence().is_some());
}
