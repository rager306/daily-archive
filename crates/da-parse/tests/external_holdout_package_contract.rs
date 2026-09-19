//! Bounded external holdout package — fail-closed citation and human release
//! gate contract (GAR-015, M510-jsg1pj / S03 / T03).
//!
//! This gate is deliberately independent of the Rust data-contract validator in
//! `crates/da-parse/tests/data_contracts.rs` and of the Governor Python oracle
//! (`scripts/governor/adapters/checks/external_holdout_package.py`): ADR-051
//! requires an oracle to share no code with the surface it checks. It imports no
//! `da-parse` symbol, decodes no typed model, and reads the committed manifest
//! with the generic `serde_yaml` reader. The oracles below re-derive the two
//! fail-closed properties from the declaration itself, so a declaration that
//! would permit the forbidden act is refused instead of being read as a
//! configurable switch:
//!
//! 1. **No unexecuted representation is executed evidence.** An unexecuted
//!    external representation, and every external row whose state is
//!    `bytes_absent`, can never be cited as executed evidence; an executed claim
//!    is refused while the contract declares the probe `not_executed`; and the
//!    citation is admissible only for a complete, frozen package carrying a
//!    recorded human release decision whose artifact exists on disk.
//! 2. **The release decision is reachable only through the recorded human
//!    gate.** Prose never authorizes redistribution, a production splice or a
//!    graph write; and while the declared gates hold no clearance and the
//!    prohibited authorizations stay blocked, no authorization path exists at
//!    all — not even for a claimed recorded decision.
//!
//! Clones of the live document are mutated in-process, so every negative case
//! asserts a typed refusal label rather than a non-empty error vector, and a
//! positive control per oracle proves the oracle discriminates instead of
//! refusing everything. The finding is asserted to remain open: the manifest
//! declares an unexecuted probe and the GAR-015 register record is not resolved,
//! so this slice never claims an executed external probe.

use serde_yaml::Value as Yaml;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const PREFIX: &str = "[external-holdout-package]";
const MANIFEST: &str = "data/parse/acceptance_manifest.yaml";
const REGISTER: &str = "doc/review/grobid-absorption-review-2026-09-10.jsonl";
const FINDING_ID: &str = "GAR-015";

/// The six canonical bounded-package elements (GAR-015), ids and canonical refs,
/// in the declared order.
const CANONICAL_ELEMENTS: [(&str, &str); 6] = [
    (
        "acceptance_margins",
        "external_holdout_contract.acceptance_margins",
    ),
    (
        "document_level_uncertainty_method",
        "external_holdout_contract.document_level_reporting.uncertainty_method",
    ),
    (
        "transfer_latency_and_cost_accounting",
        "external_holdout_contract.transfer_accounting",
    ),
    (
        "frozen_probe_identity",
        "external_holdout_contract.identity_binding",
    ),
    (
        "result_invalidation_rules",
        "external_holdout_contract.result_invalidation",
    ),
    (
        "recorded_human_release_decision",
        "release_decision_package.human_release_review",
    ),
];

/// The five conditions the declaration itself says an authorization requires.
const HUMAN_AUTHORIZATION_GATES: [&str; 5] = [
    "named_reviewer",
    "explicit_decision",
    "passing_quality_gate",
    "passing_cost_gate",
    "passing_rights_gate",
];

/// The four result-invalidation triggers, each of which must retire a result.
const REQUIRED_INVALIDATION_KEYS: [&str; 4] = [
    "on_probe_identity_change",
    "on_source_revision_change",
    "on_manifest_identity_change",
    "on_margin_revision_change",
];

const RECORDED_DECISION_STATES: [&str; 2] = ["recorded", "approved"];
const CLOSED_EXTERNAL_ROW_STATES: [&str; 2] = ["executed", "bytes_absent"];
const OPEN_FINDING_STATES: [&str; 3] = ["open", "in_progress", "blocked"];
const PROHIBITED_ACTIONS: [&str; 5] = [
    "redistribution",
    "production_use",
    "model_distribution",
    "external_downloads",
    "graph_write_or_import",
];

/// The independent oracles' closed reading of a cleared gate / authorized
/// package. Absence is refusal, never a default pass: the live manifest holds no
/// gate in this vocabulary (`quality`/`rights` blocked, `cost`
/// bounded_not_measured) and `package_status: pending_human_review`, which is
/// exactly why no authorization is reachable from the committed tree. A future
/// genuine release must introduce these tokens explicitly.
const GATE_PASSED: &str = "passed";
const AUTHORIZED_PACKAGE_STATUS: &str = "approved";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel))
        .unwrap_or_else(|e| panic!("{PREFIX} cannot read `{rel}`: {e}"))
}

fn manifest() -> Yaml {
    serde_yaml::from_str(&read(MANIFEST))
        .unwrap_or_else(|e| panic!("{PREFIX} `{MANIFEST}` is not valid YAML: {e}"))
}

fn at<'a>(node: &'a Yaml, key: &str) -> Option<&'a Yaml> {
    node.get(key)
}

fn at_path<'a>(node: &'a Yaml, path: &str) -> Option<&'a Yaml> {
    let mut current = node;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn at_path_mut<'a>(node: &'a mut Yaml, segments: &[&str]) -> Option<&'a mut Yaml> {
    match segments.split_first() {
        Some((head, tail)) => at_path_mut(node.get_mut(*head)?, tail),
        None => Some(node),
    }
}

fn str_at<'a>(node: &'a Yaml, key: &str) -> Option<&'a str> {
    at(node, key).and_then(Yaml::as_str)
}

fn flag(node: &Yaml, key: &str) -> Option<bool> {
    at(node, key).and_then(Yaml::as_bool)
}

fn seq<'a>(node: &'a Yaml, key: &str) -> Option<&'a Vec<Yaml>> {
    at(node, key).and_then(Yaml::as_sequence)
}

fn strings_of(values: &[Yaml]) -> BTreeSet<String> {
    values
        .iter()
        .filter_map(Yaml::as_str)
        .map(ToString::to_string)
        .collect()
}

fn put_str(doc: &mut Yaml, path: &str, value: &str) {
    put_node(doc, path, Yaml::String(value.to_string()));
}

fn put_bool(doc: &mut Yaml, path: &str, value: bool) {
    put_node(doc, path, Yaml::Bool(value));
}

fn put_node(doc: &mut Yaml, path: &str, value: Yaml) {
    let segments: Vec<&str> = path.split('.').collect();
    let target = at_path_mut(doc, &segments)
        .unwrap_or_else(|| panic!("{PREFIX} cannot resolve `{path}` for mutation"));
    *target = value;
}

/// External rows are `classes.<class>.rows`; a `local_proxy` block is never an
/// external row. Returns `(class, id, state)`.
fn external_rows(doc: &Yaml) -> Vec<(String, String, String)> {
    let mut rows = Vec::new();
    let Some(classes) = at(doc, "classes").and_then(Yaml::as_mapping) else {
        return rows;
    };
    for (class_name, class) in classes {
        let Some(name) = class_name.as_str() else {
            continue;
        };
        let Some(entries) = at(class, "rows").and_then(Yaml::as_sequence) else {
            continue;
        };
        for entry in entries {
            rows.push((
                name.to_string(),
                str_at(entry, "id").unwrap_or("<unnamed>").to_string(),
                str_at(entry, "state").unwrap_or("<missing>").to_string(),
            ));
        }
    }
    rows
}

fn finding_record() -> Yaml {
    for line in read(REGISTER).lines().filter(|l| !l.trim().is_empty()) {
        let record: Yaml = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("{PREFIX} `{REGISTER}` is not valid JSONL: {e}"));
        if str_at(&record, "finding_id") == Some(FINDING_ID) {
            return record;
        }
    }
    panic!("{PREFIX} `{REGISTER}` must contain a `{FINDING_ID}` record");
}

// ───────────────────────────── citation oracle ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Representation {
    Executed,
    NotExecuted,
    BytesAbsent,
}

impl Representation {
    fn from_token(token: &str) -> Option<Self> {
        match token {
            "executed" => Some(Self::Executed),
            "not_executed" => Some(Self::NotExecuted),
            "bytes_absent" => Some(Self::BytesAbsent),
            _ => None,
        }
    }
}

/// How the caller claims the recorded human release decision is satisfied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HumanGate {
    /// No gate was consulted.
    Absent,
    /// A release decision is claimed but no recorded artifact names it.
    ClaimedWithoutRecord,
    /// The recorded human gate in the release-decision package.
    Recorded,
}

/// May `representation` be cited as EXECUTED evidence of an external holdout
/// result? Every branch is derived from the declaration, and a declaration that
/// would permit the citation is itself refused rather than read as a switch:
/// `unexecuted_external_rows_are_evidence: true` is a refusal, not a permission.
fn cite_as_executed_evidence(
    doc: &Yaml,
    representation: Representation,
    human_gate: HumanGate,
    prose: Option<&str>,
) -> Result<(), String> {
    let (Some(contract), Some(release)) = (
        at(doc, "external_holdout_contract"),
        at(doc, "release_decision_package"),
    ) else {
        return Err("package_root_missing".into());
    };

    // ── completeness: the six canonical elements, once each, locks held ──────
    let package =
        at(contract, "package_elements").ok_or_else(|| "package_element_missing".to_string())?;
    if flag(package, "required_before_citation") != Some(true)
        || flag(package, "citation_blocked_until_complete") != Some(true)
    {
        return Err("package_completeness_lock_eroded".into());
    }
    let declared: Vec<(String, String)> = seq(package, "elements")
        .map(|elements| {
            elements
                .iter()
                .filter_map(|element| {
                    Some((
                        str_at(element, "id")?.to_string(),
                        str_at(element, "ref")?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let expected: BTreeSet<(String, String)> = CANONICAL_ELEMENTS
        .iter()
        .map(|(id, reference)| (id.to_string(), reference.to_string()))
        .collect();
    let declared_set: BTreeSet<(String, String)> = declared.iter().cloned().collect();
    if declared.len() != CANONICAL_ELEMENTS.len() || declared_set != expected {
        return Err("package_element_missing".into());
    }
    for (_, reference) in &declared {
        if at_path(doc, reference).is_none() {
            return Err("package_element_ref_unresolved".into());
        }
    }

    // ── the fail-closed citation rule ────────────────────────────────────────
    let citation = at(contract, "citation_rule")
        .ok_or_else(|| "citation_without_release_decision".to_string())?;
    if flag(citation, "requires_executed_state") != Some(true)
        || str_at(citation, "citation_of_unexecuted_external_rows") != Some("forbidden")
        || flag(citation, "unexecuted_external_rows_are_evidence") != Some(false)
    {
        return Err("unexecuted_rows_read_as_evidence".into());
    }
    if flag(citation, "bytes_absent_rows_are_evidence") != Some(false) {
        return Err("bytes_absent_rows_read_as_evidence".into());
    }
    if flag(citation, "prose_alone_authorizes_citation") != Some(false) {
        return Err("prose_alone_authorizes_citation".into());
    }
    if flag(citation, "requires_recorded_human_release_decision") != Some(true)
        || str_at(citation, "citation_without_release_decision") != Some("forbidden")
    {
        return Err("citation_without_release_decision".into());
    }
    let citation_record = str_at(citation, "release_decision_record").unwrap_or_default();
    if citation_record.trim().is_empty() || !root().join(citation_record).is_file() {
        return Err("release_decision_not_recorded".into());
    }

    // ── byte policy: no external byte ever enters the evidence path ──────────
    let byte_policy =
        at(contract, "byte_policy").ok_or_else(|| "external_bytes_load_permitted".to_string())?;
    if str_at(byte_policy, "external_bytes") != Some("bytes_absent")
        || str_at(byte_policy, "load_external_bytes") != Some("forbidden")
        || str_at(byte_policy, "download_external_bytes") != Some("forbidden")
        || str_at(byte_policy, "materialize_external_files") != Some("forbidden")
    {
        return Err("external_bytes_load_permitted".into());
    }
    if flag(byte_policy, "external_rows_are_evidence") != Some(false) {
        return Err("bytes_absent_rows_read_as_evidence".into());
    }

    // ── frozen probe identity + invalidation rules ───────────────────────────
    let identity =
        at(contract, "identity_binding").ok_or_else(|| "probe_identity_not_frozen".to_string())?;
    if flag(identity, "frozen_before_execution") != Some(true)
        || flag(identity, "frozen_apart_from_production_corpora") != Some(true)
        || str_at(identity, "production_corpus_identity_overlap") != Some("forbidden")
    {
        return Err("probe_identity_not_frozen".into());
    }
    let invalidation = at(contract, "result_invalidation")
        .ok_or_else(|| "invalidation_rule_missing".to_string())?;
    if REQUIRED_INVALIDATION_KEYS
        .iter()
        .any(|key| str_at(invalidation, key) != Some("retire_result"))
        || str_at(invalidation, "stale_result_behavior") != Some("fail_visible")
    {
        return Err("invalidation_rule_missing".into());
    }
    if flag(invalidation, "retired_result_may_be_cited") != Some(false) {
        return Err("retired_result_citable".into());
    }

    // ── the representation itself ────────────────────────────────────────────
    let execution_status = str_at(contract, "execution_status")
        .ok_or_else(|| "execution_state_unknown".to_string())?;
    if !matches!(execution_status, "not_executed" | "executed") {
        return Err("execution_state_unknown".into());
    }
    match representation {
        Representation::BytesAbsent => return Err("bytes_absent_rows_read_as_evidence".into()),
        Representation::NotExecuted => return Err("unexecuted_rows_read_as_evidence".into()),
        Representation::Executed => {}
    }
    if execution_status == "not_executed" {
        // An executed representation cannot exist while the contract says the
        // probe was never executed: the claim is refused, never repaired.
        return Err("execution_claimed_without_release".into());
    }

    // ── the recorded human release gate ──────────────────────────────────────
    if prose.is_some() && human_gate != HumanGate::Recorded {
        return Err("prose_alone_authorizes_citation".into());
    }
    if human_gate != HumanGate::Recorded {
        return Err("citation_without_release_decision".into());
    }
    let human = at(release, "human_release_review")
        .ok_or_else(|| "release_decision_not_recorded".to_string())?;
    if flag(human, "required") != Some(true) {
        return Err("release_decision_not_recorded".into());
    }
    let decision_record = str_at(human, "decision_record").unwrap_or_default();
    if decision_record.trim().is_empty() || !root().join(decision_record).is_file() {
        return Err("release_decision_not_recorded".into());
    }
    if flag(human, "prose_alone_authorizes_release") != Some(false) {
        return Err("release_decision_prose_only".into());
    }
    let declared_gates = seq(human, "authorization_requires")
        .map(|gates| strings_of(gates))
        .unwrap_or_default();
    let required_gates: BTreeSet<String> = HUMAN_AUTHORIZATION_GATES
        .iter()
        .map(ToString::to_string)
        .collect();
    if !required_gates.is_subset(&declared_gates) {
        return Err("release_decision_prose_only".into());
    }
    let decision = str_at(human, "decision").unwrap_or_default();
    if !RECORDED_DECISION_STATES.contains(&decision) {
        return Err("release_decision_not_recorded".into());
    }
    if str_at(human, "reviewer").is_none_or(|reviewer| reviewer.trim().is_empty()) {
        return Err("release_decision_claimed_without_record".into());
    }
    Ok(())
}

// ──────────────────────────── release oracle ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReleaseAction {
    Redistribution,
    ProductionSplice,
    GraphWrite,
}

impl ReleaseAction {
    fn authorization_key(self) -> &'static str {
        match self {
            Self::Redistribution => "redistribution",
            Self::ProductionSplice => "production_use",
            Self::GraphWrite => "graph_write_or_import",
        }
    }

    fn token(self) -> &'static str {
        match self {
            Self::Redistribution => "redistribution",
            Self::ProductionSplice => "production_splice",
            Self::GraphWrite => "graph_write",
        }
    }
}

/// What the claimant offers as the authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReleaseClaim<'a> {
    /// A prose note is offered as the authorization.
    ProseOnly(&'a str),
    /// Nothing is offered.
    Unoffered,
    /// The recorded human release gate in the document is offered.
    RecordedGate,
}

/// May `action` be authorized? The oracle never reads prose as authority, and it
/// evaluates the recorded human gate against the declaration's own conditions
/// (`authorization_requires`, the gate statuses, `prohibited_authorizations`)
/// rather than against the claimant's assertion.
fn authorize_release(
    doc: &Yaml,
    action: ReleaseAction,
    claim: ReleaseClaim<'_>,
) -> Result<(), String> {
    let release = at(doc, "release_decision_package")
        .ok_or_else(|| "release_decision_not_recorded".to_string())?;
    let human = at(release, "human_release_review")
        .ok_or_else(|| "release_decision_not_recorded".to_string())?;
    if flag(human, "prose_alone_authorizes_release") != Some(false) {
        return Err("release_decision_prose_only".into());
    }
    match claim {
        ReleaseClaim::ProseOnly(_) => return Err("prose_alone_authorizes_release".into()),
        ReleaseClaim::Unoffered => return Err("release_decision_not_recorded".into()),
        ReleaseClaim::RecordedGate => {}
    }
    if flag(human, "required") != Some(true) {
        return Err("release_decision_not_recorded".into());
    }
    let decision_record = str_at(human, "decision_record").unwrap_or_default();
    if decision_record.trim().is_empty() || !root().join(decision_record).is_file() {
        return Err("release_decision_not_recorded".into());
    }
    let declared_gates = seq(human, "authorization_requires")
        .map(|gates| strings_of(gates))
        .unwrap_or_default();
    let required_gates: BTreeSet<String> = HUMAN_AUTHORIZATION_GATES
        .iter()
        .map(ToString::to_string)
        .collect();
    if !required_gates.is_subset(&declared_gates) {
        return Err("release_decision_prose_only".into());
    }
    let decision = str_at(human, "decision").unwrap_or_default();
    if !RECORDED_DECISION_STATES.contains(&decision) {
        return Err("release_decision_not_recorded".into());
    }
    if str_at(human, "reviewer").is_none_or(|reviewer| reviewer.trim().is_empty()) {
        return Err("release_decision_claimed_without_record".into());
    }
    if str_at(release, "package_status") != Some(AUTHORIZED_PACKAGE_STATUS) {
        return Err("release_decision_not_recorded".into());
    }
    for gate in ["quality", "cost", "rights"] {
        if at(release, gate).and_then(|node| str_at(node, "status")) != Some(GATE_PASSED) {
            return Err("release_gate_not_passed".into());
        }
    }
    let prohibited = at(release, "prohibited_authorizations")
        .ok_or_else(|| "release_decision_not_recorded".to_string())?;
    if str_at(prohibited, action.authorization_key()) != Some("allowed") {
        return Err("prohibited_authorization_blocked".into());
    }
    Ok(())
}

fn cleared_release_document(action: ReleaseAction) -> Yaml {
    let mut cleared = manifest();
    put_str(
        &mut cleared,
        "release_decision_package.package_status",
        AUTHORIZED_PACKAGE_STATUS,
    );
    put_str(
        &mut cleared,
        "release_decision_package.human_release_review.reviewer",
        "named-reviewer",
    );
    put_str(
        &mut cleared,
        "release_decision_package.human_release_review.decision",
        "recorded",
    );
    for gate in ["quality", "cost", "rights"] {
        put_str(
            &mut cleared,
            &format!("release_decision_package.{gate}.status"),
            GATE_PASSED,
        );
    }
    put_str(
        &mut cleared,
        &format!(
            "release_decision_package.prohibited_authorizations.{}",
            action.authorization_key()
        ),
        "allowed",
    );
    cleared
}

// ──────────────────────────────── tests ──────────────────────────────────

#[test]
fn bounded_package_declares_six_canonical_elements_that_resolve() {
    let doc = manifest();
    let package = at_path(&doc, "external_holdout_contract.package_elements")
        .unwrap_or_else(|| panic!("{PREFIX} `{MANIFEST}` must declare the bounded package"));
    assert_eq!(
        str_at(package, "schema_version"),
        Some("external-holdout-package-elements.v1")
    );
    assert_eq!(flag(package, "required_before_citation"), Some(true));
    assert_eq!(flag(package, "citation_blocked_until_complete"), Some(true));

    let declared: Vec<(String, String)> = seq(package, "elements")
        .expect("package_elements.elements")
        .iter()
        .map(|element| {
            (
                str_at(element, "id").expect("element id").to_string(),
                str_at(element, "ref").expect("element ref").to_string(),
            )
        })
        .collect();
    let expected: Vec<(String, String)> = CANONICAL_ELEMENTS
        .iter()
        .map(|(id, reference)| (id.to_string(), reference.to_string()))
        .collect();
    assert_eq!(declared, expected, "the declared package must be exact");
    for (id, reference) in &declared {
        assert!(
            at_path(&doc, reference).is_some(),
            "{PREFIX} element `{id}` points at a dangling ref `{reference}`"
        );
    }

    // The citation gate is only meaningful with a recorded human artifact route.
    assert!(
        root()
            .join(
                at_path(
                    &doc,
                    "external_holdout_contract.citation_rule.release_decision_record"
                )
                .and_then(Yaml::as_str)
                .expect("citation release_decision_record")
            )
            .is_file(),
        "{PREFIX} the recorded human release-decision artifact must exist on disk"
    );
}

#[test]
fn unexecuted_and_bytes_absent_representations_are_never_executed_evidence() {
    let doc = manifest();
    let contract = at_path(&doc, "external_holdout_contract").expect("external_holdout_contract");
    assert_eq!(str_at(contract, "execution_status"), Some("not_executed"));
    let byte_policy = at(contract, "byte_policy").expect("byte_policy");
    assert_eq!(str_at(byte_policy, "external_bytes"), Some("bytes_absent"));
    assert_eq!(flag(byte_policy, "external_rows_are_evidence"), Some(false));
    let citation = at(contract, "citation_rule").expect("citation_rule");
    assert_eq!(flag(citation, "requires_executed_state"), Some(true));
    assert_eq!(
        flag(citation, "unexecuted_external_rows_are_evidence"),
        Some(false)
    );
    assert_eq!(
        flag(citation, "bytes_absent_rows_are_evidence"),
        Some(false)
    );
    assert_eq!(
        flag(citation, "prose_alone_authorizes_citation"),
        Some(false)
    );
    assert_eq!(
        flag(citation, "requires_recorded_human_release_decision"),
        Some(true)
    );
    assert_eq!(
        str_at(citation, "citation_of_unexecuted_external_rows"),
        Some("forbidden")
    );
    assert_eq!(
        str_at(citation, "citation_without_release_decision"),
        Some("forbidden")
    );

    let rows = external_rows(&doc);
    assert!(!rows.is_empty(), "{PREFIX} external rows must be declared");
    for (class, id, state) in &rows {
        assert!(
            CLOSED_EXTERNAL_ROW_STATES.contains(&state.as_str()),
            "{PREFIX} external row `{class}/{id}` has an unclassifiable state `{state}`"
        );
        assert_eq!(
            state, "bytes_absent",
            "{PREFIX} no external row may be executed on the committed tree"
        );
        let representation = Representation::from_token(state).expect("closed row state");
        assert_eq!(
            cite_as_executed_evidence(&doc, representation, HumanGate::Recorded, None),
            Err("bytes_absent_rows_read_as_evidence".to_string()),
            "{PREFIX} `{class}/{id}` must never be cited as executed evidence"
        );
        assert_eq!(
            cite_as_executed_evidence(&doc, Representation::NotExecuted, HumanGate::Recorded, None),
            Err("unexecuted_rows_read_as_evidence".to_string())
        );
    }

    // An executed claim while the contract declares the probe not executed.
    assert_eq!(
        cite_as_executed_evidence(&doc, Representation::Executed, HumanGate::Recorded, None),
        Err("execution_claimed_without_release".to_string())
    );
}

#[test]
fn incomplete_or_unlocked_package_is_refused_before_any_citation() {
    let doc = manifest();

    for (index, (missing_id, _)) in CANONICAL_ELEMENTS.iter().enumerate() {
        let mut broken = doc.clone();
        let elements = at_path_mut(
            &mut broken,
            &["external_holdout_contract", "package_elements", "elements"],
        )
        .and_then(Yaml::as_sequence_mut)
        .expect("package_elements.elements");
        elements.remove(index);
        assert_eq!(
            cite_as_executed_evidence(&broken, Representation::Executed, HumanGate::Recorded, None),
            Err("package_element_missing".to_string()),
            "{PREFIX} dropping `{missing_id}` must refuse the package"
        );
    }

    let mut unlocked = doc.clone();
    put_bool(
        &mut unlocked,
        "external_holdout_contract.package_elements.citation_blocked_until_complete",
        false,
    );
    assert_eq!(
        cite_as_executed_evidence(
            &unlocked,
            Representation::Executed,
            HumanGate::Recorded,
            None
        ),
        Err("package_completeness_lock_eroded".to_string())
    );

    // A declaration that would permit the forbidden act is refused, not obeyed.
    let mut unexecuted = doc.clone();
    put_bool(
        &mut unexecuted,
        "external_holdout_contract.citation_rule.unexecuted_external_rows_are_evidence",
        true,
    );
    assert_eq!(
        cite_as_executed_evidence(
            &unexecuted,
            Representation::Executed,
            HumanGate::Recorded,
            None
        ),
        Err("unexecuted_rows_read_as_evidence".to_string())
    );

    let mut prose = doc.clone();
    put_bool(
        &mut prose,
        "external_holdout_contract.citation_rule.prose_alone_authorizes_citation",
        true,
    );
    assert_eq!(
        cite_as_executed_evidence(&prose, Representation::Executed, HumanGate::Recorded, None),
        Err("prose_alone_authorizes_citation".to_string())
    );

    let mut retired = doc.clone();
    put_bool(
        &mut retired,
        "external_holdout_contract.result_invalidation.retired_result_may_be_cited",
        true,
    );
    assert_eq!(
        cite_as_executed_evidence(
            &retired,
            Representation::Executed,
            HumanGate::Recorded,
            None
        ),
        Err("retired_result_citable".to_string())
    );

    // Human-gate branches, reachable only once the contract says executed.
    let mut executed = doc.clone();
    put_str(
        &mut executed,
        "external_holdout_contract.execution_status",
        "executed",
    );
    assert_eq!(
        cite_as_executed_evidence(&executed, Representation::Executed, HumanGate::Absent, None),
        Err("citation_without_release_decision".to_string())
    );
    assert_eq!(
        cite_as_executed_evidence(
            &executed,
            Representation::Executed,
            HumanGate::Absent,
            Some("the release note says approved")
        ),
        Err("prose_alone_authorizes_citation".to_string())
    );
    assert_eq!(
        cite_as_executed_evidence(
            &executed,
            Representation::Executed,
            HumanGate::ClaimedWithoutRecord,
            None
        ),
        Err("citation_without_release_decision".to_string())
    );
    // The live recorded gate is still pending, so even the recorded path refuses.
    assert_eq!(
        cite_as_executed_evidence(
            &executed,
            Representation::Executed,
            HumanGate::Recorded,
            None
        ),
        Err("release_decision_not_recorded".to_string())
    );

    // Positive control: the oracles discriminate; a complete executed package
    // with a recorded human decision IS citable.
    let mut complete = executed.clone();
    put_str(
        &mut complete,
        "release_decision_package.human_release_review.reviewer",
        "named-reviewer",
    );
    put_str(
        &mut complete,
        "release_decision_package.human_release_review.decision",
        "recorded",
    );
    assert!(
        cite_as_executed_evidence(
            &complete,
            Representation::Executed,
            HumanGate::Recorded,
            None
        )
        .is_ok(),
        "{PREFIX} the citation oracle must not refuse everything"
    );
}

#[test]
fn release_authorization_is_reachable_only_through_the_recorded_human_gate() {
    let doc = manifest();
    let human = at_path(&doc, "release_decision_package.human_release_review")
        .expect("human_release_review");
    assert_eq!(flag(human, "required"), Some(true));
    assert_eq!(flag(human, "prose_alone_authorizes_release"), Some(false));
    assert!(
        str_at(human, "reviewer").is_none_or(|reviewer| reviewer.trim().is_empty()),
        "{PREFIX} the live release review must not name a reviewer"
    );
    assert_eq!(str_at(human, "decision"), Some("pending"));
    assert_eq!(
        str_at(human, "decision_record"),
        Some("doc/review/HUMAN-REVIEW-TEMPLATE.md")
    );
    assert_eq!(
        seq(human, "authorization_requires")
            .map(|gates| strings_of(gates))
            .expect("authorization_requires"),
        HUMAN_AUTHORIZATION_GATES
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<String>>()
    );

    let release = at(&doc, "release_decision_package").expect("release_decision_package");
    assert_eq!(
        str_at(release, "package_status"),
        Some("pending_human_review")
    );
    assert_eq!(
        at_path(release, "quality.status").and_then(Yaml::as_str),
        Some("blocked")
    );
    assert_eq!(
        at_path(release, "cost.status").and_then(Yaml::as_str),
        Some("bounded_not_measured")
    );
    assert_eq!(
        at_path(release, "rights.status").and_then(Yaml::as_str),
        Some("blocked")
    );
    let prohibited = at(release, "prohibited_authorizations").expect("prohibited_authorizations");
    for action in PROHIBITED_ACTIONS {
        assert_eq!(
            str_at(prohibited, action),
            Some("blocked"),
            "{PREFIX} `{action}` must stay blocked"
        );
    }

    let actions = [
        ReleaseAction::Redistribution,
        ReleaseAction::ProductionSplice,
        ReleaseAction::GraphWrite,
    ];
    for action in actions {
        // Prose is never an authority, on the live tree or anywhere else.
        assert_eq!(
            authorize_release(
                &doc,
                action,
                ReleaseClaim::ProseOnly("release note says approved")
            ),
            Err("prose_alone_authorizes_release".to_string()),
            "{PREFIX} prose must never authorize {}",
            action.token()
        );
        assert_eq!(
            authorize_release(&doc, action, ReleaseClaim::Unoffered),
            Err("release_decision_not_recorded".to_string())
        );
        // Even a claimed recorded gate cannot authorize: the decision is pending.
        assert_eq!(
            authorize_release(&doc, action, ReleaseClaim::RecordedGate),
            Err("release_decision_not_recorded".to_string()),
            "{PREFIX} {} must not be authorizable from the committed tree",
            action.token()
        );
    }

    // A recorded gate is necessary but not sufficient: the declared prohibition
    // still blocks the action.
    let mut recorded_but_prohibited = manifest();
    put_str(
        &mut recorded_but_prohibited,
        "release_decision_package.package_status",
        AUTHORIZED_PACKAGE_STATUS,
    );
    put_str(
        &mut recorded_but_prohibited,
        "release_decision_package.human_release_review.reviewer",
        "named-reviewer",
    );
    put_str(
        &mut recorded_but_prohibited,
        "release_decision_package.human_release_review.decision",
        "recorded",
    );
    for gate in ["quality", "cost", "rights"] {
        put_str(
            &mut recorded_but_prohibited,
            &format!("release_decision_package.{gate}.status"),
            GATE_PASSED,
        );
    }
    assert_eq!(
        authorize_release(
            &recorded_but_prohibited,
            ReleaseAction::Redistribution,
            ReleaseClaim::RecordedGate
        ),
        Err("prohibited_authorization_blocked".to_string())
    );

    // Positive control: the recorded human gate is a reachable path once every
    // declared condition holds, and prose is still refused on that same tree.
    for action in actions {
        let cleared = cleared_release_document(action);
        assert!(
            authorize_release(&cleared, action, ReleaseClaim::RecordedGate).is_ok(),
            "{PREFIX} the recorded human gate must be reachable for {}",
            action.token()
        );
        assert_eq!(
            authorize_release(&cleared, action, ReleaseClaim::ProseOnly("approved")),
            Err("prose_alone_authorizes_release".to_string()),
            "{PREFIX} prose must never authorize {} even with the gate clear",
            action.token()
        );
    }
}

#[test]
fn finding_stays_open_and_no_executed_external_probe_is_claimed() {
    let record = finding_record();
    assert_eq!(str_at(&record, "finding_id"), Some(FINDING_ID));
    let status = str_at(&record, "status").expect("finding status");
    assert!(
        OPEN_FINDING_STATES.contains(&status),
        "{PREFIX} {FINDING_ID} must stay open until an executed frozen probe and a \
         recorded human release decision exist; found `{status}`"
    );
    assert_eq!(
        at_path(&record, "governor.proposed_rule_id").and_then(Yaml::as_str),
        Some("external_holdout_package")
    );
    let verification = at(&record, "verification")
        .and_then(Yaml::as_sequence)
        .expect("finding verification");
    assert!(
        verification
            .iter()
            .any(|entry| str_at(entry, "ref") == Some("external-holdout-package")),
        "{PREFIX} the finding must still demand the bounded external holdout package"
    );
    assert!(
        verification.iter().any(|entry| {
            str_at(entry, "mode") == Some("human")
                && str_at(entry, "ref") == Some("release-review-decision")
        }),
        "{PREFIX} the finding must still demand the recorded human release decision"
    );

    // Nothing on the committed tree claims an executed external probe.
    let doc = manifest();
    let contract = at(&doc, "external_holdout_contract").expect("external_holdout_contract");
    let release = at(&doc, "release_decision_package").expect("release_decision_package");
    assert_eq!(str_at(contract, "execution_status"), Some("not_executed"));
    assert_eq!(
        at_path(contract, "byte_policy.external_bytes").and_then(Yaml::as_str),
        Some("bytes_absent")
    );
    assert_eq!(
        at_path(contract, "byte_policy.external_rows_are_evidence").and_then(Yaml::as_bool),
        Some(false)
    );
    assert_eq!(
        at_path(contract, "acceptance_margins.measured").and_then(Yaml::as_bool),
        Some(false)
    );
    assert_eq!(
        at_path(contract, "acceptance_margins.margins_are_provisional").and_then(Yaml::as_bool),
        Some(true)
    );
    assert_eq!(
        at_path(contract, "transfer_accounting.measured").and_then(Yaml::as_bool),
        Some(false)
    );
    assert_eq!(
        at_path(contract, "transfer_accounting.transfer_bytes_loaded").and_then(Yaml::as_bool),
        Some(false)
    );
    assert_eq!(
        at_path(contract, "transfer_accounting.transfer_claim_allowed").and_then(Yaml::as_bool),
        Some(false)
    );
    assert_eq!(
        at_path(contract, "cost_bounds.release_claim_allowed").and_then(Yaml::as_bool),
        Some(false)
    );
    assert_eq!(
        str_at(release, "package_status"),
        Some("pending_human_review")
    );
    assert_eq!(
        at_path(release, "human_release_review.decision").and_then(Yaml::as_str),
        Some("pending")
    );
    assert!(
        at_path(release, "human_release_review.reviewer").is_none_or(|reviewer| reviewer.is_null()),
        "{PREFIX} no reviewer may be recorded yet"
    );
    assert_eq!(
        at_path(release, "quality.evidence.external_holdout_status").and_then(Yaml::as_str),
        Some("not_executed")
    );

    let rows = external_rows(&doc);
    assert!(!rows.is_empty());
    assert!(
        rows.iter().all(|(_, _, state)| state == "bytes_absent"),
        "{PREFIX} no external row may be executed on the committed tree"
    );
}
