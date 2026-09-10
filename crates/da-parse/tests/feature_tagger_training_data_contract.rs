//! DATA contract for the Java GROBID feature/tagger/training inventory (S06/T01).
//! This test imports no da-parse symbols: it validates only redacted, source-bound
//! authorities and deliberately rejects parity claims, payload leaks and drift.

use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str = "artifacts/parse_compat/java_grobid_feature_tagger_training_source.json";
const DATA: &str = "data/parse/feature_tagger_training_parity.yaml";
const JAVA_REVISION: &str = "0.9.0-crf";
const FEATURES: [&str; 13] = [
    "FeaturesVector",
    "FeaturesVectorBlock",
    "FeaturesVectorCluster",
    "FeaturesVectorCrfpp",
    "FeaturesVectorFigure",
    "FeaturesVectorFullText",
    "FeaturesVectorHeader",
    "FeaturesVectorLine",
    "FeaturesVectorName",
    "FeaturesVectorReference",
    "FeaturesVectorSection",
    "FeaturesVectorToken",
    "FeaturesVectorWithContext",
];
const ENGINES: [&str; 4] = ["WAPITI", "CRFPP", "DELFT", "DUMMY"];
const FORBIDDEN: [&str; 15] = [
    "raw_pdf",
    "raw_text",
    "raw_tei",
    "pdf_bytes",
    "tei_xml",
    "page_image",
    "image_bytes",
    "model_payload",
    "weights",
    "credentials",
    "physical_path",
    "vendor_path",
    "executable",
    "hostname",
    "secret",
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
fn y<'a>(v: &'a Yaml, keys: &[&str]) -> &'a Yaml {
    keys.iter().fold(v, |v, k| &v[*k])
}
fn strings(v: &Yaml, out: &mut Vec<String>) {
    match v {
        Yaml::String(s) => out.push(s.clone()),
        Yaml::Sequence(xs) => xs.iter().for_each(|x| strings(x, out)),
        Yaml::Mapping(m) => m.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}
fn json_keys(v: &Json, at: &str, out: &mut Vec<String>) {
    match v {
        Json::Object(m) => {
            for (k, x) in m {
                if FORBIDDEN.contains(&k.as_str()) {
                    out.push(format!("{at}.{k}"));
                }
                json_keys(x, &format!("{at}.{k}"), out);
            }
        }
        Json::Array(xs) => xs
            .iter()
            .enumerate()
            .for_each(|(i, x)| json_keys(x, &format!("{at}[{i}]"), out)),
        _ => {}
    }
}
fn yaml_keys(v: &Yaml, at: &str, out: &mut Vec<String>) {
    match v {
        Yaml::Mapping(m) => {
            for (k, x) in m {
                if k.as_str().is_some_and(|k| FORBIDDEN.contains(&k)) {
                    out.push(format!("{at}.{k:?}"));
                }
                yaml_keys(x, &format!("{at}.{k:?}"), out);
            }
        }
        Yaml::Sequence(xs) => xs
            .iter()
            .enumerate()
            .for_each(|(i, x)| yaml_keys(x, &format!("{at}[{i}]"), out)),
        _ => {}
    }
}
fn sha(path: &Path) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(fs::read(path).expect("artifact readable"))
    )
}

fn validate(source: &Json, data: &Yaml) -> Result<(), String> {
    if source["schema_version"] != "java-grobid-feature-tagger-training-source.v1"
        || source["vendor_revision"] != JAVA_REVISION
        || source["authority"] != "java-grobid-http@0.9.0-crf"
    {
        return Err("source identity drift".into());
    }
    let units = source["feature_source_units"]
        .as_array()
        .ok_or("feature units missing")?;
    if units.len() != 14 {
        return Err("feature source denominator drift".into());
    }
    let features: BTreeSet<_> = units
        .iter()
        .filter(|u| u["kind"] == "features_vector")
        .map(|u| u["name"].as_str().unwrap())
        .collect();
    if features != FEATURES.into_iter().collect() {
        return Err("13 FeaturesVector vocabulary drift".into());
    }
    if units
        .iter()
        .filter(|u| u["kind"] == "feature_factory")
        .count()
        != 1
    {
        return Err("FeatureFactory unit missing or duplicated".into());
    }
    if source["tagger_engines"].as_array().map(|x| x.len()) != Some(4) {
        return Err("engine denominator drift".into());
    }
    let engines: BTreeSet<_> = source["tagger_engines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    if engines != ENGINES.into_iter().collect() {
        return Err("engine vocabulary drift".into());
    }
    if source["corpus"]["documents"].as_array().map(|x| x.len()) != Some(3)
        || source["corpus"]["pages_per_lane"] != 202
        || source["corpus"]["lane_pages"] != 606
        || source["corpus"]["document_lane_outcomes"] != 9
    {
        return Err("corpus denominator drift".into());
    }
    if source["dispositions"].as_array().map(|x| x.len()) != Some(9) {
        return Err("disposition denominator drift".into());
    }

    if y(data, &["schema_version"]).as_str() != Some("feature-tagger-training-parity.v1")
        || y(data, &["source_contract", "vendor_revision"]).as_str() != Some(JAVA_REVISION)
        || y(data, &["source_contract", "source_sha256"]).as_str()
            != Some(sha(&root().join(SOURCE)).as_str())
    {
        return Err("DATA source binding drift".into());
    }
    if y(data, &["inventory", "feature_source_units"]).as_u64() != Some(14)
        || y(data, &["inventory", "features_vector_classes"]).as_u64() != Some(13)
        || y(data, &["inventory", "tagger_engines"]).as_u64() != Some(4)
    {
        return Err("inventory counts drift".into());
    }
    let columns: Vec<_> = y(data, &["print_vector", "ordered_columns"])
        .as_sequence()
        .ok_or("columns missing")?
        .iter()
        .map(|v| v.as_str().unwrap_or("?"))
        .collect();
    if columns
        != [
            "token",
            "line_status",
            "block_status",
            "font",
            "font_size",
            "relative_position",
            "capitalisation",
            "digitisation",
            "punctuation",
            "lexicon_flags",
            "label",
        ]
    {
        return Err("printVector order drift".into());
    }
    if y(data, &["corpus_binding", "documents"]).as_u64() != Some(3)
        || y(data, &["corpus_binding", "pages_per_lane"]).as_u64() != Some(202)
        || y(data, &["corpus_binding", "lane_pages"]).as_u64() != Some(606)
        || y(data, &["corpus_binding", "document_lane_outcomes"]).as_u64() != Some(9)
    {
        return Err("DATA corpus denominator drift".into());
    }
    let modes = y(data, &["training_surfaces", "generator_modes"])
        .as_sequence()
        .ok_or("generator modes missing")?;
    let expected_modes = [
        "segmentation_from_layout",
        "segmentation_from_tei",
        "segmentation_from_reference",
    ];
    if modes.len() != 3
        || modes.iter().zip(expected_modes).any(|(m, expected)| {
            m["mode"].as_str() != Some(expected) || m["mixing"].as_str() != Some("forbidden")
        })
    {
        return Err("generator mode drift".into());
    }
    for key in [
        "inventoried",
        "executable_in_shadow",
        "unsupported_lexicon_backed",
        "source_not_established",
        "forbidden_this_wave",
    ] {
        if !data.get(key).is_some() {
            return Err(format!("missing closed disposition: {key}"));
        }
    }
    if y(data, &["policy_locks", "import_eligible"]).as_bool() != Some(false)
        || y(data, &["policy_locks", "production_route"]).as_str() != Some("forbidden")
        || y(data, &["policy_locks", "auto_promote"]).as_str() != Some("forbidden")
    {
        return Err("production unlock".into());
    }
    if y(data, &["claims", "implementation_parity"]).as_bool() != Some(false) {
        return Err("parity claim not explicitly closed".into());
    }
    let mut vals = Vec::new();
    strings(data, &mut vals);
    if vals
        .iter()
        .any(|s| s.starts_with('/') || s.contains("/root/") || s.contains("/tmp/"))
    {
        return Err("physical path leaked".into());
    }
    let mut keys = Vec::new();
    json_keys(source, "source", &mut keys);
    if !keys.is_empty() {
        return Err(format!("forbidden payload keys: {keys:?}"));
    }
    yaml_keys(data, "data", &mut keys);
    if !keys.is_empty() {
        return Err(format!("forbidden DATA keys: {keys:?}"));
    }
    Ok(())
}

#[test]
fn source_and_data_are_exactly_bound_and_closed() {
    validate(&source(), &data()).expect("valid feature/tagger DATA contract");
}

#[test]
fn adversarial_mutations_fail_closed() {
    let src = source();
    let original = data();
    type Mutation = (&'static str, Box<dyn Fn(&mut Yaml)>);
    let mutations: [Mutation; 8] = [
        (
            "feature denominator",
            Box::new(|d| d["inventory"]["feature_source_units"] = Yaml::Number(13.into())),
        ),
        (
            "column order",
            Box::new(|d| d["print_vector"]["ordered_columns"][0] = Yaml::String("label".into())),
        ),
        (
            "sha binding",
            Box::new(|d| {
                d["source_contract"]["source_sha256"] = Yaml::String("sha256:drift".into())
            }),
        ),
        (
            "generator mixing",
            Box::new(|d| {
                d["training_surfaces"]["generator_modes"][0]["mode"] = Yaml::String("mixed".into())
            }),
        ),
        (
            "production unlock",
            Box::new(|d| d["policy_locks"]["import_eligible"] = Yaml::Bool(!false)),
        ),
        (
            "parity invention",
            Box::new(|d| d["claims"]["implementation_parity"] = Yaml::Bool(true)),
        ),
        (
            "redaction leak",
            Box::new(|d| d["raw_text"] = Yaml::String("leak".into())),
        ),
        (
            "lane denominator",
            Box::new(|d| d["corpus_binding"]["lane_pages"] = Yaml::Number(605.into())),
        ),
    ];
    for (name, mutate) in mutations {
        let mut d = original.clone();
        mutate(&mut d);
        assert!(validate(&src, &d).is_err(), "mutation accepted: {name}");
    }
}
