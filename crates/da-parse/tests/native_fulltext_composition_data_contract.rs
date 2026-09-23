//! S18/T01 DATA admission contract.
//! The fixture is intentionally parsed as generic YAML so this gate remains
//! independent from the future composer implementation.
//!
//! S02/T04 adds one *deliberately* code-bound block: the reference-link
//! integrity rule is checked against the composer's own constants and its
//! serialized ledger shape, because a contract that carries a second copy of the
//! vocabulary is not a contract. The pre-existing gates stay code-independent.
use da_parse::{
    NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION, NATIVE_FULLTEXT_REFERENCE_LINK_ID_SPACE,
    NATIVE_FULLTEXT_REFERENCE_LINK_POSITIONAL_FALLBACK,
    NATIVE_FULLTEXT_REFERENCE_LINK_REQUIRED_TARGETS, NATIVE_FULLTEXT_REFERENCE_LINK_RESOLVE_GATE,
    ReferenceLinkLedger,
};
use da_ports::citation_link::positional_entry_key;
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::PathBuf;

const AUTHORITY: &str = "data/parse/native_fulltext_composition.yaml";
const COMPARISON: &str = "data/parse/comparison_contract.yaml";
const LEDGER: &str = "data/parse/api_compatibility_ledger.yaml";
const ROLLBACK: &str = "java-grobid-http@0.9.0-crf";
const COMPONENTS: [&str; 7] = [
    "header",
    "references",
    "document_segmentation",
    "figure",
    "table",
    "formula",
    "learning_policy",
];
const STATES: [&str; 7] = [
    "passed",
    "fallback",
    "rejected",
    "unsupported",
    "not_observed",
    "invalid_evidence",
    "stale",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn yaml(path: &str) -> Value {
    serde_yaml::from_slice(&std::fs::read(root().join(path)).unwrap()).unwrap()
}
fn text(v: &Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(str::to_owned)
}
fn seq(v: &Value, path: &[&str]) -> Vec<String> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key).unwrap();
    }
    cur.as_sequence()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}
fn authority_errors(v: &Value) -> Vec<String> {
    let mut e = Vec::new();
    let required = [
        "schema_version",
        "adr_ref",
        "authority_id",
        "component_ids",
        "dispositions",
        "strata",
        "admission",
        "source_evidence",
        "component_receipts",
        "rollback_target",
        "redaction",
    ];
    for k in required {
        if v.get(k).is_none() {
            e.push(format!("missing:{k}"));
        }
    }
    if text(v, &["schema_version"]).as_deref() != Some("native-fulltext-composition.v1") {
        e.push("schema drift".into());
    }
    for (k, expected) in [
        ("observation_mode", "shadow"),
        ("promotion_state", "candidate"),
        ("auto_promote", "forbidden"),
        ("production_route", "forbidden"),
        ("graph_mutation", "forbidden"),
    ] {
        if text(v, &[k]).as_deref() != Some(expected) {
            e.push(format!("policy:{k}"));
        }
    }
    if v.get("import_eligible").and_then(Value::as_bool) != Some(false) {
        e.push("policy:import".into());
    }
    if text(v, &["rollback_target"]).as_deref() != Some(ROLLBACK) {
        e.push("rollback".into());
    }
    if seq(v, &["component_ids"]) != COMPONENTS {
        e.push("component ids".into());
    }
    if seq(v, &["dispositions"]) != STATES {
        e.push("disposition vocabulary".into());
    }
    if v.get("strata")
        .and_then(|x| x.get("closed"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        e.push("strata open".into());
    }
    let req = seq(v, &["admission", "native_substitution_requires"]);
    if req
        != [
            "exact_source_identity",
            "exact_schema_version",
            "optional_sha256_when_present",
            "passing_receipt",
        ]
    {
        e.push("admission identity binding".into());
    }
    if v.get("admission")
        .and_then(|x| x.get("terminal_receipt_is_pass"))
        .and_then(Value::as_bool)
        != Some(false)
    {
        e.push("terminal-as-pass".into());
    }
    if seq(v, &["admission", "independently_selectable"]) != ["header", "references"] {
        e.push("independently selectable must stay header and references".into());
    }
    let matrix = v.get("admission").and_then(|x| x.get("current_matrix"));
    for family in ["document_segmentation", "figure", "table", "formula"] {
        if text(matrix.unwrap_or(&Value::Null), &[family]).as_deref() != Some("fallback") {
            e.push(format!("matrix:{family} must stay fallback"));
        }
    }
    if text(v, &["admission", "pooled_winner"]).as_deref() != Some("forbidden") {
        e.push("pooling".into());
    }
    if text(v, &["admission", "fallback_target"]).as_deref() != Some(ROLLBACK) {
        e.push("fallback".into());
    }
    let matrix = v.get("admission").and_then(|x| x.get("current_matrix"));
    for (k, expected) in [
        ("header", "passed"),
        ("references", "passed"),
        ("document_segmentation", "fallback"),
        ("figure", "fallback"),
        ("table", "fallback"),
        ("formula", "fallback"),
        ("learning_policy", "not_observed"),
    ] {
        if matrix.and_then(|x| x.get(k)).and_then(Value::as_str) != Some(expected) {
            e.push(format!("matrix:{k}"));
        }
    }
    for (k, producer, disposition, evidence, schema) in [
        (
            "header",
            "S12",
            "passed",
            "passing",
            "native-composition.outcome/v1",
        ),
        (
            "references",
            "S12",
            "passed",
            "passing",
            "native-composition.outcome/v1",
        ),
        (
            "document_segmentation",
            "S15",
            "fallback",
            "invalid_evidence",
            "native-segmentation.v1",
        ),
        (
            "figure",
            "S16",
            "fallback",
            "unsupported",
            "native-visual-heads.v1",
        ),
        (
            "table",
            "S16",
            "fallback",
            "not_observed",
            "native-visual-heads.v1",
        ),
        (
            "formula",
            "S16",
            "fallback",
            "unsupported",
            "native-visual-heads.v1",
        ),
        (
            "learning_policy",
            "S17",
            "not_observed",
            "not_observed",
            "candidate-lineage.v1",
        ),
    ] {
        let r = v.get("component_receipts").and_then(|x| x.get(k));
        if r.and_then(|x| x.get("producer")).and_then(Value::as_str) != Some(producer) {
            e.push(format!("receipt producer:{k}"));
        }
        if r.and_then(|x| x.get("disposition")).and_then(Value::as_str) != Some(disposition) {
            e.push(format!("receipt disposition:{k}"));
        }
        if r.and_then(|x| x.get("evidence_state"))
            .and_then(Value::as_str)
            != Some(evidence)
        {
            e.push(format!("receipt evidence:{k}"));
        }
        if r.and_then(|x| x.get("schema_version"))
            .and_then(Value::as_str)
            != Some(schema)
        {
            e.push(format!("receipt schema:{k}"));
        }
        if r.and_then(|x| x.get("source_hash")).and_then(Value::as_str) != Some("required_sha256") {
            e.push(format!("receipt source hash:{k}"));
        }
        if r.and_then(|x| x.get("artifact_sha256"))
            .and_then(Value::as_str)
            != Some("optional_sha256_when_present")
        {
            e.push(format!("receipt artifact hash:{k}"));
        }
        for field in [
            "source_hash",
            "artifact_sha256",
            "schema_version",
            "fallback_target",
        ] {
            if r.and_then(|x| x.get(field)).is_none() {
                e.push(format!("receipt:{k}:{field}"));
            }
        }
    }
    let d = v
        .get("source_evidence")
        .and_then(|x| x.get("s12_named_denominators"))
        .unwrap();
    for (k, n) in [
        ("date_total", 173),
        ("header_raw_total", 36),
        ("header_accepted_total", 35),
        ("header_rejected_or_incomplete", 1),
        ("citation_names_total", 2198),
        ("affiliation_recognition_total", 13),
        ("affiliation_linking_total", 13),
        ("references_total", 207),
        ("anchors_total", 380),
        ("anchors_targeted", 217),
        ("anchors_targetless", 163),
        ("anchors_fragmented_adjacency", 168),
    ] {
        if d.get(k).and_then(Value::as_i64) != Some(n) {
            e.push(format!("denominator:{k}"));
        }
    }
    let lift = d.get("java_lift_same_n");
    if text(
        lift.unwrap_or(&Value::Null),
        &["java_header_authors_equals"],
    )
    .as_deref()
        != Some("header_raw_total")
    {
        e.push("java header equals raw, not accepted".into());
    }
    if text(lift.unwrap_or(&Value::Null), &["java_biblstruct_equals"]).as_deref()
        != Some("references_total")
    {
        e.push("java biblStruct equals references_total, not citation_names".into());
    }
    if lift
        .and_then(|x| x.get("hub_native_accepted_header"))
        .and_then(Value::as_i64)
        != Some(20)
        || lift
            .and_then(|x| x.get("hub_java_header_raw"))
            .and_then(Value::as_i64)
            != Some(21)
    {
        e.push("hub native accepted 20 vs java header_raw 21 must stay named".into());
    }
    if lift
        .and_then(|x| x.get("citation_names_not_biblstruct"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        e.push("citation_names is not biblStruct".into());
    }
    if text(lift.unwrap_or(&Value::Null), &["pooling_accepted_with_raw"]).as_deref()
        != Some("forbidden")
        || text(
            lift.unwrap_or(&Value::Null),
            &["pooling_citation_names_with_references"],
        )
        .as_deref()
            != Some("forbidden")
    {
        e.push("pooling named header or citation populations".into());
    }
    for (section, expected) in [
        ("s15_shared_intersection", "immutable_s15_manifest"),
        ("s16_shared_intersection", "immutable_s16_manifest"),
        ("s17_lineage", "immutable_s17_candidate_lineage"),
    ] {
        if text(v, &["source_evidence", section, "source"]).as_deref() != Some(expected) {
            e.push(format!("source:{section}"));
        }
    }
    if v.get("source_evidence")
        .and_then(|x| x.get("s15_shared_intersection"))
        .and_then(|x| x.get("terminal_lane_receipts"))
        .and_then(Value::as_i64)
        != Some(9)
    {
        e.push("S15 receipts".into());
    }
    if v.get("source_evidence")
        .and_then(|x| x.get("s16_shared_intersection"))
        .and_then(|x| x.get("visual_receipts"))
        .and_then(Value::as_i64)
        != Some(27)
    {
        e.push("S16 receipts".into());
    }
    let forbidden = seq(v, &["redaction", "forbidden"]);
    for x in [
        "raw_tei",
        "tei_xml",
        "pdf_bytes",
        "body_text",
        "physical_path",
        "hostname",
        "executable",
        "secret",
    ] {
        if !forbidden.contains(&x.to_owned()) {
            e.push(format!("redaction:{x}"));
        }
    }
    e
}
fn mutate(mut v: Value, path: &[&str], value: Value) -> Value {
    let mut cur = &mut v;
    for key in &path[..path.len() - 1] {
        cur = cur.get_mut(*key).unwrap();
    }
    let map: &mut Mapping = cur.as_mapping_mut().unwrap();
    map.insert(Value::String(path[path.len() - 1].into()), value);
    v
}
fn rejects(v: Value, marker: &str) {
    let errors = authority_errors(&v);
    assert!(
        errors.iter().any(|x| x.contains(marker)),
        "mutation `{marker}` was accepted: {errors:?}"
    );
}

#[test]
fn native_fulltext_authority_is_closed_and_honest() {
    let a = yaml(AUTHORITY);
    assert!(
        authority_errors(&a).is_empty(),
        "authority errors: {:?}",
        authority_errors(&a)
    );
    let c = yaml(COMPARISON);
    assert_eq!(
        text(
            &c,
            &["native_fulltext_composition_shadow", "schema_version"]
        )
        .as_deref(),
        Some("native-fulltext-composition.v1")
    );
    let l = yaml(LEDGER);
    assert!(
        l.get("entries")
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .any(|entry| {
                entry.get("id").and_then(Value::as_str)
                    == Some("parse.component.native_fulltext_composition_shadow")
            })
    );
}

#[test]
fn native_fulltext_mutations_fail_closed() {
    let a = yaml(AUTHORITY);
    rejects(
        mutate(
            a.clone(),
            &["admission", "terminal_receipt_is_pass"],
            Value::Bool(true),
        ),
        "terminal-as-pass",
    );
    rejects(
        mutate(
            a.clone(),
            &[
                "source_evidence",
                "s12_named_denominators",
                "references_total",
            ],
            Value::Number(206.into()),
        ),
        "denominator:references_total",
    );
    rejects(
        mutate(
            a.clone(),
            &["dispositions"],
            Value::Sequence(vec![Value::String("mystery".into())]),
        ),
        "disposition vocabulary",
    );
    rejects(
        mutate(
            a.clone(),
            &["component_receipts", "header", "artifact_sha256"],
            Value::Null,
        ),
        "receipt artifact hash:header",
    );
    rejects(
        mutate(
            a.clone(),
            &[
                "component_receipts",
                "document_segmentation",
                "evidence_state",
            ],
            Value::String("passing".into()),
        ),
        "receipt evidence:document_segmentation",
    );
    rejects(
        mutate(
            a.clone(),
            &["component_receipts", "figure", "schema_version"],
            Value::String("native-visual-heads.v2".into()),
        ),
        "receipt schema:figure",
    );
    rejects(
        mutate(
            a.clone(),
            &["component_receipts", "table", "source_hash"],
            Value::String("stale".into()),
        ),
        "receipt source hash:table",
    );
    rejects(
        mutate(
            a.clone(),
            &[
                "source_evidence",
                "s12_named_denominators",
                "java_lift_same_n",
                "java_header_authors_equals",
            ],
            Value::String("header_accepted_total".into()),
        ),
        "java header equals raw, not accepted",
    );
    rejects(
        mutate(
            a.clone(),
            &[
                "source_evidence",
                "s12_named_denominators",
                "java_lift_same_n",
                "citation_names_not_biblstruct",
            ],
            Value::Bool(false),
        ),
        "citation_names is not biblStruct",
    );
    rejects(
        mutate(
            a.clone(),
            &["admission", "pooled_winner"],
            Value::String("best".into()),
        ),
        "pooling",
    );
    rejects(
        mutate(
            a.clone(),
            &["rollback_target"],
            Value::String("none".into()),
        ),
        "rollback",
    );
    rejects(
        mutate(
            a.clone(),
            &["promotion_state"],
            Value::String("promoted".into()),
        ),
        "policy:promotion_state",
    );
    rejects(
        mutate(a.clone(), &["import_eligible"], Value::Bool(true)),
        "policy:import",
    );
    rejects(
        mutate(a, &["redaction", "forbidden"], Value::Sequence(vec![])),
        "redaction:raw_tei",
    );
}

// ───────── S02/T04: reference-link integrity DATA contract ─────────

/// The governor registry that binds shipped DATA and artifacts by digest.
///
/// The reference-link contract is registered only through this file's existing
/// `data/parse/native_fulltext_composition.yaml` binding: never by adding a
/// second registry entry and never by adding a second `data/parse/*.yaml`
/// carrier (D377).
const REGISTRY: &str = "scripts/governor/checks/scholarly_parse_plane.yaml";
const REFERENCE_LINK_CONTRACT_ID: &str = "native-fulltext-reference-link-integrity.v1";
const REFERENCE_LINK_REFUSAL_CLASSES: [&str; 4] = [
    "BibliographyDomainMismatch",
    "BibliographyKeyCollision",
    "UnresolvedBibliographyTarget",
    "InconservableBibliographyChild",
];

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The composer's serialized ledger shape. Comparing DATA against *this* rather
/// than a second literal copy is what makes `ledger_fields` a contract: a field
/// added to or dropped from `ReferenceLinkLedger` cannot leave the DATA block
/// stale, and `#[serde(default)]` keeps pre-S02 manifests decodable.
fn ledger_field_names() -> BTreeSet<String> {
    serde_json::to_value(ReferenceLinkLedger::default())
        .expect("the ledger serializes")
        .as_object()
        .expect("the ledger serializes to an object")
        .keys()
        .cloned()
        .collect()
}

/// Every reason the `reference_link_integrity` DATA block is unacceptable.
///
/// The scalar tokens are compared against the composer's own constants
/// (`NATIVE_FULLTEXT_REFERENCE_LINK_*`), never against a re-typed literal, so a
/// rename on either side fails this gate instead of silently drifting.
fn reference_link_integrity_errors(v: &Value) -> Vec<String> {
    let mut e = Vec::new();
    let Some(block) = v.get("reference_link_integrity") else {
        e.push("reference-link-integrity:missing".into());
        return e;
    };
    for (field, expected) in [
        ("contract_id", REFERENCE_LINK_CONTRACT_ID),
        ("adr_ref", "ADR-076"),
        ("id_space", NATIVE_FULLTEXT_REFERENCE_LINK_ID_SPACE),
        ("derivation", NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION),
        (
            "positional_fallback",
            NATIVE_FULLTEXT_REFERENCE_LINK_POSITIONAL_FALLBACK,
        ),
        (
            "required_targets",
            NATIVE_FULLTEXT_REFERENCE_LINK_REQUIRED_TARGETS,
        ),
        ("resolve_gate", NATIVE_FULLTEXT_REFERENCE_LINK_RESOLVE_GATE),
    ] {
        if block.get(field).and_then(Value::as_str) != Some(expected) {
            e.push(format!("reference-link-integrity:{field}"));
        }
    }
    let classes: Vec<&str> = block
        .get("refusal_classes")
        .and_then(Value::as_sequence)
        .map(|seq| seq.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if classes != REFERENCE_LINK_REFUSAL_CLASSES {
        e.push("reference-link-integrity:refusal_classes".into());
    }
    let declared: BTreeSet<String> = block
        .get("ledger_fields")
        .and_then(Value::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if declared.is_empty() || declared != ledger_field_names() {
        e.push("reference-link-integrity:ledger_fields".into());
    }
    if block.get("pooling").and_then(Value::as_str) != Some("forbidden") {
        e.push("reference-link-integrity:pooling".into());
    }
    e
}

fn rejects_reference_link(v: Value, marker: &str) {
    let errors = reference_link_integrity_errors(&v);
    assert!(
        errors.iter().any(|x| x.contains(marker)),
        "reference-link mutation `{marker}` was accepted: {errors:?}"
    );
}

#[test]
fn reference_link_integrity_matches_the_composer_vocabulary() {
    let a = yaml(AUTHORITY);
    let errors = reference_link_integrity_errors(&a);
    assert!(
        errors.is_empty(),
        "reference-link integrity errors: {errors:?}"
    );
    // A whole-block absence must be reported as an absence, not as drift.
    assert!(
        reference_link_integrity_errors(&Value::Null)
            .iter()
            .any(|x| x.contains("reference-link-integrity:missing"))
    );

    // The positional fallback is not a second copy of the literal: it renders
    // exactly the text the composer's citation-link entry key publishes.
    let template = text(&a, &["reference_link_integrity", "positional_fallback"])
        .expect("the fallback template is declared");
    for (ordinal, expected) in [(0_u32, "b0"), (7, "b7"), (42, "b42")] {
        assert_eq!(
            positional_entry_key(ordinal),
            expected,
            "the composer's positional fallback changed shape"
        );
        assert_eq!(
            template.replace("{ordinal}", &ordinal.to_string()),
            positional_entry_key(ordinal),
            "the DATA fallback template must render like positional_entry_key"
        );
    }
}

#[test]
fn reference_link_integrity_is_registered_only_through_the_sanctioned_block() {
    // No parallel carrier: the contract lives inside an already sanctioned DATA
    // file, never in a new `data/parse/*.yaml` (D377).
    for entry in std::fs::read_dir(root().join("data/parse")).expect("data/parse is readable") {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .to_string();
        assert!(
            !name.contains("reference_link"),
            "the reference-link contract must not create a parallel data/parse carrier: {name}"
        );
    }

    // Exactly one registry binding, and it must be current.
    let registry = yaml(REGISTRY);
    let mut bindings: Vec<(String, String)> = Vec::new();
    for section in ["source_data", "canonical_artifacts"] {
        let items = registry
            .get("bindings")
            .and_then(|b| b.get(section))
            .and_then(Value::as_sequence)
            .cloned()
            .unwrap_or_default();
        for item in items {
            bindings.push((
                item.get("path")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                item.get("sha256")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ));
        }
    }
    let ours: Vec<&(String, String)> = bindings
        .iter()
        .filter(|(path, _)| path == AUTHORITY)
        .collect();
    assert_eq!(
        ours.len(),
        1,
        "the composition DATA must carry exactly one governor registry binding"
    );
    let on_disk = std::fs::read(root().join(AUTHORITY)).expect("the composition DATA is readable");
    assert_eq!(
        ours[0].1,
        sha256_hex(&on_disk),
        "the registry digest must be re-pinned to the current composition DATA"
    );
    assert!(
        !bindings
            .iter()
            .any(|(path, _)| path.contains("reference_link_integrity")),
        "the reference-link contract must not add a second registry entry"
    );
}

#[test]
fn reference_link_integrity_mutations_fail_closed() {
    let a = yaml(AUTHORITY);
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "id_space"],
            Value::String("bibl-3".into()),
        ),
        "reference-link-integrity:id_space",
    );
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "derivation"],
            Value::String("synthesized_bibl_domain".into()),
        ),
        "reference-link-integrity:derivation",
    );
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "positional_fallback"],
            Value::String("b{index}".into()),
        ),
        "reference-link-integrity:positional_fallback",
    );
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "required_targets"],
            Value::String("all_ref_elements".into()),
        ),
        "reference-link-integrity:required_targets",
    );
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "resolve_gate"],
            Value::String("pre_splice".into()),
        ),
        "reference-link-integrity:resolve_gate",
    );
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "contract_id"],
            Value::String("invented.contract".into()),
        ),
        "reference-link-integrity:contract_id",
    );
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "adr_ref"],
            Value::String("ADR-075".into()),
        ),
        "reference-link-integrity:adr_ref",
    );
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "refusal_classes"],
            Value::Sequence(
                REFERENCE_LINK_REFUSAL_CLASSES[..3]
                    .iter()
                    .map(|x| Value::String((*x).into()))
                    .collect(),
            ),
        ),
        "reference-link-integrity:refusal_classes",
    );
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "ledger_fields"],
            Value::Sequence(vec![Value::String("source_entries".into())]),
        ),
        "reference-link-integrity:ledger_fields",
    );
    rejects_reference_link(
        mutate(
            a.clone(),
            &["reference_link_integrity", "pooling"],
            Value::String("allowed".into()),
        ),
        "reference-link-integrity:pooling",
    );
    rejects_reference_link(
        mutate(a, &["reference_link_integrity"], Value::Null),
        "reference-link-integrity:contract_id",
    );
}
