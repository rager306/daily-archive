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
//! Since S04/T04 the gate additionally enforces:
//! - the fulltext endpoint entry documents the S04 typed transport
//!   (`typed_transport:` structured record) and it cross-validates against
//!   the LIVE da_parse vocabulary — request schema version, the closed
//!   eight-code failure vocabulary, the exact 200/204/400/413/503 status
//!   taxonomy, honest no-retry 503 with bounded Retry-After metadata and the
//!   two-variant compatibility facade mapping;
//! - the ledger wire dialect stays inside the registry-of-record official
//!   dialect (no invented fields/modes/families);
//! - ledger_version stays 1: the legacy wire contract and D204 identities
//!   are byte-unchanged (additive typed observation, not wire evolution).
//!
//! Since S05/T03 the gate additionally enforces:
//! - the two sanctioned proxy endpoint entries
//!   (parse.endpoint.process_header_document,
//!   parse.endpoint.process_references_document) carry their own
//!   `typed_transport` records, cross-validated against the LIVE da_parse
//!   vocabulary: closed endpoint/result-kind names, per-endpoint request
//!   schema versions, result identity namespaces that prefix the live
//!   canonical identities, the SHARED eight-code failure schema, the exact
//!   status taxonomy, honest no-retry 503 with bounded Retry-After, the
//!   registry-of-record rollback target and the exact closed multipart field
//!   allowlists on the exact static routes;
//! - the registry-of-record official dialects of all three sanctioned
//!   endpoints equal the live allowlists (header/references carry NO
//!   teiCoordinates — the pre-S05 DATA drift is corrected and must never
//!   return), cite the live static routes and stay in `proxy` state;
//! - an invented endpoint, route, field, version, result kind, status or
//!   failure code in either document fails loudly.
//!
//! Since S06/T05 the gate additionally enforces:
//! - the additive `batch_cache_observation:` record on the observation
//!   ledger (bounded batch + two-stage cache semantics) cross-validates
//!   against the LIVE da_parse vocabulary: the exact four-mode cache
//!   vocabulary with its truth table, the two stage identities with their
//!   payload schema versions and exact key-dependency sets, the projector
//!   schema fingerprint, the NonZero batch bounds (excluded from content
//!   identity), the schema-bound fail-closed cursor, the per-document
//!   terminal taxonomy (backend + cache-integrity codes), the run-summary
//!   serialized field set, the legacy read-only identities and the redb
//!   role `rebuildable_only` (D295);
//! - the registry-of-record `defaults.batch_cache` declaration agrees with
//!   the observation record and the live vocabulary (DATA↔DATA↔live);
//! - `configuration_keys_documented_not_owned` equals EXACTLY the live
//!   da-cli composition-root key constants — an invented key (documented
//!   but undefined) or a dropped live key fails by exact name;
//! - the `data/parse` tree stays exactly the sanctioned file set (no
//!   parallel ledger tree) and the ledger text stays free of raw parse
//!   payload material.
//!
//! Scope guards honored by construction: this binary contacts no network
//! service, parses no corpus document through the parser pipeline, touches no
//! graph store, and depends on no wall-clock state. Project hard locks are
//! asserted verbatim on both manifests, and every defect must FAIL LOUDLY —
//! the silent skip is exactly the regression class this slice removes.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use da_parse::{
    BatchCursor, CacheIntegrityCode, CacheMode, Consolidation, CoordinateSet,
    FULLTEXT_REQUEST_SCHEMA_VERSION, FailureCode, FulltextDialectV1, FulltextRequestVersioned,
    GrobidProfile, HEADER_REQUEST_SCHEMA_VERSION, HeaderDialectV1,
    PARSE_BATCH_CURSOR_SCHEMA_VERSION, PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION,
    PARSE_BATCH_SCHEMA_VERSION, PARSE_FAILURE_SCHEMA_VERSION,
    PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION, PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    PROJECT_ARTICLE_STAGE_ID, ParseBackendRequest, ParseBatchLimits, ParseCacheStage,
    ParseEndpoint, RAW_TEI_PAYLOAD_SCHEMA_VERSION, RAW_TEI_STAGE_ID,
    REFERENCES_REQUEST_SCHEMA_VERSION, ReferencesDialectV1, project_article_stage_key,
    raw_tei_stage_key,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const LEDGER_REL_PATH: &str = "data/parse/api_compatibility_ledger.yaml";
const REGRESSION_REL_PATH: &str = "data/parse/regression_cases.yaml";
/// Registry of record for R113/R117 compatibility states (loaded generically).
const REGISTRY_REL_PATH: &str = "data/parse/grobid_compatibility.yaml";
/// Comparison contract carrying the S08/T03 native date shadow DATA terms.
const COMPARISON_CONTRACT_REL: &str = "data/parse/comparison_contract.yaml";
const ACCEPTANCE_REL_PATH: &str = "data/parse/acceptance_manifest.yaml";

/// Canonical cache/configuration fingerprints (D204). The positive tests bind
/// these strings to the LIVE API values, so any future drift in
/// `GrobidProfile::configuration_fingerprint` must update both manifests or
/// this gate fails.
const RICH_FP: &str = "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula";
const STRUCTURAL_FP: &str = "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula";

/// Ids that MUST exist in the ledger for every later slice task to resolve.
const ANCHOR_ENTRY_IDS: [&str; 10] = [
    "parse.entrypoint.grobid_parser.parse_pdf",
    "parse.constructor.grobid_parser",
    "parse.probe.is_alive",
    "parse.endpoint.process_fulltext_document",
    "parse.endpoint.process_header_document",
    "parse.endpoint.process_references_document",
    "parse.stage.lift_tei",
    "parse.stage.project_article",
    "parse.profile.rich",
    "parse.profile.structural",
];

/// Allowed vocabulary for `required_evidence_fields`.
const EVIDENCE_VOCAB: [&str; 16] = [
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
    "native_date_shadow",
    "native_personal_name_shadow",
    "native_affiliation_shadow",
    "native_citation_shadow",
    "native_header_reference_composition_shadow",
    "native_fulltext_composition_shadow",
];

/// Every case must request at least this evidence core.
const MIN_EVIDENCE_FIELDS: [&str; 3] = ["source_sha256", "source_path", "profile_fingerprint"];

/// The sanctioned carriers of typed transport records (S05/T03: the S04
/// fulltext binding plus the two proxy operations — no parallel record).
const TYPED_TRANSPORT_ENTRY_IDS: [&str; 3] = [
    "parse.endpoint.process_fulltext_document",
    "parse.endpoint.process_header_document",
    "parse.endpoint.process_references_document",
];
const TYPED_TRANSPORT_ENTRY_ID: &str = TYPED_TRANSPORT_ENTRY_IDS[0];

/// Registry-of-record capability per sanctioned typed-transport endpoint
/// (cross-validated in `validate_registry_endpoint_dialects`).
const SANCTIONED_REGISTRY_ENDPOINTS: [(&str, ParseEndpoint); 3] = [
    ("rest.processFulltextDocument", ParseEndpoint::Fulltext),
    ("rest.processHeaderDocument", ParseEndpoint::Header),
    ("rest.processReferences", ParseEndpoint::References),
];

/// Status → typed-outcome taxonomy proven by the HTTP stub contract
/// (S04/T02). Exactly these five statuses own a typed branch.
const TYPED_TRANSPORT_STATUSES: [u16; 5] = [200, 204, 400, 413, 503];

/// Every sanctioned V1 dialect wire name the ledger request_fields must
/// carry (exact Java GROBID spelling; S04/T01–T02 typed surface).
const SANCTIONED_WIRE_FIELD_NAMES: [&str; 11] = [
    "input",
    "consolidateHeader",
    "consolidateCitations",
    "consolidateFunders",
    "includeRawCitations",
    "includeRawAffiliations",
    "teiCoordinates",
    "segmentSentences",
    "generateIDs",
    "start",
    "end",
];

/// Live closed multipart field allowlists of the two S05 proxy operations
/// (the exact Java GROBID projector surface; no foreign knob, no
/// teiCoordinates — the Java 0.9.0 handlers accept none).
const HEADER_WIRE_FIELDS: [&str; 3] = ["input", "consolidateHeader", "includeRawAffiliations"];
const REFERENCES_WIRE_FIELDS: [&str; 3] = ["input", "consolidateCitations", "includeRawCitations"];

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
    /// S08/T03: an observable pure shadow component (no runtime route).
    ShadowComponent,
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

/// Typed transport contract documented on a sanctioned endpoint entry
/// (`deny_unknown_fields`: an invented field fails loudly). Since S05/T03
/// the record carries the closed endpoint/result identities and the registry
/// rollback binding next to the request/failure schema.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct TypedTransportContract {
    endpoint: String,
    request_schema: String,
    request_schema_version: u8,
    result_kind: String,
    result_identity_namespace: String,
    failure_schema_version: u32,
    statuses: Vec<u16>,
    failure_codes: Vec<String>,
    retry_policy: String,
    retry_after_metadata: String,
    compatibility_facade: String,
    rollback_target: String,
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
    #[serde(default)]
    typed_transport: Option<TypedTransportContract>,
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
    batch_cache_observation: BatchCacheObservation,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptancePolicyLocks {
    import_eligible: bool,
    auto_promote: String,
    production_route: String,
    fallback_target: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceRules {
    allowed_classes: Vec<String>,
    allowed_row_states: Vec<String>,
    allowed_comparison_kinds: Vec<String>,
    missing_behavior: String,
    same_n: String,
    catalogued_bytes_are_executed: bool,
    omnidocbench_is_tei_oracle: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceRow {
    id: String,
    state: String,
    identity: Option<String>,
    dataset_id: Option<String>,
    source_url: Option<String>,
    expected_n: Option<u64>,
    gold_format: Option<String>,
    filter: Option<String>,
    identity_ref: Option<String>,
    pdf_path: Option<String>,
    pdf_sha256: Option<String>,
    tei_ref: Option<String>,
    tei_sha256: Option<String>,
    page_count: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceProxy {
    proxy_of: String,
    same_n: bool,
    rows: Vec<AcceptanceRow>,
    page_denominator: Option<u64>,
    document_denominator: Option<u64>,
    oracle_ref: Option<String>,
    source_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceClass {
    comparison_kind: String,
    purpose: String,
    authority: String,
    license: String,
    expected_structure: String,
    rows: Vec<AcceptanceRow>,
    local_proxy: AcceptanceProxy,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceClasses {
    publisher_acceptance: AcceptanceClass,
    layout_omnidocbench: AcceptanceClass,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceManifest {
    schema_version: String,
    adr_ref: String,
    manifest_id: String,
    requirement_ids: Vec<String>,
    policy_locks: AcceptancePolicyLocks,
    comparison_rules: AcceptanceRules,
    classes: AcceptanceClasses,
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
                | EntryKind::ShadowComponent
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
            EntryKind::Entrypoint
                | EntryKind::LiftStage
                | EntryKind::ProjectStage
                | EntryKind::ShadowComponent
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
                | EntryKind::ShadowComponent
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

    // S04/S05: every typed transport record must exist on exactly its
    // sanctioned endpoint binding and cross-validate against the live Rust
    // vocabulary of that endpoint.
    let rollback_target = registry_rollback_target();
    for entry_id in TYPED_TRANSPORT_ENTRY_IDS {
        if let Some(e) = l.entries.iter().find(|e| e.id == entry_id)
            && let Some(tt) = &e.typed_transport
        {
            errs.extend(validate_typed_transport(e, tt, &rollback_target));
        }
    }
    errs.extend(validate_typed_transport_placement(l));
    errs.extend(validate_registry_endpoint_dialects(&load_registry_value()));
    errs.extend(validate_registry_config_keys(&load_registry_value()));
    errs.extend(validate_data_parse_tree());
    errs.extend(validate_batch_cache_registry_consistency(
        &l.batch_cache_observation,
        &load_registry_value(),
    ));
    errs.extend(validate_batch_cache_observation(
        &l.batch_cache_observation,
        &ledger_id_set(l),
    ));

    for anchor in ANCHOR_ENTRY_IDS {
        if !seen.contains(anchor) {
            errs.push(format!("[missing-anchor-entry] ledger lost `{anchor}`"));
        }
    }
    errs
}

/// Live closed failure-code set derived from the public `FailureCode`
/// vocabulary (serde snake_case wire form == `FailureCode::as_str`).
fn live_failure_code_names() -> BTreeSet<String> {
    [
        FailureCode::Busy,
        FailureCode::Timeout,
        FailureCode::Transport,
        FailureCode::NoContent,
        FailureCode::LimitExceeded,
        FailureCode::InvalidRequest,
        FailureCode::ConversionFailed,
        FailureCode::InvalidResponse,
    ]
    .into_iter()
    .map(|code| code.as_str().to_string())
    .collect()
}

/// Live closed endpoint vocabulary parsed from the wire (snake_case) form.
fn parse_endpoint_name(name: &str) -> Option<ParseEndpoint> {
    [
        ParseEndpoint::Fulltext,
        ParseEndpoint::Header,
        ParseEndpoint::References,
    ]
    .into_iter()
    .find(|endpoint| endpoint.as_str() == name)
}

/// Live request schema version + sanctioned multipart field allowlist of a
/// closed endpoint (the exact Java GROBID projector surface).
fn live_endpoint_facts(endpoint: ParseEndpoint) -> (u8, &'static [&'static str]) {
    match endpoint {
        ParseEndpoint::Fulltext => (
            FULLTEXT_REQUEST_SCHEMA_VERSION,
            &SANCTIONED_WIRE_FIELD_NAMES,
        ),
        ParseEndpoint::Header => (HEADER_REQUEST_SCHEMA_VERSION, &HEADER_WIRE_FIELDS),
        ParseEndpoint::References => (REFERENCES_REQUEST_SCHEMA_VERSION, &REFERENCES_WIRE_FIELDS),
    }
}

/// The LIVE canonical identity of the endpoint dialect starts with the
/// documented versioned namespace — a live-code proof, not just a YAML
/// string comparison (extended fulltext identities and every header/
/// references dialect start with `endpoint=v<version>;`).
fn live_result_identity_starts_with(endpoint: ParseEndpoint, namespace: &str) -> bool {
    match endpoint {
        ParseEndpoint::Fulltext => FulltextDialectV1::legacy_rich()
            .with_raw_citations(true)
            .canonical_identity()
            .is_some_and(|identity| identity.starts_with(namespace)),
        ParseEndpoint::Header => HeaderDialectV1::legacy_rich()
            .canonical_identity()
            .starts_with(namespace),
        ParseEndpoint::References => ReferencesDialectV1::legacy_rich()
            .canonical_identity()
            .starts_with(namespace),
    }
}

/// Wire field names documented in an endpoint entry's request_fields
/// (`name — description` ledger list style).
fn documented_field_names(entry: &LedgerEntry) -> Vec<String> {
    entry
        .request_fields
        .iter()
        .map(|field| field.split('—').next().unwrap_or("").trim().to_string())
        .collect()
}

/// The registry-of-record rollback target every typed transport must bind.
fn registry_rollback_target() -> String {
    load_registry_value()
        .get("defaults")
        .and_then(|d| d.get("rollback_target"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// A typed transport record must cross-validate against the LIVE Rust
/// vocabulary of its endpoint: closed endpoint/result-kind names, the
/// per-endpoint request schema version, a result identity namespace that
/// prefixes the live canonical identity, the shared failure schema, the
/// exact status taxonomy, honest no-retry 503 with bounded Retry-After, the
/// registry rollback target, the exact closed multipart field allowlist and
/// the exact static route.
fn validate_typed_transport(
    entry: &LedgerEntry,
    tt: &TypedTransportContract,
    registry_rollback: &str,
) -> Vec<String> {
    let mut errs = Vec::new();
    let ctx = entry.id.as_str();

    // Closed endpoint vocabulary: an invented proxy operation fails loudly.
    let Some(endpoint) = parse_endpoint_name(&tt.endpoint) else {
        errs.push(format!(
            "[unknown-typed-transport-endpoint] {ctx}: `{}` is not a live ParseEndpoint name",
            tt.endpoint
        ));
        return errs;
    };
    let (live_version, allowed_fields) = live_endpoint_facts(endpoint);

    if tt.request_schema_version != live_version {
        errs.push(format!(
            "[typed-transport-version] {ctx}: request_schema_version {} != live {endpoint} schema version {live_version}",
            tt.request_schema_version
        ));
    }
    if tt.request_schema != format!("{}-v{}", tt.endpoint, tt.request_schema_version) {
        errs.push(format!(
            "[typed-transport-version] {ctx}: request_schema `{}` must be spelled `{}-v{}`",
            tt.request_schema, tt.endpoint, tt.request_schema_version
        ));
    }

    // The closed result kind is the static endpoint → result-kind mapping.
    if tt.result_kind != endpoint.result_kind().as_str() {
        errs.push(format!(
            "[unknown-result-kind] {ctx}: `{}` is not the live {endpoint} result kind `{}`",
            tt.result_kind,
            endpoint.result_kind().as_str()
        ));
    }
    // The result identity namespace must be the versioned namespace AND the
    // live canonical identity must really start with it.
    let expected_namespace = format!("{}=v{}", tt.endpoint, tt.request_schema_version);
    if tt.result_identity_namespace != expected_namespace {
        errs.push(format!(
            "[typed-transport-identity-namespace] {ctx}: result_identity_namespace `{}` must be `{expected_namespace}`",
            tt.result_identity_namespace
        ));
    } else if !live_result_identity_starts_with(endpoint, &expected_namespace) {
        errs.push(format!(
            "[typed-transport-identity-namespace] {ctx}: live dialect identity no longer starts with `{expected_namespace}`"
        ));
    }

    if tt.failure_schema_version != PARSE_FAILURE_SCHEMA_VERSION {
        errs.push(format!(
            "[typed-transport-version] {ctx}: failure_schema_version {} != live PARSE_FAILURE_SCHEMA_VERSION {PARSE_FAILURE_SCHEMA_VERSION}",
            tt.failure_schema_version
        ));
    }

    let mut statuses = tt.statuses.clone();
    statuses.sort_unstable();
    statuses.dedup();
    if statuses != TYPED_TRANSPORT_STATUSES.to_vec() {
        errs.push(format!(
            "[typed-transport-status-taxonomy] {ctx}: statuses {:?} must be exactly {:?} (200 success, 204 no_content, 400 invalid_request, 413 limit, 503 busy)",
            tt.statuses, TYPED_TRANSPORT_STATUSES
        ));
    }

    let live = live_failure_code_names();
    let documented: BTreeSet<String> = tt.failure_codes.iter().cloned().collect();
    for code in &documented {
        if !live.contains(code) {
            errs.push(format!(
                "[unknown-failure-code] {ctx}: `{code}` is not a live FailureCode serde name"
            ));
        }
    }
    if documented != live {
        errs.push(format!(
            "[incomplete-failure-vocabulary] {ctx}: documented codes must equal the closed live set {live:?}, got {documented:?}"
        ));
    }

    if tt.retry_policy != "honest_503_no_hidden_retry" {
        errs.push(format!(
            "[typed-transport-retry-drift] {ctx}: retry_policy `{}` must stay honest_503_no_hidden_retry",
            tt.retry_policy
        ));
    }
    if tt.retry_after_metadata != "bounded_parse" {
        errs.push(format!(
            "[typed-transport-retry-drift] {ctx}: retry_after_metadata `{}` must stay bounded_parse",
            tt.retry_after_metadata
        ));
    }
    if !tt.compatibility_facade.contains("Unavailable")
        || !tt.compatibility_facade.contains("ParseFailed")
    {
        errs.push(format!(
            "[typed-transport-facade-drift] {ctx}: compatibility_facade must document both legacy ParserError variants (Unavailable + ParseFailed)"
        ));
    }
    if tt.rollback_target != registry_rollback {
        errs.push(format!(
            "[typed-transport-rollback-drift] {ctx}: rollback_target `{}` != registry of record rollback target `{registry_rollback}`",
            tt.rollback_target
        ));
    }

    // The documented multipart projection is the EXACT closed allowlist of
    // this endpoint — a foreign knob (e.g. teiCoordinates on the header
    // proxy) is visible drift.
    let allowed: BTreeSet<&str> = allowed_fields.iter().copied().collect();
    let documented_names = documented_field_names(entry);
    let documented_fields: BTreeSet<&str> = documented_names.iter().map(String::as_str).collect();
    for name in &documented_fields {
        if !allowed.contains(name) {
            errs.push(format!(
                "[foreign-endpoint-field] {ctx}: `{name}` is not a sanctioned {endpoint} field (allowed: {allowed:?})"
            ));
        }
    }
    if documented_fields != allowed {
        errs.push(format!(
            "[incomplete-endpoint-field-allowlist] {ctx}: documented fields {documented_fields:?} must equal the closed {endpoint} allowlist {allowed:?}"
        ));
    }

    // The documented route is the live static path — never a free-form path.
    if entry.http_endpoint.as_deref() != Some(endpoint.static_path()) {
        errs.push(format!(
            "[typed-transport-route-drift] {ctx}: http_endpoint {:?} != live static path `{}`",
            entry.http_endpoint,
            endpoint.static_path()
        ));
    }

    errs
}

/// `typed_transport` is sanctioned on exactly the three endpoint bindings.
fn validate_typed_transport_placement(l: &LedgerFile) -> Vec<String> {
    let mut errs = Vec::new();
    for e in &l.entries {
        if TYPED_TRANSPORT_ENTRY_IDS.contains(&e.id.as_str()) {
            if e.typed_transport.is_none() {
                errs.push(format!(
                    "[missing-typed-transport] {} must document its typed transport",
                    e.id
                ));
            }
        } else if e.typed_transport.is_some() {
            errs.push(format!(
                "[misplaced-typed-transport] {}: typed_transport is sanctioned only on {TYPED_TRANSPORT_ENTRY_IDS:?}",
                e.id
            ));
        }
    }
    errs
}

fn ledger_id_set(l: &LedgerFile) -> HashSet<String> {
    l.entries.iter().map(|e| e.id.clone()).collect()
}

/// The registry-of-record official dialects of the three sanctioned
/// endpoints must equal the LIVE closed allowlists (S05/T03 drift
/// correction: header/references carry NO teiCoordinates and must never
/// re-gain it), cite the live static routes and stay in `proxy` state.
fn validate_registry_endpoint_dialects(registry: &serde_yaml::Value) -> Vec<String> {
    const DRIFT: &str = "[registry-endpoint-dialect-drift]";
    const STATE_DRIFT: &str = "[registry-endpoint-state-drift]";
    let mut errs = Vec::new();
    let Some(caps) = registry.get("capabilities").and_then(|c| c.as_sequence()) else {
        errs.push(format!("{DRIFT} registry lost its capabilities sequence"));
        return errs;
    };
    for (cap_id, endpoint) in SANCTIONED_REGISTRY_ENDPOINTS {
        let Some(cap) = caps
            .iter()
            .find(|c| c.get("id").and_then(|i| i.as_str()) == Some(cap_id))
        else {
            errs.push(format!("{DRIFT} registry lost capability `{cap_id}`"));
            continue;
        };
        if cap.get("state").and_then(|s| s.as_str()) != Some("proxy") {
            errs.push(format!(
                "{STATE_DRIFT} {cap_id} must stay in `proxy` state (sanctioned live operation)"
            ));
        }
        let official = cap.get("official");
        let identity = official
            .and_then(|o| o.get("identity"))
            .and_then(|i| i.as_str())
            .unwrap_or_default();
        if !identity.contains(endpoint.static_path()) {
            errs.push(format!(
                "{DRIFT} {cap_id} official identity `{identity}` no longer cites the live static route `{}`",
                endpoint.static_path()
            ));
        }
        let (_, allowed_fields) = live_endpoint_facts(endpoint);
        let official_names: Option<BTreeSet<String>> = official
            .and_then(|o| o.get("request_fields"))
            .and_then(|f| f.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|f| f.get("name").and_then(|n| n.as_str()).map(str::to_string))
                    .collect()
            });
        let Some(official_names) = official_names else {
            errs.push(format!("{DRIFT} {cap_id} lost its official request_fields"));
            continue;
        };
        let allowed: BTreeSet<String> = allowed_fields.iter().map(|s| s.to_string()).collect();
        for name in &official_names {
            if !allowed.contains(name) {
                errs.push(format!(
                    "[registry-endpoint-field-drift] {cap_id}: official field `{name}` is not a sanctioned {} field {allowed:?} — pre-S05 drift must never return",
                    endpoint.as_str()
                ));
            }
        }
        if official_names != allowed {
            errs.push(format!(
                "[registry-endpoint-allowlist-incomplete] {cap_id}: official fields {official_names:?} must equal the closed {} allowlist {allowed:?}",
                endpoint.as_str()
            ));
        }
    }
    errs
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
const REGISTRY_BINDING_KINDS: [EntryKind; 4] = [
    EntryKind::LiftStage,
    EntryKind::ProjectStage,
    EntryKind::Endpoint,
    EntryKind::ShadowComponent,
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

// ───── S06/T05: batch/cache observation ↔ live Rust cross-checks ─────

/// Additive batch/cache observation of the observation ledger
/// (`deny_unknown_fields`: an invented field fails loudly at load).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchCacheObservation {
    carrier_entry_ids: Vec<String>,
    cache_modes: Vec<CacheModeObservation>,
    #[allow(dead_code)] // prose carrier of the mode semantics
    cache_modes_note: String,
    cache_stages: Vec<CacheStageObservation>,
    batch_limits: BatchLimitsObservation,
    cursor: CursorObservation,
    terminal_per_document_taxonomy: TerminalTaxonomyObservation,
    run_summary: RunSummaryObservation,
    legacy_read_only: LegacyReadOnlyObservation,
    redb: RedbObservation,
    raw_content_free: bool,
}

/// One row of the four-mode cache vocabulary plus its truth table.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CacheModeObservation {
    mode: String,
    reads_stages: bool,
    publishes_results: bool,
    bypasses_hits: bool,
}

/// One row of the two-stage cache vocabulary with its live identities.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheStageObservation {
    stage: String,
    stage_id: String,
    payload_schema_version: u32,
    /// Only the projector stage may declare this (raw stays fingerprint-free).
    projector_schema_fingerprint: Option<String>,
    key_dependencies: Vec<String>,
    paper_id_in_key: bool,
    rotation: String,
}

/// Bounded batch limits semantics.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchLimitsObservation {
    bounded_fields: Vec<String>,
    nonzero_bounds: bool,
    content_identity_excluded: bool,
    admission_scope: String,
}

/// Schema-bound resume cursor semantics.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CursorObservation {
    input_binding: String,
    schema_version: u32,
    fail_closed: bool,
}

/// Per-document terminal outcome taxonomy.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalTaxonomyObservation {
    outcome_schema_version: u32,
    one_outcome_per_admitted_ordinal: bool,
    backend_failure_codes: Vec<String>,
    cache_failure_stages: Vec<String>,
    cache_integrity_codes: Vec<String>,
}

/// Run summary schema + counter field set.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunSummaryObservation {
    schema_version: u32,
    counters: Vec<String>,
}

/// Legacy read-only parse identities (read-only reuse, never promotion).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyReadOnlyObservation {
    identities: Vec<String>,
    projection_schema: String,
    hit_creates_canonical_record: bool,
}

/// redb accelerator role (D295: rebuildable only, never authority).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RedbObservation {
    role: String,
    feature_gated: bool,
    owner_crates: Vec<String>,
    #[allow(dead_code)] // prose statement of what deletion preserves
    deletion_preserves: String,
}

/// The live ParseCacheStage for a serde wire name.
fn parse_live_cache_stage(name: &str) -> Option<ParseCacheStage> {
    match name {
        "raw_tei" => Some(ParseCacheStage::RawTei),
        "project_article" => Some(ParseCacheStage::ProjectArticle),
        _ => None,
    }
}

/// The live cache mode for a wire name (closed four-mode vocabulary).
fn parse_live_cache_mode(name: &str) -> Option<CacheMode> {
    [
        CacheMode::Off,
        CacheMode::ReadOnly,
        CacheMode::WriteThrough,
        CacheMode::Refresh,
    ]
    .into_iter()
    .find(|mode| mode.as_str() == name)
}

/// The live closed cache-integrity vocabulary (serde snake_case wire form).
fn live_integrity_code_names() -> BTreeSet<String> {
    [
        CacheIntegrityCode::ChecksumMismatch,
        CacheIntegrityCode::ManifestCorrupt,
        CacheIntegrityCode::IdentityMismatch,
        CacheIntegrityCode::PayloadUnreadable,
        CacheIntegrityCode::StageWriteFailed,
    ]
    .into_iter()
    .map(|code| code.as_str().to_string())
    .collect()
}

/// Live ParseBatchRunSummary serialized field names, extracted from the
/// source Serialize impl (mechanical DATA↔live proof, no instance needed).
fn live_run_summary_fields() -> BTreeSet<String> {
    let source = std::fs::read_to_string(repo_root().join("crates/da-parse/src/batch.rs"))
        .unwrap_or_else(|e| panic!("batch.rs unreadable: {e}"));
    let start = source
        .find("impl Serialize for ParseBatchRunSummary")
        .expect("run summary serializer present in batch.rs");
    let end = source[start..]
        .find("impl<'de> Deserialize<'de> for ParseBatchRunSummary")
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    let mut names = BTreeSet::new();
    for rest in source[start..end].split("serialize_field(\"").skip(1) {
        let name = rest.split('"').next().unwrap_or_default();
        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }
    // Positive controls: the identity/header fields the parser must have
    // seen — if the serializer shape changes, this gate fails loudly.
    for header in [
        "schema_version",
        "mode",
        "limits",
        "input_fingerprint",
        "resumed_from_ordinal",
        "cursor",
    ] {
        assert!(
            names.contains(header),
            "positive control failed: run summary header field `{header}` not parsed"
        );
    }
    assert!(
        names.len() >= 20,
        "positive control failed: only {} run summary fields parsed",
        names.len()
    );
    names
}

/// The counter fields of the run summary = serialized fields minus the
/// identity/header fields the DATA declares separately.
fn live_run_summary_counter_names() -> BTreeSet<String> {
    const HEADER: [&str; 6] = [
        "schema_version",
        "mode",
        "limits",
        "input_fingerprint",
        "resumed_from_ordinal",
        "cursor",
    ];
    live_run_summary_fields()
        .into_iter()
        .filter(|name| !HEADER.contains(&name.as_str()))
        .collect()
}

/// Live composition-root config key names, extracted from the `*_KEY`
/// string constants of crates/da-cli/src/parse_config.rs.
fn live_config_keys() -> BTreeSet<String> {
    let source = std::fs::read_to_string(repo_root().join("crates/da-cli/src/parse_config.rs"))
        .unwrap_or_else(|e| panic!("parse_config.rs unreadable: {e}"));
    let mut keys = BTreeSet::new();
    for rest in source.split("_KEY: &str = \"").skip(1) {
        let value = rest.split('"').next().unwrap_or_default();
        if !value.is_empty() {
            keys.insert(value.to_string());
        }
    }
    assert!(
        keys.len() >= 8,
        "positive control failed: composition-root key constants not parsed: {keys:?}"
    );
    keys
}

/// The registry `configuration_keys_documented_not_owned` list must equal
/// EXACTLY the live da-cli composition-root key set — an invented
/// (documented but undefined) key or a dropped live key fails by name.
fn validate_registry_config_keys(registry: &serde_yaml::Value) -> Vec<String> {
    const DRIFT: &str = "[registry-config-key-drift]";
    let mut errs = Vec::new();
    let documented: Option<Vec<String>> = registry
        .get("defaults")
        .and_then(|d| d.get("configuration_keys_documented_not_owned"))
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        });
    let Some(documented) = documented else {
        errs.push(format!(
            "{DRIFT} registry lost `configuration_keys_documented_not_owned`"
        ));
        return errs;
    };
    let live = live_config_keys();
    let declared: BTreeSet<String> = documented.into_iter().collect();
    for key in &declared {
        if !live.contains(key) {
            errs.push(format!(
                "[unknown-config-key] `{key}` is documented-not-owned but no live da-cli composition-root constant defines it"
            ));
        }
    }
    if declared != live {
        for key in live.difference(&declared) {
            errs.push(format!(
                "[incomplete-config-key-set] live composition-root key `{key}` is missing from the registry list"
            ));
        }
    }
    errs
}

/// The observation ledger tree stays exactly the sanctioned data/parse file
/// set — no parallel ledger tree may appear (R113/R117).
fn data_parse_tree_errs(entries: Vec<(String, bool)>) -> Vec<String> {
    const SANCTIONED: [&str; 10] = [
        "SCHEMA.yaml",
        "api_compatibility_ledger.yaml",
        "comparison_contract.yaml",
        "grobid_compatibility.yaml",
        "regression_cases.yaml",
        "layout_bakeoff.yaml",
        "document_segmentation.yaml",
        "native_visual_heads.yaml",
        "native_fulltext_composition.yaml",
        "acceptance_manifest.yaml",
    ];
    let mut errs = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (name, is_dir) in entries {
        if is_dir {
            errs.push(format!(
                "[parallel-ledger-tree] unexpected directory `data/parse/{name}` in the parse-plane ledger tree"
            ));
            continue;
        }
        if !SANCTIONED.contains(&name.as_str()) {
            errs.push(format!(
                "[parallel-ledger-tree] unexpected file `data/parse/{name}` — the ledger tree is exactly the sanctioned set"
            ));
        }
        seen.insert(name);
    }
    for name in SANCTIONED {
        if !seen.contains(name) {
            errs.push(format!(
                "[missing-sanctioned-data-file] `data/parse/{name}` vanished"
            ));
        }
    }
    errs
}

fn validate_data_parse_tree() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(repo_root().join("data/parse")) else {
        return vec!["[unreadable-data-parse-tree] data/parse disappeared".to_string()];
    };
    let listing = entries
        .flatten()
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_string(),
                entry.path().is_dir(),
            )
        })
        .collect();
    data_parse_tree_errs(listing)
}

/// The ledger is an observation document: raw parse payload material must
/// never enter it (no TEI/PDF bytes, no payload-envelope field names).
fn raw_content_scan(text: &str) -> Vec<String> {
    const RAW_MARKERS: [&str; 3] = ["<TEI", "%PDF-", "tei_xml_hex"];
    RAW_MARKERS
        .into_iter()
        .filter(|marker| text.contains(marker))
        .map(|marker| {
            format!("[raw-content-in-ledger] ledger text contains raw material marker `{marker}`")
        })
        .collect()
}

/// The mutual DATA↔DATA check: the registry-of-record `defaults.batch_cache`
/// declaration must agree with the observation record (and both are already
/// checked against the live vocabulary).
fn validate_batch_cache_registry_consistency(
    obs: &BatchCacheObservation,
    registry: &serde_yaml::Value,
) -> Vec<String> {
    const DRIFT: &str = "[registry-batch-cache-drift]";
    let mut errs = Vec::new();
    let Some(section) = registry.get("defaults").and_then(|d| d.get("batch_cache")) else {
        errs.push(format!("{DRIFT} registry lost `defaults.batch_cache`"));
        return errs;
    };
    let str_at = |node: &serde_yaml::Value, key: &str| -> String {
        node.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let seq_at = |node: &serde_yaml::Value, key: &str| -> Vec<String> {
        node.get(key)
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let nested = |node: &serde_yaml::Value, key: &str| -> serde_yaml::Value {
        node.get(key).cloned().unwrap_or(serde_yaml::Value::Null)
    };

    // The four-mode vocabulary agrees across both documents.
    let registry_modes: BTreeSet<String> = seq_at(section, "cache_modes").into_iter().collect();
    let observation_modes: BTreeSet<String> =
        obs.cache_modes.iter().map(|m| m.mode.clone()).collect();
    if registry_modes != observation_modes {
        errs.push(format!(
            "{DRIFT} registry cache_modes {registry_modes:?} != observation modes {observation_modes:?}"
        ));
    }

    // Stage identities agree.
    let stages = nested(section, "cache_stages");
    let raw = obs.cache_stages.iter().find(|s| s.stage == "raw_tei");
    let projector = obs
        .cache_stages
        .iter()
        .find(|s| s.stage == "project_article");
    if let (Some(raw), Some(projector)) = (raw, projector) {
        let projector_fp = projector
            .projector_schema_fingerprint
            .clone()
            .unwrap_or_default();
        for (key, expected) in [
            ("raw_tei_stage_id", raw.stage_id.clone()),
            ("project_article_stage_id", projector.stage_id.clone()),
            ("projector_schema_fingerprint", projector_fp),
        ] {
            let actual = str_at(&stages, key);
            if actual != expected {
                errs.push(format!(
                    "{DRIFT} registry `{key}` `{actual}` != observation `{expected}`"
                ));
            }
        }
        for (key, expected) in [
            ("raw_payload_schema_version", raw.payload_schema_version),
            (
                "project_article_payload_schema_version",
                projector.payload_schema_version,
            ),
        ] {
            let actual = stages.get(key).and_then(serde_yaml::Value::as_u64);
            if actual != Some(u64::from(expected)) {
                errs.push(format!(
                    "{DRIFT} registry `{key}` {actual:?} != observation {expected}"
                ));
            }
        }
    }

    // Batch limits, cursor binding and redb role agree.
    let limits_fields: BTreeSet<String> = seq_at(section, "batch_limits_bounded_fields")
        .into_iter()
        .collect();
    let observation_limits: BTreeSet<String> =
        obs.batch_limits.bounded_fields.iter().cloned().collect();
    if limits_fields != observation_limits {
        errs.push(format!(
            "{DRIFT} registry batch_limits_bounded_fields {limits_fields:?} != observation bounds {observation_limits:?}"
        ));
    }
    if str_at(section, "cursor_input_binding") != obs.cursor.input_binding {
        errs.push(format!(
            "{DRIFT} registry cursor_input_binding `{}` != observation `{}`",
            str_at(section, "cursor_input_binding"),
            obs.cursor.input_binding
        ));
    }
    let redb_owners: BTreeSet<String> = seq_at(section, "redb_owner_crates").into_iter().collect();
    let observation_owners: BTreeSet<String> = obs.redb.owner_crates.iter().cloned().collect();
    if redb_owners != observation_owners {
        errs.push(format!(
            "{DRIFT} registry redb_owner_crates {redb_owners:?} != observation owners {observation_owners:?}"
        ));
    }
    if str_at(section, "redb_role") != obs.redb.role {
        errs.push(format!(
            "{DRIFT} registry redb_role `{}` != observation `{}`",
            str_at(section, "redb_role"),
            obs.redb.role
        ));
    }

    // Legacy read-only identities agree.
    let legacy: BTreeSet<String> = seq_at(section, "legacy_read_only_identities")
        .into_iter()
        .collect();
    let observation_legacy: BTreeSet<String> =
        obs.legacy_read_only.identities.iter().cloned().collect();
    if legacy != observation_legacy {
        errs.push(format!(
            "{DRIFT} registry legacy_read_only_identities {legacy:?} != observation {observation_legacy:?}"
        ));
    }
    errs
}

/// Full cross-validation of the batch/cache observation record against the
/// LIVE da_parse vocabulary. Drift is localized to the exact field via the
/// tagged errors below.
fn validate_batch_cache_observation(
    obs: &BatchCacheObservation,
    ledger_ids: &HashSet<String>,
) -> Vec<String> {
    let mut errs = Vec::new();

    // Carriers: the observation rides EXISTING sanctioned entries only —
    // no new capability identities (R105/R113).
    for id in &obs.carrier_entry_ids {
        if !ledger_ids.contains(id) {
            errs.push(format!(
                "[unresolved-batch-cache-carrier] `{id}` is not a ledger entry id"
            ));
        }
    }
    for required in [
        "parse.endpoint.process_fulltext_document",
        "parse.stage.project_article",
        "parse.profile.structural",
    ] {
        if !obs
            .carrier_entry_ids
            .iter()
            .any(|carrier| carrier == required)
        {
            errs.push(format!(
                "[incomplete-batch-cache-carriers] missing required carrier `{required}`"
            ));
        }
    }

    // Cache modes: exactly the live four-mode vocabulary, truth table exact.
    let mut seen_modes: HashSet<&str> = HashSet::new();
    for row in &obs.cache_modes {
        let Some(live_mode) = parse_live_cache_mode(&row.mode) else {
            errs.push(format!(
                "[unknown-cache-mode] `{}` is not a live CacheMode wire form",
                row.mode
            ));
            continue;
        };
        if !seen_modes.insert(row.mode.as_str()) {
            errs.push(format!(
                "[duplicate-cache-mode] `{}` declared twice",
                row.mode
            ));
        }
        let live_row = (
            live_mode.reads_stages(),
            live_mode.publishes_results(),
            live_mode.bypasses_hits(),
        );
        let declared_row = (row.reads_stages, row.publishes_results, row.bypasses_hits);
        if declared_row != live_row {
            errs.push(format!(
                "[cache-mode-truth-table] `{}` declared {:?} but the live truth table is {live_row:?}",
                row.mode, declared_row
            ));
        }
    }
    let live_mode_names: BTreeSet<String> = [
        CacheMode::Off,
        CacheMode::ReadOnly,
        CacheMode::WriteThrough,
        CacheMode::Refresh,
    ]
    .into_iter()
    .map(|mode| mode.as_str().to_string())
    .collect();
    let declared_modes: BTreeSet<String> = obs.cache_modes.iter().map(|m| m.mode.clone()).collect();
    if declared_modes != live_mode_names {
        errs.push(format!(
            "[incomplete-cache-mode-vocabulary] declared {declared_modes:?} must equal the closed live set {live_mode_names:?}"
        ));
    }

    // Cache stages: closed two-stage vocabulary with live identities.
    for stage in &obs.cache_stages {
        let Some(live) = parse_live_cache_stage(&stage.stage) else {
            errs.push(format!(
                "[unknown-cache-stage] `{}` is not a live ParseCacheStage serde name",
                stage.stage
            ));
            continue;
        };
        if stage.stage_id != live.stage_id() {
            errs.push(format!(
                "[unknown-cache-stage-id] `{}` stage_id `{}` != live `{}`",
                stage.stage,
                stage.stage_id,
                live.stage_id()
            ));
        }
        let live_payload_version = match live {
            ParseCacheStage::RawTei => RAW_TEI_PAYLOAD_SCHEMA_VERSION,
            ParseCacheStage::ProjectArticle => PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION,
        };
        if stage.payload_schema_version != live_payload_version {
            errs.push(format!(
                "[cache-stage-schema-drift] `{}` payload_schema_version {} != live {live_payload_version}",
                stage.stage, stage.payload_schema_version
            ));
        }
        match live {
            ParseCacheStage::RawTei => {
                if stage.projector_schema_fingerprint.is_some() {
                    errs.push(
                        "[cache-stage-schema-drift] raw_tei must not carry projector_schema_fingerprint"
                            .into(),
                    );
                }
            }
            ParseCacheStage::ProjectArticle => {
                if stage.projector_schema_fingerprint.as_deref()
                    != Some(PROJECT_ARTICLE_SCHEMA_FINGERPRINT)
                {
                    errs.push(format!(
                        "[cache-stage-schema-drift] projector_schema_fingerprint {:?} != live `{PROJECT_ARTICLE_SCHEMA_FINGERPRINT}`",
                        stage.projector_schema_fingerprint
                    ));
                }
            }
        }
        let allowed: &[&str] = match live {
            ParseCacheStage::RawTei => &["endpoint", "dialect_wire", "source_bytes"],
            ParseCacheStage::ProjectArticle => &[
                "paper_id",
                "pdf_sha256",
                "raw_payload_sha256",
                "projector_schema_fingerprint",
            ],
        };
        let allowed_set: BTreeSet<&str> = allowed.iter().copied().collect();
        let declared_deps: BTreeSet<&str> =
            stage.key_dependencies.iter().map(String::as_str).collect();
        for dep in &declared_deps {
            if !allowed_set.contains(dep) {
                errs.push(format!(
                    "[unknown-stage-key-dependency] `{}`: `{dep}` is not a sanctioned key dependency {allowed_set:?}",
                    stage.stage
                ));
            }
        }
        if declared_deps != allowed_set {
            errs.push(format!(
                "[incomplete-stage-key-dependencies] `{}`: dependencies {declared_deps:?} must equal {allowed_set:?}",
                stage.stage
            ));
        }
        let expected_paper_id_in_key = live == ParseCacheStage::ProjectArticle;
        if stage.paper_id_in_key != expected_paper_id_in_key {
            errs.push(format!(
                "[cache-stage-key-binding] `{}` paper_id_in_key {} must be {expected_paper_id_in_key}",
                stage.stage, stage.paper_id_in_key
            ));
        }
        if stage.rotation != "independent" {
            errs.push(format!(
                "[cache-stage-rotation-drift] `{}` rotation `{}` must stay `independent`",
                stage.stage, stage.rotation
            ));
        }
    }
    let declared_stages: BTreeSet<&str> =
        obs.cache_stages.iter().map(|s| s.stage.as_str()).collect();
    let expected_stages: BTreeSet<&str> = ["raw_tei", "project_article"].into_iter().collect();
    if declared_stages != expected_stages {
        errs.push(format!(
            "[incomplete-cache-stage-vocabulary] stages {declared_stages:?} must equal {expected_stages:?}"
        ));
    }

    // Batch limits semantics.
    {
        let bl = &obs.batch_limits;
        let live_fields: BTreeSet<&str> = [
            "workers",
            "queue_capacity",
            "max_in_flight",
            "max_documents",
        ]
        .into_iter()
        .collect();
        let declared_fields: BTreeSet<&str> =
            bl.bounded_fields.iter().map(String::as_str).collect();
        for field in &declared_fields {
            if !live_fields.contains(field) {
                errs.push(format!(
                    "[unknown-batch-limit-field] `{field}` is not a live ParseBatchLimits bound"
                ));
            }
        }
        if declared_fields != live_fields {
            errs.push(format!(
                "[incomplete-batch-limit-fields] bounds {declared_fields:?} must equal {live_fields:?}"
            ));
        }
        if !bl.nonzero_bounds || !bl.content_identity_excluded {
            errs.push(
                "[batch-limit-drift] nonzero_bounds and content_identity_excluded must both stay true"
                    .into(),
            );
        }
        if bl.admission_scope != "batch_wide_error_only_at_admission" {
            errs.push(format!(
                "[batch-admission-scope-drift] admission_scope `{}` must stay batch_wide_error_only_at_admission",
                bl.admission_scope
            ));
        }
    }

    // Cursor binding.
    {
        let cursor = &obs.cursor;
        if cursor.input_binding != "ordered_input_fingerprint" {
            errs.push(format!(
                "[cursor-binding-drift] input_binding `{}` must stay ordered_input_fingerprint",
                cursor.input_binding
            ));
        }
        if cursor.schema_version != PARSE_BATCH_CURSOR_SCHEMA_VERSION {
            errs.push(format!(
                "[cursor-schema-drift] schema_version {} != live {PARSE_BATCH_CURSOR_SCHEMA_VERSION}",
                cursor.schema_version
            ));
        }
        if !cursor.fail_closed {
            errs.push("[cursor-binding-drift] fail_closed must stay true".into());
        }
    }

    // Terminal per-document taxonomy.
    {
        let taxonomy = &obs.terminal_per_document_taxonomy;
        if taxonomy.outcome_schema_version != PARSE_BATCH_SCHEMA_VERSION {
            errs.push(format!(
                "[terminal-schema-drift] outcome_schema_version {} != live {PARSE_BATCH_SCHEMA_VERSION}",
                taxonomy.outcome_schema_version
            ));
        }
        if !taxonomy.one_outcome_per_admitted_ordinal {
            errs.push(
                "[terminal-taxonomy-drift] one_outcome_per_admitted_ordinal must stay true".into(),
            );
        }
        let live_backend = live_failure_code_names();
        let declared_backend: BTreeSet<String> =
            taxonomy.backend_failure_codes.iter().cloned().collect();
        for code in &declared_backend {
            if !live_backend.contains(code) {
                errs.push(format!(
                    "[unknown-failure-code] terminal taxonomy code `{code}` is not a live FailureCode serde name"
                ));
            }
        }
        if declared_backend != live_backend {
            errs.push(
                "[incomplete-failure-vocabulary] terminal taxonomy codes must equal the closed live set"
                    .into(),
            );
        }
        let live_stage_ids: BTreeSet<String> = [
            ParseCacheStage::RawTei.stage_id().to_string(),
            ParseCacheStage::ProjectArticle.stage_id().to_string(),
        ]
        .into_iter()
        .collect();
        let declared_cache_stages: BTreeSet<String> =
            taxonomy.cache_failure_stages.iter().cloned().collect();
        for id in &declared_cache_stages {
            if !live_stage_ids.contains(id) {
                errs.push(format!(
                    "[unknown-cache-stage-id] cache failure stage `{id}` is not a live stage id"
                ));
            }
        }
        if declared_cache_stages != live_stage_ids {
            errs.push(
                "[incomplete-cache-stage-vocabulary] cache_failure_stages must equal the two live stage ids"
                    .into(),
            );
        }
        let live_integrity = live_integrity_code_names();
        let declared_integrity: BTreeSet<String> =
            taxonomy.cache_integrity_codes.iter().cloned().collect();
        for code in &declared_integrity {
            if !live_integrity.contains(code) {
                errs.push(format!(
                    "[unknown-cache-integrity-code] `{code}` is not a live CacheIntegrityCode serde name"
                ));
            }
        }
        if declared_integrity != live_integrity {
            errs.push(
                "[incomplete-cache-integrity-vocabulary] integrity codes must equal the closed live set"
                    .into(),
            );
        }
    }

    // Run summary schema + counters.
    {
        let summary = &obs.run_summary;
        if summary.schema_version != PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION {
            errs.push(format!(
                "[run-summary-schema-drift] schema_version {} != live {PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION}",
                summary.schema_version
            ));
        }
        let live_counters = live_run_summary_counter_names();
        let declared_counters: BTreeSet<String> = summary.counters.iter().cloned().collect();
        for counter in &declared_counters {
            if !live_counters.contains(counter) {
                errs.push(format!(
                    "[unknown-run-summary-counter] `{counter}` is not a live ParseBatchRunSummary serialized field"
                ));
            }
        }
        if declared_counters != live_counters {
            errs.push(
                "[incomplete-run-summary-counters] counters must equal the live serialized field set"
                    .into(),
            );
        }
    }

    // Legacy read-only identities.
    {
        let legacy = &obs.legacy_read_only;
        let live_identities: BTreeSet<&str> = ["pdf-grobid", "html-primary"].into_iter().collect();
        let declared: BTreeSet<&str> = legacy.identities.iter().map(String::as_str).collect();
        for id in &declared {
            if !live_identities.contains(id) {
                errs.push(format!(
                    "[unknown-legacy-identity] `{id}` is not a sanctioned legacy read-only identity"
                ));
            }
        }
        if declared != live_identities {
            errs.push(
                "[incomplete-legacy-identity-set] identities must be exactly pdf-grobid and html-primary"
                    .into(),
            );
        }
        if legacy.projection_schema != "ParsedArticle.v1" {
            errs.push(format!(
                "[legacy-identity-schema-drift] projection_schema `{}` must stay ParsedArticle.v1",
                legacy.projection_schema
            ));
        }
        if legacy.hit_creates_canonical_record {
            errs.push(
                "[legacy-identity-write-drift] a legacy hit must never create a canonical record"
                    .into(),
            );
        }
    }

    // redb accelerator role (D295).
    {
        let redb = &obs.redb;
        if redb.role != "rebuildable_only" {
            errs.push(format!(
                "[redb-role-drift] role `{}` must stay rebuildable_only (D295)",
                redb.role
            ));
        }
        if !redb.feature_gated {
            errs.push("[redb-role-drift] feature_gated must stay true".into());
        }
        let live_owners: BTreeSet<&str> = ["da-adapters", "da-cli"].into_iter().collect();
        let declared_owners: BTreeSet<&str> =
            redb.owner_crates.iter().map(String::as_str).collect();
        if declared_owners != live_owners {
            errs.push(format!(
                "[redb-role-drift] owner_crates {declared_owners:?} must be exactly the infrastructure crates {live_owners:?}"
            ));
        }
    }

    if !obs.raw_content_free {
        errs.push("[raw-content-flag-drift] raw_content_free must stay true".into());
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

fn acceptance_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn valid_acceptance_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
}

fn validate_acceptance_row(
    row: &AcceptanceRow,
    class: &str,
    proxy: bool,
    require_pdf: bool,
    require_tei: bool,
) -> Vec<String> {
    let mut errs = Vec::new();
    let ctx = format!("{class} row `{}`", row.id);
    if !valid_acceptance_id(&row.id) {
        errs.push(format!("[acceptance-bad-id] {ctx}"));
    }
    match row.state.as_str() {
        "bound" => {
            if require_pdf {
                let Some(path) = row.pdf_path.as_deref() else {
                    errs.push(format!("[acceptance-bound-path-missing] {ctx}"));
                    return errs;
                };
                if !proxy
                    || !path.starts_with("data/")
                    || path.starts_with('/')
                    || path.contains("..")
                {
                    errs.push(format!("[acceptance-path-escape] {ctx}"));
                }
                let Some(sha) = row.pdf_sha256.as_deref() else {
                    errs.push(format!("[acceptance-bound-without-sha256] {ctx}"));
                    return errs;
                };
                if !acceptance_sha256(sha) {
                    errs.push(format!("[acceptance-bad-sha256] {ctx}"));
                } else if let Ok(bytes) = std::fs::read(repo_root().join(path)) {
                    if hex::encode(Sha256::digest(&bytes)) != sha {
                        errs.push(format!("[acceptance-sha256-mismatch] {ctx}"));
                    }
                } else {
                    errs.push(format!("[acceptance-missing-bound-bytes] {ctx}"));
                }
            }
            if proxy && row.identity_ref.as_deref().unwrap_or("").is_empty() {
                errs.push(format!("[acceptance-missing-authority-ref] {ctx}"));
            }
            if require_tei {
                if let Some(tei) = row.tei_ref.as_deref() {
                    if !tei.starts_with("data/parse/") || row.tei_sha256.as_deref().is_none() {
                        errs.push(format!("[acceptance-tei-binding-invalid] {ctx}"));
                    }
                    if let Some(sha) = row.tei_sha256.as_deref()
                        && !acceptance_sha256(sha)
                    {
                        errs.push(format!("[acceptance-bad-sha256] {ctx} tei"));
                    }
                } else {
                    errs.push(format!("[acceptance-tei-ref-missing] {ctx}"));
                }
            }
        }
        "catalogued" | "bytes_absent" => {
            if row.pdf_sha256.is_some() || row.tei_sha256.is_some() {
                errs.push(format!("[acceptance-premature-sha256] {ctx}"));
            }
            if proxy {
                errs.push(format!("[acceptance-remote-in-proxy] {ctx}"));
            }
            if row.identity.as_deref().unwrap_or("").is_empty()
                || row.dataset_id.as_deref().unwrap_or("").is_empty()
                || row.source_url.as_deref().unwrap_or("").is_empty()
            {
                errs.push(format!("[acceptance-catalogue-identity-missing] {ctx}"));
            }
        }
        _ => errs.push(format!("[acceptance-unknown-row-state] {ctx}")),
    }
    errs
}

fn validate_acceptance_manifest(m: &AcceptanceManifest) -> Vec<String> {
    let mut errs = Vec::new();
    if m.schema_version != "parse-acceptance.v1" {
        errs.push("[acceptance-schema-version-drift] expected parse-acceptance.v1".into());
    }
    if m.adr_ref != "ADR-075" || m.manifest_id != "scholarly-parse-acceptance-s20" {
        errs.push("[acceptance-identity-drift] manifest identity is not sanctioned".into());
    }
    if m.requirement_ids
        != [
            "R085", "R091", "R105", "R107", "R109", "R110", "R113", "R114", "R116", "R117",
        ]
    {
        errs.push("[acceptance-requirement-binding-drift] requirement set changed".into());
    }
    if m.policy_locks.import_eligible
        || m.policy_locks.auto_promote != "forbidden"
        || m.policy_locks.production_route != "forbidden"
        || m.policy_locks.fallback_target != "java-grobid-http@0.9.0-crf"
    {
        errs.push("[acceptance-hard-lock-violation] acceptance policy locks eroded".into());
    }
    let rules = &m.comparison_rules;
    if rules.allowed_classes != ["publisher_acceptance", "layout_omnidocbench"]
        || rules.allowed_row_states != ["bound", "catalogued", "bytes_absent"]
        || rules.allowed_comparison_kinds != ["tei_family", "layout_markdown"]
    {
        errs.push("[acceptance-taxonomy-drift] closed acceptance taxonomy changed".into());
    }
    if rules.missing_behavior != "fail_visible"
        || rules.same_n != "required"
        || rules.catalogued_bytes_are_executed
        || rules.omnidocbench_is_tei_oracle
    {
        errs.push("[acceptance-semantics-violation] fail-visible or role semantics eroded".into());
    }

    let publisher = &m.classes.publisher_acceptance;
    if publisher.comparison_kind != "tei_family" {
        errs.push("[acceptance-comparison-kind-drift] publisher must use tei_family".into());
    }
    if publisher.purpose
        != "official publisher-style GROBID acceptance identities; local proxy is not a publisher identity"
        || publisher.authority != "https://huggingface.co/datasets/sciencialab/grobid-evaluation"
        || publisher.license != "CC-BY"
        || publisher.expected_structure != "article_directory_with_pdf_and_gold_jats_xml"
    {
        errs.push("[acceptance-publisher-authority-drift] publisher metadata changed".into());
    }
    for row in &publisher.rows {
        errs.extend(validate_acceptance_row(
            row,
            "publisher_acceptance",
            false,
            false,
            false,
        ));
        if row.state != "bytes_absent" {
            errs.push(format!(
                "[acceptance-remote-must-be-absent] publisher row `{}`",
                row.id
            ));
        }
        if row.expected_n.unwrap_or(0) == 0
            || !matches!(row.gold_format.as_deref(), Some("jats_nxml" | "jats_xml"))
            || row.filter.is_some()
        {
            errs.push(format!(
                "[acceptance-publisher-shape-drift] publisher row `{}`",
                row.id
            ));
        }
    }
    if publisher.rows.len() != 4 {
        errs.push(
            "[acceptance-publisher-denominator-drift] expected four official identities".into(),
        );
    }
    let layout = &m.classes.layout_omnidocbench;
    if layout.comparison_kind != "layout_markdown" {
        errs.push("[acceptance-omnidocbench-as-tei] layout class must be layout_markdown".into());
    }
    if layout.purpose
        != "scientific OmniDocBench layout and Markdown comparison only; never TEI conservation"
        || layout.authority != "https://huggingface.co/datasets/opendatalab/OmniDocBench"
        || layout.license != "Apache-2.0"
        || layout.expected_structure != "pdf_pages_with_json_block_and_span_annotations"
    {
        errs.push("[acceptance-layout-authority-drift] OmniDocBench metadata changed".into());
    }
    for row in &layout.rows {
        errs.extend(validate_acceptance_row(
            row,
            "layout_omnidocbench",
            false,
            false,
            false,
        ));
        if row.state != "bytes_absent" {
            errs.push(format!(
                "[acceptance-remote-must-be-absent] layout row `{}`",
                row.id
            ));
        }
        if row.expected_n != Some(1651)
            || row.gold_format.as_deref() != Some("json_annotations")
            || row.filter.as_deref()
                != Some("data_source in [academic_literature, research_report]")
        {
            errs.push(format!(
                "[acceptance-layout-shape-drift] layout row `{}`",
                row.id
            ));
        }
    }
    if layout.local_proxy.proxy_of != "scientific_layout_pages"
        || !layout.local_proxy.same_n
        || layout.local_proxy.page_denominator != Some(202)
        || layout.local_proxy.document_denominator != Some(3)
        || layout.local_proxy.oracle_ref.as_deref()
            != Some("crates/da-parse/tests/fixtures/layout_oracle.yaml")
        || layout.local_proxy.source_ref.as_deref() != Some("data/parse/layout_bakeoff.yaml#corpus")
    {
        errs.push(
            "[acceptance-layout-proxy-drift] layout proxy must bind independent 202-page oracle"
                .into(),
        );
    }
    if layout
        .local_proxy
        .oracle_ref
        .as_deref()
        .is_some_and(|p| !repo_root().join(p).is_file())
    {
        errs.push("[acceptance-missing-layout-oracle] independent oracle is absent".into());
    }
    let publisher_proxy = &publisher.local_proxy;
    if publisher_proxy.proxy_of != "scholarly_pdf_plus_java_tei" || !publisher_proxy.same_n {
        errs.push(
            "[acceptance-tei-proxy-drift] publisher proxy must preserve same-n TEI identity".into(),
        );
    }
    if publisher_proxy.rows.len() != 3 || layout.local_proxy.rows.len() != 3 {
        errs.push(
            "[acceptance-same-n-mismatch] local proxy denominator must be exactly three".into(),
        );
    }
    for row in &publisher_proxy.rows {
        errs.extend(validate_acceptance_row(
            row,
            "local_proxy",
            true,
            true,
            true,
        ));
    }
    for row in &layout.local_proxy.rows {
        errs.extend(validate_acceptance_row(
            row,
            "local_proxy",
            true,
            false,
            false,
        ));
    }
    let total_pages: u64 = layout
        .local_proxy
        .rows
        .iter()
        .filter_map(|r| r.page_count)
        .sum();
    if total_pages != 202 {
        errs.push(format!(
            "[acceptance-page-denominator-mismatch] expected 202, got {total_pages}"
        ));
    }
    if !publisher_proxy
        .rows
        .iter()
        .all(|r| layout.local_proxy.rows.iter().any(|l| l.id == r.id))
    {
        errs.push("[acceptance-same-n-mismatch] TEI and layout proxy identities differ".into());
    }
    errs
}

fn authority_record<'a>(value: &'a serde_yaml::Value, id: &str) -> Option<&'a serde_yaml::Mapping> {
    match value {
        serde_yaml::Value::Mapping(map) => {
            if map.get("id").and_then(serde_yaml::Value::as_str) == Some(id) {
                return Some(map);
            }
            map.values().find_map(|child| authority_record(child, id))
        }
        serde_yaml::Value::Sequence(items) => {
            items.iter().find_map(|child| authority_record(child, id))
        }
        _ => None,
    }
}

fn authority_lookup(reference: &str, id: &str) -> serde_yaml::Mapping {
    let path = reference.split('#').next().expect("authority path");
    let value: serde_yaml::Value = load_yaml(path).expect("authority YAML must load");
    authority_record(&value, id)
        .cloned()
        .unwrap_or_else(|| panic!("authority record `{id}` missing from `{path}`"))
}

// ───────────────────────────── positives ──────────────────────────────

#[test]
fn acceptance_manifest_loads_and_passes_contract_validation() {
    let manifest: AcceptanceManifest = expect_loaded(ACCEPTANCE_REL_PATH);
    let errs = validate_acceptance_manifest(&manifest);
    assert!(
        errs.is_empty(),
        "acceptance manifest contract violations:\n{}",
        errs.join("\n")
    );
}

#[test]
fn acceptance_manifest_references_resolve_to_original_authorities() {
    let manifest: AcceptanceManifest = expect_loaded(ACCEPTANCE_REL_PATH);
    for row in &manifest.classes.publisher_acceptance.local_proxy.rows {
        let record = authority_lookup(row.identity_ref.as_deref().unwrap(), &row.id);
        assert_eq!(
            record.get("source_pdf").and_then(serde_yaml::Value::as_str),
            row.pdf_path.as_deref()
        );
        assert_eq!(
            record.get("sha256").and_then(serde_yaml::Value::as_str),
            row.pdf_sha256.as_deref()
        );

        let reference = row.tei_ref.as_deref().unwrap();
        let id = reference
            .split('#')
            .nth(1)
            .unwrap()
            .trim_start_matches("cases.");
        let tei_record = authority_lookup(reference, id);
        let fixture = tei_record
            .get("fixture")
            .expect("regression fixture authority");
        assert_eq!(
            fixture.get("sha256").and_then(serde_yaml::Value::as_str),
            row.tei_sha256.as_deref()
        );
    }
    for row in &manifest.classes.layout_omnidocbench.local_proxy.rows {
        let record = authority_lookup(row.identity_ref.as_deref().unwrap(), &row.id);
        assert_eq!(
            record.get("pages").and_then(serde_yaml::Value::as_u64),
            row.page_count
        );
    }
}

#[test]
fn acceptance_manifest_preserves_remote_absence_and_local_same_n() {
    let manifest: AcceptanceManifest = expect_loaded(ACCEPTANCE_REL_PATH);
    assert!(
        manifest
            .classes
            .publisher_acceptance
            .rows
            .iter()
            .all(|row| row.state == "bytes_absent" && row.pdf_sha256.is_none())
    );
    assert_eq!(
        manifest.classes.publisher_acceptance.local_proxy.rows.len(),
        3
    );
    assert_eq!(
        manifest.classes.layout_omnidocbench.local_proxy.rows.len(),
        3
    );
    assert_eq!(
        manifest
            .classes
            .layout_omnidocbench
            .local_proxy
            .page_denominator,
        Some(202)
    );
}

#[test]
fn acceptance_manifest_rejects_isolated_mutations() {
    let valid: AcceptanceManifest = expect_loaded(ACCEPTANCE_REL_PATH);

    let mut missing = valid.clone();
    missing.classes.publisher_acceptance.local_proxy.rows[0].pdf_path =
        Some("data/sources/does-not-exist.pdf".into());
    negative(
        "[acceptance-missing-bound-bytes]",
        &validate_acceptance_manifest(&missing),
        "missing bound bytes",
    );

    let mut drift = valid.clone();
    drift.classes.publisher_acceptance.local_proxy.rows[0].pdf_sha256 = Some("a".repeat(64));
    negative(
        "[acceptance-sha256-mismatch]",
        &validate_acceptance_manifest(&drift),
        "checksum drift",
    );

    let mut premature = valid.clone();
    premature.classes.publisher_acceptance.rows[0].pdf_sha256 = Some("a".repeat(64));
    negative(
        "[acceptance-premature-sha256]",
        &validate_acceptance_manifest(&premature),
        "remote checksum substitution",
    );

    let mut same_n = valid.clone();
    same_n.classes.layout_omnidocbench.local_proxy.rows.pop();
    negative(
        "[acceptance-same-n-mismatch]",
        &validate_acceptance_manifest(&same_n),
        "same-n erosion",
    );

    let mut role = valid.clone();
    role.classes.layout_omnidocbench.comparison_kind = "tei_family".into();
    negative(
        "[acceptance-omnidocbench-as-tei]",
        &validate_acceptance_manifest(&role),
        "OmniDocBench used as TEI oracle",
    );

    let mut locks = valid;
    locks.policy_locks.import_eligible = true;
    negative(
        "[acceptance-hard-lock-violation]",
        &validate_acceptance_manifest(&locks),
        "import lock erosion",
    );
}

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

// ───────── S04/T04 typed transport cross-validation ─────────

#[test]
fn typed_transport_cross_validates_the_live_rust_vocabulary() {
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let errs = validate_ledger(&ledger);
    assert!(
        errs.is_empty(),
        "ledger with typed transport must validate:\n{}",
        errs.join("\n")
    );

    let entry = ledger
        .entries
        .iter()
        .find(|e| e.id == TYPED_TRANSPORT_ENTRY_ID)
        .expect("fulltext endpoint entry");
    let tt = entry
        .typed_transport
        .as_ref()
        .expect("typed transport record present");

    // Versioned request schema equals the live constants and the envelope
    // version label.
    assert_eq!(tt.request_schema_version, FULLTEXT_REQUEST_SCHEMA_VERSION);
    assert_eq!(
        tt.request_schema,
        format!(
            "fulltext-{}",
            FulltextRequestVersioned::v1(FulltextDialectV1::legacy_rich()).version_label()
        )
    );
    assert_eq!(tt.failure_schema_version, PARSE_FAILURE_SCHEMA_VERSION);
    let documented: BTreeSet<String> = tt.failure_codes.iter().cloned().collect();
    assert_eq!(documented, live_failure_code_names());
    assert_eq!(tt.statuses, TYPED_TRANSPORT_STATUSES.to_vec());

    // The documented request fields carry every sanctioned V1 dialect wire
    // name in the exact Java GROBID spelling the projector emits.
    let documented_names: Vec<&str> = entry
        .request_fields
        .iter()
        .map(|field| field.split('—').next().unwrap_or("").trim())
        .collect();
    for name in SANCTIONED_WIRE_FIELD_NAMES {
        assert!(
            documented_names.contains(&name),
            "ledger request_fields must document the V1 wire field `{name}`: {documented_names:?}"
        );
    }
}

#[test]
fn ledger_wire_fields_stay_inside_the_registry_official_dialect() {
    // No invented wire dialect: every ledger request-field name and every
    // da-parse accepted enum value must exist in the registry-of-record
    // official request_fields for rest.processFulltextDocument.
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let registry = load_registry_value();
    let cap = registry
        .get("capabilities")
        .and_then(|c| c.as_sequence())
        .and_then(|seq| {
            seq.iter().find(|c| {
                c.get("id").and_then(|i| i.as_str()) == Some("rest.processFulltextDocument")
            })
        })
        .expect("registry keeps the fulltext endpoint");
    let official_fields = cap
        .get("official")
        .and_then(|o| o.get("request_fields"))
        .and_then(|f| f.as_sequence())
        .expect("official request_fields list");

    let mut official_names: HashSet<String> = HashSet::new();
    let mut official_values: HashMap<String, HashSet<String>> = HashMap::new();
    for field in official_fields {
        let name = field
            .get("name")
            .and_then(|n| n.as_str())
            .expect("official field name")
            .to_string();
        if let Some(values) = field.get("values").and_then(|v| v.as_sequence()) {
            let set = official_values.entry(name.clone()).or_default();
            for value in values {
                if let Some(n) = value.as_u64() {
                    set.insert(n.to_string());
                } else if let Some(s) = value.as_str() {
                    set.insert(s.to_string());
                }
            }
        }
        official_names.insert(name);
    }

    let entry = ledger
        .entries
        .iter()
        .find(|e| e.id == TYPED_TRANSPORT_ENTRY_ID)
        .expect("fulltext endpoint entry");
    for field in &entry.request_fields {
        let name = field.split('—').next().unwrap_or("").trim();
        assert!(
            official_names.contains(name),
            "ledger wire field `{name}` is not an official registry field"
        );
    }

    // Live da-parse sanctioned ranges ⊆ official enum domains (probe the
    // FULL official domain so out-of-domain values prove rejection too).
    let header_accepted: Vec<u8> = (0..=3u8)
        .filter(|v| Consolidation::try_new(*v, 0, 0).is_ok())
        .collect();
    let citations_accepted: Vec<u8> = (0..=2u8)
        .filter(|v| Consolidation::try_new(0, *v, 0).is_ok())
        .collect();
    let funders_accepted: Vec<u8> = (0..=2u8)
        .filter(|v| Consolidation::try_new(0, 0, *v).is_ok())
        .collect();
    assert_eq!(
        header_accepted,
        vec![0, 1, 2],
        "sanctioned header range is 0..=2"
    );
    assert_eq!(
        citations_accepted,
        vec![0, 1],
        "sanctioned citations range is 0..=1"
    );
    assert_eq!(
        funders_accepted,
        vec![0, 1],
        "sanctioned funders range is 0..=1"
    );
    for (field_name, accepted) in [
        ("consolidateHeader", &header_accepted),
        ("consolidateCitations", &citations_accepted),
        ("consolidateFunders", &funders_accepted),
    ] {
        let official = official_values
            .get(field_name)
            .unwrap_or_else(|| panic!("registry declares {field_name} enum values"));
        for level in accepted {
            assert!(
                official.contains(&level.to_string()),
                "da-parse accepts {field_name}={level} but the registry official dialect does not"
            );
        }
    }

    // Coordinate families stay inside the official teiCoordinates list.
    let official_coords = official_values
        .get("teiCoordinates")
        .expect("registry declares teiCoordinates values");
    for family in CoordinateSet::all().wire_value().split(',') {
        assert!(
            official_coords.contains(family),
            "da-parse coordinate family `{family}` is not an official teiCoordinates value"
        );
    }
}

/// Mutate the typed transport record of a sanctioned endpoint binding.
fn typed_transport_mut<F: FnOnce(&mut TypedTransportContract)>(entry_id: &str, f: F) -> LedgerFile {
    let (mut ledger, _) = loaded_pair();
    let idx = ledger
        .entries
        .iter()
        .position(|e| e.id == entry_id)
        .unwrap_or_else(|| panic!("{entry_id} present"));
    let mut tt = ledger.entries[idx]
        .typed_transport
        .clone()
        .expect("typed transport present");
    f(&mut tt);
    ledger.entries[idx].typed_transport = Some(tt);
    ledger
}

fn fulltext_typed_transport_mut<F: FnOnce(&mut TypedTransportContract)>(f: F) -> LedgerFile {
    typed_transport_mut(TYPED_TRANSPORT_ENTRY_ID, f)
}

/// Mutate the observable entry fields (not the typed transport record) of a
/// sanctioned binding.
fn ledger_entry_mut<F: FnOnce(&mut LedgerEntry)>(entry_id: &str, f: F) -> LedgerFile {
    let (mut ledger, _) = loaded_pair();
    let idx = ledger
        .entries
        .iter()
        .position(|e| e.id == entry_id)
        .unwrap_or_else(|| panic!("{entry_id} present"));
    f(&mut ledger.entries[idx]);
    ledger
}

#[test]
fn rejects_invented_failure_code_in_typed_transport() {
    let ledger = fulltext_typed_transport_mut(|tt| {
        tt.failure_codes.push("model_overheated".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative("[unknown-failure-code]", &errs, "invented failure code");
    negative(
        "[incomplete-failure-vocabulary]",
        &errs,
        "vocabulary no longer equals the closed set",
    );
}

#[test]
fn rejects_incomplete_closed_failure_vocabulary() {
    let ledger = fulltext_typed_transport_mut(|tt| {
        tt.failure_codes.retain(|code| code != "no_content");
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[incomplete-failure-vocabulary]",
        &errs,
        "dropped a closed code",
    );
}

#[test]
fn rejects_unsupported_request_version_in_typed_transport() {
    let ledger = fulltext_typed_transport_mut(|tt| {
        tt.request_schema_version = 9;
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[typed-transport-version]",
        &errs,
        "unsupported request schema version",
    );

    let ledger = fulltext_typed_transport_mut(|tt| {
        tt.request_schema = "fulltext-v9".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative("[typed-transport-version]", &errs, "invented version label");
}

#[test]
fn rejects_unmapped_status_in_typed_transport() {
    let ledger = fulltext_typed_transport_mut(|tt| {
        tt.statuses.push(418);
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[typed-transport-status-taxonomy]",
        &errs,
        "status outside the typed taxonomy",
    );
}

#[test]
fn rejects_dropped_retry_or_facade_guarantees_in_typed_transport() {
    let ledger = fulltext_typed_transport_mut(|tt| {
        tt.retry_policy = "silent_retry_on_503".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[typed-transport-retry-drift]",
        &errs,
        "hidden retry policy",
    );

    let ledger = fulltext_typed_transport_mut(|tt| {
        tt.compatibility_facade = "single new ParserError variant".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[typed-transport-facade-drift]",
        &errs,
        "facade mapping lost a legacy variant",
    );
}

#[test]
fn rejects_missing_or_misplaced_typed_transport_record() {
    let (mut ledger, _) = loaded_pair();
    let idx = ledger
        .entries
        .iter()
        .position(|e| e.id == TYPED_TRANSPORT_ENTRY_ID)
        .expect("fulltext endpoint entry");
    ledger.entries[idx].typed_transport = None;
    let errs = validate_ledger(&ledger);
    negative(
        "[missing-typed-transport]",
        &errs,
        "removed typed transport record",
    );

    let (mut ledger, _) = loaded_pair();
    let record = ledger.entries[idx].typed_transport.clone();
    let lift_idx = ledger
        .entries
        .iter()
        .position(|e| e.id == "parse.stage.lift_tei")
        .expect("lift entry present");
    ledger.entries[lift_idx].typed_transport = record;
    let errs = validate_ledger(&ledger);
    negative(
        "[misplaced-typed-transport]",
        &errs,
        "typed transport moved off the endpoint binding",
    );

    // The header proxy record is equally load-bearing (S05/T03).
    let mut ledger = typed_transport_mut(TYPED_TRANSPORT_ENTRY_IDS[1], |_| {});
    let idx = ledger
        .entries
        .iter()
        .position(|e| e.id == TYPED_TRANSPORT_ENTRY_IDS[1])
        .expect("header endpoint entry");
    ledger.entries[idx].typed_transport = None;
    let errs = validate_ledger(&ledger);
    negative(
        "[missing-typed-transport]",
        &errs,
        "removed header proxy typed transport",
    );
}

#[test]
fn rejects_unknown_fields_inside_typed_transport() {
    let bad = "endpoint: header\nrequest_schema: header-v1\nrequest_schema_version: 1\nresult_kind: header\nresult_identity_namespace: header=v1\nfailure_schema_version: 1\nstatuses: [200]\nfailure_codes: []\nretry_policy: honest_503_no_hidden_retry\nretry_after_metadata: bounded_parse\ncompatibility_facade: x\nrollback_target: java-grobid-http@0.9.0-crf\ninvented_field: true\n";
    match serde_yaml::from_str::<TypedTransportContract>(bad) {
        Err(e) => assert!(
            e.to_string().contains("unknown field"),
            "deny_unknown_fields should trigger, got: {e}"
        ),
        Ok(_) => panic!("invented typed_transport field must not deserialize"),
    }
}

#[test]
fn rejects_invented_registry_binding_reference_on_the_endpoint() {
    let (mut ledger, _) = loaded_pair();
    let idx = ledger
        .entries
        .iter()
        .position(|e| e.id == TYPED_TRANSPORT_ENTRY_ID)
        .expect("fulltext endpoint entry");
    for n in &mut ledger.entries[idx].notes {
        if n.trim_start().starts_with("registry_binding:") {
            *n = "registry_binding: rest.inventedSuperParser".to_string();
        }
    }
    let errs = validate_registry_binding_notes(&ledger, &load_registry_ids());
    negative(
        "[unresolved-registry-binding-note]",
        &errs,
        "invented endpoint registry reference",
    );
}

// ───────── S05/T03 proxy typed transport cross-validation ─────────

#[test]
fn proxy_typed_transport_cross_validates_the_live_rust_vocabulary() {
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let errs = validate_ledger(&ledger);
    assert!(
        errs.is_empty(),
        "ledger with proxy typed transports must validate:\n{}",
        errs.join("\n")
    );

    let mut records: Vec<(String, TypedTransportContract)> = Vec::new();
    for entry_id in TYPED_TRANSPORT_ENTRY_IDS {
        let entry = ledger
            .entries
            .iter()
            .find(|e| e.id == entry_id)
            .unwrap_or_else(|| panic!("{entry_id} present"));
        let tt = entry
            .typed_transport
            .as_ref()
            .unwrap_or_else(|| panic!("{entry_id} carries a typed transport record"));
        let endpoint = parse_endpoint_name(&tt.endpoint)
            .unwrap_or_else(|| panic!("{entry_id}: closed endpoint expected"));
        assert_eq!(
            entry.http_endpoint.as_deref(),
            Some(endpoint.static_path()),
            "{entry_id}: documented route must be the live static path"
        );
        assert_eq!(
            tt.result_kind,
            endpoint.result_kind().as_str(),
            "{entry_id}: result kind must equal the live closed mapping"
        );
        records.push((tt.endpoint.clone(), tt.clone()));
    }

    // Exactly the three sanctioned endpoints — an invented proxy operation
    // cannot smuggle itself into the ledger.
    let endpoints: BTreeSet<String> = records
        .iter()
        .map(|(endpoint, _)| endpoint.clone())
        .collect();
    assert_eq!(
        endpoints,
        ["fulltext", "header", "references"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        "exactly the three sanctioned endpoints may carry typed transports"
    );

    // ONE shared failure schema across all three operations (same exchange).
    let shared = |tt: &TypedTransportContract| {
        (
            tt.failure_schema_version,
            tt.statuses.clone(),
            tt.failure_codes.clone(),
            tt.retry_policy.clone(),
            tt.retry_after_metadata.clone(),
        )
    };
    let baseline = shared(&records[0].1);
    for (endpoint, tt) in &records[1..] {
        assert_eq!(
            shared(tt),
            baseline,
            "{endpoint}: the sanctioned operations share ONE failure schema"
        );
    }

    // Per-endpoint multipart allowlists equal the live closed projectors.
    let header_entry = ledger
        .entries
        .iter()
        .find(|e| e.id == TYPED_TRANSPORT_ENTRY_IDS[1])
        .expect("header entry");
    let references_entry = ledger
        .entries
        .iter()
        .find(|e| e.id == TYPED_TRANSPORT_ENTRY_IDS[2])
        .expect("references entry");
    let header_documented: BTreeSet<String> =
        documented_field_names(header_entry).into_iter().collect();
    let references_documented: BTreeSet<String> = documented_field_names(references_entry)
        .into_iter()
        .collect();
    let header_allowed: BTreeSet<String> =
        HEADER_WIRE_FIELDS.into_iter().map(str::to_string).collect();
    let references_allowed: BTreeSet<String> = REFERENCES_WIRE_FIELDS
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(
        header_documented, header_allowed,
        "header proxy allowlist must be exactly the closed live projector surface"
    );
    assert_eq!(
        references_documented, references_allowed,
        "references proxy allowlist must be exactly the closed live projector surface"
    );
}

#[test]
fn registry_official_dialects_match_the_live_endpoint_allowlists() {
    let errs = validate_registry_endpoint_dialects(&load_registry_value());
    assert!(
        errs.is_empty(),
        "registry-of-record endpoint dialects drifted from the live allowlists:\n{}",
        errs.join("\n")
    );
}

#[test]
fn rejects_invented_endpoint_or_result_kind_in_proxy_typed_transport() {
    let ledger = typed_transport_mut(TYPED_TRANSPORT_ENTRY_IDS[1], |tt| {
        tt.endpoint = "processFulltextAssetDocument".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[unknown-typed-transport-endpoint]",
        &errs,
        "invented endpoint name",
    );

    let ledger = typed_transport_mut(TYPED_TRANSPORT_ENTRY_IDS[1], |tt| {
        tt.result_kind = "bibliography".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative("[unknown-result-kind]", &errs, "invented result kind");

    let ledger = typed_transport_mut(TYPED_TRANSPORT_ENTRY_IDS[2], |tt| {
        tt.result_kind = "full_document".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative("[unknown-result-kind]", &errs, "cross-endpoint result kind");
}

#[test]
fn rejects_foreign_knob_in_proxy_request_fields() {
    // teiCoordinates must never return to the corrected header/references
    // dialects, and no references/header knob may cross over.
    let ledger = ledger_entry_mut(TYPED_TRANSPORT_ENTRY_IDS[1], |entry| {
        entry
            .request_fields
            .push("teiCoordinates — invented drift re-add".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[foreign-endpoint-field]",
        &errs,
        "teiCoordinates on header",
    );

    let ledger = ledger_entry_mut(TYPED_TRANSPORT_ENTRY_IDS[1], |entry| {
        entry
            .request_fields
            .push("includeRawCitations — references-only knob".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[foreign-endpoint-field]",
        &errs,
        "citations knob on header",
    );

    let ledger = ledger_entry_mut(TYPED_TRANSPORT_ENTRY_IDS[2], |entry| {
        entry
            .request_fields
            .push("consolidateHeader — header-only knob".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[foreign-endpoint-field]",
        &errs,
        "header knob on references",
    );
}

#[test]
fn rejects_incomplete_proxy_field_allowlist() {
    let ledger = ledger_entry_mut(TYPED_TRANSPORT_ENTRY_IDS[2], |entry| {
        entry
            .request_fields
            .retain(|field| !field.starts_with("input"));
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[incomplete-endpoint-field-allowlist]",
        &errs,
        "dropped input field",
    );
}

#[test]
fn rejects_proxy_version_route_identity_or_rollback_drift() {
    let ledger = typed_transport_mut(TYPED_TRANSPORT_ENTRY_IDS[1], |tt| {
        tt.request_schema_version = 2;
    });
    let errs = validate_ledger(&ledger);
    negative("[typed-transport-version]", &errs, "unsupported version");

    let ledger = typed_transport_mut(TYPED_TRANSPORT_ENTRY_IDS[1], |tt| {
        tt.result_identity_namespace = "header=v2".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[typed-transport-identity-namespace]",
        &errs,
        "invented identity namespace",
    );

    let ledger = ledger_entry_mut(TYPED_TRANSPORT_ENTRY_IDS[1], |entry| {
        entry.http_endpoint = Some("/api/processHeaderDocumentV2".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative("[typed-transport-route-drift]", &errs, "free-form route");

    let ledger = typed_transport_mut(TYPED_TRANSPORT_ENTRY_IDS[2], |tt| {
        tt.rollback_target = "native-rust-parser@v9".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[typed-transport-rollback-drift]",
        &errs,
        "rotated rollback target",
    );

    let ledger = typed_transport_mut(TYPED_TRANSPORT_ENTRY_IDS[2], |tt| {
        tt.statuses.push(418);
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[typed-transport-status-taxonomy]",
        &errs,
        "status outside the typed taxonomy",
    );

    let ledger = typed_transport_mut(TYPED_TRANSPORT_ENTRY_IDS[2], |tt| {
        tt.failure_codes.push("model_overheated".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative("[unknown-failure-code]", &errs, "invented failure code");
}

#[test]
fn rejects_registry_dialect_drift_back_to_pre_s05_claims() {
    // Re-adding teiCoordinates to the corrected header dialect fails.
    let mut registry = load_registry_value();
    let caps = registry
        .get_mut("capabilities")
        .and_then(|c| c.as_sequence_mut())
        .expect("capabilities sequence");
    let header = caps
        .iter_mut()
        .find(|c| c.get("id").and_then(|i| i.as_str()) == Some("rest.processHeaderDocument"))
        .expect("header capability");
    let fields = header
        .get_mut("official")
        .and_then(|o| o.get_mut("request_fields"))
        .and_then(|f| f.as_sequence_mut())
        .expect("official request_fields");
    let mut invented = serde_yaml::Mapping::new();
    invented.insert(
        serde_yaml::Value::String("name".into()),
        serde_yaml::Value::String("teiCoordinates".into()),
    );
    fields.push(serde_yaml::Value::Mapping(invented));
    let errs = validate_registry_endpoint_dialects(&registry);
    negative(
        "[registry-endpoint-field-drift]",
        &errs,
        "teiCoordinates re-added to header",
    );

    // Demoting the sanctioned proxy back to absent fails.
    let mut registry = load_registry_value();
    let caps = registry
        .get_mut("capabilities")
        .and_then(|c| c.as_sequence_mut())
        .expect("capabilities sequence");
    let references = caps
        .iter_mut()
        .find(|c| c.get("id").and_then(|i| i.as_str()) == Some("rest.processReferences"))
        .expect("references capability");
    if let Some(state) = references.get_mut("state") {
        *state = serde_yaml::Value::String("absent".into());
    }
    let errs = validate_registry_endpoint_dialects(&registry);
    negative(
        "[registry-endpoint-state-drift]",
        &errs,
        "proxy demoted to absent",
    );

    // Dropping the input field breaks the closed allowlist.
    let mut registry = load_registry_value();
    let caps = registry
        .get_mut("capabilities")
        .and_then(|c| c.as_sequence_mut())
        .expect("capabilities sequence");
    let header = caps
        .iter_mut()
        .find(|c| c.get("id").and_then(|i| i.as_str()) == Some("rest.processHeaderDocument"))
        .expect("header capability");
    if let Some(fields) = header
        .get_mut("official")
        .and_then(|o| o.get_mut("request_fields"))
        .and_then(|f| f.as_sequence_mut())
    {
        fields.retain(|f| f.get("name").and_then(|n| n.as_str()) != Some("input"));
    }
    let errs = validate_registry_endpoint_dialects(&registry);
    negative(
        "[registry-endpoint-allowlist-incomplete]",
        &errs,
        "input dropped from header",
    );
}

// ───── S06/T05 batch/cache observation cross-validation tests ─────

/// Mutate the batch/cache observation record of the loaded ledger.
fn observation_mut<F: FnOnce(&mut BatchCacheObservation)>(f: F) -> LedgerFile {
    let (mut ledger, _) = loaded_pair();
    f(&mut ledger.batch_cache_observation);
    ledger
}

#[test]
fn batch_cache_observation_cross_validates_the_live_rust_vocabulary() {
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let errs = validate_ledger(&ledger);
    assert!(
        errs.is_empty(),
        "ledger with batch/cache observation must validate:\n{}",
        errs.join("\n")
    );
    let observation = &ledger.batch_cache_observation;

    // Modes: the DATA truth table equals the LIVE truth-table methods.
    assert_eq!(
        observation.cache_modes.len(),
        4,
        "exactly the four sanctioned modes"
    );
    for row in &observation.cache_modes {
        let mode = parse_live_cache_mode(&row.mode).expect("closed mode");
        assert_eq!(row.reads_stages, mode.reads_stages(), "{}", row.mode);
        assert_eq!(
            row.publishes_results,
            mode.publishes_results(),
            "{}",
            row.mode
        );
        assert_eq!(row.bypasses_hits, mode.bypasses_hits(), "{}", row.mode);
    }

    // Stage identities equal the live constants, and the live stage keys
    // really carry the DATA-declared stage-id prefixes.
    for stage in &observation.cache_stages {
        let live = parse_live_cache_stage(&stage.stage).expect("closed stage");
        assert_eq!(stage.stage_id, live.stage_id());
    }
    let request = ParseBackendRequest::with_dialect(
        "paper-ledger",
        b"pdf-bytes".to_vec(),
        FulltextDialectV1::legacy_rich(),
    );
    assert!(
        raw_tei_stage_key(&request).starts_with(RAW_TEI_STAGE_ID),
        "the live raw stage key must carry the DATA-declared stage-id prefix"
    );
    assert!(
        project_article_stage_key(
            "paper-ledger",
            "pdf-sha",
            "raw-sha",
            PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        )
        .starts_with(PROJECT_ARTICLE_STAGE_ID),
        "the live projector stage key must carry the DATA-declared stage-id prefix"
    );

    // Cursor: fail-closed against a foreign input fingerprint (live probe).
    let cursor = BatchCursor::at("fingerprint-a", 1);
    assert!(cursor.validate_for("fingerprint-b", 4).is_err());
    assert!(cursor.validate_for("fingerprint-a", 4).is_ok());

    // Batch limits: zero bounds fail closed at the wire (live NonZero type).
    let zero_wire = format!(
        "{{\"schema_version\":{PARSE_BATCH_SCHEMA_VERSION},\"workers\":0,\"queue_capacity\":2,\"max_in_flight\":2,\"max_documents\":4}}"
    );
    assert!(
        serde_json::from_str::<ParseBatchLimits>(&zero_wire).is_err(),
        "a zero bound must be a wire error (NonZero bounds)"
    );

    // Config keys: the registry documents exactly the live da-cli keys.
    let key_errs = validate_registry_config_keys(&load_registry_value());
    assert!(
        key_errs.is_empty(),
        "config key drift:\n{}",
        key_errs.join("\n")
    );

    // The ledger tree stays exactly the sanctioned file set.
    let tree_errs = validate_data_parse_tree();
    assert!(
        tree_errs.is_empty(),
        "parallel ledger tree:\n{}",
        tree_errs.join("\n")
    );

    // No raw parse material entered the ledger text.
    let ledger_text =
        std::fs::read_to_string(repo_root().join(LEDGER_REL_PATH)).expect("ledger readable");
    let raw_errs = raw_content_scan(&ledger_text);
    assert!(
        raw_errs.is_empty(),
        "raw content in ledger:\n{}",
        raw_errs.join("\n")
    );
}

#[test]
fn rejects_invented_cache_mode_in_batch_cache_observation() {
    let ledger = observation_mut(|obs| {
        obs.cache_modes.push(CacheModeObservation {
            mode: "archive-only".to_string(),
            reads_stages: false,
            publishes_results: false,
            bypasses_hits: false,
        });
    });
    let errs = validate_ledger(&ledger);
    negative("[unknown-cache-mode]", &errs, "invented cache mode");
    negative(
        "[incomplete-cache-mode-vocabulary]",
        &errs,
        "vocabulary no longer equals the closed set",
    );
}

#[test]
fn rejects_cache_mode_truth_table_drift() {
    let ledger = observation_mut(|obs| {
        if let Some(row) = obs
            .cache_modes
            .iter_mut()
            .find(|row| row.mode == "write-through")
        {
            row.publishes_results = false;
        }
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[cache-mode-truth-table]",
        &errs,
        "write-through lost its publish",
    );

    let ledger = observation_mut(|obs| {
        if let Some(row) = obs.cache_modes.iter_mut().find(|row| row.mode == "refresh") {
            row.bypasses_hits = false;
        }
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[cache-mode-truth-table]",
        &errs,
        "refresh lost its hit bypass",
    );
}

#[test]
fn rejects_invented_cache_stage_or_schema_drift() {
    let ledger = observation_mut(|obs| {
        if let Some(raw) = obs.cache_stages.iter_mut().find(|s| s.stage == "raw_tei") {
            raw.stage_id = "parse.raw_tei.v2".to_string();
        }
    });
    let errs = validate_ledger(&ledger);
    negative("[unknown-cache-stage-id]", &errs, "rotated raw stage id");

    let ledger = observation_mut(|obs| {
        if let Some(projector) = obs
            .cache_stages
            .iter_mut()
            .find(|s| s.stage == "project_article")
        {
            projector.payload_schema_version += 1;
        }
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[cache-stage-schema-drift]",
        &errs,
        "rotated payload schema version",
    );

    let ledger = observation_mut(|obs| {
        if let Some(raw) = obs.cache_stages.iter_mut().find(|s| s.stage == "raw_tei") {
            raw.rotation = "shared_with_projector".to_string();
        }
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[cache-stage-rotation-drift]",
        &errs,
        "stages no longer rotate independently",
    );

    let ledger = observation_mut(|obs| {
        obs.cache_stages.push(CacheStageObservation {
            stage: "summary_text".to_string(),
            stage_id: "parse.summary_text.v1".to_string(),
            payload_schema_version: 1,
            projector_schema_fingerprint: None,
            key_dependencies: vec!["paper_id".to_string()],
            paper_id_in_key: true,
            rotation: "independent".to_string(),
        });
    });
    let errs = validate_ledger(&ledger);
    negative("[unknown-cache-stage]", &errs, "invented cache stage");
    negative(
        "[incomplete-cache-stage-vocabulary]",
        &errs,
        "vocabulary no longer equals the closed set",
    );
}

#[test]
fn rejects_invented_stage_key_dependency() {
    let ledger = observation_mut(|obs| {
        if let Some(raw) = obs.cache_stages.iter_mut().find(|s| s.stage == "raw_tei") {
            raw.key_dependencies.push("worker_count".to_string());
        }
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[unknown-stage-key-dependency]",
        &errs,
        "tuning became key material",
    );

    let ledger = observation_mut(|obs| {
        if let Some(raw) = obs.cache_stages.iter_mut().find(|s| s.stage == "raw_tei") {
            raw.key_dependencies.retain(|dep| dep != "source_bytes");
        }
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[incomplete-stage-key-dependencies]",
        &errs,
        "dropped a key dependency",
    );

    let ledger = observation_mut(|obs| {
        if let Some(projector) = obs
            .cache_stages
            .iter_mut()
            .find(|s| s.stage == "project_article")
        {
            projector.paper_id_in_key = false;
        }
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[cache-stage-key-binding]",
        &errs,
        "paper id left the projector key",
    );
}

#[test]
fn rejects_batch_limits_cursor_and_taxonomy_drift() {
    let ledger = observation_mut(|obs| {
        obs.batch_limits.bounded_fields.push("timeouts".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[unknown-batch-limit-field]",
        &errs,
        "tuning became a declared bound",
    );

    let ledger = observation_mut(|obs| {
        obs.batch_limits.content_identity_excluded = false;
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[batch-limit-drift]",
        &errs,
        "bounds entered content identity",
    );

    let ledger = observation_mut(|obs| {
        obs.cursor.input_binding = "manifest_row_order".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative("[cursor-binding-drift]", &errs, "invented cursor binding");

    let ledger = observation_mut(|obs| {
        obs.terminal_per_document_taxonomy.outcome_schema_version += 1;
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[terminal-schema-drift]",
        &errs,
        "unsupported outcome schema version",
    );

    let ledger = observation_mut(|obs| {
        obs.terminal_per_document_taxonomy
            .backend_failure_codes
            .push("model_overheated".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[unknown-failure-code]",
        &errs,
        "invented terminal failure code",
    );

    let ledger = observation_mut(|obs| {
        obs.terminal_per_document_taxonomy
            .cache_integrity_codes
            .push("baton_pass_failed".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[unknown-cache-integrity-code]",
        &errs,
        "invented integrity code",
    );

    let ledger = observation_mut(|obs| {
        obs.terminal_per_document_taxonomy
            .cache_failure_stages
            .push("parse.invented.v1".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[unknown-cache-stage-id]",
        &errs,
        "invented cache failure stage",
    );
}

#[test]
fn rejects_run_summary_counter_drift() {
    let ledger = observation_mut(|obs| {
        obs.run_summary.counters.push("invented_gauge".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative("[unknown-run-summary-counter]", &errs, "invented counter");

    let ledger = observation_mut(|obs| {
        obs.run_summary
            .counters
            .retain(|counter| counter != "succeeded");
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[incomplete-run-summary-counters]",
        &errs,
        "dropped a live counter",
    );
}

#[test]
fn rejects_legacy_identity_and_redb_role_drift() {
    let ledger = observation_mut(|obs| {
        obs.legacy_read_only
            .identities
            .push("native-parser-v9".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[unknown-legacy-identity]",
        &errs,
        "invented legacy identity",
    );

    let ledger = observation_mut(|obs| {
        obs.legacy_read_only.hit_creates_canonical_record = true;
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[legacy-identity-write-drift]",
        &errs,
        "legacy read-only became a promotion",
    );

    let ledger = observation_mut(|obs| {
        obs.redb.role = "authoritative_store".to_string();
    });
    let errs = validate_ledger(&ledger);
    negative("[redb-role-drift]", &errs, "redb claimed authority");

    let ledger = observation_mut(|obs| {
        obs.redb.owner_crates.push("da-parse".to_string());
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[redb-role-drift]",
        &errs,
        "redb moved into the parse plane",
    );
}

#[test]
fn rejects_invented_config_key_in_the_registry() {
    let mut registry = load_registry_value();
    let keys = registry
        .get_mut("defaults")
        .and_then(|d| d.get_mut("configuration_keys_documented_not_owned"))
        .and_then(|v| v.as_sequence_mut())
        .expect("config key sequence");
    keys.push(serde_yaml::Value::String("DA_PARSE_INVENTED_KEY".into()));
    let errs = validate_registry_config_keys(&registry);
    negative(
        "[unknown-config-key]",
        &errs,
        "documented key no live constant defines",
    );

    let mut registry = load_registry_value();
    let keys = registry
        .get_mut("defaults")
        .and_then(|d| d.get_mut("configuration_keys_documented_not_owned"))
        .and_then(|v| v.as_sequence_mut())
        .expect("config key sequence");
    keys.retain(|value| value.as_str() != Some("DA_PARSE_CACHE_MODE"));
    let errs = validate_registry_config_keys(&registry);
    negative("[incomplete-config-key-set]", &errs, "dropped a live key");
}

#[test]
fn rejects_parallel_ledger_tree_and_raw_content() {
    let errs = data_parse_tree_errs(vec![
        ("SCHEMA.yaml".to_string(), false),
        ("api_compatibility_ledger.yaml".to_string(), false),
        ("comparison_contract.yaml".to_string(), false),
        ("grobid_compatibility.yaml".to_string(), false),
        ("regression_cases.yaml".to_string(), false),
        ("layout_bakeoff.yaml".to_string(), false),
        ("document_segmentation.yaml".to_string(), false),
        ("native_visual_heads.yaml".to_string(), false),
        ("parse_ledger_v2.yaml".to_string(), false),
    ]);
    negative(
        "[parallel-ledger-tree]",
        &errs,
        "a second ledger file appeared",
    );

    let errs = data_parse_tree_errs(vec![("api_compatibility_ledger.yaml".to_string(), false)]);
    negative(
        "[missing-sanctioned-data-file]",
        &errs,
        "a sanctioned file vanished",
    );

    let errs = raw_content_scan("report: <TEI>verbatim</TEI>");
    negative("[raw-content-in-ledger]", &errs, "raw TEI in the ledger");

    let ledger = observation_mut(|obs| {
        obs.raw_content_free = false;
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[raw-content-flag-drift]",
        &errs,
        "raw-content flag relaxed",
    );
}

#[test]
fn rejects_registry_batch_cache_drift_against_the_observation() {
    let (ledger, _) = loaded_pair();
    let mut registry = load_registry_value();
    let section = registry
        .get_mut("defaults")
        .and_then(|d| d.get_mut("batch_cache"))
        .expect("registry batch_cache section");
    if let Some(modes) = section
        .get_mut("cache_modes")
        .and_then(|v| v.as_sequence_mut())
    {
        modes.retain(|mode| mode.as_str() != Some("refresh"));
        modes.push(serde_yaml::Value::String("rebuild".into()));
    }
    let errs =
        validate_batch_cache_registry_consistency(&ledger.batch_cache_observation, &registry);
    negative(
        "[registry-batch-cache-drift]",
        &errs,
        "registry mode vocabulary drifted",
    );

    let mut registry = load_registry_value();
    let section = registry
        .get_mut("defaults")
        .and_then(|d| d.get_mut("batch_cache"))
        .expect("registry batch_cache section");
    if let Some(role) = section.get_mut("redb_role") {
        *role = serde_yaml::Value::String("authoritative_store".into());
    }
    let errs =
        validate_batch_cache_registry_consistency(&ledger.batch_cache_observation, &registry);
    negative(
        "[registry-batch-cache-drift]",
        &errs,
        "registry redb role drifted",
    );

    let empty = serde_yaml::from_str::<serde_yaml::Value>("defaults: {}").expect("empty registry");
    let errs = validate_batch_cache_registry_consistency(&ledger.batch_cache_observation, &empty);
    negative(
        "[registry-batch-cache-drift]",
        &errs,
        "registry lost defaults.batch_cache",
    );
}

#[test]
fn rejects_invented_field_inside_batch_cache_observation() {
    let text = std::fs::read_to_string(repo_root().join(LEDGER_REL_PATH)).expect("ledger readable");
    let tampered = text.replace(
        "  raw_content_free: true",
        "  raw_content_free: true\n  invented_accelerator: redb-authority",
    );
    assert_ne!(text, tampered, "anchor for the injected field not found");
    match serde_yaml::from_str::<LedgerFile>(&tampered) {
        Err(e) => assert!(
            e.to_string().contains("unknown field"),
            "deny_unknown_fields should trigger, got: {e}"
        ),
        Ok(_) => panic!("an invented observation field must not deserialize"),
    }
}

// ───────────────── S07/T05 projection-identity rotation ───────────────────
//
// The additive P0 lift families joined the ParsedArticle wire, so the
// projection schema fingerprint rotated (ParsedArticle@serde-v4-p0-tei-lift).
// These checks bind the rotation mutually: Rust const ↔ ledger ↔ registry,
// the retired v3 literal must be fully rotated out of the live surfaces,
// and a re-introduced retired value must fail the exact tagged checks.

/// The retired projection fingerprint. Its ONLY sanctioned residence is
/// rotation-history documentation (e.g. the da-parse const doc comment);
/// it must never label a live stage key or data contract again.
const RETIRED_PROJECTION_FINGERPRINT: &str = "ParsedArticle@serde-v3-unique-navigation";

#[test]
fn s07_projection_fingerprint_is_mutually_bound_across_rust_ledger_and_registry() {
    // The live const carries the S07 rotation.
    assert_eq!(
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT, "ParsedArticle@serde-v4-p0-tei-lift",
        "the projection schema fingerprint must carry the S07 P0-lift rotation"
    );

    // Direct Rust↔registry binding: the registry of record declares the
    // LIVE fingerprint (the ledger↔registry agreement is checked elsewhere;
    // this closes the triangle so a registry-only revert fails here).
    let registry = load_registry_value();
    let registry_fp = registry
        .get("defaults")
        .and_then(|d| d.get("batch_cache"))
        .and_then(|b| b.get("cache_stages"))
        .and_then(|s| s.get("projector_schema_fingerprint"))
        .and_then(|v| v.as_str())
        .expect("registry declares projector_schema_fingerprint");
    assert_eq!(
        registry_fp, PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        "registry projector_schema_fingerprint drifted from the live da-parse const"
    );

    // The ledger observation record binds the same value through the full
    // validator (live-vocabulary check included).
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let ledger_ids = ledger_id_set(&ledger);
    let mut errs = validate_batch_cache_observation(&ledger.batch_cache_observation, &ledger_ids);
    errs.extend(validate_batch_cache_registry_consistency(
        &ledger.batch_cache_observation,
        &load_registry_value(),
    ));
    assert!(
        errs.is_empty(),
        "batch/cache observation must stay consistent with the rotated fingerprint: {errs:?}"
    );
}

#[test]
fn s07_retired_projection_fingerprint_is_fully_rotated_out_of_live_surfaces() {
    // 1. No data/parse contract may still label the retired identity.
    let parse_dir = repo_root().join("data/parse");
    let mut scanned = 0;
    let mut entries = std::fs::read_dir(&parse_dir)
        .unwrap_or_else(|e| panic!("data/parse readable: {e}"))
        .collect::<Result<Vec<_>, _>>()
        .expect("data/parse entries readable");
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
        scanned += 1;
        assert!(
            !text.contains(RETIRED_PROJECTION_FINGERPRINT),
            "{} still carries the retired projection fingerprint — the S07/T05 rotation is incomplete",
            path.display()
        );
    }
    assert!(
        scanned >= 4,
        "expected the four data/parse contracts, scanned {scanned}"
    );

    // 2. The production surfaces reference the da-parse const instead of
    //    carrying their own (drift-prone) literal copy.
    for rel in [
        "crates/da-cli/src/production.rs",
        "crates/da-adapters/src/pipeline_stages.rs",
        "crates/da-cli/examples/catalog_sweep_runner.rs",
        "crates/da-cli/examples/gold_corpus_canary.rs",
        "crates/da-adapters/tests/pipeline_cache_contract.rs",
    ] {
        let text = String::from_utf8(
            std::fs::read(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel} readable: {e}")),
        )
        .unwrap_or_else(|_| panic!("{rel} must be utf-8"));
        assert!(
            !text.contains(RETIRED_PROJECTION_FINGERPRINT),
            "{rel} still hardcodes the retired projection fingerprint"
        );
    }
}

#[test]
fn s07_reintroducing_the_retired_fingerprint_fails_the_tagged_checks() {
    // Observation record reverting to the retired value → live-vocabulary
    // drift is named by the exact tag.
    let ledger = observation_mut(|obs| {
        if let Some(projector) = obs
            .cache_stages
            .iter_mut()
            .find(|s| s.stage == "project_article")
        {
            projector.projector_schema_fingerprint =
                Some(RETIRED_PROJECTION_FINGERPRINT.to_string());
        }
    });
    let errs = validate_ledger(&ledger);
    negative(
        "[cache-stage-schema-drift]",
        &errs,
        "observation reverted to the retired projection fingerprint",
    );

    // Registry reverting while the observation stays rotated → DATA↔DATA
    // drift is named by the exact tag.
    let (ledger, _) = loaded_pair();
    let mut registry = load_registry_value();
    let section = registry
        .get_mut("defaults")
        .and_then(|d| d.get_mut("batch_cache"))
        .and_then(|b| b.get_mut("cache_stages"))
        .expect("registry cache_stages section");
    if let Some(fp) = section.get_mut("projector_schema_fingerprint") {
        *fp = serde_yaml::Value::String(RETIRED_PROJECTION_FINGERPRINT.into());
    }
    let errs =
        validate_batch_cache_registry_consistency(&ledger.batch_cache_observation, &registry);
    negative(
        "[registry-batch-cache-drift]",
        &errs,
        "registry reverted to the retired projection fingerprint",
    );
}

// ─────────────── S12/T03: native composition DATA gates ───────────────

const NATIVE_COMPOSITION_ID: &str = "parse.component.native_header_reference_composition_shadow";
const NATIVE_COMPOSITION_CANDIDATE: &str = "da-parse.native_header_reference_composition@1.0.0";
const NATIVE_COMPOSITION_ROLLBACK: &str = "java-grobid-http@0.9.0-crf";

fn validate_native_composition_shadow(
    registry: &serde_yaml::Value,
    manifest: &RegressionManifest,
    contract: &serde_yaml::Value,
    ledger: &LedgerFile,
) -> Vec<String> {
    let mut errs = Vec::new();
    let Some(shadow) = contract.get("native_header_reference_composition_shadow") else {
        return vec!["[native-composition-contract-lost] composition contract missing".into()];
    };
    for key in [
        "native_date_shadow",
        "native_personal_name_shadow",
        "native_affiliation_shadow",
        "native_citation_shadow",
    ] {
        if !shadow["component_receipts"]["required"]
            .as_sequence()
            .is_some_and(|xs| xs.iter().any(|x| x.as_str() == Some(key)))
        {
            errs.push(format!(
                "[native-composition-component-missing] `{key}` receipt missing"
            ));
        }
    }
    // These are deliberately independent paths.  A generic component ID is
    // not enough to prove that composition conserved each leaf denominator.
    let required_receipts = [
        ("date", "total", 173_i64),
        ("header", "raw_total", 36),
        ("header", "accepted_total", 35),
        ("header", "rejected_or_incomplete", 1),
        ("header", "strict_n", 0),
        ("header", "soft_initial_n", 35),
        ("citation_names", "total", 2198),
        ("citation_names", "strict_n", 0),
        ("citation_names", "soft_initial_n", 2196),
        ("citation_names", "unsupported_n", 2),
        ("affiliation", "recognition_total", 13),
        ("affiliation", "linking_total", 13),
        ("affiliation", "linked_authors", 11),
        ("affiliation", "zero_link_authors", 25),
        ("affiliation", "exactly_one_three_link", 1),
        ("affiliation", "address_observed_n", 0),
        ("references", "total", 207),
        ("anchors", "total", 380),
        ("anchors", "targeted", 217),
        ("anchors", "targetless", 163),
        ("anchors", "fragmented_adjacency", 168),
    ];
    for (receipt, field, expected) in required_receipts {
        let actual = shadow["component_receipts"][receipt][field].as_i64();
        if actual != Some(expected) {
            errs.push(format!(
                "[native-composition-subreceipt-drift] component_receipts.{receipt}.{field} must stay {expected}, got {actual:?}"
            ));
        }
    }
    if shadow["observation_mode"].as_str() != Some("shadow")
        || shadow["promotion_state"].as_str() != Some("candidate")
        || shadow["auto_promote"].as_str() != Some("forbidden")
    {
        errs.push(
            "[native-composition-policy-drift] shadow/candidate/forbidden policy changed".into(),
        );
    }
    for lane in ["header", "references"] {
        if shadow["rollback"][lane].as_str() != Some(NATIVE_COMPOSITION_ROLLBACK) {
            errs.push(format!(
                "[native-composition-rollback-drift] {lane} rollback changed"
            ));
        }
        if shadow["lanes"][lane]["candidate_state"].as_str() != Some("candidate") {
            errs.push(format!(
                "[native-composition-state-drift] {lane} candidate state changed"
            ));
        }
    }
    let forbidden = shadow["forbidden"]
        .as_sequence()
        .cloned()
        .unwrap_or_default();
    for key in [
        "raw_text",
        "raw_tei",
        "pooled_denominators",
        "invented_stage",
        "auto_promotion",
    ] {
        if !forbidden.iter().any(|x| x.as_str() == Some(key)) {
            errs.push(format!(
                "[native-composition-forbidden-drift] `{key}` missing"
            ));
        }
    }
    for (lane, expected) in [("header", 35_i64), ("references", 207_i64)] {
        let denominator = if lane == "references" {
            shadow["lanes"][lane]["references_denominator"].as_i64()
        } else {
            shadow["lanes"][lane]["denominator"].as_i64()
        };
        if denominator != Some(expected) {
            errs.push(format!(
                "[native-composition-denominator-erosion] {lane} denominator changed"
            ));
        }
        let regressions = shadow["lanes"][lane]["hard_regressions"]
            .as_sequence()
            .cloned()
            .unwrap_or_default();
        for required in if lane == "header" {
            vec!["invented_revision", "nondeterministic_output"]
        } else {
            vec!["lost_references_or_links", "changed_reading_order"]
        } {
            if !regressions.iter().any(|x| x.as_str() == Some(required)) {
                errs.push(format!(
                    "[native-composition-regression-drift] {lane} lost `{required}`"
                ));
            }
        }
    }
    if shadow["lanes"]["references"]["anchors_denominator"].as_i64() != Some(380)
        || shadow["lanes"]["references"]["targeted_anchors"].as_i64() != Some(217)
        || shadow["lanes"]["references"]["targetless_anchors"].as_i64() != Some(163)
        || shadow["lanes"]["references"]["fragmented_adjacency"].as_i64() != Some(168)
    {
        errs.push(
            "[native-composition-denominator-erosion] reference anchor topology changed".into(),
        );
    }
    let ids: HashSet<&str> = ledger.entries.iter().map(|e| e.id.as_str()).collect();
    if !ids.contains(NATIVE_COMPOSITION_ID) {
        errs.push(
            "[native-composition-ledger-binding-missing] shadow component entry missing".into(),
        );
    }
    for stage in ["stage.header_composition", "stage.reference_segmentation"] {
        let Some(cap) = registry["capabilities"]
            .as_sequence()
            .and_then(|xs| xs.iter().find(|x| x["id"].as_str() == Some(stage)))
        else {
            errs.push(format!(
                "[native-composition-stage-missing] `{stage}` missing"
            ));
            continue;
        };
        if cap["candidate"].as_str()
            != Some(&format!(
                "{NATIVE_COMPOSITION_CANDIDATE}:{}",
                if stage.ends_with("header_composition") {
                    "header"
                } else {
                    "references"
                }
            ))
            || cap["baseline"].as_str() != Some(NATIVE_COMPOSITION_ROLLBACK)
            || cap["rollback_target"].as_str() != Some(NATIVE_COMPOSITION_ROLLBACK)
        {
            errs.push(format!("[native-composition-stage-binding-drift] `{stage}` must bind versioned candidate and Java rollback"));
        }
    }
    for case in manifest
        .cases
        .iter()
        .filter(|c| c.binding_status == BindingStatus::Bound)
    {
        if !case
            .required_evidence_fields
            .iter()
            .any(|x| x == "native_header_reference_composition_shadow")
            || !case
                .ledger_bindings
                .iter()
                .any(|x| x == NATIVE_COMPOSITION_ID)
        {
            errs.push(format!(
                "[native-composition-obligation-missing] case `{}`",
                case.id
            ));
        }
    }
    errs
}

#[test]
fn native_composition_shadow_data_contracts_hold() {
    let registry = load_registry_value();
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let contract: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let errs = validate_native_composition_shadow(&registry, &manifest, &contract, &ledger);
    assert!(
        errs.is_empty(),
        "native composition DATA violations:\n{}",
        errs.join("\n")
    );
}

#[test]
fn rejects_native_composition_promotion_and_denominator_pooling() {
    let registry = load_registry_value();
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let mut contract: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
    contract["native_header_reference_composition_shadow"]["promotion_state"] =
        serde_yaml::Value::String("promoted".into());
    contract["native_header_reference_composition_shadow"]["forbidden"] =
        serde_yaml::Value::Sequence(vec![]);
    let errs = validate_native_composition_shadow(&registry, &manifest, &contract, &ledger);
    negative(
        "[native-composition-policy-drift]",
        &errs,
        "promotion and forbidden policy erosion",
    );
    negative(
        "[native-composition-forbidden-drift]",
        &errs,
        "pooled denominator/raw content guard",
    );
}

#[test]
fn rejects_native_composition_subreceipt_removal_and_change() {
    let registry = load_registry_value();
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let receipts = [
        ("date", "total", 173_i64),
        ("header", "raw_total", 36),
        ("header", "accepted_total", 35),
        ("header", "rejected_or_incomplete", 1),
        ("header", "strict_n", 0),
        ("header", "soft_initial_n", 35),
        ("citation_names", "total", 2198),
        ("citation_names", "strict_n", 0),
        ("citation_names", "soft_initial_n", 2196),
        ("citation_names", "unsupported_n", 2),
        ("affiliation", "recognition_total", 13),
        ("affiliation", "linking_total", 13),
        ("affiliation", "linked_authors", 11),
        ("affiliation", "zero_link_authors", 25),
        ("affiliation", "exactly_one_three_link", 1),
        ("affiliation", "address_observed_n", 0),
        ("references", "total", 207),
        ("anchors", "total", 380),
        ("anchors", "targeted", 217),
        ("anchors", "targetless", 163),
        ("anchors", "fragmented_adjacency", 168),
    ];

    for (receipt, field, expected) in receipts {
        let mut removed: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
        removed["native_header_reference_composition_shadow"]["component_receipts"][receipt]
            [field] = serde_yaml::Value::Null;
        let errs = validate_native_composition_shadow(&registry, &manifest, &removed, &ledger);
        negative(
            "[native-composition-subreceipt-drift]",
            &errs,
            &format!("removed component_receipts.{receipt}.{field}"),
        );

        let mut changed: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
        changed["native_header_reference_composition_shadow"]["component_receipts"][receipt]
            [field] = serde_yaml::Value::Number((expected + 1).into());
        let errs = validate_native_composition_shadow(&registry, &manifest, &changed, &ledger);
        negative(
            "[native-composition-subreceipt-drift]",
            &errs,
            &format!("changed component_receipts.{receipt}.{field}"),
        );
    }

    // A pooled citation/header denominator is not an acceptable substitute
    // for the separately named receipts above.
    let mut pooled: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
    pooled["native_header_reference_composition_shadow"]["component_receipts"]["citation_names"]
        ["total"] = serde_yaml::Value::Number(35.into());
    let errs = validate_native_composition_shadow(&registry, &manifest, &pooled, &ledger);
    negative(
        "[native-composition-subreceipt-drift]",
        &errs,
        "pooled citation names into accepted header denominator",
    );
}

// ─────────────── S08/T03: native date shadow DATA gates ───────────────

/// Versioned Rust shadow candidate identity (must equal the LIVE da-parse
/// constants; data/parse/comparison_contract.yaml `native_date_shadow`).
const NATIVE_DATE_CANDIDATE_ID: &str = da_parse::NATIVE_DATE_PARSER_ID;
const NATIVE_DATE_CANDIDATE_VERSION: &str = da_parse::NATIVE_DATE_PARSER_VERSION;
const NATIVE_DATE_OUTCOME_SCHEMA: &str = da_parse::NATIVE_DATE_OUTCOME_SCHEMA_VERSION;
const NATIVE_DATE_ENGINE: &str = "da_parse::NativeDateParser";
const NATIVE_DATE_ROLLBACK: &str = "java-grobid-http@0.9.0-crf";
/// Checksum-bound occurrence denominator pinned by the comparison contract.
const NATIVE_DATE_DENOMINATOR_TOTAL: i64 = 173;

fn validate_native_date_shadow(
    registry: &serde_yaml::Value,
    manifest: &RegressionManifest,
    contract: &serde_yaml::Value,
) -> Vec<String> {
    let mut errs = Vec::new();
    let cap = registry
        .get("capabilities")
        .and_then(|c| c.as_sequence())
        .and_then(|seq| {
            seq.iter()
                .find(|c| c.get("id").and_then(|i| i.as_str()) == Some("rest.processDate"))
        });
    let Some(cap) = cap else {
        errs.push("[native-date-capability-lost] registry lost `rest.processDate`".into());
        return errs;
    };
    // Honest state: shadow + candidate promotion + Java rollback preserved.
    if cap.get("state").and_then(|s| s.as_str()) != Some("shadow") {
        errs.push("[native-date-state-drift] rest.processDate must stay `shadow`".into());
    }
    if cap.get("promotion_state").and_then(|s| s.as_str()) != Some("candidate") {
        errs.push(
            "[native-date-state-drift] rest.processDate promotion_state must stay `candidate"
                .into(),
        );
    }
    let candidate = cap
        .get("candidate")
        .and_then(|c| c.as_str())
        .unwrap_or_default();
    let expected_candidate = format!("{NATIVE_DATE_CANDIDATE_ID}@{NATIVE_DATE_CANDIDATE_VERSION}");
    if candidate != expected_candidate {
        errs.push(format!(
            "[native-date-candidate-drift] rest.processDate candidate `{candidate}` != versioned `{expected_candidate}`"
        ));
    }
    for key in ["baseline", "rollback_target"] {
        if cap.get(key).and_then(|s| s.as_str()) != Some(NATIVE_DATE_ROLLBACK) {
            errs.push(format!(
                "[native-date-rollback-drift] rest.processDate {key} must stay `{NATIVE_DATE_ROLLBACK}`"
            ));
        }
    }
    let regressions: Vec<String> = cap
        .get("hard_regressions")
        .and_then(|h| h.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    for required in ["invented_revision", "nondeterministic_output"] {
        if !regressions.contains(&required.to_string()) {
            errs.push(format!(
                "[native-date-regression-drift] rest.processDate hard_regressions lost `{required}`"
            ));
        }
    }
    let official_fields: Vec<String> = cap
        .get("official")
        .and_then(|o| o.get("request_fields"))
        .and_then(|f| f.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|f| f.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if official_fields != vec!["date".to_string()] {
        errs.push(format!(
            "[native-date-field-drift] rest.processDate official request fields {official_fields:?} must stay the closed single-field [\"date\"] allowlist"
        ));
    }

    // Contract terms: shadow mode, zero-invention budget, pinned denominator.
    let Some(shadow) = contract.get("native_date_shadow") else {
        errs.push(
            "[native-date-contract-lost] comparison contract lost `native_date_shadow".into(),
        );
        return errs;
    };
    if shadow.get("engine").and_then(|s| s.as_str()) != Some(NATIVE_DATE_ENGINE) {
        errs.push(
            "[native-date-engine-drift] contract engine must stay `da_parse::NativeDateParser"
                .into(),
        );
    }
    if shadow.get("candidate_id").and_then(|s| s.as_str()) != Some(NATIVE_DATE_CANDIDATE_ID) {
        errs.push(
            "[native-date-candidate-drift] contract candidate_id diverges from the live constant"
                .into(),
        );
    }
    if shadow.get("candidate_version").and_then(|s| s.as_str())
        != Some(NATIVE_DATE_CANDIDATE_VERSION)
    {
        errs.push(
            "[native-date-candidate-drift] contract candidate_version diverges from the live constant"
                .into(),
        );
    }
    if shadow
        .get("outcome_schema_version")
        .and_then(|s| s.as_str())
        != Some(NATIVE_DATE_OUTCOME_SCHEMA)
    {
        errs.push(
            "[native-date-schema-drift] contract outcome_schema_version diverges from the live constant"
                .into(),
        );
    }
    if shadow.get("observation_mode").and_then(|s| s.as_str()) != Some("shadow") {
        errs.push(
            "[native-date-state-drift] contract observation_mode must stay `shadow` (never a runtime route)"
                .into(),
        );
    }
    if shadow.get("promotion_state").and_then(|s| s.as_str()) != Some("candidate") {
        errs.push(
            "[native-date-state-drift] contract promotion_state must stay `candidate` (auto-promotion forbidden)"
                .into(),
        );
    }
    if shadow.get("rollback_target").and_then(|s| s.as_str()) != Some(NATIVE_DATE_ROLLBACK) {
        errs.push(format!(
            "[native-date-rollback-drift] contract rollback_target must stay `{NATIVE_DATE_ROLLBACK}`"
        ));
    }
    let denominator = shadow.get("denominator");
    if denominator
        .and_then(|d| d.get("total"))
        .and_then(|t| t.as_i64())
        != Some(NATIVE_DATE_DENOMINATOR_TOTAL)
    {
        errs.push(
            "[native-date-denominator-erosion] contract denominator.total must stay 173".into(),
        );
    }
    let mut per_fixture: Vec<(String, i64)> = denominator
        .and_then(|d| d.get("per_fixture"))
        .and_then(|p| p.as_mapping())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| Some((k.as_str()?.to_string(), v.as_i64()?)))
                .collect()
        })
        .unwrap_or_default();
    per_fixture.sort();
    let expected_split = vec![
        ("reg.tei.hub-bibliography-2507.19457".to_string(), 80),
        ("reg.tei.reca-bibliography-2605.26525".to_string(), 43),
        ("reg.tei.rlm-2512.24601".to_string(), 50),
    ];
    if per_fixture != expected_split {
        errs.push(format!(
            "[native-date-denominator-erosion] per-fixture denominator {per_fixture:?} must stay the pinned split 80 + 50 + 43"
        ));
    }
    let semantics = shadow.get("semantics");
    if semantics
        .and_then(|s| s.get("same_n"))
        .and_then(|s| s.as_str())
        != Some("required")
    {
        errs.push("[native-date-same-n-drift] semantics.same_n must stay `required".into());
    }
    if semantics
        .and_then(|s| s.get("invented_component_budget"))
        .and_then(|b| b.as_i64())
        != Some(0)
    {
        errs.push(
            "[native-date-invention-budget] semantics.invented_component_budget must stay 0".into(),
        );
    }
    let forbidden: Vec<String> = shadow
        .get("forbidden")
        .and_then(|f| f.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    for required in [
        "auto_promotion",
        "runtime_routing",
        "raw_display_text",
        "tei_fragments",
        "invented_bindings",
    ] {
        if !forbidden.contains(&required.to_string()) {
            errs.push(format!(
                "[native-date-forbidden-drift] contract forbidden list lost `{required}`"
            ));
        }
    }

    // Every bound case carries the date obligation (registry capability,
    // observable surface and shadow evidence field).
    for case in manifest
        .cases
        .iter()
        .filter(|c| c.binding_status == BindingStatus::Bound)
    {
        for binding in ["rest.processDate", "parse.component.native_date_shadow"] {
            if !case.ledger_bindings.iter().any(|b| b == binding) {
                errs.push(format!(
                    "[native-date-obligation-missing] case `{}` lost the `{binding}` binding",
                    case.id
                ));
            }
        }
        if !case
            .required_evidence_fields
            .iter()
            .any(|f| f == "native_date_shadow")
        {
            errs.push(format!(
                "[native-date-obligation-missing] case `{}` lost the `native_date_shadow` evidence field",
                case.id
            ));
        }
    }
    errs
}

#[test]
fn native_date_shadow_data_contracts_hold() {
    let registry = load_registry_value();
    let contract: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let errs = validate_native_date_shadow(&registry, &manifest, &contract);
    assert!(
        errs.is_empty(),
        "native date shadow DATA violations:\n{}",
        errs.join("\n")
    );
    // The ledger carries the shadow component entry and cross-binds the
    // registry capability.
    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let entry = ledger
        .entries
        .iter()
        .find(|e| e.id == "parse.component.native_date_shadow")
        .expect("shadow component entry present");
    assert_eq!(entry.owner_crate, "da-parse");
    assert!(matches!(entry.kind, EntryKind::ShadowComponent));
    assert!(
        entry
            .compatibility_obligation
            .contains("java-grobid-http@0.9.0-crf")
    );
    let note_errs = validate_registry_binding_notes(&ledger, &load_registry_ids());
    assert!(
        note_errs.is_empty(),
        "shadow registry_binding note must resolve: {note_errs:?}"
    );
}

#[test]
fn rejects_native_date_state_promotion() {
    let mut registry = load_registry_value();
    if let Some(caps) = registry
        .get_mut("capabilities")
        .and_then(|c| c.as_sequence_mut())
    {
        for cap in caps.iter_mut() {
            if cap.get("id").and_then(|i| i.as_str()) == Some("rest.processDate") {
                *cap.get_mut("state").expect("state key") =
                    serde_yaml::Value::String("promoted".into());
            }
        }
    }
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let contract: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
    let errs = validate_native_date_shadow(&registry, &manifest, &contract);
    negative(
        "[native-date-state-drift]",
        &errs,
        "registry state promotion",
    );
}

#[test]
fn rejects_native_date_candidate_erasure() {
    let mut registry = load_registry_value();
    if let Some(caps) = registry
        .get_mut("capabilities")
        .and_then(|c| c.as_sequence_mut())
    {
        for cap in caps.iter_mut() {
            if cap.get("id").and_then(|i| i.as_str()) == Some("rest.processDate") {
                *cap.get_mut("candidate").expect("candidate key") =
                    serde_yaml::Value::String("none".into());
            }
        }
    }
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let contract: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
    let errs = validate_native_date_shadow(&registry, &manifest, &contract);
    negative("[native-date-candidate-drift]", &errs, "candidate erasure");
}

#[test]
fn rejects_native_date_denominator_erosion() {
    let registry = load_registry_value();
    let mut contract: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
    contract["native_date_shadow"]["denominator"]["total"] = serde_yaml::Value::Number(172.into());
    let manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let errs = validate_native_date_shadow(&registry, &manifest, &contract);
    negative(
        "[native-date-denominator-erosion]",
        &errs,
        "denominator erosion",
    );
}

#[test]
fn rejects_native_date_obligation_removal_and_invented_binding() {
    let registry = load_registry_value();
    let contract: serde_yaml::Value = load_yaml(COMPARISON_CONTRACT_REL).expect("contract");
    let mut manifest: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    let idx = manifest
        .cases
        .iter()
        .position(|c| c.binding_status == BindingStatus::Bound)
        .expect("bound case");
    manifest.cases[idx]
        .ledger_bindings
        .retain(|b| b != "rest.processDate");
    manifest.cases[idx]
        .required_evidence_fields
        .retain(|f| f != "native_date_shadow");
    let errs = validate_native_date_shadow(&registry, &manifest, &contract);
    negative(
        "[native-date-obligation-missing]",
        &errs,
        "date obligation stripped",
    );
    // An invented date binding never resolves (R117 referential integrity).
    let (ledger, _) = loaded_pair();
    let mut m2: RegressionManifest = expect_loaded(REGRESSION_REL_PATH);
    m2.cases[idx]
        .ledger_bindings
        .push("rest.processDateV2".to_string());
    let refs = refs_from(&ledger);
    let errs2 = validate_manifest(&m2, &refs);
    negative("[unresolved-ledger-ref]", &errs2, "invented date binding");
}
