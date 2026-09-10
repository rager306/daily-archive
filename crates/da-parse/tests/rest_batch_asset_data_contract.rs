//! Independent DATA/source contract for the REST batch and asset inventory.
//!
//! This target intentionally imports no da-parse symbols.  It validates only
//! the redacted Java ledger and the schema-shaped DATA disposition contract.

use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str = "artifacts/parse_compat/java_grobid_rest_batch_asset_source.json";
const DATA: &str = "data/parse/rest_batch_asset_parity.yaml";
const JAVA_REVISION: &str = "0b0ab53b159b32792a01fca1d9d04fd02cf8b24b";
const AUTHORITY: &str = "java-grobid-http@0.9.0-crf";
const INVENTORY_COUNT: usize = 19;
const DEMAND_COUNT: usize = 19;
const EXECUTABLE_COUNT: usize = 4;
const DISPOSITIONS: [&str; 7] = [
    "existing_typed_transport",
    "existing_local_batch_composition",
    "product_required_gap",
    "not_required_by_current_journey",
    "unsupported_without_source",
    "forbidden_this_wave",
    "propagated_terminal",
];
const FAMILIES: [&str; 5] = ["fulltext", "html", "annotation", "training", "batch"];
const FORBIDDEN_KEYS: [&str; 14] = [
    "raw_pdf",
    "pdf_bytes",
    "raw_text",
    "text",
    "tei_xml",
    "raw_tei",
    "images",
    "page_image",
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
fn strings(value: &Yaml, key: &str) -> BTreeSet<String> {
    value[key]
        .as_sequence()
        .expect(key)
        .iter()
        .map(|v| v.as_str().expect("string").to_owned())
        .collect()
}
fn strings_at(value: &Yaml, keys: &[&str]) -> BTreeSet<String> {
    let selected = keys
        .iter()
        .fold(value, |current, key| current.get(*key).expect(key));
    selected
        .as_sequence()
        .expect("DATA sequence")
        .iter()
        .map(|v| v.as_str().expect("DATA string").to_owned())
        .collect()
}
fn forbidden_keys(value: &Json, at: &str, out: &mut Vec<String>) {
    match value {
        Json::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN_KEYS.contains(&key.as_str()) {
                    out.push(format!("{at}.{key}"));
                }
                forbidden_keys(child, &format!("{at}.{key}"), out);
            }
        }
        Json::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                forbidden_keys(child, &format!("{at}[{i}]"), out);
            }
        }
        _ => {}
    }
}
fn all_strings_json(value: &Json, out: &mut Vec<String>) {
    match value {
        Json::String(s) => out.push(s.clone()),
        Json::Array(a) => a.iter().for_each(|v| all_strings_json(v, out)),
        Json::Object(o) => o.values().for_each(|v| all_strings_json(v, out)),
        _ => {}
    }
}
fn all_strings_yaml(value: &Yaml, out: &mut Vec<String>) {
    match value {
        Yaml::String(s) => out.push(s.clone()),
        Yaml::Sequence(a) => a.iter().for_each(|v| all_strings_yaml(v, out)),
        Yaml::Mapping(o) => o.values().for_each(|v| all_strings_yaml(v, out)),
        _ => {}
    }
}

fn validate(source: &Json, data: &Yaml) -> Result<(), String> {
    if source["schema_version"] != "java-grobid-rest-batch-asset-source.v1"
        || source["authority"] != AUTHORITY
        || source["vendor_revision"] != JAVA_REVISION
    {
        return Err("Java identity drift".into());
    }
    let files = source["source_files"]
        .as_object()
        .ok_or("source_files missing")?;
    if files.len() != 3
        || files.values().any(|v| {
            v["logical_source"].as_str().is_none()
                || v["sha256"].as_str().is_none_or(|s| s.len() != 64)
        })
    {
        return Err("source files must be three digest-only logical identities".into());
    }
    if source["evidence_policy"]["raw_java_source_embedded"] != false
        || source["evidence_policy"]["import_eligible"] != false
        || source["evidence_policy"]["graph_write"] != "forbidden"
        || source["evidence_policy"]["auto_promote"] != "forbidden"
        || source["evidence_policy"]["production_route"] != "forbidden"
    {
        return Err("evidence policy unlocked".into());
    }
    let ops = source["operations"]
        .as_array()
        .ok_or("operations missing")?;
    if ops.len() != INVENTORY_COUNT {
        return Err("inventory denominator drift".into());
    }
    let mut ids = BTreeSet::new();
    for op in ops {
        let id = op["operation_id"].as_str().ok_or("operation id missing")?;
        if !ids.insert(id) {
            return Err(format!("duplicate operation {id}"));
        }
        if !FAMILIES.contains(&op["family"].as_str().ok_or("family missing")?) {
            return Err("unknown family".into());
        }
        if !["GET", "POST", "not_established"]
            .contains(&op["method"].as_str().ok_or("method missing")?)
        {
            return Err("method drift".into());
        }
        if op["request_schema"].as_str().is_none()
            || op["result_schema"].as_str().is_none()
            || op["cardinality"].as_str().is_none()
            || op["cache_relevance"].as_str().is_none()
            || op["source_identity"].is_null() && op["source_status"] != "source_not_established"
        {
            return Err(format!("incomplete operation {id}"));
        }
    }
    if data["schema_version"] != "rest-batch-asset-parity.v1"
        || data["source_contract"]["ledger_schema"] != "java-grobid-rest-batch-asset-source.v1"
        || data["source_contract"]["java_authority"] != AUTHORITY
        || data["source_contract"]["java_revision"] != JAVA_REVISION
    {
        return Err("DATA source binding drift".into());
    }
    if data["denominators"]["inventory"] != INVENTORY_COUNT
        || data["denominators"]["demand_review"] != DEMAND_COUNT
        || data["denominators"]["executable"] != EXECUTABLE_COUNT
    {
        return Err("denominator mismatch".into());
    }
    if strings(data, "closed_dispositions") != DISPOSITIONS.into_iter().map(str::to_owned).collect()
    {
        return Err("disposition vocabulary drift".into());
    }
    if data["policy_locks"]["import_eligible"] != false
        || data["policy_locks"]["graph_write"] != "forbidden"
        || data["policy_locks"]["auto_promote"] != "forbidden"
        || data["policy_locks"]["production_route"] != "forbidden"
        || strings_at(data, &["policy_locks", "independently_selectable"])
            != ["header", "references"]
                .into_iter()
                .map(str::to_owned)
                .collect()
    {
        return Err("policy lock drift".into());
    }
    let rows = data["demand_review"]
        .as_sequence()
        .ok_or("demand review missing")?;
    if rows.len() != DEMAND_COUNT {
        return Err("demand denominator drift".into());
    }
    let mut reviewed = BTreeSet::new();
    for row in rows {
        let id = row["operation_id"].as_str().ok_or("review id missing")?;
        if !reviewed.insert(id) || !ids.contains(id) {
            return Err("review rows must be one-to-one with inventory".into());
        }
        let disposition = row["disposition"].as_str().ok_or("disposition missing")?;
        if !DISPOSITIONS.contains(&disposition)
            || row["caller_evidence"].as_str().is_none()
            || row["acceptance_obligation"].as_str().is_none()
            || row["rationale_code"].as_str().is_none()
        {
            return Err(format!("invalid demand row {id}"));
        }
        if disposition == "product_required_gap"
            && (row["caller_evidence"] == "none" || row["acceptance_obligation"] == "none")
        {
            return Err("product gap without independent demand proof".into());
        }
    }
    if reviewed.len() != INVENTORY_COUNT
        || data["product_demand_verdict"] != "no_independently_evidenced_gap"
        || data["implemented_new_endpoints"] != 0
        || data["implementation_parity_claim"] != false
    {
        return Err("demand verdict drift".into());
    }
    let mut values = Vec::new();
    all_strings_json(source, &mut values);
    all_strings_yaml(data, &mut values);
    if values.iter().any(|v| {
        v.contains("http://")
            || v.contains("https://")
            || v.contains("/root/")
            || v.contains("/tmp/")
            || v.contains("/vendor/")
    }) {
        return Err("URL, host, or physical path leakage".into());
    }
    let mut keys = Vec::new();
    forbidden_keys(source, "source", &mut keys);
    if !keys.is_empty() {
        return Err(format!("forbidden key leakage: {keys:?}"));
    }
    Ok(())
}

#[test]
fn source_and_data_contract_is_closed_and_sha_bound() {
    let source = json(SOURCE);
    let data = yaml(DATA);
    let mut canonical = source.clone();
    canonical["ledger_sha256"] = Json::String("TO_BE_FILLED".into());
    let canonical_bytes = serde_json::to_vec(&canonical).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(canonical_bytes)),
        source["ledger_sha256"].as_str().unwrap()
    );
    validate(&source, &data).unwrap();
}

#[test]
fn adversarial_mutations_fail_closed() {
    let source = json(SOURCE);
    let data = yaml(DATA);
    let mut duplicate = source.clone();
    let first_operation = duplicate["operations"][0].clone();
    duplicate["operations"]
        .as_array_mut()
        .unwrap()
        .push(first_operation);
    assert!(validate(&duplicate, &data).is_err());
    let mut route = source.clone();
    route["operations"][0]["method"] = Json::String("PUT".into());
    assert!(validate(&route, &data).is_err());
    let mut invented = data.clone();
    invented["closed_dispositions"]
        .as_sequence_mut()
        .unwrap()
        .push(Yaml::String("invented".into()));
    assert!(validate(&source, &invented).is_err());
    let mut demand = data.clone();
    demand["demand_review"][0]["disposition"] = Yaml::String("product_required_gap".into());
    demand["demand_review"][0]["caller_evidence"] = Yaml::String("none".into());
    assert!(validate(&source, &demand).is_err());
    let mut unlock = data.clone();
    unlock["policy_locks"]["import_eligible"] = Yaml::Bool(!false);
    assert!(validate(&source, &unlock).is_err());
    let mut leak = source.clone();
    leak["source_files"]["paths"]["logical_source"] = Json::String("/vendor/source.java".into());
    assert!(validate(&leak, &data).is_err());
}
