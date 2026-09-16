//! S07/T03 — closure negatives for policy variants (M507-4fk6m6).
//!
//! The closed sets of the parse-policy plane (exactly six `DecisionPoint`s,
//! `parse_pdf_consults == [grammar_route, profile_route]`, the `later` names
//! outside the enum, the empty-native unknown points) are pinned statically
//! against COMMITTED DATA by `crates/da-parse/tests/data_contracts.rs`,
//! `architecture_contract.rs` and the governor. Nothing there proves that a
//! MUTATED policy variant cannot open that closure at RUNTIME — which is the
//! second half of the slice binding goal ("six DecisionPoints stay closed").
//!
//! These tests drive the REAL production dispatch
//! ([`da_cli::source_cli::resolve_parsed_dispatch`]) against derived registry
//! artifacts (temp copies; the committed `data/policies/registry.yaml` is never
//! rewritten). Every negative must end in a typed refusal — no panic.
//!
//! Mutating process CWD/env is process-global, so every test that touches
//! either holds [`CWD_LOCK`] and restores state through RAII guards.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use da_application::parse_policy_ablation::{
    POLICY_ABLATION_AUTO_PROMOTE, POLICY_ABLATION_PROMOTION_STATE, POLICY_ABLATION_SCHEMA_VERSION,
    PolicyVariantManifest,
};
use da_application::{
    CriticObservation, CriticStatus, EvaluationError, FrozenEvalCase, ProducerProvenance,
    PromotionQualityPolicy, aggregate_policy_variant_ablation,
};
use da_domain::fsm_policy::{DecisionPoint, PolicyRegistry, PolicySource};
use da_ports::{PromotionState, Sha256Digest, SplitIdentity};
use sha2::Digest;

/// The only parsed-stage CLI actions the closed policy may ever select.
const CLOSED_ACTIONS: [&str; 2] = ["parse-pdf", "parse-pdf-toc"];
/// The only grammar routes the closed `GrammarRoute` enum carries.
const CLOSED_ROUTES: [&str; 3] = ["page_outline", "xy_cut", "grobid_structural"];
/// The six sanctioned `DecisionPoint` wires (closed set).
const SIX_POINTS: [&str; 6] = [
    "grammar_route",
    "batch_admission",
    "concurrency_tune",
    "retry_policy",
    "summary_depth",
    "profile_route",
];
/// ADR-076 "later" names: never DecisionPoints, never consulted by parse-pdf.
const LATER_NAMES: [&str; 4] = [
    "head_fallback",
    "visual_bundle",
    "critic_sample",
    "invented_route",
];
/// sha256 of the committed registry (slice contract: byte-for-byte unchanged).
const COMMITTED_REGISTRY_SHA256: &str =
    "300b1c2107cfcf683a17ced994535667ef49b416e54759f54bc9d61fcc376f14";

/// Serializes every CWD/env-mutating test in this binary.
static CWD_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ─────────────────────────── RAII guards ───────────────────────────

/// Restores the original CWD on drop.
struct CwdGuard {
    original: PathBuf,
}

impl CwdGuard {
    fn new(dir: &Path) -> Self {
        let original = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(dir).expect("chdir");
        Self { original }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.original).expect("restore cwd");
    }
}

/// Restores (or removes) a previously-set env var on drop. Edition 2024 marks
/// `std::env::set_var`/`remove_var` unsafe; callers hold [`CWD_LOCK`].
struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: `CWD_LOCK` serializes every env-mutating test in this binary.
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }

    fn clear(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: see `EnvGuard::set`.
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: see `EnvGuard::set`; restoration happens under `CWD_LOCK`.
        match &self.previous {
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

// ─────────────────────────── fixtures ───────────────────────────

fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must resolve")
}

fn committed_registry_path() -> PathBuf {
    repo_root().join("data/policies/registry.yaml")
}

/// Absolute path to a committed, git-tracked policy fixture.
fn policy_fixture_path(name: &str) -> PathBuf {
    let path = repo_root().join("data/policies/fixtures").join(name);
    assert!(
        path.exists(),
        "committed fixture `{name}` must exist: {}",
        path.display()
    );
    path
}

fn committed_registry_value() -> serde_yaml::Value {
    let text = std::fs::read_to_string(committed_registry_path())
        .expect("committed registry must be readable");
    serde_yaml::from_str(&text).expect("committed registry must parse")
}

fn key(name: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(name.to_string())
}

fn seq(items: &[&str]) -> serde_yaml::Value {
    serde_yaml::Value::Sequence(items.iter().map(|s| key(s)).collect())
}

fn strs(value: &serde_yaml::Value) -> Vec<String> {
    value
        .as_sequence()
        .unwrap_or_else(|| panic!("expected a sequence, got {value:?}"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("expected a string, got {item:?}"))
                .to_string()
        })
        .collect()
}

fn sub_mut<'a>(value: &'a mut serde_yaml::Value, name: &str) -> &'a mut serde_yaml::Value {
    value
        .get_mut(name)
        .unwrap_or_else(|| panic!("policy registry lost `{name}`"))
}

fn map_mut<'a>(value: &'a mut serde_yaml::Value, name: &str) -> &'a mut serde_yaml::Mapping {
    sub_mut(value, name)
        .as_mapping_mut()
        .unwrap_or_else(|| panic!("`{name}` is not a mapping"))
}

/// Point a decision point at an absolute `.wat` path and drop the sibling
/// `wasm` key (the process CWD is a temp dir, so relative DATA paths would not
/// resolve).
fn set_wat(registry: &mut serde_yaml::Value, point: &str, wat: &Path) {
    let spec = map_mut(sub_mut(registry, "decision_points"), point);
    spec.insert(
        key("wat"),
        serde_yaml::Value::String(wat.display().to_string()),
    );
    spec.insert(key("wasm"), serde_yaml::Value::Null);
}

fn write_registry(dir: &Path, registry: &serde_yaml::Value, name: &str) -> PathBuf {
    let out = dir.join(name);
    std::fs::write(
        &out,
        serde_yaml::to_string(registry).expect("render registry"),
    )
    .expect("write temp registry");
    out
}

/// Materialize the parsed-stage TOC observation cache so `toc_exists` is true.
fn write_toc_cache(tmp: &Path, paper_id: &str) {
    let dir = tmp.join("artifacts/pipeline_cache/pdf_toc_observations");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{paper_id}.json")),
        r#"{"total_pages": 18, "toc": [{"title": "1 Introduction"}]}"#,
    )
    .unwrap();
}

/// Content hash of the COMPILED module — exactly what a `Wasm` source stamps.
fn compiled_wat_hash(path: &Path) -> String {
    let raw = std::fs::read(path).expect("wat fixture must be readable");
    let compiled = wat::parse_bytes(&raw).expect("wat fixture must compile");
    let mut hasher = sha2::Sha256::new();
    hasher.update(&compiled);
    hex::encode(hasher.finalize())
}

/// Deterministic article features: 18 pages, high text-layer quality, no
/// bookmarks in the environment. The committed `grammar_route.wat` selects
/// `xy_cut` for these.
fn pin_features() -> (EnvGuard, EnvGuard, EnvGuard) {
    (
        EnvGuard::set("DA_PARSE_PAGES", "18"),
        EnvGuard::set("DA_PARSE_BOOKMARKS", "0"),
        EnvGuard::set("DA_PARSE_TEXT_QUALITY", "0.85"),
    )
}

// ─────────────────────────── T01: the enum is the gate ───────────────────────────

/// A registry that LISTS the later names as if they were consulted decision
/// points cannot widen the runtime consult surface: the closed `DecisionPoint`
/// enum is the gate, so the extra DATA keys stay inert and the dispatch still
/// comes from `grammar_route`.
#[test]
fn seventh_point_and_later_names_stay_inert() {
    let _guard = lock();
    let tmp = tempfile::tempdir().unwrap();
    let paper = "5070.00001";
    write_toc_cache(tmp.path(), paper);
    let _restore = CwdGuard::new(tmp.path());
    let (_pages, _bookmarks, _quality) = pin_features();

    // The 7th-point artifacts WOULD select grobid_structural if consulted;
    // grammar_route selects page_outline. Only the latter may decide. Every
    // OTHER point is poisoned the same way — including the four later names
    // and the sanctioned-but-never-consulted `batch_admission` + empty-native
    // points (`concurrency_tune`, `retry_policy`, `summary_depth`).
    let page_outline_fixture = policy_fixture_path("grammar_route_always_page_outline.wat");
    let grobid_fixture = policy_fixture_path("grammar_route_always_grobid.wat");
    let poisoned: Vec<&str> = SIX_POINTS
        .iter()
        .copied()
        .filter(|point| *point != "grammar_route")
        .chain(LATER_NAMES.iter().copied())
        .collect();
    assert_eq!(poisoned.len(), 9, "five sanctioned + four later names");

    let mut registry = committed_registry_value();
    set_wat(&mut registry, "grammar_route", &page_outline_fixture);
    let points = map_mut(&mut registry, "decision_points");
    for point in &poisoned {
        let mut spec = serde_yaml::Mapping::new();
        spec.insert(
            key("wat"),
            serde_yaml::Value::String(grobid_fixture.display().to_string()),
        );
        spec.insert(key("wasm"), serde_yaml::Value::Null);
        spec.insert(
            key("native"),
            serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
        );
        points.insert(key(point), serde_yaml::Value::Mapping(spec));
    }
    // The mutated registry CLAIMS all ten names are parse-pdf consults.
    let mut claimed_consults: Vec<&str> = SIX_POINTS.to_vec();
    claimed_consults.extend(LATER_NAMES);
    let status = map_mut(&mut registry, "consult_status");
    status.insert(key("parse_pdf_consults"), seq(&claimed_consults));

    let registry_path = write_registry(tmp.path(), &registry, "mutated-consults.yaml");
    let rendered = std::fs::read_to_string(&registry_path).unwrap();

    // The mutated registry PARSES (inert DATA, not a silent loss).
    let parsed = PolicyRegistry::from_yaml_str(&rendered).expect("mutated registry must parse");
    assert_eq!(
        parsed.decision_points.len(),
        SIX_POINTS.len() + LATER_NAMES.len(),
        "extra DATA keys are retained as inert entries"
    );
    assert!(
        parsed.spec(DecisionPoint::GrammarRoute).is_some(),
        "the sanctioned point must survive the mutation"
    );
    // There is no accessor that accepts a later name: the enum is the gate.
    for later in LATER_NAMES {
        assert!(
            DecisionPoint::parse(later).is_none(),
            "`{later}` must not be a DecisionPoint"
        );
    }
    // And the mutated consult list cannot widen the consult surface: the four
    // later names are not decision points at all, and the runtime dispatch still
    // comes from the sanctioned grammar_route artifact — none of the nine
    // poisoned points may decide, nor may any of them add a second consult.
    let listed: Vec<String> = strs(&registry["consult_status"]["parse_pdf_consults"]);
    assert_eq!(listed.len(), SIX_POINTS.len() + LATER_NAMES.len());
    let consultable: Vec<String> = listed
        .iter()
        .filter(|name| DecisionPoint::parse(name.as_str()).is_some())
        .cloned()
        .collect();
    assert_eq!(
        consultable,
        SIX_POINTS
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<_>>(),
        "a later name listed in parse_pdf_consults is still not a consult target"
    );

    let _registry_env = EnvGuard::set("DA_POLICY_REGISTRY", registry_path.to_str().unwrap());
    let (g, chosen) = da_cli::source_cli::resolve_parsed_dispatch(paper)
        .expect("mutated registry must still dispatch through grammar_route");

    assert!(
        CLOSED_ACTIONS.contains(&chosen),
        "action `{chosen}` escaped the closed action set"
    );
    assert_eq!(chosen, "parse-pdf-toc", "page_outline + TOC cache");
    assert_eq!(g.route.as_str(), "page_outline");
    assert!(
        CLOSED_ROUTES.contains(&g.route.as_str()),
        "route `{}` escaped the closed route set",
        g.route.as_str()
    );
    assert_eq!(g.grammar_profile, "page_outline");
    assert_eq!(
        g.policy_hash,
        compiled_wat_hash(&page_outline_fixture),
        "the decision must come from the sanctioned grammar_route artifact"
    );
    assert_ne!(
        g.policy_hash,
        compiled_wat_hash(&grobid_fixture),
        "no poisoned point (7th point, later name, batch_admission, empty native) \
         may decide the dispatch"
    );
}

// ─────────────────────────── T02: unknown route code ───────────────────────────

/// An out-of-range route code is a hard error: the selector yields NO
/// policy-derived action (fail-closed), for every invalid code — never a
/// silent XyCut/default dispatch and never a panic.
#[test]
fn unknown_route_code_never_selects_a_policy_action() {
    let _guard = lock();
    let tmp = tempfile::tempdir().unwrap();
    let paper = "5070.00002";
    write_toc_cache(tmp.path(), paper);
    let _restore = CwdGuard::new(tmp.path());
    let (_pages, _bookmarks, _quality) = pin_features();

    for code in [3_i32, 7, -1, 99] {
        let wat_path = tmp.path().join(format!("grammar_route_code_{code}.wat"));
        let body = "(module (func (export \"route\") (param i32 i32 i32) (result i32) \
                    (i32.const CODE)))\n"
            .replace("CODE", &code.to_string());
        std::fs::write(&wat_path, body).unwrap();

        let mut registry = committed_registry_value();
        set_wat(&mut registry, "grammar_route", &wat_path);
        let registry_path = write_registry(tmp.path(), &registry, &format!("code-{code}.yaml"));
        let _registry_env = EnvGuard::set("DA_POLICY_REGISTRY", registry_path.to_str().unwrap());

        assert!(
            da_cli::source_cli::resolve_parsed_dispatch(paper).is_none(),
            "code {code} must fail closed, not select a policy action"
        );
    }
}

// ─────────────────────────── T03: DATA profile remap ───────────────────────────

/// A sanctioned DATA remap (`profile_map`) can only move the dispatch inside
/// the closed action pair, and the chosen action still depends on the observed
/// TOC cache — a garbage profile name cannot escape to an unknown CLI action.
#[test]
fn profile_map_remap_stays_inside_the_closed_action_set() {
    let _guard = lock();
    let tmp = tempfile::tempdir().unwrap();
    let with_toc = "5070.00003";
    let without_toc = "5070.00004";
    write_toc_cache(tmp.path(), with_toc);
    let _restore = CwdGuard::new(tmp.path());
    let (_pages, _bookmarks, _quality) = pin_features();

    // `grammar_route.wat` selects xy_cut for the pinned features; the DATA remap
    // decides which FSM profile that route maps to.
    let fixture = policy_fixture_path("grammar_route.wat");

    let mut remapped = committed_registry_value();
    set_wat(&mut remapped, "grammar_route", &fixture);
    map_mut(sub_mut(&mut remapped, "decision_points"), "grammar_route")
        .get_mut(key("profile_map"))
        .and_then(|v| v.as_mapping_mut())
        .expect("grammar_route.profile_map")
        .insert(key("xy_cut"), key("page_outline"));
    let remapped_path = write_registry(tmp.path(), &remapped, "remap-page-outline.yaml");

    let _registry_env = EnvGuard::set("DA_POLICY_REGISTRY", remapped_path.to_str().unwrap());

    let (g_toc, action_toc) = da_cli::source_cli::resolve_parsed_dispatch(with_toc)
        .expect("remapped registry must dispatch");
    assert_eq!(g_toc.route.as_str(), "xy_cut");
    assert_eq!(
        g_toc.grammar_profile, "page_outline",
        "DATA remap is honoured"
    );
    assert_eq!(action_toc, "parse-pdf-toc");
    assert!(CLOSED_ACTIONS.contains(&action_toc));

    let (g_no_toc, action_no_toc) = da_cli::source_cli::resolve_parsed_dispatch(without_toc)
        .expect("remapped registry must dispatch");
    assert_eq!(g_no_toc.grammar_profile, "page_outline");
    assert_eq!(
        action_no_toc, "parse-pdf",
        "the remap alone cannot force parse-pdf-toc without the TOC observation"
    );

    // A garbage profile name is DATA, and DATA cannot invent an action.
    let mut garbage = committed_registry_value();
    set_wat(&mut garbage, "grammar_route", &fixture);
    map_mut(sub_mut(&mut garbage, "decision_points"), "grammar_route")
        .get_mut(key("profile_map"))
        .and_then(|v| v.as_mapping_mut())
        .expect("grammar_route.profile_map")
        .insert(key("xy_cut"), key("not_a_profile"));
    let garbage_path = write_registry(tmp.path(), &garbage, "remap-garbage-profile.yaml");
    let _garbage_env = EnvGuard::set("DA_POLICY_REGISTRY", garbage_path.to_str().unwrap());

    let (g_garbage, action_garbage) = da_cli::source_cli::resolve_parsed_dispatch(with_toc)
        .expect("garbage profile must still dispatch to a closed action");
    assert_eq!(g_garbage.grammar_profile, "not_a_profile");
    assert_eq!(action_garbage, "parse-pdf");
    assert!(
        CLOSED_ACTIONS.contains(&action_garbage),
        "`{action_garbage}` escaped the closed action set"
    );
}

// ─────────────────────────── T04: missing grammar_route ───────────────────────────

/// A registry without `grammar_route` falls open to the DATA-declared default
/// profile with `PolicySource::Native` — no wasm claim is invented and the
/// action stays inside the closed pair.
#[test]
fn missing_grammar_route_falls_open_to_parse_pdf() {
    let _guard = lock();
    let tmp = tempfile::tempdir().unwrap();
    let paper = "5070.00005";
    write_toc_cache(tmp.path(), paper);
    let _restore = CwdGuard::new(tmp.path());
    let (_pages, _bookmarks, _quality) = pin_features();

    let mut registry = committed_registry_value();
    map_mut(&mut registry, "decision_points").remove(key("grammar_route"));
    let registry_path = write_registry(tmp.path(), &registry, "no-grammar-route.yaml");
    let _registry_env = EnvGuard::set("DA_POLICY_REGISTRY", registry_path.to_str().unwrap());

    let (g, chosen) = da_cli::source_cli::resolve_parsed_dispatch(paper)
        .expect("absent grammar_route must fall open, not fail the dispatch");

    assert_eq!(chosen, "parse-pdf");
    assert!(
        CLOSED_ACTIONS.contains(&chosen),
        "`{chosen}` escaped the closed action set"
    );
    assert_eq!(
        g.source,
        PolicySource::Native,
        "an absent point must never be stamped as wasm"
    );
    assert_eq!(g.policy_hash, "native");
    assert_eq!(g.grammar_profile, "arxiv_pdf_dot");
    assert!(CLOSED_ROUTES.contains(&g.route.as_str()));
}

// ─────────────────────────── T05: promotion locks ───────────────────────────

fn digest(ch: char) -> Sha256Digest {
    Sha256Digest::new(ch.to_string().repeat(64)).expect("fixed digest")
}

fn variant_manifest(deployed: bool, import_eligible: bool) -> PolicyVariantManifest {
    PolicyVariantManifest {
        variant_id: "grammar-route-always-page-outline".into(),
        policy_hash: digest('7'),
        baseline_policy_hash: digest('6'),
        registry_digest: digest('5'),
        label: "always page_outline".into(),
        promotion_state: PromotionState::Candidate,
        deployed,
        import_eligible,
    }
}

fn frozen_cases() -> Vec<FrozenEvalCase> {
    (0..3)
        .map(|order| FrozenEvalCase {
            case_id: format!("case-{order}"),
            corpus: digest('c'),
            order,
            expected_digest: digest(['0', '1', '2'][order as usize]),
        })
        .collect()
}

fn critic_rows(producer: &str, metrics: [f64; 3]) -> Vec<CriticObservation> {
    metrics
        .iter()
        .enumerate()
        .map(|(order, metric_one)| CriticObservation {
            schema_version: 1,
            case_id: format!("case-{order}"),
            corpus: digest('c'),
            order: order as u32,
            producer: producer.into(),
            metric_one: *metric_one,
            metric_two: *metric_one / 2.0,
            status: CriticStatus::Ok,
        })
        .collect()
}

fn critic_producer(code: char) -> ProducerProvenance {
    ProducerProvenance {
        code_digest: digest(code),
        input_digest: digest('c'),
        method: "policy-dispatch-critic".into(),
        authority: "fixture-authority".into(),
    }
}

/// A policy-variant ablation report can never claim deployment, import
/// eligibility or automatic promotion, and a manifest that claims either lock
/// is rejected with a typed error (never a panic, never a silent report).
#[test]
fn policy_variant_report_keeps_the_promotion_locks() {
    let split = SplitIdentity {
        train: vec![digest('0'), digest('1')],
        validation: vec![digest('2')],
    };
    let quality_policy = PromotionQualityPolicy {
        minimum_validation_quality: 0.5,
        maximum_validation_gap: 0.25,
    };
    let cases = frozen_cases();
    let baseline = critic_rows("dispatch-critic-a", [0.5, 0.5, 0.5]);
    let candidate = critic_rows("dispatch-critic-b", [0.9, 0.7, 0.6]);

    let manifest = variant_manifest(false, false);
    let report = aggregate_policy_variant_ablation(
        &manifest,
        &cases,
        &baseline,
        &candidate,
        true,
        &split,
        &critic_producer('a'),
        &critic_producer('b'),
        quality_policy,
    )
    .expect("candidate-only fixture must aggregate");

    assert_eq!(report.schema_version, POLICY_ABLATION_SCHEMA_VERSION);
    assert_eq!(report.promotion_state, POLICY_ABLATION_PROMOTION_STATE);
    assert_eq!(report.promotion_state, "candidate");
    assert_eq!(report.auto_promote, POLICY_ABLATION_AUTO_PROMOTE);
    assert_eq!(report.auto_promote, "forbidden");
    assert!(!report.deployed, "an ablation report can never deploy");
    assert!(
        !report.import_eligible,
        "an ablation report can never be import-eligible"
    );
    assert_eq!(manifest.promotion_state, PromotionState::Candidate);
    assert!(manifest.validate().is_ok());

    // Either lock claim is rejected before any aggregation happens.
    for (deployed, import_eligible) in [(true, false), (false, true), (true, true)] {
        let locked = variant_manifest(deployed, import_eligible);
        assert_eq!(
            locked.validate(),
            Err(EvaluationError::CandidateLockViolation),
            "deployed={deployed} import_eligible={import_eligible} must be rejected"
        );
        assert_eq!(
            aggregate_policy_variant_ablation(
                &locked,
                &cases,
                &baseline,
                &candidate,
                true,
                &split,
                &critic_producer('a'),
                &critic_producer('b'),
                quality_policy,
            ),
            Err(EvaluationError::CandidateLockViolation),
            "a locked manifest must never produce a report"
        );
    }

    // A promoted/deployed promotion_state is unrepresentable, not merely
    // ignored: deserialization of an injected manifest fails typed.
    let injected = format!(
        r#"{{"variant_id":"v","policy_hash":"{}","baseline_policy_hash":"{}",
             "registry_digest":"{}","label":"l","promotion_state":"deployed",
             "deployed":false,"import_eligible":false}}"#,
        digest('7').as_str(),
        digest('6').as_str(),
        digest('5').as_str()
    );
    let err = serde_json::from_str::<PolicyVariantManifest>(&injected)
        .expect_err("a deployed promotion_state must not deserialize");
    let message = err.to_string();
    assert!(
        message.contains("unknown variant `deployed`") && message.contains("expected `candidate`"),
        "the refusal must be the closed PromotionState enum, got: {message}"
    );
}

// ─────────────────────────── T06: committed DATA untouched ───────────────────────────

/// The committed registry — reached through the DEFAULT production path
/// (`policy_registry_path()` with no override) — still carries the exact closed
/// consult sets, and its bytes are unchanged.
#[test]
fn committed_closure_data_is_untouched() {
    let _guard = lock();

    // Byte identity of the committed registry artifact.
    let bytes = std::fs::read(committed_registry_path()).expect("committed registry readable");
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    assert_eq!(
        hex::encode(hasher.finalize()),
        COMMITTED_REGISTRY_SHA256,
        "data/policies/registry.yaml must stay byte-identical to the slice pin"
    );

    let registry = committed_registry_value();
    let mut registry_copy = registry.clone();
    let status = sub_mut(&mut registry_copy, "consult_status");
    assert_eq!(
        strs(&status["parse_pdf_consults"]),
        vec!["grammar_route".to_string(), "profile_route".to_string()],
        "parse-pdf consults exactly two of the six DecisionPoints"
    );
    assert_eq!(
        strs(&status["empty_native_unknown_point"]),
        vec![
            "concurrency_tune".to_string(),
            "retry_policy".to_string(),
            "summary_depth".to_string()
        ],
        "empty native maps stay UnknownPoint, not live production policy"
    );
    assert_eq!(
        strs(&status["later"]),
        vec![
            "head_fallback".to_string(),
            "visual_bundle".to_string(),
            "critic_sample".to_string()
        ],
        "later names stay later"
    );
    assert_eq!(
        strs(&status["native_rules_not_consulted"]),
        vec!["batch_admission".to_string()],
        "batch_admission is named, never consulted by parse-pdf"
    );

    // decision_points stays exactly the six sanctioned keys.
    let mut declared: Vec<String> = registry
        .get("decision_points")
        .and_then(|v| v.as_mapping())
        .expect("decision_points mapping")
        .keys()
        .map(|k| k.as_str().expect("string key").to_string())
        .collect();
    declared.sort();
    let mut expected: Vec<String> = SIX_POINTS.iter().map(|s| (*s).to_string()).collect();
    expected.sort();
    assert_eq!(declared, expected, "decision_points must stay the six keys");

    // Production default path: no DA_POLICY_REGISTRY override, CWD = repo root,
    // so `policy_registry_path()` resolves `data/policies/registry.yaml`.
    // The asserted outcome is invariant to the ambient TOC cache: the committed
    // fixture selects `xy_cut` for the pinned features, and `xy_cut` maps to
    // `arxiv_pdf_dot` -> `parse-pdf` with or without a TOC observation.
    let paper = "5070.00006";
    let _cwd = CwdGuard::new(&repo_root());
    let _cleared = EnvGuard::clear("DA_POLICY_REGISTRY");
    let (_pages, _bookmarks, _quality) = pin_features();

    let (g, chosen) = da_cli::source_cli::resolve_parsed_dispatch(paper)
        .expect("the committed registry must dispatch through the default path");
    assert!(
        matches!(&g.source, PolicySource::Wasm { .. }),
        "the committed registry points at the committed wasm fixture"
    );
    assert_eq!(chosen, "parse-pdf");
    assert_eq!(g.route.as_str(), "xy_cut");
    assert!(
        CLOSED_ACTIONS.contains(&chosen) && CLOSED_ROUTES.contains(&g.route.as_str()),
        "the committed registry dispatches inside the closed sets"
    );
}
