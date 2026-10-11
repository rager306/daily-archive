//! Cross-language governed-process-record oracle, Rust half (S10/T05).
//!
//! The governor rule `governed_process_record` and the runtime must agree that
//! one driven request's [`GovernedProcessRecord`] binds the DATA route
//! decision, the per-stage admission codes and the registry-bound prompt
//! lineage against the SAME live DATA. This test is the Rust half of that
//! shared oracle:
//!
//! * `governor/fixtures/governed_process_record/consult_oracle.vectors.json`
//!   drives the pure consult predicate against the live
//!   `data/policies/registry.yaml` `consult_status` block (allowed =
//!   `parse_pdf_consults`, forbidden = every other list), and every case must
//!   resolve to the same violation set the Python adapter computes;
//! * the committed positive fixture (the record the runtime actually produced)
//!   must be clean, and every negative record must trigger exactly its
//!   labelled violation, using the same full predicate the Python adapter
//!   asserts (consult + admission + lineage + lock).
//!
//! The lineage predicate resolves the live unified prompt registry through the
//! production `da_adapters::prompt_registry` loader, so a fixture fingerprint
//! that disagrees with the registry is a named violation rather than a green
//! record.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use da_adapters::prompt_registry::load_prompt_registry;
use da_domain::governed_process_record::{GOVERNED_ADMISSION_CODES, GovernedProcessRecord};
use da_domain::prompt_registry::PromptRegistry;

const CONTRACT_REL: &str = "scripts/governor/checks/governed_process_record.yaml";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ── the governor contract (DATA) ────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct Contract {
    locks: Locks,
    policy: Policy,
    prompt: Prompt,
    admission: Admission,
    fixtures: Fixtures,
    oracle: Oracle,
}

#[derive(serde::Deserialize)]
struct Locks {
    forbidden_decision_points: Vec<String>,
}

#[derive(serde::Deserialize)]
struct Policy {
    registry_path: String,
    consult_status_key: String,
    allowed_consult_key: String,
}

#[derive(serde::Deserialize)]
struct Prompt {
    registry_path: String,
    task_registry_path: String,
}

#[derive(serde::Deserialize)]
struct Admission {
    closed_codes: Vec<String>,
}

#[derive(serde::Deserialize)]
struct Fixtures {
    positive: FixtureSpec,
    negative: NegativeSpec,
}

#[derive(serde::Deserialize)]
struct FixtureSpec {
    path: String,
}

#[derive(serde::Deserialize)]
struct NegativeSpec {
    path: String,
    expect: Expect,
}

#[derive(serde::Deserialize)]
struct Expect {
    require_violations: Vec<String>,
}

#[derive(serde::Deserialize)]
struct Oracle {
    path: String,
}

#[derive(serde::Deserialize)]
struct ConsultVector {
    case: String,
    consulted: Vec<String>,
    expect_violations: Vec<String>,
}

#[derive(serde::Deserialize)]
struct NegativeRecord {
    violation: String,
    record: GovernedProcessRecord,
}

fn contract() -> Contract {
    let bytes =
        std::fs::read(repo_root().join(CONTRACT_REL)).expect("read governed-process contract");
    serde_yaml::from_slice(&bytes).expect("parse governed-process contract")
}

/// The allowed / forbidden consult sets derived from the live policy registry.
fn consult_sets(contract: &Contract) -> (BTreeSet<String>, BTreeSet<String>) {
    let bytes = std::fs::read(repo_root().join(&contract.policy.registry_path))
        .expect("read live policy registry");
    let registry: BTreeMap<String, serde_yaml::Value> =
        serde_yaml::from_slice(&bytes).expect("parse live policy registry");
    let status_value = registry
        .get(&contract.policy.consult_status_key)
        .expect("the live policy registry declares the consult_status block");
    let consult_status: BTreeMap<String, Vec<String>> =
        serde_yaml::from_value(status_value.clone()).expect("parse the consult_status block");
    let allowed: BTreeSet<String> = consult_status
        .get(&contract.policy.allowed_consult_key)
        .expect("consult_status declares the allowed-consult key")
        .iter()
        .cloned()
        .collect();
    let mut forbidden: BTreeSet<String> = BTreeSet::new();
    for (key, names) in &consult_status {
        if key == &contract.policy.allowed_consult_key {
            continue;
        }
        forbidden.extend(names.iter().cloned());
    }
    (allowed, forbidden)
}

/// The live unified prompt registry, loaded through the production loader.
fn prompt_registry(contract: &Contract) -> PromptRegistry {
    let root = repo_root();
    let task_registry = da_adapters::prompt_task_registry::load_prompt_task_registry(
        &root.join(&contract.prompt.task_registry_path),
    )
    .expect("load prompt task registry");
    load_prompt_registry(
        &root.join(&contract.prompt.registry_path),
        &task_registry,
        &root,
    )
    .expect("load unified prompt registry")
}

// ── the shared predicates ───────────────────────────────────────────────────

fn is_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn consult_violation(
    consulted: &BTreeSet<String>,
    allowed: &BTreeSet<String>,
    forbidden: &BTreeSet<String>,
) -> bool {
    consulted.is_empty() || !consulted.is_subset(allowed) || !consulted.is_disjoint(forbidden)
}

struct Context {
    allowed: BTreeSet<String>,
    forbidden: BTreeSet<String>,
    closed_codes: BTreeSet<String>,
    forbidden_locks: BTreeSet<String>,
    registry: PromptRegistry,
}

fn record_violations(record: &GovernedProcessRecord, ctx: &Context) -> BTreeSet<&'static str> {
    let mut violations = BTreeSet::new();
    let consulted: BTreeSet<String> = record
        .policy_decisions
        .iter()
        .map(|decision| decision.decision_point.clone())
        .collect();

    if consult_violation(&consulted, &ctx.allowed, &ctx.forbidden) {
        violations.insert("consult_violation");
    }
    if record.import_eligible || !consulted.is_disjoint(&ctx.forbidden_locks) {
        violations.insert("lock_leak");
    }
    if record
        .admissions
        .iter()
        .any(|admission| !ctx.closed_codes.contains(&admission.code))
    {
        violations.insert("admission_violation");
    }
    for lineage in &record.prompt_lineages {
        let ok = match ctx
            .registry
            .resolve(&lineage.prompt_id, &lineage.prompt_version)
        {
            Ok(entry) => {
                entry.prompt_sha256.as_str() == lineage.prompt_sha256
                    && entry.schema_sha256.as_str() == lineage.schema_sha256
                    && is_hex64(&lineage.prompt_sha256)
                    && is_hex64(&lineage.schema_sha256)
            }
            Err(_) => false,
        };
        if !ok {
            violations.insert("lineage_violation");
        }
    }
    violations
}

fn context(contract: &Contract) -> Context {
    let (allowed, forbidden) = consult_sets(contract);
    Context {
        allowed,
        forbidden,
        closed_codes: contract.admission.closed_codes.iter().cloned().collect(),
        forbidden_locks: contract
            .locks
            .forbidden_decision_points
            .iter()
            .cloned()
            .collect(),
        registry: prompt_registry(contract),
    }
}

// ── tests ───────────────────────────────────────────────────────────────────

#[test]
fn consult_oracle_agrees_with_the_governor_rule() {
    let contract = contract();
    let (allowed, forbidden) = consult_sets(&contract);
    let bytes = std::fs::read(repo_root().join(&contract.oracle.path)).expect("read oracle");
    let vectors: Vec<ConsultVector> = serde_json::from_slice(&bytes).expect("parse oracle");
    assert!(
        !vectors.is_empty(),
        "the shared consult oracle must carry at least one case"
    );
    for vector in &vectors {
        let consulted: BTreeSet<String> = vector.consulted.iter().cloned().collect();
        let expected: BTreeSet<String> = vector.expect_violations.iter().cloned().collect();
        let observed: BTreeSet<String> = if consult_violation(&consulted, &allowed, &forbidden) {
            ["consult_violation".to_string()].into_iter().collect()
        } else {
            BTreeSet::new()
        };
        assert_eq!(
            observed, expected,
            "consult oracle case {}: governor decided {observed:?}, oracle expects {expected:?}",
            vector.case
        );
    }
}

#[test]
fn the_allowed_consult_set_is_derived_from_live_data() {
    let contract = contract();
    let (allowed, forbidden) = consult_sets(&contract);
    assert!(
        allowed.contains("profile_route"),
        "the parse-pdf FSM consults profile_route"
    );
    // The later / unregistered names live only in the forbidden set.
    for later in [
        "batch_admission",
        "concurrency_tune",
        "retry_policy",
        "summary_depth",
        "head_fallback",
        "visual_bundle",
        "critic_sample",
    ] {
        assert!(
            !allowed.contains(later),
            "later name {later} must not be in the allowed consult set"
        );
        assert!(
            forbidden.contains(later),
            "later name {later} must be derived into the forbidden set"
        );
    }
}

#[test]
fn the_committed_positive_fixture_is_clean() {
    let contract = contract();
    let ctx = context(&contract);
    let bytes = std::fs::read(repo_root().join(&contract.fixtures.positive.path))
        .expect("read positive fixture");
    let records: Vec<GovernedProcessRecord> =
        serde_json::from_slice(&bytes).expect("parse positive fixture");
    assert!(!records.is_empty(), "positive fixture must carry a record");
    for record in &records {
        let violations = record_violations(record, &ctx);
        assert!(
            violations.is_empty(),
            "positive fixture must be clean, got {violations:?}"
        );
    }
}

#[test]
fn the_committed_negative_fixture_triggers_its_labelled_violations() {
    let contract = contract();
    let ctx = context(&contract);
    let bytes = std::fs::read(repo_root().join(&contract.fixtures.negative.path))
        .expect("read negative fixture");
    let records: Vec<NegativeRecord> =
        serde_json::from_slice(&bytes).expect("parse negative fixture");
    assert!(!records.is_empty(), "negative fixture must carry records");
    let mut observed: BTreeSet<String> = BTreeSet::new();
    for negative in &records {
        let violations = record_violations(&negative.record, &ctx);
        observed.extend(violations.iter().map(|token| token.to_string()));
        assert!(
            violations.contains(negative.violation.as_str()),
            "negative fixture record must trigger its labelled violation {} , got {violations:?}",
            negative.violation
        );
    }
    for required in &contract.fixtures.negative.expect.require_violations {
        assert!(
            observed.contains(required),
            "negative fixture must cover required violation {required}, observed {observed:?}"
        );
    }
}

#[test]
fn the_closed_admission_codes_match_the_governor_contract() {
    let contract = contract();
    let declared: BTreeSet<String> = contract.admission.closed_codes.iter().cloned().collect();
    let domain: BTreeSet<String> = GOVERNED_ADMISSION_CODES
        .iter()
        .map(|code| code.to_string())
        .collect();
    assert_eq!(
        declared, domain,
        "the governor contract's closed admission-code set must equal the domain's"
    );
}
