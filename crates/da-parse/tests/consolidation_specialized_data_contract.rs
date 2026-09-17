//! Independent DATA/source contract for S05 specialized metadata.
//!
//! This target intentionally imports no da-parse symbols. It validates the
//! redacted Java ledger and schema-shaped YAML as an evidence boundary only.

use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str = "artifacts/parse_compat/java_grobid_consolidation_specialized_source.json";
const DATA: &str = "data/parse/consolidation_specialized_semantics.yaml";
const SOURCE_SHA: &str = "5dfcb5f73eb551e4ef6a4a2fd8fafb208eca58b0a2c1c969e3b8dd8cd34a66ec";
const LATTICE_SHA: &str = "6e10a02eaeab1d1a39191451874d3b4a14b80c74dc9da45bb8f35a7b329d5f0f";
const SEGMENTATION_SHA: &str = "2bc5d8b9fe03e01eae94f7373d4777dec1318f7eacd59feb3c8237b48fd5e0ae";
const FULLTEXT_SHA: &str = "b39736cf64e3914c7fe9f59f43be7e8ef0b5c7621d6541aeb1195cce50302989";
const JAVA_REVISION: &str = "0b0ab53b159b32792a01fca1d9d04fd02cf8b24b";
const JAVA_AUTHORITY: &str = "java-grobid-http@0.9.0-crf";
const FAMILIES: [&str; 6] = [
    "funding",
    "acknowledgement",
    "licence",
    "copyright",
    "software",
    "consolidation",
];
const DISPOSITIONS: [&str; 4] = [
    "source_not_established",
    "unsupported",
    "enrichment_not_required",
    "propagated_terminal",
];
const FORBIDDEN_KEYS: [&str; 14] = [
    "raw_pdf",
    "pdf_bytes",
    "raw_text",
    "text",
    "tei_xml",
    "raw_tei",
    "page_image",
    "images",
    "path",
    "physical_path",
    "hostname",
    "executable",
    "vendor_path",
    "credentials",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn json(path: &str) -> Json {
    serde_json::from_str(&fs::read_to_string(root().join(path)).expect(path)).expect(path)
}

fn yaml(path: &str) -> Yaml {
    serde_yaml::from_str(&fs::read_to_string(root().join(path)).expect(path)).expect(path)
}

fn at<'a>(value: &'a Yaml, keys: &[&str]) -> &'a Yaml {
    keys.iter().fold(value, |current, key| {
        current
            .get(*key)
            .unwrap_or_else(|| panic!("missing DATA key {key}"))
    })
}

fn strings(value: &Yaml, keys: &[&str]) -> Vec<String> {
    at(value, keys)
        .as_sequence()
        .expect("DATA sequence")
        .iter()
        .map(|item| item.as_str().expect("DATA string").to_owned())
        .collect()
}

fn json_strings(value: &Json, output: &mut Vec<String>) {
    match value {
        Json::String(item) => output.push(item.clone()),
        Json::Array(items) => items.iter().for_each(|item| json_strings(item, output)),
        Json::Object(items) => items.values().for_each(|item| json_strings(item, output)),
        _ => {}
    }
}

fn yaml_strings(value: &Yaml, output: &mut Vec<String>) {
    match value {
        Yaml::String(item) => output.push(item.clone()),
        Yaml::Sequence(items) => items.iter().for_each(|item| yaml_strings(item, output)),
        Yaml::Mapping(items) => items.values().for_each(|item| yaml_strings(item, output)),
        _ => {}
    }
}

fn forbidden_keys(value: &Json, location: &str, output: &mut Vec<String>) {
    match value {
        Json::Object(items) => {
            for (key, child) in items {
                if FORBIDDEN_KEYS.contains(&key.as_str()) {
                    output.push(format!("{location}.{key}"));
                }
                forbidden_keys(child, &format!("{location}.{key}"), output);
            }
        }
        Json::Array(items) => items.iter().enumerate().for_each(|(index, item)| {
            forbidden_keys(item, &format!("{location}[{index}]"), output)
        }),
        _ => {}
    }
}

fn exact_strings(value: &Yaml, keys: &[&str], expected: &[&str]) -> bool {
    let actual: BTreeSet<_> = strings(value, keys).into_iter().collect();
    actual == expected.iter().map(|item| (*item).to_owned()).collect()
}

fn validate(source: &Json, data: &Yaml) -> Result<(), String> {
    if source["schema_version"] != "java-grobid-consolidation-specialized-source.v1"
        || source["vendor_revision"] != JAVA_REVISION
        || source["authority"] != JAVA_AUTHORITY
    {
        return Err("Java source identity drift".into());
    }
    let source_files = source["source_files"]
        .as_object()
        .ok_or("source_files missing")?;
    if source_files.len() != 3
        || source_files.values().any(|file| {
            file["logical_path"].as_str().is_none()
                || file["sha256"].as_str().is_none_or(|sha| sha.len() != 64)
        })
    {
        return Err("source ledger must contain three digest-only files".into());
    }
    let families = source["families"]
        .as_object()
        .ok_or("ledger families missing")?;
    if families.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != FAMILIES.into_iter().collect()
    {
        return Err("ledger family vocabulary is not exactly six families".into());
    }
    for family in FAMILIES {
        let entry = &families[family];
        let status = entry["source_status"]
            .as_str()
            .ok_or("missing source status")?;
        if !["established", "gap"].contains(&status) {
            return Err(format!("unknown source status for {family}"));
        }
        if entry["source_seam"].is_null() && status != "gap" {
            return Err(format!("established family {family} has no seam"));
        }
        if entry["positive_semantics"].as_str() != Some("source_not_established")
            && family != "consolidation"
        {
            return Err(format!("invented positive semantics for {family}"));
        }
    }
    if source["evidence_policy"]["raw_java_source_embedded"] != false
        || source["evidence_policy"]["raw_payloads"] != false
        || source["evidence_policy"]["import_eligible"] != false
        || source["evidence_policy"]["graph_write"] != "forbidden"
        || source["evidence_policy"]["auto_promote"] != "forbidden"
        || source["evidence_policy"]["production_route"] != "forbidden"
    {
        return Err("source evidence policy unlocked".into());
    }

    if at(data, &["schema_version"]).as_str() != Some("consolidation-specialized-semantics.v1")
        || at(data, &["source_contract", "ledger_schema"]).as_str()
            != Some("java-grobid-consolidation-specialized-source.v1")
        || at(data, &["source_contract", "java_revision"]).as_str() != Some(JAVA_REVISION)
        || at(data, &["source_contract", "java_authority"]).as_str() != Some(JAVA_AUTHORITY)
        || at(data, &["source_contract", "binding"]).as_str() != Some("sha256_and_schema_exact")
    {
        return Err("DATA source binding drift".into());
    }
    if !exact_strings(data, &["families", "required"], &FAMILIES)
        || at(data, &["families", "closed"]).as_bool() != Some(true)
    {
        return Err("DATA family vocabulary is not closed".into());
    }
    for family in FAMILIES {
        let entry = at(data, &["families", family]);
        let expected_positive = if family == "consolidation" {
            "profile_wire_only"
        } else {
            "source_not_established"
        };
        if entry["positive_disposition"].as_str() != Some(expected_positive)
            || !exact_strings(
                entry,
                &["allowed_dispositions"],
                if family == "consolidation" {
                    &[
                        "structural_default",
                        "rich_opt_in",
                        "enrichment_not_required",
                        "propagated_terminal",
                    ]
                } else {
                    &[
                        "source_not_established",
                        "unsupported",
                        "propagated_terminal",
                    ]
                },
            )
        {
            return Err(format!("family disposition vocabulary drift for {family}"));
        }
    }
    for (keys, expected) in [
        (&["corpus_binding", "documents"][..], 3),
        (&["corpus_binding", "pages_per_lane"][..], 202),
        (&["corpus_binding", "lane_pages"][..], 606),
        (&["corpus_binding", "document_lane_outcomes"][..], 9),
        (&["corpus_binding", "unpooled_cells"][..], 54),
        (&["independent_oracle", "cells"][..], 54),
    ] {
        if at(data, keys).as_u64() != Some(expected) {
            return Err(format!("denominator drift at {keys:?}"));
        }
    }
    for (path, keys, expected) in [
        (
            "data/parse/layout_lattice_parity.yaml",
            &["source_contract", "lattice_sha256"][..],
            LATTICE_SHA,
        ),
        (
            "data/parse/document_segmentation.yaml",
            &["source_contract", "segmentation_sha256"][..],
            SEGMENTATION_SHA,
        ),
        (
            "data/parse/fulltext_block_semantics.yaml",
            &["source_contract", "fulltext_sha256"][..],
            FULLTEXT_SHA,
        ),
    ] {
        let actual = format!(
            "{:x}",
            Sha256::digest(fs::read(root().join(path)).expect(path))
        );
        if at(data, keys).as_str() != Some(expected) || actual != expected {
            return Err(format!("source SHA drift at {path}"));
        }
    }
    if at(data, &["profiles", "corpus_default"]).as_str() != Some("structural")
        || at(data, &["profiles", "structural", "consolidateHeader"]).as_i64() != Some(0)
        || at(data, &["profiles", "structural", "consolidateCitations"]).as_i64() != Some(0)
        || at(data, &["profiles", "rich", "opt_in"]).as_bool() != Some(true)
        || at(data, &["profiles", "rich", "separately_cached"]).as_bool() != Some(true)
        || at(data, &["profiles", "enrichment_required_for_base_parse"]).as_bool() != Some(false)
        || !exact_strings(
            data,
            &["profiles", "independently_selectable"],
            &["header", "references"],
        )
    {
        return Err("profile or enrichment policy drift".into());
    }
    if at(data, &["policy_locks", "production_authority"]).as_str() != Some(JAVA_AUTHORITY)
        || at(data, &["policy_locks", "import_eligible"]).as_bool() != Some(false)
        || at(data, &["policy_locks", "decision_point"]).as_u64() != Some(6)
        || at(data, &["policy_locks", "graph_write"]).as_str() != Some("forbidden")
        || at(data, &["policy_locks", "auto_promote"]).as_str() != Some("forbidden")
        || at(data, &["policy_locks", "production_route"]).as_str() != Some("forbidden")
        || at(data, &["policy_locks", "winner_selection"]).as_str() != Some("forbidden")
        || at(data, &["policy_locks", "pooling"]).as_str() != Some("forbidden")
    {
        return Err("policy lock drift".into());
    }
    if !exact_strings(data, &["outcomes", "closed_dispositions"], &DISPOSITIONS)
        || at(
            data,
            &["outcomes", "positive_semantics_without_source_proof"],
        )
        .as_str()
            != Some("forbidden")
        || at(data, &["outcomes", "invented_winner"]).as_str() != Some("forbidden")
    {
        return Err("outcome vocabulary drift".into());
    }
    if !exact_strings(data, &["redaction", "forbidden_keys"], &FORBIDDEN_KEYS)
        || at(data, &["redaction", "digest_prefix"]).as_str() != Some("sha256:")
    {
        return Err("redaction contract drift".into());
    }
    let mut values = Vec::new();
    json_strings(source, &mut values);
    yaml_strings(data, &mut values);
    if values.iter().any(|value| {
        value.starts_with('/')
            || value.contains("/root/")
            || value.contains("/tmp/")
            || value.contains("http://")
            || value.contains("https://")
    }) {
        return Err("host, URL, or physical path leakage".into());
    }
    let mut keys = Vec::new();
    forbidden_keys(source, "source", &mut keys);
    if !keys.is_empty() {
        return Err(format!("raw source key leakage: {keys:?}"));
    }
    Ok(())
}

#[test]
fn source_and_data_contract_is_closed_and_sha_bound() {
    let source = json(SOURCE);
    let data = yaml(DATA);
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(fs::read(root().join(SOURCE)).unwrap())
        ),
        SOURCE_SHA
    );
    validate(&source, &data).unwrap();
}

#[test]
fn adversarial_mutations_fail_closed() {
    let source = json(SOURCE);
    let data = yaml(DATA);

    let mut unknown_family = data.clone();
    unknown_family["families"]["required"]
        .as_sequence_mut()
        .unwrap()
        .push(Yaml::String("invented".into()));
    assert!(validate(&source, &unknown_family).is_err());

    let mut unknown_reason = data.clone();
    unknown_reason["outcomes"]["closed_dispositions"]
        .as_sequence_mut()
        .unwrap()
        .push(Yaml::String("maybe".into()));
    assert!(validate(&source, &unknown_reason).is_err());

    let mut positive = data.clone();
    positive["families"]["software"]["positive_disposition"] =
        Yaml::String("selected_software".into());
    assert!(validate(&source, &positive).is_err());

    let mut winner = source.clone();
    winner["families"]["software"]["positive_semantics"] = Json::String("selected".into());
    assert!(validate(&winner, &data).is_err());

    let mut denominator = data.clone();
    denominator["corpus_binding"]["lane_pages"] = Yaml::Number(605.into());
    assert!(validate(&source, &denominator).is_err());

    let mut sha = data.clone();
    sha["source_contract"]["fulltext_sha256"] = Yaml::String("0".repeat(64));
    assert!(validate(&source, &sha).is_err());

    let mut enrichment = data.clone();
    enrichment["profiles"]["enrichment_required_for_base_parse"] = Yaml::Bool(true);
    assert!(validate(&source, &enrichment).is_err());

    let mut unlock = data.clone();
    unlock["policy_locks"]["import_eligible"] = Yaml::Bool(true);
    assert!(validate(&source, &unlock).is_err());

    let mut leak = source.clone();
    leak["source_files"]["fulltext_parser"]["path"] =
        Json::String("/root/vendor/source.java".into());
    assert!(validate(&leak, &data).is_err());
}
