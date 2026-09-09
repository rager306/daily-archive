//! Fail-closed DATA gate for the Java DocumentSource/pdfalto layout boundary.
//! This test reads only repository-owned, redacted DATA and never executes a
//! donor, parser, vendor checkout, subprocess, or corpus fixture.

use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str = "artifacts/parse_compat/java_grobid_layout_source.json";
const CONTRACT: &str = "data/parse/layout_lattice_parity.yaml";
const BAKEOFF: &str = "data/parse/layout_bakeoff.yaml";
const REVISION: &str = "0b0ab53b159b32792a01fca1d9d04fd02cf8b24b";
const PRODUCTION: &str = "java-grobid-http@0.9.0-crf";
const FORBIDDEN_KEYS: &[&str] = &[
    "raw_pdf",
    "pdf_bytes",
    "raw_text",
    "body_text",
    "tei_xml",
    "raw_tei",
    "page_image",
    "images",
    "executable_path",
    "host_path",
    "physical_path",
    "vendor_path",
];
const EXIT_CODES: &[&str] = &["0", "5", "4", "98", "126", "127", "143", "other_nonzero"];
const FAILURES: &[&str] = &[
    "missing",
    "checksum_mismatch",
    "malformed",
    "unsupported",
    "timeout",
    "nonzero",
    "oversize",
    "encrypted",
    "truncated",
    "invalid_geometry",
    "invalid_topology",
    "unknown_exit_code",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn json() -> Json {
    serde_json::from_str(
        &fs::read_to_string(root().join(SOURCE)).expect("source artifact readable"),
    )
    .expect("source artifact is JSON")
}

fn yaml(path: &str) -> Yaml {
    serde_yaml::from_str(&fs::read_to_string(root().join(path)).expect("DATA readable"))
        .expect("DATA is YAML")
}

fn at<'a>(value: &'a Yaml, path: &[&str]) -> &'a Yaml {
    path.iter().fold(value, |value, key| &value[*key])
}

fn string_at<'a>(value: &'a Yaml, path: &[&str]) -> &'a str {
    at(value, path).as_str().expect("DATA string")
}

fn collect_forbidden_keys(value: &Json, path: &str, found: &mut Vec<String>) {
    match value {
        Json::Object(map) => {
            for (key, child) in map {
                let policy_declaration = path.starts_with("source.evidence_policy");
                if !policy_declaration && FORBIDDEN_KEYS.contains(&key.as_str()) {
                    found.push(format!("{path}.{key}"));
                }
                collect_forbidden_keys(child, &format!("{path}.{key}"), found);
            }
        }
        Json::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_forbidden_keys(child, &format!("{path}[{index}]"), found);
            }
        }
        _ => {}
    }
}

fn collect_strings(value: &Json, found: &mut Vec<String>) {
    match value {
        Json::String(value) => found.push(value.clone()),
        Json::Object(map) => map.values().for_each(|value| collect_strings(value, found)),
        Json::Array(items) => items.iter().for_each(|value| collect_strings(value, found)),
        _ => {}
    }
}

fn validate_source(value: &Json) -> Result<(), String> {
    if value["schema_version"] != "java-grobid-layout-source.v1" {
        return Err("source schema drift".into());
    }
    if value["vendor_revision"] != REVISION {
        return Err("vendor revision drift".into());
    }
    let files = value["source_files"]
        .as_object()
        .ok_or("source_files missing")?;
    let expected: BTreeSet<_> = [
        (
            "document_source",
            "grobid-core/src/main/java/org/grobid/core/document/DocumentSource.java",
            "969db636abd435b632f720ad159f7526cd53aafacfbb9c2609bbb3f14287f13a",
        ),
        (
            "pdfalto_sax_handler",
            "grobid-core/src/main/java/org/grobid/core/sax/PDFALTOSaxHandler.java",
            "cc502102be58a5145b18aa81292023f72105dde7e94158eae670e0e7ffff5bb4",
        ),
        (
            "page",
            "grobid-core/src/main/java/org/grobid/core/layout/Page.java",
            "6273b466344c48fa90c57075c23b5c96ef8a16c6d00082d720ea07fb63d087d9",
        ),
        (
            "block",
            "grobid-core/src/main/java/org/grobid/core/layout/Block.java",
            "e4c30e9a8db29a133f487a7308216cb4f24d00172675ca9ea8b1a0c92e010629",
        ),
        (
            "layout_token",
            "grobid-core/src/main/java/org/grobid/core/layout/LayoutToken.java",
            "67c31cd8a3804df51278f5fd6a0e053ecd4259e2a3e430ed329d1007afefaf9c",
        ),
    ]
    .into_iter()
    .map(|(name, path, sha)| (name, (path, sha)))
    .collect();
    let actual: BTreeSet<_> = files
        .iter()
        .map(|(name, file)| {
            let path = file["logical_path"]
                .as_str()
                .ok_or("logical path missing")?;
            let sha = file["sha256"].as_str().ok_or("source hash missing")?;
            if sha.len() != 64 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!("invalid hash for {name}"));
            }
            Ok((name.as_str(), (path, sha)))
        })
        .collect::<Result<_, String>>()?;
    if actual != expected {
        return Err("source file set or hash drift".into());
    }
    if value["exit_disposition"]["success"] != Json::Array(vec![0.into()])
        || value["exit_disposition"]["success_with_warning"] != Json::Array(vec![5.into()])
        || value["exit_disposition"]["typed_failure"]
            != Json::Array(vec![
                4.into(),
                98.into(),
                126.into(),
                127.into(),
                143.into(),
            ])
    {
        return Err("exit taxonomy drift".into());
    }
    let mut forbidden = Vec::new();
    collect_forbidden_keys(value, "source", &mut forbidden);
    if !forbidden.is_empty() {
        return Err(format!("forbidden keys: {forbidden:?}"));
    }
    let mut strings = Vec::new();
    collect_strings(value, &mut strings);
    if strings
        .iter()
        .any(|s| s.starts_with('/') || s.contains("/root/vendor") || s.contains("/tmp/"))
    {
        return Err("absolute, vendor, or physical path leaked".into());
    }
    Ok(())
}

fn validate_contract(value: &Yaml, bakeoff: &Yaml) -> Result<(), String> {
    if string_at(value, &["schema_version"]) != "layout-lattice-parity.v1"
        || string_at(value, &["source_authority"]) != SOURCE
        || string_at(value, &["corpus_authority"]) != BAKEOFF
    {
        return Err("contract identity drift".into());
    }
    for (path, expected) in [
        (&["corpus_binding", "documents"][..], 3),
        (&["corpus_binding", "pages_per_lane"][..], 202),
        (&["corpus_binding", "lanes"][..], 3),
        (&["corpus_binding", "lane_pages"][..], 606),
    ] {
        if at(value, path).as_u64() != Some(expected) {
            return Err(format!("denominator drift at {path:?}"));
        }
    }
    if at(value, &["production_locks", "production_authority"]).as_str() != Some(PRODUCTION)
        || at(value, &["production_locks", "candidate_only"]).as_bool() != Some(true)
        || at(value, &["production_locks", "import_eligible"]).as_bool() != Some(false)
    {
        return Err("production lock drift".into());
    }
    let exit = at(value, &["terminal_failures", "exit_codes"])
        .as_mapping()
        .ok_or("exit map missing")?;
    let keys: BTreeSet<_> = exit.keys().map(|key| key.as_str().unwrap_or("?")).collect();
    if keys != EXIT_CODES.iter().copied().collect() {
        return Err("exit vocabulary drift".into());
    }
    let expected_exit_values = [
        ("0", "success"),
        ("5", "success_with_warning"),
        ("4", "write_failed"),
        ("98", "out_of_memory"),
        ("126", "missing_pdfalto"),
        ("127", "missing_libxml2"),
        ("143", "timeout_or_killed"),
        ("other_nonzero", "unknown_exit_code"),
    ];
    for (code, expected) in expected_exit_values {
        if exit[Yaml::String(code.into())].as_str() != Some(expected) {
            return Err(format!("exit disposition drift for {code}"));
        }
    }
    let failures: BTreeSet<_> = at(value, &["terminal_failures", "vocabulary"])
        .as_sequence()
        .ok_or("failure vocabulary missing")?
        .iter()
        .map(|v| v.as_str().unwrap_or("?"))
        .collect();
    if failures != FAILURES.iter().copied().collect() {
        return Err("failure vocabulary drift".into());
    }
    if at(value, &["lattice", "lexical_visual_crossing"]).as_str() != Some("forbidden") {
        return Err("lexical/visual separation drift".into());
    }
    if at(value, &["lattice", "furniture", "body_order"]).as_str() != Some("excluded") {
        return Err("furniture body-order drift".into());
    }
    if at(value, &["replay", "byte_identical"]).as_str() != Some("required") {
        return Err("replay lock drift".into());
    }
    let manifest = bakeoff["corpus"]
        .as_mapping()
        .ok_or("bakeoff corpus missing")?;
    if manifest[Yaml::String("corpus_n".into())].as_u64() != Some(3)
        || manifest[Yaml::String("page_denominator".into())].as_u64() != Some(202)
    {
        return Err("corpus authority drift".into());
    }
    Ok(())
}

#[test]
fn source_inventory_is_exactly_sha_bound_and_redacted() {
    validate_source(&json()).unwrap();
}

#[test]
fn parity_contract_binds_corpus_denominators_and_closed_vocabularies() {
    validate_contract(&yaml(CONTRACT), &yaml(BAKEOFF)).unwrap();
}

#[test]
fn mutations_fail_closed_for_hash_exit_denominator_lock_and_payload() {
    let mut source = json();
    source["source_files"]["page"]["sha256"] = Json::String("0".repeat(64));
    assert!(validate_source(&source).is_err(), "hash mutation accepted");

    let mut contract = yaml(CONTRACT);
    contract["corpus_binding"]["lane_pages"] = Yaml::Number(605.into());
    assert!(
        validate_contract(&contract, &yaml(BAKEOFF)).is_err(),
        "denominator mutation accepted"
    );

    let mut contract = yaml(CONTRACT);
    contract["production_locks"]["production_authority"] = Yaml::String("pdfalto".into());
    assert!(
        validate_contract(&contract, &yaml(BAKEOFF)).is_err(),
        "production lock mutation accepted"
    );

    let mut contract = yaml(CONTRACT);
    contract["terminal_failures"]["exit_codes"]["5"] = Yaml::String("typed_failure".into());
    assert!(
        validate_contract(&contract, &yaml(BAKEOFF)).is_err(),
        "exit taxonomy mutation accepted"
    );

    let mut source = json();
    source["payload"] = serde_json::json!({"raw_text": "forbidden"});
    assert!(
        validate_source(&source).is_err(),
        "forbidden payload accepted"
    );
}
