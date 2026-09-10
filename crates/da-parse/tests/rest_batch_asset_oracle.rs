//! Independently reviewed product-demand oracle for the S07 asset inventory.
//!
//! This test is deliberately data-only: it imports no ParseEndpoint,
//! ParseBatchExecutor, cache implementation, CLI manifest parser, or SUT
//! digest helper.  It proves demand dispositions rather than implementation
//! parity or an endpoint score.

use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const LEDGER: &str = "artifacts/parse_compat/java_grobid_rest_batch_asset_source.json";
const ORACLE: &str = "crates/da-parse/tests/fixtures/rest_batch_asset_oracle.yaml";
const SOURCE: &str = "artifacts/parse_compat/rest_batch_asset_oracle_source.json";
const REVISION: &str = "0b0ab53b159b32792a01fca1d9d04fd02cf8b24b";
const AUTHORITY: &str = "java-grobid-http@0.9.0-crf";
const OPS: usize = 19;
const DISPOSITIONS: [&str; 7] = [
    "existing_typed_transport",
    "existing_local_batch_composition",
    "product_required_gap",
    "not_required_by_current_journey",
    "unsupported_without_source",
    "forbidden_this_wave",
    "propagated_terminal",
];
const FORBIDDEN: [&str; 14] = [
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
        .unwrap()
        .to_path_buf()
}
fn json(p: &str) -> Json {
    serde_json::from_str(&fs::read_to_string(root().join(p)).unwrap()).unwrap()
}
fn yaml(p: &str) -> Yaml {
    serde_yaml::from_str(&fs::read_to_string(root().join(p)).unwrap()).unwrap()
}
fn strings(v: &Yaml, key: &str) -> BTreeSet<String> {
    v[key]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}
fn all_strings(v: &Yaml, out: &mut Vec<String>) {
    match v {
        Yaml::String(s) => out.push(s.clone()),
        Yaml::Sequence(xs) => xs.iter().for_each(|x| all_strings(x, out)),
        Yaml::Mapping(m) => m.values().for_each(|x| all_strings(x, out)),
        _ => {}
    }
}
fn all_json_strings(v: &Json, out: &mut Vec<String>) {
    match v {
        Json::String(s) => out.push(s.clone()),
        Json::Array(xs) => xs.iter().for_each(|x| all_json_strings(x, out)),
        Json::Object(m) => m.values().for_each(|x| all_json_strings(x, out)),
        _ => {}
    }
}
fn keys(v: &Json, at: &str, out: &mut Vec<String>) {
    if let Json::Object(m) = v {
        for (k, x) in m {
            let allowed_denominator = at == "source.review_denominators" && k == "executable";
            if FORBIDDEN.contains(&k.as_str()) && !allowed_denominator {
                out.push(format!("{at}.{k}"));
            }
            keys(x, &format!("{at}.{k}"), out);
        }
    } else if let Json::Array(xs) = v {
        for (i, x) in xs.iter().enumerate() {
            keys(x, &format!("{at}[{i}]"), out);
        }
    }
}

fn validate(ledger: &Json, oracle: &Yaml, source: &Json) -> Result<(), String> {
    if ledger["vendor_revision"].as_str() != Some(REVISION)
        || ledger["authority"].as_str() != Some(AUTHORITY)
        || ledger["operations"].as_array().unwrap().len() != OPS
    {
        return Err("ledger identity or denominator drift".into());
    }
    if oracle["schema_version"].as_str() != Some("rest-batch-asset-demand-oracle.v1")
        || oracle["source_contract"]["java_revision"].as_str() != Some(REVISION)
        || oracle["source_contract"]["java_authority"].as_str() != Some(AUTHORITY)
    {
        return Err("oracle source binding drift".into());
    }
    if oracle["denominators"]["inventory"].as_i64() != Some(OPS as i64)
        || oracle["denominators"]["demand_review"].as_i64() != Some(OPS as i64)
        || oracle["denominators"]["executable"].as_i64() != Some(4)
    {
        return Err("oracle denominator drift".into());
    }
    if oracle["denominators"]["corpus_documents"].as_i64() != Some(3)
        || oracle["denominators"]["corpus_pages_per_lane"].as_i64() != Some(202)
        || oracle["denominators"]["corpus_lane_pages"].as_i64() != Some(606)
        || oracle["denominators"]["corpus_document_lane_outcomes"].as_i64() != Some(9)
        || oracle["denominators"]["upstream_terminal_outcomes"].as_i64() != Some(6)
        || oracle["denominators"]["s03_s05_cells"].as_i64() != Some(54)
    {
        return Err("dependency identity drift".into());
    }
    if strings(oracle, "closed_dispositions")
        != DISPOSITIONS.into_iter().map(str::to_owned).collect()
    {
        return Err("closed disposition vocabulary drift".into());
    }
    if oracle["product_demand_verdict"].as_str() != Some("no_independently_evidenced_gap")
        || oracle["implemented_new_endpoints"].as_i64() != Some(0)
        || oracle["interface_decision"]["selected"].as_str()
            != Some("zero_new_endpoint_inventory_only")
    {
        return Err("product verdict or interface decision drift".into());
    }
    if oracle["policy_locks"]["import_eligible"].as_bool() != Some(false)
        || oracle["policy_locks"]["graph_write"].as_str() != Some("forbidden")
        || oracle["policy_locks"]["auto_promote"].as_str() != Some("forbidden")
        || oracle["policy_locks"]["production_route"].as_str() != Some("forbidden")
        || oracle["policy_locks"]["generic_arbitrary_route"].as_str() != Some("forbidden")
        || oracle["policy_locks"]["independently_selectable"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
            != vec!["header", "references"]
    {
        return Err("policy unlock".into());
    }
    let ops = ledger["operations"].as_array().unwrap();
    let rows = oracle["review_rows"]
        .as_sequence()
        .ok_or("review rows missing")?;
    if rows.len() != OPS {
        return Err("review denominator drift".into());
    }
    let mut by_id = std::collections::BTreeMap::new();
    let mut reviewed = BTreeSet::new();
    for op in ops {
        by_id.insert(
            op["operation_id"].as_str().ok_or("operation id missing")?,
            op,
        );
    }
    for row in rows {
        let id = row["operation_id"].as_str().ok_or("review id missing")?;
        if !reviewed.insert(id.to_owned()) {
            return Err(format!("duplicate review row: {id}"));
        }
        let op = by_id
            .get(id)
            .ok_or_else(|| format!("unknown review operation: {id}"))?;
        for field in [
            "caller_class",
            "current_journey_evidence",
            "request_identity",
            "result_identity",
            "disposition",
            "rationale_code",
        ] {
            if row[field].as_str().is_none() {
                return Err(format!("missing review field {field}"));
            }
        }
        let caller = row["caller_class"].as_str().unwrap();
        if ![
            "da-application.parse_pdf",
            "da-cli.parse_batch",
            "none",
            "independent-review",
        ]
        .contains(&caller)
        {
            return Err(format!("invented caller: {caller}"));
        }
        let local_batch = id == "rest.batchProcessFulltext";
        if !local_batch
            && (row["request_identity"].as_str() != op["request_schema"].as_str()
                || row["result_identity"].as_str() != op["result_schema"].as_str())
        {
            return Err(format!("request/result identity drift: {id}"));
        }
        if local_batch
            && (row["request_identity"].as_str() != Some("batch.v1")
                || row["result_identity"].as_str() != Some("batch.result.v1"))
        {
            return Err(format!("local batch identity drift: {id}"));
        }
        if row["static_method"].as_str() != op["method"].as_str()
            && !(id == "rest.batchProcessFulltext"
                && row["static_method"].as_str() == Some("composition"))
        {
            return Err(format!("route method substitution: {id}"));
        }
        if row["static_route"].as_str() != op["route"].as_str()
            && !(id == "rest.batchProcessFulltext"
                && row["static_route"].as_str() == Some("local-parse-batch"))
        {
            return Err(format!("route substitution: {id}"));
        }
        let d = row["disposition"].as_str().unwrap();
        if !DISPOSITIONS.contains(&d) {
            return Err("invented disposition".into());
        }
        if d == "product_required_gap"
            && (caller == "none" || row["acceptance_obligation"].as_str() == Some("none"))
        {
            return Err("product gap lacks caller or acceptance proof".into());
        }
    }
    if reviewed.len() != OPS {
        return Err("review rows are not one-to-one".into());
    }
    let mut vals = Vec::new();
    all_strings(oracle, &mut vals);
    all_json_strings(source, &mut vals);
    if vals.iter().any(|s| {
        s.contains("http://")
            || s.contains("https://")
            || s.contains("/root/")
            || s.contains("/tmp/")
            || s.contains("/vendor/")
    }) {
        return Err("raw URL or physical path leakage".into());
    }
    let mut found = Vec::new();
    keys(source, "source", &mut found);
    if !found.is_empty() {
        return Err(format!("forbidden keys: {found:?}"));
    }
    Ok(())
}

#[test]
fn independently_reviewed_oracle_is_reproducible() {
    let ledger = json(LEDGER);
    let oracle = yaml(ORACLE);
    let source = json(SOURCE);
    let bytes = fs::read(root().join(ORACLE)).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        source["fixture_sha256"].as_str().unwrap()
    );
    assert_eq!(
        source["oracle_schema"].as_str(),
        oracle["schema_version"].as_str()
    );
    validate(&ledger, &oracle, &source).unwrap();
}

#[test]
fn adversarial_oracle_mutations_fail_closed() {
    let ledger = json(LEDGER);
    let oracle = yaml(ORACLE);
    let source = json(SOURCE);
    let mut missing = oracle.clone();
    missing["review_rows"].as_sequence_mut().unwrap().pop();
    assert!(validate(&ledger, &missing, &source).is_err());
    let mut duplicate = oracle.clone();
    let first = duplicate["review_rows"][0].clone();
    duplicate["review_rows"]
        .as_sequence_mut()
        .unwrap()
        .push(first);
    assert!(validate(&ledger, &duplicate, &source).is_err());
    let mut caller = oracle.clone();
    caller["review_rows"][1]["caller_class"] = Yaml::String("invented.caller".into());
    caller["review_rows"][1]["disposition"] = Yaml::String("product_required_gap".into());
    caller["review_rows"][1]["acceptance_obligation"] = Yaml::String("acceptance".into());
    assert!(validate(&ledger, &caller, &source).is_err());
    let mut pooled = oracle.clone();
    pooled["review_rows"][1]["operation_id"] = pooled["review_rows"][0]["operation_id"].clone();
    assert!(validate(&ledger, &pooled, &source).is_err());
    let mut score = oracle.clone();
    score["implemented_new_endpoints"] = Yaml::Number(1.into());
    assert!(validate(&ledger, &score, &source).is_err());
    let mut leak = oracle.clone();
    leak["review_rows"][0]["rationale_code"] = Yaml::String("/tmp/body".into());
    assert!(validate(&ledger, &leak, &source).is_err());
}
