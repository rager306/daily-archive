//! S06/T01 DATA contract for the four native visual/funding families.
//!
//! The fixture is parsed as generic YAML, so this gate stays independent of the
//! family runners that T02–T06 will add: it binds the transcription in
//! `data/parse/native_visual_families.yaml` to the frozen GROBID 0.9.0 model
//! declarations (checkpoint id, `#rdr#` referenced-column window, label block)
//! and to the shadow locks, and it proves the mutation surface is fail-closed.
//!
//! The expected label blocks are transcribed here *by hand from the frozen model
//! headers* (verbatim model order, both `I-` and bare spellings), not derived
//! from the DATA file being checked — a table that read its own expectation out
//! of the file under test would prove nothing.

use serde_yaml::{Mapping, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

const AUTHORITY: &str = "data/parse/native_visual_families.yaml";
const SCHEMA: &str = "native-visual-families.v1";
const RIGHTS_REF: &str = "data/parse/grobid_model_rights.yaml";
const MODEL_ROOT_ENV: &str = "DA_GROBID_MODEL_ROOT";
const FAMILIES: [&str; 4] = ["figure", "table", "formula", "funding"];
const NOT_IN_MODEL_KEY: &str = "java_declared_not_in_model";

/// Shape-only gate for the transcribed DATA file: it declares facts, it never
/// carries model material (D358).
const FORBIDDEN_MODEL_MATERIAL: [&str; 2] = ["#qrk#", "%x["];
const MAX_DATA_BYTES: usize = 32 * 1024;

const FIGURE_CHECKPOINT: &str = "#mdl#2#521";
const FIGURE_RDR: &str = "#rdr#85/25/0";
const FIGURE_LABELS: [&str; 7] = [
    "I-<figure_head>",
    "I-<label>",
    "I-<figDesc>",
    "<figDesc>",
    "I-<content>",
    "<content>",
    "<figure_head>",
];
const FIGURE_BASES: [&str; 4] = ["<figure_head>", "<label>", "<figDesc>", "<content>"];

const TABLE_CHECKPOINT: &str = "#mdl#2#1391";
const TABLE_LABELS: [&str; 10] = [
    "I-<other>",
    "I-<figure_head>",
    "I-<label>",
    "I-<figDesc>",
    "<figDesc>",
    "I-<content>",
    "<content>",
    "I-<note>",
    "<note>",
    "<figure_head>",
];
const TABLE_BASES: [&str; 6] = [
    "<other>",
    "<figure_head>",
    "<label>",
    "<figDesc>",
    "<content>",
    "<note>",
];

const FUNDING_CHECKPOINT: &str = "#mdl#2#15784";
const FUNDING_RDR: &str = "#rdr#100/16/0";
const FUNDING_LABELS: [&str; 20] = [
    "I-<other>",
    "<other>",
    "I-<funderName>",
    "<funderName>",
    "I-<grantNumber>",
    "<grantNumber>",
    "I-<person>",
    "<person>",
    "I-<programName>",
    "<programName>",
    "I-<grantName>",
    "<grantName>",
    "I-<projectName>",
    "<projectName>",
    "I-<affiliation>",
    "<affiliation>",
    "I-<institution>",
    "<institution>",
    "I-<infrastructure>",
    "<infrastructure>",
];
const FUNDING_BASES: [&str; 10] = [
    "<other>",
    "<funderName>",
    "<grantNumber>",
    "<person>",
    "<programName>",
    "<grantName>",
    "<projectName>",
    "<affiliation>",
    "<institution>",
    "<infrastructure>",
];
const FUNDING_ONLY_IN_JAVA: [&str; 4] =
    ["<funderAbbrv>", "<programAbbrv>", "<projectAbbrv>", "<url>"];
const FUNDING_GAZETTEER_LANES: [&str; 2] = ["knownFunder", "knownInfrastructure"];
const FUNDING_GAZETTEER_SOURCES: [&str; 2] = [
    "lexicon/organisations/funders.txt",
    "lexicon/organisations/research_infrastructures.txt",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn yaml(path: &str) -> Value {
    serde_yaml::from_str(&std::fs::read_to_string(root().join(path)).expect(path)).expect(path)
}

fn raw(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).expect(path)
}

fn as_owned(path: &[&str]) -> Vec<String> {
    path.iter().map(|key| (*key).to_string()).collect()
}

fn get<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(key.as_str())?;
    }
    Some(current)
}

fn text_at(value: &Value, path: &[String]) -> Option<String> {
    get(value, path)?.as_str().map(str::to_owned)
}

fn strings_at(value: &Value, path: &[String]) -> Vec<String> {
    get(value, path)
        .and_then(Value::as_sequence)
        .map(|sequence| {
            sequence
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn number_at(value: &Value, path: &[String]) -> Option<u64> {
    get(value, path)?.as_u64()
}

fn text(value: &Value, path: &[&str]) -> Option<String> {
    text_at(value, &as_owned(path))
}

fn strings(value: &Value, path: &[&str]) -> Vec<String> {
    strings_at(value, &as_owned(path))
}

fn number(value: &Value, path: &[&str]) -> Option<u64> {
    number_at(value, &as_owned(path))
}

fn family_path(family: &str, path: &[&str]) -> Vec<String> {
    let mut full = vec!["families".to_string(), family.to_string()];
    full.extend_from_slice(&as_owned(path));
    full
}

fn at_mut<'a>(value: &'a mut Value, path: &[&str]) -> &'a mut Value {
    let mut current = value;
    for key in path {
        current = current
            .get_mut(*key)
            .unwrap_or_else(|| panic!("missing DATA key {key}"));
    }
    current
}

fn with(value: &Value, path: &[&str], replacement: Value) -> Value {
    let mut mutated = value.clone();
    *at_mut(&mut mutated, path) = replacement;
    mutated
}

fn with_sequence(value: &Value, path: &[&str], mutate: impl FnOnce(&mut Vec<Value>)) -> Value {
    let mut mutated = value.clone();
    let sequence = at_mut(&mut mutated, path)
        .as_sequence_mut()
        .expect("sequence");
    mutate(sequence);
    mutated
}

fn with_family(value: &Value, family: &str, replacement: Value) -> Value {
    with(value, &["families", family], replacement)
}

fn rejects(value: &Value, marker: &str) {
    let errors = validate(value);
    assert!(
        errors.iter().any(|error| error.contains(marker)),
        "mutation `{marker}` was accepted: {errors:?}"
    );
}

/// The base name of one spelling (the `I-` prefix is a spelling, not a name).
fn base_of(label: &str) -> &str {
    label.strip_prefix("I-").unwrap_or(label)
}

/// `#rdr#<patterns>/<max column index>/<token count>` — the position the model
/// header declares.
fn rdr_max_column_index(declared: &str) -> Option<usize> {
    declared
        .strip_prefix("#rdr#")?
        .split('/')
        .nth(1)?
        .parse()
        .ok()
}

/// A label spelling is `<name>` or `I-<name>`: the closed wire shape the family
/// dictionaries carry.
fn is_label_shape(label: &str) -> bool {
    let bare = base_of(label);
    bare.len() > 2 && bare.starts_with('<') && bare.ends_with('>')
}

fn validate(value: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    if text(value, &["schema_version"]).as_deref() != Some(SCHEMA) {
        errors.push("[schema] native visual families schema drift".into());
    }
    for key in [
        "schema_version",
        "contract_id",
        "adr_ref",
        "decisions",
        "rights_ref",
        "bundled_model_bytes",
        "model_root_env",
        "observation_mode",
        "promotion_state",
        "production_route",
        "graph_write",
        "import_eligible",
        "auto_promote",
        "deployed",
        "decision_point",
        "independently_selectable",
        "grouping",
        "caption_link_lane",
        "families",
    ] {
        if value.get(key).is_none() {
            errors.push(format!("[missing-key] native visual families lost `{key}`"));
        }
    }

    // Shadow locks: the band observes, it never routes, writes or promotes.
    for (key, expected) in [
        ("observation_mode", "shadow"),
        ("promotion_state", "candidate"),
        ("production_route", "forbidden"),
        ("graph_write", "forbidden"),
        ("auto_promote", "forbidden"),
    ] {
        if text(value, &[key]).as_deref() != Some(expected) {
            errors.push(format!("[lock] `{key}` must be `{expected}`"));
        }
    }
    if value.get("import_eligible").and_then(Value::as_bool) != Some(false) {
        errors.push("[lock] `import_eligible` must stay false (D127)".into());
    }
    if value.get("bundled_model_bytes").and_then(Value::as_bool) != Some(false) {
        errors.push("[lock] model bytes must never be bundled (D358)".into());
    }
    if value.get("deployed").and_then(Value::as_bool) != Some(false) {
        errors.push("[lock] the band must not be deployed".into());
    }
    if number(value, &["decision_point"]) != Some(6) {
        errors.push("[lock] the band must stay on DecisionPoint 6".into());
    }
    if strings(value, &["independently_selectable"]) != ["header", "references"] {
        errors.push("[lock] `independently_selectable` must be exactly header+references".into());
    }
    if text(value, &["model_root_env"]).as_deref() != Some(MODEL_ROOT_ENV) {
        errors.push("[lock] `model_root_env` is the audited env var and has no default".into());
    }
    if text(value, &["rights_ref"]).as_deref() != Some(RIGHTS_REF) {
        errors.push("[rights] the rights review path is pinned".into());
    }
    if !root().join(RIGHTS_REF).is_file() {
        errors.push("[rights] the rights review file is missing".into());
    }

    // The grouping rule is DATA: the clusteror rule, and its explicit divergence
    // from the D359 base-name rule.
    if text(value, &["grouping", "rule"]).as_deref() != Some("tagging_token_clusteror_is_beginning")
    {
        errors.push("[grouping] the clusteror rule must be declared".into());
    }
    if text(value, &["grouping", "name_prefix"]).as_deref() != Some("I-") {
        errors.push("[grouping] the cluster-opening prefix is `I-`".into());
    }
    if text(value, &["grouping", "diverges_from"]).as_deref() != Some("base_label_runs") {
        errors.push("[grouping] the D359 divergence must stay declared".into());
    }

    // The caption-link lane: caller-supplied inventory, S04-owned resolution, and
    // `NotObserved` for an absent inventory (never a zero count, H12-N1).
    for (key, expected) in [
        ("marker_dialect", "citation_marker"),
        ("inventory_source", "caller"),
        ("resolve_owner", "native_callout_matching"),
        ("empty_inventory", "not_observed"),
    ] {
        if text(value, &["caption_link_lane", key]).as_deref() != Some(expected) {
            errors.push(format!("[caption-lane] `{key}` must be `{expected}`"));
        }
    }

    // The family set is closed: four families, no more and no fewer.
    let declared_families: Vec<String> = value
        .get("families")
        .and_then(Value::as_mapping)
        .map(|mapping| {
            mapping
                .keys()
                .map(|key| key.as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default();
    let expected_families: BTreeSet<&str> = FAMILIES.into_iter().collect();
    let declared_set: BTreeSet<&str> = declared_families.iter().map(String::as_str).collect();
    if declared_set != expected_families {
        errors.push(format!(
            "[family-set] families must be exactly {FAMILIES:?}, found {declared_families:?}"
        ));
    }

    errors.extend(validate_crf_family(
        value,
        "figure",
        FIGURE_CHECKPOINT,
        FIGURE_RDR,
        26,
        27,
        &FIGURE_LABELS,
        &FIGURE_BASES,
        &["<other>"],
    ));
    errors.extend(validate_crf_family(
        value,
        "table",
        TABLE_CHECKPOINT,
        FIGURE_RDR,
        26,
        27,
        &TABLE_LABELS,
        &TABLE_BASES,
        &[],
    ));
    errors.extend(validate_formula(value));
    errors.extend(validate_funding(value));

    errors
}

#[allow(clippy::too_many_arguments)]
fn validate_crf_family(
    value: &Value,
    family: &str,
    checkpoint: &str,
    rdr: &str,
    referenced_columns: usize,
    emitted_columns: usize,
    labels: &[&str],
    bases: &[&str],
    java_only: &[&str],
) -> Vec<String> {
    let mut errors = Vec::new();
    let present = value
        .get("families")
        .and_then(|families| families.get(family))
        .is_some_and(|family| !family.is_null());
    if !present {
        errors.push(format!("[family-set] `{family}` is missing"));
        return errors;
    }

    if text_at(value, &family_path(family, &["kind"])).as_deref() != Some("crf_sequence") {
        errors.push(format!("[family:{family}] kind must be crf_sequence"));
    }
    if text_at(value, &family_path(family, &["model_id"])).as_deref() != Some(family) {
        errors.push(format!(
            "[family:{family}] model_id must name the frozen model"
        ));
    }
    if text_at(value, &family_path(family, &["model_file"])).as_deref()
        != Some(&format!("{family}/model.wapiti"))
    {
        errors.push(format!(
            "[family:{family}] model_file must be the frozen path"
        ));
    }
    if text_at(value, &family_path(family, &["declared_checkpoint"])).as_deref() != Some(checkpoint)
    {
        errors.push(format!("[family:{family}] declared checkpoint drift"));
    }

    // The declared `#rdr#` window must agree with the transcribed widths: the
    // header's second field is the highest referenced column index.
    let declared_rdr = text_at(value, &family_path(family, &["declared_rdr"])).unwrap_or_default();
    if declared_rdr != rdr {
        errors.push(format!("[family:{family}] declared `#rdr#` drift"));
    }
    if rdr_max_column_index(&declared_rdr) != Some(referenced_columns - 1) {
        errors.push(format!(
            "[family:{family}] `#rdr#` max column index disagrees with referenced_columns"
        ));
    }
    if number_at(value, &family_path(family, &["referenced_columns"]))
        != Some(referenced_columns as u64)
    {
        errors.push(format!("[widths:{family}] referenced_columns drift"));
    }
    if number_at(value, &family_path(family, &["emitted_columns"])) != Some(emitted_columns as u64)
    {
        errors.push(format!("[widths:{family}] emitted_columns drift"));
    }
    let excluded: Vec<u64> = get(value, &family_path(family, &["template_excludes"]))
        .and_then(Value::as_sequence)
        .map(|sequence| sequence.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    if excluded != [emitted_columns as u64 - 1] {
        errors.push(format!(
            "[widths:{family}] the fulltext ABI column above the model window must be excluded"
        ));
    }

    let declared_labels = strings_at(value, &family_path(family, &["declared_labels"]));
    let declared_bases = strings_at(value, &family_path(family, &["base_labels"]));
    let expected_labels: Vec<String> = labels.iter().map(|label| (*label).to_string()).collect();
    let expected_bases: Vec<String> = bases.iter().map(|base| (*base).to_string()).collect();
    if declared_labels != expected_labels {
        errors.push(format!("[labels:{family}] the declared label block drift"));
    }
    if declared_bases != expected_bases {
        errors.push(format!("[labels:{family}] base_labels drift"));
    }
    if number_at(value, &family_path(family, &["declared_label_count"]))
        != Some(declared_labels.len() as u64)
    {
        errors.push(format!("[count:{family}] declared_label_count drift"));
    }
    if declared_labels.is_empty() {
        errors.push(format!(
            "[labels:{family}] the dictionary must not be empty"
        ));
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for label in &declared_labels {
        if !is_label_shape(label) {
            errors.push(format!(
                "[labels:{family}] `{label}` is not a label spelling"
            ));
        }
        if !seen.insert(label.as_str()) {
            errors.push(format!("[labels:{family}] `{label}` is duplicated"));
        }
        if !declared_bases.iter().any(|base| base == base_of(label)) {
            errors.push(format!(
                "[labels:{family}] `{label}` has no base in base_labels"
            ));
        }
    }
    // Every base must be reachable from at least one spelling, and the base order
    // must be the first-appearance order of the declared block.
    let mut first_appearance: Vec<String> = Vec::new();
    for label in &declared_labels {
        let base = base_of(label).to_string();
        if !first_appearance.contains(&base) {
            first_appearance.push(base);
        }
    }
    if declared_bases != first_appearance {
        errors.push(format!(
            "[labels:{family}] base_labels is not the first-appearance order"
        ));
    }

    let java_only: Vec<String> = java_only.iter().map(|label| (*label).to_string()).collect();
    if strings_at(value, &family_path(family, &[NOT_IN_MODEL_KEY])) != java_only {
        errors.push(format!(
            "[java-label:{family}] java_declared_not_in_model drift"
        ));
    }
    for label in &java_only {
        if declared_labels.contains(label) {
            errors.push(format!(
                "[java-label:{family}] `{label}` is declared absent from the model but the block carries it"
            ));
        }
    }
    errors
}

fn validate_formula(value: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if !value
        .get("families")
        .and_then(|families| families.get("formula"))
        .is_some_and(|formula| formula.get("model_id").is_some_and(Value::is_null))
    {
        errors.push("[formula] GROBID 0.9.0 ships no formula model: model_id must be null".into());
    }
    if text(value, &["families", "formula", "kind"]).as_deref() != Some("locator_join") {
        errors.push("[formula] the family is a locator join, not a decoder".into());
    }
    if text(value, &["families", "formula", "locator"]).as_deref() != Some("fulltext") {
        errors.push("[formula] equations are located in the fulltext label sequence".into());
    }
    if strings(value, &["families", "formula", "locator_labels"])
        != ["<equation>", "<equation_label>", "<equation_marker>"]
    {
        errors.push("[formula] the fulltext locator labels drift".into());
    }
    if strings(value, &["families", "formula", "join_labels"]) != ["<equation>", "<equation_label>"]
    {
        errors.push("[formula] the joined labels drift".into());
    }
    errors
}

fn validate_funding(value: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let present = value
        .get("families")
        .and_then(|families| families.get("funding"))
        .is_some_and(|funding| !funding.is_null());
    if !present {
        errors.push("[family-set] `funding` is missing".into());
        return errors;
    }
    if text(value, &["families", "funding", "model_id"]).as_deref()
        != Some("funding-acknowledgement")
    {
        errors.push("[family:funding] model_id drift".into());
    }
    if text(value, &["families", "funding", "declared_checkpoint"]).as_deref()
        != Some(FUNDING_CHECKPOINT)
    {
        errors.push("[family:funding] declared checkpoint drift".into());
    }
    let declared_rdr = text(value, &["families", "funding", "declared_rdr"]).unwrap_or_default();
    if declared_rdr != FUNDING_RDR {
        errors.push("[family:funding] declared `#rdr#` drift".into());
    }
    if rdr_max_column_index(&declared_rdr) != Some(16) {
        errors.push("[family:funding] `#rdr#` max column index must be 16".into());
    }
    if number(value, &["families", "funding", "referenced_columns"]) != Some(17) {
        errors.push("[funding-abi] the model references the 17 featured columns".into());
    }
    if number(value, &["families", "funding", "emitted_columns"]) != Some(17) {
        errors.push("[funding-abi] emitted_columns must be the 17 featured columns".into());
    }
    if number(value, &["families", "funding", "label_column_index"]) != Some(17) {
        errors.push("[funding-abi] the dummy label slot sits at index 17".into());
    }
    if text(value, &["families", "funding", "label_column_value"]).as_deref() != Some("0") {
        errors.push("[funding-abi] the inference label literal must be `0`".into());
    }
    let labels = strings(value, &["families", "funding", "declared_labels"]);
    let expected: Vec<String> = FUNDING_LABELS
        .iter()
        .map(|label| (*label).to_string())
        .collect();
    if labels != expected {
        errors.push("[labels:funding] the declared label block drift".into());
    }
    if strings(value, &["families", "funding", "base_labels"])
        != FUNDING_BASES
            .iter()
            .map(|base| (*base).to_string())
            .collect::<Vec<_>>()
    {
        errors.push("[labels:funding] base_labels drift".into());
    }
    if number(value, &["families", "funding", "declared_label_count"]) != Some(labels.len() as u64)
    {
        errors.push("[count:funding] declared_label_count drift".into());
    }
    if strings(value, &["families", "funding", NOT_IN_MODEL_KEY]) != FUNDING_ONLY_IN_JAVA {
        errors.push("[java-label:funding] the unmodelled Java labels drift".into());
    }
    if strings(value, &["families", "funding", "gazetteer_lanes"]) != FUNDING_GAZETTEER_LANES {
        errors.push("[funding-gazetteer] the two caller-supplied lanes drift".into());
    }
    if strings(value, &["families", "funding", "gazetteer_sources"]) != FUNDING_GAZETTEER_SOURCES {
        errors.push("[funding-gazetteer] the non-bundled lexicon paths drift".into());
    }
    errors
}

/// D358 beside this file: the transcription must not smuggle model material.
fn raw_material_errors(text: &str) -> Vec<String> {
    let mut errors = Vec::new();
    for marker in FORBIDDEN_MODEL_MATERIAL {
        if text.contains(marker) {
            errors.push(format!(
                "[raw-model-material] the DATA file carries `{marker}` (D358 forbids model bytes)"
            ));
        }
    }
    if text.len() > MAX_DATA_BYTES {
        errors.push(format!(
            "[raw-model-material] the DATA file is {} bytes; a declaration-only file stays under {MAX_DATA_BYTES}",
            text.len()
        ));
    }
    errors
}

#[test]
fn native_visual_families_data_is_closed_and_consistent() {
    let authority = yaml(AUTHORITY);
    let errors = validate(&authority);
    assert!(errors.is_empty(), "DATA contract errors: {errors:?}");
}

#[test]
fn native_visual_families_never_carry_model_material() {
    let text = raw(AUTHORITY);
    let errors = raw_material_errors(&text);

    // The transcriptions are declarations only: the frozen headers appear as
    // strings, never as a template or a weight table.
    assert!(
        text.contains(FIGURE_RDR) && text.contains(FUNDING_RDR),
        "the declared headers must be transcribed"
    );
    assert!(errors.is_empty(), "raw model material: {errors:?}");

    // A pasted weight table would be caught even if the file stayed small.
    let smuggled = format!("{text}\n#qrk#2\n1:A,\n");
    assert!(
        raw_material_errors(&smuggled)
            .iter()
            .any(|error| error.contains("[raw-model-material]")),
        "a pasted weight table must be refused"
    );
}

#[test]
fn native_visual_families_mutations_fail_closed() {
    let base = yaml(AUTHORITY);
    assert!(validate(&base).is_empty(), "the unmutated DATA must pass");

    // The shadow locks, the lane owners and the transcribed widths must not
    // erode silently. Each row names the exact diagnostic that must appear.
    let mutations: Vec<(Vec<&str>, Value, &str)> = vec![
        (
            vec!["import_eligible"],
            Value::Bool(true),
            "[lock] `import_eligible`",
        ),
        (
            vec!["graph_write"],
            Value::String("allowed".into()),
            "[lock] `graph_write`",
        ),
        (
            vec!["production_route"],
            Value::String("allowed".into()),
            "[lock] `production_route`",
        ),
        (
            vec!["auto_promote"],
            Value::String("allowed".into()),
            "[lock] `auto_promote`",
        ),
        (vec!["deployed"], Value::Bool(true), "[lock] the band"),
        (
            vec!["decision_point"],
            Value::from(7u64),
            "[lock] the band must stay on DecisionPoint 6",
        ),
        (
            vec!["independently_selectable"],
            Value::Sequence(vec![Value::String("header".into())]),
            "[lock] `independently_selectable`",
        ),
        (
            vec!["bundled_model_bytes"],
            Value::Bool(true),
            "[lock] model bytes",
        ),
        (
            vec!["model_root_env"],
            Value::String("/opt/grobid-home/models".into()),
            "[lock] `model_root_env`",
        ),
        (
            vec!["rights_ref"],
            Value::String("data/parse/SCHEMA.yaml".into()),
            "[rights]",
        ),
        (
            vec!["schema_version"],
            Value::String("native-visual-families.v2".into()),
            "[schema]",
        ),
        (
            vec!["caption_link_lane", "resolve_owner"],
            Value::String("native_caption_resolver".into()),
            "[caption-lane] `resolve_owner`",
        ),
        (
            vec!["caption_link_lane", "empty_inventory"],
            Value::String("zero".into()),
            "[caption-lane] `empty_inventory`",
        ),
        (
            vec!["caption_link_lane", "inventory_source"],
            Value::String("bundled_lexicon".into()),
            "[caption-lane] `inventory_source`",
        ),
        (
            vec!["grouping", "rule"],
            Value::String("base_label_runs".into()),
            "[grouping]",
        ),
        (vec!["grouping", "diverges_from"], Value::Null, "[grouping]"),
        (
            vec!["grouping", "name_prefix"],
            Value::String("B-".into()),
            "[grouping]",
        ),
        (
            vec!["families", "formula", "model_id"],
            Value::String("formula".into()),
            "[formula]",
        ),
        (
            vec!["families", "formula", "kind"],
            Value::String("crf_sequence".into()),
            "[formula]",
        ),
        (
            vec!["families", "formula", "locator"],
            Value::String("header".into()),
            "[formula]",
        ),
        (
            vec!["families", "funding", "label_column_value"],
            Value::String("1".into()),
            "[funding-abi]",
        ),
        (
            vec!["families", "funding", "label_column_index"],
            Value::from(16u64),
            "[funding-abi]",
        ),
        (
            vec!["families", "funding", "referenced_columns"],
            Value::from(18u64),
            "[funding-abi]",
        ),
        (
            vec!["families", "funding", "declared_checkpoint"],
            Value::String("#mdl#2#15785".into()),
            "[family:funding] declared checkpoint",
        ),
        (
            vec!["families", "funding", "declared_label_count"],
            Value::from(19u64),
            "[count:funding]",
        ),
        (
            vec!["families", "figure", "emitted_columns"],
            Value::from(26u64),
            "[widths:figure]",
        ),
        (
            vec!["families", "figure", "referenced_columns"],
            Value::from(27u64),
            "[widths:figure]",
        ),
        (
            vec!["families", "figure", "template_excludes"],
            Value::Sequence(vec![]),
            "[widths:figure]",
        ),
        (
            vec!["families", "figure", "declared_rdr"],
            Value::String("#rdr#85/26/0".into()),
            "[family:figure] `#rdr#` max column index",
        ),
        (
            vec!["families", "table", "declared_rdr"],
            Value::String("#rdr#85/24/0".into()),
            "[family:table] declared `#rdr#` drift",
        ),
        (
            vec!["families", "table", "model_file"],
            Value::String("table/model.wapiti.bak".into()),
            "[family:table] model_file",
        ),
        (
            vec!["families", "table", "model_id"],
            Value::String("figure".into()),
            "[family:table] model_id",
        ),
        (
            vec!["families", "figure", NOT_IN_MODEL_KEY],
            Value::Sequence(vec![]),
            "[java-label:figure]",
        ),
        (
            vec!["families", "funding", "gazetteer_lanes"],
            Value::Sequence(vec![Value::String("knownFunder".into())]),
            "[funding-gazetteer]",
        ),
        (
            vec!["families", "funding", "gazetteer_sources"],
            Value::Sequence(vec![Value::String(
                "lexicon/organisations/funders.txt".into(),
            )]),
            "[funding-gazetteer]",
        ),
    ];
    for (path, replacement, marker) in mutations {
        rejects(&with(&base, &path, replacement), marker);
    }

    // An emptied, trimmed or padded dictionary must be refused, not tolerated.
    rejects(
        &with_sequence(
            &base,
            &["families", "figure", "declared_labels"],
            |labels| {
                labels.clear();
            },
        ),
        "[labels:figure]",
    );
    rejects(
        &with_sequence(
            &base,
            &["families", "figure", "declared_labels"],
            |labels| {
                labels.retain(|label| label.as_str() != Some("I-<content>"));
            },
        ),
        "[count:figure]",
    );
    rejects(
        &with_sequence(
            &base,
            &["families", "figure", "declared_labels"],
            |labels| {
                labels.push(Value::String("<content>".into()));
            },
        ),
        "[labels:figure] `",
    );
    rejects(
        &with_sequence(&base, &["families", "table", "declared_labels"], |labels| {
            labels.retain(|label| label.as_str() != Some("I-<note>"));
        }),
        "[labels:table] the declared label block drift",
    );
    rejects(
        &with_sequence(
            &base,
            &["families", "funding", "declared_labels"],
            |labels| {
                labels.push(Value::String("<funderAbbrv>".into()));
            },
        ),
        "[count:funding]",
    );
    rejects(
        &with_sequence(&base, &["families", "figure", "base_labels"], |bases| {
            bases.pop();
        }),
        "[labels:figure] base_labels",
    );

    // A whole family may not vanish, be blanked, or appear beside the four.
    rejects(
        &with_family(&base, "figure", Value::Null),
        "[family-set] `figure` is missing",
    );
    rejects(
        &with_family(&base, "table", Value::Null),
        "[family-set] `table` is missing",
    );
    rejects(
        &with_family(&base, "funding", Value::Null),
        "[family-set] `funding` is missing",
    );
    for family in FAMILIES {
        rejects(
            &with_family(&base, family, Value::Mapping(Mapping::new())),
            if family == "formula" {
                "[formula]"
            } else {
                "[family:"
            },
        );
    }
    let mut extra = base.clone();
    extra
        .get_mut("families")
        .and_then(Value::as_mapping_mut)
        .expect("families mapping")
        .insert(Value::String("table_v2".into()), Value::Null);
    rejects(&extra, "[family-set]");

    // Dropping a root key wholesale is refused before any value check.
    let mut dropped = base.clone();
    dropped
        .as_mapping_mut()
        .expect("mapping")
        .remove(Value::String("import_eligible".into()));
    rejects(&dropped, "[missing-key]");
}
