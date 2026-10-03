//! M515-v2mvmc / S01 / T02 — the preservation fraction reproduces exactly from
//! the frozen reference TEI.
//!
//! This is the highest-risk step of the slice: a silent divergence in the
//! extracted measurement flips `preservation.occurrence_conservation` from met
//! to unmet, and a hardcoded expectation would hide it.  Every number this test
//! asserts is therefore read **by pointer** from the committed carriers:
//!
//! * `data/parse/acceptance_manifest.yaml` ->
//!   `external_holdout_contract.release_probe.reference_tei` (the 531 frozen
//!   reference rows, each with `article_id`, `path`, `sha256`, `size_bytes`);
//! * `artifacts/parse_compat/absorption_progress_reference_pins.json` ->
//!   `values[metric_id == "preservation.occurrence_conservation"].fraction`;
//! * `artifacts/parse_compat/external_holdout_release_probe.json` ->
//!   `evaluated_population.aggregate`, `class_profiles` and `class_narrowings`.
//!
//! The test is deliberately **ungated** (no `#![cfg(feature = ...)]`): the shared
//! module `da_cli::native_composition_measurement` compiles in the default
//! feature set, so `da-cli --tests` actually executes this target rather than
//! compiling it to an empty binary.
//!
//! If a frozen reference byte drifts the sha256 assertion fails; if the shared
//! measurement drifts the per-document reconciliation against the probe artifact
//! fails and names the first drifting document; if the pinned fraction changes
//! the numerator/denominator assertions fail.

use da_cli::native_composition_measurement::{
    build_admitted_band, class_admissions, measure_document_admitted_coverage, measure_source,
};
use da_cli::native_family_receipt::sha256_prefixed;
use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
const PINS_REL: &str = "artifacts/parse_compat/absorption_progress_reference_pins.json";
const PROBE_REL: &str = "artifacts/parse_compat/external_holdout_release_probe.json";
const PRESERVATION_METRIC_ID: &str = "preservation.occurrence_conservation";
const REFERENCE_TEI_COUNT: usize = 531;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels under the repository root")
        .to_path_buf()
}

fn read_bytes(root: &Path, rel: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(fs::read(root.join(rel))?)
}

fn read_text(root: &Path, rel: &str) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(root.join(rel))?)
}

fn yaml_str<'a>(row: &'a Yaml, key: &str) -> Result<&'a str, String> {
    row.get(key)
        .and_then(Yaml::as_str)
        .ok_or_else(|| format!("manifest row is missing string `{key}`"))
}

fn yaml_u64(row: &Yaml, key: &str) -> Result<u64, String> {
    row.get(key)
        .and_then(Yaml::as_u64)
        .ok_or_else(|| format!("manifest row is missing integer `{key}`"))
}

fn json_str<'a>(row: &'a Json, key: &str) -> Result<&'a str, String> {
    row.get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("artifact row is missing string `{key}`"))
}

fn json_u64(row: &Json, key: &str) -> Result<u64, String> {
    row.get(key)
        .and_then(Json::as_u64)
        .ok_or_else(|| format!("artifact row is missing integer `{key}`"))
}

#[test]
fn preservation_fraction_reproduces_from_the_frozen_reference_tei() -> Result<(), Box<dyn Error>> {
    let root = repo_root();

    // 1. the frozen reference identity, by pointer
    let manifest: Yaml = serde_yaml::from_str(&read_text(&root, MANIFEST_REL)?)?;
    let rows = manifest
        .get("external_holdout_contract")
        .and_then(|contract| contract.get("release_probe"))
        .and_then(|lane| lane.get("reference_tei"))
        .and_then(Yaml::as_sequence)
        .ok_or("manifest is missing external_holdout_contract.release_probe.reference_tei")?;
    assert_eq!(
        rows.len(),
        REFERENCE_TEI_COUNT,
        "the frozen reference_tei population must be {REFERENCE_TEI_COUNT} rows"
    );

    // 2. the pinned preservation fraction, by pointer (never a literal)
    let pins: Json = serde_json::from_str(&read_text(&root, PINS_REL)?)?;
    let fraction = pins
        .get("values")
        .and_then(Json::as_array)
        .and_then(|values| {
            values.iter().find(|value| {
                value.get("metric_id").and_then(Json::as_str) == Some(PRESERVATION_METRIC_ID)
            })
        })
        .and_then(|value| value.get("fraction"))
        .ok_or("pins are missing the preservation.occurrence_conservation fraction")?;
    let numerator = json_u64(fraction, "numerator")? as usize;
    let denominator = json_u64(fraction, "denominator")? as usize;
    assert!(
        denominator > 0,
        "the preservation denominator must be non-empty"
    );

    // 3 + 4. re-measure every frozen reference document and sum
    let mut sum_admitted = 0usize;
    let mut sum_source = 0usize;
    let mut per_document: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut paths: BTreeMap<String, String> = BTreeMap::new();
    for row in rows {
        let article_id = yaml_str(row, "article_id")?.to_string();
        let rel = yaml_str(row, "path")?.to_string();
        let pin = yaml_str(row, "sha256")?.to_string();
        let size_bytes = yaml_u64(row, "size_bytes")?;

        let bytes = read_bytes(&root, &rel)
            .map_err(|error| format!("{article_id}: cannot read {rel}: {error}"))?;
        assert_eq!(
            bytes.len() as u64,
            size_bytes,
            "{article_id}: size_bytes drifted for {rel}"
        );
        assert_eq!(
            sha256_prefixed(&bytes),
            pin,
            "{article_id}: sha256 drifted for {rel}"
        );
        let xml = std::str::from_utf8(&bytes)
            .map_err(|error| format!("{article_id}: {rel} is not UTF-8: {error}"))?;

        let counts = measure_document_admitted_coverage(xml)
            .map_err(|error| format!("{article_id}: {error}"))?;
        sum_admitted += counts.admitted;
        sum_source += counts.source_population;
        per_document.insert(
            article_id.clone(),
            (counts.admitted, counts.source_population),
        );
        paths.insert(article_id, rel);
    }
    assert_eq!(
        per_document.len(),
        REFERENCE_TEI_COUNT,
        "every frozen reference article identity must be measured exactly once"
    );
    assert_eq!(
        sum_admitted, numerator,
        "sum(admitted) must equal the pinned preservation numerator"
    );
    assert_eq!(
        sum_source, denominator,
        "sum(source_population) must equal the pinned preservation denominator"
    );

    // 5a. the probe artifact's aggregate is the same sum, by pointer
    let probe: Json = serde_json::from_str(&read_text(&root, PROBE_REL)?)?;
    let aggregate = probe
        .get("evaluated_population")
        .and_then(|population| population.get("aggregate"))
        .ok_or("probe artifact is missing evaluated_population.aggregate")?;
    assert_eq!(
        json_u64(aggregate, "admitted")? as usize,
        sum_admitted,
        "probe aggregate.admitted must equal sum(admitted)"
    );
    assert_eq!(
        json_u64(aggregate, "source_population")? as usize,
        sum_source,
        "probe aggregate.source_population must equal sum(source_population)"
    );
    assert_eq!(
        json_u64(aggregate, "documents")? as usize,
        REFERENCE_TEI_COUNT,
        "probe aggregate.documents must cover the frozen reference"
    );

    // 5b + 6. every per-document count must equal the probe artifact's, and the
    // first drift names its document.
    let profiles = probe
        .get("class_profiles")
        .and_then(Json::as_array)
        .ok_or("probe artifact is missing class_profiles")?;
    assert_eq!(
        profiles.len(),
        REFERENCE_TEI_COUNT,
        "class_profiles must cover the frozen reference"
    );
    for profile in profiles {
        let document_id = json_str(profile, "document_id")?;
        let classes = profile
            .get("classes")
            .and_then(Json::as_array)
            .ok_or_else(|| format!("{document_id}: class_profile is missing classes"))?;
        let mut expected_admitted = 0usize;
        let mut expected_source = 0usize;
        for class in classes {
            expected_admitted += json_u64(class, "admitted")? as usize;
            expected_source += json_u64(class, "source_population")? as usize;
        }
        let (actual_admitted, actual_source) =
            per_document.get(document_id).copied().ok_or_else(|| {
                format!("probe artifact names {document_id}, absent from the frozen reference_tei")
            })?;
        assert_eq!(
            (actual_admitted, actual_source),
            (expected_admitted, expected_source),
            "document {document_id} drifted: shared module measured \
             {actual_admitted}/{actual_source}, probe artifact records \
             {expected_admitted}/{expected_source}"
        );
    }

    // 5c. the narrowing rows for one real document must match the shared module's
    // per-class enumeration (the probe projects these from the same rows).
    let narrowings = probe
        .get("class_narrowings")
        .and_then(Json::as_array)
        .ok_or("probe artifact is missing class_narrowings")?;
    let spot_id = narrowings
        .first()
        .and_then(|row| row.get("document_id"))
        .and_then(Json::as_str)
        .ok_or("class_narrowings is empty")?
        .to_string();
    let spot_rel = paths
        .get(&spot_id)
        .ok_or_else(|| format!("class_narrowings names {spot_id}, absent from reference_tei"))?
        .clone();
    let spot_bytes = read_bytes(&root, &spot_rel)?;
    let spot_xml = std::str::from_utf8(&spot_bytes)
        .map_err(|error| format!("{spot_id}: {spot_rel} is not UTF-8: {error}"))?;
    let spot_source = measure_source("", "", spot_xml).map_err(|e| format!("{spot_id}: {e}"))?;
    let spot_admitted = build_admitted_band(&spot_source).map_err(|e| format!("{spot_id}: {e}"))?;
    let spot_classes = class_admissions(&spot_source, &spot_admitted, spot_xml);

    let mut narrowing_admitted = 0usize;
    let mut narrowing_source = 0usize;
    let mut narrowing_rows = 0usize;
    for row in narrowings {
        if row.get("document_id").and_then(Json::as_str) != Some(spot_id.as_str()) {
            continue;
        }
        let class = json_str(row, "class")?;
        let expected_admitted = json_u64(row, "admitted")? as usize;
        let expected_source = json_u64(row, "source_population")? as usize;
        let admission = spot_classes
            .iter()
            .find(|admission| admission.class() == class)
            .ok_or_else(|| format!("{spot_id}: the module does not enumerate class `{class}`"))?;
        assert_eq!(
            admission.admitted_len(),
            expected_admitted,
            "{spot_id} class {class}: admitted drifted"
        );
        assert_eq!(
            admission.source_population(),
            expected_source,
            "{spot_id} class {class}: source_population drifted"
        );
        narrowing_admitted += expected_admitted;
        narrowing_source += expected_source;
        narrowing_rows += 1;
    }
    assert!(
        narrowing_rows > 0,
        "class_narrowings must carry at least one row for {spot_id}"
    );
    let (spot_total_admitted, spot_total_source) = per_document
        .get(&spot_id)
        .copied()
        .ok_or_else(|| format!("{spot_id} was not measured in the reference loop"))?;
    assert!(
        narrowing_admitted <= spot_total_admitted && narrowing_source <= spot_total_source,
        "{spot_id}: narrowing rows cannot exceed the document's totals"
    );

    Ok(())
}

/// The frozen partition is not a silent filter: every document the lane cannot
/// measure is named with a reason, and the shared measurement must refuse it by
/// that reason rather than measure a fabricated zero.  This is the negative half
/// of the same surface the reproduction test exercises.
#[test]
fn unmeasurable_reference_exceptions_are_refused_by_name() -> Result<(), Box<dyn Error>> {
    let root = repo_root();
    let manifest: Yaml = serde_yaml::from_str(&read_text(&root, MANIFEST_REL)?)?;
    let exceptions = manifest
        .get("external_holdout_contract")
        .and_then(|contract| contract.get("release_probe"))
        .and_then(|lane| lane.get("reference_exceptions"))
        .and_then(Yaml::as_sequence)
        .ok_or(
            "manifest is missing external_holdout_contract.release_probe.reference_exceptions",
        )?;
    let absent_date = exceptions
        .iter()
        .find(|row| row.get("reason").and_then(Yaml::as_str) == Some("header_date_absent"))
        .ok_or("reference_exceptions declares no header_date_absent row")?;
    let article_id = yaml_str(absent_date, "article_id")?;
    let rel = yaml_str(absent_date, "path")?;
    let pin = yaml_str(absent_date, "sha256")?;

    let bytes = read_bytes(&root, rel)?;
    assert_eq!(
        sha256_prefixed(&bytes),
        pin,
        "{article_id}: sha256 drifted for {rel}"
    );
    let xml = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{article_id}: {rel} is not UTF-8: {error}"))?;

    let error = measure_document_admitted_coverage(xml).expect_err(
        "a frozen header without a dated element must be refused, never measured as zero",
    );
    assert!(
        error.reason().contains("no dated element"),
        "{article_id}: the refusal must name the missing dated element, got: {error}"
    );
    Ok(())
}
