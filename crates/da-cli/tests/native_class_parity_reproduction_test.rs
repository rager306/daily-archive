//! M515-v2mvmc / S02 / T01 — the shared class-parity projection reproduces the
//! frozen extraction thresholds.
//!
//! The two `extraction.native_capability_recall` thresholds are distinguished
//! only by stratum: the micro occurrence recall and the weakest class.  This test
//! drives the *shared* projection (`native_class_parity`) over the frozen
//! reference TEI the manifest pins and requires the two numbers to equal the
//! prospectively pinned reference fractions, read by pointer — never restated as
//! literals:
//!
//! * the reference TEI byte pins, the source-consumed class set and the reference
//!   profile/consolidation fingerprints from
//!   `data/parse/acceptance_manifest.yaml#external_holdout_contract.release_probe`;
//! * the expected micros and weakest fraction from
//!   `artifacts/parse_compat/absorption_progress_reference_pins.json#values`.
//!
//! A hermetic refusal test proves a dropped class is named
//! (`ClassParityClassMissing`), not silently averaged into a shrunken
//! denominator.

use da_cli::native_class_parity::{
    CLASS_PARITY_CLASS_MISSING, ClassParityCounts, compose_native_document,
    measure_document_class_profiles,
};
use da_cli::native_composition_measurement::{build_admitted_band, measure_source};
use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
const PINS_REL: &str = "artifacts/parse_compat/absorption_progress_reference_pins.json";
/// A git-tracked frozen reference TEI, used only by the hermetic refusal test.
const REFERENCE_TEI: &str = include_str!("../../da-parse/tests/fixtures/holdout/1203.2295.tei.xml");

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_yaml() -> Yaml {
    let text = fs::read_to_string(repo_root().join(MANIFEST_REL))
        .expect("the acceptance manifest must be readable");
    serde_yaml::from_str(&text).expect("the acceptance manifest must be valid YAML")
}

fn load_pins() -> Json {
    let text = fs::read_to_string(repo_root().join(PINS_REL))
        .expect("the reference pins must be readable");
    serde_json::from_str(&text).expect("the reference pins must be valid JSON")
}

/// The pinned entry for one threshold, by pointer (never a restated literal).
fn find_pin<'a>(pins: &'a Json, threshold_id: &str) -> &'a Json {
    pins.get("values")
        .and_then(Json::as_array)
        .expect("pins.values")
        .iter()
        .find(|value| value.get("threshold_id").and_then(Json::as_str) == Some(threshold_id))
        .unwrap_or_else(|| panic!("the pins must declare {threshold_id}"))
}

/// The frozen reference profile/consolidation fingerprint pair the probe carries
/// into `NativeFulltextCompositionInput`, reconstructed from DATA exactly as the
/// probe's `read_lane` does.
fn reference_fingerprints(lane: &Yaml) -> (String, String) {
    let profile = lane
        .get("reference_profile")
        .and_then(Yaml::as_str)
        .expect("release_probe.reference_profile");
    let flags = lane
        .get("reference_consolidation_flags")
        .and_then(Yaml::as_mapping)
        .expect("release_probe.reference_consolidation_flags");
    let mut flag_pairs: Vec<String> = flags
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                key.as_str().expect("flag key"),
                value.as_str().expect("flag value")
            )
        })
        .collect();
    flag_pairs.sort();
    (
        format!("profile={profile}"),
        format!("consolidation={}", flag_pairs.join(",")),
    )
}

#[test]
fn shared_class_parity_reproduces_the_frozen_extraction_thresholds() {
    let manifest = load_yaml();
    let lane = manifest
        .get("external_holdout_contract")
        .and_then(|contract| contract.get("release_probe"))
        .expect("external_holdout_contract.release_probe");
    let (profile_fingerprint, config_fingerprint) = reference_fingerprints(lane);

    let source_consumed_classes: BTreeSet<String> = lane
        .get("class_parity")
        .and_then(|parity| parity.get("source_consumed_classes"))
        .and_then(Yaml::as_sequence)
        .expect("release_probe.class_parity.source_consumed_classes")
        .iter()
        .map(|class| {
            class
                .as_str()
                .expect("source_consumed_classes entries are strings")
                .to_string()
        })
        .collect();
    assert!(
        !source_consumed_classes.is_empty(),
        "the source-consumed class set is DATA, never empty"
    );

    let pins = load_pins();
    let micros_pin = find_pin(&pins, "extraction_micro_occurrence")
        .get("fraction")
        .expect("extraction_micro_occurrence.fraction");
    let expected_micros = micros_pin
        .get("numerator")
        .and_then(Json::as_u64)
        .expect("micro numerator");
    assert_eq!(
        micros_pin.get("denominator").and_then(Json::as_u64),
        Some(1_000_000),
        "the micro fraction is frozen as integer micros"
    );
    let weakest_pin = find_pin(&pins, "extraction_weakest_family")
        .get("fraction")
        .expect("extraction_weakest_family.fraction");
    let expected_weakest = (
        weakest_pin
            .get("numerator")
            .and_then(Json::as_u64)
            .expect("weakest numerator"),
        weakest_pin
            .get("denominator")
            .and_then(Json::as_u64)
            .expect("weakest denominator"),
    );

    let rows = lane
        .get("reference_tei")
        .and_then(Yaml::as_sequence)
        .expect("release_probe.reference_tei");
    assert!(
        !rows.is_empty(),
        "the measurable population must not be empty"
    );

    let mut profiles = Vec::new();
    for row in rows {
        let article_id = row
            .get("article_id")
            .and_then(Yaml::as_str)
            .expect("article_id");
        let path = row.get("path").and_then(Yaml::as_str).expect("path");
        let declared_sha256 = row.get("sha256").and_then(Yaml::as_str).expect("sha256");
        let size_bytes = row
            .get("size_bytes")
            .and_then(Yaml::as_u64)
            .expect("size_bytes");

        let bytes = fs::read(repo_root().join(path))
            .unwrap_or_else(|error| panic!("cannot read the pinned TEI {path}: {error}"));
        let observed = format!("sha256:{:x}", Sha256::digest(&bytes));
        assert_eq!(
            observed, declared_sha256,
            "the reference TEI {path} drifted from its declared byte pin"
        );
        assert_eq!(
            bytes.len() as u64,
            size_bytes,
            "the reference TEI {path} drifted from its declared size"
        );
        let xml = String::from_utf8(bytes).expect("the frozen TEI is UTF-8");
        let identity_sha256 = declared_sha256
            .strip_prefix("sha256:")
            .unwrap_or(declared_sha256)
            .to_string();

        let source = measure_source(article_id, &identity_sha256, &xml)
            .unwrap_or_else(|error| panic!("measure_source {article_id}: {error}"));
        let admitted = build_admitted_band(&source)
            .unwrap_or_else(|error| panic!("build_admitted_band {article_id}: {error}"));
        let emission = compose_native_document(
            &profile_fingerprint,
            &config_fingerprint,
            &source,
            &admitted,
            &xml,
        )
        .unwrap_or_else(|error| panic!("compose_native_document {article_id}: {error}"));
        let document_profiles = measure_document_class_profiles(
            &source,
            &xml,
            &admitted,
            &emission,
            &source_consumed_classes,
        )
        .unwrap_or_else(|error| panic!("measure_document_class_profiles {article_id}: {error}"));
        profiles.extend(document_profiles);
    }

    let counts = ClassParityCounts::from_profiles(&profiles);
    assert!(
        counts.native_denominator > 0,
        "the micro denominator must be non-empty"
    );
    assert_eq!(
        counts.native_capability_recall_micros(),
        Some(expected_micros),
        "the shared projection must reproduce the pinned micro occurrence recall"
    );
    let weakest = counts
        .weakest_class()
        .expect("the weakest stratum must have a denominator");
    assert_eq!(
        (weakest.1 as u64, weakest.2 as u64),
        expected_weakest,
        "the shared projection must reproduce the pinned weakest class"
    );
}

#[test]
fn an_emission_without_its_class_carrying_anchors_is_refused_by_name() {
    let source = measure_source("1203.2295", "", REFERENCE_TEI)
        .expect("the tracked reference fixture must be measurable");
    let admitted =
        build_admitted_band(&source).expect("the tracked reference fixture must admit its band");
    // `<TEI></TEI>` carries no `teiHeader`/`listBibl`, so the class set cannot be
    // enumerated: the projection must refuse it by name rather than panic.
    let error = measure_document_class_profiles(
        &source,
        REFERENCE_TEI,
        &admitted,
        "<TEI></TEI>",
        &BTreeSet::new(),
    )
    .expect_err("an incomplete class set must be refused");
    assert_eq!(
        error.reason(),
        CLASS_PARITY_CLASS_MISSING,
        "the refusal must be named, not a silent shrunken denominator"
    );
}
