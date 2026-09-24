//! S05/T02: frozen corpus identity, denominators and metric as a DATA block.
//!
//! The independent corpus regression of S05/T01 measures ten classes over the
//! three sha256-pinned preserved TEI. This gate freezes that measurement as DATA
//! inside the already-sanctioned `native_fulltext_composition_shadow` block of
//! `data/parse/comparison_contract.yaml`, so a corpus regression is a NUMBER,
//! not prose:
//!
//! * a missing block is `corpus-semantic-regression:missing` — absence, not
//!   drift;
//! * every class token, the refusal vocabulary and the fallback target are
//!   compared against the composer's LIVE constants
//!   ([`SemanticFieldLoss::ALL`], [`citation_field_role_token`],
//!   [`NATIVE_FULLTEXT_FALLBACK_TARGET`],
//!   [`NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION`]), never a second literal
//!   copy of the vocabulary;
//! * the frozen metric (`precision`/`recall`/`f1`) is re-derived from the frozen
//!   counters, so a substituted value fails instead of being trusted;
//! * every document `sha256` is re-computed from the tracked fixture bytes;
//! * DATA↔DATA: the block must agree with
//!   `data/parse/native_fulltext_composition.yaml#source_evidence.s12_named_denominators`
//!   and with the parent block's own `s12_named_denominators`.
//!
//! D377: no new `data/parse/*` file is created — the block is a child of the
//! existing sanctioned carrier. Shadow/offline only; D127 stays closed.
use da_parse::{
    CitationFieldRole, NATIVE_FULLTEXT_FALLBACK_TARGET, NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION,
    SemanticFieldLoss, citation_field_role_token,
};
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const COMPARISON: &str = "data/parse/comparison_contract.yaml";
const AUTHORITY: &str = "data/parse/native_fulltext_composition.yaml";
const REGISTRY: &str = "scripts/governor/checks/scholarly_parse_plane.yaml";
const POSITIVE_FIXTURE: &str = "governor/fixtures/scholarly_parse_plane/contract.positive.json";
const SHADOW_KEY: &str = "native_fulltext_composition_shadow";
const BLOCK_KEY: &str = "corpus_semantic_regression";

const CONTRACT_ID: &str = "native-fulltext-corpus-semantic-regression.v1";
const SCHEMA_VERSION: &str = "native-fulltext-corpus-regression.v1";
const ADR_REF: &str = "ADR-076";
const IDENTITY_KIND: &str = "preserved_three_sha";
const POOLING: &str = "forbidden";
/// The extra measured class S05/T01 added beyond the production loss tokens.
const EXTRA_CLASS: &str = "AdmittedAuthorLine";
const MEASURED_ROLES: [(CitationFieldRole, &str); 4] = [
    (CitationFieldRole::Doi, "CitationFieldRole::doi"),
    (CitationFieldRole::ArxivId, "CitationFieldRole::arxiv_id"),
    (CitationFieldRole::Url, "CitationFieldRole::url"),
    (
        CitationFieldRole::ArticleTitle,
        "CitationFieldRole::article_title",
    ),
];
/// The production loss classes this corpus measures directly. `CitationRole` is
/// measured indirectly through the per-role citation classes below, so it is in
/// `refusal_classes` but not in `class_order`.
const MEASURED_LOSS_CLASSES: [SemanticFieldLoss; 4] = [
    SemanticFieldLoss::HeaderTitle,
    SemanticFieldLoss::HeaderEmail,
    SemanticFieldLoss::HeaderIds,
    SemanticFieldLoss::AuthorAffiliation,
];
const USED_BY: [&str; 3] = ["S05-T01", "S05-T03", "S05-T04"];
const DETECTIONS: [&str; 4] = [
    "typed_refusal",
    "metric_deviation",
    "byte_accounting",
    "identity_failure",
];
/// The metric definition is DATA; these are the frozen tokens it must carry.
const METRIC_DEFINITION: [(&str, &str); 9] = [
    ("aggregation", "micro_occurrence"),
    ("tp", "conserved"),
    ("fp", "fabricated"),
    ("fn", "admitted_minus_conserved"),
    ("precision", "tp_over_tp_plus_fp"),
    ("recall", "tp_over_tp_plus_fn"),
    ("f1", "harmonic_mean_of_precision_and_recall"),
    ("admission_coverage", "admitted_over_source_population"),
    (
        "zero_zero_denominator",
        "coverage_is_one_at_fully_admitted_empty_population",
    ),
];
/// The named denominators the corpus block freezes and cross-checks.
const DECLARED_DENOMINATORS: [&str; 9] = [
    "references_total",
    "header_raw_total",
    "header_title_elements_total",
    "header_emails_total",
    "header_pubnum_elements_total",
    "affiliation_linking_total",
    "bibl_struct_hub_total",
    "bibl_struct_rlm_total",
    "bibl_struct_reca_total",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn yaml(path: &str) -> Value {
    let bytes = std::fs::read(root().join(path))
        .unwrap_or_else(|error| panic!("{path}: unreadable ({error})"));
    serde_yaml::from_slice(&bytes).unwrap_or_else(|error| panic!("{path}: unparsable ({error})"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn block(comparison: &Value) -> Option<&Value> {
    comparison.get(SHADOW_KEY)?.get(BLOCK_KEY)
}

fn seq(value: &Value, path: &[&str]) -> Vec<String> {
    let mut cursor = value;
    for key in path {
        match cursor.get(*key) {
            Some(next) => cursor = next,
            None => return Vec::new(),
        }
    }
    cursor
        .as_sequence()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn counter(record: &Value, key: &str) -> Option<usize> {
    record
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        1.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn harmonic(precision: f64, recall: f64) -> f64 {
    if precision + recall == 0.0 {
        1.0
    } else {
        2.0 * precision * recall / (precision + recall)
    }
}

fn float_eq(stored: Option<f64>, expected: f64) -> bool {
    stored.is_some_and(|value| (value - expected).abs() < 1e-9)
}

/// The live class vocabulary: the extra measured class, the four directly
/// measured production loss tokens, the reference-domain token and the four
/// measured citation roles.
fn expected_class_order() -> Vec<String> {
    let mut order = vec![EXTRA_CLASS.to_string()];
    order.extend(
        MEASURED_LOSS_CLASSES
            .iter()
            .map(|class| class.token().to_string()),
    );
    order.push(NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION.to_string());
    order.extend(MEASURED_ROLES.iter().map(|(_, token)| token.to_string()));
    order
}

/// Every reason the `corpus_semantic_regression` DATA block is unacceptable.
fn corpus_semantic_regression_errors(comparison: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(block) = block(comparison) else {
        errors.push("corpus-semantic-regression:missing".into());
        return errors;
    };

    // ── identity scalars ──
    for (field, expected) in [
        ("contract_id", CONTRACT_ID),
        ("schema_version", SCHEMA_VERSION),
        ("adr_ref", ADR_REF),
        ("corpus_id", IDENTITY_KIND),
        ("identity_kind", IDENTITY_KIND),
        ("fallback_target", NATIVE_FULLTEXT_FALLBACK_TARGET),
        ("pooling", POOLING),
    ] {
        if block.get(field).and_then(Value::as_str) != Some(expected) {
            errors.push(format!("corpus-semantic-regression:{field}"));
        }
    }
    if block.get("unique_source_sha256").and_then(Value::as_u64) != Some(3) {
        errors.push("corpus-semantic-regression:unique_source_sha256".into());
    }

    // ── locks ──
    let locks = block.get("locks").unwrap_or(&Value::Null);
    if locks.get("import_eligible").and_then(Value::as_bool) != Some(false) {
        errors.push("corpus-semantic-regression:locks.import_eligible".into());
    }
    for key in ["production_route", "graph_write", "auto_promote"] {
        if locks.get(key).and_then(Value::as_str) != Some("forbidden") {
            errors.push(format!("corpus-semantic-regression:locks.{key}"));
        }
    }
    if locks.get("deployed").and_then(Value::as_bool) != Some(false) {
        errors.push("corpus-semantic-regression:locks.deployed".into());
    }

    // ── vocabulary bound to the LIVE composer constants ──
    let expected_refusals: Vec<String> = SemanticFieldLoss::ALL
        .iter()
        .map(|class| class.token().to_string())
        .collect();
    if seq(block, &["refusal_classes"]) != expected_refusals {
        errors.push("corpus-semantic-regression:refusal_classes".into());
    }
    let order = seq(block, &["class_order"]);
    for (role, token) in MEASURED_ROLES {
        let wire = citation_field_role_token(role);
        if token != format!("CitationFieldRole::{wire}") {
            errors.push(format!("corpus-semantic-regression:role_token:{token}"));
        }
        // The composer token must mirror the serde wire form, so a renamed role
        // variant cannot leave the DATA block stale.
        let serde_token = serde_json::to_value(role)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned));
        if serde_token.as_deref() != Some(wire) {
            errors.push(format!("corpus-semantic-regression:role_serde:{token}"));
        }
    }
    if order != expected_class_order() {
        errors.push("corpus-semantic-regression:class_order".into());
    }

    // ── frozen metric re-derived from the frozen counters ──
    let classes = block.get("classes").unwrap_or(&Value::Null);
    let Some(class_map) = classes.as_mapping() else {
        errors.push("corpus-semantic-regression:classes".into());
        return errors;
    };
    if class_map.len() != order.len() {
        errors.push("corpus-semantic-regression:classes".into());
    }
    let mut totals = (0usize, 0usize, 0usize);
    let mut class_population: BTreeMap<String, usize> = BTreeMap::new();
    let mut class_admitted: BTreeMap<String, usize> = BTreeMap::new();
    for class in &order {
        let Some(record) = class_map.get(class.as_str()) else {
            errors.push(format!("corpus-semantic-regression:classes.{class}"));
            continue;
        };
        let (Some(population), Some(admitted), Some(conserved), Some(fabricated)) = (
            counter(record, "source_population"),
            counter(record, "admitted"),
            counter(record, "conserved"),
            counter(record, "fabricated"),
        ) else {
            errors.push(format!(
                "corpus-semantic-regression:classes.{class}.counters"
            ));
            continue;
        };
        class_population.insert(class.clone(), population);
        class_admitted.insert(class.clone(), admitted);
        if population == 0 {
            errors.push(format!(
                "corpus-semantic-regression:classes.{class}.source_population"
            ));
        }
        if conserved != admitted {
            errors.push(format!(
                "corpus-semantic-regression:classes.{class}.conserved"
            ));
        }
        if fabricated != 0 {
            errors.push(format!(
                "corpus-semantic-regression:classes.{class}.fabricated"
            ));
        }
        let lost = admitted.saturating_sub(conserved);
        let precision = ratio(conserved, conserved + fabricated);
        let recall = ratio(conserved, conserved + lost);
        if !float_eq(record.get("precision").and_then(Value::as_f64), precision) {
            errors.push(format!(
                "corpus-semantic-regression:classes.{class}.precision"
            ));
        }
        if !float_eq(record.get("recall").and_then(Value::as_f64), recall) {
            errors.push(format!("corpus-semantic-regression:classes.{class}.recall"));
        }
        if !float_eq(
            record.get("f1").and_then(Value::as_f64),
            harmonic(precision, recall),
        ) {
            errors.push(format!("corpus-semantic-regression:classes.{class}.f1"));
        }
        totals.0 += conserved;
        totals.1 += fabricated;
        totals.2 += lost;
    }

    let aggregate = block.get("aggregate").unwrap_or(&Value::Null);
    let (tp, fp, fn_) = totals;
    if counter(aggregate, "tp") != Some(tp) {
        errors.push("corpus-semantic-regression:aggregate.tp".into());
    }
    if counter(aggregate, "fp") != Some(fp) {
        errors.push("corpus-semantic-regression:aggregate.fp".into());
    }
    if counter(aggregate, "fn") != Some(fn_) {
        errors.push("corpus-semantic-regression:aggregate.fn".into());
    }
    let precision = ratio(tp, tp + fp);
    let recall = ratio(tp, tp + fn_);
    if !float_eq(
        aggregate.get("precision").and_then(Value::as_f64),
        precision,
    ) {
        errors.push("corpus-semantic-regression:aggregate.precision".into());
    }
    if !float_eq(aggregate.get("recall").and_then(Value::as_f64), recall) {
        errors.push("corpus-semantic-regression:aggregate.recall".into());
    }
    if !float_eq(
        aggregate.get("f1").and_then(Value::as_f64),
        harmonic(precision, recall),
    ) {
        errors.push("corpus-semantic-regression:aggregate.f1".into());
    }

    // ── metric definition ──
    let metric = block.get("metric").unwrap_or(&Value::Null);
    for (key, expected) in METRIC_DEFINITION {
        if metric.get(key).and_then(Value::as_str) != Some(expected) {
            errors.push(format!("corpus-semantic-regression:metric.{key}"));
        }
    }
    if metric
        .get("measured_at")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        errors.push("corpus-semantic-regression:metric.measured_at".into());
    }

    // ── documents: length, live sha256, per-class sums ──
    let documents = block.get("documents").and_then(Value::as_sequence);
    let Some(documents) = documents else {
        errors.push("corpus-semantic-regression:documents".into());
        return errors;
    };
    if documents.len() != 3 {
        errors.push("corpus-semantic-regression:documents".into());
    }
    let denominators = block.get("denominators").unwrap_or(&Value::Null);
    if counter(denominators, "documents") != Some(documents.len()) {
        errors.push("corpus-semantic-regression:denominators.documents".into());
    }
    if counter(denominators, "unique_source_sha256") != Some(3) {
        errors.push("corpus-semantic-regression:denominators.unique_source_sha256".into());
    }
    let mut digests: BTreeSet<String> = BTreeSet::new();
    let mut population_sums: BTreeMap<String, usize> = BTreeMap::new();
    let mut admitted_sums: BTreeMap<String, usize> = BTreeMap::new();
    for document in documents {
        let Some(id) = document.get("id").and_then(Value::as_str) else {
            errors.push("corpus-semantic-regression:document.id".into());
            continue;
        };
        let Some(path) = document.get("path").and_then(Value::as_str) else {
            errors.push(format!("corpus-semantic-regression:document:{id}.path"));
            continue;
        };
        let Some(declared) = document.get("sha256").and_then(Value::as_str) else {
            errors.push(format!("corpus-semantic-regression:document:{id}.sha256"));
            continue;
        };
        match std::fs::read(root().join(path)) {
            Ok(bytes) => {
                if sha256_hex(&bytes) != declared {
                    errors.push(format!("corpus-semantic-regression:document_sha256:{id}"));
                }
            }
            Err(error) => errors.push(format!(
                "corpus-semantic-regression:document_unreadable:{id}:{error}"
            )),
        }
        if !digests.insert(declared.to_string()) {
            errors.push(format!(
                "corpus-semantic-regression:document_sha256:{id}:duplicate"
            ));
        }
        for (field, sums) in [
            ("source_population", &mut population_sums),
            ("admitted", &mut admitted_sums),
        ] {
            let Some(map) = document.get(field) else {
                errors.push(format!("corpus-semantic-regression:document:{id}.{field}"));
                continue;
            };
            if map.as_mapping().map(serde_yaml::Mapping::len) != Some(order.len()) {
                errors.push(format!("corpus-semantic-regression:document:{id}.{field}"));
            }
            for class in &order {
                let Some(value) = map.get(class.as_str()).and_then(Value::as_u64) else {
                    errors.push(format!(
                        "corpus-semantic-regression:document:{id}.{field}.{class}"
                    ));
                    continue;
                };
                *sums.entry(class.clone()).or_default() += value as usize;
            }
        }
    }
    for class in &order {
        if class_population.get(class) != population_sums.get(class) {
            errors.push(format!(
                "corpus-semantic-regression:documents.source_population.{class}"
            ));
        }
        if class_admitted.get(class) != admitted_sums.get(class) {
            errors.push(format!(
                "corpus-semantic-regression:documents.admitted.{class}"
            ));
        }
    }

    // ── denominators restate the class/документ totals ──
    let class_pop = |class: &str| class_population.get(class).copied();
    let doc_reference_population = |index: usize| {
        documents
            .get(index)
            .and_then(|document| document.get("source_population"))
            .and_then(|map| map.get(NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION))
            .and_then(Value::as_u64)
            .map(|value| value as usize)
    };
    let restated: [(&str, Option<usize>); 9] = [
        (
            "references_total",
            class_pop(NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION),
        ),
        ("header_raw_total", class_pop(EXTRA_CLASS)),
        (
            "header_title_elements_total",
            class_pop(SemanticFieldLoss::HeaderTitle.token()),
        ),
        (
            "header_emails_total",
            class_pop(SemanticFieldLoss::HeaderEmail.token()),
        ),
        (
            "header_pubnum_elements_total",
            class_pop(SemanticFieldLoss::HeaderIds.token()),
        ),
        (
            "affiliation_linking_total",
            class_pop(SemanticFieldLoss::AuthorAffiliation.token()),
        ),
        ("bibl_struct_hub_total", doc_reference_population(0)),
        ("bibl_struct_rlm_total", doc_reference_population(1)),
        ("bibl_struct_reca_total", doc_reference_population(2)),
    ];
    for (name, measured) in restated {
        if counter(denominators, name) != measured {
            errors.push(format!("corpus-semantic-regression:denominators.{name}"));
        }
    }
    for name in DECLARED_DENOMINATORS {
        if denominators.get(name).and_then(Value::as_u64).is_none() {
            errors.push(format!("corpus-semantic-regression:denominators.{name}"));
        }
    }

    // ── mutation matrix (consumed by S05/T03) ──
    let matrix = block.get("mutation_matrix").and_then(Value::as_sequence);
    let Some(matrix) = matrix else {
        errors.push("corpus-semantic-regression:mutation_matrix".into());
        return errors;
    };
    if matrix.len() < 11 {
        errors.push("corpus-semantic-regression:mutation_matrix".into());
    }
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for entry in matrix {
        let Some(id) = entry.get("id").and_then(Value::as_str) else {
            errors.push("corpus-semantic-regression:mutation_matrix.id".into());
            continue;
        };
        if !ids.insert(id.to_string()) {
            errors.push(format!(
                "corpus-semantic-regression:mutation_matrix.duplicate:{id}"
            ));
        }
        let Some(detection) = entry.get("detection").and_then(Value::as_str) else {
            errors.push(format!("corpus-semantic-regression:mutation_matrix.{id}"));
            continue;
        };
        if !DETECTIONS.contains(&detection) {
            errors.push(format!("corpus-semantic-regression:mutation_matrix.{id}"));
            continue;
        }
        let refusal = entry.get("refusal_class").and_then(Value::as_str);
        match detection {
            "typed_refusal" => {
                let Some(refusal) = refusal else {
                    errors.push(format!(
                        "corpus-semantic-regression:mutation_matrix.{id}.refusal_class"
                    ));
                    continue;
                };
                if refusal.starts_with("SemanticFieldLoss::")
                    && !expected_refusals.iter().any(|class| class == refusal)
                {
                    errors.push(format!(
                        "corpus-semantic-regression:mutation_matrix.{id}.refusal_class"
                    ));
                }
            }
            "metric_deviation" => {
                if entry.get("metric").and_then(Value::as_str) != Some("precision")
                    || entry.get("expectation").and_then(Value::as_str) != Some("below_one")
                {
                    errors.push(format!("corpus-semantic-regression:mutation_matrix.{id}"));
                }
            }
            "byte_accounting" => {
                if refusal.is_none_or(str::is_empty) {
                    errors.push(format!(
                        "corpus-semantic-regression:mutation_matrix.{id}.refusal_class"
                    ));
                }
            }
            "identity_failure"
                if entry.get("expectation").and_then(Value::as_str)
                    != Some("unique_source_sha256_not_three") =>
            {
                errors.push(format!("corpus-semantic-regression:mutation_matrix.{id}"));
            }
            _ => {}
        }
    }

    if seq(block, &["used_by"]) != USED_BY {
        errors.push("corpus-semantic-regression:used_by".into());
    }

    errors
}

/// DATA↔DATA: the frozen denominators must agree with the independently declared
/// `s12_named_denominators` of the composition authority, and the document count
/// with the shared-intersection document count. A divergence is named by carrier
/// and declaration, never rounded away.
fn carrier_errors(block: &Value, authority: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(declared) = authority
        .get("source_evidence")
        .and_then(|evidence| evidence.get("s12_named_denominators"))
    else {
        return vec![format!(
            "corpus-semantic-regression:carrier:{AUTHORITY}#source_evidence.s12_named_denominators"
        )];
    };
    let Some(denominators) = block.get("denominators") else {
        return vec!["corpus-semantic-regression:missing".into()];
    };
    let class_pop = |class: &str| {
        block
            .get("classes")
            .and_then(|classes| classes.get(class))
            .and_then(|record| record.get("source_population"))
            .and_then(Value::as_u64)
    };
    let doc_reference_population = |index: usize| {
        block
            .get("documents")
            .and_then(Value::as_sequence)
            .and_then(|documents| documents.get(index))
            .and_then(|document| document.get("source_population"))
            .and_then(|map| map.get(NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION))
            .and_then(Value::as_u64)
    };
    let restated: [(&str, Option<u64>); 9] = [
        (
            "references_total",
            class_pop(NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION),
        ),
        ("header_raw_total", class_pop(EXTRA_CLASS)),
        (
            "header_title_elements_total",
            class_pop(SemanticFieldLoss::HeaderTitle.token()),
        ),
        (
            "header_emails_total",
            class_pop(SemanticFieldLoss::HeaderEmail.token()),
        ),
        (
            "header_pubnum_elements_total",
            class_pop(SemanticFieldLoss::HeaderIds.token()),
        ),
        (
            "affiliation_linking_total",
            class_pop(SemanticFieldLoss::AuthorAffiliation.token()),
        ),
        ("bibl_struct_hub_total", doc_reference_population(0)),
        ("bibl_struct_rlm_total", doc_reference_population(1)),
        ("bibl_struct_reca_total", doc_reference_population(2)),
    ];
    for (name, measured) in restated {
        let expected = declared.get(name).and_then(Value::as_u64);
        if measured != expected {
            errors.push(format!(
                "corpus-semantic-regression:carrier:{AUTHORITY}#source_evidence.s12_named_denominators.{name}"
            ));
        }
        if denominators.get(name).and_then(Value::as_u64) != expected {
            errors.push(format!("corpus-semantic-regression:denominators.{name}"));
        }
    }
    let declared_documents = authority
        .get("source_evidence")
        .and_then(|evidence| evidence.get("s15_shared_intersection"))
        .and_then(|intersection| intersection.get("documents"))
        .and_then(Value::as_u64);
    if denominators.get("documents").and_then(Value::as_u64) != declared_documents {
        errors.push(format!(
            "corpus-semantic-regression:carrier:{AUTHORITY}#source_evidence.s15_shared_intersection.documents"
        ));
    }
    errors
}

/// DATA↔DATA within one file: the parent block already publishes the same named
/// denominators, so the child must agree with the sibling it is nested inside.
fn parent_carrier_errors(comparison: &Value) -> Vec<String> {
    let Some(block) = block(comparison) else {
        return vec!["corpus-semantic-regression:missing".into()];
    };
    let Some(parent) = comparison
        .get(SHADOW_KEY)
        .and_then(|shadow| shadow.get("source_evidence"))
        .and_then(|evidence| evidence.get("s12_named_denominators"))
    else {
        return vec![format!(
            "corpus-semantic-regression:carrier:{COMPARISON}#{SHADOW_KEY}.source_evidence.s12_named_denominators"
        )];
    };
    let mut errors = Vec::new();
    for name in [
        "references_total",
        "header_raw_total",
        "affiliation_linking_total",
    ] {
        let expected = parent.get(name).and_then(Value::as_u64);
        if block
            .get("denominators")
            .and_then(|denominators| denominators.get(name))
            .and_then(Value::as_u64)
            != expected
        {
            errors.push(format!(
                "corpus-semantic-regression:carrier:{COMPARISON}#{SHADOW_KEY}.source_evidence.s12_named_denominators.{name}"
            ));
        }
    }
    errors
}

fn all_errors(comparison: &Value) -> Vec<String> {
    let mut errors = corpus_semantic_regression_errors(comparison);
    if let Some(block) = block(comparison) {
        errors.extend(carrier_errors(block, &yaml(AUTHORITY)));
        errors.extend(parent_carrier_errors(comparison));
    }
    errors
}

fn mutate(mut value: Value, path: &[&str], replacement: Value) -> Value {
    let mut cursor = &mut value;
    for key in &path[..path.len() - 1] {
        cursor = cursor.get_mut(*key).unwrap();
    }
    cursor
        .as_mapping_mut()
        .unwrap()
        .insert(Value::String(path[path.len() - 1].into()), replacement);
    value
}

fn rejects(value: &Value, marker: &str) {
    let errors = all_errors(value);
    assert!(
        errors.iter().any(|error| error.contains(marker)),
        "mutation `{marker}` was accepted: {errors:?}"
    );
}

#[test]
fn corpus_semantic_regression_block_is_frozen_and_bound_to_the_composer() {
    let comparison = yaml(COMPARISON);
    let errors = corpus_semantic_regression_errors(&comparison);
    assert!(
        errors.is_empty(),
        "corpus semantic regression DATA errors:\n{}",
        errors.join("\n")
    );
    let carrier = all_errors(&comparison);
    assert!(
        carrier.is_empty(),
        "corpus semantic regression carrier errors:\n{}",
        carrier.join("\n")
    );

    // The block is DATA-bound to the live vocabulary, not a second literal copy.
    assert_eq!(
        SemanticFieldLoss::ALL.len(),
        5,
        "the production loss vocabulary changed shape"
    );
    let measured: Vec<SemanticFieldLoss> = SemanticFieldLoss::ALL
        .iter()
        .copied()
        .filter(|class| *class != SemanticFieldLoss::CitationRole)
        .collect();
    assert_eq!(
        measured,
        MEASURED_LOSS_CLASSES.to_vec(),
        "the directly measured loss classes must be ALL minus CitationRole"
    );
    assert!(
        SemanticFieldLoss::ALL.contains(&SemanticFieldLoss::CitationRole),
        "CitationRole stays in the refusal vocabulary"
    );
    let comparison_block = block(&comparison).expect("block");
    assert_eq!(
        seq(comparison_block, &["class_order"]),
        expected_class_order(),
        "the frozen class order must be the live composer vocabulary"
    );
    assert_eq!(
        seq(comparison_block, &["refusal_classes"]),
        SemanticFieldLoss::ALL
            .iter()
            .map(|class| class.token().to_string())
            .collect::<Vec<_>>(),
        "if the SemanticFieldLoss enum gains a class the DATA block must carry it"
    );

    // A whole-block absence must be reported as absence, not drift.
    let missing = corpus_semantic_regression_errors(&Value::Null);
    assert!(
        missing
            .iter()
            .any(|error| error.contains("corpus-semantic-regression:missing")),
        "a missing block must be reported as missing: {missing:?}"
    );
}

#[test]
fn corpus_semantic_regression_is_registered_only_through_the_sanctioned_block() {
    let comparison = yaml(COMPARISON);
    // Exactly one top-level shadow key, and the block is a CHILD of it: never a
    // new top-level sibling and never a new `data/parse/*` carrier (D377).
    let top = comparison.as_mapping().expect("comparison mapping");
    let shadow_count = top
        .keys()
        .filter(|key| key.as_str() == Some(SHADOW_KEY))
        .count();
    assert_eq!(
        shadow_count, 1,
        "the composition shadow must stay one top-level key"
    );
    assert!(
        comparison.get(BLOCK_KEY).is_none(),
        "the corpus block must nest inside the sanctioned shadow block"
    );
    for entry in std::fs::read_dir(root().join("data/parse")).expect("data/parse is readable") {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .to_string();
        assert!(
            !name.contains("corpus"),
            "the corpus regression must not create a parallel data/parse carrier: {name}"
        );
    }

    // The governor digest pins must be re-pinned to the current carrier bytes.
    let on_disk = std::fs::read(root().join(COMPARISON)).expect("comparison contract is readable");
    let digest = sha256_hex(&on_disk);
    let registry = yaml(REGISTRY);
    let bindings = registry
        .get("bindings")
        .and_then(|bindings| bindings.get("source_data"))
        .and_then(Value::as_sequence)
        .expect("registry source_data");
    let ours: Vec<&Value> = bindings
        .iter()
        .filter(|binding| binding.get("path").and_then(Value::as_str) == Some(COMPARISON))
        .collect();
    assert_eq!(
        ours.len(),
        1,
        "the comparison contract must carry exactly one governor registry binding"
    );
    assert_eq!(
        ours[0].get("sha256").and_then(Value::as_str),
        Some(digest.as_str()),
        "the governor registry digest must be re-pinned to the current comparison contract"
    );

    let fixture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root().join(POSITIVE_FIXTURE)).expect("fixture"))
            .expect("fixture parses");
    let fixture_binding = fixture
        .get("source_data")
        .and_then(serde_json::Value::as_array)
        .and_then(|bindings| {
            bindings
                .iter()
                .find(|binding| binding.get("path").and_then(|p| p.as_str()) == Some(COMPARISON))
        })
        .expect("positive fixture binding");
    assert_eq!(
        fixture_binding
            .get("sha256")
            .and_then(|value| value.as_str()),
        Some(digest.as_str()),
        "the governor positive fixture must mirror the re-pinned digest"
    );
}

#[test]
fn corpus_semantic_regression_mutations_fail_closed_by_name() {
    let base = yaml(COMPARISON);

    // 1. a removed document
    let mut removed = base.clone();
    removed[SHADOW_KEY][BLOCK_KEY]["documents"]
        .as_sequence_mut()
        .expect("documents sequence")
        .pop();
    rejects(&removed, "corpus-semantic-regression:documents");

    // 2. a substituted document sha256
    let mut substituted = base.clone();
    substituted[SHADOW_KEY][BLOCK_KEY]["documents"][0]["sha256"] = Value::String("0".repeat(64));
    rejects(&substituted, "corpus-semantic-regression:document_sha256");

    // 3. identity_kind -> synthetic_clone
    rejects(
        &mutate(
            base.clone(),
            &[SHADOW_KEY, BLOCK_KEY, "identity_kind"],
            Value::String("synthetic_clone".into()),
        ),
        "corpus-semantic-regression:identity_kind",
    );

    // 4. unique_source_sha256 -> 1
    rejects(
        &mutate(
            base.clone(),
            &[SHADOW_KEY, BLOCK_KEY, "unique_source_sha256"],
            Value::Number(1.into()),
        ),
        "corpus-semantic-regression:unique_source_sha256",
    );

    // 5. pooling -> allowed
    rejects(
        &mutate(
            base.clone(),
            &[SHADOW_KEY, BLOCK_KEY, "pooling"],
            Value::String("allowed".into()),
        ),
        "corpus-semantic-regression:pooling",
    );

    // 6. a substituted metric (f1 no longer matches the frozen counters)
    rejects(
        &mutate(
            base.clone(),
            &[SHADOW_KEY, BLOCK_KEY, "classes", EXTRA_CLASS, "f1"],
            serde_yaml::from_str("0.5").expect("float literal"),
        ),
        "corpus-semantic-regression:classes.AdmittedAuthorLine",
    );

    // 7. a trimmed refusal vocabulary
    let mut trimmed = base.clone();
    trimmed[SHADOW_KEY][BLOCK_KEY]["refusal_classes"]
        .as_sequence_mut()
        .expect("refusal sequence")
        .pop();
    rejects(&trimmed, "corpus-semantic-regression:refusal_classes");

    // 8. denominators.documents -> 2
    rejects(
        &mutate(
            base.clone(),
            &[SHADOW_KEY, BLOCK_KEY, "denominators", "documents"],
            Value::Number(2.into()),
        ),
        "corpus-semantic-regression:denominators.documents",
    );

    // 9. the whole block -> Value::Null
    rejects(
        &mutate(base.clone(), &[SHADOW_KEY, BLOCK_KEY], Value::Null),
        "corpus-semantic-regression:missing",
    );

    // 10. DATA↔DATA: a carrier denominator drifts from the authority
    rejects(
        &mutate(
            base.clone(),
            &[SHADOW_KEY, BLOCK_KEY, "denominators", "references_total"],
            Value::Number(206.into()),
        ),
        "corpus-semantic-regression:carrier",
    );

    // 11. a dropped class from the frozen order
    let mut order = base.clone();
    order[SHADOW_KEY][BLOCK_KEY]["class_order"]
        .as_sequence_mut()
        .expect("class order sequence")
        .pop();
    rejects(&order, "corpus-semantic-regression:class_order");

    // 12. an emptied mutation matrix
    rejects(
        &mutate(
            base.clone(),
            &[SHADOW_KEY, BLOCK_KEY, "mutation_matrix"],
            Value::Sequence(Vec::new()),
        ),
        "corpus-semantic-regression:mutation_matrix",
    );

    // 13. a drifted used_by list
    rejects(
        &mutate(
            base.clone(),
            &[SHADOW_KEY, BLOCK_KEY, "used_by"],
            Value::Sequence(vec![Value::String("S05-T99".into())]),
        ),
        "corpus-semantic-regression:used_by",
    );

    // 14. an eroded lock
    rejects(
        &mutate(
            base.clone(),
            &[SHADOW_KEY, BLOCK_KEY, "locks", "import_eligible"],
            Value::Bool(true),
        ),
        "corpus-semantic-regression:locks.import_eligible",
    );
}
