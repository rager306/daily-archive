//! Generic DATA/source contract gate for S03/T01.
//!
//! This test deliberately imports no da-parse symbols and does not execute a
//! parser. It validates only the redacted JSON/YAML authorities and exercises
//! fail-closed mutations of their vocabulary, locks, denominators and redaction.

use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str = "artifacts/parse_compat/java_grobid_fulltext_semantics_source.json";
const DATA: &str = "data/parse/fulltext_block_semantics.yaml";
const REVISION: &str = "0b0ab53b159b32792a01fca1d9d04fd02cf8b24b";
const PRODUCTION: &str = "java-grobid-http@0.9.0-crf";
const AXES: [&str; 6] = [
    "paragraph",
    "section",
    "list_item",
    "note",
    "annex",
    "marker",
];
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const FORBIDDEN: [&str; 15] = [
    "pdf_bytes",
    "tei_xml",
    "raw_text",
    "body_text",
    "page_image",
    "images",
    "text",
    "path",
    "physical_path",
    "hostname",
    "executable",
    "vendor_path",
    "raw_source",
    "raw_tei",
    "raw_pdf",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn source() -> Json {
    serde_json::from_str(&fs::read_to_string(root().join(SOURCE)).expect("source readable"))
        .expect("source JSON")
}

fn data() -> Yaml {
    serde_yaml::from_str(&fs::read_to_string(root().join(DATA)).expect("DATA readable"))
        .expect("DATA YAML")
}

fn yaml_at<'a>(value: &'a Yaml, keys: &[&str]) -> &'a Yaml {
    keys.iter().fold(value, |current, key| &current[*key])
}

fn yaml_str<'a>(value: &'a Yaml, keys: &[&str]) -> &'a str {
    yaml_at(value, keys).as_str().expect("DATA string")
}

fn yaml_strings(value: &Yaml, out: &mut Vec<String>) {
    match value {
        Yaml::String(s) => out.push(s.clone()),
        Yaml::Sequence(items) => items.iter().for_each(|item| yaml_strings(item, out)),
        Yaml::Mapping(map) => map.values().for_each(|item| yaml_strings(item, out)),
        _ => {}
    }
}

fn json_strings(value: &Json, out: &mut Vec<String>) {
    match value {
        Json::String(s) => out.push(s.clone()),
        Json::Array(items) => items.iter().for_each(|item| json_strings(item, out)),
        Json::Object(map) => map.values().for_each(|item| json_strings(item, out)),
        _ => {}
    }
}

fn forbidden_json_keys(value: &Json, path: &str, out: &mut Vec<String>) {
    match value {
        Json::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN.contains(&key.as_str()) && !path.ends_with("evidence_policy") {
                    out.push(format!("{path}.{key}"));
                }
                forbidden_json_keys(child, &format!("{path}.{key}"), out);
            }
        }
        Json::Array(items) => items
            .iter()
            .enumerate()
            .for_each(|(i, item)| forbidden_json_keys(item, &format!("{path}[{i}]"), out)),
        _ => {}
    }
}

fn validate(source: &Json, data: &Yaml) -> Result<(), String> {
    if source["schema_version"] != "java-grobid-fulltext-semantics-source.v1"
        || source["vendor_revision"] != REVISION
        || source["authority"] != PRODUCTION
    {
        return Err("source identity drift".into());
    }
    let source_files = source["source_files"]
        .as_object()
        .ok_or("source files missing")?;
    if source_files.len() != 3
        || source_files.values().any(|file| {
            file["logical_path"].as_str().is_none()
                || file["sha256"].as_str().is_none_or(|sha| sha.len() != 64)
        })
    {
        return Err("source file ledger drift".into());
    }
    let expected_labels: BTreeSet<_> = [
        "paragraph",
        "item",
        "section",
        "citation_marker",
        "figure_marker",
        "table_marker",
        "equation_marker",
        "figure",
        "table",
        "equation",
        "equation_label",
        "other",
    ]
    .into_iter()
    .collect();
    let actual_labels: BTreeSet<_> = source["fulltext_labels"]
        .as_object()
        .ok_or("fulltext labels missing")?
        .keys()
        .map(String::as_str)
        .collect();
    if actual_labels != expected_labels {
        return Err("closed Java vocabulary drift".into());
    }
    if source["note_classifier"]["java_model_label"] != Json::Null
        || source["note_classifier"]["source_proof"] != "not_established"
        || source["note_classifier"]["positive_invention"] != "forbidden"
    {
        return Err("note invention policy drift".into());
    }

    if yaml_str(data, &["schema_version"]) != "fulltext-block-semantics.v1"
        || yaml_str(data, &["source_contract", "java_source_schema"])
            != "java-grobid-fulltext-semantics-source.v1"
        || yaml_str(data, &["source_contract", "java_revision"]) != REVISION
        || yaml_str(data, &["source_contract", "binding"]) != "sha256_and_schema_exact"
    {
        return Err("DATA source binding drift".into());
    }
    if yaml_str(data, &["policy_locks", "production_authority"]) != PRODUCTION
        || yaml_at(data, &["policy_locks", "import_eligible"]).as_bool() != Some(false)
        || yaml_str(data, &["policy_locks", "auto_promote"]) != "forbidden"
        || yaml_str(data, &["policy_locks", "graph_mutation"]) != "forbidden"
    {
        return Err("production lock drift".into());
    }
    let axes: BTreeSet<_> = yaml_at(data, &["semantic_model", "evaluation_axes"])
        .as_sequence()
        .ok_or("axes missing")?
        .iter()
        .map(|v| v.as_str().unwrap_or("?"))
        .collect();
    if axes != AXES.into_iter().collect() {
        return Err("evaluation axes drift".into());
    }
    if yaml_at(data, &["semantic_model", "leaf_roles"])
        .as_sequence()
        .unwrap()
        .len()
        != 5
        || yaml_str(data, &["semantic_model", "annex_parent_scope"]) != "s02_annex_units_only"
        || yaml_str(data, &["semantic_model", "sibling_overlap"]) != "forbidden"
        || yaml_str(data, &["policy_locks", "resolution"]) != "forbidden"
        || yaml_str(data, &["policy_locks", "target_matching"]) != "forbidden"
        || yaml_str(data, &["policy_locks", "cross_reference_linkage"]) != "forbidden"
    {
        return Err("parent/leaf or marker policy drift".into());
    }
    let marker_subtypes: BTreeSet<_> = yaml_at(data, &["semantic_model", "marker_subtypes"])
        .as_sequence()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    if marker_subtypes
        != ["citation", "figure", "table", "equation"]
            .into_iter()
            .collect()
    {
        return Err("marker subtype drift".into());
    }
    if yaml_at(data, &["semantic_model", "note", "java_model_label"]) != &Yaml::Null
        || yaml_str(data, &["semantic_model", "note", "source_proof"]) != "not_established"
        || yaml_str(data, &["semantic_model", "note", "positive_invention"]) != "forbidden"
    {
        return Err("note semantics drift".into());
    }
    if yaml_at(data, &["corpus_binding", "documents"]).as_u64() != Some(3)
        || yaml_at(data, &["corpus_binding", "pages_per_lane"]).as_u64() != Some(202)
        || yaml_at(data, &["corpus_binding", "lanes"]).as_u64() != Some(3)
        || yaml_at(data, &["corpus_binding", "lane_pages"]).as_u64() != Some(606)
    {
        return Err("corpus denominator drift".into());
    }
    let lanes: Vec<_> = yaml_at(data, &["corpus_binding", "lane_ids"])
        .as_sequence()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    if lanes != LANES {
        return Err("lane vocabulary drift".into());
    }
    if yaml_str(data, &["corpus_binding", "body_counts"]) != "pending_independent_oracle"
        || yaml_str(data, &["independent_oracle", "status"]) != "pending_independent_oracle"
    {
        return Err("denominator invention".into());
    }
    let mut values = Vec::new();
    json_strings(source, &mut values);
    yaml_strings(data, &mut values);
    if values
        .iter()
        .any(|s| s.starts_with('/') || s.contains("/root/") || s.contains("/tmp/"))
    {
        return Err("physical path leaked".into());
    }
    let mut keys = Vec::new();
    forbidden_json_keys(source, "source", &mut keys);
    if !keys.is_empty() {
        return Err(format!("raw source keys: {keys:?}"));
    }
    if yaml_at(data, &["redaction", "forbidden_keys"])
        .as_sequence()
        .unwrap()
        .iter()
        .any(|key| !FORBIDDEN.contains(&key.as_str().unwrap()))
    {
        return Err("redaction vocabulary drift".into());
    }
    if yaml_str(data, &["production_boundary", "new_calls"]) != "forbidden"
        || yaml_str(
            data,
            &["production_boundary", "production_import_graph_promotion"],
        ) != "forbidden"
    {
        return Err("production boundary unlocked".into());
    }
    Ok(())
}

#[test]
fn source_and_data_ledgers_are_closed_and_exactly_bound() {
    let source_value = source();
    let data_value = data();
    validate(&source_value, &data_value).expect("valid source/DATA contract");
    let source_hash = format!(
        "{:x}",
        Sha256::digest(fs::read(root().join(SOURCE)).unwrap())
    );
    assert_eq!(
        yaml_str(&data_value, &["source_contract", "java_source_sha256"]),
        source_hash
    );
}

#[test]
fn adversarial_mutations_fail_closed() {
    let source_value = source();
    let data_value = data();
    type Mutation = (&'static str, fn(&mut Yaml));
    let mutations: [Mutation; 6] = [
        ("vocabulary drift", |d: &mut Yaml| {
            d["semantic_model"]["evaluation_axes"] =
                Yaml::Sequence(vec![Yaml::String("paragraph".into())])
        }),
        ("note invention", |d: &mut Yaml| {
            d["semantic_model"]["note"]["source_proof"] = Yaml::String("positive".into())
        }),
        ("marker resolution", |d: &mut Yaml| {
            d["policy_locks"]["resolution"] = Yaml::String("allowed".into())
        }),
        ("illegal annex parent", |d: &mut Yaml| {
            d["semantic_model"]["annex_parent_scope"] = Yaml::String("all_regions".into())
        }),
        ("denominator erosion", |d: &mut Yaml| {
            d["corpus_binding"]["lane_pages"] = Yaml::Number(605.into())
        }),
        ("production unlock", |d: &mut Yaml| {
            d["policy_locks"]["import_eligible"] = Yaml::Bool(!false)
        }),
    ];
    for (name, mutate) in mutations {
        let mut mutated = data_value.clone();
        mutate(&mut mutated);
        assert!(
            validate(&source_value, &mutated).is_err(),
            "mutation accepted: {name}"
        );
    }
    let mut raw = source_value.clone();
    raw["raw_text"] = Json::String("leak".into());
    assert!(validate(&raw, &data_value).is_err(), "raw payload accepted");
}
