//! DATA contract for the S13 same-input PDF layout bakeoff.
//! The oracle is deliberately independent of da-parse donor implementations:
//! it validates only redacted identities, bounded geometry and denominators.

use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST: &str = "data/parse/layout_bakeoff.yaml";
const ORACLE: &str = "crates/da-parse/tests/fixtures/layout_oracle.yaml";
const FORBIDDEN: &[&str] = &[
    "raw_text",
    "pdf_bytes",
    "tei_xml",
    "page_image",
    "physical_path",
    "body_text",
    "raw_document_content",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn yaml(path: &str) -> Value {
    let text = fs::read_to_string(repo_root().join(path)).expect("DATA file readable");
    serde_yaml::from_str(&text).expect("DATA file is valid YAML")
}

fn str_at<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    path.iter()
        .fold(value, |current, key| &current[*key])
        .as_str()
        .expect("DATA string")
}

fn sha256(path: &Path) -> String {
    let bytes = fs::read(path).expect("source PDF readable");
    hex::encode(Sha256::digest(bytes))
}

fn reject_forbidden(value: &Value, path: &str) {
    match value {
        Value::Mapping(map) => {
            for (key, child) in map {
                if let Some(key) = key.as_str() {
                    // The top-level `forbidden_fields` list is policy metadata;
                    // the same names are forbidden only as payload keys.
                    let is_policy_declaration = path == "oracle" && key == "forbidden_fields";
                    assert!(
                        is_policy_declaration || !FORBIDDEN.contains(&key),
                        "forbidden raw field `{key}` at {path}"
                    );
                    if !is_policy_declaration {
                        reject_forbidden(child, &format!("{path}.{key}"));
                    }
                }
            }
        }
        Value::Sequence(items) => {
            for (index, child) in items.iter().enumerate() {
                reject_forbidden(child, &format!("{path}[{index}]"));
            }
        }
        _ => {}
    }
}

#[test]
fn same_input_manifest_binds_three_pdf_sources_and_exact_pages() {
    let manifest = yaml(MANIFEST);
    assert_eq!(str_at(&manifest, &["schema_version"]), "layout-bakeoff.v1");
    assert_eq!(manifest["corpus"]["corpus_n"].as_u64(), Some(3));
    assert_eq!(manifest["corpus"]["page_denominator"].as_u64(), Some(202));
    assert_eq!(manifest["lanes"]["same_input"].as_bool(), Some(true));
    assert_eq!(
        manifest["lanes"]["all_lanes_required"].as_bool(),
        Some(true)
    );
    assert_eq!(
        manifest["acceptance"]["no_winner_encoded"].as_bool(),
        Some(true)
    );
    assert_eq!(
        manifest["acceptance"]["selected_state"].as_str(),
        Some("forbidden")
    );
    assert_eq!(
        manifest["acceptance"]["promoted_state"].as_str(),
        Some("forbidden")
    );

    let expected_lanes: BTreeSet<_> = [
        "pdfalto",
        "pdf_inspector_positioned",
        "pdf_inspector_positioned_xy_cut",
    ]
    .into_iter()
    .collect();
    let lanes: BTreeSet<_> = manifest["lanes"]["required"]
        .as_sequence()
        .expect("required lanes")
        .iter()
        .map(|v| v.as_str().expect("lane name"))
        .collect();
    assert_eq!(lanes, expected_lanes);

    let docs = manifest["corpus"]["documents"]
        .as_sequence()
        .expect("documents");
    let mut pages = 0;
    let mut ids = BTreeSet::new();
    for doc in docs {
        let id = doc["id"].as_str().expect("document id");
        assert!(ids.insert(id), "duplicate document identity: {id}");
        let path = repo_root().join(doc["source_pdf"].as_str().expect("source path"));
        assert!(
            path.is_file(),
            "missing source PDF for {id}: {}",
            path.display()
        );
        let expected = doc["sha256"].as_str().expect("source checksum");
        assert_eq!(expected.len(), 64, "checksum shape for {id}");
        assert_eq!(sha256(&path), expected, "checksum drift for {id}");
        let count = doc["pages"].as_u64().expect("page count");
        assert!(count > 0, "empty page denominator for {id}");
        pages += count;
        assert!(
            doc["page_classes"]
                .as_sequence()
                .is_some_and(|v| v.len() >= 6),
            "document {id} lacks required layout-risk classes"
        );
        assert!(doc["supporting_s07_case"].as_str().is_some());
    }
    assert_eq!(pages, 202, "manifest page denominator drift");
}

#[test]
fn independent_oracle_is_redacted_and_has_complete_page_denominators() {
    let manifest = yaml(MANIFEST);
    let oracle = yaml(ORACLE);
    reject_forbidden(&oracle, "oracle");
    assert_eq!(oracle["schema_version"].as_str(), Some("layout-oracle.v1"));
    assert_eq!(
        oracle["oracle_id"].as_str(),
        Some("independent-layout-receipts-v1")
    );
    assert_eq!(manifest["oracle"]["independent"].as_bool(), Some(true));
    assert_eq!(manifest["oracle"]["no_donor_helpers"].as_bool(), Some(true));
    assert_eq!(manifest["oracle"]["no_tei_lifter"].as_bool(), Some(true));

    let docs = manifest["corpus"]["documents"].as_sequence().unwrap();
    let receipts = oracle["receipts"].as_sequence().expect("oracle receipts");
    assert_eq!(
        receipts.len(),
        docs.len(),
        "one oracle receipt per document"
    );
    for doc in docs {
        let id = doc["id"].as_str().unwrap();
        let receipt = receipts
            .iter()
            .find(|r| r["document_id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("missing oracle receipt for {id}"));
        assert_eq!(receipt["source_sha256"], doc["sha256"]);
        assert_eq!(receipt["page_count"], doc["pages"]);
        let pages = receipt["page_ordinals"]
            .as_sequence()
            .expect("page ordinals");
        let expected: Vec<_> = (0..doc["pages"].as_u64().unwrap()).collect();
        let actual: Vec<_> = pages.iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!(actual, expected, "page ordinal loss for {id}");
        let tokens = receipt["token_receipts"].as_sequence().expect("token rows");
        let mut token_ordinals = BTreeSet::new();
        for token in tokens {
            assert!(token_ordinals.insert(token["token_ordinal"].as_u64().unwrap()));
            let bbox = token["bbox_72dpi_top_left"].as_sequence().unwrap();
            assert_eq!(bbox.len(), 4);
            let values: Vec<_> = bbox
                .iter()
                .map(|v| {
                    v.as_f64()
                        .or_else(|| v.as_u64().map(|n| n as f64))
                        .expect("finite bbox number")
                })
                .collect();
            assert!(values.iter().all(|v| v.is_finite()));
            assert!(
                values[0] >= 0.0
                    && values[1] >= 0.0
                    && values[2] > values[0]
                    && values[3] > values[1]
            );
            assert_eq!(token["token_digest"].as_str().unwrap().len(), 64);
            assert!(token["role"].as_str().is_some());
        }
    }
}

#[test]
fn failure_taxonomy_is_closed_and_no_selection_is_encoded() {
    let manifest = yaml(MANIFEST);
    let allowed: BTreeSet<_> = [
        "missing",
        "checksum_mismatch",
        "malformed",
        "unsupported",
        "timeout",
        "nonzero",
        "oversize",
        "encrypted",
        "truncated",
    ]
    .into_iter()
    .collect();
    let actual: BTreeSet<_> = manifest["lanes"]["unavailable_outcomes"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(actual, allowed);
    assert_eq!(
        manifest["acceptance"]["no_silent_skips"].as_bool(),
        Some(true)
    );
    assert_eq!(
        manifest["acceptance"]["promotion_state"].as_str(),
        Some("candidate")
    );
}

fn smp_contract_error(contract: &Value) -> Option<String> {
    let topology = &contract["topology"];
    if topology["source"].as_str()
        != Some("linux_allowed_cpu_set_intersected_with_sysfs_package_core")
        || topology["identity_key"].as_str() != Some("physical_package_id_and_core_id")
        || topology["unsupported_outcome"].as_str() != Some("topology_unavailable")
        || topology["logical_cpu_fallback"].as_str() != Some("forbidden")
        || topology["numa_claim"].as_str() != Some("forbidden")
    {
        return Some("physical topology provenance is not fail-closed".into());
    }

    let execution = &contract["execution"];
    let required_execution = [
        ("scope", "invocation_private"),
        ("worker_matrix", "1..=physical_core_cap"),
        ("requested_above_cap", "fail_before_spawn"),
        ("silent_clamp", "forbidden"),
        ("input_queue_capacity", "positive_integer_bounded"),
        ("result_queue_capacity", "positive_integer_bounded"),
        ("full_queue_behavior", "backpressure"),
        ("drop_on_full", "forbidden"),
        ("global_pool", "forbidden"),
        ("per_worker_scratch", "bounded_private_reusable"),
        ("unsafe_project_code", "forbidden"),
    ];
    for (key, expected) in required_execution {
        if execution[key].as_str() != Some(expected) {
            return Some(format!("execution.{key} must be {expected}"));
        }
    }

    let acceptance = &contract["denominators"]["acceptance"];
    if acceptance["corpus_n"].as_u64() != Some(3)
        || acceptance["lanes_n"].as_u64() != Some(3)
        || acceptance["pages_per_lane"].as_u64() != Some(202)
        || acceptance["total_lane_pages"].as_u64() != Some(606)
        || acceptance["fixed"].as_bool() != Some(true)
    {
        return Some("acceptance denominator drift".into());
    }
    let benchmark = &contract["denominators"]["benchmark"];
    if benchmark["repetitions"].as_str() != Some("positive_integer")
        || benchmark["separate_from_acceptance"].as_bool() != Some(true)
        || benchmark["pooled_with_acceptance"].as_str() != Some("forbidden")
    {
        return Some("benchmark denominator is pooled or unbounded".into());
    }

    let failures: BTreeSet<_> = contract["terminal_receipts"]["failure_vocabulary"]
        .as_sequence()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<BTreeSet<_>>>()?;
    let expected_failures: BTreeSet<_> = [
        "topology_unavailable",
        "invalid_config",
        "over_core_cap",
        "worker_panic",
        "channel_closed",
        "interrupted",
        "semantic_mismatch",
    ]
    .into_iter()
    .collect();
    if failures != expected_failures
        || contract["terminal_receipts"]["admitted_equals_terminal"].as_str() != Some("required")
        || contract["terminal_receipts"]["exactly_one_per_admitted_unit"].as_str()
            != Some("required")
    {
        return Some("terminal receipt or failure vocabulary is not closed".into());
    }

    let metrics = &contract["metrics"]["required"];
    let required_metrics = [
        "p50_ms",
        "p95_ms",
        "p99_ms",
        "throughput_units_per_second",
        "process_rss_bytes",
        "allocation_or_private_capacity_bytes",
        "queue_wait_ms",
    ];
    if !required_metrics.iter().all(|metric| {
        metrics
            .as_sequence()
            .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(metric)))
    }) || contract["metrics"]["per_worker_terminal_counts"].as_str() != Some("required")
        || contract["metrics"]["per_lane_metrics"].as_str() != Some("required")
    {
        return Some("required SMP metrics are incomplete".into());
    }

    let breakpoint_verdicts: BTreeSet<_> = contract["breakpoints"]["verdicts"]
        .as_sequence()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<BTreeSet<_>>>()?;
    let expected_breakpoints: BTreeSet<_> = [
        "not_observed",
        "saturation",
        "first_regression",
        "queue_pressure",
        "memory_pressure",
        "inconclusive",
    ]
    .into_iter()
    .collect();
    if breakpoint_verdicts != expected_breakpoints
        || contract["breakpoints"]["closed"].as_bool() != Some(true)
        || contract["breakpoints"]["data_defined"].as_bool() != Some(true)
    {
        return Some("breakpoint vocabulary is not closed".into());
    }

    let optimization = &contract["optimization"];
    let states: BTreeSet<_> = optimization["simd_state"]
        .as_sequence()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<BTreeSet<_>>>()?;
    let expected_states: BTreeSet<_> = ["not_applicable", "measured_rejected", "measured_retained"]
        .into_iter()
        .collect();
    if states != expected_states
        || optimization["simd_selection"].as_str() != Some("forbidden")
        || optimization["safe_rust_only"].as_bool() != Some(true)
        || optimization["scalar_oracle"].as_str() != Some("required")
    {
        return Some("optimization state is open or unsafe".into());
    }

    let redaction: BTreeSet<_> = contract["redaction"]["forbidden"]
        .as_sequence()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<BTreeSet<_>>>()?;
    let required_redactions: BTreeSet<_> = [
        "raw_pdf",
        "raw_text",
        "raw_tei",
        "images",
        "token_text",
        "executable",
        "physical_path",
        "hostname",
        "topology_path",
    ]
    .into_iter()
    .collect();
    if !required_redactions.is_subset(&redaction) {
        return Some("SMP report redaction is incomplete".into());
    }

    if contract["promotion"]["state"].as_str() != Some("candidate")
        || contract["promotion"]["selected"].as_str() != Some("forbidden")
        || contract["promotion"]["promoted"].as_str() != Some("forbidden")
        || contract["promotion"]["auto_promote"].as_str() != Some("forbidden")
        || contract["promotion"]["rollback_target"].as_str() != Some("java-grobid-http@0.9.0-crf")
    {
        return Some("promotion or rollback lock changed".into());
    }
    None
}

fn assert_smp_contract(contract: &Value) {
    if let Some(error) = smp_contract_error(contract) {
        panic!("invalid cpu_smp_candidate contract: {error}");
    }
}

#[test]
fn bounded_smp_contract_is_additive_and_fail_closed() {
    let manifest = yaml(MANIFEST);
    let candidate = &manifest["cpu_smp_candidate"];
    assert_smp_contract(candidate);
    assert_eq!(
        manifest["policy_locks"]["import_eligible"].as_bool(),
        Some(false)
    );
    assert_eq!(
        manifest["lanes"]["required"].as_sequence().unwrap().len(),
        3
    );
}

#[test]
fn bounded_smp_contract_rejects_security_and_denominator_mutations() {
    let baseline = yaml(MANIFEST)["cpu_smp_candidate"].clone();
    let mutations = [
        (
            "missing worker matrix",
            vec![("execution", "worker_matrix", Value::Null)],
        ),
        (
            "over-cap clamp",
            vec![(
                "execution",
                "requested_above_cap",
                Value::String("clamp".into()),
            )],
        ),
        (
            "drop-on-full",
            vec![("execution", "drop_on_full", Value::String("allowed".into()))],
        ),
        (
            "pooled denominators",
            vec![(
                "denominators",
                "benchmark",
                Value::Mapping(Default::default()),
            )],
        ),
        (
            "fake NUMA",
            vec![("topology", "numa_claim", Value::String("bound".into()))],
        ),
        (
            "global pool",
            vec![("execution", "global_pool", Value::String("allowed".into()))],
        ),
        (
            "unsafe project",
            vec![(
                "execution",
                "unsafe_project_code",
                Value::String("allowed".into()),
            )],
        ),
        (
            "selected",
            vec![("promotion", "selected", Value::String("allowed".into()))],
        ),
        (
            "token text leak",
            vec![(
                "redaction",
                "forbidden",
                Value::Sequence(vec![Value::String("raw_pdf".into())]),
            )],
        ),
    ];
    for (name, mutation) in mutations {
        let mut candidate = baseline.clone();
        for (section, key, value) in mutation {
            candidate[section][key] = value;
        }
        assert!(
            smp_contract_error(&candidate).is_some(),
            "mutation must fail: {name}"
        );
    }
}

#[test]
fn comparison_ledger_owns_the_same_smp_contract_without_route_promotion() {
    let ledger = yaml("data/parse/comparison_contract.yaml");
    let candidate = &ledger["cpu_smp_candidate"];
    assert_eq!(candidate["manifest_binding"].as_str(), Some(MANIFEST));
    assert_eq!(candidate["promotion"]["state"].as_str(), Some("candidate"));
    assert_eq!(
        candidate["promotion"]["selected"].as_str(),
        Some("forbidden")
    );
    assert_eq!(
        candidate["promotion"]["promoted"].as_str(),
        Some("forbidden")
    );
    assert_eq!(
        candidate["topology"]["unavailable_state"].as_str(),
        Some("topology_unavailable")
    );
    assert_eq!(
        candidate["execution"]["requested_above_cap"].as_str(),
        Some("fail_before_spawn")
    );
    assert_eq!(
        candidate["denominators"]["acceptance"]["total_lane_pages"].as_u64(),
        Some(606)
    );
    assert_eq!(
        candidate["optimization"]["selection"].as_str(),
        Some("forbidden")
    );
    for field in [
        "raw_pdf",
        "raw_text",
        "raw_tei",
        "images",
        "token_text",
        "executable",
        "physical_path",
        "hostname",
        "topology_path",
    ] {
        assert!(
            candidate["redaction"]["forbidden"]
                .as_sequence()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some(field)),
            "missing redaction {field}"
        );
    }
}

// ---------------------------------------------------------------------------
// S03/T03 (M514-1lss2s): the Java-free vertical slice declaration (ADR-077 §3)
// ---------------------------------------------------------------------------

/// The existing cascade DATA the slice stage ids resolve against. This carrier
/// is read only — the declaration must not restate it.
const COMPATIBILITY: &str = "data/parse/grobid_compatibility.yaml";

/// Resolve a dotted `a.b.c` pointer against a YAML value.
fn dotted<'a>(value: &'a Value, pointer: &str) -> Option<&'a Value> {
    let mut current = value;
    for key in pointer.split('.') {
        let Value::Mapping(map) = current else {
            return None;
        };
        current = map
            .iter()
            .find(|(candidate, _)| candidate.as_str() == Some(key))
            .map(|(_, child)| child)?;
    }
    Some(current)
}

fn stage_id_set(value: &Value) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    if let Some(stages) = value.as_sequence() {
        for stage in stages {
            if let Some(id) = stage["stage_id"].as_str() {
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

/// Fail-closed validation of the additive `java_free_vertical_slice` block.
/// Every defect is a named, typed error — nothing is silently skipped.
fn java_free_slice_errs(manifest: &Value, compatibility: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    let slice = &manifest["java_free_vertical_slice"];
    if slice.is_null() {
        return vec!["[java-free-slice-missing] declaration block absent".to_string()];
    }

    for (key, expected) in [
        ("schema_version", "java-free-vertical-slice.v1"),
        ("slice_id", "pdf-layout-p1-p2"),
        ("adr_ref", "ADR-077"),
        ("claim_scope", "diagnostic_only"),
        ("product_coverage_effect", "none"),
    ] {
        if slice[key].as_str() != Some(expected) {
            errs.push(format!(
                "[java-free-slice-schema] {key} must be `{expected}`"
            ));
        }
    }

    // Corpus identity is bound by path + pointer, never copied in. The pointer
    // names a corpus block's `.documents` list inside this manifest; neither the
    // block name nor the list segment is hardcoded, so the native route can
    // carry its own corpus without touching the layout donor corpus.
    let binding = &slice["corpus_binding"];
    let corpus_block_pointer = binding["pointer"]
        .as_str()
        .and_then(|pointer| pointer.strip_suffix(".documents"))
        .filter(|prefix| !prefix.is_empty() && !prefix.contains('/') && !prefix.contains('#'));
    let corpus_block = corpus_block_pointer.and_then(|prefix| dotted(manifest, prefix));
    let documents = corpus_block.and_then(|block| block["documents"].as_sequence());
    let document_count = binding["document_count"].as_u64();
    let page_denominator = corpus_block.and_then(|block| block["page_denominator"].as_u64());
    let corpus_n = corpus_block.and_then(|block| block["corpus_n"].as_u64());
    let page_sum: u64 = documents
        .map(|seq| {
            seq.iter()
                .filter_map(|document| document["pages"].as_u64())
                .sum()
        })
        .unwrap_or(0);
    if binding["manifest"].as_str() != Some(MANIFEST)
        || corpus_block_pointer.is_none()
        || documents.is_none()
        || document_count != documents.map(|seq| seq.len() as u64)
        || corpus_n != document_count
        || page_denominator != Some(page_sum)
        || binding["identity_rule"].as_str() != Some("exact_document_id_and_sha256")
    {
        errs.push(
            "[java-free-slice-corpus-binding] corpus identity must bind by path + pointer"
                .to_string(),
        );
    }
    if ["documents", "sha256", "ids", "document_ids"]
        .iter()
        .any(|key| !binding[*key].is_null())
    {
        errs.push(
            "[java-free-slice-corpus-binding] document identities must not be duplicated"
                .to_string(),
        );
    }

    // Stage ids resolve against the existing cascade DATA.
    let source = &slice["stage_id_source"];
    if source["manifest"].as_str() != Some(COMPATIBILITY)
        || source["pointer"].as_str() != Some("defaults.cascade_stages")
    {
        errs.push(
            "[java-free-slice-stage-source] stage ids must resolve from the cascade DATA"
                .to_string(),
        );
    }
    let declared: BTreeSet<String> = match &compatibility["defaults"]["cascade_stages"] {
        Value::Mapping(map) => map
            .keys()
            .filter_map(|key| key.as_str().map(str::to_string))
            .collect(),
        _ => BTreeSet::new(),
    };
    if declared.is_empty() {
        errs.push(
            "[java-free-slice-stage-source] cascade stage set is empty or unreadable".to_string(),
        );
        return errs;
    }

    let in_slice = &slice["stages_in_slice"];
    let out_slice = &slice["stages_out_of_slice"];
    let in_ids = stage_id_set(in_slice);
    let out_ids = stage_id_set(out_slice);

    if in_ids != BTreeSet::from(["P1".to_string(), "P2".to_string()]) {
        errs.push(
            "[java-free-slice-stage-partition] slice stages must be exactly P1 and P2".to_string(),
        );
    }
    if !in_ids.is_disjoint(&out_ids) {
        errs.push(
            "[java-free-slice-stage-partition] a stage is declared both in and out of the slice"
                .to_string(),
        );
    }
    // P0 is the identity/profile admission point: neither a slice lane nor a
    // counted out-of-slice stage. It is classified explicitly, so the ONLY
    // declared cascade stage outside in ∪ out is that one admission stage. A
    // new, unclassified cascade stage therefore fails loudly (no silent skip).
    let admission = &slice["identity_admission_stage"];
    let admission_id = admission["stage_id"].as_str().unwrap_or("");
    let admission_pointer = admission["name_pointer"].as_str().unwrap_or("");
    let admission_resolves = dotted(compatibility, admission_pointer)
        .and_then(Value::as_str)
        .is_some_and(|name| !name.is_empty());
    if admission_id.is_empty() || !declared.contains(admission_id) || !admission_resolves {
        errs.push(
            "[java-free-slice-identity-admission] the boundary admission stage must resolve from the cascade DATA"
                .to_string(),
        );
    }
    if in_ids.contains(admission_id) || out_ids.contains(admission_id) {
        errs.push(
            "[java-free-slice-identity-admission] the boundary admission stage must not be counted"
                .to_string(),
        );
    }
    let mut covered: BTreeSet<String> = in_ids.union(&out_ids).cloned().collect();
    covered.insert(admission_id.to_string());
    if covered != declared {
        errs.push(
            "[java-free-slice-stage-partition] in/out stages plus the boundary admission must cover every cascade stage exactly once"
                .to_string(),
        );
    }

    if let Some(stages) = in_slice.as_sequence() {
        for stage in stages {
            let id = stage["stage_id"].as_str().unwrap_or("<unnamed>");
            if stage["java_required"].as_bool() != Some(false) {
                errs.push(format!(
                    "[java-free-slice-stage-partition] {id} is in slice and must not require Java"
                ));
            }
            if stage["disposition"].as_str() != Some("observed_zero_under_guard") {
                errs.push(format!(
                    "[java-free-slice-disposition] {id} must carry observed_zero_under_guard"
                ));
            }
            if !stage["name"].is_null() {
                errs.push(format!(
                    "[java-free-slice-stage-source] {id} must read its name by pointer, not copy it"
                ));
            }
            match stage["name_pointer"].as_str() {
                Some(pointer)
                    if dotted(compatibility, pointer)
                        .and_then(Value::as_str)
                        .is_some_and(|name| !name.is_empty()) => {}
                Some(pointer) => errs.push(format!(
                    "[java-free-slice-stage-source] {id} name pointer `{pointer}` does not resolve"
                )),
                None => errs.push(format!(
                    "[java-free-slice-stage-source] {id} carries no name pointer"
                )),
            }
        }
    }

    let vocabulary: BTreeSet<&str> = slice["out_of_slice_reason_vocabulary"]
        .as_sequence()
        .map(|items| items.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    if vocabulary.is_empty() {
        errs.push("[java-free-slice-out-of-slice-reason] reason vocabulary is empty".to_string());
    }

    if let Some(stages) = out_slice.as_sequence() {
        for stage in stages {
            let id = stage["stage_id"].as_str().unwrap_or("<unnamed>");
            if stage["java_required"].as_bool() != Some(true) {
                errs.push(format!(
                    "[java-free-slice-stage-partition] {id} is out of slice and must require Java"
                ));
            }
            match stage["reason"].as_str() {
                Some(reason) if vocabulary.contains(reason) => {}
                Some(reason) => errs.push(format!(
                    "[java-free-slice-out-of-slice-reason] {id} reason `{reason}` is outside the closed vocabulary"
                )),
                None => errs.push(format!(
                    "[java-free-slice-out-of-slice-reason] {id} carries no typed reason"
                )),
            }
            if ["count", "attempts", "observed", "value", "java_calls"]
                .iter()
                .any(|key| !stage[*key].is_null())
            {
                errs.push(format!(
                    "[java-free-slice-numeric-out-of-slice] {id} must not carry a numeric count"
                ));
            }
        }
    }

    // Lanes: the existing Rust donor lanes, Java-free.
    let lanes: BTreeSet<&str> = slice["lanes_in_slice"]
        .as_sequence()
        .map(|items| {
            items
                .iter()
                .filter_map(|lane| lane["lane_id"].as_str())
                .collect()
        })
        .unwrap_or_default();
    if lanes
        != BTreeSet::from([
            "pdf_inspector_positioned",
            "pdf_inspector_positioned_xy_cut",
        ])
    {
        errs.push(
            "[java-free-slice-lane] donor lanes must be exactly the two pdf-inspector lanes"
                .to_string(),
        );
    }
    if let Some(lanes) = slice["lanes_in_slice"].as_sequence() {
        for lane in lanes {
            if lane["java_required"].as_bool() != Some(false) {
                errs.push(format!(
                    "[java-free-slice-lane] lane `{}` must not require Java",
                    lane["lane_id"].as_str().unwrap_or("<unnamed>")
                ));
            }
        }
    }

    // This block is the sole source of the slice boundary.
    let authority = &slice["authority"];
    if authority["declaration_manifest"].as_str() != Some(MANIFEST)
        || authority["declaration_pointer"].as_str() != Some("java_free_vertical_slice")
        || authority["sole_source"].as_bool() != Some(true)
        || authority["second_contract_file"].as_str() != Some("forbidden")
    {
        errs.push(
            "[java-free-slice-authority] the declaration must be the sole source of the boundary"
                .to_string(),
        );
    }

    // Locks stay closed.
    let locks = &slice["locks"];
    if locks["deployed"].as_bool() != Some(false)
        || locks["import_eligible"].as_bool() != Some(false)
    {
        errs.push("[java-free-slice-lock] deployed/import_eligible must stay false".to_string());
    }
    for key in ["auto_promote", "production_route", "graph_write"] {
        if locks[key].as_str() != Some("forbidden") {
            errs.push(format!("[java-free-slice-lock] {key} must stay forbidden"));
        }
    }

    errs
}

/// A one-shot mutation of the declaration block, used to prove the gate drops
/// a bad declaration by name rather than silently accepting it.
type SliceMutation = Box<dyn FnOnce(&mut Value)>;

fn mutated_slice<F: FnOnce(&mut Value)>(mutate: F) -> Value {
    let mut manifest = yaml(MANIFEST);
    let mut slice = manifest["java_free_vertical_slice"].clone();
    mutate(&mut slice);
    manifest["java_free_vertical_slice"] = slice;
    manifest
}

#[test]
fn java_free_vertical_slice_declaration_is_bound_to_existing_carriers() {
    let manifest = yaml(MANIFEST);
    let compatibility = yaml(COMPATIBILITY);
    let errs = java_free_slice_errs(&manifest, &compatibility);
    assert!(errs.is_empty(), "declaration must be clean: {errs:?}");

    // The in-slice stage ids resolve to the REAL cascade names by pointer.
    let slice = &manifest["java_free_vertical_slice"];
    let stages = slice["stages_in_slice"]
        .as_sequence()
        .expect("slice stages");
    let p1 = dotted(
        &compatibility,
        stages[0]["name_pointer"].as_str().expect("P1 pointer"),
    )
    .and_then(Value::as_str);
    let p2 = dotted(
        &compatibility,
        stages[1]["name_pointer"].as_str().expect("P2 pointer"),
    )
    .and_then(Value::as_str);
    assert_eq!(p1, Some("pdfalto_layout_lattice"), "P1 name via pointer");
    assert_eq!(p2, Some("document_segmentation"), "P2 name via pointer");

    // The corpus binding resolves to the route's own pinned twelve documents,
    // while the layout donor corpus stays the untouched three / 202.
    let binding = &slice["corpus_binding"];
    let pointer = binding["pointer"].as_str().expect("corpus pointer");
    let block_pointer = pointer
        .strip_suffix(".documents")
        .expect("corpus list pointer");
    let block = dotted(&manifest, block_pointer).expect("corpus block resolves");
    assert_eq!(block["documents"].as_sequence().unwrap().len(), 12);
    assert_eq!(block["corpus_n"].as_u64(), Some(12));
    assert_eq!(block["page_denominator"].as_u64(), Some(381));
    assert_eq!(binding["document_count"].as_u64(), Some(12));
    assert_eq!(manifest["corpus"]["corpus_n"].as_u64(), Some(3));
    assert_eq!(manifest["corpus"]["page_denominator"].as_u64(), Some(202));
    assert_eq!(
        manifest["corpus"]["documents"].as_sequence().unwrap().len(),
        3
    );
}

#[test]
fn java_free_vertical_slice_mutations_fail_visible() {
    let compatibility = yaml(COMPATIBILITY);
    let cases: [(&str, SliceMutation, &str); 10] = [
        (
            "invented out-of-slice stage",
            Box::new(|slice| {
                slice["stages_out_of_slice"][0]["stage_id"] = Value::String("P99".into());
            }),
            "[java-free-slice-stage-partition]",
        ),
        (
            "dropped out-of-slice stage",
            Box::new(|slice| {
                slice["stages_out_of_slice"]
                    .as_sequence_mut()
                    .expect("out-of-slice stages")
                    .pop();
            }),
            "[java-free-slice-stage-partition]",
        ),
        (
            "stage declared in and out of the slice",
            Box::new(|slice| {
                slice["stages_in_slice"][0]["stage_id"] = Value::String("P3".into());
            }),
            "[java-free-slice-stage-partition]",
        ),
        (
            "reason outside the closed vocabulary",
            Box::new(|slice| {
                slice["stages_out_of_slice"][0]["reason"] = Value::String("because".into());
            }),
            "[java-free-slice-out-of-slice-reason]",
        ),
        (
            "numeric count on an out-of-slice stage",
            Box::new(|slice| {
                slice["stages_out_of_slice"][0]["count"] = Value::Number(0.into());
            }),
            "[java-free-slice-numeric-out-of-slice]",
        ),
        (
            "Java-required donor lane",
            Box::new(|slice| {
                slice["lanes_in_slice"][0]["java_required"] = Value::Bool(true);
            }),
            "[java-free-slice-lane]",
        ),
        (
            "product claim scope",
            Box::new(|slice| {
                slice["claim_scope"] = Value::String("product".into());
            }),
            "[java-free-slice-schema]",
        ),
        (
            "opened import lock",
            Box::new(|slice| {
                slice["locks"]["import_eligible"] = Value::Bool(true);
            }),
            "[java-free-slice-lock]",
        ),
        (
            "duplicated document identities",
            Box::new(|slice| {
                slice["corpus_binding"]["documents"] = Value::Sequence(Vec::new());
            }),
            "[java-free-slice-corpus-binding]",
        ),
        (
            "unclassified identity admission stage",
            Box::new(|slice| {
                slice["identity_admission_stage"]["stage_id"] = Value::Null;
            }),
            "[java-free-slice-identity-admission]",
        ),
    ];

    for (name, mutate, expected) in cases {
        let manifest = mutated_slice(mutate);
        let errs = java_free_slice_errs(&manifest, &compatibility);
        assert!(
            errs.iter().any(|err| err.contains(expected)),
            "mutation `{name}` must fail with {expected}: {errs:?}"
        );
    }

    // A missing block and an empty cascade are also typed refusals.
    let mut absent = yaml(MANIFEST);
    absent["java_free_vertical_slice"] = Value::Null;
    assert!(
        java_free_slice_errs(&absent, &compatibility)
            .iter()
            .any(|err| err.contains("[java-free-slice-missing]")),
        "an absent declaration must be a typed refusal"
    );
}
