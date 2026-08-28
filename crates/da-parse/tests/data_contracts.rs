//! Data-contract gate for the scholarly parse plane
//! (M464-z84in2 / S01 / T01 + S02 / T04).
//!
//! Validates contract SHAPE and REFERENTIAL INTEGRITY ONLY for:
//! - `data/parse/api_compatibility_ledger.yaml`
//! - `data/parse/regression_cases.yaml`
//! - bindings resolve against registry ids from data/parse/grobid_compatibility.yaml
//!
//! Since S03/T04 every regression case must additionally require
//! `conservation_delta` typed conservation evidence (sanctioned engine only).
//!
//! Since S02/T04 the gate additionally enforces:
//! - staged-platform seam bindings — ParseBackend / TeiEnvelope / TeiLifter /
//!   ScholarlyParser must stay documented under their EXISTING sanctioned
//!   entry ids (no parallel ledger, no new capability identities, R105);
//! - stage/endpoint entries must cite a resolvable registry-of-record
//!   capability id via a `registry_binding:` note;
//! - the registry `stage.tei_lift` capability keeps binding the lift stage
//!   to the da-parse lifter surface (`da-parse@lift-v1` candidate identity,
//!   `preserved-java-tei` baseline identity).
//!
//! Scope guards honored by construction: this binary contacts no network
//! service, parses no corpus document through the parser pipeline, touches no
//! graph store, and depends on no wall-clock state. Project hard locks are
//! asserted verbatim on both manifests, and every defect must FAIL LOUDLY —
//! the silent skip is exactly the regression class this slice removes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use da_parse::GrobidProfile;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const LEDGER_REL_PATH: &str = "data/parse/api_compatibility_ledger.yaml";
const REGRESSION_REL_PATH: &str = "data/parse/regression_cases.yaml";
/// Registry of record for R113/R117 compatibility states (loaded generically).
const REGISTRY_REL_PATH: &str = "data/parse/grobid_compatibility.yaml";

/// Canonical cache/configuration fingerprints (D204). The positive tests bind
/// these strings to the LIVE API values, so any future drift in
/// `GrobidProfile::configuration_fingerprint` must update both manifests or
/// this gate fails.
const RICH_FP: &str = "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula";
const STRUCTURAL_FP: &str = "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula";

/// Ids that MUST exist in the ledger for every later slice task to resolve.
const ANCHOR_ENTRY_IDS: [&str; 8] = [
    "parse.entrypoint.grobid_parser.parse_pdf",
    "parse.constructor.grobid_parser",
    "parse.probe.is_alive",
    "parse.endpoint.process_fulltext_document",
    "parse.stage.lift_tei",
    "parse.stage.project_article",
    "parse.profile.rich",
    "parse.profile.structural",
];

/// Allowed vocabulary for `required_evidence_fields`.
const EVIDENCE_VOCAB: [&str; 10] = [
    "source_sha256",
    "source_path",
    "profile_fingerprint",
    "request_fields_snapshot",
    "tei_output_sha256",
    "family_counts",
    "lifted_counts",
    "conservation_delta",
    "latency_ms_when_executable",
    "known_regressions",
];

/// Every case must request at least this evidence core.
const MIN_EVIDENCE_FIELDS: [&str; 3] = ["source_sha256", "source_path", "profile_fingerprint"];

// ───────────────────────────── data model ─────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PolicyLocks {
    import_eligible: bool,
    dspy_enabled: bool,
    graph_mutation: String,
    llm_promotion: String,
    free_label_invention: String,
    sidecar_status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ProfileName {
    Rich,
    Structural,
}

impl ProfileName {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "rich" => Some(Self::Rich),
            "structural" => Some(Self::Structural),
            _ => None,
        }
    }
    fn canonical_fp(self) -> &'static str {
        match self {
            Self::Rich => RICH_FP,
            Self::Structural => STRUCTURAL_FP,
        }
    }
}

impl ProfileName {
    fn label(self) -> &'static str {
        match self {
            Self::Rich => "rich",
            Self::Structural => "structural",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EntryKind {
    Entrypoint,
    Constructor,
    Probe,
    Endpoint,
    LiftStage,
    ProjectStage,
    ProfileContract,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Stability {
    Stable,
    InternalObservable,
}

#[derive(Debug, Clone, Deserialize)]
enum HttpMethod {
    #[allow(clippy::upper_case_acronyms)]
    GET,
    #[allow(clippy::upper_case_acronyms)]
    POST,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerEntry {
    id: String,
    kind: EntryKind,
    owner_crate: String,
    visibility: String,
    #[allow(dead_code)]
    stability: Stability,
    entry_point: Option<String>,
    #[serde(default)]
    #[allow(dead_code)] // mirrors observable API even where this gate reads a subset
    implemented_by: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    port_trait: Option<String>,
    request_shape: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    input_kind: Option<String>,
    result_shape: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    error_shapes: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    async_behavior: Option<String>,
    http_endpoint: Option<String>,
    http_method: Option<HttpMethod>,
    #[serde(default)]
    request_fields: Vec<String>,
    profile_name: Option<String>,
    configuration_fingerprint: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    notes: Vec<String>,
    compatibility_obligation: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerFile {
    ledger_version: u32,
    #[allow(dead_code)]
    scope_note: String,
    policy_locks: PolicyLocks,
    entries: Vec<LedgerEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SourceKind {
    PreservedGrobidTei,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BindingStatus {
    Bound,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Stage {
    HttpFetchTei,
    TeiLiftIr,
    IrProjectArticle,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ExecutionMode {
    OfflineLocal,
    RequiresGrobidService,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    ParseOk,
    TypedMissingnessDocumented,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRef {
    path: String,
    #[serde(default)]
    sha256: Option<String>,
    origin: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    #[allow(dead_code)]
    source_kind: SourceKind,
    fixture: FixtureRef,
    binding_status: BindingStatus,
    #[serde(default)]
    binding_owner_task: Option<String>,
    grobid_profile: ProfileName,
    expected_cache_config_fingerprint: String,
    stages: Vec<Stage>,
    stage_execution_mode: ExecutionMode,
    #[allow(dead_code)]
    expected_outcome: Outcome,
    required_evidence_fields: Vec<String>,
    ledger_bindings: Vec<String>,
    obligation_acknowledged: bool,
    #[serde(default)]
    notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegressionManifest {
    manifest_version: u32,
    #[allow(dead_code)]
    scope_note: String,
    policy_locks: PolicyLocks,
    profile_fingerprint_binding_required: bool,
    cache_key_separation_rule: String,
    cases: Vec<Case>,
}

// Custom Deserialize for ProfileName (string-tagged).
mod profile_name_serde {
    use super::ProfileName;
    use serde::Deserialize;
    use serde::de::{self, Deserializer};
    impl<'de> Deserialize<'de> for ProfileName {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let s = String::deserialize(d)?;
            ProfileName::parse(&s)
                .ok_or_else(|| de::Error::custom(format!("unknown profile `{s}`")))
        }
    }
}

// ─────────────────────────── loading helpers ──────────────────────────

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn load_yaml<T: for<'de> Deserialize<'de>>(rel: &str) -> Result<T, String> {
    let full = repo_root().join(rel);
    let text =
        std::fs::read_to_string(&full).map_err(|e| format!("[unreadable-artifact] {rel}: {e}"))?;
    serde_yaml::from_str(&text).map_err(|e| format!("[malformed-yaml] {rel}: {e}"))
}

fn expect_loaded<T: for<'de> Deserialize<'de>>(rel: &str) -> T {
    match load_yaml::<T>(rel) {
        Ok(v) => v,
        Err(e) => panic!("data-contract artifact must load cleanly: {e}"),
    }
}

fn expected_locks() -> PolicyLocks {
    PolicyLocks {
        import_eligible: false,
        dspy_enabled: false,
        graph_mutation: "forbidden".to_string(),
        llm_promotion: "forbidden".to_string(),
        free_label_invention: "forbidden".to_string(),
        sidecar_status: "candidate_evidence_only".to_string(),
    }
}

fn valid_case_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-' || c == '_'
        })
}

// ─────────────────────────── ledger validation ────────────────────────

fn validate_ledger(l: &LedgerFile) -> Vec<String> {
    let mut errs = Vec::new();

    if l.policy_locks != expected_locks() {
        errs.push("[hard-lock-violation] ledger policy_locks diverge from project locks".into());
    }
    if l.ledger_version != 1 {
        errs.push("[ledger-version-drift] expected ledger_version 1".into());
    }

    let mut seen: HashSet<String> = HashSet::new();
    for e in &l.entries {
        if !valid_case_id(&e.id) {
            errs.push(format!("[bad-entry-id] `{}`", e.id));
        }
        if !seen.insert(e.id.clone()) {
            errs.push(format!("[dup-id] duplicate ledger entry id `{}`", e.id));
        }
        if e.owner_crate.is_empty() || e.visibility.is_empty() {
            errs.push(format!(
                "[entry-shape] {} missing owner_crate/visibility",
                e.id
            ));
        }
        if e.compatibility_obligation.trim().is_empty() {
            errs.push(format!(
                "[empty-obligation] {} has no compatibility obligation",
                e.id
            ));
        }
        // Kind-specific observable-shape requirements.
        let needs_entry_point = matches!(
            e.kind,
            EntryKind::Entrypoint
                | EntryKind::Constructor
                | EntryKind::Probe
                | EntryKind::LiftStage
                | EntryKind::ProjectStage
                | EntryKind::ProfileContract
        );
        if needs_entry_point
            && e.entry_point
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            errs.push(format!("[entry-shape] {} kind requires entry_point", e.id));
        }
        let needs_io_shapes = matches!(
            e.kind,
            EntryKind::Entrypoint | EntryKind::LiftStage | EntryKind::ProjectStage
        );
        if needs_io_shapes && e.request_shape.as_deref().is_none_or(str::is_empty) {
            errs.push(format!("[entry-shape] {} must declare request_shape", e.id));
        }
        if needs_io_shapes && e.result_shape.as_deref().is_none_or(str::is_empty) {
            errs.push(format!("[entry-shape] {} must declare result_shape", e.id));
        }
        let forbids_fingerprint = matches!(
            e.kind,
            EntryKind::Entrypoint
                | EntryKind::Constructor
                | EntryKind::LiftStage
                | EntryKind::ProjectStage
        );
        if forbids_fingerprint && e.configuration_fingerprint.is_some() {
            errs.push(format!(
                "[entry-shape] {} must not carry a fingerprint outside profile contracts",
                e.id
            ));
        }
        match e.kind {
            EntryKind::Probe | EntryKind::Endpoint => {
                if !e
                    .http_endpoint
                    .as_deref()
                    .is_some_and(|ep| ep.starts_with('/'))
                {
                    errs.push(format!(
                        "[entry-shape] {} must declare an http_endpoint",
                        e.id
                    ));
                }
                if e.http_method.is_none() {
                    errs.push(format!("[entry-shape] {} must declare http_method", e.id));
                }
            }
            _ => {}
        }
        if e.kind == EntryKind::Endpoint && e.request_fields.is_empty() {
            errs.push(format!(
                "[entry-shape] endpoint {} must list request_fields",
                e.id
            ));
        }
        if let Some(name) = &e.profile_name {
            let parsed = ProfileName::parse(name);
            if parsed.is_none() {
                errs.push(format!("[bad-profile-name] {}", e.id));
            }
            if let Some(pn) = parsed
                && !e
                    .configuration_fingerprint
                    .as_deref()
                    .is_some_and(|fp| fp == pn.canonical_fp())
            {
                errs.push(format!(
                    "[ledger-fingerprint-drift] {} declares `{:?}` but live API identity is `{}`",
                    e.id,
                    e.configuration_fingerprint,
                    pn.canonical_fp()
                ));
            }
        }
    }

    for anchor in ANCHOR_ENTRY_IDS {
        if !seen.contains(anchor) {
            errs.push(format!("[missing-anchor-entry] ledger lost `{anchor}`"));
        }
    }
    errs
}

fn ledger_id_set(l: &LedgerFile) -> HashSet<String> {
    l.entries.iter().map(|e| e.id.clone()).collect()
}

fn ledger_profile_fingerprints(l: &LedgerFile) -> HashMap<ProfileName, String> {
    let mut map = HashMap::new();
    for e in &l.entries {
        if let Some(pn) = e.profile_name.as_deref().and_then(ProfileName::parse)
            && let Some(fp) = &e.configuration_fingerprint
        {
            map.insert(pn, fp.clone());
        }
    }
    map
}

// ─────────────── staged-platform seam bindings (S02/T04) ─────────────

/// S02 staged-platform seams bound to EXISTING sanctioned entry ids (R105):
/// no parallel ledger, no new capability identities. Each seam must remain
/// documented under its entry, with the named symbol evidence present in the
/// entry's observable text (entry_point / request_shape / notes).
const SEAM_BINDINGS: [(&str, &str, [&str; 2]); 4] = [
    (
        "ParseBackend",
        "parse.endpoint.process_fulltext_document",
        ["ParseBackend", "TeiEnvelope"],
    ),
    (
        "TeiEnvelope",
        "parse.stage.lift_tei",
        ["TeiEnvelope", "TeiLifter"],
    ),
    (
        "TeiLifter",
        "parse.stage.lift_tei",
        ["TeiLifter", "TeiEnvelope"],
    ),
    (
        "ScholarlyParser",
        "parse.entrypoint.grobid_parser.parse_pdf",
        ["ScholarlyParser", "ParseBackend"],
    ),
];

/// Entry kinds that must cite a registry-of-record capability id via a
/// `registry_binding:` note (referential integrity INTO the registry — the
/// ledger is an observation input, never a competing namespace).
const REGISTRY_BINDING_KINDS: [EntryKind; 3] = [
    EntryKind::LiftStage,
    EntryKind::ProjectStage,
    EntryKind::Endpoint,
];

/// All observable text of an entry where seam symbols must appear.
fn entry_observable_text(e: &LedgerEntry) -> String {
    let mut text = String::new();
    if let Some(ep) = &e.entry_point {
        text.push_str(ep);
        text.push(' ');
    }
    if let Some(shape) = &e.request_shape {
        text.push_str(shape);
        text.push(' ');
    }
    for note in &e.notes {
        text.push_str(note);
        text.push(' ');
    }
    text
}

/// Every staged-platform seam must stay bound to its sanctioned entry id.
fn validate_seam_bindings(l: &LedgerFile) -> Vec<String> {
    let mut errs = Vec::new();
    for (seam, entry_id, required_symbols) in SEAM_BINDINGS {
        let Some(entry) = l.entries.iter().find(|e| e.id == entry_id) else {
            errs.push(format!(
                "[seam-binding-missing] seam `{seam}`: entry `{entry_id}` vanished from the ledger"
            ));
            continue;
        };
        let text = entry_observable_text(entry);
        for symbol in required_symbols {
            if !text.contains(symbol) {
                errs.push(format!(
                    "[seam-binding-missing] seam `{seam}`: entry `{entry_id}` no longer documents `{symbol}`"
                ));
            }
        }
    }
    errs
}

/// Stage/endpoint entries must cite >=1 existing registry capability id via
/// a `registry_binding:` note.
fn validate_registry_binding_notes(l: &LedgerFile, registry_ids: &HashSet<String>) -> Vec<String> {
    let mut errs = Vec::new();
    for e in &l.entries {
        if !REGISTRY_BINDING_KINDS.contains(&e.kind) {
            continue;
        }
        let Some(note) = e
            .notes
            .iter()
            .find(|n| n.trim_start().starts_with("registry_binding:"))
        else {
            errs.push(format!(
                "[missing-registry-binding-note] {} must cite a registry capability via `registry_binding:`",
                e.id
            ));
            continue;
        };
        let tokens: Vec<&str> = note
            .split(':')
            .nth(1)
            .unwrap_or("")
            .split(|c: char| c == ';' || c == ',' || c.is_whitespace())
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.is_empty() {
            errs.push(format!(
                "[missing-registry-binding-note] {}: empty registry_binding note",
                e.id
            ));
            continue;
        }
        let mut resolved = 0usize;
        for token in tokens {
            if !token.contains('.') {
                continue;
            }
            if registry_ids.contains(token) {
                resolved += 1;
            } else {
                errs.push(format!(
                    "[unresolved-registry-binding-note] {}: `{token}` is not a registry capability id",
                    e.id
                ));
            }
        }
        if resolved == 0 {
            errs.push(format!(
                "[missing-registry-binding-note] {}: no resolvable registry capability cited",
                e.id
            ));
        }
    }
    errs
}

/// Full registry of record loaded generically (R117 generic reader).
fn load_registry_value() -> serde_yaml::Value {
    let full = repo_root().join(REGISTRY_REL_PATH);
    let text = std::fs::read_to_string(&full)
        .unwrap_or_else(|e| panic!("registry of record unreadable: {e}"));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("registry yaml malformed: {e}"))
}

/// The registry-of-record `stage.tei_lift` capability must keep binding the
/// lift stage to the da-parse lifter surface and its sanctioned identities.
fn validate_registry_stage_binding(registry: &serde_yaml::Value) -> Vec<String> {
    const DRIFT: &str = "[registry-stage-binding-drift]";
    let mut errs = Vec::new();
    let Some(cap) = registry
        .get("capabilities")
        .and_then(|c| c.as_sequence())
        .and_then(|seq| {
            seq.iter()
                .find(|c| c.get("id").and_then(|i| i.as_str()) == Some("stage.tei_lift"))
        })
    else {
        errs.push(format!("{DRIFT} registry lost capability `stage.tei_lift`"));
        return errs;
    };
    let str_at = |node: &serde_yaml::Value, path: &[&str]| -> String {
        let mut cur = node;
        for key in path {
            cur = match cur.get(key) {
                Some(v) => v,
                None => return String::new(),
            };
        }
        cur.as_str().unwrap_or_default().to_string()
    };
    if str_at(cap, &["implementation", "crate"]) != "da-parse" {
        errs.push(format!(
            "{DRIFT} stage.tei_lift implementation.crate is no longer da-parse"
        ));
    }
    if !str_at(cap, &["implementation", "surface"]).contains("crates/da-parse/src/lift.rs") {
        errs.push(format!(
            "{DRIFT} stage.tei_lift surface must stay on crates/da-parse/src/lift.rs"
        ));
    }
    if str_at(cap, &["baseline"]) != "preserved-java-tei" {
        errs.push(format!("{DRIFT} stage.tei_lift baseline identity changed"));
    }
    if str_at(cap, &["candidate"]) != "da-parse@lift-v1" {
        errs.push(format!("{DRIFT} stage.tei_lift candidate identity rotated"));
    }
    errs
}

// ────────────────────────── manifest validation ───────────────────────

fn check_fixture_path_shape(path: &str, rel_case: &str, errs: &mut Vec<String>) {
    if path.starts_with('/') || path.len() > 1 && path.as_bytes()[1] == b':' {
        errs.push(format!("[fixture-path-absolute] {rel_case}: `{path}`"));
        return;
    }
    let comps: Vec<&str> = path.split('/').collect();
    if comps.contains(&"..") {
        errs.push(format!("[fixture-path-escape] {rel_case}: `{path}`"));
        return;
    }
    let root_ok = matches!(comps.first().copied(), Some("data") | Some("crates"));
    if !root_ok {
        errs.push(format!("[fixture-path-root] {rel_case}: `{path}`"));
    }
    if comps.len() < 2 || comps.last().is_none_or(|c| c.is_empty()) {
        errs.push(format!("[fixture-path-empty-name] {rel_case}: `{path}`"));
    }
}

/// Registry of record ids loaded generically (R117 generic reader).
fn load_registry_ids() -> HashSet<String> {
    let full = repo_root().join(REGISTRY_REL_PATH);
    let text = std::fs::read_to_string(&full)
        .unwrap_or_else(|e| panic!("registry of record unreadable: {e}"));
    let value: serde_yaml::Value =
        serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("registry yaml malformed: {e}"));
    let mut ids = HashSet::new();
    if let Some(caps) = value.get("capabilities").and_then(|c| c.as_sequence()) {
        for cap in caps {
            if let Some(id) = cap.get("id").and_then(|i| i.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

/// Shared reference sets handed to the manifest validator.
#[derive(Debug, Clone)]
struct ValidationRefs {
    ledger_ids: HashSet<String>,
    registry_ids: HashSet<String>,
    fps: HashMap<ProfileName, String>,
}

fn refs_from(l: &LedgerFile) -> ValidationRefs {
    ValidationRefs {
        ledger_ids: ledger_id_set(l),
        registry_ids: load_registry_ids(),
        fps: ledger_profile_fingerprints(l),
    }
}

fn validate_manifest(m: &RegressionManifest, refs: &ValidationRefs) -> Vec<String> {
    let mut errs = Vec::new();

    if m.policy_locks != expected_locks() {
        errs.push("[hard-lock-violation] manifest policy_locks diverge from project locks".into());
    }
    if m.manifest_version != 1 {
        errs.push("[manifest-version-drift] expected manifest_version 1".into());
    }
    if !m.profile_fingerprint_binding_required {
        errs.push("[binding-required-flag] profile fingerprint binding must be required".into());
    }
    if !m.cache_key_separation_rule.contains("D204") {
        errs.push("[cache-rule-drift] separation rule must cite D204".into());
    }
    if m.cases.is_empty() {
        errs.push("[empty-cases] regression manifest carries no cases".into());
        return errs;
    }

    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut rich_seen = false;
    let mut structural_seen = false;

    for c in &m.cases {
        let ctx = format!("case `{}`", c.id);
        if !c.id.starts_with("reg.") || !valid_case_id(&c.id) {
            errs.push(format!("[bad-case-id] {ctx}"));
        }
        if !seen_ids.insert(c.id.clone()) {
            errs.push(format!("[dup-id] duplicate regression case id `{}`", c.id));
        }

        check_fixture_path_shape(&c.fixture.path, &ctx, &mut errs);
        if !c.fixture.path.ends_with(".xml") {
            errs.push(format!(
                "[fixture-path-extension] {ctx}: preserved TEI fixtures are .xml"
            ));
        }
        if c.fixture.origin.trim().chars().count() < 24 {
            errs.push(format!(
                "[fixture-origin-thin] {ctx}: state where the bytes come from"
            ));
        }
        match (&c.binding_status, &c.fixture.sha256) {
            (BindingStatus::Bound, Some(sha)) => {
                if sha.len() != 64
                    || !sha
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                {
                    errs.push(format!("[bad-sha256] {ctx}"));
                }
                let full = repo_root().join(&c.fixture.path);
                match std::fs::read(&full) {
                    Ok(body) if body.is_empty() => {
                        errs.push(format!("[empty-fixture-file] {ctx}: {}", c.fixture.path))
                    }
                    Ok(body) => {
                        let actual = hex::encode(Sha256::digest(&body));
                        if actual != *sha {
                            errs.push(format!(
                                "[fixture-sha256-mismatch] {ctx}: declared `{sha}` actual `{actual}`"
                            ));
                        }
                    }
                    Err(e) => errs.push(format!(
                        "[missing-fixture-file] {ctx}: {} ({e})",
                        c.fixture.path
                    )),
                }
            }
            (BindingStatus::Bound, None) => {
                errs.push(format!("[bound-without-sha256] {ctx}"));
            }
            (BindingStatus::Pending, Some(_)) => {
                errs.push(format!(
                    "[pending-with-sha256] {ctx}: checksums arrive with binding"
                ));
            }
            (BindingStatus::Pending, None) => {
                if c.binding_owner_task
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
                {
                    errs.push(format!(
                        "[pending-unowned] {ctx}: declare binding_owner_task"
                    ));
                }
                if c.notes.iter().all(|n| n.trim().is_empty()) {
                    errs.push(format!(
                        "[pending-requires-note] {ctx}: document the binding state"
                    ));
                }
            }
        }

        if c.expected_cache_config_fingerprint != c.grobid_profile.canonical_fp() {
            errs.push(format!(
                "[fingerprint-profile-mismatch] {ctx}: profile `{}` expects `{}` but declared `{}`",
                c.grobid_profile.label(),
                c.grobid_profile.canonical_fp(),
                c.expected_cache_config_fingerprint
            ));
        }
        if let Some(ledger_fp) = refs.fps.get(&c.grobid_profile) {
            if ledger_fp.as_str() != c.grobid_profile.canonical_fp() {
                errs.push(format!(
                    "[ledger-fingerprint-drift] {ctx}: ledger disagrees with live identity for profile `{}`",
                    c.grobid_profile.label()
                ));
            }
        } else {
            errs.push(format!(
                "[missing-ledger-profile-contract] {ctx}: no ledger profile_contract for `{}`",
                c.grobid_profile.label()
            ));
        }

        if c.stages.is_empty() {
            errs.push(format!("[no-stages] {ctx}"));
        }
        let has_http_stage = c.stages.contains(&Stage::HttpFetchTei);
        match c.stage_execution_mode {
            ExecutionMode::OfflineLocal if has_http_stage => {
                errs.push(format!(
                    "[stage-mode-conflict] {ctx}: offline case cannot fetch TEI over HTTP"
                ));
            }
            ExecutionMode::RequiresGrobidService if !has_http_stage => {
                errs.push(format!(
                    "[stage-mode-conflict] {ctx}: service-mode case must include http_fetch_tei"
                ));
            }
            _ => {}
        }

        if c.required_evidence_fields.is_empty() {
            errs.push(format!("[missing-evidence-fields] {ctx}: none declared"));
        }
        for f in &c.required_evidence_fields {
            if !EVIDENCE_VOCAB.contains(&f.as_str()) {
                errs.push(format!("[unknown-evidence-field] {ctx}: `{f}`"));
            }
        }
        for f in MIN_EVIDENCE_FIELDS {
            if !c.required_evidence_fields.contains(&f.to_string()) {
                errs.push(format!(
                    "[missing-evidence-fields] {ctx}: lacks required `{f}`"
                ));
            }
        }

        if c.ledger_bindings.is_empty() {
            errs.push(format!(
                "[unbound-case] {ctx}: no compatibility-ledger binding"
            ));
        }
        let mut has_registry_ref = false;
        for b in &c.ledger_bindings {
            if !refs.ledger_ids.contains(b) && !refs.registry_ids.contains(b) {
                errs.push(format!("[unresolved-ledger-ref] {ctx}: `{b}`"));
            }
            if refs.registry_ids.contains(b) {
                has_registry_ref = true;
            }
        }
        if !has_registry_ref {
            errs.push(format!(
                "[missing-registry-binding] {ctx}: cite >=1 grobid_compatibility capability"
            ));
        }
        if !c.obligation_acknowledged {
            errs.push(format!("[obligation-unacknowledged] {ctx}"));
        }

        match c.grobid_profile {
            ProfileName::Rich => rich_seen = true,
            ProfileName::Structural => structural_seen = true,
        }
    }

    if !(rich_seen && structural_seen) {
        errs.push(
            "[profile-coverage] manifest must keep structural and rich profiles represented distinctly"
                .into(),
        );
    }
    errs
}

// ───────────────────────────── positives ──────────────────────────────

#[test]
fn ledger_loads_and_passes_contract_validation() {
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let errs = validate_ledger(&ledger);
    assert!(
        errs.is_empty(),
        "ledger contract violations:\n{}",
        errs.join("\n")
    );
}

#[test]
fn regression_manifest_passes_contract_validation_against_the_ledger() {
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let errs = validate_manifest(&manifest, &refs_from(&ledger));
    assert!(
        errs.is_empty(),
        "regression-manifest contract violations:\n{}",
        errs.join("\n")
    );
}

#[test]
fn ledger_fingerprints_equal_the_live_api_values() {
    assert_eq!(GrobidProfile::rich().configuration_fingerprint(), RICH_FP);
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        STRUCTURAL_FP
    );
    assert_eq!(
        GrobidProfile::tei_coordinates_json(),
        "[\"figure\",\"table\",\"note\",\"formula\"]"
    );
}

#[test]
fn hard_locks_are_conserved_across_both_data_contracts() {
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let locks = expected_locks();
    assert_eq!(ledger.policy_locks, locks, "ledger drifted from hard locks");
    assert_eq!(
        manifest.policy_locks, locks,
        "manifest drifted from hard locks"
    );
    assert!(
        manifest.cache_key_separation_rule.contains("D204"),
        "cache separation rationale must cite D204"
    );
    assert!(manifest.profile_fingerprint_binding_required);
}

#[test]
fn structural_and_rich_profiles_stay_distinct_in_the_bounded_set() {
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let fps: HashSet<&str> = manifest
        .cases
        .iter()
        .map(|c| c.expected_cache_config_fingerprint.as_str())
        .collect();
    assert!(
        fps.contains(RICH_FP) && fps.contains(STRUCTURAL_FP),
        "both distinct identities must appear: {fps:?}"
    );
}

// ───────────────────────────── negatives ──────────────────────────────

fn loaded_pair() -> (LedgerFile, RegressionManifest) {
    (
        expect_loaded(LEDGER_REL_PATH),
        expect_loaded(REGRESSION_REL_PATH),
    )
}

fn negative(tag: &str, errs: &[String], where_: &str) {
    assert!(
        errs.iter().any(|e| e.contains(tag)),
        "expected `{tag}` for {where_}, got: {:?}",
        errs
    );
}

#[test]
fn rejects_duplicate_case_ids() {
    let (ledger, mut m) = loaded_pair();
    m.cases.push(m.cases[0].clone());
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative("[dup-id]", &e, "duplicate case id");
}

#[test]
fn rejects_unresolved_ledger_binding_reference() {
    let (ledger, mut m) = loaded_pair();
    m.cases[0]
        .ledger_bindings
        .push("parse.stage.invented_native_superparser".to_string());
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative("[unresolved-ledger-ref]", &e, "unresolved ledger ref");
}

#[test]
fn rejects_case_without_registry_capability_binding() {
    let (ledger, mut m) = loaded_pair();
    m.cases[0].ledger_bindings.retain(|b| {
        !(b.starts_with("rest.") || b.starts_with("dialect.") || b.starts_with("stage."))
    });
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative(
        "[missing-registry-binding]",
        &e,
        "stripped registry binding",
    );
}

#[test]
fn rejects_unacknowledged_compatibility_obligation() {
    let (ledger, mut m) = loaded_pair();
    m.cases[0].obligation_acknowledged = false;
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative(
        "[obligation-unacknowledged]",
        &e,
        "unacknowledged obligation",
    );
}

#[test]
fn rejects_bound_case_without_sha256_checksum() {
    let (ledger, mut m) = loaded_pair();
    let idx = m
        .cases
        .iter()
        .position(|c| c.binding_status == BindingStatus::Pending)
        .expect("at least one pending case");
    m.cases[idx].binding_status = BindingStatus::Bound;
    m.cases[idx].fixture.sha256 = None;
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative("[bound-without-sha256]", &e, "bound without checksum");
}

#[test]
fn rejects_bound_case_pointing_at_missing_fixture_file() {
    let (ledger, mut m) = loaded_pair();
    let idx = m
        .cases
        .iter()
        .position(|c| c.binding_status == BindingStatus::Pending)
        .expect("at least one pending case");
    m.cases[idx].fixture.path = "data/__contract_negatives__/absent_2507.tei.xml".to_string();
    m.cases[idx].binding_status = BindingStatus::Bound;
    m.cases[idx].fixture.sha256 = Some("a".repeat(64));
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative("[missing-fixture-file]", &e, "absent bound fixture");
}

#[test]
fn rejects_pending_case_carrying_a_premature_checksum() {
    let (ledger, mut m) = loaded_pair();
    let idx = m
        .cases
        .iter()
        .position(|c| c.binding_status == BindingStatus::Pending)
        .expect("at least one pending case");
    m.cases[idx].fixture.sha256 = Some("b".repeat(64));
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative("[pending-with-sha256]", &e, "premature checksum");
}

#[test]
fn rejects_bound_case_whose_bytes_do_not_match_declared_sha256() {
    let (ledger, mut m) = loaded_pair();
    let idx = m
        .cases
        .iter()
        .position(|c| c.binding_status == BindingStatus::Bound)
        .expect("at least one bound case");
    m.cases[idx].fixture.sha256 = Some("c".repeat(64));
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative(
        "[fixture-sha256-mismatch]",
        &e,
        "bound checksum disagrees with bytes",
    );
}

#[test]
fn rejects_fingerprint_profile_mismatch_on_cases() {
    let (ledger, mut m) = loaded_pair();
    let idx = m
        .cases
        .iter()
        .position(|c| c.grobid_profile == ProfileName::Structural)
        .expect("at least one structural case");
    m.cases[idx].expected_cache_config_fingerprint = RICH_FP.to_string();
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative(
        "[fingerprint-profile-mismatch]",
        &e,
        "cross-wired fingerprint",
    );
}

#[test]
fn rejects_unknown_and_missing_required_evidence_fields() {
    let (ledger, mut m) = loaded_pair();
    m.cases[0]
        .required_evidence_fields
        .push("vibes".to_string());
    m.cases[1]
        .required_evidence_fields
        .retain(|f| f != "source_sha256");
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative("[unknown-evidence-field]", &e, "unknown evidence field");
    negative(
        "[missing-evidence-fields]",
        &e,
        "dropped core evidence field",
    );
}

#[test]
fn rejects_fixture_path_escape_attempts() {
    let (ledger, mut m) = loaded_pair();
    m.cases[0].fixture.path = "crates/../data/evil.tei.xml".to_string();
    m.cases[1].fixture.path = "/etc/passwd".to_string();
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative("[fixture-path-escape]", &e, "traversal escape");
    negative("[fixture-path-absolute]", &e, "absolute path");
}

#[test]
fn rejects_stage_mode_conflicts() {
    let (ledger, mut m) = loaded_pair();
    let idx = m
        .cases
        .iter()
        .position(|c| c.stage_execution_mode == ExecutionMode::OfflineLocal)
        .expect("at least one offline case");
    m.cases[idx].stages.push(Stage::HttpFetchTei);
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative("[stage-mode-conflict]", &e, "HTTP stage under offline mode");
}

#[test]
fn rejects_hard_lock_relaxation_in_the_manifest() {
    let (ledger, mut m) = loaded_pair();
    m.policy_locks.import_eligible = true;
    let e = validate_manifest(&m, &refs_from(&ledger));
    negative("[hard-lock-violation]", &e, "import_eligible=true attempt");
}

#[test]
fn rejects_empty_compatibility_obligation_in_the_ledger() {
    let (mut ledger, _) = loaded_pair();
    ledger.entries[0].compatibility_obligation = "   ".to_string();
    let e = validate_ledger(&ledger);
    negative("[empty-obligation]", &e, "blank obligation");
}

#[test]
fn rejects_duplicate_and_missing_anchor_entries_in_the_ledger() {
    let (mut ledger, _) = loaded_pair();
    let clone = ledger.entries[0].clone();
    ledger.entries.push(clone);
    let dup = validate_ledger(&ledger);
    negative("[dup-id]", &dup, "duplicate entry id");

    let (mut fresh, _) = loaded_pair();
    fresh.entries.remove(0);
    let anchor = validate_ledger(&fresh);
    negative("[missing-anchor-entry]", &anchor, "removed anchor entry");
}

#[test]
fn rejects_ledger_fingerprint_drift_from_the_canonical_identity() {
    let (mut ledger, _) = loaded_pair();
    let idx = ledger
        .entries
        .iter()
        .position(|e| e.profile_name.as_deref() == Some("structural"))
        .expect("structural profile contract present");
    ledger.entries[idx].configuration_fingerprint = Some("profile=structural;consolidateHeader=1;consolidateCitations=0;teiCoordinates=figure,table,note".to_string());
    let e = validate_ledger(&ledger);
    negative("[ledger-fingerprint-drift]", &e, "tampered fingerprint");
}

#[test]
fn unknown_fields_are_denied_on_both_artifact_shapes() {
    let bad_ledger = "ledger_version: 1\nbogus_top_level: true\nscope_note: x\n";
    match serde_yaml::from_str::<LedgerFile>(bad_ledger) {
        Err(e) => assert!(
            e.to_string().contains("unknown field"),
            "deny_unknown_fields should trigger, got: {e}"
        ),
        Ok(_) => panic!("malformed ledger shape must not deserialize"),
    }
    let bad_manifest = "manifest_version: 1\nsomething_else: oops\nscope_note: x\n";
    match serde_yaml::from_str::<RegressionManifest>(bad_manifest) {
        Err(e) => assert!(
            e.to_string().contains("unknown field"),
            "deny_unknown_fields should trigger, got: {e}"
        ),
        Ok(_) => panic!("malformed manifest shape must not deserialize"),
    }
}

// ─────────────── staged-platform seam checks (S02/T04) ───────────────

#[test]
fn staged_platform_seams_are_public_api_with_exact_provenance() {
    // Compile-time export proof for the R105 seam set that the DATA
    // bindings above reference: the provider-neutral types resolve from the
    // crate root and keep their documented identity accessors.
    let envelope = da_parse::TeiEnvelope::new(
        b"<TEI/>".to_vec(),
        da_parse::TeiIdentity::cached_tei(GrobidProfile::structural()),
    );
    assert_eq!(
        envelope.identity().backend_id(),
        da_parse::CACHED_TEI_BACKEND_ID
    );
    assert_eq!(envelope.identity().profile_fingerprint(), STRUCTURAL_FP);
    let _lifter = da_parse::TeiLifter::new();
    let _request = da_parse::ParseBackendRequest::new("paper", b"pdf".to_vec());

    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<da_parse::ScholarlyParser<da_parse::CachedTeiBackend>>();
    assert_send_sync::<da_parse::CachedTeiBackend>();
}

#[test]
fn seams_stay_bound_to_sanctioned_ledger_ids() {
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let errs = validate_seam_bindings(&ledger);
    assert!(
        errs.is_empty(),
        "staged-platform seam bindings drifted:\n{}",
        errs.join("\n")
    );
}

#[test]
fn stage_entries_cite_existing_registry_capabilities_and_stage_tei_lift_stays_bound() {
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let registry = load_registry_value();

    let note_errs = validate_registry_binding_notes(&ledger, &load_registry_ids());
    assert!(
        note_errs.is_empty(),
        "registry_binding note violations:\n{}",
        note_errs.join("\n")
    );

    let stage_errs = validate_registry_stage_binding(&registry);
    assert!(
        stage_errs.is_empty(),
        "stage.tei_lift registry binding drifted:\n{}",
        stage_errs.join("\n")
    );
}

#[test]
fn rejects_seam_documentation_removal_from_the_lift_entry() {
    let (mut ledger, _) = loaded_pair();
    let idx = ledger
        .entries
        .iter()
        .position(|e| e.id == "parse.stage.lift_tei")
        .expect("lift entry present");
    ledger.entries[idx].entry_point = Some("da_parse::GrobidParser::lift_tei".to_string());
    ledger.entries[idx].request_shape =
        Some("lift_tei(tei_xml: String, paper_id, pdf_hash) -> TeiParseIr".to_string());
    ledger.entries[idx]
        .notes
        .retain(|n| !n.contains("TeiLifter") && !n.contains("TeiEnvelope"));
    let errs = validate_seam_bindings(&ledger);
    negative(
        "[seam-binding-missing]",
        &errs,
        "stripped seam documentation",
    );
}

#[test]
fn rejects_removed_registry_binding_note() {
    let (mut ledger, _) = loaded_pair();
    for e in &mut ledger.entries {
        e.notes
            .retain(|n| !n.trim_start().starts_with("registry_binding:"));
    }
    let errs = validate_registry_binding_notes(&ledger, &load_registry_ids());
    negative(
        "[missing-registry-binding-note]",
        &errs,
        "removed registry_binding note",
    );
}

#[test]
fn rejects_invented_registry_binding_reference() {
    let (mut ledger, _) = loaded_pair();
    let idx = ledger
        .entries
        .iter()
        .position(|e| e.id == "parse.stage.lift_tei")
        .expect("lift entry present");
    for n in &mut ledger.entries[idx].notes {
        if n.trim_start().starts_with("registry_binding:") {
            *n = "registry_binding: stage.invented_lift".to_string();
        }
    }
    let errs = validate_registry_binding_notes(&ledger, &load_registry_ids());
    negative(
        "[unresolved-registry-binding-note]",
        &errs,
        "invented registry reference",
    );
}

#[test]
fn rejects_registry_stage_binding_drift_on_a_synthetic_registry() {
    // A registry whose stage.tei_lift capability points elsewhere (another
    // crate, rotated candidate identity) must fail the binding proof.
    let yaml = "capabilities:\n  - id: stage.tei_lift\n    implementation:\n      crate: da-adapters\n      surface: crates/da-adapters/src/somewhere.rs\n    baseline: preserved-java-tei\n    candidate: da-adapters@lift-v1\n";
    let registry: serde_yaml::Value = serde_yaml::from_str(yaml).expect("synthetic registry");
    let errs = validate_registry_stage_binding(&registry);
    assert!(
        errs.iter()
            .any(|e| e.contains("[registry-stage-binding-drift]")),
        "expected drift detection, got {errs:?}"
    );
    // Losing the capability entirely must also fail.
    let empty: serde_yaml::Value = serde_yaml::from_str("capabilities: []").expect("empty");
    let errs = validate_registry_stage_binding(&empty);
    negative(
        "[registry-stage-binding-drift]",
        &errs,
        "removed stage.tei_lift capability",
    );
}

// ───────── S03/T04 conservation evidence requirement ─────────

#[test]
fn every_case_requires_conservation_delta_evidence() {
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    assert!(!manifest.cases.is_empty(), "positive control: cases loaded");
    for case in &manifest.cases {
        assert!(
            case.required_evidence_fields
                .iter()
                .any(|field| field == "conservation_delta"),
            "case `{}` must require `conservation_delta` evidence (S03/T04: every exact TeiEnvelope/TeiParseIr pair runs through the sanctioned conservation engine)",
            case.id
        );
    }
}
