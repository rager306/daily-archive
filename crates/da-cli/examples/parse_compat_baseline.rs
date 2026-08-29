//! Parse-compat baseline emitter (M464-z84in2 / S01 / T03).
//!
//! Deterministic, offline-first compatibility report over the S01 DATA layer:
//!
//! - Canonical inputs (treated immutable): `data/parse/regression_cases.yaml`
//!   (bounded regression manifest), `data/parse/api_compatibility_ledger.yaml`
//!   (observation ledger), `data/parse/grobid_compatibility.yaml` (registry of
//!   record), `data/parse/comparison_contract.yaml` (report contract) and the
//!   T02 fixture provenance `crates/da-parse/tests/fixtures/preserved/provenance.yaml`.
//! - Direct-TEI path (default): reads ONLY preserved fixtures, runs the current
//!   da-parse lift/project path (`GrobidParser::lift_tei` + `project_article`),
//!   and binds source hash, fixture/provenance identity, profile fingerprint,
//!   output hash, family counts, current lifted counts and hard-regression
//!   statuses. RAW PDF/TEI/document text NEVER enters the report.
//! - Conservation evidence (S03/T04): every executed case runs its exact
//!   `TeiEnvelope`/`TeiParseIr` pair through the public provider-neutral
//!   `da_parse::ConservationEngine`; the report carries per-case and
//!   aggregate family/span/structure/reason-code counts, the delta verdict
//!   and digest, and a malformed summary — keyed to the exact
//!   fixture/backend/profile identity. Replay equality is asserted in-process
//!   (byte-identical delta), and injected loss stays visible as typed
//!   non-zero reasons (proven by the focused tests below).
//! - Native date shadow (S08/T03): the pure `NativeDateParser` candidate is
//!   evaluated on the SAME checksum-bound ordered `<date type="published">`
//!   occurrence vector as the T02 oracle contract (denominator 173 =
//!   80 + 50 + 43) and reported as additive `native_date_shadow` evidence —
//!   identity/ordinal/span/SHA-256/closed-outcome only, strict/soft flags,
//!   same-n, invented/malformed/unsupported counts, a redacting aggregate
//!   digest and bucketized latency characterization. Shadow only: the
//!   candidate is never a runtime route; Java GROBID stays oracle/rollback.
//! - Java GROBID evidence is a separate, explicitly selected mode (`--java`).
//!   The default run never contacts GROBID. Unavailable/unobserved Java
//!   evidence is reported honestly; no version/request/latency/output value is
//!   ever fabricated.
//! - Projection identity (S07/T05): the `ParsedArticle` projection schema
//!   fingerprint is `ParsedArticle@serde-v4-p0-tei-lift` — the additive P0
//!   lift families joined the wire, so the v3 label would have been
//!   dishonest — and the output-hash preimage is
//!   `da-parse.lift-output-preimage.v2` (v1 was blind to exactly the lifted
//!   families). Raw-TEI stage keys, D204 profile fingerprints and endpoint
//!   identities stay frozen; the per-case report additionally carries the
//!   closed S07 typed-missingness counts (machine tags only).
//! - Ordering and serialization are stable: identical inputs produce identical
//!   identity/count/regression content (verified in-file and by running the
//!   command twice and comparing bytes).
//!
//! Hard locks preserved: `import_eligible=false`, sidecar TEI is candidate
//! evidence only, no graph writes, no LLM promotion, no DSPy, no invented
//! labels, no new persistence backend.
//!
//! Usage:
//! ```text
//! cargo run -p da-cli --example parse_compat_baseline -- \
//!   --fixtures crates/da-parse/tests/fixtures \
//!   --report artifacts/parse_compat/baseline.json
//! ```
//!
//! Optional live observation (additional evidence only):
//! ```text
//! ... --java [--grobid-url http://127.0.0.1:8070] [--java-pdf-dir DIR]
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::Parser as CliParser;
use da_parse::{
    CACHED_TEI_BACKEND_ID, ConservationDelta, ConservationEngine, ConservationFamily,
    ConservationReasonCode, DatePrecision, FamilyDeltaCounts, GrobidParser, GrobidProfile,
    NATIVE_DATE_OUTCOME_SCHEMA_VERSION, NATIVE_DATE_PARSER_ID, NATIVE_DATE_PARSER_VERSION,
    NativeDateOutcome, NativeDateParser, RecognizedDate, TeiEnvelope, TeiIdentity, TeiLifter,
};
use da_ports::parser::{ParsedArticle, ParserPort};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

// ───────────────────────────── constants ──────────────────────────────

/// Repo-relative prefix every manifest fixture path must carry.
const FIXTURE_PREFIX: &str = "crates/da-parse/tests/fixtures/";
const DEFAULT_MANIFEST_REL: &str = "data/parse/regression_cases.yaml";
const DEFAULT_LEDGER_REL: &str = "data/parse/api_compatibility_ledger.yaml";
const DEFAULT_REGISTRY_REL: &str = "data/parse/grobid_compatibility.yaml";
const DEFAULT_CONTRACT_REL: &str = "data/parse/comparison_contract.yaml";
const PROVENANCE_REL_IN_FIXTURES: &str = "preserved/provenance.yaml";
/// Loopback default mirrored from da-parse (DEFAULT_GROBID_URL is pub(crate)).
const DEFAULT_GROBID_URL: &str = "http://127.0.0.1:8070";
/// Short bounded timeout for the optional live Java probe.
const JAVA_PROBE_TIMEOUT: Duration = Duration::from_secs(10);

const REPORT_SCHEMA_VERSION: &str = "parse-compat-baseline.v1";
const CONTRACT_ID: &str = "parse-comparison";
/// Sanctioned conservation engine bound by the comparison contract
/// (data/parse/comparison_contract.yaml `conservation_evidence.engine`).
const CONSERVATION_ENGINE_ID: &str = "da_parse::ConservationEngine";
const BACKEND_ID_DIRECT: &str = "da-parse-direct-tei";
const CORPUS_CLASS: &str = "fast_local_fixtures";
/// Ledger entry whose multipart request fields Java observations must echo.
const JAVA_REQUEST_FIELDS_LEDGER_ENTRY: &str = "parse.endpoint.process_fulltext_document";

/// Forbidden report fields (data/parse/comparison_contract.yaml). The report
/// self-validates that none of these appear as keys anywhere in the document.
const FORBIDDEN_FALLBACK_FIELDS: [&str; 7] = [
    "tei_xml",
    "body_text",
    "pdf_bytes",
    "raw_document_content",
    "raw_tei",
    "fulltext",
    "page_images",
];

// ─────────────────────────────── CLI ──────────────────────────────────

#[derive(Debug, CliParser)]
#[command(
    name = "parse_compat_baseline",
    about = "Emit the source-bound direct-TEI (and optional Java) parse-compat baseline report"
)]
struct Args {
    /// Fixtures root containing preserved/ (default is the repo fixture set).
    #[arg(long, default_value = "crates/da-parse/tests/fixtures")]
    fixtures: PathBuf,
    /// Report output path (repo-relative by default).
    #[arg(long, default_value = "artifacts/parse_compat/baseline.json")]
    report: PathBuf,
    /// Regression manifest override (default data/parse/regression_cases.yaml).
    #[arg(long)]
    manifest: Option<PathBuf>,
    /// EXPLICITLY select live Java GROBID observation mode. The default run
    /// never contacts GROBID.
    #[arg(long)]
    java: bool,
    /// Java backend base URL (default: GROBID_URL env or loopback 8070).
    #[arg(long)]
    grobid_url: Option<String>,
    /// Directory searched for original PDFs when capturing Java observations.
    #[arg(long)]
    java_pdf_dir: Option<PathBuf>,
}

// ─────────────────────────── input data model ─────────────────────────

#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PolicyLocks {
    import_eligible: bool,
    dspy_enabled: bool,
    graph_mutation: String,
    llm_promotion: String,
    free_label_invention: String,
    sidecar_status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRef {
    path: String,
    #[serde(default)]
    sha256: Option<String>,
    #[allow(dead_code)]
    origin: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    #[allow(dead_code)]
    source_kind: String,
    fixture: FixtureRef,
    binding_status: String,
    #[serde(default)]
    binding_owner_task: Option<String>,
    grobid_profile: String,
    expected_cache_config_fingerprint: String,
    stages: Vec<String>,
    stage_execution_mode: String,
    #[allow(dead_code)]
    expected_outcome: String,
    #[allow(dead_code)]
    required_evidence_fields: Vec<String>,
    ledger_bindings: Vec<String>,
    #[allow(dead_code)]
    obligation_acknowledged: bool,
    #[serde(default)]
    notes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    #[allow(dead_code)]
    manifest_version: u32,
    #[allow(dead_code)]
    scope_note: String,
    policy_locks: PolicyLocks,
    #[allow(dead_code)]
    profile_fingerprint_binding_required: bool,
    #[allow(dead_code)]
    cache_key_separation_rule: String,
    cases: Vec<Case>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceFixture {
    id: String,
    paper_id: String,
    arxiv_id: String,
    title: String,
    path: String,
    #[allow(dead_code)]
    origin_path: String,
    sha256: String,
    #[allow(dead_code)]
    bytes: u64,
    #[serde(default)]
    #[allow(dead_code)]
    covers: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    notes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnboundFixture {
    #[allow(dead_code)]
    id: String,
    path: String,
    reason: String,
    #[allow(dead_code)]
    binding_status: String,
    #[allow(dead_code)]
    binding_owner_task: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    #[allow(dead_code)]
    schema_version: String,
    policy_locks: PolicyLocks,
    #[allow(dead_code)]
    origin_corpus: String,
    #[allow(dead_code)]
    origin_probe: String,
    backend: String,
    #[allow(dead_code)]
    assumed_grobid_profile: String,
    #[allow(dead_code)]
    configuration_fingerprint: String,
    #[allow(dead_code)]
    license: String,
    fixtures: Vec<ProvenanceFixture>,
    #[serde(default)]
    unbound: Vec<UnboundFixture>,
}

// ─────────────────────────── report data model ────────────────────────

#[derive(Debug, serde::Serialize)]
struct ReportIdentity {
    source_hash: String,
    backend_id: String,
    backend_version: String,
    backend_version_basis: String,
    profile_fingerprint: String,
    request_fields: Vec<String>,
    output_hash: String,
    corpus_id: String,
    corpus_hash: String,
    corpus_n: usize,
    same_n: bool,
    same_n_meaning: String,
    fixture_ids: Vec<String>,
    family_counts: BTreeMap<String, u64>,
    current_lifted_counts: BTreeMap<String, u64>,
    /// Aggregate of the per-case S07 typed-missingness counts (exact sum —
    /// machine tags only, never document content).
    typed_missingness: BTreeMap<String, u64>,
    promotion_state: String,
    rollback_target: String,
    known_hard_regressions: Vec<HardRegressionStatus>,
}

#[derive(Debug, serde::Serialize)]
struct HardRegressionStatus {
    id: String,
    description: String,
    status: String,
    evidence: String,
}

/// Bounded span-retention counts: how many observed source byte spans exist
/// and how many of them are retained verbatim in the candidate identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
struct SourceSpanRetention {
    source_spans: u64,
    retained_spans: u64,
}

/// Per-case conservation evidence from the sanctioned engine. Serialized
/// content is identity/counts/ordinals/ranges/digests only — the embedded
/// `delta` (full ConservationDelta) is redacting by construction.
#[derive(Debug, Clone, serde::Serialize)]
struct ConservationCaseEvidence {
    engine: String,
    backend_id: String,
    profile_fingerprint: String,
    paper_id: String,
    raw_tei_len: u64,
    raw_tei_sha256: String,
    verdict: String,
    delta_digest: String,
    total_rows: u64,
    loss_rows: u64,
    malformed_rows: u64,
    reason_counts: BTreeMap<String, u64>,
    structure_counts: BTreeMap<String, u64>,
    family_counts: BTreeMap<ConservationFamily, FamilyDeltaCounts>,
    source_span_retention: SourceSpanRetention,
    delta: ConservationDelta,
}

/// Deterministic conservation aggregate: every counter is exactly the sum
/// over the per-case rows (self-validated on every render, tested in-file).
#[derive(Debug, Clone, serde::Serialize)]
struct ConservationAggregate {
    cases: usize,
    verdicts: BTreeMap<String, u64>,
    reason_counts: BTreeMap<String, u64>,
    structure_counts: BTreeMap<String, u64>,
    family_counts: BTreeMap<ConservationFamily, FamilyDeltaCounts>,
    total_rows: u64,
    loss_rows: u64,
    malformed_rows: u64,
    source_span_retention: SourceSpanRetention,
    delta_digests: BTreeMap<String, String>,
}

#[derive(Debug, serde::Serialize)]
struct CaseReport {
    case_id: String,
    binding_status: String,
    source_path: String,
    source_sha256: String,
    provenance_fixture_id: String,
    provenance_paper_id: String,
    provenance_backend: String,
    grobid_profile: String,
    profile_fingerprint: String,
    stages: Vec<String>,
    stage_execution_mode: String,
    ledger_bindings: Vec<String>,
    obligation_acknowledged: bool,
    family_counts: BTreeMap<String, u64>,
    lifted_counts: BTreeMap<String, u64>,
    /// Closed typed-missingness counts of the S07 P0 families (machine tags
    /// only — never raw document content).
    typed_missingness: BTreeMap<String, u64>,
    output_hash: String,
    outcome: String,
    conservation: ConservationCaseEvidence,
}

#[derive(Debug, serde::Serialize)]
struct PendingCaseReport {
    case_id: String,
    binding_owner_task: String,
    expected_profile_fingerprint: String,
    status: String,
    typed_absence: bool,
    reason: String,
}

#[derive(Debug, serde::Serialize)]
struct JavaObservation {
    case_id: String,
    status: String,
    profile_fingerprint: String,
    request_fields_snapshot: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pdf_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    returned_tei_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct JavaEvidence {
    selected: bool,
    status: String,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    backend_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    grobid_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observations: Option<Vec<JavaObservation>>,
}

#[derive(Debug, serde::Serialize)]
struct BaselineReport {
    schema_version: String,
    contract_id: String,
    corpus_classes: serde_json::Value,
    report_identity: ReportIdentity,
    cases: Vec<CaseReport>,
    pending_cases: Vec<PendingCaseReport>,
    conservation: ConservationAggregate,
    /// S08/T03: additive shadow evidence from the pure native date candidate
    /// over the checksum-bound ordered occurrence vector. Shadow only — the
    /// candidate is never a runtime route and never enters output identity.
    native_date_shadow: NativeDateShadowEvidence,
    java_grobid: JavaEvidence,
    policy_locks: PolicyLocks,
}

// ─────────────────────────── run configuration ────────────────────────

#[derive(Debug, Clone, Default)]
struct JavaSelection {
    enabled: bool,
    url: Option<String>,
    pdf_dir: Option<PathBuf>,
}

struct BaselineInputs<'a> {
    repo_root: &'a Path,
    fixtures_root: &'a Path,
    /// `None` uses the canonical data/parse manifest.
    manifest_path: Option<&'a Path>,
    java: JavaSelection,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root resolves")
}

fn resolve_against_repo(repo_root: &Path, p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        repo_root.join(p)
    }
}

fn read_text(path: &Path, what: &str) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|e| format!("[unreadable-artifact] {what} {}: {e}", path.display()))
}

fn parse_yaml<T: for<'de> Deserialize<'de>>(text: &str, what: &str) -> Result<T, String> {
    serde_yaml::from_str(text).map_err(|e| format!("[malformed-yaml] {what}: {e}"))
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

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn profile_by_name(name: &str) -> Result<GrobidProfile, String> {
    match name {
        "rich" => Ok(GrobidProfile::rich()),
        "structural" => Ok(GrobidProfile::structural()),
        other => Err(format!("[unknown-profile] `{other}`")),
    }
}

// ─────────────────────── generic DATA readers (R117) ──────────────────

/// Registry-of-record capability ids (capabilities[].id).
fn registry_capability_ids(repo_root: &Path) -> Result<BTreeSet<String>, String> {
    let text = read_text(&repo_root.join(DEFAULT_REGISTRY_REL), "registry of record")?;
    let v: serde_yaml::Value = parse_yaml(&text, "registry of record")?;
    let mut ids = BTreeSet::new();
    if let Some(caps) = v.get("capabilities").and_then(|c| c.as_sequence()) {
        for cap in caps {
            if let Some(id) = cap.get("id").and_then(|i| i.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }
    if ids.is_empty() {
        return Err("[registry-empty] no capabilities in registry of record".into());
    }
    Ok(ids)
}

/// Observation-ledger entry ids (entries[].id).
fn ledger_entry_ids(v: &serde_yaml::Value) -> BTreeSet<String> {
    v.get("entries")
        .and_then(|e| e.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|e| e.get("id").and_then(|i| i.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Multipart request fields of the Java endpoint entry, read from the ledger
/// (never hardcoded here — the wire contract is DATA-owned).
fn ledger_request_fields(v: &serde_yaml::Value) -> Result<Vec<String>, String> {
    let entry = v
        .get("entries")
        .and_then(|e| e.as_sequence())
        .and_then(|seq| {
            seq.iter().find(|e| {
                e.get("id").and_then(|i| i.as_str()) == Some(JAVA_REQUEST_FIELDS_LEDGER_ENTRY)
            })
        })
        .ok_or_else(|| {
            format!(
                "[ledger-entry-shape] missing ledger entry `{JAVA_REQUEST_FIELDS_LEDGER_ENTRY}`"
            )
        })?;
    let fields = entry
        .get("request_fields")
        .and_then(|f| f.as_sequence())
        .ok_or_else(|| {
            format!(
                "[ledger-entry-shape] `{JAVA_REQUEST_FIELDS_LEDGER_ENTRY}` carries no request_fields"
            )
        })?;
    fields
        .iter()
        .map(|f| {
            f.as_str().map(str::to_string).ok_or_else(|| {
                format!("[ledger-entry-shape] non-string request field in `{JAVA_REQUEST_FIELDS_LEDGER_ENTRY}`")
            })
        })
        .collect()
}

struct ContractData {
    required_identity_fields: Vec<String>,
    forbidden_fields: Vec<String>,
    promotion_default: String,
    rollback_target: String,
    hard_regressions: Vec<(String, String)>,
    conservation_required_fields: Vec<String>,
    /// S08/T03: DATA-owned native date shadow terms (denominator, same-n,
    /// invented budget, shadow/state/rollback identity).
    native_date_shadow: NativeDateShadowTerms,
}

/// DATA-pinned native date shadow terms (comparison_contract.yaml
/// `native_date_shadow`), validated against the LIVE da-parse candidate
/// constants while loading. Any drift fails visibly — never silently.
#[derive(Debug, Clone)]
struct NativeDateShadowTerms {
    engine: String,
    candidate_id: String,
    candidate_version: String,
    outcome_schema_version: String,
    observation_mode: String,
    promotion_state: String,
    rollback_target: String,
    oracle_source: String,
    corpus_class: String,
    scope_order: Vec<String>,
    denominator_total: usize,
    denominator_per_fixture: BTreeMap<String, usize>,
    invented_component_budget: u64,
}

fn load_contract(repo_root: &Path) -> Result<ContractData, String> {
    let text = read_text(&repo_root.join(DEFAULT_CONTRACT_REL), "comparison contract")?;
    let v: serde_yaml::Value = parse_yaml(&text, "comparison contract")?;
    let str_vec = |node: Option<&serde_yaml::Value>, what: &str| -> Result<Vec<String>, String> {
        node.and_then(|n| n.as_sequence())
            .ok_or_else(|| format!("[contract-shape] missing {what}"))?
            .iter()
            .map(|i| {
                i.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("[contract-shape] non-string item in {what}"))
            })
            .collect()
    };
    let required_identity_fields = str_vec(
        v.get("report_identity")
            .and_then(|r| r.get("required_fields")),
        "report_identity.required_fields",
    )?;
    let forbidden_fields = match v
        .get("forbidden_report_fields")
        .and_then(|f| f.as_sequence())
    {
        Some(seq) => seq
            .iter()
            .filter_map(|i| i.as_str().map(str::to_string))
            .collect(),
        None => FORBIDDEN_FALLBACK_FIELDS
            .iter()
            .map(|s| s.to_string())
            .collect(),
    };
    let promotion_default = v
        .get("promotion")
        .and_then(|p| p.get("default_state"))
        .and_then(|s| s.as_str())
        .map(str::to_string)
        .ok_or("[contract-shape] missing promotion.default_state")?;
    let rollback_target = v
        .get("rollback")
        .and_then(|r| r.get("default_target"))
        .and_then(|s| s.as_str())
        .map(str::to_string)
        .ok_or("[contract-shape] missing rollback.default_target")?;
    // S03/T04: the contract binds the sanctioned conservation engine and the
    // per-case evidence fields this emitter must carry and self-validate.
    let conservation = v
        .get("conservation_evidence")
        .ok_or("[contract-shape] missing conservation_evidence (S03/T04 binding)")?;
    let engine = conservation
        .get("engine")
        .and_then(|e| e.as_str())
        .ok_or("[contract-shape] missing conservation_evidence.engine")?;
    if engine != CONSERVATION_ENGINE_ID {
        return Err(format!(
            "[contract-engine-drift] contract binds conservation engine `{engine}` but the sanctioned engine is `{CONSERVATION_ENGINE_ID}`"
        ));
    }
    let conservation_required_fields = str_vec(
        conservation.get("required_per_case_fields"),
        "conservation_evidence.required_per_case_fields",
    )?;
    let mut hard_regressions = Vec::new();
    if let Some(seq) = v.get("hard_regressions").and_then(|h| h.as_sequence()) {
        for item in seq {
            let id = item
                .get("id")
                .and_then(|i| i.as_str())
                .map(str::to_string)
                .ok_or("[contract-shape] hard regression without id")?;
            let description = item
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            hard_regressions.push((id, description));
        }
    }
    // S08/T03: the native date shadow terms are DATA-owned. Validate them
    // against the LIVE candidate constants at load time — any drift fails
    // visibly, never silently.
    let shadow = v
        .get("native_date_shadow")
        .ok_or("[contract-shape] missing native_date_shadow (S08/T03 binding)")?;
    let shadow_str = |key: &str| -> Result<String, String> {
        shadow
            .get(key)
            .and_then(|s| s.as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("[contract-shape] missing native_date_shadow.{key}"))
    };
    let engine = shadow_str("engine")?;
    if engine != NATIVE_DATE_ENGINE_ID {
        return Err(format!(
            "[contract-shadow-drift] contract binds engine `{engine}` but the sanctioned candidate is `{NATIVE_DATE_ENGINE_ID}`"
        ));
    }
    let candidate_id = shadow_str("candidate_id")?;
    if candidate_id != NATIVE_DATE_PARSER_ID {
        return Err(format!(
            "[contract-shadow-drift] contract candidate `{candidate_id}` != live `{NATIVE_DATE_PARSER_ID}`"
        ));
    }
    let candidate_version = shadow_str("candidate_version")?;
    if candidate_version != NATIVE_DATE_PARSER_VERSION {
        return Err(format!(
            "[contract-shadow-drift] contract candidate version `{candidate_version}` != live `{NATIVE_DATE_PARSER_VERSION}`"
        ));
    }
    let outcome_schema_version = shadow_str("outcome_schema_version")?;
    if outcome_schema_version != NATIVE_DATE_OUTCOME_SCHEMA_VERSION {
        return Err(format!(
            "[contract-shadow-drift] contract outcome schema `{outcome_schema_version}` != live `{NATIVE_DATE_OUTCOME_SCHEMA_VERSION}`"
        ));
    }
    let observation_mode = shadow_str("observation_mode")?;
    if observation_mode != "shadow" {
        return Err(format!(
            "[contract-shadow-drift] native_date_shadow.observation_mode must stay `shadow` (got `{observation_mode}`) — the candidate is never a runtime route"
        ));
    }
    let shadow_promotion_state = shadow_str("promotion_state")?;
    if shadow_promotion_state != "candidate" {
        return Err(format!(
            "[contract-shadow-drift] native_date_shadow.promotion_state must stay `candidate` (got `{shadow_promotion_state}`) — auto-promotion is forbidden"
        ));
    }
    let shadow_rollback = shadow_str("rollback_target")?;
    if shadow_rollback != rollback_target {
        return Err(format!(
            "[contract-shadow-drift] native_date_shadow.rollback_target `{shadow_rollback}` diverges from the contract default `{rollback_target}`"
        ));
    }
    let oracle_source = shadow_str("oracle_source")?;
    if oracle_source != "tei_when_attribute" {
        return Err(format!(
            "[contract-shadow-drift] native_date_shadow.oracle_source must stay `tei_when_attribute` (got `{oracle_source}`)"
        ));
    }
    let shadow_corpus_class = shadow_str("corpus_class")?;
    if shadow_corpus_class != CORPUS_CLASS {
        return Err(format!(
            "[contract-shadow-drift] native_date_shadow.corpus_class `{shadow_corpus_class}` diverges from the executed corpus `{CORPUS_CLASS}`"
        ));
    }
    let scope_order = str_vec(shadow.get("scope_order"), "native_date_shadow.scope_order")?;
    if scope_order != NATIVE_DATE_SCOPES {
        return Err(format!(
            "[contract-shadow-drift] native_date_shadow.scope_order {scope_order:?} diverges from the pinned scope tags {NATIVE_DATE_SCOPES:?}"
        ));
    }
    let denominator = shadow
        .get("denominator")
        .ok_or("[contract-shape] missing native_date_shadow.denominator")?;
    let denominator_total = denominator
        .get("total")
        .and_then(|t| t.as_u64())
        .ok_or("[contract-shape] missing native_date_shadow.denominator.total")?
        as usize;
    let mut denominator_per_fixture = BTreeMap::new();
    if let Some(map) = denominator.get("per_fixture").and_then(|p| p.as_mapping()) {
        for (key, value) in map {
            let case_id = key
                .as_str()
                .ok_or("[contract-shape] non-string denominator per_fixture key")?;
            let n = value
                .as_u64()
                .ok_or("[contract-shape] non-numeric denominator per_fixture value")?
                as usize;
            denominator_per_fixture.insert(case_id.to_string(), n);
        }
    }
    if denominator_per_fixture.is_empty() {
        return Err("[contract-shape] empty native_date_shadow.denominator.per_fixture".into());
    }
    let per_fixture_sum: usize = denominator_per_fixture.values().sum();
    if per_fixture_sum != denominator_total {
        return Err(format!(
            "[contract-shadow-denominator] per-fixture denominator sum {per_fixture_sum} != pinned total {denominator_total}"
        ));
    }
    let semantics = shadow
        .get("semantics")
        .ok_or("[contract-shape] missing native_date_shadow.semantics")?;
    if semantics.get("same_n").and_then(|s| s.as_str()) != Some("required") {
        return Err(
            "[contract-shadow-drift] native_date_shadow.semantics.same_n must stay `required"
                .into(),
        );
    }
    for key in ["strict", "soft"] {
        if semantics
            .get(key)
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(format!(
                "[contract-shape] native_date_shadow.semantics.{key} must state the DATA-owned semantics"
            ));
        }
    }
    let invented_component_budget = semantics
        .get("invented_component_budget")
        .and_then(|b| b.as_u64())
        .ok_or("[contract-shape] missing native_date_shadow.semantics.invented_component_budget")?;
    if invented_component_budget != 0 {
        return Err(format!(
            "[contract-shadow-drift] native_date_shadow.semantics.invented_component_budget must stay 0 (got {invented_component_budget})"
        ));
    }
    let forbidden: Vec<String> = shadow
        .get("forbidden")
        .and_then(|f| f.as_sequence())
        .ok_or("[contract-shape] missing native_date_shadow.forbidden")?
        .iter()
        .map(|i| {
            i.as_str().map(str::to_string).ok_or_else(|| {
                "[contract-shape] non-string item in native_date_shadow.forbidden".to_string()
            })
        })
        .collect::<Result<Vec<String>, String>>()?;
    for required in [
        "auto_promotion",
        "runtime_routing",
        "raw_display_text",
        "tei_fragments",
        "invented_bindings",
    ] {
        if !forbidden.iter().any(|f| f == required) {
            return Err(format!(
                "[contract-shadow-drift] native_date_shadow.forbidden lost `{required}`"
            ));
        }
    }
    let shadow_regressions: Vec<String> = shadow
        .get("hard_regressions")
        .and_then(|h| h.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if shadow_regressions.is_empty() {
        return Err(
            "[contract-shape] native_date_shadow.hard_regressions must cite top-level regression ids"
                .into(),
        );
    }
    let top_regression_ids: BTreeSet<String> =
        hard_regressions.iter().map(|(id, _)| id.clone()).collect();
    for id in &shadow_regressions {
        if !top_regression_ids.contains(id) {
            return Err(format!(
                "[contract-shadow-drift] native_date_shadow.hard_regressions cites unknown id `{id}`"
            ));
        }
    }
    let native_date_shadow = NativeDateShadowTerms {
        engine,
        candidate_id,
        candidate_version,
        outcome_schema_version,
        observation_mode,
        promotion_state: shadow_promotion_state,
        rollback_target: shadow_rollback,
        oracle_source,
        corpus_class: shadow_corpus_class,
        scope_order,
        denominator_total,
        denominator_per_fixture,
        invented_component_budget,
    };
    Ok(ContractData {
        required_identity_fields,
        forbidden_fields,
        promotion_default,
        rollback_target,
        hard_regressions,
        conservation_required_fields,
        native_date_shadow,
    })
}

/// Workspace version of the da-parse backend (its Cargo.toml inherits
/// [workspace.package].version). Honest source-tree identity binding.
fn backend_version(repo_root: &Path) -> (String, String) {
    let basis =
        "workspace Cargo.toml [workspace.package].version (da-parse builds from this source tree; no deployed backend version)"
            .to_string();
    let Ok(text) = std::fs::read_to_string(repo_root.join("Cargo.toml")) else {
        return ("unresolved".to_string(), basis);
    };
    let mut in_ws_package = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_ws_package = trimmed == "[workspace.package]";
            continue;
        }
        if in_ws_package && let Some(rest) = trimmed.strip_prefix("version") {
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix('=').map(str::trim) {
                return (value.trim_matches('"').to_string(), basis);
            }
        }
    }
    ("unresolved".to_string(), basis)
}

// ─────────────────────── lift/project execution ───────────────────────

/// Canonical digest preimage of the lifted projection. Deliberately EXCLUDES
/// raw `tei_xml` (bound to `null`): the identity covers the lifted content
/// only, and raw TEI/PDF/document text never reaches the report.
///
/// v2 (M464-z84in2 S07/T05): the additive P0 lift families — declared arXiv
/// identity + typed missingness, header authors, non-table figures with
/// graphic coordinates, figure/table visual callouts — joined the preimage.
/// The v1 digest would have been blind to exactly the families this slice
/// lifted, so the output-hash pin rotates with them (raw-TEI stage identity
/// is untouched).
fn lifted_output_preimage(article: &ParsedArticle) -> serde_json::Value {
    json!({
        "schema": "da-parse.lift-output-preimage.v2",
        "paper_id": article.paper_id,
        "pdf_hash": article.pdf_hash,
        "title": article.title,
        "abstract_text": article.abstract_text,
        "body_text": article.body_text,
        "sections": &article.sections,
        "citations": &article.citations,
        "tables": &article.tables,
        "table_missingness": &article.table_missingness,
        "formulas": &article.formulas,
        "formula_missingness": &article.formula_missingness,
        "declared_arxiv_id": &article.declared_arxiv_id,
        "declared_id_missingness": &article.declared_id_missingness,
        "header_authors": &article.header_authors,
        "figures": &article.figures,
        "visual_callouts": &article.visual_callouts,
        "tei_xml": serde_json::Value::Null,
    })
}

fn output_hash_of(article: &ParsedArticle) -> String {
    let preimage = lifted_output_preimage(article);
    let canonical = serde_json::to_string(&preimage).expect("preimage serializes");
    format!("sha256:{}", sha256_hex(canonical.as_bytes()))
}

fn lifted_counts(article: &ParsedArticle) -> BTreeMap<String, u64> {
    let mut m = BTreeMap::new();
    m.insert(
        "title_present".into(),
        u64::from(!article.title.trim().is_empty()),
    );
    m.insert(
        "abstract_present".into(),
        u64::from(!article.abstract_text.trim().is_empty()),
    );
    m.insert(
        "body_chars".into(),
        article.body_text.chars().count() as u64,
    );
    m.insert("sections".into(), article.sections.len() as u64);
    m.insert(
        "sections_with_path_id".into(),
        article
            .sections
            .iter()
            .filter(|s| s.path_id.is_some())
            .count() as u64,
    );
    m.insert("citations".into(), article.citations.len() as u64);
    m.insert(
        "citation_anchors".into(),
        article
            .citations
            .iter()
            .map(|c| c.anchors.len())
            .sum::<usize>() as u64,
    );
    m.insert("tables".into(), article.tables.len() as u64);
    m.insert(
        "table_cells".into(),
        article.tables.iter().map(|t| t.cells.len()).sum::<usize>() as u64,
    );
    m.insert("formulas".into(), article.formulas.len() as u64);
    m.insert(
        "formula_representations".into(),
        article
            .formulas
            .iter()
            .filter(|f| f.missingness.is_none())
            .count() as u64,
    );
    // S07 P0 lift families (M464-z84in2): counts only — names, captions,
    // coordinates and quotes never leave the digest preimage above.
    m.insert(
        "declared_arxiv_id".into(),
        u64::from(article.declared_arxiv_id.is_some()),
    );
    m.insert(
        "declared_id_missingness".into(),
        u64::from(article.declared_id_missingness.is_some()),
    );
    m.insert(
        "declared_id_revision_declared".into(),
        u64::from(
            article
                .declared_arxiv_id
                .as_ref()
                .is_some_and(|id| id.revision.is_some()),
        ),
    );
    m.insert("header_authors".into(), article.header_authors.len() as u64);
    m.insert("figures".into(), article.figures.len() as u64);
    m.insert(
        "figure_coordinates".into(),
        article
            .figures
            .iter()
            .filter(|f| f.coordinates.is_some())
            .count() as u64,
    );
    m.insert(
        "figure_graphic_missingness".into(),
        article
            .figures
            .iter()
            .filter(|f| f.graphic_missingness.is_some())
            .count() as u64,
    );
    m.insert(
        "visual_callouts".into(),
        article.visual_callouts.len() as u64,
    );
    m.insert(
        "visual_callouts_resolved".into(),
        article
            .visual_callouts
            .iter()
            .filter(|c| c.unresolved.is_none())
            .count() as u64,
    );
    m.insert(
        "visual_callouts_unresolved".into(),
        article
            .visual_callouts
            .iter()
            .filter(|c| c.unresolved.is_some())
            .count() as u64,
    );
    m
}

fn family_counts_of(lifted: &BTreeMap<String, u64>) -> BTreeMap<String, u64> {
    [
        "sections",
        "citations",
        "tables",
        "formulas",
        "header_authors",
        "figures",
        "visual_callouts",
    ]
    .into_iter()
    .map(|k| (k.to_string(), lifted.get(k).copied().unwrap_or(0)))
    .collect()
}

/// Closed typed-missingness vocabulary of the S07 P0 lift families, as
/// counts keyed by machine tags. Details of malformed values never leave
/// the digest — reports carry the closed tag, never document content.
fn typed_missingness_counts(article: &ParsedArticle) -> BTreeMap<String, u64> {
    use da_ports::parser::{
        CalloutUnresolvedReason, DeclaredIdMissingReason, FigureGraphicMissingReason,
    };
    let mut m = BTreeMap::new();
    match &article.declared_id_missingness {
        Some(DeclaredIdMissingReason::NoIdno) => {
            m.insert("declared_id_no_idno".to_string(), 1);
        }
        Some(DeclaredIdMissingReason::Malformed(_)) => {
            m.insert("declared_id_malformed".to_string(), 1);
        }
        None => {}
    }
    for figure in &article.figures {
        let tag = match &figure.graphic_missingness {
            Some(FigureGraphicMissingReason::NoGraphicChild) => "figure_graphic_no_graphic_child",
            Some(FigureGraphicMissingReason::GraphicWithoutCoordinates) => {
                "figure_graphic_graphic_without_coordinates"
            }
            Some(FigureGraphicMissingReason::Malformed(_)) => "figure_graphic_malformed",
            None => continue,
        };
        *m.entry(tag.to_string()).or_insert(0) += 1;
    }
    for callout in &article.visual_callouts {
        let tag = match &callout.unresolved {
            Some(CalloutUnresolvedReason::NoTarget) => "callout_unresolved_no_target",
            Some(CalloutUnresolvedReason::TargetNotInInventory) => {
                "callout_unresolved_target_not_in_inventory"
            }
            None => continue,
        };
        *m.entry(tag.to_string()).or_insert(0) += 1;
    }
    m
}

// ─────────────────── conservation evidence (S03/T04, R106) ────────────

/// Closed reason-code counts of one delta (canonical snake_case keys).
fn reason_counts_of(delta: &ConservationDelta) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for entry in &delta.entries {
        *counts.entry(entry.reason.as_str().to_string()).or_insert(0) += 1;
    }
    counts
}

/// Structure observations grouped by their stable structure key.
fn structure_counts_of(delta: &ConservationDelta) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for entry in &delta.entries {
        if let Some(key) = &entry.structure_key {
            *counts.entry(key.clone()).or_insert(0) += 1;
        }
    }
    counts
}

/// How many observed source byte spans exist and how many survived as
/// retained candidate identity (contract metric `source_span_retention`).
fn source_span_retention_of(delta: &ConservationDelta) -> SourceSpanRetention {
    let source_spans = delta
        .family_counts
        .values()
        .map(|counts| u64::from(counts.source))
        .sum();
    let retained_spans = delta
        .entries
        .iter()
        .filter(|entry| {
            entry.reason == ConservationReasonCode::Retained && entry.byte_range.is_some()
        })
        .count() as u64;
    SourceSpanRetention {
        source_spans,
        retained_spans,
    }
}

/// Per-case evidence derived from the engine's redacting delta only.
fn conservation_case_evidence(delta: &ConservationDelta) -> ConservationCaseEvidence {
    ConservationCaseEvidence {
        engine: CONSERVATION_ENGINE_ID.into(),
        backend_id: delta.identity.backend_id.clone(),
        profile_fingerprint: delta.identity.profile_fingerprint.clone(),
        paper_id: delta.identity.paper_id.clone(),
        raw_tei_len: u64::from(delta.identity.raw_tei_len),
        raw_tei_sha256: delta.identity.raw_tei_sha256.clone(),
        verdict: delta.verdict.as_str().into(),
        delta_digest: delta.delta_digest.clone(),
        total_rows: delta.entries.len() as u64,
        loss_rows: delta.entries.iter().filter(|e| e.reason.is_loss()).count() as u64,
        malformed_rows: delta
            .entries
            .iter()
            .filter(|e| e.reason == ConservationReasonCode::MalformedSource)
            .count() as u64,
        reason_counts: reason_counts_of(delta),
        structure_counts: structure_counts_of(delta),
        family_counts: delta.family_counts.clone(),
        source_span_retention: source_span_retention_of(delta),
        delta: delta.clone(),
    }
}

/// Deterministic aggregate: every counter is exactly the sum over the
/// per-case rows (self-validated and byte-compared in tests).
fn aggregate_conservation(cases: &[CaseReport]) -> ConservationAggregate {
    let mut aggregate = ConservationAggregate {
        cases: cases.len(),
        verdicts: BTreeMap::new(),
        reason_counts: BTreeMap::new(),
        structure_counts: BTreeMap::new(),
        family_counts: BTreeMap::new(),
        total_rows: 0,
        loss_rows: 0,
        malformed_rows: 0,
        source_span_retention: SourceSpanRetention {
            source_spans: 0,
            retained_spans: 0,
        },
        delta_digests: BTreeMap::new(),
    };
    for case in cases {
        let evidence = &case.conservation;
        *aggregate
            .verdicts
            .entry(evidence.verdict.clone())
            .or_insert(0) += 1;
        for (reason, count) in &evidence.reason_counts {
            *aggregate.reason_counts.entry(reason.clone()).or_insert(0) += count;
        }
        for (key, count) in &evidence.structure_counts {
            *aggregate.structure_counts.entry(key.clone()).or_insert(0) += count;
        }
        for (family, counts) in &evidence.family_counts {
            let totals = aggregate.family_counts.entry(*family).or_default();
            totals.source += counts.source;
            totals.candidate += counts.candidate;
            totals.retained += counts.retained;
            totals.unsupported += counts.unsupported;
            totals.lost += counts.lost;
            totals.malformed += counts.malformed;
        }
        aggregate.total_rows += evidence.total_rows;
        aggregate.loss_rows += evidence.loss_rows;
        aggregate.malformed_rows += evidence.malformed_rows;
        aggregate.source_span_retention.source_spans += evidence.source_span_retention.source_spans;
        aggregate.source_span_retention.retained_spans +=
            evidence.source_span_retention.retained_spans;
        aggregate
            .delta_digests
            .insert(case.case_id.clone(), evidence.delta_digest.clone());
    }
    aggregate
}

// ───────────────────── S08/T03 native date shadow ─────────────────────

/// Closed latency-bucket vocabulary for the shadow evidence. Exact
/// wall-clock values never enter the report, so two offline generations
/// over unchanged inputs serialize byte-identically.
const NATIVE_DATE_LATENCY_BUCKETS: [&str; 5] =
    ["le_1us", "le_10us", "le_100us", "le_1ms", "gt_1ms"];

const NATIVE_DATE_SHADOW_SCHEMA: &str = "parse-compat-native-date-shadow.v1";
/// Digest preimage tag: case/fixture identity, ordinal, span, scope tag,
/// both SHA-256 fields, the closed outcome wire and the strict/soft flags
/// only — never raw text.
const NATIVE_DATE_SHADOW_DIGEST_SCHEMA: &str = "da-parse.native-date-shadow-digest.v1";
const NATIVE_DATE_ENGINE_ID: &str = "da_parse::NativeDateParser";

/// Per-occurrence wire keys — the closed evidence surface. Any other key is
/// a redaction breach and fails self-validation.
const NATIVE_DATE_ROW_KEYS: [&str; 11] = [
    "case_id",
    "fixture_id",
    "ordinal",
    "span",
    "scope",
    "input_sha256",
    "oracle_when_sha256",
    "outcome",
    "precision",
    "strict",
    "soft",
];

/// TEI scopes of the checksum-bound occurrence vector (T02 oracle contract;
/// DATA-pinned order in comparison_contract.yaml `native_date_shadow.scope_order`).
const NATIVE_DATE_SCOPES: [&str; 3] = [
    "header_article_date",
    "header_bibliography_date",
    "bibliography_date",
];

/// Independent oracle `when` model: strict `####` | `####-##` | `####-##-##`
/// with its own calendar math — deliberately never the candidate's helpers
/// (no common-mode failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShadowOracleWhen {
    Y(i32),
    YM(i32, u8),
    Ymd(i32, u8, u8),
}

impl ShadowOracleWhen {
    fn parse(raw: &str) -> Option<Self> {
        fn digits(part: &[u8]) -> bool {
            !part.is_empty() && part.iter().all(u8::is_ascii_digit)
        }
        fn num(part: &[u8]) -> Option<i64> {
            if digits(part) {
                std::str::from_utf8(part).ok()?.parse().ok()
            } else {
                None
            }
        }
        fn days_in(year: i32, month: u8) -> Option<u8> {
            match month {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
                4 | 6 | 9 | 11 => Some(30),
                2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => Some(29),
                2 => Some(28),
                _ => None,
            }
        }
        let parts: Vec<&[u8]> = raw.as_bytes().split(|b| *b == b'-').collect();
        match parts.as_slice() {
            [y] => {
                let year = num(y)?;
                (y.len() == 4 && (1000..=9999).contains(&year)).then(|| Self::Y(year as i32))
            }
            [y, m] => {
                let (year, month) = (num(y)?, u8::try_from(num(m)?).ok()?);
                (y.len() == 4
                    && m.len() == 2
                    && (1000..=9999).contains(&year)
                    && (1..=12).contains(&month))
                .then(|| Self::YM(year as i32, month))
            }
            [y, m, d] => {
                let (year, month) = (num(y)?, u8::try_from(num(m)?).ok()?);
                let day = u8::try_from(num(d)?).ok()?;
                (y.len() == 4
                    && m.len() == 2
                    && d.len() == 2
                    && (1000..=9999).contains(&year)
                    && (1..=12).contains(&month)
                    && day >= 1
                    && days_in(year as i32, month).is_some_and(|max| day <= max))
                .then(|| Self::Ymd(year as i32, month, day))
            }
            _ => None,
        }
    }

    fn as_recognized(self) -> Option<RecognizedDate> {
        match self {
            Self::Y(y) => RecognizedDate::new(y, None, None),
            Self::YM(y, m) => RecognizedDate::new(y, Some(m), None),
            Self::Ymd(y, m, d) => RecognizedDate::new(y, Some(m), Some(d)),
        }
    }

    fn year(self) -> i32 {
        match self {
            Self::Y(y) | Self::YM(y, _) | Self::Ymd(y, ..) => y,
        }
    }

    fn month(self) -> Option<u8> {
        match self {
            Self::Y(_) => None,
            Self::YM(_, m) | Self::Ymd(_, m, _) => Some(m),
        }
    }

    fn day(self) -> Option<u8> {
        match self {
            Self::Y(_) | Self::YM(..) => None,
            Self::Ymd(.., d) => Some(d),
        }
    }
}

/// Typed scan failures over the checksum-bound fixture bytes. Carries
/// fixture-independent identity only (offset/ordinal/reason) — never text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NativeDateScanError {
    XmlMalformed {
        offset: usize,
        reason: &'static str,
    },
    UnexpectedScope {
        ordinal: usize,
    },
    EntityInText {
        ordinal: usize,
    },
    NonUtf8Text {
        ordinal: usize,
    },
    /// A `<date>` element exists whose type is not `published`: a re-typed
    /// date must never be silently excluded from the denominator.
    RetypedDate {
        published: usize,
        total: usize,
    },
}

fn find_from(data: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || from >= data.len() {
        return None;
    }
    data[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| position + from)
}

fn tag_starts(data: &[u8], at: usize, name: &str, closing: bool) -> bool {
    let mut prefix = String::with_capacity(name.len() + 2);
    prefix.push('<');
    if closing {
        prefix.push('/');
    }
    prefix.push_str(name);
    data.len() >= at + prefix.len()
        && &data[at..at + prefix.len()] == prefix.as_bytes()
        && data.get(at + prefix.len()).is_none_or(|b| {
            !b.is_ascii_alphanumeric() && *b != b'_' && *b != b'-' && *b != b'.' && *b != b':'
        })
}

/// Plain bounded prefix check (comments/CDATA/PI probes carry `-`/`[` and
/// must not go through the element-name boundary check).
fn bytes_start_at(data: &[u8], at: usize, prefix: &[u8]) -> bool {
    data.len() >= at + prefix.len() && &data[at..at + prefix.len()] == prefix
}

fn xml_element_name(tag: &[u8]) -> Option<String> {
    let mut i = usize::from(tag.first() == Some(&b'/'));
    let start = i;
    if i < tag.len() && (tag[i].is_ascii_alphabetic() || tag[i] == b'_') {
        while i < tag.len()
            && (tag[i].is_ascii_alphanumeric()
                || tag[i] == b'_'
                || tag[i] == b'.'
                || tag[i] == b'-'
                || tag[i] == b':')
        {
            i += 1;
        }
        return Some(String::from_utf8_lossy(&tag[start..i]).into_owned());
    }
    None
}

/// Bounded attribute reader for one tag body (`name="value"` / `'value'`).
/// Known simplification: the tag body ends at the first `>` byte;
/// attribute values in the pinned oracle corpus never contain `>`.
fn xml_attr_map(tag: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tag.len() {
        let is_name_start = |b: u8| b.is_ascii_alphabetic() || b == b'_' || b == b':';
        let is_name_byte =
            |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b':' || b == b'.' || b == b'-';
        if is_name_start(tag[i]) {
            let start = i;
            while i < tag.len() && is_name_byte(tag[i]) {
                i += 1;
            }
            let name = String::from_utf8_lossy(&tag[start..i]).into_owned();
            while i < tag.len() && tag[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < tag.len() && tag[i] == b'=' {
                i += 1;
                while i < tag.len() && tag[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i < tag.len() && (tag[i] == b'"' || tag[i] == b'\'') {
                    let quote = tag[i];
                    i += 1;
                    let value_start = i;
                    while i < tag.len() && tag[i] != quote {
                        i += 1;
                    }
                    let value = String::from_utf8_lossy(&tag[value_start..i]).into_owned();
                    if i < tag.len() {
                        i += 1;
                    }
                    out.push((name, value));
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

/// One scanned `<date type="published">` element of the checksum-bound
/// fixture bytes. Display text and `when` stay in-memory only — they are
/// hashed and redacted before anything reaches the report.
#[derive(Debug)]
struct ScannedOccurrence {
    span: (usize, usize),
    scope: &'static str,
    when: String,
    text: String,
}

/// Byte-level scan for all and only `<date type="published">` occurrences
/// in the three sanctioned TEI scopes, in document order — the exact
/// ordered occurrence vector the T02 oracle contract pins.
fn scan_published_occurrences(data: &[u8]) -> Result<Vec<ScannedOccurrence>, NativeDateScanError> {
    let n = data.len();
    let mut i = 0usize;
    let mut stack: Vec<String> = Vec::new();
    let mut published: Vec<ScannedOccurrence> = Vec::new();
    let mut date_elements_total = 0usize;
    while i < n {
        let lt = match find_from(data, i, b"<") {
            Some(p) => p,
            None => break,
        };
        if bytes_start_at(data, lt, b"<!--") {
            i = match find_from(data, lt, b"-->") {
                Some(e) => e + 3,
                None => {
                    return Err(NativeDateScanError::XmlMalformed {
                        offset: lt,
                        reason: "unterminated_comment",
                    });
                }
            };
            continue;
        }
        if bytes_start_at(data, lt, b"<![CDATA[") {
            i = match find_from(data, lt, b"]]>") {
                Some(e) => e + 3,
                None => {
                    return Err(NativeDateScanError::XmlMalformed {
                        offset: lt,
                        reason: "unterminated_cdata",
                    });
                }
            };
            continue;
        }
        if data.get(lt) == Some(&b'?') || data.get(lt) == Some(&b'!') {
            i = match find_from(data, lt, b">") {
                Some(e) => e + 1,
                None => {
                    return Err(NativeDateScanError::XmlMalformed {
                        offset: lt,
                        reason: "unterminated_decl",
                    });
                }
            };
            continue;
        }
        let gt = match find_from(data, lt, b">") {
            Some(g) => g,
            None => {
                return Err(NativeDateScanError::XmlMalformed {
                    offset: lt,
                    reason: "unterminated_tag",
                });
            }
        };
        let tag = &data[lt + 1..gt];
        i = gt + 1;
        let closing = tag.first() == Some(&b'/');
        let name = match xml_element_name(tag) {
            Some(nm) => nm,
            None => continue,
        };
        let selfclose = tag.last() == Some(&b'/');
        if closing {
            match stack.last() {
                Some(top) if *top == name => {
                    stack.pop();
                }
                _ => {
                    return Err(NativeDateScanError::XmlMalformed {
                        offset: lt,
                        reason: "mismatched_close",
                    });
                }
            }
            continue;
        }
        if name == "date" {
            date_elements_total += 1;
            let attrs = xml_attr_map(tag);
            let is_published = attrs
                .iter()
                .any(|(key, value)| key == "type" && value == "published");
            if is_published {
                let scope = match stack
                    .iter()
                    .map(String::as_str)
                    .find(|s| *s == "publicationStmt" || *s == "sourceDesc" || *s == "listBibl")
                {
                    Some("publicationStmt") => NATIVE_DATE_SCOPES[0],
                    Some("sourceDesc") => NATIVE_DATE_SCOPES[1],
                    Some("listBibl") => NATIVE_DATE_SCOPES[2],
                    _ => {
                        return Err(NativeDateScanError::UnexpectedScope {
                            ordinal: published.len(),
                        });
                    }
                };
                if selfclose {
                    return Err(NativeDateScanError::XmlMalformed {
                        offset: lt,
                        reason: "published_date_selfclosed",
                    });
                }
                let mut j = i;
                let mut content_end: Option<usize> = None;
                while j < n {
                    let nxt = match find_from(data, j, b"<") {
                        Some(p) => p,
                        None => break,
                    };
                    if tag_starts(data, nxt, "date", true) {
                        content_end = Some(nxt);
                        j = match find_from(data, nxt, b">") {
                            Some(g) => g + 1,
                            None => {
                                return Err(NativeDateScanError::XmlMalformed {
                                    offset: nxt,
                                    reason: "unterminated_date_close",
                                });
                            }
                        };
                        break;
                    } else {
                        j = match find_from(data, nxt, b">") {
                            Some(g) => g + 1,
                            None => {
                                return Err(NativeDateScanError::XmlMalformed {
                                    offset: nxt,
                                    reason: "unterminated_inner_tag",
                                });
                            }
                        };
                    }
                }
                let content_end = match content_end {
                    Some(end) => end,
                    None => {
                        return Err(NativeDateScanError::XmlMalformed {
                            offset: lt,
                            reason: "date_not_closed",
                        });
                    }
                };
                let inner = &data[i..content_end];
                if inner.contains(&b'&') {
                    return Err(NativeDateScanError::EntityInText {
                        ordinal: published.len(),
                    });
                }
                let text =
                    std::str::from_utf8(inner).map_err(|_| NativeDateScanError::NonUtf8Text {
                        ordinal: published.len(),
                    })?;
                let when = attrs
                    .iter()
                    .find(|(key, _)| key == "when")
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                published.push(ScannedOccurrence {
                    span: (lt, j),
                    scope,
                    when,
                    text: text.to_owned(),
                });
                // Fully consumed through `</date>`: nothing pushed on the stack.
                i = j;
                continue;
            }
        }
        if !selfclose {
            stack.push(name);
        }
    }
    if date_elements_total != published.len() {
        return Err(NativeDateScanError::RetypedDate {
            published: published.len(),
            total: date_elements_total,
        });
    }
    Ok(published)
}

fn native_date_outcome_tag(outcome: &NativeDateOutcome) -> &'static str {
    match outcome {
        NativeDateOutcome::Recognized { .. } => "recognized",
        NativeDateOutcome::Empty => "empty",
        NativeDateOutcome::Malformed { .. } => "malformed",
        NativeDateOutcome::Unsupported { .. } => "unsupported",
        NativeDateOutcome::Ambiguous { .. } => "ambiguous",
        NativeDateOutcome::Oversized => "oversized",
    }
}

/// One per-occurrence shadow evidence row. Serialized content is
/// identity/span/hash/closed-outcome only — no display text, no `when`
/// value, no TEI fragment.
#[derive(Debug, Clone, serde::Serialize)]
struct NativeDateOccurrence {
    case_id: String,
    fixture_id: String,
    ordinal: usize,
    span: (usize, usize),
    scope: &'static str,
    input_sha256: String,
    oracle_when_sha256: String,
    outcome: NativeDateOutcome,
    precision: Option<DatePrecision>,
    strict: bool,
    soft: bool,
}

/// One candidate evaluation per oracle occurrence, plus the non-report
/// side products (latency probe, invented-component count).
#[derive(Debug)]
struct ShadowParse {
    row: NativeDateOccurrence,
    nanos: u128,
    invented_components: u64,
}

fn build_shadow_row(
    case_id: &str,
    fixture_id: &str,
    ordinal: usize,
    scanned: &ScannedOccurrence,
    parser: NativeDateParser,
) -> Result<ShadowParse, String> {
    let oracle = ShadowOracleWhen::parse(&scanned.when).ok_or_else(|| {
        format!(
            "[native-date-oracle-when] case `{case_id}` fixture `{fixture_id}` ordinal {ordinal}: {}",
            if scanned.when.is_empty() { "missing" } else { "malformed" }
        )
    })?;
    let started = Instant::now();
    let outcome = parser.parse(&scanned.text);
    let nanos = started.elapsed().as_nanos();
    let recognized = outcome.recognized();
    // Strict: the candidate matches the oracle exactly (every declared
    // component and the implied precision). Soft: year-level agreement.
    let strict = recognized == oracle.as_recognized();
    let soft = recognized.is_some_and(|r| r.year == oracle.year());
    let invented_components = recognized
        .map(|r| {
            u64::from(r.month.is_some() && oracle.month().is_none())
                + u64::from(r.day.is_some() && oracle.day().is_none())
        })
        .unwrap_or(0);
    Ok(ShadowParse {
        row: NativeDateOccurrence {
            case_id: case_id.to_string(),
            fixture_id: fixture_id.to_string(),
            ordinal,
            span: scanned.span,
            scope: scanned.scope,
            input_sha256: sha256_hex(scanned.text.as_bytes()),
            oracle_when_sha256: sha256_hex(scanned.when.as_bytes()),
            outcome,
            precision: recognized.map(|r| r.precision),
            strict,
            soft,
        },
        nanos,
        invented_components,
    })
}

/// Redacting deterministic digest: flat over the ordered occurrence vector,
/// covering case/fixture identity, ordinal, span, scope tag, both SHA-256
/// fields, the closed outcome wire and the strict/soft flags. Used by BOTH
/// the builder and self-validation (single code path — no drift).
fn native_date_shadow_digest(rows: &[NativeDateOccurrence]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(NATIVE_DATE_SHADOW_DIGEST_SCHEMA.as_bytes());
    for row in rows {
        hasher.update(row.case_id.as_bytes());
        hasher.update([0x00]);
        hasher.update(row.fixture_id.as_bytes());
        hasher.update([0x00]);
        hasher.update((row.ordinal as u64).to_le_bytes());
        hasher.update((row.span.0 as u64).to_le_bytes());
        hasher.update((row.span.1 as u64).to_le_bytes());
        hasher.update(row.scope.as_bytes());
        hasher.update([0x01]);
        hasher.update(row.input_sha256.as_bytes());
        hasher.update([0x02]);
        hasher.update(row.oracle_when_sha256.as_bytes());
        hasher.update([0x03]);
        hasher.update(serde_json::to_vec(&row.outcome).expect("closed outcome always serializes"));
        hasher.update([0x04]);
        hasher.update([u8::from(row.strict), u8::from(row.soft)]);
    }
    hex::encode(hasher.finalize())
}

fn latency_bucket(nanos: u128) -> &'static str {
    const US: u128 = 1_000;
    const MS: u128 = 1_000 * US;
    if nanos <= US {
        "le_1us"
    } else if nanos <= 10 * US {
        "le_10us"
    } else if nanos <= 100 * US {
        "le_100us"
    } else if nanos <= MS {
        "le_1ms"
    } else {
        "gt_1ms"
    }
}

/// Bucketized p50/p95/p99 characterization: exact wall-clock values are
/// never serialized, so two offline generations stay byte-identical.
fn latency_characterization(nanos: &mut [u128]) -> NativeDateLatencyCharacterization {
    nanos.sort_unstable();
    let pick = |q: usize| -> &'static str {
        if nanos.is_empty() {
            return "gt_1ms";
        }
        let index = (nanos.len() - 1) * q / 100;
        latency_bucket(nanos[index])
    };
    NativeDateLatencyCharacterization {
        p50: pick(50),
        p95: pick(95),
        p99: pick(99),
        bucket_vocabulary: &NATIVE_DATE_LATENCY_BUCKETS,
        policy: "bucketized closed vocabulary; exact wall-clock never serialized so two offline generations stay byte-identical",
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct NativeDateLatencyCharacterization {
    p50: &'static str,
    p95: &'static str,
    p99: &'static str,
    bucket_vocabulary: &'static [&'static str],
    policy: &'static str,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NativeDateFixtureEvidence {
    case_id: String,
    fixture_id: String,
    oracle_n: usize,
    candidate_n: usize,
    same_n: bool,
    strict_n: usize,
    soft_n: usize,
    invented_component_count: u64,
    malformed_count: u64,
    unsupported_count: u64,
    outcome_counts: BTreeMap<String, u64>,
    scope_counts: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NativeDateShadowEvidence {
    schema_version: &'static str,
    engine: String,
    candidate_id: String,
    candidate_version: String,
    outcome_schema_version: String,
    observation_mode: String,
    promotion_state: String,
    rollback_target: String,
    corpus_id: String,
    oracle_source: String,
    digest_schema: &'static str,
    denominator_total: usize,
    oracle_n: usize,
    candidate_n: usize,
    same_n: bool,
    strict_n: usize,
    soft_n: usize,
    invented_component_count: u64,
    malformed_count: u64,
    unsupported_count: u64,
    outcome_counts: BTreeMap<String, u64>,
    scope_counts: BTreeMap<String, u64>,
    per_fixture: Vec<NativeDateFixtureEvidence>,
    per_occurrence: Vec<NativeDateOccurrence>,
    aggregate_digest: String,
    latency_characterization: NativeDateLatencyCharacterization,
}

/// Per-case shadow parses collected while the case loop already owns the
/// checksum-bound fixture bytes.
struct ShadowCaseParse {
    case_id: String,
    fixture_id: String,
    parses: Vec<ShadowParse>,
}

fn count_rows_where(
    rows: &[&NativeDateOccurrence],
    pred: impl Fn(&NativeDateOccurrence) -> bool,
) -> usize {
    rows.iter().filter(|r| pred(r)).count()
}

fn bump_all_counts(target: &mut BTreeMap<String, u64>, counts: &BTreeMap<String, u64>) {
    for (key, value) in counts {
        *target.entry(key.clone()).or_insert(0) += value;
    }
}

/// Deterministic assembly of the additive shadow evidence from the
/// per-case parses. Fails visibly on any denominator drift against the
/// DATA-pinned per-fixture numbers.
fn assemble_native_date_shadow(
    terms: &NativeDateShadowTerms,
    cases: &[ShadowCaseParse],
) -> Result<NativeDateShadowEvidence, String> {
    let mut per_fixture = Vec::with_capacity(cases.len());
    let mut per_occurrence = Vec::new();
    let mut nanos: Vec<u128> = Vec::new();
    for case in cases {
        let pinned = terms
            .denominator_per_fixture
            .get(&case.case_id)
            .copied()
            .ok_or_else(|| {
                format!(
                    "[native-date-denominator] case `{}` absent from the DATA denominator",
                    case.case_id
                )
            })?;
        if case.parses.len() != pinned {
            return Err(format!(
                "[native-date-denominator-drift] case `{}`: pinned {pinned} occurrences, scanned {}",
                case.case_id,
                case.parses.len()
            ));
        }
        let rows: Vec<NativeDateOccurrence> = case.parses.iter().map(|p| p.row.clone()).collect();
        let borrowed: Vec<&NativeDateOccurrence> = rows.iter().collect();
        let mut outcome_counts: BTreeMap<String, u64> = BTreeMap::new();
        let mut scope_counts: BTreeMap<String, u64> = BTreeMap::new();
        for row in &rows {
            *outcome_counts
                .entry(native_date_outcome_tag(&row.outcome).to_string())
                .or_insert(0) += 1;
            *scope_counts.entry(row.scope.to_string()).or_insert(0) += 1;
        }
        let invented: u64 = case.parses.iter().map(|p| p.invented_components).sum();
        per_fixture.push(NativeDateFixtureEvidence {
            case_id: case.case_id.clone(),
            fixture_id: case.fixture_id.clone(),
            oracle_n: rows.len(),
            candidate_n: rows.len(),
            same_n: true,
            strict_n: count_rows_where(&borrowed, |r| r.strict),
            soft_n: count_rows_where(&borrowed, |r| r.soft),
            invented_component_count: invented,
            malformed_count: count_rows_where(&borrowed, |r| {
                native_date_outcome_tag(&r.outcome) == "malformed"
            }) as u64,
            unsupported_count: count_rows_where(&borrowed, |r| {
                native_date_outcome_tag(&r.outcome) == "unsupported"
            }) as u64,
            outcome_counts,
            scope_counts,
        });
        per_occurrence.extend(rows);
        nanos.extend(case.parses.iter().map(|p| p.nanos));
    }
    let empty_borrowed: Vec<&NativeDateOccurrence> = per_occurrence.iter().collect();
    let mut aggregate_outcome_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut aggregate_scope_counts: BTreeMap<String, u64> = BTreeMap::new();
    for fixture in &per_fixture {
        bump_all_counts(&mut aggregate_outcome_counts, &fixture.outcome_counts);
        bump_all_counts(&mut aggregate_scope_counts, &fixture.scope_counts);
    }
    Ok(NativeDateShadowEvidence {
        schema_version: NATIVE_DATE_SHADOW_SCHEMA,
        engine: terms.engine.clone(),
        candidate_id: terms.candidate_id.clone(),
        candidate_version: terms.candidate_version.clone(),
        outcome_schema_version: terms.outcome_schema_version.clone(),
        observation_mode: terms.observation_mode.clone(),
        promotion_state: terms.promotion_state.clone(),
        rollback_target: terms.rollback_target.clone(),
        corpus_id: terms.corpus_class.clone(),
        oracle_source: terms.oracle_source.clone(),
        digest_schema: NATIVE_DATE_SHADOW_DIGEST_SCHEMA,
        denominator_total: terms.denominator_total,
        oracle_n: empty_borrowed.len(),
        candidate_n: empty_borrowed.len(),
        same_n: per_fixture.iter().all(|f| f.same_n),
        strict_n: count_rows_where(&empty_borrowed, |r| r.strict),
        soft_n: count_rows_where(&empty_borrowed, |r| r.soft),
        invented_component_count: per_fixture.iter().map(|f| f.invented_component_count).sum(),
        malformed_count: count_rows_where(&empty_borrowed, |r| {
            native_date_outcome_tag(&r.outcome) == "malformed"
        }) as u64,
        unsupported_count: count_rows_where(&empty_borrowed, |r| {
            native_date_outcome_tag(&r.outcome) == "unsupported"
        }) as u64,
        outcome_counts: aggregate_outcome_counts,
        scope_counts: aggregate_scope_counts,
        per_fixture,
        aggregate_digest: format!("sha256:{}", native_date_shadow_digest(&per_occurrence)),
        per_occurrence,
        latency_characterization: latency_characterization(&mut nanos),
    })
}

// ──────────────────────── fixture resolution ──────────────────────────

struct ResolvedFixture {
    case_index: usize,
    source_path: String,
    source_sha256: String,
    provenance: ProvenanceFixture,
}

fn resolve_bound_fixtures(
    manifest: &Manifest,
    provenance: &Provenance,
    fixtures_root: &Path,
) -> Result<Vec<ResolvedFixture>, String> {
    let by_path: BTreeMap<&str, &ProvenanceFixture> = provenance
        .fixtures
        .iter()
        .map(|f| (f.path.as_str(), f))
        .collect();
    let mut resolved = Vec::new();
    for (idx, case) in manifest.cases.iter().enumerate() {
        if case.binding_status != "bound" {
            if case.binding_owner_task.as_deref().unwrap_or("").is_empty() {
                return Err(format!(
                    "[pending-unowned] case `{}` declares no binding_owner_task",
                    case.id
                ));
            }
            continue;
        }
        let path = &case.fixture.path;
        if path.starts_with('/') || path.split('/').any(|c| c == "..") {
            return Err(format!(
                "[fixture-path-escape] case `{}`: `{path}`",
                case.id
            ));
        }
        let rel = path.strip_prefix(FIXTURE_PREFIX).ok_or_else(|| {
            format!(
                "[fixture-path-outside-fixtures-root] case `{}`: `{path}`",
                case.id
            )
        })?;
        let declared_sha = case
            .fixture
            .sha256
            .as_deref()
            .filter(|s| is_sha256_hex(s))
            .ok_or_else(|| format!("[bound-without-sha256] case `{}`", case.id))?;

        let full = fixtures_root.join(rel);
        let bytes = std::fs::read(&full).map_err(|e| {
            format!(
                "[missing-fixture-file] case `{}`: {} ({e})",
                case.id,
                full.display()
            )
        })?;
        if bytes.is_empty() {
            return Err(format!(
                "[empty-fixture-file] case `{}`: {}",
                case.id,
                full.display()
            ));
        }
        let actual = sha256_hex(&bytes);
        if actual != declared_sha {
            return Err(format!(
                "[fixture-sha256-mismatch] case `{}`: declared `{declared_sha}` actual `{actual}`",
                case.id
            ));
        }
        let prov = by_path.get(path.as_str()).ok_or_else(|| {
            format!(
                "[provenance-missing-entry] case `{}`: `{path}` absent from provenance.yaml",
                case.id
            )
        })?;
        if prov.sha256 != actual {
            return Err(format!(
                "[provenance-sha256-mismatch] case `{}`: provenance `{}` actual `{actual}`",
                case.id, prov.sha256
            ));
        }
        if prov.path != *path {
            return Err(format!(
                "[provenance-path-mismatch] case `{}`: manifest `{path}` vs provenance `{}`",
                case.id, prov.path
            ));
        }
        resolved.push(ResolvedFixture {
            case_index: idx,
            source_path: path.clone(),
            source_sha256: actual,
            provenance: (**prov).clone(),
        });
    }
    if resolved.is_empty() {
        return Err("[no-bound-cases] manifest carries no bound cases to execute".into());
    }
    Ok(resolved)
}

fn validate_bindings_and_fingerprints(
    manifest: &Manifest,
    ledger: &serde_yaml::Value,
    registry_ids: &BTreeSet<String>,
) -> Result<(), String> {
    let ledger_ids = ledger_entry_ids(ledger);
    let mut seen = BTreeSet::new();
    for case in &manifest.cases {
        if !seen.insert(case.id.as_str()) {
            return Err(format!("[dup-id] duplicate case id `{}`", case.id));
        }
        for binding in &case.ledger_bindings {
            if !ledger_ids.contains(binding) && !registry_ids.contains(binding) {
                return Err(format!(
                    "[unresolved-ledger-ref] case `{}`: `{binding}` resolves to neither ledger entry nor registry capability",
                    case.id
                ));
            }
        }
        let live_fp = profile_by_name(&case.grobid_profile)?.configuration_fingerprint();
        if case.expected_cache_config_fingerprint != live_fp {
            return Err(format!(
                "[fingerprint-profile-mismatch] case `{}`: declared `{}` but live API identity is `{live_fp}`",
                case.id, case.expected_cache_config_fingerprint
            ));
        }
    }
    Ok(())
}

// ────────────────────────── hard regressions ──────────────────────────

fn hard_regression_statuses(
    contract: &ContractData,
    cases: &[CaseReport],
    determinism_held: bool,
) -> Vec<HardRegressionStatus> {
    let monitored = |what: &str| {
        format!(
            "monitored — per-case {what} counts are bound in this report; behavioral floors enforced by crates/da-parse real_tei_tests"
        )
    };
    contract
        .hard_regressions
        .iter()
        .map(|(id, description)| {
            let (status, evidence) = match id.as_str() {
                "nondeterministic_output" => (
                    if determinism_held { "pass" } else { "FAIL" },
                    "sha256 output identity computed twice per executed case in-process; identical digests required".to_string(),
                ),
                "silent_skip_missing_tei_fixture" => (
                    "pass",
                    "no skip path exists: missing/empty/checksum-mismatch fixtures abort with typed errors ([missing-fixture-file]/[empty-fixture-file]/[fixture-sha256-mismatch])".to_string(),
                ),
                "panic" => (
                    "pass",
                    "lift/project path is total string processing; failures surface as typed errors, no panic observed during this run".to_string(),
                ),
                "incorrect_cache_reuse" => (
                    "not_applicable_offline",
                    "direct-TEI path performs no cache reads or writes; profile fingerprint equality is asserted against the live GrobidProfile identity".to_string(),
                ),
                "lost_references_or_links" => ("monitored", monitored("citation/anchor")),
                "changed_reading_order" => ("monitored", monitored("section/path_id")),
                "lost_figures_tables_formulas" => ("monitored", monitored("table/cell/formula")),
                "invented_revision" => (
                    "pass",
                    "declared-only idno/revision lift since S07: the revision is carried only when explicitly declared in the raw `<idno type=\"arXiv\">` value; missing/malformed idno surfaces as typed declared_id_missingness, never a guessed revision (pinned by p0_lift_contract + real_tei_tests)".to_string(),
                ),
                "dropped_parser_work" => (
                    "not_applicable_offline",
                    "single-process in-memory run; no parser task queue involved".to_string(),
                ),
                "train_eval_contamination" => (
                    "not_applicable",
                    "no training or evaluation corpus is touched by this report".to_string(),
                ),
                "self_promotion" => (
                    "pass",
                    "promotion_state is reported verbatim from the comparison contract default (candidate); no promotion receipt is minted".to_string(),
                ),
                other => (
                    "unassessed",
                    format!("no offline check defined for hard regression `{other}`"),
                ),
            };
            HardRegressionStatus {
                id: id.clone(),
                description: description.clone(),
                status: status.to_string(),
                evidence: evidence.to_string(),
            }
        })
        .chain(cases.iter().map(|c| HardRegressionStatus {
            id: format!("per_case_counts.{}", c.case_id),
            description: "aggregate-counts-only observation row (no raw content)".to_string(),
            status: "observed".to_string(),
            evidence: format!(
                "sections={} citations={} tables={} formulas={} header_authors={} figures={} visual_callouts={} typed_missingness={} output_hash={}",
                c.family_counts.get("sections").unwrap_or(&0),
                c.family_counts.get("citations").unwrap_or(&0),
                c.family_counts.get("tables").unwrap_or(&0),
                c.family_counts.get("formulas").unwrap_or(&0),
                c.family_counts.get("header_authors").unwrap_or(&0),
                c.family_counts.get("figures").unwrap_or(&0),
                c.family_counts.get("visual_callouts").unwrap_or(&0),
                c.typed_missingness.values().sum::<u64>(),
                c.output_hash
            ),
        }))
        .collect()
}

// ───────────────────────────── Java mode ──────────────────────────────

fn locate_pdf(
    repo_root: &Path,
    java_pdf_dir: Option<&Path>,
    paper_id: &str,
    arxiv_id: &str,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = java_pdf_dir {
        for id in [paper_id, arxiv_id] {
            candidates.push(dir.join(format!("{id}.pdf")));
        }
    }
    let archive =
        repo_root.join("archive/data-legacy/article_corpora/m033-grobid-probe-v1/per-paper");
    for id in [paper_id, arxiv_id] {
        candidates.push(archive.join(id).join(format!("{id}.pdf")));
        candidates.push(archive.join(id).join("source").join(format!("{id}.pdf")));
    }
    candidates.into_iter().find(|p| p.is_file())
}

async fn probe_grobid_version(client: &reqwest::Client, base_url: &str) -> Result<String, String> {
    let url = format!("{base_url}/api/version");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GET /api/version transport failure: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET /api/version returned HTTP {}", resp.status()));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| format!("GET /api/version body read failure: {e}"))?;
    Ok(text.trim().to_string())
}

async fn build_java_evidence(
    repo_root: &Path,
    selection: &JavaSelection,
    cases: &[Case],
    resolved: &[ResolvedFixture],
    ledger: &serde_yaml::Value,
) -> JavaEvidence {
    let base_url = selection
        .url
        .clone()
        .or_else(|| std::env::var("GROBID_URL").ok())
        .unwrap_or_else(|| DEFAULT_GROBID_URL.to_string());
    let request_fields = match ledger_request_fields(ledger) {
        Ok(f) => f,
        Err(e) => {
            return JavaEvidence {
                selected: true,
                status: "unavailable".into(),
                reason: format!(
                    "ledger request-field snapshot unavailable, refusing to guess the wire contract: {e}"
                ),
                backend_url: Some(base_url),
                grobid_version: None,
                observations: None,
            };
        }
    };

    let parser = GrobidParser::new(Some(&base_url)).with_timeout(JAVA_PROBE_TIMEOUT);
    if !parser.is_alive().await {
        return JavaEvidence {
            selected: true,
            status: "unavailable".into(),
            reason: format!(
                "isalive probe against {base_url} failed within {:?}; backend treated unavailable — no version, request, latency or output values fabricated",
                JAVA_PROBE_TIMEOUT
            ),
            backend_url: Some(base_url),
            grobid_version: None,
            observations: None,
        };
    }

    let probe_client = match reqwest::Client::builder()
        .timeout(JAVA_PROBE_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return JavaEvidence {
                selected: true,
                status: "unavailable".into(),
                reason: format!("http client build failure: {e}"),
                backend_url: Some(base_url),
                grobid_version: None,
                observations: None,
            };
        }
    };
    let grobid_version = probe_grobid_version(&probe_client, &base_url).await.ok();

    let mut observations = Vec::new();
    for resolved_case in resolved {
        let case = &cases[resolved_case.case_index];
        let profile = match profile_by_name(&case.grobid_profile) {
            Ok(p) => p,
            Err(e) => {
                observations.push(JavaObservation {
                    case_id: case.id.clone(),
                    status: "unavailable".into(),
                    profile_fingerprint: String::new(),
                    request_fields_snapshot: request_fields.clone(),
                    pdf_sha256: None,
                    returned_tei_sha256: None,
                    output_hash: None,
                    latency_ms: None,
                    reason: Some(e),
                });
                continue;
            }
        };
        let pdf_path = locate_pdf(
            repo_root,
            selection.pdf_dir.as_deref(),
            &resolved_case.provenance.paper_id,
            &resolved_case.provenance.arxiv_id,
        );
        let Some(pdf_path) = pdf_path else {
            observations.push(JavaObservation {
                case_id: case.id.clone(),
                status: "not_observed".into(),
                profile_fingerprint: profile.configuration_fingerprint(),
                request_fields_snapshot: request_fields.clone(),
                pdf_sha256: None,
                returned_tei_sha256: None,
                output_hash: None,
                latency_ms: None,
                reason: Some(
                    "no original PDF source located (offline fixture set carries TEI only); provide --java-pdf-dir to capture live evidence".into(),
                ),
            });
            continue;
        };
        let pdf_bytes = match std::fs::read(&pdf_path) {
            Ok(b) => b,
            Err(e) => {
                observations.push(JavaObservation {
                    case_id: case.id.clone(),
                    status: "unavailable".into(),
                    profile_fingerprint: profile.configuration_fingerprint(),
                    request_fields_snapshot: request_fields.clone(),
                    pdf_sha256: None,
                    returned_tei_sha256: None,
                    output_hash: None,
                    latency_ms: None,
                    reason: Some(format!("pdf read failure {}: {e}", pdf_path.display())),
                });
                continue;
            }
        };
        let started = Instant::now();
        let outcome = parser
            .parse_pdf(
                &pdf_path.to_string_lossy(),
                &resolved_case.provenance.paper_id,
            )
            .await;
        let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match outcome {
            Ok(article) => {
                let tei_sha = article.tei_xml.as_deref().map(|t| sha256_hex(t.as_bytes()));
                observations.push(JavaObservation {
                    case_id: case.id.clone(),
                    status: "observed".into(),
                    profile_fingerprint: article_profile_fingerprint(&profile),
                    request_fields_snapshot: request_fields.clone(),
                    pdf_sha256: Some(sha256_hex(&pdf_bytes)),
                    returned_tei_sha256: tei_sha,
                    output_hash: Some(output_hash_of(&article)),
                    latency_ms: Some(latency_ms),
                    reason: None,
                });
            }
            Err(e) => {
                observations.push(JavaObservation {
                    case_id: case.id.clone(),
                    status: "unavailable".into(),
                    profile_fingerprint: profile.configuration_fingerprint(),
                    request_fields_snapshot: request_fields.clone(),
                    pdf_sha256: Some(sha256_hex(&pdf_bytes)),
                    returned_tei_sha256: None,
                    output_hash: None,
                    latency_ms: Some(latency_ms),
                    reason: Some(format!("typed parser error: {e}")),
                });
            }
        }
    }

    JavaEvidence {
        selected: true,
        status: "observed".into(),
        reason: "live observation mode explicitly selected via --java; latency is an inherently variable observation and is isolated from the deterministic compatibility identity".into(),
        backend_url: Some(base_url),
        grobid_version,
        observations: Some(observations),
    }
}

/// The parser's effective profile fingerprint (kept explicit for honesty).
fn article_profile_fingerprint(profile: &GrobidProfile) -> String {
    profile.configuration_fingerprint()
}

fn java_not_observed() -> JavaEvidence {
    JavaEvidence {
        selected: false,
        status: "not_observed".into(),
        reason: "java mode not selected; the default direct-TEI run never contacts GROBID (no version, request, latency or output values exist to report)".into(),
        backend_url: None,
        grobid_version: None,
        observations: None,
    }
}

// ─────────────────────────── report assembly ──────────────────────────

fn load_manifest(inputs: &BaselineInputs<'_>) -> Result<(Manifest, String), String> {
    let path = inputs
        .manifest_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| inputs.repo_root.join(DEFAULT_MANIFEST_REL));
    let text = read_text(&path, "regression manifest")?;
    let manifest: Manifest = parse_yaml(&text, "regression manifest")?;
    Ok((manifest, text))
}

fn build_baseline_report(inputs: &BaselineInputs<'_>) -> Result<BaselineReport, String> {
    // 1. Immutable DATA inputs.
    let (manifest, manifest_text) = load_manifest(inputs)?;
    if manifest.policy_locks != expected_locks() {
        return Err(
            "[hard-lock-violation] manifest policy_locks diverge from project locks".into(),
        );
    }
    let ledger_text = read_text(
        &inputs.repo_root.join(DEFAULT_LEDGER_REL),
        "observation ledger",
    )?;
    let ledger: serde_yaml::Value = parse_yaml(&ledger_text, "observation ledger")?;
    let registry_ids = registry_capability_ids(inputs.repo_root)?;
    let contract = load_contract(inputs.repo_root)?;
    let provenance_text = read_text(
        &inputs.fixtures_root.join(PROVENANCE_REL_IN_FIXTURES),
        "fixture provenance",
    )?;
    let provenance: Provenance = parse_yaml(&provenance_text, "fixture provenance")?;
    if provenance.policy_locks != expected_locks() {
        return Err(
            "[hard-lock-violation] provenance policy_locks diverge from project locks".into(),
        );
    }

    // 2. Binding + referential-integrity validation (fail visibly).
    validate_bindings_and_fingerprints(&manifest, &ledger, &registry_ids)?;
    let resolved = resolve_bound_fixtures(&manifest, &provenance, inputs.fixtures_root)?;

    // 3. Deterministic direct-TEI execution: lift + project twice per case.
    //    Any digest divergence aborts the run above, so reaching this point
    //    means determinism held for every executed case.
    let determinism_held = true;
    let mut case_reports = Vec::new();
    // S08/T03: per-case shadow parses collected while the loop already owns
    // the checksum-bound fixture bytes.
    let mut shadow_cases: Vec<ShadowCaseParse> = Vec::new();
    for r in &resolved {
        let case = &manifest.cases[r.case_index];
        let tei_bytes = std::fs::read(
            inputs.fixtures_root.join(
                r.source_path
                    .strip_prefix(FIXTURE_PREFIX)
                    .expect("prefix re-checked at resolution"),
            ),
        )
        .map_err(|e| format!("[missing-fixture-file] case `{}`: {e}", case.id))?;
        // The fail-visible UTF-8 gate is preserved: preserved fixtures must
        // be clean UTF-8 (the lifter's lossy decode is only a totality floor
        // for adversarial input, never an excuse for drifted fixtures).
        String::from_utf8(tei_bytes.clone())
            .map_err(|e| format!("[fixture-not-utf8] case `{}`: {e}", case.id))?;

        // S08/T03: the SAME checksum-bound bytes drive the ordered
        // <date type="published"> occurrence vector; the pure
        // NativeDateParser evaluates each occurrence as a shadow-only
        // candidate — exactly one evaluation per oracle row (structural
        // same-n). Typed failures abort the run: an outage never passes.
        // Runs BEFORE the envelope consumes the bytes.
        let scanned = scan_published_occurrences(&tei_bytes).map_err(|e| {
            format!(
                "[native-date-scan] case `{}` fixture `{}`: {e:?}",
                case.id, r.provenance.id
            )
        })?;
        let parser = NativeDateParser::new();
        let mut parses = Vec::with_capacity(scanned.len());
        for (ordinal, occurrence) in scanned.iter().enumerate() {
            parses.push(build_shadow_row(
                &case.id,
                &r.provenance.id,
                ordinal,
                occurrence,
                parser,
            )?);
        }
        let pinned = contract
            .native_date_shadow
            .denominator_per_fixture
            .get(&case.id)
            .copied()
            .ok_or_else(|| {
                format!(
                    "[native-date-denominator] case `{}` absent from the DATA denominator",
                    case.id
                )
            })?;
        if parses.len() != pinned {
            return Err(format!(
                "[native-date-denominator-drift] case `{}`: pinned {pinned} occurrences, scanned {}",
                case.id,
                parses.len()
            ));
        }
        shadow_cases.push(ShadowCaseParse {
            case_id: case.id.clone(),
            fixture_id: r.provenance.id.clone(),
            parses,
        });

        let profile = profile_by_name(&case.grobid_profile)?;
        // Exact envelope/IR pair (S03/T04): cached-TEI envelope identity
        // rides the case's own profile fingerprint; no HTTP anywhere.
        let envelope = TeiEnvelope::new(tei_bytes, TeiIdentity::cached_tei(profile));
        let engine = ConservationEngine::new();
        let lifter = TeiLifter::new();
        let ir = lifter.lift(
            &envelope,
            r.provenance.paper_id.clone(),
            r.source_sha256.clone(),
        );
        let delta_first = engine.compare(&envelope, &ir);
        let article = GrobidParser::project_article(ir);
        let hash_first = output_hash_of(&article);

        // Replay: identical bytes must reproduce the identical lift output
        // AND the identical conservation delta (full equality, not just the
        // digest).
        let ir_again = lifter.lift(
            &envelope,
            r.provenance.paper_id.clone(),
            r.source_sha256.clone(),
        );
        let delta_again = engine.compare(&envelope, &ir_again);
        let article_again = GrobidParser::project_article(ir_again);
        let hash_again = output_hash_of(&article_again);
        if hash_first != hash_again {
            return Err(format!(
                "[nondeterministic-output] case `{}`: output hash changed across in-process replay ({hash_first} vs {hash_again})",
                case.id
            ));
        }
        if delta_first != delta_again {
            return Err(format!(
                "[nondeterministic-conservation] case `{}`: conservation delta changed across in-process replay",
                case.id
            ));
        }

        let lifted = lifted_counts(&article);
        let typed_missingness = typed_missingness_counts(&article);
        case_reports.push(CaseReport {
            case_id: case.id.clone(),
            binding_status: case.binding_status.clone(),
            source_path: r.source_path.clone(),
            source_sha256: r.source_sha256.clone(),
            provenance_fixture_id: r.provenance.id.clone(),
            provenance_paper_id: r.provenance.paper_id.clone(),
            provenance_backend: provenance.backend.clone(),
            grobid_profile: case.grobid_profile.clone(),
            profile_fingerprint: profile.configuration_fingerprint(),
            stages: case.stages.clone(),
            stage_execution_mode: case.stage_execution_mode.clone(),
            ledger_bindings: case.ledger_bindings.clone(),
            obligation_acknowledged: case.obligation_acknowledged,
            family_counts: family_counts_of(&lifted),
            lifted_counts: lifted,
            typed_missingness,
            output_hash: hash_first,
            outcome: "parse_ok".into(),
            conservation: conservation_case_evidence(&delta_first),
        });
    }

    let conservation = aggregate_conservation(&case_reports);

    // S08/T03: additive shadow evidence — the same ordered occurrence
    // vector over the same checksum-bound bytes; the lift/conservation
    // path above is untouched and its aggregate identity is unchanged.
    let native_date_shadow =
        assemble_native_date_shadow(&contract.native_date_shadow, &shadow_cases)?;

    // 4. Aggregate deterministic identity (sorted, path-independent).
    let mut fixture_ids = BTreeSet::new();
    let mut source_lines = String::new();
    let mut output_lines = String::new();
    let mut family_totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut lifted_totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut missingness_totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut fingerprints = BTreeSet::new();
    for c in &case_reports {
        fixture_ids.insert(c.case_id.clone());
        source_lines.push_str(&format!(
            "{}\t{}\t{}\n",
            c.case_id, c.source_path, c.source_sha256
        ));
        output_lines.push_str(&format!("{}\t{}\n", c.case_id, c.output_hash));
        fingerprints.insert(c.profile_fingerprint.clone());
        for (k, v) in &c.family_counts {
            *family_totals.entry(k.clone()).or_insert(0) += v;
        }
        for (k, v) in &c.lifted_counts {
            *lifted_totals.entry(k.clone()).or_insert(0) += v;
        }
        for (k, v) in &c.typed_missingness {
            *missingness_totals.entry(k.clone()).or_insert(0) += v;
        }
    }
    let (backend_version, backend_version_basis) = backend_version(inputs.repo_root);
    let identity = ReportIdentity {
        source_hash: format!("sha256:{}", sha256_hex(source_lines.as_bytes())),
        backend_id: BACKEND_ID_DIRECT.into(),
        backend_version,
        backend_version_basis,
        profile_fingerprint: fingerprints.into_iter().collect::<Vec<_>>().join(";"),
        request_fields: vec![
            "mode=direct_tei_offline".into(),
            "stage=parse.stage.lift_tei(tei_xml,paper_id,pdf_hash)".into(),
            "stage=parse.stage.project_article(ir)".into(),
            "http_request=none; Java multipart fields are bound only under java_grobid evidence".into(),
        ],
        output_hash: format!("sha256:{}", sha256_hex(output_lines.as_bytes())),
        corpus_id: CORPUS_ID.into(),
        corpus_hash: format!("sha256:{}", sha256_hex(manifest_text.as_bytes())),
        corpus_n: case_reports.len(),
        same_n: true,
        same_n_meaning: "all executed bound cases and (when selected) java observations bind the identical tracked fixture set; the native_date_shadow candidate additionally evaluates the identical checksum-bound ordered published-date occurrence vector (denominator pinned by the comparison contract)".into(),
        fixture_ids: fixture_ids.into_iter().collect(),
        family_counts: family_totals,
        current_lifted_counts: lifted_totals,
        typed_missingness: missingness_totals,
        promotion_state: contract.promotion_default.clone(),
        rollback_target: contract.rollback_target.clone(),
        known_hard_regressions: hard_regression_statuses(&contract, &case_reports, determinism_held),
    };

    // 5. Typed absences for pending cases (honest, DATA-quoted).
    let mut pending_reports = Vec::new();
    for case in &manifest.cases {
        if case.binding_status == "bound" {
            continue;
        }
        let prov_reason = provenance
            .unbound
            .iter()
            .find(|u| u.path == case.fixture.path)
            .map(|u| u.reason.trim().to_string())
            .unwrap_or_default();
        let mut reason = prov_reason;
        if !case.notes.is_empty() {
            if !reason.is_empty() {
                reason.push(' ');
            }
            reason.push_str(&case.notes.join(" "));
        }
        pending_reports.push(PendingCaseReport {
            case_id: case.id.clone(),
            binding_owner_task: case.binding_owner_task.clone().unwrap_or_default(),
            expected_profile_fingerprint: case.expected_cache_config_fingerprint.clone(),
            status: "not_observed".into(),
            typed_absence: true,
            reason: if reason.is_empty() {
                "no structural-profile TEI bytes exist; none were fabricated".into()
            } else {
                reason
            },
        });
    }

    // 6. Java evidence — separate, explicit, honest.
    let java_grobid = if inputs.java.enabled {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("[runtime] tokio runtime build failure: {e}"))?;
        rt.block_on(build_java_evidence(
            inputs.repo_root,
            &inputs.java,
            &manifest.cases,
            &resolved,
            &ledger,
        ))
    } else {
        java_not_observed()
    };

    Ok(BaselineReport {
        schema_version: REPORT_SCHEMA_VERSION.into(),
        contract_id: CONTRACT_ID.into(),
        corpus_classes: json!({
            "used": CORPUS_ID,
            "location": "crates/da-parse/tests/fixtures/",
            "publisher_acceptance": "not_added — later slices; never fetched or executed by this command",
            "layout_omnidocbench": "not_added — later slices",
            "entity_f1_supporting": "not_used_as_parse_proof (supporting evidence only)"
        }),
        report_identity: identity,
        cases: case_reports,
        pending_cases: pending_reports,
        conservation,
        native_date_shadow,
        java_grobid,
        policy_locks: manifest.policy_locks,
    })
}

const CORPUS_ID: &str = CORPUS_CLASS;

// ──────────────────────── self-validation + write ─────────────────────

fn collect_object_keys(value: &serde_json::Value, keys: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                keys.push(k.clone());
                collect_object_keys(v, keys);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_object_keys(item, keys);
            }
        }
        _ => {}
    }
}

/// Contract compliance + raw-content leakage canaries. Executed on EVERY
/// render (CLI runs and tests alike), not only in tests.
fn self_validate(
    report: &BaselineReport,
    contract: &ContractData,
    raw_canaries: &[String],
) -> Result<(), String> {
    let value = serde_json::to_value(report).map_err(|e| format!("[serialization] {e}"))?;

    // Required identity fields present.
    let identity = value
        .get("report_identity")
        .ok_or("[self-validation] report_identity missing")?;
    for field in &contract.required_identity_fields {
        if identity.get(field).is_none() {
            return Err(format!(
                "[missing-identity-field] `{field}` absent from report_identity"
            ));
        }
    }

    // Forbidden report fields must never appear as keys.
    let mut keys = Vec::new();
    collect_object_keys(&value, &mut keys);
    for forbidden in &contract.forbidden_fields {
        if keys.iter().any(|k| k == forbidden) {
            return Err(format!(
                "[forbidden-field] `{forbidden}` leaked into the report"
            ));
        }
    }

    // Conservation evidence: contract-required per-case fields, exact
    // fixture/backend/profile identity binding, digest shape, and aggregate
    // == case-row sums (S03/T04).
    for case in &report.cases {
        let cons =
            serde_json::to_value(&case.conservation).map_err(|e| format!("[serialization] {e}"))?;
        for field in &contract.conservation_required_fields {
            if cons.get(field).is_none() {
                return Err(format!(
                    "[missing-conservation-field] `{field}` absent from case `{}` conservation evidence",
                    case.case_id
                ));
            }
        }
        if case.conservation.engine != CONSERVATION_ENGINE_ID
            || case.conservation.backend_id != CACHED_TEI_BACKEND_ID
            || case.conservation.raw_tei_sha256 != case.source_sha256
            || case.conservation.profile_fingerprint != case.profile_fingerprint
        {
            return Err(format!(
                "[conservation-binding-mismatch] case `{}`: delta identity does not bind the exact fixture/backend/profile identity",
                case.case_id
            ));
        }
        let digest = case.conservation.delta_digest.trim_start_matches("sha256:");
        if !is_sha256_hex(digest) {
            return Err(format!(
                "[conservation-digest-shape] case `{}`: delta digest is not a sha256 hex string",
                case.case_id
            ));
        }
    }
    let recomputed = aggregate_conservation(&report.cases);
    let recomputed_value =
        serde_json::to_value(&recomputed).map_err(|e| format!("[serialization] {e}"))?;
    let aggregate_value =
        serde_json::to_value(&report.conservation).map_err(|e| format!("[serialization] {e}"))?;
    if recomputed_value != aggregate_value {
        return Err(
            "[conservation-aggregate-mismatch] aggregate conservation counters are not the exact sum over per-case rows"
                .into(),
        );
    }
    if report.conservation.cases != report.cases.len() {
        return Err(
            "[conservation-same-n] conservation aggregate does not cover every executed case"
                .into(),
        );
    }

    // S08/T03: native date shadow — DATA-bound denominator, closed
    // redacting row surface, aggregate == per-fixture == per-occurrence
    // sums, same-n/invention guards, digest replay, closed latency buckets.
    validate_native_date_shadow_evidence(report, contract)?;

    // Raw-document canaries: bibliographic titles from provenance and TEI
    // structural markers must not appear anywhere in the serialized report.
    let serialized = serde_json::to_string(&value).expect("value re-serializes");
    for canary in raw_canaries {
        if serialized.contains(canary.as_str()) {
            return Err(format!(
                "[raw-content-leak] canary `{}` (document text) found in the report",
                canary
            ));
        }
    }
    for marker in ["<teiHeader", "<body", "biblStruct", "<?xml"] {
        if serialized.contains(marker) {
            return Err(format!(
                "[raw-content-leak] TEI marker `{marker}` found in the report"
            ));
        }
    }

    // Java honesty: unselected mode must not carry observed fields.
    if !report.java_grobid.selected
        && (report.java_grobid.grobid_version.is_some()
            || report.java_grobid.observations.is_some())
    {
        return Err("[java-dishonesty] unselected java mode carries observed values".into());
    }
    Ok(())
}

fn raw_content_canaries(provenance: &Provenance) -> Vec<String> {
    provenance
        .fixtures
        .iter()
        .map(|f| f.title.clone())
        .filter(|t| t.trim().len() >= 8)
        .collect()
}

/// S08/T03: shadow evidence validation — identity binding to the DATA
/// terms, closed per-occurrence surface, aggregate == per-fixture sums,
/// denominator/same-n/invention guards and digest replay.
fn validate_native_date_shadow_evidence(
    report: &BaselineReport,
    contract: &ContractData,
) -> Result<(), String> {
    let shadow = &report.native_date_shadow;
    let terms = &contract.native_date_shadow;
    if shadow.engine != terms.engine
        || shadow.candidate_id != terms.candidate_id
        || shadow.candidate_version != terms.candidate_version
        || shadow.outcome_schema_version != terms.outcome_schema_version
        || shadow.observation_mode != terms.observation_mode
        || shadow.promotion_state != terms.promotion_state
        || shadow.rollback_target != terms.rollback_target
        || shadow.oracle_source != terms.oracle_source
        || shadow.corpus_id != terms.corpus_class
        || shadow.denominator_total != terms.denominator_total
    {
        return Err(
            "[native-date-identity-drift] shadow candidate identity diverges from the DATA contract"
                .into(),
        );
    }
    for row in &shadow.per_occurrence {
        let value = serde_json::to_value(row).map_err(|e| format!("[serialization] {e}"))?;
        let object = value
            .as_object()
            .ok_or("[native-date-row-shape] occurrence row is not an object")?;
        for key in object.keys() {
            if !NATIVE_DATE_ROW_KEYS.contains(&key.as_str()) {
                return Err(format!(
                    "[native-date-row-leak] unexpected per-occurrence key `{key}`"
                ));
            }
        }
        if !terms.scope_order.iter().any(|s| s == row.scope) {
            return Err(format!(
                "[native-date-row-shape] unknown scope `{}`",
                row.scope
            ));
        }
        if !is_sha256_hex(&row.input_sha256) || !is_sha256_hex(&row.oracle_when_sha256) {
            return Err(format!(
                "[native-date-row-shape] case `{}` ordinal {}: occurrence hashes must be 64-hex",
                row.case_id, row.ordinal
            ));
        }
        if (row.strict || row.soft) && row.outcome.recognized().is_none() {
            return Err(format!(
                "[native-date-row-shape] case `{}` ordinal {}: strict/soft rows must be recognized",
                row.case_id, row.ordinal
            ));
        }
        if row.strict && row.precision.is_none() {
            return Err(format!(
                "[native-date-row-shape] case `{}` ordinal {}: strict rows must carry exact precision",
                row.case_id, row.ordinal
            ));
        }
    }
    if shadow.per_fixture.len() != terms.denominator_per_fixture.len() {
        return Err(
            "[native-date-same-n] shadow evidence does not cover every pinned fixture case".into(),
        );
    }
    for fixture in &shadow.per_fixture {
        let pinned = terms
            .denominator_per_fixture
            .get(&fixture.case_id)
            .copied()
            .ok_or_else(|| {
                format!(
                    "[native-date-denominator] fixture case `{}` not pinned in the DATA denominator",
                    fixture.case_id
                )
            })?;
        if fixture.oracle_n != pinned || fixture.candidate_n != pinned {
            return Err(format!(
                "[native-date-denominator-drift] case `{}`: pinned {pinned}, reported oracle {}/candidate {}",
                fixture.case_id, fixture.oracle_n, fixture.candidate_n
            ));
        }
        if !fixture.same_n {
            return Err("[native-date-same-n] per-fixture same_n drifted".into());
        }
        let rows: Vec<&NativeDateOccurrence> = shadow
            .per_occurrence
            .iter()
            .filter(|r| r.case_id == fixture.case_id)
            .collect();
        if rows.len() != fixture.oracle_n {
            return Err(format!(
                "[native-date-same-n] case `{}`: per-occurrence rows {} != oracle_n {}",
                fixture.case_id,
                rows.len(),
                fixture.oracle_n
            ));
        }
        if fixture.strict_n != rows.iter().filter(|r| r.strict).count()
            || fixture.soft_n != rows.iter().filter(|r| r.soft).count()
        {
            return Err(format!(
                "[native-date-counter-drift] case `{}`: strict/soft counters are not the exact row sums",
                fixture.case_id
            ));
        }
        for tag in ["malformed", "unsupported"] {
            let counted = rows
                .iter()
                .filter(|r| native_date_outcome_tag(&r.outcome) == tag)
                .count() as u64;
            let reported = if tag == "malformed" {
                fixture.malformed_count
            } else {
                fixture.unsupported_count
            };
            if reported != counted {
                return Err(format!(
                    "[native-date-counter-drift] case `{}`: {tag} count {reported} != row sum {counted}",
                    fixture.case_id
                ));
            }
        }
        let mut outcome_counts: BTreeMap<String, u64> = BTreeMap::new();
        let mut scope_counts: BTreeMap<String, u64> = BTreeMap::new();
        for row in &rows {
            *outcome_counts
                .entry(native_date_outcome_tag(&row.outcome).to_string())
                .or_insert(0) += 1;
            *scope_counts.entry(row.scope.to_string()).or_insert(0) += 1;
        }
        if outcome_counts != fixture.outcome_counts || scope_counts != fixture.scope_counts {
            return Err(format!(
                "[native-date-counter-drift] case `{}`: outcome/scope counts are not the exact row sums",
                fixture.case_id
            ));
        }
    }
    let oracle_sum: usize = shadow.per_fixture.iter().map(|f| f.oracle_n).sum();
    let candidate_sum: usize = shadow.per_fixture.iter().map(|f| f.candidate_n).sum();
    let strict_sum: usize = shadow.per_fixture.iter().map(|f| f.strict_n).sum();
    let soft_sum: usize = shadow.per_fixture.iter().map(|f| f.soft_n).sum();
    if shadow.oracle_n != oracle_sum
        || shadow.candidate_n != candidate_sum
        || shadow.strict_n != strict_sum
        || shadow.soft_n != soft_sum
    {
        return Err(
            "[native-date-counter-drift] aggregate n/strict/soft counters are not the exact per-fixture sums"
                .into(),
        );
    }
    let invented_sum: u64 = shadow
        .per_fixture
        .iter()
        .map(|f| f.invented_component_count)
        .sum();
    let malformed_sum: u64 = shadow.per_fixture.iter().map(|f| f.malformed_count).sum();
    let unsupported_sum: u64 = shadow.per_fixture.iter().map(|f| f.unsupported_count).sum();
    if shadow.invented_component_count != invented_sum
        || shadow.malformed_count != malformed_sum
        || shadow.unsupported_count != unsupported_sum
    {
        return Err(
            "[native-date-counter-drift] aggregate invented/malformed/unsupported counters are not the exact per-fixture sums"
                .into(),
        );
    }
    let mut aggregate_outcome_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut aggregate_scope_counts: BTreeMap<String, u64> = BTreeMap::new();
    for fixture in &shadow.per_fixture {
        bump_all_counts(&mut aggregate_outcome_counts, &fixture.outcome_counts);
        bump_all_counts(&mut aggregate_scope_counts, &fixture.scope_counts);
    }
    if aggregate_outcome_counts != shadow.outcome_counts
        || aggregate_scope_counts != shadow.scope_counts
    {
        return Err(
            "[native-date-counter-drift] aggregate outcome/scope counts are not the exact per-fixture sums"
                .into(),
        );
    }
    if shadow.oracle_n != shadow.denominator_total
        || shadow.per_occurrence.len() != shadow.denominator_total
    {
        return Err(
            "[native-date-denominator-erosion] shadow evidence erodes the pinned denominator"
                .into(),
        );
    }
    if !shadow.same_n {
        return Err("[native-date-same-n] aggregate same_n must stay true".into());
    }
    if shadow.invented_component_count > terms.invented_component_budget {
        return Err(format!(
            "[native-date-invention] invented components {} exceed the DATA budget {}",
            shadow.invented_component_count, terms.invented_component_budget
        ));
    }
    let digest = native_date_shadow_digest(&shadow.per_occurrence);
    if shadow.aggregate_digest != format!("sha256:{digest}") {
        return Err(
            "[native-date-digest-mismatch] aggregate digest does not replay from the occurrence rows"
                .into(),
        );
    }
    let latency = &shadow.latency_characterization;
    for bucket in [latency.p50, latency.p95, latency.p99] {
        if !NATIVE_DATE_LATENCY_BUCKETS.contains(&bucket) {
            return Err(format!(
                "[native-date-latency-vocabulary] latency bucket `{bucket}` is outside the closed vocabulary"
            ));
        }
    }
    Ok(())
}

fn build_and_render(inputs: &BaselineInputs<'_>) -> Result<(BaselineReport, String), String> {
    let provenance_text = read_text(
        &inputs.fixtures_root.join(PROVENANCE_REL_IN_FIXTURES),
        "fixture provenance",
    )?;
    let provenance: Provenance = parse_yaml(&provenance_text, "fixture provenance")?;
    let canaries = raw_content_canaries(&provenance);
    let contract = load_contract(inputs.repo_root)?;
    let report = build_baseline_report(inputs)?;
    self_validate(&report, &contract, &canaries)?;
    let mut text =
        serde_json::to_string_pretty(&report).map_err(|e| format!("[serialization] {e}"))?;
    text.push('\n');
    Ok((report, text))
}

fn write_report(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Err(format!("[io] cannot create {}: {e}", parent.display()));
    }
    std::fs::write(path, text).map_err(|e| format!("[io] cannot write {}: {e}", path.display()))
}

fn main() -> ExitCode {
    let _log_guard = da_cli::logging::init_pipeline_logging("parse_compat_baseline");
    let args = Args::parse();
    let repo_root = repo_root();
    let fixtures_root = resolve_against_repo(&repo_root, &args.fixtures);
    let manifest_path = args
        .manifest
        .as_ref()
        .map(|p| resolve_against_repo(&repo_root, p));
    let java = JavaSelection {
        enabled: args.java,
        url: args.grobid_url.clone(),
        pdf_dir: args
            .java_pdf_dir
            .as_ref()
            .map(|p| resolve_against_repo(&repo_root, p)),
    };
    let started = Instant::now();
    let inputs = BaselineInputs {
        repo_root: &repo_root,
        fixtures_root: &fixtures_root,
        manifest_path: manifest_path.as_deref(),
        java,
    };
    match build_and_render(&inputs) {
        Ok((report, text)) => {
            let report_path = resolve_against_repo(&repo_root, &args.report);
            if let Err(e) = write_report(&report_path, &text) {
                eprintln!("parse_compat_baseline: FAILED: {e}");
                return ExitCode::FAILURE;
            }
            let identity = &report.report_identity;
            println!("parse_compat_baseline: wrote {}", report_path.display());
            println!(
                "  corpus: {} (n={}, same_n={}), source_hash={}",
                identity.corpus_id, identity.corpus_n, identity.same_n, identity.source_hash
            );
            println!(
                "  output_hash={} profile={}",
                identity.output_hash, identity.profile_fingerprint
            );
            println!(
                "  conservation: cases={} verdicts={:?} total_rows={} loss_rows={} malformed_rows={}",
                report.conservation.cases,
                report.conservation.verdicts,
                report.conservation.total_rows,
                report.conservation.loss_rows,
                report.conservation.malformed_rows
            );
            println!(
                "  native_date_shadow: oracle_n={} candidate_n={} same_n={} strict_n={} invented={} malformed={} unsupported={} latency_p50={} p95={} p99={} digest={}",
                report.native_date_shadow.oracle_n,
                report.native_date_shadow.candidate_n,
                report.native_date_shadow.same_n,
                report.native_date_shadow.strict_n,
                report.native_date_shadow.invented_component_count,
                report.native_date_shadow.malformed_count,
                report.native_date_shadow.unsupported_count,
                report.native_date_shadow.latency_characterization.p50,
                report.native_date_shadow.latency_characterization.p95,
                report.native_date_shadow.latency_characterization.p99,
                report.native_date_shadow.aggregate_digest
            );
            println!("  family_counts: {:?}", identity.family_counts);
            println!("  typed_missingness: {:?}", identity.typed_missingness);
            println!(
                "  pending_cases={} java={}",
                report.pending_cases.len(),
                report.java_grobid.status
            );
            for c in &report.cases {
                println!(
                    "  case {}: sections={} citations={} tables={} formulas={} header_authors={} figures={} visual_callouts={} output_hash={} conservation={}/{} rows={} digest={}",
                    c.case_id,
                    c.family_counts.get("sections").unwrap_or(&0),
                    c.family_counts.get("citations").unwrap_or(&0),
                    c.family_counts.get("tables").unwrap_or(&0),
                    c.family_counts.get("formulas").unwrap_or(&0),
                    c.family_counts.get("header_authors").unwrap_or(&0),
                    c.family_counts.get("figures").unwrap_or(&0),
                    c.family_counts.get("visual_callouts").unwrap_or(&0),
                    c.output_hash,
                    c.conservation.verdict,
                    c.conservation.loss_rows,
                    c.conservation.total_rows,
                    c.conservation.delta_digest
                );
            }
            println!("  elapsed_ms={}", started.elapsed().as_millis());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("parse_compat_baseline: FAILED: {e}");
            ExitCode::FAILURE
        }
    }
}

// ─────────────────────────────── tests ────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_inputs(java: JavaSelection) -> (BaselineInputs<'static>, PathBuf) {
        let repo_root: &'static Path = Box::leak(Box::new(repo_root()));
        let fixtures_root: &'static Path =
            Box::leak(Box::new(repo_root.join("crates/da-parse/tests/fixtures")));
        (
            BaselineInputs {
                repo_root,
                fixtures_root,
                manifest_path: None,
                java,
            },
            repo_root.to_path_buf(),
        )
    }

    fn default_java() -> JavaSelection {
        JavaSelection::default()
    }

    #[test]
    fn report_is_deterministic_across_independent_runs() {
        let (inputs, _) = test_inputs(default_java());
        let (_, first) = build_and_render(&inputs).expect("first render");
        let (_, second) = build_and_render(&inputs).expect("second render");
        assert_eq!(first, second, "identical inputs must serialize identically");
        assert!(
            !first.contains("elapsed"),
            "wall-clock must not enter the report"
        );
    }

    #[test]
    fn identity_binds_source_profile_and_output_per_case() {
        let (inputs, repo) = test_inputs(default_java());
        let (report, _) = build_and_render(&inputs).expect("render");
        let manifest_text = std::fs::read_to_string(repo.join(DEFAULT_MANIFEST_REL)).unwrap();
        let manifest: Manifest = parse_yaml(&manifest_text, "manifest (test)").unwrap();

        assert_eq!(report.cases.len(), 3, "three bound cases execute");
        for case in &report.cases {
            let declared = manifest
                .cases
                .iter()
                .find(|c| c.id == case.case_id)
                .expect("case exists in manifest")
                .fixture
                .sha256
                .as_ref()
                .expect("bound case carries sha256");
            assert_eq!(
                &case.source_sha256, declared,
                "source hash must bind the manifest checksum"
            );
            let live_fp = profile_by_name(&case.grobid_profile)
                .expect("known profile")
                .configuration_fingerprint();
            assert_eq!(
                case.profile_fingerprint, live_fp,
                "fingerprint must bind the live API identity"
            );
            let hex_part = case
                .output_hash
                .strip_prefix("sha256:")
                .expect("sha256: prefix");
            assert!(is_sha256_hex(hex_part), "output hash is 64-hex");
        }
        // Distinct documents must produce distinct output identities.
        let mut hashes: Vec<&str> = report
            .cases
            .iter()
            .map(|c| c.output_hash.as_str())
            .collect();
        hashes.sort();
        hashes.dedup();
        assert_eq!(
            hashes.len(),
            3,
            "output hashes must differ across documents"
        );
        assert_eq!(report.report_identity.corpus_n, 3);
        assert_eq!(report.report_identity.promotion_state, "candidate");
        assert_eq!(
            report.report_identity.rollback_target,
            "java-grobid-http@0.9.0-crf"
        );
        assert!(!report.report_identity.backend_version.is_empty());
    }

    #[test]
    fn aggregate_counts_equal_per_case_sums() {
        let (inputs, _) = test_inputs(default_java());
        let (report, _) = build_and_render(&inputs).expect("render");
        for (family, total) in &report.report_identity.family_counts {
            let sum: u64 = report
                .cases
                .iter()
                .map(|c| c.family_counts.get(family).copied().unwrap_or(0))
                .sum();
            assert_eq!(
                *total, sum,
                "family {family} aggregate must equal the per-case sum"
            );
        }
        for (key, total) in &report.report_identity.current_lifted_counts {
            let sum: u64 = report
                .cases
                .iter()
                .map(|c| c.lifted_counts.get(key).copied().unwrap_or(0))
                .sum();
            assert_eq!(
                *total, sum,
                "lifted count {key} aggregate must equal the per-case sum"
            );
        }
    }

    #[test]
    fn absent_bound_fixture_fails_visibly() {
        let (inputs, _repo) = test_inputs(default_java());
        // Manifest override (in a tempdir, never polluting data/parse)
        // referencing a fixture that does not exist.
        let manifest = format!(
            "manifest_version: 1\nscope_note: negative test manifest\npolicy_locks:\n  import_eligible: false\n  dspy_enabled: false\n  graph_mutation: forbidden\n  llm_promotion: forbidden\n  free_label_invention: forbidden\n  sidecar_status: candidate_evidence_only\nprofile_fingerprint_binding_required: true\ncache_key_separation_rule: D204 separation\ncases:\n  - id: reg.tei.negative-absent\n    source_kind: preserved_grobid_tei\n    fixture:\n      path: crates/da-parse/tests/fixtures/preserved/__absent__.tei.xml\n      sha256: {}\n      origin: negative-test synthetic binding (never shipped)\n    binding_status: bound\n    grobid_profile: rich\n    expected_cache_config_fingerprint: profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula\n    stages: [tei_lift_ir]\n    stage_execution_mode: offline_local\n    expected_outcome: parse_ok\n    required_evidence_fields: [source_sha256, source_path, profile_fingerprint]\n    ledger_bindings: [parse.stage.lift_tei, rest.processFulltextDocument]\n    obligation_acknowledged: true\n",
            "a".repeat(64)
        );
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("negative_absent_manifest.yaml");
        std::fs::write(&path, &manifest).unwrap();
        let inputs = BaselineInputs {
            manifest_path: Some(path.as_path()),
            repo_root: inputs.repo_root,
            fixtures_root: inputs.fixtures_root,
            java: default_java(),
        };
        let err = build_and_render(&inputs).expect_err("absent fixture must fail");
        assert!(err.contains("[missing-fixture-file]"), "got: {err}");
    }

    #[test]
    fn checksum_mismatch_fails_visibly() {
        let (inputs, repo) = test_inputs(default_java());
        // Tamper one byte of a copied fixture inside a temp fixtures root.
        let tmp = tempfile::tempdir().expect("tempdir");
        let preserved = tmp.path().join("preserved");
        std::fs::create_dir_all(&preserved).unwrap();
        // Copy all three fixtures so the runner reaches rlm (case order:
        // hub, rlm, reca) and trips on its TAMPERED bytes, not on a missing
        // earlier file.
        let fixtures_dir = repo.join("crates/da-parse/tests/fixtures/preserved");
        for name in [
            "hub_2507.19457.tei.xml",
            "reca_2605.26525v1.tei.xml",
            "provenance.yaml",
        ] {
            std::fs::copy(fixtures_dir.join(name), preserved.join(name)).unwrap();
        }
        let original = fixtures_dir.join("rlm_2512.24601.tei.xml");
        let mut bytes = std::fs::read(&original).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] = bytes[mid].wrapping_add(1);
        std::fs::write(preserved.join("rlm_2512.24601.tei.xml"), &bytes).unwrap();

        let inputs = BaselineInputs {
            fixtures_root: tmp.path(),
            ..BaselineInputs {
                repo_root: inputs.repo_root,
                fixtures_root: inputs.fixtures_root,
                manifest_path: None,
                java: default_java(),
            }
        };
        let err = build_and_render(&inputs).expect_err("tampered fixture must fail");
        assert!(
            err.contains("[fixture-sha256-mismatch]")
                || err.contains("[provenance-sha256-mismatch]"),
            "got: {err}"
        );
    }

    #[test]
    fn no_raw_document_content_leaks_into_the_report() {
        let (inputs, repo) = test_inputs(default_java());
        let (report, text) = build_and_render(&inputs).expect("render");
        let prov_text = std::fs::read_to_string(
            repo.join("crates/da-parse/tests/fixtures/preserved/provenance.yaml"),
        )
        .unwrap();
        let prov: Provenance = parse_yaml(&prov_text, "provenance (test)").unwrap();
        for fixture in &prov.fixtures {
            assert!(
                !text.contains(&fixture.title),
                "bibliographic title must not leak: {}",
                fixture.title
            );
        }
        for marker in ["<teiHeader", "biblStruct", "<?xml", "<text>"] {
            assert!(
                !text.contains(marker),
                "TEI marker `{marker}` must not leak"
            );
        }
        // Identity/count/shape fields stay present while raw families stay out.
        assert!(text.contains("family_counts"));
        assert!(
            text.contains("\"conservation\"") && text.contains("delta_digest"),
            "conservation evidence must be present in the serialized report"
        );
        let value = serde_json::to_value(&report).unwrap();
        let mut keys = Vec::new();
        collect_object_keys(&value, &mut keys);
        for forbidden in FORBIDDEN_FALLBACK_FIELDS {
            assert!(!keys.contains(&forbidden.to_string()));
        }
    }

    #[test]
    fn java_mode_defaults_to_honest_not_observed() {
        let (inputs, _) = test_inputs(default_java());
        let (report, _) = build_and_render(&inputs).expect("render");
        assert!(!report.java_grobid.selected);
        assert_eq!(report.java_grobid.status, "not_observed");
        assert!(report.java_grobid.grobid_version.is_none());
        assert!(report.java_grobid.observations.is_none());
        assert!(report.java_grobid.backend_url.is_none());
        assert!(
            report
                .pending_cases
                .iter()
                .any(|p| p.typed_absence && p.status == "not_observed")
        );
    }

    #[test]
    fn java_unavailable_is_reported_honestly_without_fabricated_values() {
        // Port 9 (discard) is closed in this environment: connection refused.
        let java = JavaSelection {
            enabled: true,
            url: Some("http://127.0.0.1:9".into()),
            pdf_dir: None,
        };
        let (inputs, _) = test_inputs(java);
        let (report, text) =
            build_and_render(&inputs).expect("render succeeds even when java is down");
        assert!(report.java_grobid.selected);
        assert_eq!(report.java_grobid.status, "unavailable");
        assert!(report.java_grobid.reason.contains("isalive"));
        assert!(
            report.java_grobid.grobid_version.is_none(),
            "no fabricated version"
        );
        assert!(
            report.java_grobid.observations.is_none(),
            "no fabricated observations"
        );
        assert!(!text.contains("\"latency_ms\""), "no fabricated latency");
        // The direct-TEI baseline itself must stay fully successful.
        assert_eq!(report.report_identity.corpus_n, 3);
        assert!(
            report
                .report_identity
                .known_hard_regressions
                .iter()
                .any(|r| r.status == "pass")
        );
    }

    #[test]
    fn structural_identity_case_is_a_typed_absence() {
        let (inputs, _) = test_inputs(default_java());
        let (report, _) = build_and_render(&inputs).expect("render");
        let structural: Vec<_> = report
            .pending_cases
            .iter()
            .filter(|p| {
                p.expected_profile_fingerprint
                    .contains("profile=structural")
            })
            .collect();
        assert_eq!(structural.len(), 1, "exactly one structural identity case");
        assert_eq!(structural[0].status, "not_observed");
        assert!(structural[0].typed_absence);
        assert_eq!(structural[0].binding_owner_task, "S01/T03");
        assert!(
            structural[0]
                .reason
                .contains("No preserved structural-profile TEI"),
            "reason must quote the DATA-owned provenance text"
        );
        // No structural TEI bytes were fabricated into the tracked fixture set.
        assert!(
            !inputs
                .fixtures_root
                .join("preserved/hub_2507.19457.structural.tei.xml")
                .exists(),
            "structural fixture must NOT be invented"
        );
    }

    // ────────────── S03/T04 conservation evidence ──────────────

    const HUB_FIXTURE_REL: &str = "crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml";

    fn find_sub(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    /// M2-style injected corruption (mirrors the S03/T02 contract helper):
    /// break the first `</head>` close tag so the source span is damaged.
    fn corrupt_first_head_close_tag(bytes: &[u8]) -> Vec<u8> {
        let position = find_sub(bytes, b"</head>").expect("fixture carries a </head> close tag");
        let mut mutated = bytes.to_vec();
        mutated.splice(position..position + 7, b"</heab>".iter().copied());
        mutated
    }

    #[test]
    fn conservation_aggregate_equals_case_sums_and_binds_exact_identity() {
        let (inputs, _) = test_inputs(default_java());
        let (report, text) = build_and_render(&inputs).expect("render");
        let aggregate = &report.conservation;
        assert_eq!(aggregate.cases, 3, "same-n conservation coverage");
        assert_eq!(
            aggregate.verdicts.get("loss_detected"),
            Some(&3),
            "pristine preserved baseline is pinned loss_detected (S03/T02)"
        );
        assert_eq!(
            aggregate.malformed_rows, 0,
            "pristine fixtures carry no malformed bytes"
        );
        for (reason, total) in &aggregate.reason_counts {
            let sum: u64 = report
                .cases
                .iter()
                .map(|c| {
                    c.conservation
                        .reason_counts
                        .get(reason)
                        .copied()
                        .unwrap_or(0)
                })
                .sum();
            assert_eq!(
                *total, sum,
                "reason {reason} aggregate must equal the case sum"
            );
        }
        for (key, total) in &aggregate.structure_counts {
            let sum: u64 = report
                .cases
                .iter()
                .map(|c| {
                    c.conservation
                        .structure_counts
                        .get(key)
                        .copied()
                        .unwrap_or(0)
                })
                .sum();
            assert_eq!(
                *total, sum,
                "structure {key} aggregate must equal the case sum"
            );
        }
        for (family, counts) in &aggregate.family_counts {
            let mut sum = FamilyDeltaCounts::default();
            for case in &report.cases {
                let per = case
                    .conservation
                    .family_counts
                    .get(family)
                    .copied()
                    .unwrap_or_default();
                sum.source += per.source;
                sum.candidate += per.candidate;
                sum.retained += per.retained;
                sum.unsupported += per.unsupported;
                sum.lost += per.lost;
                sum.malformed += per.malformed;
            }
            assert_eq!(
                counts, &sum,
                "family {family:?} aggregate must equal the case sum"
            );
        }
        let total_rows: u64 = report.cases.iter().map(|c| c.conservation.total_rows).sum();
        assert_eq!(aggregate.total_rows, total_rows);
        let loss_rows: u64 = report.cases.iter().map(|c| c.conservation.loss_rows).sum();
        assert_eq!(aggregate.loss_rows, loss_rows);
        let source_spans: u64 = report
            .cases
            .iter()
            .map(|c| c.conservation.source_span_retention.source_spans)
            .sum();
        assert_eq!(aggregate.source_span_retention.source_spans, source_spans);
        assert!(
            aggregate.source_span_retention.source_spans > 0,
            "every observed source span must be counted"
        );
        for case in &report.cases {
            let cons = &case.conservation;
            assert_eq!(cons.engine, CONSERVATION_ENGINE_ID);
            assert_eq!(cons.backend_id, CACHED_TEI_BACKEND_ID);
            assert_eq!(
                cons.raw_tei_sha256, case.source_sha256,
                "delta identity binds the exact fixture checksum"
            );
            assert_eq!(cons.profile_fingerprint, case.profile_fingerprint);
            assert_eq!(cons.paper_id, case.provenance_paper_id);
            assert!(is_sha256_hex(
                cons.delta_digest.trim_start_matches("sha256:")
            ));
            assert!(cons.total_rows > 0);
        }
        assert_eq!(aggregate.delta_digests.len(), 3, "one digest per case");
        assert!(
            text.contains("\"conservation\"") && text.contains("delta_digest"),
            "serialized report must carry the conservation evidence"
        );
    }

    #[test]
    fn injected_family_loss_yields_exact_nonzero_typed_reasons() {
        let (inputs, repo) = test_inputs(default_java());
        let (report, _) = build_and_render(&inputs).expect("render");
        let row = report
            .cases
            .iter()
            .find(|c| c.source_path.ends_with("hub_2507.19457.tei.xml"))
            .expect("hub case present");
        let (paper, sha, emitter_digest) = (
            row.provenance_paper_id.clone(),
            row.source_sha256.clone(),
            row.conservation.delta_digest.clone(),
        );
        let bytes = std::fs::read(repo.join(HUB_FIXTURE_REL)).expect("hub fixture readable");
        let envelope = TeiEnvelope::new(bytes, TeiIdentity::cached_tei(GrobidProfile::rich()));
        let engine = ConservationEngine::new();
        let lifter = TeiLifter::new();
        let pristine = engine.compare(&envelope, &lifter.lift(&envelope, paper, sha));
        assert_eq!(
            pristine.delta_digest, emitter_digest,
            "the emitter baseline and a direct engine run must agree exactly"
        );

        // Injected loss: delete one lifted citation item from the candidate.
        let mut ir = lifter.lift(&envelope, "2507.19457", "hub");
        ir.citations.pop();
        let mutated = engine.compare(&envelope, &ir);
        assert_eq!(mutated.verdict.as_str(), "loss_detected");
        let pristine_reasons = reason_counts_of(&pristine);
        let mutated_reasons = reason_counts_of(&mutated);
        let bump =
            |counts: &BTreeMap<String, u64>, key: &str| counts.get(key).copied().unwrap_or(0);
        assert_eq!(
            bump(&mutated_reasons, "observed_but_missing"),
            bump(&pristine_reasons, "observed_but_missing") + 1,
            "exactly one extra observed_but_missing row"
        );
        assert_eq!(
            bump(&mutated_reasons, "count_mismatch"),
            bump(&pristine_reasons, "count_mismatch"),
            "count_mismatch is a family-level row; the citation family already carries exactly one in the pristine baseline (header sourceDesc biblStruct)"
        );
        let pristine_citations = pristine
            .family_counts
            .get(&ConservationFamily::CitationEntry)
            .copied()
            .unwrap_or_default();
        let mutated_citations = mutated
            .family_counts
            .get(&ConservationFamily::CitationEntry)
            .copied()
            .unwrap_or_default();
        assert_eq!(
            mutated_citations.lost,
            pristine_citations.lost + 1,
            "citation_entry lost count must move by exactly one"
        );
        assert_eq!(
            mutated.entries.len(),
            pristine.entries.len(),
            "the lost item's retained row becomes the extra observed_but_missing row; the family-level count_mismatch row stays single"
        );
        assert_ne!(mutated.delta_digest, pristine.delta_digest);
    }

    #[test]
    fn injected_malformed_bytes_yield_nonzero_malformed_summary_and_stay_bounded() {
        let (_inputs, repo) = test_inputs(default_java());
        let bytes = std::fs::read(repo.join(HUB_FIXTURE_REL)).expect("hub fixture readable");
        let corrupted_bytes = corrupt_first_head_close_tag(&bytes);
        let envelope = TeiEnvelope::new(
            corrupted_bytes.clone(),
            TeiIdentity::cached_tei(GrobidProfile::rich()),
        );
        let engine = ConservationEngine::new();
        let lifter = TeiLifter::new();
        let ir = lifter.lift(&envelope, "2507.19457", "hub");
        let first = engine.compare(&envelope, &ir);
        let replay = engine.compare(&envelope, &ir);
        assert_eq!(
            first, replay,
            "conservation replay must stay byte-identical on malformed bytes"
        );
        assert_eq!(first.verdict.as_str(), "malformed_source");
        let evidence = conservation_case_evidence(&first);
        assert!(
            evidence.malformed_rows >= 1,
            "malformed summary must count the injected corruption"
        );
        assert!(
            evidence
                .reason_counts
                .get("malformed_source")
                .copied()
                .unwrap_or(0)
                >= 1
        );
        let sections = evidence
            .family_counts
            .get(&ConservationFamily::Section)
            .copied()
            .unwrap_or_default();
        assert!(
            sections.malformed >= 1,
            "the corrupted </head> close tag must surface as a malformed section span"
        );
        assert!(
            evidence.delta.entries.iter().all(|e| e
                .byte_range
                .map_or(true, |range| range.is_valid(corrupted_bytes.len()))),
            "every reported byte range must satisfy the safe-bounds invariant"
        );
    }

    // ────────────── S08/T03 native date shadow evidence ──────────────

    #[test]
    fn native_date_shadow_binds_the_173_denominator_with_strict_parity() {
        let (inputs, _) = test_inputs(default_java());
        let (report, text) = build_and_render(&inputs).expect("render");
        let shadow = &report.native_date_shadow;
        assert_eq!(shadow.denominator_total, 173);
        assert_eq!(shadow.oracle_n, 173);
        assert_eq!(shadow.candidate_n, 173);
        assert!(shadow.same_n);
        assert_eq!(
            shadow.strict_n, 173,
            "checksum-bound strict parity must stay 173/173"
        );
        assert_eq!(shadow.soft_n, 173);
        assert_eq!(
            shadow.per_occurrence.len(),
            173,
            "full digest-bound shadow denominator"
        );
        assert_eq!(shadow.invented_component_count, 0, "no invented components");
        assert_eq!(shadow.malformed_count, 0);
        assert_eq!(shadow.unsupported_count, 0);
        // Pinned per-fixture split: 80 + 50 + 43.
        let per_fixture: BTreeMap<&str, usize> = shadow
            .per_fixture
            .iter()
            .map(|f| (f.case_id.as_str(), f.oracle_n))
            .collect();
        assert_eq!(
            per_fixture.get("reg.tei.hub-bibliography-2507.19457"),
            Some(&80)
        );
        assert_eq!(per_fixture.get("reg.tei.rlm-2512.24601"), Some(&50));
        assert_eq!(
            per_fixture.get("reg.tei.reca-bibliography-2605.26525"),
            Some(&43)
        );
        // Pinned per-scope split [3, 3, 167] never collapses scopes.
        let mut scope_counts: BTreeMap<&str, usize> = BTreeMap::new();
        for row in &shadow.per_occurrence {
            *scope_counts.entry(row.scope).or_insert(0) += 1;
        }
        assert_eq!(scope_counts.get("header_article_date"), Some(&3));
        assert_eq!(scope_counts.get("header_bibliography_date"), Some(&3));
        assert_eq!(scope_counts.get("bibliography_date"), Some(&167));
        // Candidate identity: live da-parse constants + honest shadow state.
        assert_eq!(shadow.engine, "da_parse::NativeDateParser");
        assert_eq!(shadow.candidate_id, NATIVE_DATE_PARSER_ID);
        assert_eq!(shadow.candidate_version, NATIVE_DATE_PARSER_VERSION);
        assert_eq!(
            shadow.outcome_schema_version,
            NATIVE_DATE_OUTCOME_SCHEMA_VERSION
        );
        assert_eq!(shadow.observation_mode, "shadow");
        assert_eq!(shadow.promotion_state, "candidate");
        assert_eq!(shadow.rollback_target, "java-grobid-http@0.9.0-crf");
        assert_eq!(shadow.corpus_id, "fast_local_fixtures");
        assert_eq!(shadow.oracle_source, "tei_when_attribute");
        assert!(text.contains("\"native_date_shadow\""));
        assert!(text.contains("aggregate_digest"));
    }

    #[test]
    fn native_date_shadow_rows_are_redacted_and_digest_bound() {
        let (inputs, _) = test_inputs(default_java());
        let (report, text) = build_and_render(&inputs).expect("render");
        let shadow = &report.native_date_shadow;
        let closed_outcome_tags = [
            "recognized",
            "empty",
            "malformed",
            "unsupported",
            "ambiguous",
            "oversized",
        ];
        for row in &shadow.per_occurrence {
            let value = serde_json::to_value(row).unwrap();
            for key in value.as_object().unwrap().keys() {
                assert!(
                    NATIVE_DATE_ROW_KEYS.contains(&key.as_str()),
                    "unexpected row key {key}"
                );
            }
            assert!(is_sha256_hex(&row.input_sha256));
            assert!(is_sha256_hex(&row.oracle_when_sha256));
            assert!(closed_outcome_tags.contains(&native_date_outcome_tag(&row.outcome)));
        }
        // No TEI marker and no `when=` attribute may enter the evidence.
        for marker in ["<date", "when=\""] {
            assert!(!text.contains(marker), "`{marker}` must never leak");
        }
        // Digest replay across two independent in-process builds.
        let (_, second) = build_and_render(&inputs).expect("second render");
        assert_eq!(
            text, second,
            "two offline generations must stay byte-identical"
        );
    }

    #[test]
    fn native_date_scanner_rejects_corruption_typewise() {
        // (a) A re-typed <date> never silently leaves the denominator.
        let doc = b"<TEI><listBibl><bibl><date type=\"published\" when=\"2024-01-02\">Jan 2, 2024</date></bibl><bibl><date type=\"issued\" when=\"2020-01-01\">2020</date></bibl></listBibl></TEI>";
        let err = scan_published_occurrences(doc).expect_err("re-typed date must fail");
        assert_eq!(
            err,
            NativeDateScanError::RetypedDate {
                published: 1,
                total: 2
            }
        );
        // (b) Unterminated published element.
        let err =
            scan_published_occurrences(b"<listBibl><date type=\"published\" when=\"2024\">oops")
                .expect_err("unterminated date must fail");
        assert!(matches!(err, NativeDateScanError::XmlMalformed { .. }));
        // (c) Published date outside the three sanctioned scopes.
        let err = scan_published_occurrences(
            b"<TEI><text><body><p><date type=\"published\" when=\"2024\">2024</date></p></body></text></TEI>",
        )
        .expect_err("unexpected scope must fail");
        assert_eq!(err, NativeDateScanError::UnexpectedScope { ordinal: 0 });
        // (d) Entity in display text.
        let err = scan_published_occurrences(
            b"<listBibl><date type=\"published\" when=\"2024\">2024&amp;</date></listBibl>",
        )
        .expect_err("entity text must fail");
        assert_eq!(err, NativeDateScanError::EntityInText { ordinal: 0 });
        // (e) Oracle-when failures are typed, never skipped.
        let parser = NativeDateParser::new();
        let scanned = scan_published_occurrences(
            b"<listBibl><date type=\"published\">no when</date></listBibl>",
        )
        .expect("scan ok");
        let err = build_shadow_row("c", "f", 0, &scanned[0], parser).unwrap_err();
        assert!(
            err.contains("[native-date-oracle-when]") && err.contains("missing"),
            "got: {err}"
        );
        let scanned = scan_published_occurrences(
            b"<listBibl><date type=\"published\" when=\"2024-13-99\">bad</date></listBibl>",
        )
        .expect("scan ok");
        let err = build_shadow_row("c", "f", 0, &scanned[0], parser).unwrap_err();
        assert!(err.contains("malformed"), "got: {err}");
    }

    #[test]
    fn native_date_shadow_strict_soft_and_invention_semantics() {
        let parser = NativeDateParser::new();
        // Strict: candidate equals the oracle exactly.
        let occurrence = ScannedOccurrence {
            span: (0, 8),
            scope: "header_article_date",
            when: "2025-07-16".into(),
            text: "2025. July 16, 2025".into(),
        };
        let parsed = build_shadow_row("c", "f", 0, &occurrence, parser).expect("row");
        assert!(parsed.row.strict && parsed.row.soft);
        assert_eq!(parsed.invented_components, 0);
        assert_eq!(parsed.row.precision, Some(DatePrecision::FullDate));
        // Soft-but-not-strict: a year-only oracle against a fuller candidate —
        // the candidate's month/day count as invented components.
        let occurrence = ScannedOccurrence {
            span: (0, 8),
            scope: "bibliography_date",
            when: "2019".into(),
            text: "2019-01-15".into(),
        };
        let parsed = build_shadow_row("c", "f", 0, &occurrence, parser).expect("row");
        assert!(!parsed.row.strict);
        assert!(parsed.row.soft);
        assert_eq!(parsed.invented_components, 2);
        // Non-recognized candidates are neither strict nor soft.
        let occurrence = ScannedOccurrence {
            span: (0, 8),
            scope: "bibliography_date",
            when: "2021-05-06".into(),
            text: "2019-01-15x".into(),
        };
        let parsed = build_shadow_row("c", "f", 0, &occurrence, parser).expect("row");
        assert!(!parsed.row.strict && !parsed.row.soft);
        assert_eq!(parsed.invented_components, 0);
        assert_eq!(native_date_outcome_tag(&parsed.row.outcome), "malformed");
    }
}
