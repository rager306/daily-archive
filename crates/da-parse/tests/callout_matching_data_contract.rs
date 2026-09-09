//! Independent DATA/source contract for the S04 callout matching shadow.
//!
//! This test intentionally imports no da-parse symbols. It validates only the
//! redacted Java ledger and YAML contract, including fail-closed mutations.

use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str = "artifacts/parse_compat/java_grobid_callout_matching_source.json";
const DATA: &str = "data/parse/callout_matching_semantics.yaml";
const FULLTEXT: &str = "data/parse/fulltext_block_semantics.yaml";
const JAVA_REVISION: &str = "0b0ab53b159b32792a01fca1d9d04fd02cf8b24b";
const PRODUCTION: &str = "java-grobid-http@0.9.0-crf";
const SOURCE_SHA: &str = "c0729ce1f99d8af5b64071dd2b898e4c322fae9dcb3fdc33e32547300e203a0e";
const MARKERS: [&str; 4] = ["citation", "figure", "table", "equation"];
const TARGET_OUTCOMES: [&str; 4] = ["selected", "tie", "empty", "source_not_established"];
const UNRESOLVED: [&str; 6] = [
    "no_target",
    "target_not_in_inventory",
    "ambiguous",
    "illegal_adjacency",
    "unsupported_equation",
    "source_not_established",
];
const FORBIDDEN_KEYS: [&str; 15] = [
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
fn json(path: &str) -> Json {
    serde_json::from_str(&fs::read_to_string(root().join(path)).unwrap()).unwrap()
}
fn yaml(path: &str) -> Yaml {
    serde_yaml::from_str(&fs::read_to_string(root().join(path)).unwrap()).unwrap()
}
fn at<'a>(v: &'a Yaml, keys: &[&str]) -> &'a Yaml {
    keys.iter().fold(v, |v, k| &v[*k])
}
fn string_at<'a>(v: &'a Yaml, keys: &[&str]) -> &'a str {
    at(v, keys).as_str().expect("string field")
}
fn strings_yaml(v: &Yaml, out: &mut Vec<String>) {
    match v {
        Yaml::String(s) => out.push(s.clone()),
        Yaml::Sequence(xs) => xs.iter().for_each(|x| strings_yaml(x, out)),
        Yaml::Mapping(m) => m.values().for_each(|x| strings_yaml(x, out)),
        _ => {}
    }
}
fn strings_json(v: &Json, out: &mut Vec<String>) {
    match v {
        Json::String(s) => out.push(s.clone()),
        Json::Array(xs) => xs.iter().for_each(|x| strings_json(x, out)),
        Json::Object(m) => m.values().for_each(|x| strings_json(x, out)),
        _ => {}
    }
}
fn forbidden_keys(v: &Json, where_: &str, out: &mut Vec<String>) {
    if let Json::Object(m) = v {
        for (k, child) in m {
            if !where_.ends_with("evidence_policy") && FORBIDDEN_KEYS.contains(&k.as_str()) {
                out.push(format!("{where_}.{k}"));
            }
            forbidden_keys(child, &format!("{where_}.{k}"), out);
        }
    } else if let Json::Array(xs) = v {
        for (i, child) in xs.iter().enumerate() {
            forbidden_keys(child, &format!("{where_}[{i}]"), out);
        }
    }
}
fn exact(v: &Yaml, keys: &[&str], expected: &[&str]) -> bool {
    let actual: BTreeSet<_> = at(v, keys)
        .as_sequence()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    actual == expected.iter().copied().collect()
}

fn validate(source: &Json, data: &Yaml, _fulltext: &Yaml) -> Result<(), String> {
    if source["schema_version"] != "java-grobid-callout-matching-source.v1"
        || source["vendor_revision"] != JAVA_REVISION
        || source["authority"] != PRODUCTION
    {
        return Err("source identity drift".into());
    }
    let files = source["source_files"]
        .as_object()
        .ok_or("source files missing")?;
    if files.len() != 2
        || files.values().any(|f| {
            f["logical_path"].as_str().is_none()
                || f["sha256"].as_str().is_none_or(|x| x.len() != 64)
        })
    {
        return Err("source ledger drift".into());
    }
    let labels: BTreeSet<_> = source["marker_vocabulary"]["labels"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    if labels != MARKERS.into_iter().collect()
        || source["marker_vocabulary"]["closed"] != true
        || source["marker_vocabulary"]["unknown"]["disposition"] != "unsupported"
    {
        return Err("marker vocabulary drift".into());
    }
    for key in [
        "target_inventory",
        "majority_style",
        "adjacency",
        "equation_inventory",
    ] {
        if source["matching_evidence"][key]["status"] != "source_not_established" {
            return Err(format!("{key} invented evidence"));
        }
    }
    if source["matching_evidence"]["target_inventory"]["fuzzy_matching"] != "forbidden"
        || source["matching_evidence"]["equation_inventory"]["positive_target_invention"]
            != "forbidden"
    {
        return Err("target/equation lock drift".into());
    }

    if string_at(data, &["schema_version"]) != "callout-matching-semantics.v1"
        || string_at(data, &["source_contract", "java_revision"]) != JAVA_REVISION
        || string_at(data, &["source_contract", "java_source_sha256"]) != SOURCE_SHA
        || string_at(data, &["source_contract", "binding"]) != "sha256_and_schema_exact"
    {
        return Err("DATA source binding drift".into());
    }
    if string_at(data, &["source_contract", "fulltext_authority"]) != FULLTEXT
        || string_at(data, &["source_contract", "fulltext_schema"]) != "fulltext-block-semantics.v1"
    {
        return Err("S03 binding drift".into());
    }
    if !exact(data, &["marker_model", "vocabulary"], &MARKERS)
        || !exact(data, &["marker_model", "target_classes"], &MARKERS)
    {
        return Err("closed marker set drift".into());
    }
    if !exact(
        data,
        &["matching_semantics", "majority_style", "outcomes"],
        &TARGET_OUTCOMES,
    ) || !exact(
        data,
        &["terminal_and_unresolved", "unresolved_reasons"],
        &UNRESOLVED,
    ) {
        return Err("outcome vocabulary drift".into());
    }
    for keys in [
        &["matching_semantics", "target_inventory", "status"][..],
        &["matching_semantics", "majority_style", "source_proof"][..],
        &["matching_semantics", "adjacency", "source_proof"][..],
        &["matching_semantics", "equation", "inventory_source"][..],
    ] {
        if string_at(data, keys) != "source_not_established" {
            return Err(format!("unproven matching evidence at {keys:?}"));
        }
    }
    if string_at(
        data,
        &["matching_semantics", "equation", "unresolved_disposition"],
    ) != "source_not_established"
        || string_at(
            data,
            &["matching_semantics", "target_inventory", "fuzzy_guess"],
        ) != "forbidden"
        || string_at(data, &["matching_semantics", "equation", "positive_match"])
            != "forbidden_until_inventory_proven"
    {
        return Err("equation or target invention lock drift".into());
    }
    for (field, expected) in [
        (&["corpus_binding", "documents"][..], 3),
        (&["corpus_binding", "pages"][..], 202),
        (&["corpus_binding", "lanes"][..], 3),
        (&["corpus_binding", "pages_per_lane"][..], 202),
        (&["corpus_binding", "lane_pages"][..], 606),
        (&["corpus_binding", "document_lane_outcomes"][..], 9),
        (&["corpus_binding", "s03_cells"][..], 54),
    ] {
        if at(data, field).as_u64() != Some(expected) {
            return Err(format!("denominator drift at {field:?}"));
        }
    }
    if string_at(data, &["corpus_binding", "body_counts"]) != "pending_independent_oracle"
        || string_at(data, &["corpus_binding", "callout_counts"]) != "pending_independent_oracle"
    {
        return Err("invented count".into());
    }
    if string_at(data, &["policy_locks", "production_authority"]) != PRODUCTION
        || at(data, &["policy_locks", "import_eligible"]).as_bool() != Some(false)
        || string_at(data, &["policy_locks", "graph_write"]) != "forbidden"
        || string_at(data, &["policy_locks", "auto_promote"]) != "forbidden"
        || string_at(data, &["policy_locks", "cross_reference_linkage"]) != "forbidden"
        || at(data, &["policy_locks", "no_pooling"]).as_bool() != Some(true)
    {
        return Err("policy unlock".into());
    }
    if !exact(data, &["redaction", "forbidden_keys"], &FORBIDDEN_KEYS)
        || string_at(data, &["redaction", "digest_prefix"]) != "sha256:"
    {
        return Err("redaction drift".into());
    }
    if string_at(data, &["production_boundary", "new_calls"]) != "forbidden"
        || string_at(
            data,
            &["production_boundary", "production_import_graph_promotion"],
        ) != "forbidden"
        || at(
            data,
            &["production_boundary", "java_http_authority_unchanged"],
        )
        .as_bool()
            != Some(true)
    {
        return Err("production boundary unlocked".into());
    }
    if string_at(data, &["independent_oracle", "status"]) != "pending_independent_oracle"
        || string_at(data, &["independent_oracle", "common_mode_with_matcher"]) != "forbidden"
    {
        return Err("oracle independence drift".into());
    }
    let mut values = Vec::new();
    strings_json(source, &mut values);
    strings_yaml(data, &mut values);
    if values
        .iter()
        .any(|s| s.starts_with('/') || s.contains("/root/") || s.contains("/tmp/"))
    {
        return Err("physical path leakage".into());
    }
    let mut keys = Vec::new();
    forbidden_keys(source, "source", &mut keys);
    if !keys.is_empty() {
        return Err(format!("raw key leakage: {keys:?}"));
    }
    Ok(())
}

#[test]
fn source_and_data_contract_is_closed_and_exactly_bound() {
    let source_value = json(SOURCE);
    let data_value = yaml(DATA);
    let fulltext_value = yaml(FULLTEXT);
    if let Err(error) = validate(&source_value, &data_value, &fulltext_value) {
        panic!("valid callout contract: {error}");
    }
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(fs::read(root().join(SOURCE)).unwrap())
        ),
        SOURCE_SHA
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(fs::read(root().join(FULLTEXT)).unwrap())
        ),
        "fbe1a91083b0a5944a9c1cb0beb3674a65731478665d1545623b4d715fac5819"
    );
}

#[test]
fn adversarial_mutations_fail_closed() {
    let source = json(SOURCE);
    let data = yaml(DATA);
    let fulltext = yaml(FULLTEXT);
    let mut unknown = data.clone();
    unknown["marker_model"]["vocabulary"]
        .as_sequence_mut()
        .unwrap()
        .push(Yaml::String("unknown".into()));
    assert!(validate(&source, &unknown, &fulltext).is_err());
    let mut invented = data.clone();
    invented["matching_semantics"]["equation"]["inventory_source"] =
        Yaml::String("reviewed_inventory".into());
    assert!(validate(&source, &invented, &fulltext).is_err());
    let mut denominator = data.clone();
    denominator["corpus_binding"]["lane_pages"] = Yaml::Number(605.into());
    assert!(validate(&source, &denominator, &fulltext).is_err());
    let mut unlock = data.clone();
    unlock["policy_locks"]["import_eligible"] = Yaml::Bool(true);
    assert!(validate(&source, &unlock, &fulltext).is_err());
    let mut leak = source.clone();
    leak["source_files"]["fulltext_parser"]["path"] =
        Json::String("/root/vendor/source.java".into());
    assert!(validate(&leak, &data, &fulltext).is_err());
    let mut winner = source.clone();
    winner["matching_evidence"]["majority_style"]["status"] = Json::String("selected".into());
    assert!(validate(&winner, &data, &fulltext).is_err());
}
