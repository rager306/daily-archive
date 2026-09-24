#![cfg(feature = "pdf-layout")]

//! S05/T04 — the runtime `preserved_three_sha` corpus artifact is frozen,
//! non-vacuous, reproducible and redacted (H18).
//!
//! The structural, independent oracle for this corpus is the dev-only
//! `da-parse` test `native_fulltext_corpus_semantics.rs` (S05/T01) and its DATA
//! carrier `data/parse/comparison_contract.yaml#native_fulltext_composition_shadow.corpus_semantic_regression`
//! (S05/T02).  This test owns the *runtime* half: the example
//! `examples/native_fulltext_corpus_shadow.rs` reads the same three sha256-pinned
//! preserved TEI, fails FATAL on drift/absence/non-UTF-8, composes each document
//! and emits `artifacts/parse_compat/native_fulltext_corpus_shadow.json`.
//!
//! The artifact is the same-n regression surface the governor and H18 need, so
//! the audit below pins the identity (`preserved_three_sha`, three distinct
//! sha256), the GAR-006 population split (reference population identity-only
//! with its own document count and per-class source_population/admitted, the
//! executed population with `corpus_execution`/`execution_evidence`, the
//! evaluated population with the measured `status`), the non-vacuity of the
//! metric (TP = 573, P/R/F1 = 1.0), the policy locks and the redaction.
//! Denominator erosion, identity collapse, a fabricated band, a disguised typed
//! refusal, a collapsed population split, a reintroduced legacy `denominators`
//! block, an unlocked policy or a leaked payload are all rejected by the same
//! audit, which the negative test proves one mutation at a time.
//!
//! `cargo test --test NAME` does not build examples, so when the runner is
//! missing this test builds it through cargo with the same target directory
//! (cargo releases the build lock while tests run, so this cannot deadlock).

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

/// The committed runtime artifact under test.
const ARTIFACT: &str = "artifacts/parse_compat/native_fulltext_corpus_shadow.json";
const SCHEMA_VERSION: &str = "native-fulltext-corpus-shadow.v1";
const IDENTITY_KIND: &str = "preserved_three_sha";
const FALLBACK_TARGET: &str = "java-grobid-http@0.9.0-crf";
/// GAR-006 split literals the emitted report must carry (T07).  The evidence
/// token's kind is a member of the governor's declared `evidence_kinds`
/// vocabulary; the governor owns that vocabulary, so this pins the exact literal
/// the runner emits rather than duplicating the list.
const REFERENCE_POPULATION_ROLE: &str = "identity_only";
const EXECUTION_EVIDENCE: &str = "tracked_offline_fixture:preserved_three_sha_tei";
const EVALUATED_STATUS: &str = "evaluated";
/// The preserved corpus is exactly three documents with three distinct sha256.
const DOCUMENTS: usize = 3;
/// Frozen per-class denominators: `(class, source_population, admitted)`,
/// measured by S05/T01 and frozen in the DATA carrier by S05/T02.
const CLASS_DENOMINATORS: [(&str, usize, usize); 10] = [
    ("AdmittedAuthorLine", 36, 35),
    ("SemanticFieldLoss::HeaderTitle", 6, 3),
    ("SemanticFieldLoss::HeaderEmail", 3, 3),
    ("SemanticFieldLoss::HeaderIds", 3, 3),
    ("SemanticFieldLoss::AuthorAffiliation", 13, 13),
    ("source_listbibl_direct_children", 207, 207),
    ("CitationFieldRole::doi", 60, 60),
    ("CitationFieldRole::arxiv_id", 35, 35),
    ("CitationFieldRole::url", 148, 148),
    ("CitationFieldRole::article_title", 66, 66),
];
/// Frozen micro-occurrence aggregate: `(tp, fp, fn)`.
const AGGREGATE: (usize, usize, usize) = (573, 0, 0);
/// The twelve mutations S05/T03 proves are detected, by id.
const MUTATION_IDS: [&str; 12] = [
    "collapse_identity_to_one_sha",
    "drop_admitted_author_affiliation",
    "drop_admitted_header_email",
    "drop_admitted_header_ids",
    "drop_admitted_header_title",
    "inject_fabricated_band_note_group",
    "inject_fabricated_header_title",
    "raw_ampersand_xml",
    "substitute_byte_scope",
    "substitute_citation_field_role",
    "truncate_admitted_references",
    "unbalanced_xml",
];
/// The two H18-critical detectors, pinned exactly: identity erosion and byte
/// accounting are the failure modes this runner exists to observe.
const CRITICAL_DETECTORS: [(&str, &str); 2] = [
    (
        "collapse_identity_to_one_sha",
        "identity_failure:unique_source_sha256_not_three",
    ),
    (
        "substitute_byte_scope",
        "byte_accounting:ByteAccountingViolation::NativePartitionDisagreesWithLedger",
    ),
];
/// Every redaction flag the artifact must carry, all `false`.
const REDACTION_FLAGS: [&str; 15] = [
    "raw_tei",
    "raw_pdf",
    "raw_text",
    "field_values",
    "names",
    "email_addresses",
    "captions",
    "references",
    "physical_paths",
    "hostname",
    "executable",
    "secrets",
    "pdf_bytes",
    "graph_payload",
    "fulltext",
];
/// Payload keys that may only ever appear as redaction *flags*.
const FORBIDDEN_KEYS: [&str; 12] = [
    "raw_tei",
    "tei_xml",
    "pdf_bytes",
    "body_text",
    "text",
    "captions",
    "references",
    "physical_path",
    "path",
    "hostname",
    "executable",
    "secret",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn artifact_path() -> PathBuf {
    repo_root().join(ARTIFACT)
}

fn load_committed_artifact() -> Value {
    let path = artifact_path();
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "the canonical artifact {} must be regenerated by `cargo run -p da-cli --features pdf-layout --example native_fulltext_corpus_shadow -- --report {ARTIFACT}` ({error})",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("the canonical artifact is valid JSON")
}

fn example_binary() -> PathBuf {
    let binary = repo_root().join("target/debug/examples/native_fulltext_corpus_shadow");
    if binary.is_file() {
        return binary;
    }
    let build = Command::new("cargo")
        .args([
            "build",
            "-q",
            "-p",
            "da-cli",
            "--features",
            "pdf-layout",
            "--example",
            "native_fulltext_corpus_shadow",
        ])
        .current_dir(repo_root())
        .output()
        .expect("build feature-gated corpus shadow example");
    assert!(
        build.status.success(),
        "example build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    binary
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn collect_strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::String(text) => out.push(text),
        Value::Array(items) => items.iter().for_each(|item| collect_strings(item, out)),
        Value::Object(map) => map.values().for_each(|child| collect_strings(child, out)),
        _ => {}
    }
}

/// Payload keys may only appear as redaction flags.
fn raw_key_errors(report: &Value) -> Vec<String> {
    fn visit(value: &Value, inside_redaction: bool, errors: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let in_redaction = inside_redaction || key == "redaction";
                    if !in_redaction && FORBIDDEN_KEYS.contains(&key.as_str()) {
                        errors.push(format!("corpus-shadow:raw_payload.key:{key}"));
                    }
                    visit(child, in_redaction, errors);
                }
            }
            Value::Array(items) => items
                .iter()
                .for_each(|item| visit(item, inside_redaction, errors)),
            _ => {}
        }
    }
    let mut errors = Vec::new();
    visit(report, false, &mut errors);
    errors
}

/// Every reason the runtime `preserved_three_sha` artifact is unacceptable.
///
/// The audit is a pure predicate over the parsed report so the negative test can
/// prove it rejects one violation at a time without touching a fixture.
fn corpus_shadow_artifact_errors(report: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    if report["schema_version"].as_str() != Some(SCHEMA_VERSION) {
        errors.push("corpus-shadow:schema_version".into());
    }
    if report["corpus_id"].as_str() != Some(IDENTITY_KIND) {
        errors.push("corpus-shadow:corpus_id".into());
    }

    // ── identity: three distinct sha256, pinned to the real corpus ──────────
    let identity = &report["source_identity"];
    if identity["identity_kind"].as_str() != Some(IDENTITY_KIND) {
        errors.push("corpus-shadow:identity.identity_kind".into());
    }
    if identity["unique_source_sha256"].as_u64() != Some(DOCUMENTS as u64) {
        errors.push("corpus-shadow:identity.unique_source_sha256".into());
    }
    if identity["fallback_target"].as_str() != Some(FALLBACK_TARGET) {
        errors.push("corpus-shadow:identity.fallback_target".into());
    }
    for key in ["profile_fingerprint", "config_fingerprint"] {
        if identity[key].as_str().map(str::is_empty).unwrap_or(true) {
            errors.push(format!("corpus-shadow:identity.{key}"));
        }
    }
    let identity_shas: Vec<String> = identity["source_sha256"]
        .as_array()
        .map(|shas| {
            shas.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if identity_shas.len() != DOCUMENTS {
        errors.push("corpus-shadow:identity.source_sha256.cardinality".into());
    }
    if identity_shas.iter().any(|sha| !is_sha256(sha)) {
        errors.push("corpus-shadow:identity.source_sha256.shape".into());
    }
    if identity_shas.iter().collect::<BTreeSet<_>>().len() != identity_shas.len() {
        errors.push("corpus-shadow:identity.source_sha256.distinct".into());
    }

    // ── reference population: the same-n surface (GAR-006 identity-only) ────
    let reference = &report["reference_population"];
    if reference["role"].as_str() != Some(REFERENCE_POPULATION_ROLE) {
        errors.push("corpus-shadow:reference_population.role".into());
    }
    if reference["identity"]
        .as_str()
        .map(str::is_empty)
        .unwrap_or(true)
    {
        errors.push("corpus-shadow:reference_population.identity".into());
    }
    if reference["documents"].as_u64() != Some(DOCUMENTS as u64) {
        errors.push("corpus-shadow:reference_population.documents".into());
    }
    if reference["unique_source_sha256"].as_u64() != Some(DOCUMENTS as u64) {
        errors.push("corpus-shadow:reference_population.unique_source_sha256".into());
    }
    let per_class = &reference["per_class"];
    let declared_classes: BTreeSet<&str> = per_class
        .as_object()
        .map(|classes| classes.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let expected_classes: BTreeSet<&str> = CLASS_DENOMINATORS
        .iter()
        .map(|(class, _, _)| *class)
        .collect();
    if declared_classes != expected_classes {
        errors.push("corpus-shadow:reference_population.per_class.scope".into());
    }
    for (class, source_population, admitted) in CLASS_DENOMINATORS {
        let row = &per_class[class];
        let measured_population = row["source_population"].as_u64();
        let measured_admitted = row["admitted"].as_u64();
        if measured_population != Some(source_population as u64) {
            errors.push(format!(
                "corpus-shadow:reference_population.{class}.source_population"
            ));
        }
        if measured_admitted != Some(admitted as u64) {
            errors.push(format!(
                "corpus-shadow:reference_population.{class}.admitted"
            ));
        }
        if let (Some(population), Some(admitted)) = (measured_population, measured_admitted)
            && admitted > population
        {
            errors.push(format!(
                "corpus-shadow:reference_population.{class}.erosion"
            ));
        }
    }

    // ── executed population: what ran, and the evidence that produced it ────
    let executed = &report["executed_population"];
    if executed["corpus_execution"].as_bool() != Some(true) {
        errors.push("corpus-shadow:executed_population.corpus_execution".into());
    }
    if executed["execution_evidence"].as_str() != Some(EXECUTION_EVIDENCE) {
        errors.push("corpus-shadow:executed_population.execution_evidence".into());
    }
    if executed["documents"].as_u64() != Some(DOCUMENTS as u64) {
        errors.push("corpus-shadow:executed_population.documents".into());
    }
    if executed["accepted"].as_u64() != Some(DOCUMENTS as u64) {
        errors.push("corpus-shadow:executed_population.accepted".into());
    }

    // ── GAR-006: the split is real, and the legacy blocks are gone ──────────
    for legacy in ["denominators", "corpus"] {
        if report.get(legacy).is_some() {
            errors.push(format!("corpus-shadow:legacy_blocks.{legacy}"));
        }
    }
    let evaluated = &report["evaluated_population"];
    if reference == executed || reference == evaluated || executed == evaluated {
        errors.push("corpus-shadow:populations.indistinguishable".into());
    }

    // ── per-document outcomes: all accepted, none refused ───────────────────
    let outcomes = report["outcomes"].as_array().cloned().unwrap_or_default();
    if outcomes.len() != DOCUMENTS {
        errors.push("corpus-shadow:outcomes.cardinality".into());
    }
    let mut admitted_by_class: BTreeMap<&str, usize> = CLASS_DENOMINATORS
        .iter()
        .map(|(class, _, _)| (*class, 0))
        .collect();
    let mut conserved_total = 0usize;
    for (index, row) in outcomes.iter().enumerate() {
        let sha = row["source_sha256"].as_str().unwrap_or_default();
        if !is_sha256(sha) {
            errors.push(format!("corpus-shadow:outcomes[{index}].source_sha256"));
        }
        if row["document_id"]
            .as_str()
            .map(str::is_empty)
            .unwrap_or(true)
        {
            errors.push(format!("corpus-shadow:outcomes[{index}].document_id"));
        }
        if row["disposition"].as_str() != Some("accepted") {
            errors.push(format!("corpus-shadow:outcomes[{index}].disposition"));
        }
        for key in ["profile_fingerprint", "config_fingerprint"] {
            if row[key] != identity[key] {
                errors.push(format!("corpus-shadow:outcomes[{index}].{key}"));
            }
        }
        let classes = match row["classes"].as_array() {
            Some(classes) => classes,
            None => {
                errors.push(format!("corpus-shadow:outcomes[{index}].classes"));
                continue;
            }
        };
        let mut row_classes: BTreeSet<&str> = BTreeSet::new();
        for class_row in classes {
            let class = class_row["class"].as_str().unwrap_or_default();
            if !row_classes.insert(class) {
                errors.push(format!("corpus-shadow:outcomes[{index}].class.duplicate"));
            }
            if !expected_classes.contains(class) {
                errors.push(format!(
                    "corpus-shadow:outcomes[{index}].class.scope:{class}"
                ));
                continue;
            }
            let admitted = class_row["admitted"].as_u64().unwrap_or(0) as usize;
            let conserved = class_row["conserved"].as_u64().unwrap_or(0) as usize;
            if class_row["admitted"].as_u64() != class_row["conserved"].as_u64() {
                errors.push(format!(
                    "corpus-shadow:outcomes[{index}].class.{class}.conservation"
                ));
            }
            if class_row["fabricated"].as_u64() != Some(0) {
                errors.push(format!(
                    "corpus-shadow:outcomes[{index}].class.{class}.fabricated"
                ));
            }
            *admitted_by_class.entry(class).or_insert(0) += admitted;
            conserved_total += conserved;
        }
        if row_classes != expected_classes {
            errors.push(format!("corpus-shadow:outcomes[{index}].classes.scope"));
        }
    }
    // The three rows must carry three distinct sha256, and they must be the same
    // three the identity block names: a duplicated row would make the corpus look
    // larger than it is.
    let outcome_sha_set: BTreeSet<&str> = outcomes
        .iter()
        .filter_map(|row| row["source_sha256"].as_str())
        .collect();
    if outcome_sha_set.len() != outcomes.len() {
        errors.push("corpus-shadow:outcomes.source_sha256.distinct".into());
    }
    let identity_sha_set: BTreeSet<&str> = identity_shas.iter().map(String::as_str).collect();
    if outcome_sha_set != identity_sha_set {
        errors.push("corpus-shadow:outcomes.source_sha256.identity".into());
    }
    // The rows must re-derive the frozen per-class denominators, so a
    // document-level drift cannot leave the corpus table looking intact.
    for (class, _, admitted) in CLASS_DENOMINATORS {
        if admitted_by_class.get(class).copied() != Some(admitted) {
            errors.push(format!("corpus-shadow:outcomes.admitted_sum:{class}"));
        }
    }

    // ── evaluated population: the measured quality claim, kept separate ─────
    if evaluated["status"].as_str() != Some(EVALUATED_STATUS) {
        errors.push("corpus-shadow:evaluated_population.status".into());
    }
    if evaluated["documents"].as_u64() != Some(DOCUMENTS as u64) {
        errors.push("corpus-shadow:evaluated_population.documents".into());
    }
    let aggregate = &evaluated["aggregate"];
    let tp = aggregate["tp"].as_u64();
    if tp != Some(AGGREGATE.0 as u64) {
        errors.push("corpus-shadow:aggregate.tp".into());
    }
    if aggregate["fp"].as_u64() != Some(AGGREGATE.1 as u64) {
        errors.push("corpus-shadow:aggregate.fp".into());
    }
    if aggregate["fn"].as_u64() != Some(AGGREGATE.2 as u64) {
        errors.push("corpus-shadow:aggregate.fn".into());
    }
    for key in ["precision", "recall", "f1"] {
        if aggregate[key].as_f64() != Some(1.0) {
            errors.push(format!("corpus-shadow:aggregate.{key}"));
        }
    }
    match aggregate["admission_coverage"].as_f64() {
        Some(coverage) if coverage > 0.0 && coverage <= 1.0 => {}
        _ => errors.push("corpus-shadow:aggregate.admission_coverage".into()),
    }
    if conserved_total != AGGREGATE.0 {
        errors.push("corpus-shadow:aggregate.conserved_sum".into());
    }
    if tp != Some(conserved_total as u64) {
        errors.push("corpus-shadow:aggregate.tp_consistency".into());
    }

    // ── the twelve S05/T03 mutations are declared, with their detectors ─────
    let summary = report["mutation_summary"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let declared_mutations: BTreeSet<&str> = summary.keys().map(String::as_str).collect();
    let expected_mutations: BTreeSet<&str> = MUTATION_IDS.iter().copied().collect();
    if declared_mutations != expected_mutations {
        errors.push("corpus-shadow:mutation_summary.scope".into());
    }
    for (id, detector) in &summary {
        if detector.as_str().map(str::is_empty).unwrap_or(true) {
            errors.push(format!("corpus-shadow:mutation_summary.{id}"));
        }
    }
    for (id, expected) in CRITICAL_DETECTORS {
        if report["mutation_summary"][id].as_str() != Some(expected) {
            errors.push(format!("corpus-shadow:mutation_summary.{id}.detector"));
        }
    }

    // ── policy locks: shadow only, D127 closed ──────────────────────────────
    let policy = &report["policy"];
    for (key, expected) in [
        ("production_route", "forbidden"),
        ("graph_mutation", "forbidden"),
        ("auto_promote", "forbidden"),
        ("pooled_winner", "forbidden"),
    ] {
        if policy[key].as_str() != Some(expected) {
            errors.push(format!("corpus-shadow:policy.{key}"));
        }
    }
    if policy["import_eligible"].as_bool() != Some(false) {
        errors.push("corpus-shadow:policy.import_eligible".into());
    }
    if policy["deployed"].as_bool() != Some(false) {
        errors.push("corpus-shadow:policy.deployed".into());
    }

    // ── redaction: declared flags only, all false, no payload anywhere ──────
    let redaction = &report["redaction"];
    let declared_flags: BTreeSet<&str> = redaction
        .as_object()
        .map(|flags| flags.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let expected_flags: BTreeSet<&str> = REDACTION_FLAGS.iter().copied().collect();
    if declared_flags != expected_flags {
        errors.push("corpus-shadow:redaction.scope".into());
    }
    if let Some(flags) = redaction.as_object() {
        for (flag, value) in flags {
            if value.as_bool() != Some(false) {
                errors.push(format!("corpus-shadow:redaction.{flag}"));
            }
        }
    }
    errors.extend(raw_key_errors(report));
    if serde_json::to_string(report)
        .unwrap_or_default()
        .contains('<')
    {
        errors.push("corpus-shadow:raw_payload.xml".into());
    }
    let mut strings = Vec::new();
    collect_strings(report, &mut strings);
    for text in strings.into_iter().filter(|text| text.contains('@')) {
        // The pinned fallback target spells an `@`; anything else is a leaked
        // address or a raw value.
        if text != FALLBACK_TARGET {
            errors.push("corpus-shadow:raw_payload.email".into());
        }
    }

    errors
}

#[test]
fn committed_artifact_is_a_frozen_preserved_three_sha_measurement() {
    let report = load_committed_artifact();
    let errors = corpus_shadow_artifact_errors(&report);
    assert!(
        errors.is_empty(),
        "the committed {} failed its own audit: {errors:#?}",
        artifact_path().display()
    );
    // Non-vacuity, stated once outside the audit: a zero-TP artifact would make
    // every same-n claim above meaningless.
    assert!(
        report["evaluated_population"]["aggregate"]["tp"]
            .as_u64()
            .unwrap_or(0)
            > 0,
        "the corpus metric must be non-vacuous"
    );
}

#[test]
fn artifact_audit_rejects_every_declared_violation() {
    let valid = load_committed_artifact();
    assert!(
        corpus_shadow_artifact_errors(&valid).is_empty(),
        "the unmutated artifact must pass the audit"
    );

    let first_identity_sha = valid["source_identity"]["source_sha256"][0].clone();
    let first_outcome_sha = valid["outcomes"][0]["source_sha256"].clone();

    let mut mutations: Vec<(&str, Value)> = Vec::new();

    let mut synthetic = valid.clone();
    synthetic["source_identity"]["identity_kind"] = json!("synthetic_clone");
    mutations.push(("identity_kind_is_not_preserved", synthetic));

    let mut collapsed = valid.clone();
    collapsed["source_identity"]["source_sha256"] = json!([first_identity_sha]);
    mutations.push(("identity_sha_collapsed_to_one", collapsed));

    let mut duplicated = valid.clone();
    duplicated["outcomes"][1]["source_sha256"] = first_outcome_sha;
    mutations.push(("outcome_sha_duplicated", duplicated));

    let mut eroded = valid.clone();
    eroded["reference_population"]["per_class"]["source_listbibl_direct_children"]["source_population"] =
        json!(100);
    mutations.push(("denominator_erosion", eroded));

    let mut lost_document = valid.clone();
    lost_document["reference_population"]["documents"] = json!(1);
    mutations.push(("document_denominator_lost", lost_document));

    let mut legacy_block = valid.clone();
    legacy_block["denominators"] = json!({"documents": DOCUMENTS});
    mutations.push(("legacy_denominators_block_reintroduced", legacy_block));

    let mut legacy_corpus = valid.clone();
    legacy_corpus["corpus"] = json!({"documents": DOCUMENTS});
    mutations.push(("legacy_corpus_block_reintroduced", legacy_corpus));

    let mut missing_split = valid.clone();
    missing_split
        .as_object_mut()
        .expect("report object")
        .remove("executed_population");
    mutations.push(("executed_population_removed", missing_split));

    let mut collapsed_split = valid.clone();
    collapsed_split["executed_population"] = valid["reference_population"].clone();
    mutations.push(("populations_collapsed_to_one_object", collapsed_split));

    let mut unlabelled_reference = valid.clone();
    unlabelled_reference["reference_population"]["role"] = json!("identity_and_execution");
    mutations.push(("reference_population_role_rewritten", unlabelled_reference));

    let mut denied_execution = valid.clone();
    denied_execution["executed_population"]["corpus_execution"] = json!(false);
    mutations.push(("corpus_execution_denied", denied_execution));

    let mut unbound_evidence = valid.clone();
    unbound_evidence["executed_population"]["execution_evidence"] = json!("n/a");
    mutations.push(("execution_evidence_unbound", unbound_evidence));

    let mut unstated_status = valid.clone();
    unstated_status["evaluated_population"]["status"] = json!("");
    mutations.push(("evaluated_status_removed", unstated_status));

    let mut vacuous = valid.clone();
    vacuous["evaluated_population"]["aggregate"]["tp"] = json!(0);
    mutations.push(("vacuous_metric", vacuous));

    let mut fabricated = valid.clone();
    fabricated["evaluated_population"]["aggregate"]["fp"] = json!(7);
    mutations.push(("fabricated_band_admitted", fabricated));

    let mut refused = valid.clone();
    refused["outcomes"][0]["disposition"] = json!("rejected:MalformedXml");
    mutations.push(("typed_refusal_disguised_as_accepted", refused));

    let mut dropped_class = valid.clone();
    dropped_class["outcomes"][0]["classes"]
        .as_array_mut()
        .expect("class rows")
        .pop();
    mutations.push(("outcome_class_row_dropped", dropped_class));

    let mut unknown_scope = valid.clone();
    unknown_scope["reference_population"]["per_class"]["UnknownScope"] =
        json!({"source_population": 1, "admitted": 1});
    mutations.push(("unknown_scope_admitted", unknown_scope));

    let mut unlocked_import = valid.clone();
    unlocked_import["policy"]["import_eligible"] = json!(true);
    mutations.push(("import_lock_removed", unlocked_import));

    let mut unlocked_promotion = valid.clone();
    unlocked_promotion["policy"]["auto_promote"] = json!("allowed");
    mutations.push(("auto_promote_unlocked", unlocked_promotion));

    let mut unlocked_route = valid.clone();
    unlocked_route["policy"]["production_route"] = json!("allowed");
    mutations.push(("production_route_unlocked", unlocked_route));

    let mut raw_payload = valid.clone();
    raw_payload["outcomes"][0]["raw_tei"] = json!("<teiHeader/>");
    mutations.push(("raw_payload_injected", raw_payload));

    let mut leaked_email = valid.clone();
    leaked_email["outcomes"][0]["lift"]["contact"] = json!("someone@example.org");
    mutations.push(("email_injected", leaked_email));

    let mut redaction_lie = valid.clone();
    redaction_lie["redaction"]["raw_tei"] = json!(true);
    mutations.push(("redaction_flag_raised", redaction_lie));

    let mut redaction_scope = valid.clone();
    redaction_scope["redaction"]["tei_xml"] = json!(false);
    mutations.push(("redaction_scope_replaced", redaction_scope));

    let mut shrunk_matrix = valid.clone();
    shrunk_matrix["mutation_summary"]
        .as_object_mut()
        .expect("mutation summary")
        .remove("unbalanced_xml");
    mutations.push(("mutation_dictionary_shrunk", shrunk_matrix));

    let mut wrong_detector = valid.clone();
    wrong_detector["mutation_summary"]["collapse_identity_to_one_sha"] = json!("typed_refusal");
    mutations.push(("critical_detector_rewritten", wrong_detector));

    let mut bad_schema = valid.clone();
    bad_schema["schema_version"] = json!("native-fulltext-corpus-shadow.v0");
    mutations.push(("schema_version_rewritten", bad_schema));

    for (name, mutant) in mutations {
        let errors = corpus_shadow_artifact_errors(&mutant);
        assert!(
            !errors.is_empty(),
            "the audit accepted the mutation `{name}`"
        );
    }
}

#[test]
fn guard_self_check_rejects_every_declared_violation() {
    let output = Command::new(example_binary())
        .arg("--self-check")
        .current_dir(repo_root())
        .output()
        .expect("run the corpus shadow guard self-check");
    assert!(
        output.status.success(),
        "self-check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for guard in [
        "identity_unique_sha",
        "documents_and_accepted_rows",
        "zero_class_denominator",
        "unknown_scope",
        "aggregate_metric_mismatch",
        "data_carrier_agreement",
        "fixture_checksum_drift",
        "fixture_non_utf8",
    ] {
        assert!(
            stdout.contains(&format!("guard {guard}: rejected")),
            "the self-check did not prove `{guard}` rejects its violating input:\n{stdout}"
        );
    }
}

#[test]
fn committed_artifact_equals_two_independent_runs() {
    let binary = example_binary();
    let committed = fs::read(artifact_path()).expect("committed artifact");
    let mut streams: Vec<Vec<u8>> = Vec::new();
    for _ in 0..2 {
        // A different working directory proves the artifact is not a function of
        // the invoking cwd: the runner resolves the fixtures from its manifest.
        let dir = tempdir().expect("temporary run directory");
        let report = dir.path().join("report.json");
        let output = Command::new(&binary)
            .args(["--report", report.to_str().expect("report path")])
            .current_dir(dir.path())
            .output()
            .expect("run the corpus shadow example");
        assert!(
            output.status.success(),
            "runner failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        streams.push(fs::read(&report).expect("generated report bytes"));
    }
    assert_eq!(
        streams[0], streams[1],
        "two independent runs must be byte-identical"
    );
    assert_eq!(
        streams[0], committed,
        "the committed artifact must equal a fresh canonical run"
    );
}

#[test]
fn missing_report_fails_closed_without_writing_an_artifact() {
    let output = Command::new(example_binary())
        .current_dir(repo_root())
        .output()
        .expect("run the corpus shadow example without --report");
    assert!(
        !output.status.success(),
        "the canonical run must refuse to start without --report"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--report <path> is required for the canonical run"),
        "the refusal must name the missing argument:\n{stderr}"
    );
}

#[test]
fn unfiltered_class_rows_add_up_to_the_corpus_denominators() {
    // Guard against a silent `per_class` rewrite that keeps the aggregate but
    // changes the composition: every frozen denominator must be the exact sum of
    // the three document rows.
    let report = load_committed_artifact();
    let mut per_document: BTreeMap<&str, usize> = BTreeMap::new();
    for row in report["outcomes"].as_array().expect("outcomes") {
        for class_row in row["classes"].as_array().expect("class rows") {
            let class = class_row["class"].as_str().expect("class name");
            *per_document.entry(class).or_insert(0) +=
                class_row["admitted"].as_u64().expect("admitted") as usize;
        }
    }
    for (class, _, admitted) in CLASS_DENOMINATORS {
        assert_eq!(
            per_document.get(class).copied(),
            Some(admitted),
            "{class}: the document rows must sum to the frozen admitted denominator"
        );
    }
}
