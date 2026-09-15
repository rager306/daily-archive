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
    rest_batch_asset_quarantine: RestBatchAssetQuarantine,
    batch_cache_observation: BatchCacheObservation,
    entries: Vec<LedgerEntry>,
}

/// S07/T04 additive REST asset/batch quarantine observation. It stays an
/// observation block (not a second capability ledger): zero new endpoints are
/// implemented and the production route is forbidden.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestBatchAssetQuarantine {
    source_artifacts: Vec<String>,
    sanctioned_endpoints: Vec<String>,
    local_batch_carrier: String,
    implemented_new_endpoints: u32,
    product_gap: String,
    production_route: String,
    graph_write: String,
    import_eligible: bool,
    auto_promote: String,
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
    identity_sha256: Option<String>,
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

/// Shadow-population honesty contract (S20): which blocks every shadow
/// report must carry, which legacy blocks are forbidden, and which fields an
/// *executed* population row must prove. `deny_unknown_fields` keeps the
/// vocabulary closed.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PopulationContract {
    schema_version: String,
    report_paths: Vec<String>,
    required_blocks: Vec<String>,
    forbidden_legacy_blocks: Vec<String>,
    executed_population_required_fields: Vec<String>,
    synthetic_corpus_execution: bool,
    quality_requires_evaluated_status: String,
}

/// Read-only external-holdout byte policy: external bytes are never loaded.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalHoldoutBytePolicy {
    external_bytes: String,
    load_external_bytes: String,
    download_external_bytes: String,
    materialize_external_files: String,
    external_rows_are_evidence: bool,
}

/// Identity binding of external rows (digest of canonical identity, never a
/// byte hash).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalHoldoutIdentityBinding {
    digest: String,
    input: String,
    digest_is_byte_hash: bool,
    digest_is_authority_binding: bool,
    mismatch_behavior: String,
}

/// Denominator policy: only local bound rows count toward the local proxy.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalHoldoutDenominatorPolicy {
    local_bound_rows_only: bool,
    bytes_absent_rows_in_local_denominator: bool,
    catalogued_rows_in_local_denominator: bool,
    external_probe_changes_local_denominator: String,
    same_n_local_proxy_required: bool,
}

/// Document-level reporting shape required before any execution claim.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalHoldoutDocumentReporting {
    unit: String,
    uncertainty_method: String,
    report_required_before_execution_claim: bool,
    fields: Vec<String>,
}

/// Bounded cost envelope with an explicit "unknown is not zero" lock.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalHoldoutCostBounds {
    currency: String,
    basis: String,
    lower_bound_includes: Vec<String>,
    upper_bound_includes: Vec<String>,
    unknown_cost_is_not_zero: bool,
    release_claim_allowed: bool,
}

/// Additive S20 external-holdout contract (advisory-only transferability
/// probe; no external bytes, no release claim).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalHoldoutContract {
    schema_version: String,
    purpose: String,
    execution_status: String,
    byte_policy: ExternalHoldoutBytePolicy,
    identity_binding: ExternalHoldoutIdentityBinding,
    denominator_policy: ExternalHoldoutDenominatorPolicy,
    document_level_reporting: ExternalHoldoutDocumentReporting,
    cost_bounds: ExternalHoldoutCostBounds,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseQualityEvidence {
    acceptance_manifest: String,
    local_proxy_contract: String,
    external_holdout_status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseQualityClaims {
    same_n_local_proxy: bool,
    document_level_uncertainty: String,
    external_bytes_absent: bool,
    pooled_external_quality_claim: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseQualityGate {
    status: String,
    evidence: ReleaseQualityEvidence,
    required_claims: ReleaseQualityClaims,
    release_effect: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseCostEvidence {
    cost_bounds: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseCostGate {
    status: String,
    evidence: ReleaseCostEvidence,
    required_fields: Vec<String>,
    unknown_cost_is_not_zero: bool,
    release_effect: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseRightsEvidence {
    review_manifest: String,
    corrective_register: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseRightsClaims {
    component_license_provenance: String,
    redistribution_permission: String,
    model_distribution_permission: String,
    external_dataset_permission: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseRightsGate {
    status: String,
    evidence: ReleaseRightsEvidence,
    required_claims: ReleaseRightsClaims,
    release_effect: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseProhibitedAuthorizations {
    redistribution: String,
    production_use: String,
    model_distribution: String,
    external_downloads: String,
    graph_write_or_import: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HumanReleaseReview {
    required: bool,
    reviewer: Option<String>,
    decision: String,
    decision_record: String,
    prose_alone_authorizes_release: bool,
    authorization_requires: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseInvalidation {
    on_source_revision_change: String,
    on_manifest_identity_change: String,
    on_rights_evidence_change: String,
}

/// S20 release-decision package: bounded evaluation/review only; it never
/// authorizes production, redistribution, redistribution, or downloads.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseDecisionPackage {
    schema_version: String,
    purpose: String,
    package_status: String,
    evidence_revision: String,
    required_gates: Vec<String>,
    quality: ReleaseQualityGate,
    cost: ReleaseCostGate,
    rights: ReleaseRightsGate,
    prohibited_authorizations: ReleaseProhibitedAuthorizations,
    human_release_review: HumanReleaseReview,
    invalidation: ReleaseInvalidation,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceManifest {
    schema_version: String,
    adr_ref: String,
    manifest_id: String,
    requirement_ids: Vec<String>,
    policy_locks: AcceptancePolicyLocks,
    population_contract: PopulationContract,
    comparison_rules: AcceptanceRules,
    external_holdout_contract: ExternalHoldoutContract,
    release_decision_package: ReleaseDecisionPackage,
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

fn load_json_value(rel: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("JSON artifact must be readable: {rel}: {e}"));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("JSON artifact must be valid: {rel}: {e}"))
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
    errs.extend(validate_rest_batch_asset_quarantine(
        &l.rest_batch_asset_quarantine,
    ));
    errs
}

/// S07/T04: the ledger's REST asset/batch block is an observation, not a
/// second capability ledger. It must keep zero new endpoints, the forbidden
/// production route, and physically present source artifacts.
fn validate_rest_batch_asset_quarantine(q: &RestBatchAssetQuarantine) -> Vec<String> {
    let mut errs = Vec::new();
    if q.production_route != "forbidden"
        || q.graph_write != "forbidden"
        || q.auto_promote != "forbidden"
        || q.import_eligible
    {
        errs.push("[rest-batch-asset-quarantine-lock] quarantine locks eroded".into());
    }
    if q.implemented_new_endpoints != 0 {
        errs.push(format!(
            "[rest-batch-asset-quarantine-implementation] expected 0 new endpoints, got {}",
            q.implemented_new_endpoints
        ));
    }
    if q.product_gap != "none_independently_evidenced" {
        errs.push(format!(
            "[rest-batch-asset-quarantine-product-gap] unrecognized product_gap `{}`",
            q.product_gap
        ));
    }
    if q.local_batch_carrier != "parse.batch.local_composition" {
        errs.push(format!(
            "[rest-batch-asset-quarantine-carrier] local_batch_carrier `{}`",
            q.local_batch_carrier
        ));
    }
    let expected_endpoints = [
        "parse.endpoint.process_fulltext_document",
        "parse.endpoint.process_header_document",
        "parse.endpoint.process_references_document",
    ];
    if q.sanctioned_endpoints != expected_endpoints {
        errs.push(format!(
            "[rest-batch-asset-quarantine-endpoints] sanctioned endpoints drifted: {:?}",
            q.sanctioned_endpoints
        ));
    }
    if q.source_artifacts.is_empty() {
        errs.push("[rest-batch-asset-quarantine-sources] no source artifacts declared".into());
    }
    for rel in &q.source_artifacts {
        if !repo_root().join(rel).is_file() {
            errs.push(format!(
                "[rest-batch-asset-quarantine-sources] missing source artifact `{rel}`"
            ));
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
    const SANCTIONED: [&str; 23] = [
        "SCHEMA.yaml",
        "acceptance_manifest.yaml",
        "api_compatibility_ledger.yaml",
        "callout_matching_semantics.yaml",
        "comparison_contract.yaml",
        "consolidation_specialized_semantics.yaml",
        "document_segmentation.yaml",
        "external_holdout_results.yaml",
        "feature_tagger_training_parity.yaml",
        "fulltext_block_semantics.yaml",
        "grobid_compatibility.yaml",
        "grobid_model_rights.yaml",
        "holdout_manifests.yaml",
        "layout_bakeoff.yaml",
        "layout_lattice_parity.yaml",
        "license_inventory.yaml",
        "native_body_accounting.yaml",
        "native_fulltext_composition.yaml",
        "native_visual_heads.yaml",
        "p5_cross_stage.yaml",
        "p5_shadow_join.yaml",
        "regression_cases.yaml",
        "rest_batch_asset_parity.yaml",
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
            if row.state == "bytes_absent" {
                match row.identity_sha256.as_deref() {
                    Some(sha) if acceptance_sha256(sha) => {}
                    _ => errs.push(format!("[acceptance-remote-identity-sha256-missing] {ctx}")),
                }
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
    errs.extend(validate_population_contract(&m.population_contract));
    errs.extend(validate_external_holdout_contract(
        &m.external_holdout_contract,
    ));
    errs.extend(validate_release_decision_package(
        &m.release_decision_package,
    ));
    errs
}

/// S20 shadow-population honesty: closed required/forbidden block vocabulary
/// and an explicit "no synthetic corpus execution" lock.
fn validate_population_contract(p: &PopulationContract) -> Vec<String> {
    let mut errs = Vec::new();
    if p.schema_version != "shadow-populations.v1" {
        errs.push("[population-contract-schema] expected shadow-populations.v1".into());
    }
    let expected_required = [
        "reference_population",
        "executed_population",
        "evaluated_population",
        "outcomes",
    ];
    if p.required_blocks != expected_required {
        errs.push("[population-contract-blocks] required shadow blocks drifted".into());
    }
    let expected_forbidden = ["denominators", "corpus"];
    if p.forbidden_legacy_blocks != expected_forbidden {
        errs.push("[population-contract-legacy-blocks] forbidden legacy blocks drifted".into());
    }
    let expected_executed = ["corpus_execution", "execution_evidence"];
    if p.executed_population_required_fields != expected_executed {
        errs.push(
            "[population-contract-executed-fields] executed-population fields drifted".into(),
        );
    }
    if p.synthetic_corpus_execution {
        errs.push(
            "[population-contract-synthetic] synthetic corpus execution must stay forbidden".into(),
        );
    }
    if p.quality_requires_evaluated_status != "evaluated" {
        errs.push("[population-contract-status] quality must require evaluated status".into());
    }
    if p.report_paths.is_empty() {
        errs.push("[population-contract-reports] no shadow reports declared".into());
    }
    for rel in &p.report_paths {
        if !repo_root().join(rel).is_file() {
            errs.push(format!(
                "[population-contract-reports] missing shadow report `{rel}`"
            ));
        }
    }
    errs
}

/// S20 external holdout is an advisory transferability probe: no external
/// bytes are ever loaded, no pooled denominator, no release claim.
fn validate_external_holdout_contract(e: &ExternalHoldoutContract) -> Vec<String> {
    let mut errs = Vec::new();
    if e.schema_version != "external-holdout-readonly.v1" {
        errs.push("[external-holdout-schema] expected external-holdout-readonly.v1".into());
    }
    if e.purpose != "transferability_and_cost_probe_only" {
        errs.push("[external-holdout-purpose] purpose drifted".into());
    }
    if e.execution_status != "not_executed" {
        errs.push("[external-holdout-status] external holdout must stay not_executed".into());
    }
    let b = &e.byte_policy;
    if b.external_bytes != "bytes_absent"
        || b.load_external_bytes != "forbidden"
        || b.download_external_bytes != "forbidden"
        || b.materialize_external_files != "forbidden"
        || b.external_rows_are_evidence
    {
        errs.push("[external-holdout-bytes] external byte policy eroded".into());
    }
    let i = &e.identity_binding;
    if i.digest != "sha256"
        || i.input != "canonical_dataset_id_plus_identity"
        || i.digest_is_byte_hash
        || !i.digest_is_authority_binding
        || i.mismatch_behavior != "fail_visible"
    {
        errs.push("[external-holdout-identity] identity binding drifted".into());
    }
    let d = &e.denominator_policy;
    if !d.local_bound_rows_only
        || d.bytes_absent_rows_in_local_denominator
        || d.catalogued_rows_in_local_denominator
        || d.external_probe_changes_local_denominator != "forbidden"
        || !d.same_n_local_proxy_required
    {
        errs.push("[external-holdout-denominator] denominator policy eroded".into());
    }
    let r = &e.document_level_reporting;
    if r.unit != "document"
        || r.uncertainty_method.is_empty()
        || !r.report_required_before_execution_claim
        || r.fields.is_empty()
    {
        errs.push("[external-holdout-reporting] document-level reporting drifted".into());
    }
    let c = &e.cost_bounds;
    if c.currency != "USD"
        || c.basis.is_empty()
        || c.lower_bound_includes.is_empty()
        || c.upper_bound_includes.is_empty()
        || !c.unknown_cost_is_not_zero
        || c.release_claim_allowed
    {
        errs.push("[external-holdout-cost] cost bounds eroded".into());
    }
    errs
}

/// S20 release-decision package stays a bounded review artifact: every gate
/// is held, no authorization is granted, and a named human decision is still
/// required.
fn validate_release_decision_package(p: &ReleaseDecisionPackage) -> Vec<String> {
    let mut errs = Vec::new();
    if p.schema_version != "release-decision-package.v1" {
        errs.push("[release-decision-schema] expected release-decision-package.v1".into());
    }
    if p.purpose != "bounded_evaluation_and_release_review_only"
        || !p.evidence_revision.starts_with("git:")
    {
        errs.push("[release-decision-identity] purpose or evidence revision drifted".into());
    }
    if p.package_status != "pending_human_review" {
        errs.push("[release-decision-status] package must stay pending_human_review".into());
    }
    let expected_gates = ["quality", "cost", "rights"];
    if p.required_gates != expected_gates {
        errs.push("[release-decision-gates] required gates drifted".into());
    }
    let q = &p.quality;
    if q.status != "blocked"
        || q.release_effect != "does_not_authorize_production_use"
        || !q.required_claims.same_n_local_proxy
        || q.required_claims.document_level_uncertainty != "required"
        || !q.required_claims.external_bytes_absent
        || q.required_claims.pooled_external_quality_claim != "forbidden"
    {
        errs.push("[release-decision-quality] quality gate claims eroded".into());
    }
    if !repo_root().join(&q.evidence.acceptance_manifest).is_file()
        || !repo_root().join(&q.evidence.local_proxy_contract).is_file()
    {
        errs.push("[release-decision-quality] quality evidence references missing files".into());
    }
    if q.evidence.external_holdout_status != "not_executed" {
        errs.push("[release-decision-quality] external holdout status drifted".into());
    }
    let c = &p.cost;
    let expected_cost_fields = [
        "currency",
        "cost_lower_bound",
        "cost_upper_bound",
        "unknown_cost_is_not_zero",
    ];
    if c.status != "bounded_not_measured"
        || c.required_fields != expected_cost_fields
        || !c.unknown_cost_is_not_zero
        || c.release_effect != "does_not_authorize_external_downloads"
    {
        errs.push("[release-decision-cost] cost gate eroded".into());
    }
    if c.evidence.cost_bounds
        != "data/parse/acceptance_manifest.yaml#external_holdout_contract.cost_bounds"
    {
        errs.push("[release-decision-cost] cost evidence reference drifted".into());
    }
    let rights = &p.rights;
    if rights.status != "blocked"
        || rights.release_effect != "does_not_authorize_redistribution"
        || rights.required_claims.component_license_provenance != "required"
        || rights.required_claims.redistribution_permission != "not_established"
        || rights.required_claims.model_distribution_permission != "not_established"
        || rights.required_claims.external_dataset_permission != "not_established"
    {
        errs.push("[release-decision-rights] rights gate eroded".into());
    }
    if !repo_root().join(&rights.evidence.review_manifest).is_file()
        || !repo_root()
            .join(&rights.evidence.corrective_register)
            .is_file()
    {
        errs.push("[release-decision-rights] rights evidence references missing files".into());
    }
    let a = &p.prohibited_authorizations;
    if a.redistribution != "blocked"
        || a.production_use != "blocked"
        || a.model_distribution != "blocked"
        || a.external_downloads != "blocked"
        || a.graph_write_or_import != "blocked"
    {
        errs.push("[release-decision-authorizations] prohibited authorizations eroded".into());
    }
    let h = &p.human_release_review;
    let expected_auth = [
        "named_reviewer",
        "explicit_decision",
        "passing_quality_gate",
        "passing_cost_gate",
        "passing_rights_gate",
    ];
    if !h.required
        || h.reviewer.is_some()
        || h.decision != "pending"
        || h.prose_alone_authorizes_release
        || h.authorization_requires != expected_auth
    {
        errs.push("[release-decision-human-review] human release review drifted".into());
    }
    if !repo_root().join(&h.decision_record).is_file() {
        errs.push("[release-decision-human-review] decision record missing".into());
    }
    let inv = &p.invalidation;
    if inv.on_source_revision_change != "required"
        || inv.on_manifest_identity_change != "required"
        || inv.on_rights_evidence_change != "required"
    {
        errs.push("[release-decision-invalidation] invalidation triggers drifted".into());
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
fn additive_s20_sections_are_declared_and_locked() {
    let manifest: AcceptanceManifest = expect_loaded(ACCEPTANCE_REL_PATH);
    let population = &manifest.population_contract;
    assert_eq!(population.schema_version, "shadow-populations.v1");
    assert_eq!(
        population.required_blocks,
        [
            "reference_population",
            "executed_population",
            "evaluated_population",
            "outcomes"
        ]
    );
    assert!(!population.report_paths.is_empty());
    assert!(!population.synthetic_corpus_execution);
    assert_eq!(population.quality_requires_evaluated_status, "evaluated");

    let holdout = &manifest.external_holdout_contract;
    assert_eq!(holdout.schema_version, "external-holdout-readonly.v1");
    assert_eq!(holdout.execution_status, "not_executed");
    assert_eq!(holdout.byte_policy.external_bytes, "bytes_absent");
    assert!(!holdout.byte_policy.external_rows_are_evidence);
    assert_eq!(
        holdout
            .denominator_policy
            .external_probe_changes_local_denominator,
        "forbidden"
    );
    assert!(holdout.cost_bounds.unknown_cost_is_not_zero);

    let release = &manifest.release_decision_package;
    assert_eq!(release.schema_version, "release-decision-package.v1");
    assert_eq!(release.package_status, "pending_human_review");
    assert!(release.human_release_review.required);
    assert!(release.human_release_review.reviewer.is_none());
    assert!(!release.human_release_review.prose_alone_authorizes_release);
    assert_eq!(release.prohibited_authorizations.production_use, "blocked");

    let ledger: LedgerFile = expect_loaded(LEDGER_REL_PATH);
    let quarantine = &ledger.rest_batch_asset_quarantine;
    assert_eq!(quarantine.production_route, "forbidden");
    assert_eq!(quarantine.graph_write, "forbidden");
    assert_eq!(quarantine.auto_promote, "forbidden");
    assert!(!quarantine.import_eligible);
    assert_eq!(quarantine.implemented_new_endpoints, 0);
    assert_eq!(quarantine.product_gap, "none_independently_evidenced");
    assert!(!quarantine.source_artifacts.is_empty());
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

// ───── S06/T02: hub 20 vs Java 21 stays a named DATA gap, never pooled or composed ─────
//
// The Hub 2507.19457 header gap — native accepted 20 vs Java header_raw 21 —
// is a NAMED receipt pair in DATA (`java_lift_same_n`), not a per-article
// count and not a poolable denominator. This validator reads the live
// `data/parse/comparison_contract.yaml` block and pins:
// - hub_native_accepted_header == 20, hub_java_header_raw == 21;
// - java_header_authors_equals == header_raw_total (Java truth, not native);
// - both pooling keys == forbidden;
// - no compose/pooling vocabulary substituted into the named keys.

fn validate_java_lift_same_n_gap(contract: &serde_yaml::Value) -> Vec<String> {
    let mut errs = Vec::new();
    let Some(gap) = contract
        .get("native_fulltext_composition_shadow")
        .and_then(|v| v.get("source_evidence"))
        .and_then(|v| v.get("s12_named_denominators"))
        .and_then(|v| v.get("java_lift_same_n"))
    else {
        return vec!["[hub-header-gap-lost] java_lift_same_n block missing".into()];
    };
    if gap
        .get("hub_native_accepted_header")
        .and_then(|v| v.as_i64())
        != Some(20)
    {
        errs.push(format!(
            "[hub-header-gap-drift] hub_native_accepted_header must stay 20, got {:?}",
            gap.get("hub_native_accepted_header")
        ));
    }
    if gap.get("hub_java_header_raw").and_then(|v| v.as_i64()) != Some(21) {
        errs.push(format!(
            "[hub-header-gap-drift] hub_java_header_raw must stay 21, got {:?}",
            gap.get("hub_java_header_raw")
        ));
    }
    if gap
        .get("java_header_authors_equals")
        .and_then(|v| v.as_str())
        != Some("header_raw_total")
    {
        errs.push(
            "[hub-header-gap-drift] java_header_authors_equals must stay header_raw_total".into(),
        );
    }
    // S07/T02: P4 bibliography receipt stays Java-derived — the biblStruct
    // count equals the named references_total population, never the native
    // composition output and never the citation-names population.
    if gap.get("java_biblstruct_equals").and_then(|v| v.as_str()) != Some("references_total") {
        errs.push(
            "[hub-header-gap-drift] java_biblstruct_equals must stay references_total".into(),
        );
    }
    if gap
        .get("citation_names_not_biblstruct")
        .and_then(|v| v.as_bool())
        != Some(true)
    {
        errs.push(
            "[hub-header-gap-drift] citation_names_not_biblstruct must stay boolean true".into(),
        );
    }
    for key in [
        "pooling_accepted_with_raw",
        "pooling_citation_names_with_references",
    ] {
        if gap.get(key).and_then(|v| v.as_str()) != Some("forbidden") {
            errs.push(format!(
                "[hub-header-gap-drift] `{key}` must stay forbidden"
            ));
        }
    }
    for (key, forbidden_substring) in [
        ("hub_native_accepted_header", "compose"),
        ("hub_java_header_raw", "compose"),
        ("java_header_authors_equals", "compose"),
    ] {
        if gap
            .get(key)
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains(forbidden_substring))
        {
            errs.push(format!(
                "[hub-header-gap-drift] `{key}` acquired compose vocabulary"
            ));
        }
    }
    errs
}

#[test]
fn hub_native_20_vs_java_21_stays_named_without_pooling_or_compose() {
    let contract: serde_yaml::Value =
        load_yaml(COMPARISON_CONTRACT_REL).expect("comparison contract");
    let errs = validate_java_lift_same_n_gap(&contract);
    assert!(
        errs.is_empty(),
        "hub header gap DATA violations:\n{}",
        errs.join("\n")
    );
}

#[test]
fn rejects_hub_gap_drift_pooling_or_compose_substitution() {
    let contract: serde_yaml::Value =
        load_yaml(COMPARISON_CONTRACT_REL).expect("comparison contract");
    // Drift of either named denominator must fail loudly.
    let mut drifted = contract.clone();
    drifted["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]["java_lift_same_n"]
        ["hub_native_accepted_header"] = serde_yaml::Value::Number(21.into());
    negative(
        "[hub-header-gap-drift]",
        &validate_java_lift_same_n_gap(&drifted),
        "hub native 20 drifted to 21",
    );
    let mut drifted = contract.clone();
    drifted["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]["java_lift_same_n"]
        ["hub_java_header_raw"] = serde_yaml::Value::Number(20.into());
    negative(
        "[hub-header-gap-drift]",
        &validate_java_lift_same_n_gap(&drifted),
        "hub Java 21 drifted to 20",
    );
    // Pooling erosion must fail loudly.
    let mut pooled = contract.clone();
    pooled["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]["java_lift_same_n"]
        ["pooling_accepted_with_raw"] = serde_yaml::Value::String("allowed".into());
    negative(
        "[hub-header-gap-drift]",
        &validate_java_lift_same_n_gap(&pooled),
        "pooling_accepted_with_raw erosion",
    );
    // Compose vocabulary substituted into a named key must fail loudly.
    let mut composed = contract.clone();
    composed["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]["java_lift_same_n"]
        ["java_header_authors_equals"] = serde_yaml::Value::String("compose_header".into());
    negative(
        "[hub-header-gap-drift]",
        &validate_java_lift_same_n_gap(&composed),
        "compose vocabulary in java_header_authors_equals",
    );
    // S07/T02: bibliography conflation must fail loudly — biblStruct stays
    // references_total, the boolean stays true, and pooling stays forbidden.
    let mut bibl = contract.clone();
    bibl["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]["java_lift_same_n"]
        ["java_biblstruct_equals"] = serde_yaml::Value::String("citation_names_total".into());
    negative(
        "[hub-header-gap-drift]",
        &validate_java_lift_same_n_gap(&bibl),
        "java_biblstruct conflated with citation_names_total",
    );
    let mut bibl = contract.clone();
    bibl["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]["java_lift_same_n"]
        ["citation_names_not_biblstruct"] = serde_yaml::Value::Bool(false);
    negative(
        "[hub-header-gap-drift]",
        &validate_java_lift_same_n_gap(&bibl),
        "citation_names_not_biblstruct flipped to false",
    );
    let mut bibl = contract.clone();
    bibl["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]["java_lift_same_n"]
        ["pooling_citation_names_with_references"] = serde_yaml::Value::String("allowed".into());
    negative(
        "[hub-header-gap-drift]",
        &validate_java_lift_same_n_gap(&bibl),
        "pooling citation names with references allowed",
    );
    let mut bibl = contract.clone();
    bibl["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]["java_lift_same_n"]
        ["java_biblstruct_equals"] = serde_yaml::Value::String("compose_references".into());
    negative(
        "[hub-header-gap-drift]",
        &validate_java_lift_same_n_gap(&bibl),
        "compose vocabulary in java_biblstruct_equals",
    );
}

#[test]
fn rejects_missing_or_malformed_comparison_contract_artifact() {
    // Missing artifact is loss, not silence.
    let missing: Result<serde_yaml::Value, String> =
        load_yaml("data/parse/does-not-exist-s07-bibliography.yaml");
    let err = missing.expect_err("missing comparison artifact must fail loudly");
    assert!(
        err.contains("[unreadable-artifact]"),
        "missing artifact must tag [unreadable-artifact], got: {err}"
    );
    // Malformed YAML never parses into a contract value.
    let malformed = serde_yaml::from_str::<serde_yaml::Value>("{unclosed: [1,");
    assert!(
        malformed.is_err(),
        "malformed yaml must fail loudly, not parse"
    );
}

#[test]
fn rejects_removed_named_denominators_block() {
    let contract: serde_yaml::Value =
        load_yaml(COMPARISON_CONTRACT_REL).expect("comparison contract");
    // A removed block is loss, not silence.
    let mut removed = contract.clone();
    removed["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"] =
        serde_yaml::Mapping::new().into();
    negative(
        "[hub-header-gap-lost]",
        &validate_java_lift_same_n_gap(&removed),
        "removed s12_named_denominators",
    );
}

// ───── S08/T02: P6 cascade row stays Java-lifted, never native ─────
//
// The `defaults.cascade_stages.P6` row in the registry of record pins the
// visual family to the Java TEI lift: `production_authority: java_http`,
// `observe_mode: shadow_receipt`, `fsm_consult: [none]`, with the native
// visual module as observe-only surface. `empty_count_not_success=true`
// lives on the live `stamp_p6_visual_observe` receipt (R120): a zero
// figure/table/formula count is a receipt, never a pass — and terminal
// missing Image semantics are `NotObserved`, not a successful zero count.

const P6_POLICY_TAG: &str = "[p6-visual-policy-drift]";

/// Validate the P6 cascade row of the registry of record
/// (`defaults.cascade_stages.P6`). Generic `serde_yaml::Value` reader — no
/// typed deserialization that could mask a removed field.
fn validate_p6_visual_policy(registry: &serde_yaml::Value) -> Vec<String> {
    let mut errs = Vec::new();
    let stage = registry
        .get("defaults")
        .and_then(|d| d.get("cascade_stages"))
        .and_then(|c| c.get("P6"));
    let Some(stage) = stage else {
        errs.push(format!(
            "{P6_POLICY_TAG} registry lost `defaults.cascade_stages.P6` — loss, not silence"
        ));
        return errs;
    };
    let mut str_eq = |key: &str, expected: &str| {
        if stage.get(key).and_then(|v| v.as_str()) != Some(expected) {
            errs.push(format!("{P6_POLICY_TAG} P6 `{key}` must stay `{expected}`"));
        }
    };
    str_eq("production_authority", "java_http");
    str_eq("observe_mode", "shadow_receipt");
    str_eq("native_surface", "crates/da-parse/src/native_visual.rs");
    match stage.get("fsm_consult") {
        Some(v) => {
            let is_none = v
                .as_sequence()
                .is_some_and(|seq| seq.len() == 1 && seq[0].as_str() == Some("none"));
            if !is_none {
                errs.push(format!(
                    "{P6_POLICY_TAG} P6 `fsm_consult` must stay `[none]` — visual never consults the FSM"
                ));
            }
        }
        None => errs.push(format!(
            "{P6_POLICY_TAG} registry lost P6 `fsm_consult` — loss, not silence"
        )),
    }
    if stage.get("grobid").and_then(|v| v.as_str()) != Some("figure_table_formula") {
        errs.push(format!(
            "{P6_POLICY_TAG} P6 `grobid` must stay `figure_table_formula`"
        ));
    }
    errs
}

/// Validate the `stage.figure_table_formula` capability row: shadow state,
/// candidate promotion state, Java baseline, forbidden graph truth and
/// locked import eligibility (R111/R118/R123).
fn validate_figure_table_formula_capability(registry: &serde_yaml::Value) -> Vec<String> {
    const TAG: &str = "[p6-visual-capability-drift]";
    let mut errs = Vec::new();
    let cap = registry
        .get("capabilities")
        .and_then(|c| c.as_sequence())
        .and_then(|seq| {
            seq.iter().find(|c| {
                c.get("id").and_then(|i| i.as_str()) == Some("stage.figure_table_formula")
            })
        });
    let Some(cap) = cap else {
        errs.push(format!(
            "{TAG} registry lost capability `stage.figure_table_formula` — loss, not silence"
        ));
        return errs;
    };
    let str_at = |path: &[&str]| -> String {
        let mut cur = cap;
        for key in path {
            cur = match cur.get(key) {
                Some(v) => v,
                None => return String::new(),
            };
        }
        cur.as_str().unwrap_or_default().to_string()
    };
    for (path, expected) in [
        (vec!["state"], "shadow"),
        (vec!["promotion_state"], "candidate"),
        (vec!["baseline"], "java-grobid-fulltext"),
        (vec!["candidate"], "da-parse@lift-partial"),
        (vec!["rollback_target"], "java-grobid-http@0.9.0-crf"),
    ] {
        if str_at(&path) != expected {
            errs.push(format!(
                "{TAG} stage.figure_table_formula `{}` must stay `{expected}`",
                path.join(".")
            ));
        }
    }
    if str_at(&["implementation", "surface"]) != "crates/da-parse/src/native_visual.rs" {
        errs.push(format!(
            "{TAG} stage.figure_table_formula implementation.surface must stay on native_visual.rs"
        ));
    }
    // `graph_truth: forbidden` is pinned on the `native_visual_shadow:`
    // additive shadow block (`comparison_separation` keeps Java TEI
    // supporting-only and native scores shadow-only), not on this internal
    // stage capability: pin it there instead of requiring it on this row.
    let shadow = registry.get("native_visual_shadow");
    if shadow
        .and_then(|s| s.get("graph_truth"))
        .and_then(|v| v.as_str())
        != Some("forbidden")
    {
        errs.push(format!(
            "{TAG} `native_visual_shadow.graph_truth` must stay `forbidden`"
        ));
    }
    // `import_eligible: false` is a document-level hard lock (top of the
    // registry), not a per-capability field: pin it globally instead of
    // requiring it on this capability row.
    if registry.get("import_eligible").and_then(|v| v.as_bool()) != Some(false) {
        errs.push(format!(
            "{TAG} registry `import_eligible` must stay `false` (D127)"
        ));
    }
    // Terminal missing Image is NotObserved, never a successful zero count:
    // the implementation note must keep that semantics pinned.
    let notes = str_at(&["implementation", "notes"]);
    if !notes.contains("NotObserved") {
        errs.push(format!(
            "{TAG} stage.figure_table_formula note lost terminal NotObserved semantics"
        ));
    }
    if !notes.contains("not zero figures") {
        errs.push(format!(
            "{TAG} stage.figure_table_formula note lost `not zero figures` (no-silent-success)"
        ));
    }
    errs
}

#[test]
fn p6_cascade_row_stays_java_lifted_shadow_receipt() {
    let registry = load_registry_value();
    let errs = validate_p6_visual_policy(&registry);
    assert!(
        errs.is_empty(),
        "P6 visual policy DATA violations:\n{}",
        errs.join("\n")
    );
    let errs = validate_figure_table_formula_capability(&registry);
    assert!(
        errs.is_empty(),
        "figure/table/formula capability DATA violations:\n{}",
        errs.join("\n")
    );
}

#[test]
fn rejects_p6_policy_drift_mutation_or_loss() {
    let registry = load_registry_value();
    // java_http -> native erosion must fail loudly.
    let mut drifted = registry.clone();
    drifted["defaults"]["cascade_stages"]["P6"]["production_authority"] =
        serde_yaml::Value::String("native".into());
    negative(
        P6_POLICY_TAG,
        &validate_p6_visual_policy(&drifted),
        "P6 authority -> native",
    );
    // shadow_receipt -> production_stamp erosion must fail loudly.
    let mut drifted = registry.clone();
    drifted["defaults"]["cascade_stages"]["P6"]["observe_mode"] =
        serde_yaml::Value::String("production_stamp".into());
    negative(
        P6_POLICY_TAG,
        &validate_p6_visual_policy(&drifted),
        "P6 mode -> production_stamp",
    );
    // fsm_consult [none] -> [grammar_route] (FSM promotion) must fail loudly.
    let mut drifted = registry.clone();
    drifted["defaults"]["cascade_stages"]["P6"]["fsm_consult"] =
        serde_yaml::Value::Sequence(vec![serde_yaml::Value::String("grammar_route".into())]);
    negative(
        P6_POLICY_TAG,
        &validate_p6_visual_policy(&drifted),
        "P6 consults the FSM",
    );
    // Removed P6 row is loss, not silence.
    let mut removed = registry.clone();
    if let Some(map) = removed["defaults"]["cascade_stages"].as_mapping_mut() {
        map.remove("P6");
    }
    negative(
        P6_POLICY_TAG,
        &validate_p6_visual_policy(&removed),
        "removed P6 row",
    );
    // Removed P6 fsm_consult is loss, not silence.
    let mut removed = registry.clone();
    if let Some(map) = removed["defaults"]["cascade_stages"]["P6"].as_mapping_mut() {
        map.remove("fsm_consult");
    }
    negative(
        P6_POLICY_TAG,
        &validate_p6_visual_policy(&removed),
        "removed P6 fsm_consult",
    );
    // shadow -> proxy capability promotion must fail loudly.
    let mut promoted = registry.clone();
    if let Some(seq) = promoted["capabilities"].as_sequence_mut()
        && let Some(cap) = seq
            .iter_mut()
            .find(|c| c.get("id").and_then(|i| i.as_str()) == Some("stage.figure_table_formula"))
    {
        cap["state"] = serde_yaml::Value::String("proxy".into());
    }
    negative(
        "[p6-visual-capability-drift]",
        &validate_figure_table_formula_capability(&promoted),
        "figure_table_formula -> proxy",
    );
    // Missing artifact is loss, not silence.
    let missing: Result<serde_yaml::Value, String> =
        load_yaml("data/parse/does-not-exist-s08-p6.yaml");
    let err = missing.expect_err("missing registry artifact must fail loudly");
    assert!(
        err.contains("[unreadable-artifact]"),
        "missing artifact must tag [unreadable-artifact], got: {err}"
    );
    // Malformed YAML never parses into a contract value.
    let malformed = serde_yaml::from_str::<serde_yaml::Value>("{unclosed: [1,");
    assert!(
        malformed.is_err(),
        "malformed yaml must fail loudly, not parse"
    );
}

// ───── S08/T02: visual_bundle stays a later registry name, never a DecisionPoint ─────
//
// `visual_bundle` lives in `consult_status.later` of
// `data/policies/registry.yaml` (ADR-076): parse-pdf consults exactly the two
// DecisionPoints `[grammar_route, profile_route]`, `visual_bundle` is not
// among the six `DecisionPoint` enum variants and not in
// `parse_pdf_consults`. Promotion into the FSM would need a new enum variant;
// this contract fails loudly on it. The DecisionPoint enum and the registry
// file are NOT modified here.

const POLICY_REGISTRY_REL: &str = "data/policies/registry.yaml";

fn load_policy_registry() -> serde_yaml::Value {
    let full = repo_root().join(POLICY_REGISTRY_REL);
    let text = std::fs::read_to_string(&full)
        .unwrap_or_else(|e| panic!("policy registry unreadable: {e}"));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("policy registry yaml malformed: {e}"))
}

/// Validate `consult_status`: exactly two parse_pdf consults, visual_bundle
/// in later, never in consults.
fn validate_visual_bundle_later(policy: &serde_yaml::Value) -> Vec<String> {
    const TAG: &str = "[visual-bundle-later-drift]";
    let mut errs = Vec::new();
    let status = match policy.get("consult_status") {
        Some(s) => s,
        None => {
            errs.push(format!(
                "{TAG} policy registry lost `consult_status` — loss, not silence"
            ));
            return errs;
        }
    };
    let seq_strs = |key: &str| -> Option<Vec<String>> {
        status.get(key).and_then(|v| v.as_sequence()).map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
    };
    match seq_strs("parse_pdf_consults") {
        Some(consults) => {
            if consults != vec!["grammar_route".to_string(), "profile_route".to_string()] {
                errs.push(format!(
                    "{TAG} `parse_pdf_consults` must stay exactly `[grammar_route, profile_route]`, got {consults:?}"
                ));
            }
            if consults.iter().any(|c| c == "visual_bundle") {
                errs.push(format!(
                    "{TAG} `visual_bundle` promoted into `parse_pdf_consults` — candidate-only, never consulted"
                ));
            }
        }
        None => errs.push(format!(
            "{TAG} policy registry lost `consult_status.parse_pdf_consults` — loss, not silence"
        )),
    }
    match seq_strs("later") {
        Some(later) => {
            if !later.iter().any(|c| c == "visual_bundle") {
                errs.push(format!(
                    "{TAG} `visual_bundle` must stay in `consult_status.later`, got {later:?}"
                ));
            }
        }
        None => errs.push(format!(
            "{TAG} policy registry lost `consult_status.later` — loss, not silence"
        )),
    }
    errs
}

#[test]
fn visual_bundle_stays_later_name_outside_decision_points() {
    // DATA side: consult_status pins visual_bundle to later.
    let policy = load_policy_registry();
    let errs = validate_visual_bundle_later(&policy);
    assert!(
        errs.is_empty(),
        "visual_bundle later DATA violations:\n{}",
        errs.join("\n")
    );
    // Live side: the closed DecisionPoint enum carries exactly the six
    // sanctioned variants — no visual_bundle variant exists.
    let src = std::fs::read_to_string(repo_root().join("crates/da-domain/src/fsm_policy.rs"))
        .expect("fsm_policy.rs unreadable");
    for variant in [
        "GrammarRoute",
        "BatchAdmission",
        "ConcurrencyTune",
        "RetryPolicy",
        "SummaryDepth",
        "ProfileRoute",
    ] {
        assert!(
            src.contains(variant),
            "positive control failed: DecisionPoint lost sanctioned variant {variant}"
        );
    }
    assert!(
        !src.contains("VisualBundle") && !src.contains("visual_bundle"),
        "visual_bundle promoted into the DecisionPoint enum — closed set, later stays later"
    );
}

#[test]
fn rejects_visual_bundle_promotion_into_fsm() {
    let policy = load_policy_registry();
    // Promotion into parse_pdf_consults must fail loudly.
    let mut promoted = policy.clone();
    promoted["consult_status"]["parse_pdf_consults"] = serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::String("grammar_route".into()),
        serde_yaml::Value::String("profile_route".into()),
        serde_yaml::Value::String("visual_bundle".into()),
    ]);
    negative(
        "[visual-bundle-later-drift]",
        &validate_visual_bundle_later(&promoted),
        "visual_bundle promoted into parse_pdf_consults",
    );
    // Dropping visual_bundle from later must fail loudly.
    let mut dropped = policy.clone();
    dropped["consult_status"]["later"] = serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::String("head_fallback".into()),
        serde_yaml::Value::String("critic_sample".into()),
    ]);
    negative(
        "[visual-bundle-later-drift]",
        &validate_visual_bundle_later(&dropped),
        "visual_bundle dropped from later",
    );
    // Removed consult_status is loss, not silence.
    let mut removed = policy.clone();
    if let Some(map) = removed.as_mapping_mut() {
        map.remove("consult_status");
    }
    negative(
        "[visual-bundle-later-drift]",
        &validate_visual_bundle_later(&removed),
        "removed consult_status",
    );
    // Missing artifact is loss, not silence.
    let missing: Result<serde_yaml::Value, String> =
        load_yaml("data/policies/does-not-exist-s08-visual.yaml");
    let err = missing.expect_err("missing policy artifact must fail loudly");
    assert!(
        err.contains("[unreadable-artifact]"),
        "missing artifact must tag [unreadable-artifact], got: {err}"
    );
    // Malformed YAML never parses into a contract value.
    let malformed = serde_yaml::from_str::<serde_yaml::Value>("{unclosed: [1,");
    assert!(
        malformed.is_err(),
        "malformed yaml must fail loudly, not parse"
    );
}

// ───── M498/S02/T01: P5 Java authority split and production stamp ─────
//
// P5 (`fulltext_body_sections`) keeps Java ParsedArticle as its sole
// production observation source. The native accountant is an offline,
// shadow-join-only surface; the live stamp contains shape fields only.
// Generic `serde_yaml::Value` reading
// deliberately makes removed or malformed fields fail closed.

const P5_AUTHORITY_SPLIT_TAG: &str = "[p5-authority-split-drift]";
const NATIVE_FULLTEXT_COMPOSITION_REL: &str = "data/parse/native_fulltext_composition.yaml";
const P5_JAVA_FIELDS: [&str; 2] = ["sections_len", "body_empty"];
const P5_POOLING_FORBIDDEN_WITH: [&str; 3] = ["P2", "P6", "P7"];
const P5_EXACT_KEYS: [&str; 10] = [
    "grobid",
    "production_authority",
    "native_surface",
    "fsm_consult",
    "observe_mode",
    "java_observation",
    "java_observation_fields",
    "empty_count_not_success",
    "native_accountant",
    "pooling_forbidden_with",
];
const BODY_ACCOUNTING_QUARANTINE_TOKENS: [&str; 4] = [
    "NativeBodyAccountant",
    "run_native_body_sequence",
    "join_p5_shadow",
    "check_cross_stage_semantic_invariants",
];

fn exact_string_list(value: Option<&serde_yaml::Value>, expected: &[&str]) -> bool {
    value
        .and_then(serde_yaml::Value::as_sequence)
        .is_some_and(|seq| {
            seq.len() == expected.len()
                && seq
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.as_str() == Some(*expected))
        })
}

/// Validate the complete P5 authority split in the registry of record.
fn validate_p5_authority_split_policy(registry: &serde_yaml::Value) -> Vec<String> {
    let mut errs = Vec::new();
    let stage = registry
        .get("defaults")
        .and_then(|d| d.get("cascade_stages"))
        .and_then(|c| c.get("P5"));
    let Some(stage) = stage else {
        errs.push(format!(
            "{P5_AUTHORITY_SPLIT_TAG} registry lost `defaults.cascade_stages.P5` — loss, not silence"
        ));
        return errs;
    };
    let keys = stage
        .as_mapping()
        .map(|map| {
            map.keys()
                .filter_map(serde_yaml::Value::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let expected_keys = P5_EXACT_KEYS.into_iter().collect::<BTreeSet<_>>();
    if keys != expected_keys {
        errs.push(format!(
            "{P5_AUTHORITY_SPLIT_TAG} P5 keys must be exactly {expected_keys:?}, got {keys:?}"
        ));
    }
    for (key, expected) in [
        ("grobid", "fulltext_body_sections"),
        ("production_authority", "java_http"),
        ("native_surface", "crates/da-parse/src/native_body.rs"),
        ("observe_mode", "production_stamp"),
        ("java_observation", "production"),
        ("native_accountant", "shadow_join_only"),
    ] {
        if stage.get(key).and_then(|v| v.as_str()) != Some(expected) {
            errs.push(format!(
                "{P5_AUTHORITY_SPLIT_TAG} P5 `{key}` must stay `{expected}`"
            ));
        }
    }
    if !exact_string_list(stage.get("fsm_consult"), &["none"]) {
        errs.push(format!(
            "{P5_AUTHORITY_SPLIT_TAG} P5 `fsm_consult` must stay exactly `[none]`"
        ));
    }
    if !exact_string_list(stage.get("java_observation_fields"), &P5_JAVA_FIELDS) {
        errs.push(format!(
            "{P5_AUTHORITY_SPLIT_TAG} P5 Java observation fields must be exactly `[sections_len, body_empty]`"
        ));
    }
    if stage.get("empty_count_not_success") != Some(&serde_yaml::Value::Bool(true)) {
        errs.push(format!(
            "{P5_AUTHORITY_SPLIT_TAG} P5 `empty_count_not_success` must be true"
        ));
    }
    if !exact_string_list(
        stage.get("pooling_forbidden_with"),
        &P5_POOLING_FORBIDDEN_WITH,
    ) {
        errs.push(format!(
            "{P5_AUTHORITY_SPLIT_TAG} P5 pooling identities must be exactly `[P2, P6, P7]`"
        ));
    }
    errs
}

fn validate_body_accounting_quarantine(registry: &serde_yaml::Value) -> Vec<String> {
    const TAG: &str = "[body-accounting-quarantine-drift]";
    let got = registry
        .get("defaults")
        .and_then(|defaults| defaults.get("body_accounting_quarantine"))
        .and_then(|quarantine| quarantine.get("tokens"));
    if exact_string_list(got, &BODY_ACCOUNTING_QUARANTINE_TOKENS) {
        Vec::new()
    } else {
        vec![format!(
            "{TAG} tokens must stay exactly {BODY_ACCOUNTING_QUARANTINE_TOKENS:?}"
        )]
    }
}

/// Validate the data-driven admission allowlist: `independently_selectable`
/// is exactly `[header, references]` — body is never independently
/// selectable. Generic reader so a renamed/removed field fails loudly.
fn validate_p5_admission_selectable(composition: &serde_yaml::Value) -> Vec<String> {
    const TAG: &str = "[p5-admission-selectable-drift]";
    let mut errs = Vec::new();
    let got = composition
        .get("admission")
        .and_then(|a| a.get("independently_selectable"))
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect::<Vec<_>>()
        });
    match got {
        Some(got) => {
            if got != vec!["header".to_string(), "references".to_string()] {
                errs.push(format!(
                    "{TAG} `admission.independently_selectable` must stay exactly `[header, references]`, got {got:?}"
                ));
            }
        }
        None => errs.push(format!(
            "{TAG} composition lost `admission.independently_selectable` — loss, not silence"
        )),
    }
    errs
}

#[test]
fn p5_cascade_row_preserves_java_authority_split() {
    let registry = load_registry_value();
    let errs = validate_p5_authority_split_policy(&registry);
    assert!(
        errs.is_empty(),
        "P5 authority-split DATA violations:\n{}",
        errs.join("\n")
    );
    let errs = validate_body_accounting_quarantine(&registry);
    assert!(
        errs.is_empty(),
        "body-accounting quarantine DATA violations:\n{}",
        errs.join("\n")
    );
    // Admission allowlist is DATA-driven from the composition authority.
    let composition: serde_yaml::Value = expect_loaded(NATIVE_FULLTEXT_COMPOSITION_REL);
    let errs = validate_p5_admission_selectable(&composition);
    assert!(
        errs.is_empty(),
        "P5 admission DATA violations:\n{}",
        errs.join("\n")
    );
}

#[test]
fn rejects_p5_authority_split_drift_or_loss() {
    let registry = load_registry_value();
    // The production P5 row is deep and closed: shadow-checker metadata must
    // remain in the quarantine map, never inside the production cascade row.
    let mut widened = registry.clone();
    widened["defaults"]["cascade_stages"]["P5"]["cross_stage_semantic_invariants"] =
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    negative(
        P5_AUTHORITY_SPLIT_TAG,
        &validate_p5_authority_split_policy(&widened),
        "P5 extra cross-stage key",
    );
    let mut quarantine = registry.clone();
    quarantine["defaults"]["body_accounting_quarantine"]["tokens"] = serde_yaml::Value::Sequence(
        BODY_ACCOUNTING_QUARANTINE_TOKENS
            .iter()
            .take(3)
            .map(|token| serde_yaml::Value::String((*token).into()))
            .collect(),
    );
    negative(
        "[body-accounting-quarantine-drift]",
        &validate_body_accounting_quarantine(&quarantine),
        "dropped cross-stage checker token",
    );
    // production_stamp -> accountant_later demotion must fail loudly.
    let mut drifted = registry.clone();
    drifted["defaults"]["cascade_stages"]["P5"]["observe_mode"] =
        serde_yaml::Value::String("accountant_later".into());
    negative(
        P5_AUTHORITY_SPLIT_TAG,
        &validate_p5_authority_split_policy(&drifted),
        "P5 mode -> accountant_later",
    );
    // production_stamp -> shadow_receipt quarantine collapse must fail loudly.
    let mut drifted = registry.clone();
    drifted["defaults"]["cascade_stages"]["P5"]["observe_mode"] =
        serde_yaml::Value::String("shadow_receipt".into());
    negative(
        P5_AUTHORITY_SPLIT_TAG,
        &validate_p5_authority_split_policy(&drifted),
        "P5 mode -> shadow_receipt",
    );
    // Java authority or source substitution must fail loudly.
    for (key, value, label) in [
        ("production_authority", "native", "P5 authority -> native"),
        ("native_surface", "none", "P5 native surface erased"),
        (
            "java_observation",
            "native",
            "P5 Java observation -> native",
        ),
        (
            "native_accountant",
            "production",
            "P5 native accountant promoted",
        ),
    ] {
        let mut drifted = registry.clone();
        drifted["defaults"]["cascade_stages"]["P5"][key] = serde_yaml::Value::String(value.into());
        negative(
            P5_AUTHORITY_SPLIT_TAG,
            &validate_p5_authority_split_policy(&drifted),
            label,
        );
    }
    // fsm_consult [none] -> [grammar_route] (FSM promotion) must fail loudly.
    let mut drifted = registry.clone();
    drifted["defaults"]["cascade_stages"]["P5"]["fsm_consult"] =
        serde_yaml::Value::Sequence(vec![serde_yaml::Value::String("grammar_route".into())]);
    negative(
        P5_AUTHORITY_SPLIT_TAG,
        &validate_p5_authority_split_policy(&drifted),
        "P5 consults the FSM",
    );
    // Removed P5 row is loss, not silence.
    let mut removed = registry.clone();
    if let Some(map) = removed["defaults"]["cascade_stages"].as_mapping_mut() {
        map.remove("P5");
    }
    negative(
        P5_AUTHORITY_SPLIT_TAG,
        &validate_p5_authority_split_policy(&removed),
        "removed P5 row",
    );
    // Every authority-split field is required; removal is loss, not silence.
    for key in [
        "observe_mode",
        "java_observation",
        "java_observation_fields",
        "native_accountant",
        "empty_count_not_success",
        "pooling_forbidden_with",
    ] {
        let mut removed = registry.clone();
        if let Some(map) = removed["defaults"]["cascade_stages"]["P5"].as_mapping_mut() {
            map.remove(key);
        }
        negative(
            P5_AUTHORITY_SPLIT_TAG,
            &validate_p5_authority_split_policy(&removed),
            &format!("removed P5 {key}"),
        );
    }
    // Typed empty is not success, and denominator pooling cannot be relaxed.
    let mut drifted = registry.clone();
    drifted["defaults"]["cascade_stages"]["P5"]["empty_count_not_success"] =
        serde_yaml::Value::Bool(false);
    negative(
        P5_AUTHORITY_SPLIT_TAG,
        &validate_p5_authority_split_policy(&drifted),
        "P5 empty count accepted as success",
    );
    for member in ["P2", "P6", "P7"] {
        let mut drifted = registry.clone();
        drifted["defaults"]["cascade_stages"]["P5"]["pooling_forbidden_with"] =
            serde_yaml::Value::Sequence(
                P5_POOLING_FORBIDDEN_WITH
                    .iter()
                    .filter(|candidate| **candidate != member)
                    .map(|candidate| serde_yaml::Value::String((*candidate).into()))
                    .collect(),
            );
        negative(
            P5_AUTHORITY_SPLIT_TAG,
            &validate_p5_authority_split_policy(&drifted),
            &format!("P5 pooling member {member} removed"),
        );
    }
    // Java observation must use exactly the named counters and flags.
    for fields in [
        vec!["sections_len"],
        vec!["body_empty"],
        vec!["sections_len", "body_text"],
    ] {
        let mut drifted = registry.clone();
        drifted["defaults"]["cascade_stages"]["P5"]["java_observation_fields"] =
            serde_yaml::Value::Sequence(
                fields
                    .iter()
                    .map(|field| serde_yaml::Value::String((*field).into()))
                    .collect(),
            );
        negative(
            P5_AUTHORITY_SPLIT_TAG,
            &validate_p5_authority_split_policy(&drifted),
            "P5 Java observation fields drifted",
        );
    }
    // Body smuggled into independently_selectable must fail loudly.
    let composition: serde_yaml::Value = expect_loaded(NATIVE_FULLTEXT_COMPOSITION_REL);
    let mut widened = composition.clone();
    widened["admission"]["independently_selectable"] = serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::String("header".into()),
        serde_yaml::Value::String("references".into()),
        serde_yaml::Value::String("body".into()),
    ]);
    negative(
        "[p5-admission-selectable-drift]",
        &validate_p5_admission_selectable(&widened),
        "body added to independently_selectable",
    );
    // Missing artifact is loss, not silence.
    let missing: Result<serde_yaml::Value, String> =
        load_yaml("data/parse/does-not-exist-s09-p5.yaml");
    let err = missing.expect_err("missing registry artifact must fail loudly");
    assert!(
        err.contains("[unreadable-artifact]"),
        "missing artifact must tag [unreadable-artifact], got: {err}"
    );
    // Malformed YAML never parses into a contract value.
    let malformed = serde_yaml::from_str::<serde_yaml::Value>("{unclosed: [1,");
    assert!(
        malformed.is_err(),
        "malformed yaml must fail loudly, not parse"
    );
}

// ───── M493 S10/T01: exact later set + later names out of cascade fsm_consult ─────
//
// Meta/prompt freeze after the family quarantines: `consult_status` of
// `data/policies/registry.yaml` keeps the exact ordered
// `later == [head_fallback, visual_bundle, critic_sample]`, `decision_points`
// stays exactly the six approved keys, and no cascade `fsm_consult` of the
// registry of record (`data/parse/grobid_compatibility.yaml`) may carry a
// later name. Only NEW local `s10_*` helpers are added here — no production
// symbol, no existing helper and no DATA file is touched. S09 pins (P5
// `accountant_later`, no `stamp_p5`, `independently_selectable`) are left
// intact above.

/// Exact ordered later set (ADR-076).
const S10_LATER_EXACT: [&str; 3] = ["head_fallback", "visual_bundle", "critic_sample"];

/// Exact approved decision-point keys (matches the six DecisionPoint wires).
const S10_DECISION_KEYS_EXACT: [&str; 6] = [
    "grammar_route",
    "batch_admission",
    "concurrency_tune",
    "retry_policy",
    "summary_depth",
    "profile_route",
];

/// Local S10 helper: string sequence of a mapping key, or None on
/// missing/non-sequence/non-string shape (loss/malformed, never silence).
fn s10_seq_strs(map: &serde_yaml::Mapping, key: &str) -> Option<Vec<String>> {
    map.get(serde_yaml::Value::String(key.into()))
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect()
        })
}

/// Local S10 helper: violations of the exact later-set contract in the
/// policy registry. Every violation carries `[s10-later-set-drift]`.
fn s10_later_set_violations(policy: &serde_yaml::Value) -> Vec<String> {
    const TAG: &str = "[s10-later-set-drift]";
    let mut errs = Vec::new();
    let Some(status) = policy.get("consult_status") else {
        return vec![format!(
            "{TAG} policy registry lost `consult_status` — loss, not silence"
        )];
    };
    let Some(map) = status.as_mapping() else {
        return vec![format!(
            "{TAG} `consult_status` is not a mapping — malformed, not silence"
        )];
    };
    match s10_seq_strs(map, "later") {
        Some(later) => {
            let expected: Vec<String> = S10_LATER_EXACT.iter().map(|s| s.to_string()).collect();
            if later != expected {
                errs.push(format!(
                    "{TAG} `consult_status.later` must stay exactly {expected:?} (ordered), got {later:?}"
                ));
            }
        }
        None => errs.push(format!(
            "{TAG} policy registry lost `consult_status.later` — loss, not silence"
        )),
    }
    match s10_seq_strs(map, "parse_pdf_consults") {
        Some(consults) => {
            if consults != vec!["grammar_route".to_string(), "profile_route".to_string()] {
                errs.push(format!(
                    "{TAG} `parse_pdf_consults` must stay exactly `[grammar_route, profile_route]`, got {consults:?}"
                ));
            }
            for later in S10_LATER_EXACT {
                if consults.iter().any(|c| c == later) {
                    errs.push(format!(
                        "{TAG} later name `{later}` promoted into `parse_pdf_consults` — later stays later"
                    ));
                }
            }
        }
        None => errs.push(format!(
            "{TAG} policy registry lost `consult_status.parse_pdf_consults` — loss, not silence"
        )),
    }
    errs
}

/// Local S10 helper: violations of the exact decision_points key set.
/// Every violation carries `[s10-decision-keys-drift]`.
fn s10_decision_keys_violations(policy: &serde_yaml::Value) -> Vec<String> {
    const TAG: &str = "[s10-decision-keys-drift]";
    let Some(points) = policy.get("decision_points") else {
        return vec![format!(
            "{TAG} policy registry lost `decision_points` — loss, not silence"
        )];
    };
    let Some(map) = points.as_mapping() else {
        return vec![format!(
            "{TAG} `decision_points` is not a mapping — malformed, not silence"
        )];
    };
    let mut got: Vec<String> = map
        .keys()
        .map(|k| k.as_str().unwrap_or_default().to_string())
        .collect();
    got.sort();
    let mut expected: Vec<String> = S10_DECISION_KEYS_EXACT
        .iter()
        .map(|s| s.to_string())
        .collect();
    expected.sort();
    if got != expected {
        return vec![format!(
            "{TAG} `decision_points` must stay exactly {expected:?} (six approved keys), got {got:?}"
        )];
    }
    Vec::new()
}

/// Local S10 helper: cascade fsm_consult violations in the registry of
/// record. Every violation carries `[s10-cascade-consult-drift]`.
fn s10_cascade_consult_violations(registry: &serde_yaml::Value) -> Vec<String> {
    const TAG: &str = "[s10-cascade-consult-drift]";
    let mut errs = Vec::new();
    let Some(stages) = registry
        .get("defaults")
        .and_then(|d| d.get("cascade_stages"))
        .and_then(|c| c.as_mapping())
    else {
        return vec![format!(
            "{TAG} registry lost `defaults.cascade_stages` — loss, not silence"
        )];
    };
    for (key, row) in stages {
        let sid = key.as_str().unwrap_or_default();
        let consults: Vec<String> = match row.get("fsm_consult") {
            Some(v) => match v.as_sequence() {
                Some(seq) => seq
                    .iter()
                    .map(|v| v.as_str().unwrap_or_default().to_string())
                    .collect(),
                None => {
                    errs.push(format!(
                        "{TAG} stage `{sid}` `fsm_consult` is not a sequence — malformed, not silence"
                    ));
                    continue;
                }
            },
            None => {
                errs.push(format!(
                    "{TAG} stage `{sid}` lost `fsm_consult` — loss, not silence"
                ));
                continue;
            }
        };
        for later in S10_LATER_EXACT {
            if consults.iter().any(|c| c == later) {
                errs.push(format!(
                    "{TAG} later name `{later}` in cascade stage `{sid}` `fsm_consult` — later stays later"
                ));
            }
        }
    }
    errs
}

#[test]
fn s10_registry_later_set_stays_exact_and_ordered() {
    let policy = load_policy_registry();
    let errs = s10_later_set_violations(&policy);
    assert!(
        errs.is_empty(),
        "later-set DATA violations:\n{}",
        errs.join("\n")
    );
    let errs = s10_decision_keys_violations(&policy);
    assert!(
        errs.is_empty(),
        "decision-points DATA violations:\n{}",
        errs.join("\n")
    );
    // Registry of record: no cascade fsm_consult carries a later name.
    let registry = load_registry_value();
    let errs = s10_cascade_consult_violations(&registry);
    assert!(
        errs.is_empty(),
        "cascade consult DATA violations:\n{}",
        errs.join("\n")
    );
    // Cross-check against the live six-variant source (positive control that
    // the DATA keys and the enum agree without importing production types).
    let src = std::fs::read_to_string(repo_root().join("crates/da-domain/src/fsm_policy.rs"))
        .expect("fsm_policy.rs unreadable");
    for wire in S10_DECISION_KEYS_EXACT {
        assert!(
            src.contains(&format!("\"{wire}\"")),
            "positive control failed: DecisionPoint source lost wire `{wire}`"
        );
    }
}

#[test]
fn s10_rejects_later_set_drift_and_cascade_promotion() {
    let policy = load_policy_registry();
    // Removal of a later name must fail loudly.
    let mut removed = policy.clone();
    removed["consult_status"]["later"] = serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::String("head_fallback".into()),
        serde_yaml::Value::String("critic_sample".into()),
    ]);
    negative(
        "[s10-later-set-drift]",
        &s10_later_set_violations(&removed),
        "visual_bundle dropped from later",
    );
    // Reorder must fail loudly (ordered pin).
    let mut reordered = policy.clone();
    reordered["consult_status"]["later"] = serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::String("visual_bundle".into()),
        serde_yaml::Value::String("head_fallback".into()),
        serde_yaml::Value::String("critic_sample".into()),
    ]);
    negative(
        "[s10-later-set-drift]",
        &s10_later_set_violations(&reordered),
        "later reordered",
    );
    // Extension with a seventh point must fail loudly.
    let mut extended = policy.clone();
    extended["consult_status"]["later"] = serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::String("head_fallback".into()),
        serde_yaml::Value::String("visual_bundle".into()),
        serde_yaml::Value::String("critic_sample".into()),
        serde_yaml::Value::String("invented_point".into()),
    ]);
    negative(
        "[s10-later-set-drift]",
        &s10_later_set_violations(&extended),
        "later extended",
    );
    // Later name in parse_pdf_consults must fail loudly.
    let mut promoted = policy.clone();
    promoted["consult_status"]["parse_pdf_consults"] = serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::String("grammar_route".into()),
        serde_yaml::Value::String("head_fallback".into()),
    ]);
    negative(
        "[s10-later-set-drift]",
        &s10_later_set_violations(&promoted),
        "later in parse_pdf_consults",
    );
    // A seventh decision_points key must fail loudly.
    let mut wider = policy.clone();
    wider["decision_points"]["head_fallback"] =
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    negative(
        "[s10-decision-keys-drift]",
        &s10_decision_keys_violations(&wider),
        "seventh decision_points key",
    );
    // A dropped decision_points key must fail loudly.
    let mut narrower = policy.clone();
    if let Some(map) = narrower["decision_points"].as_mapping_mut() {
        map.remove("profile_route");
    }
    negative(
        "[s10-decision-keys-drift]",
        &s10_decision_keys_violations(&narrower),
        "dropped profile_route key",
    );
    // Later name smuggled into any cascade fsm_consult must fail loudly.
    let registry = load_registry_value();
    for later in S10_LATER_EXACT {
        let mut drifted = registry.clone();
        drifted["defaults"]["cascade_stages"]["P6"]["fsm_consult"] =
            serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(later.into())]);
        negative(
            "[s10-cascade-consult-drift]",
            &s10_cascade_consult_violations(&drifted),
            &format!("later `{later}` in P6 fsm_consult"),
        );
    }
    // Removed cascade fsm_consult is loss, not silence.
    let mut lost = registry.clone();
    if let Some(map) = lost["defaults"]["cascade_stages"]["P6"].as_mapping_mut() {
        map.remove("fsm_consult");
    }
    negative(
        "[s10-cascade-consult-drift]",
        &s10_cascade_consult_violations(&lost),
        "removed P6 fsm_consult",
    );
    // Removed consult_status is loss, not silence.
    let mut removed_status = policy.clone();
    if let Some(map) = removed_status.as_mapping_mut() {
        map.remove("consult_status");
    }
    negative(
        "[s10-later-set-drift]",
        &s10_later_set_violations(&removed_status),
        "removed consult_status",
    );
    // Missing artifact is loss, not silence.
    let missing: Result<serde_yaml::Value, String> =
        load_yaml("data/policies/does-not-exist-s10-later.yaml");
    let err = missing.expect_err("missing policy artifact must fail loudly");
    assert!(
        err.contains("[unreadable-artifact]"),
        "missing artifact must tag [unreadable-artifact], got: {err}"
    );
    // Malformed YAML never parses into a contract value.
    let malformed = serde_yaml::from_str::<serde_yaml::Value>("{unclosed: [1,");
    assert!(
        malformed.is_err(),
        "malformed yaml must fail loudly, not parse"
    );
}

// ───── S11/T02: DATA constructor policy + D204 fingerprint separation ─────

/// S11 DATA constructor policy: the exact allowlist, forbidden role and scan
/// roots declared in `data/parse/grobid_compatibility.yaml
/// #defaults.production_pdf_constructors`. Live DATA is never mutated here —
/// drift is proven on synthetic clones.
const S11_CONSTRUCTOR_SCAN_ROOTS: [&str; 2] = ["crates/da-cli/src", "crates/da-cli/examples"];
const S11_CONSTRUCTOR_ALLOWLIST: [&str; 5] = [
    "health_probe",
    "with_profile",
    "consult_helper",
    "test",
    "comment",
];
const S11_CONSTRUCTOR_FORBIDDEN_ROLE: &str = "PARSE_BARE";
const S11_CONSTRUCTOR_HELPER: &str = "crates/da-cli/src/parse_profile.rs";

fn s11_constructor_policy(value: &serde_yaml::Value) -> Vec<String> {
    let mut errs = Vec::new();
    let ctor = &value["defaults"]["production_pdf_constructors"];
    if ctor.is_null() {
        return vec![
            "[s11-constructor-policy-drift] defaults.production_pdf_constructors missing"
                .to_string(),
        ];
    }
    let roots: Vec<String> = ctor
        .get("scan_roots")
        .and_then(|v| v.as_sequence())
        .map(|s| {
            s.iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let expected_roots: Vec<String> = S11_CONSTRUCTOR_SCAN_ROOTS
        .iter()
        .map(|s| s.to_string())
        .collect();
    if roots != expected_roots {
        errs.push(format!(
            "[s11-constructor-policy-drift] scan_roots {roots:?} != exact {expected_roots:?}"
        ));
    }
    let allowed: Vec<String> = ctor
        .get("allowed_from_env_roles")
        .and_then(|v| v.as_sequence())
        .map(|s| {
            s.iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let expected_allowed: Vec<String> = S11_CONSTRUCTOR_ALLOWLIST
        .iter()
        .map(|s| s.to_string())
        .collect();
    if allowed != expected_allowed {
        errs.push(format!(
            "[s11-constructor-policy-drift] allowed_from_env_roles {allowed:?} != exact {expected_allowed:?}"
        ));
    }
    if ctor.get("forbidden_role").and_then(|v| v.as_str()) != Some(S11_CONSTRUCTOR_FORBIDDEN_ROLE) {
        errs.push(format!(
            "[s11-constructor-policy-drift] forbidden_role {:?} != {S11_CONSTRUCTOR_FORBIDDEN_ROLE:?}",
            ctor.get("forbidden_role").and_then(|v| v.as_str())
        ));
    }
    if ctor.get("consult_helper").and_then(|v| v.as_str()) != Some(S11_CONSTRUCTOR_HELPER) {
        errs.push(format!(
            "[s11-constructor-policy-drift] consult_helper {:?} != {S11_CONSTRUCTOR_HELPER:?}",
            ctor.get("consult_helper").and_then(|v| v.as_str())
        ));
    }
    errs
}

#[test]
fn s11_data_constructor_policy_stays_exact() {
    let registry = load_registry_value();
    let errs = s11_constructor_policy(&registry);
    assert!(
        errs.is_empty(),
        "constructor-policy DATA violations:\n{}",
        errs.join("\n")
    );
}

#[test]
fn s11_rejects_constructor_policy_drift_on_synthetic_clones() {
    let registry = load_registry_value();
    // Dropped allowlist role.
    let mut dropped = registry.clone();
    dropped["defaults"]["production_pdf_constructors"]["allowed_from_env_roles"] =
        serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::String("health_probe".into()),
            serde_yaml::Value::String("with_profile".into()),
        ]);
    negative(
        "[s11-constructor-policy-drift]",
        &s11_constructor_policy(&dropped),
        "dropped allowlist roles",
    );
    // Invented allowlist role.
    let mut invented = registry.clone();
    if let Some(seq) = invented["defaults"]["production_pdf_constructors"]["allowed_from_env_roles"]
        .as_sequence_mut()
    {
        seq.push(serde_yaml::Value::String("bare_parse".into()));
    }
    negative(
        "[s11-constructor-policy-drift]",
        &s11_constructor_policy(&invented),
        "invented allowlist role",
    );
    // Rotated forbidden role.
    let mut rotated = registry.clone();
    rotated["defaults"]["production_pdf_constructors"]["forbidden_role"] =
        serde_yaml::Value::String("PARSE_WARN".into());
    negative(
        "[s11-constructor-policy-drift]",
        &s11_constructor_policy(&rotated),
        "rotated forbidden role",
    );
    // Narrowed scan roots.
    let mut narrowed = registry.clone();
    narrowed["defaults"]["production_pdf_constructors"]["scan_roots"] =
        serde_yaml::Value::Sequence(vec![serde_yaml::Value::String("crates/da-cli/src".into())]);
    negative(
        "[s11-constructor-policy-drift]",
        &s11_constructor_policy(&narrowed),
        "narrowed scan roots",
    );
    // Moved consult helper.
    let mut moved = registry.clone();
    moved["defaults"]["production_pdf_constructors"]["consult_helper"] =
        serde_yaml::Value::String("crates/da-cli/src/elsewhere.rs".into());
    negative(
        "[s11-constructor-policy-drift]",
        &s11_constructor_policy(&moved),
        "moved consult helper",
    );
    // Removed constructor block is loss, not silence.
    let mut lost = registry.clone();
    if let Some(map) = lost["defaults"].as_mapping_mut() {
        map.remove("production_pdf_constructors");
    }
    negative(
        "[s11-constructor-policy-drift]",
        &s11_constructor_policy(&lost),
        "removed constructor block",
    );
}

#[test]
fn s11_live_registry_default_profile_stays_structural_not_rich() {
    // DATA default (profile_route.native.default) is structural; the live
    // helper consult must agree without importing da-cli types.
    let policy: serde_yaml::Value = expect_loaded("data/policies/registry.yaml");
    let default = policy["decision_points"]["profile_route"]["native"]["default"]
        .as_str()
        .unwrap_or_default();
    assert_eq!(
        default, "structural",
        "S11: DATA profile_route default `{default}` drifted — corpus default must stay structural"
    );
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        STRUCTURAL_FP,
        "S11: live structural fingerprint drifted from the DATA-bound constant"
    );
    assert_eq!(
        GrobidProfile::rich().configuration_fingerprint(),
        RICH_FP,
        "S11: live rich fingerprint drifted from the DATA-bound constant"
    );
    assert_ne!(
        RICH_FP, STRUCTURAL_FP,
        "S11: profile/fingerprint collision — D204 separation lost"
    );
    // Synthetic collision control: a one-flag mutation matches neither.
    let mutated = "profile=structural;consolidateHeader=1;consolidateCitations=0;teiCoordinates=figure,table,note,formula";
    assert_ne!(mutated, RICH_FP);
    assert_ne!(mutated, STRUCTURAL_FP);
}

// ───── M493 S12/T01: one named candidate family after W0-W6 receipts ─────
//
// Final wave-admission contract: after W0-W6 receipts exactly one named
// native candidate contour exists (`da-parse.native_header_reference_composition`
// in shadow/candidate), its independently selectable lanes stay exactly
// `[header, references]`, admission requires the full four-receipt set with
// named non-poolable denominators (hub 20 vs 21, citation names != biblStruct),
// and the wave promotes nothing — no explicit decision, Java GROBID stays
// production oracle and sole rollback target, import/graph writes frozen.
//
// One shared validator (`validate_s12_wave_admission`) binds all three live
// DATA surfaces (registry of record, comparison contract, composition
// authority); every synthetic drift below fails through that helper, never
// through ad-hoc asserts. No production YAML or runtime code is touched.

const S12_CANDIDATE_ID: &str = "da-parse.native_header_reference_composition";
const S12_FULLTEXT_CANDIDATE_ID: &str = "da-parse.native_fulltext_composition";
const S12_ROLLBACK: &str = "java-grobid-http@0.9.0-crf";
const S12_ADMISSION_TAG: &str = "[s12-wave-admission-drift]";
const S12_LANES_EXACT: [&str; 2] = ["header", "references"];
const S12_REQUIRED_RECEIPTS: [&str; 4] = [
    "native_date_shadow",
    "native_personal_name_shadow",
    "native_affiliation_shadow",
    "native_citation_shadow",
];

struct S12AdmissionSnapshot {
    registry: serde_yaml::Value,
    comparison: serde_yaml::Value,
    composition: serde_yaml::Value,
}

fn load_s12_admission() -> S12AdmissionSnapshot {
    S12AdmissionSnapshot {
        registry: expect_loaded(REGISTRY_REL_PATH),
        comparison: expect_loaded(COMPARISON_CONTRACT_REL),
        composition: expect_loaded(NATIVE_FULLTEXT_COMPOSITION_REL),
    }
}

fn s12_seq_strs(v: &serde_yaml::Value) -> Vec<String> {
    v.as_sequence()
        .map(|seq| {
            seq.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Shared S12 wave-admission validator over the three live DATA surfaces.
/// Generic `serde_yaml::Value` reader — a renamed/removed field fails loudly.
fn validate_s12_wave_admission(snap: &S12AdmissionSnapshot) -> Vec<String> {
    let mut errs: Vec<String> = Vec::new();
    // ── Registry wave promotion freeze (this wave promotes nothing) ──
    match snap
        .registry
        .get("defaults")
        .and_then(|d| d.get("promotion"))
    {
        Some(p) => {
            if p.get("this_wave_promotes").and_then(|v| v.as_str()) != Some("none") {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} registry `defaults.promotion.this_wave_promotes` must stay `none` — this wave promotes nothing"
                ));
            }
            if p.get("max_non_candidate_families").and_then(|v| v.as_i64()) != Some(1) {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} registry `defaults.promotion.max_non_candidate_families` must stay 1 — one candidate family only"
                ));
            }
            if p.get("deployed").and_then(|v| v.as_bool()) != Some(false) {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} registry `defaults.promotion.deployed` must stay false"
                ));
            }
            if p.get("auto_promote").and_then(|v| v.as_str()) != Some("forbidden") {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} registry `defaults.promotion.auto_promote` must stay forbidden"
                ));
            }
        }
        None => errs.push(format!(
            "{S12_ADMISSION_TAG} registry lost `defaults.promotion` — loss, not silence"
        )),
    }
    if snap.registry.get("auto_promote").and_then(|v| v.as_str()) != Some("forbidden") {
        errs.push(format!(
            "{S12_ADMISSION_TAG} registry top-level `auto_promote` must stay forbidden"
        ));
    }
    if snap
        .registry
        .get("import_eligible")
        .and_then(|v| v.as_bool())
        != Some(false)
    {
        errs.push(format!(
            "{S12_ADMISSION_TAG} registry `import_eligible` must stay false — import frozen"
        ));
    }
    if snap
        .registry
        .get("defaults")
        .and_then(|d| d.get("rollback_target"))
        .and_then(|v| v.as_str())
        != Some(S12_ROLLBACK)
    {
        errs.push(format!(
            "{S12_ADMISSION_TAG} registry `defaults.rollback_target` must stay the Java rollback target"
        ));
    }
    // ── Named composition candidate: shadow/candidate, no explicit decision ──
    let Some(shadow) = snap
        .comparison
        .get("native_header_reference_composition_shadow")
    else {
        return vec![format!(
            "{S12_ADMISSION_TAG} comparison lost `native_header_reference_composition_shadow` — loss, not silence"
        )];
    };
    if shadow.get("candidate_id").and_then(|v| v.as_str()) != Some(S12_CANDIDATE_ID) {
        errs.push(format!(
            "{S12_ADMISSION_TAG} composition `candidate_id` must stay exactly `{S12_CANDIDATE_ID}`"
        ));
    }
    for (key, expected) in [
        ("observation_mode", "shadow"),
        ("promotion_state", "candidate"),
        ("auto_promote", "forbidden"),
    ] {
        if shadow.get(key).and_then(|v| v.as_str()) != Some(expected) {
            errs.push(format!(
                "{S12_ADMISSION_TAG} composition `{key}` must stay `{expected}`"
            ));
        }
    }
    // No promotion decision exists: the key stays absent-or-null until a
    // later explicit user yes (outside M493) creates one.
    if !matches!(
        shadow.get("explicit_decision"),
        None | Some(serde_yaml::Value::Null)
    ) {
        errs.push(format!(
            "{S12_ADMISSION_TAG} composition `explicit_decision` must stay absent-or-null — no promotion decision this wave"
        ));
    }
    for lane in S12_LANES_EXACT {
        if shadow
            .get("rollback")
            .and_then(|r| r.get(lane))
            .and_then(|v| v.as_str())
            != Some(S12_ROLLBACK)
        {
            errs.push(format!(
                "{S12_ADMISSION_TAG} composition `rollback.{lane}` must stay the Java rollback target"
            ));
        }
    }
    // Independently selectable lanes stay exactly [header, references], ordered.
    let lanes = s12_seq_strs(&shadow["independently_selectable_lanes"]);
    let expected_lanes: Vec<String> = S12_LANES_EXACT.iter().map(|s| s.to_string()).collect();
    if lanes != expected_lanes {
        errs.push(format!(
            "{S12_ADMISSION_TAG} composition `independently_selectable_lanes` must stay exactly `[header, references]`, got {lanes:?}"
        ));
    }
    // Admission requires the full four-receipt set, exact and ordered.
    let required = s12_seq_strs(&shadow["component_receipts"]["required"]);
    let expected_required: Vec<String> = S12_REQUIRED_RECEIPTS
        .iter()
        .map(|s| s.to_string())
        .collect();
    if required != expected_required {
        errs.push(format!(
            "{S12_ADMISSION_TAG} composition `component_receipts.required` must stay the exact four-receipt set, got {required:?}"
        ));
    }
    // Each required receipt block exists as shadow/candidate evidence bound
    // to the same Java rollback target.
    for receipt in S12_REQUIRED_RECEIPTS {
        match snap.comparison.get(receipt) {
            Some(block) => {
                if block.get("promotion_state").and_then(|v| v.as_str()) != Some("candidate") {
                    errs.push(format!(
                        "{S12_ADMISSION_TAG} receipt `{receipt}` `promotion_state` must stay candidate"
                    ));
                }
                if block.get("rollback_target").and_then(|v| v.as_str()) != Some(S12_ROLLBACK) {
                    errs.push(format!(
                        "{S12_ADMISSION_TAG} receipt `{receipt}` `rollback_target` must stay the Java rollback target"
                    ));
                }
            }
            None => errs.push(format!(
                "{S12_ADMISSION_TAG} receipt block `{receipt}` missing — loss, not silence"
            )),
        }
    }
    // Named non-poolable denominators on the composition shadow.
    for (receipt, field, expected) in [
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
    ] {
        if shadow["component_receipts"][receipt][field].as_i64() != Some(expected) {
            errs.push(format!(
                "{S12_ADMISSION_TAG} composition `component_receipts.{receipt}.{field}` must stay {expected}"
            ));
        }
    }
    if !s12_seq_strs(&shadow["forbidden"]).contains(&"pooled_denominators".to_string()) {
        errs.push(format!(
            "{S12_ADMISSION_TAG} composition `forbidden` lost `pooled_denominators`"
        ));
    }
    // ── Ablation freeze: explicit decision null, graph write forbidden ──
    match snap
        .comparison
        .get("metrics")
        .and_then(|m| m.get("learning"))
        .and_then(|l| l.get("ruvllm_ablation"))
    {
        Some(ablation) => {
            if ablation.get("explicit_decision") != Some(&serde_yaml::Value::Null) {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} ablation `explicit_decision` must stay null — no explicit yes this wave"
                ));
            }
            if ablation.get("deployed").and_then(|v| v.as_bool()) != Some(false) {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} ablation `deployed` must stay false"
                ));
            }
            if ablation.get("graph_write").and_then(|v| v.as_str()) != Some("forbidden") {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} ablation `graph_write` must stay forbidden — graph frozen"
                ));
            }
            if ablation.get("promotion_state").and_then(|v| v.as_str()) != Some("candidate") {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} ablation `promotion_state` must stay candidate"
                ));
            }
        }
        None => errs.push(format!(
            "{S12_ADMISSION_TAG} comparison lost `metrics.learning.ruvllm_ablation` — loss, not silence"
        )),
    }
    // ── Fulltext shadow + composition authority: same lanes, same hub gap ──
    for (label, block) in [
        (
            "comparison-native_fulltext_composition_shadow",
            snap.comparison.get("native_fulltext_composition_shadow"),
        ),
        ("composition-authority", Some(&snap.composition)),
    ] {
        let Some(block) = block else {
            errs.push(format!(
                "{S12_ADMISSION_TAG} `{label}` block missing — loss, not silence"
            ));
            continue;
        };
        for (key, expected) in [
            ("observation_mode", "shadow"),
            ("promotion_state", "candidate"),
            ("auto_promote", "forbidden"),
        ] {
            if block.get(key).and_then(|v| v.as_str()) != Some(expected) {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `{key}` must stay `{expected}`"
                ));
            }
        }
        if block.get("rollback_target").and_then(|v| v.as_str()) != Some(S12_ROLLBACK) {
            errs.push(format!(
                "{S12_ADMISSION_TAG} `{label}` `rollback_target` must stay the Java rollback target"
            ));
        }
        if label.starts_with("comparison") {
            if block.get("candidate_id").and_then(|v| v.as_str()) != Some(S12_FULLTEXT_CANDIDATE_ID)
            {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `candidate_id` must stay exactly `{S12_FULLTEXT_CANDIDATE_ID}`"
                ));
            }
            if block.get("import_eligible").and_then(|v| v.as_bool()) != Some(false) {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `import_eligible` must stay false"
                ));
            }
            for (key, expected) in [
                ("production_route", "forbidden"),
                ("graph_mutation", "forbidden"),
            ] {
                if block.get(key).and_then(|v| v.as_str()) != Some(expected) {
                    errs.push(format!(
                        "{S12_ADMISSION_TAG} `{label}` `{key}` must stay `{expected}`"
                    ));
                }
            }
            let lanes = s12_seq_strs(&block["admission"]["independently_selectable_lanes"]);
            if lanes != expected_lanes {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `admission.independently_selectable_lanes` must stay exactly `[header, references]`, got {lanes:?}"
                ));
            }
        } else {
            if block.get("import_eligible").and_then(|v| v.as_bool()) != Some(false) {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `import_eligible` must stay false"
                ));
            }
            for (key, expected) in [
                ("production_route", "forbidden"),
                ("graph_mutation", "forbidden"),
            ] {
                if block.get(key).and_then(|v| v.as_str()) != Some(expected) {
                    errs.push(format!(
                        "{S12_ADMISSION_TAG} `{label}` `{key}` must stay `{expected}`"
                    ));
                }
            }
            let lanes = s12_seq_strs(&block["admission"]["independently_selectable"]);
            if lanes != expected_lanes {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `admission.independently_selectable` must stay exactly `[header, references]`, got {lanes:?}"
                ));
            }
            if block["admission"]["pooled_winner"].as_str() != Some("forbidden") {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `admission.pooled_winner` must stay forbidden"
                ));
            }
            if block["admission"]["terminal_receipt_is_pass"].as_bool() != Some(false) {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `admission.terminal_receipt_is_pass` must stay false"
                ));
            }
        }
        // Named denominators: hub 20 vs 21 stays a named gap, citation
        // names never conflate with the biblStruct count.
        for (field, expected) in [
            ("date_total", 173_i64),
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
            if block["source_evidence"]["s12_named_denominators"][field].as_i64() != Some(expected)
            {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `s12_named_denominators.{field}` must stay {expected}"
                ));
            }
        }
        let hub = &block["source_evidence"]["s12_named_denominators"]["java_lift_same_n"];
        if hub["hub_native_accepted_header"].as_i64() != Some(20)
            || hub["hub_java_header_raw"].as_i64() != Some(21)
        {
            errs.push(format!(
                "{S12_ADMISSION_TAG} `{label}` hub gap must stay native-accepted 20 vs Java-raw 21"
            ));
        }
        if hub["citation_names_not_biblstruct"].as_bool() != Some(true) {
            errs.push(format!(
                "{S12_ADMISSION_TAG} `{label}` `citation_names_not_biblstruct` must stay boolean true"
            ));
        }
        if hub["java_header_authors_equals"].as_str() != Some("header_raw_total")
            || hub["java_biblstruct_equals"].as_str() != Some("references_total")
        {
            errs.push(format!(
                "{S12_ADMISSION_TAG} `{label}` Java same-n bindings must stay `header_raw_total` / `references_total`"
            ));
        }
        for key in [
            "pooling_accepted_with_raw",
            "pooling_citation_names_with_references",
        ] {
            if hub[key].as_str() != Some("forbidden") {
                errs.push(format!(
                    "{S12_ADMISSION_TAG} `{label}` `{key}` must stay forbidden — no pooled denominator"
                ));
            }
        }
    }
    // Composition authority receipts for the two admitted lanes stay
    // S12-produced, passing, and Java-fallback-bound.
    for lane in ["header", "references"] {
        let receipt = &snap.composition["component_receipts"][lane];
        if receipt["producer"].as_str() != Some("S12")
            || receipt["disposition"].as_str() != Some("passed")
            || receipt["evidence_state"].as_str() != Some("passing")
            || receipt["fallback_target"].as_str() != Some(S12_ROLLBACK)
        {
            errs.push(format!(
                "{S12_ADMISSION_TAG} composition `component_receipts.{lane}` must stay S12/passed/passing with Java fallback"
            ));
        }
    }
    errs
}

#[test]
fn s12_wave_admits_single_named_candidate_with_full_receipts() {
    let snap = load_s12_admission();
    let errs = validate_s12_wave_admission(&snap);
    assert!(
        errs.is_empty(),
        "S12 wave-admission DATA violations:\n{}",
        errs.join("\n")
    );
    // The live DATA identity binds the pinned constant — a renamed or
    // prefixed candidate never admits.
    let live_id = snap.comparison["native_header_reference_composition_shadow"]["candidate_id"]
        .as_str()
        .unwrap_or_default();
    assert_eq!(
        live_id, S12_CANDIDATE_ID,
        "S12: live candidate_id `{live_id}` drifted from the admitted contour"
    );
}

#[test]
fn s12_rejects_wave_admission_drift_on_synthetic_clones() {
    // Wave promotes a lane: this wave promotes nothing.
    let mut wave = load_s12_admission();
    wave.registry["defaults"]["promotion"]["this_wave_promotes"] =
        serde_yaml::Value::String("header".into());
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&wave),
        "this_wave_promotes -> header",
    );
    // Silent deployment without an explicit decision.
    let mut deployed = load_s12_admission();
    deployed.registry["defaults"]["promotion"]["deployed"] = serde_yaml::Value::Bool(true);
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&deployed),
        "deployed -> true",
    );
    // Second non-candidate family smuggled past the one-family cap.
    let mut families = load_s12_admission();
    families.registry["defaults"]["promotion"]["max_non_candidate_families"] =
        serde_yaml::Value::Number(2.into());
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&families),
        "max_non_candidate_families -> 2",
    );
    // Promotion state flipped to promoted without a decision receipt.
    let mut promoted = load_s12_admission();
    promoted.comparison["native_header_reference_composition_shadow"]["promotion_state"] =
        serde_yaml::Value::String("promoted".into());
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&promoted),
        "promotion_state -> promoted",
    );
    // Explicit decision invented without a later user yes.
    let mut decided = load_s12_admission();
    decided.comparison["metrics"]["learning"]["ruvllm_ablation"]["explicit_decision"] =
        serde_yaml::Value::String("user-yes-2026-09".into());
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&decided),
        "explicit_decision invented",
    );
    // A required component receipt removed from admission.
    let mut removed = load_s12_admission();
    if let Some(seq) = removed.comparison["native_header_reference_composition_shadow"]
        ["component_receipts"]["required"]
        .as_sequence_mut()
    {
        seq.retain(|x| x.as_str() != Some("native_citation_shadow"));
    }
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&removed),
        "native_citation_shadow receipt removed",
    );
    // Body lane smuggled into the independently selectable set.
    let mut widened = load_s12_admission();
    if let Some(seq) =
        widened.composition["admission"]["independently_selectable"].as_sequence_mut()
    {
        seq.push(serde_yaml::Value::String("body".into()));
    }
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&widened),
        "body added to independently_selectable",
    );
    // M494/S01: observation lane smuggled into the composition selectable set.
    let mut docseg = load_s12_admission();
    if let Some(seq) = docseg.composition["admission"]["independently_selectable"].as_sequence_mut()
    {
        seq.push(serde_yaml::Value::String("document_segmentation".into()));
    }
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&docseg),
        "document_segmentation added to independently_selectable",
    );
    // M494/S01: observation lane smuggled into the fulltext shadow selectable.
    let mut ftseg = load_s12_admission();
    if let Some(seq) = ftseg.comparison["native_fulltext_composition_shadow"]["admission"]
        ["independently_selectable_lanes"]
        .as_sequence_mut()
    {
        seq.push(serde_yaml::Value::String("document_segmentation".into()));
    }
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&ftseg),
        "document_segmentation added to fulltext shadow selectable",
    );
    // Visual lane smuggled into the composition selectable lanes.
    let mut visual = load_s12_admission();
    if let Some(seq) = visual.comparison["native_header_reference_composition_shadow"]
        ["independently_selectable_lanes"]
        .as_sequence_mut()
    {
        seq.push(serde_yaml::Value::String("visual".into()));
    }
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&visual),
        "visual added to independently_selectable_lanes",
    );
    // Pooled denominator: accepted-with-raw pooling allowed.
    let mut pooled = load_s12_admission();
    pooled.comparison["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]
        ["java_lift_same_n"]["pooling_accepted_with_raw"] =
        serde_yaml::Value::String("allowed".into());
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&pooled),
        "pooling_accepted_with_raw allowed",
    );
    // Hub gap collapsed: native 20 rewritten to Java 21.
    let mut hub = load_s12_admission();
    hub.composition["source_evidence"]["s12_named_denominators"]["java_lift_same_n"]["hub_native_accepted_header"] =
        serde_yaml::Value::Number(21.into());
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&hub),
        "hub native 20 rewritten to 21",
    );
    // Citation names conflated with the biblStruct count.
    let mut bibl = load_s12_admission();
    bibl.comparison["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"]
        ["java_lift_same_n"]["citation_names_not_biblstruct"] = serde_yaml::Value::Bool(false);
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&bibl),
        "citation_names_not_biblstruct flipped to false",
    );
    // Rollback swapped off Java: no non-Java fallback admits.
    let mut rollback = load_s12_admission();
    rollback.comparison["native_header_reference_composition_shadow"]["rollback"]["header"] =
        serde_yaml::Value::String("da-parse.native@9.9.9".into());
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&rollback),
        "rollback.header swapped off Java",
    );
    // Graph write unlocked on the ablation freeze.
    let mut graph = load_s12_admission();
    graph.comparison["metrics"]["learning"]["ruvllm_ablation"]["graph_write"] =
        serde_yaml::Value::String("allowed".into());
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&graph),
        "ablation graph_write allowed",
    );
    // Observation mode lifted to production on the shadow contour.
    let mut production = load_s12_admission();
    production.comparison["native_header_reference_composition_shadow"]["observation_mode"] =
        serde_yaml::Value::String("production".into());
    negative(
        S12_ADMISSION_TAG,
        &validate_s12_wave_admission(&production),
        "observation_mode -> production",
    );
}

#[test]
fn document_segmentation_contract() {
    let contract: serde_yaml::Value = expect_loaded("data/parse/document_segmentation.yaml");
    assert_eq!(contract["schema_version"], "document-segmentation.v2");
    assert_eq!(
        contract["compatible_with"], "document-segmentation.v1",
        "v2 must stay explicitly backward compatible with v1"
    );
    assert_eq!(contract["policy_locks"]["import_eligible"], false);
    assert_eq!(contract["policy_locks"]["graph_mutation"], "forbidden");
    assert_eq!(contract["policy_locks"]["raw_payloads"], "forbidden");

    let regions = contract["regions"]["closed_labels"]
        .as_sequence()
        .expect("closed segmentation regions");
    assert_eq!(regions.len(), 9, "v2 closes the full nine-region family");
    assert!(regions.iter().all(|value| value.as_str().is_some()));
    assert_eq!(
        contract["regions"]["assignment_rule"],
        "exactly_one_region_per_admitted_token"
    );
    assert_eq!(
        contract["regions"]["order_rule"],
        "admitted_reading_order_preserved"
    );
    let furniture = contract["furniture"]["closed_subtypes"]
        .as_sequence()
        .expect("closed furniture subtypes");
    assert_eq!(furniture.len(), 5);
    assert_eq!(
        contract["furniture"]["excluded_from_body_reading_order"],
        true
    );
    let lane_ids = contract["lanes"]["closed_ids"]
        .as_sequence()
        .expect("closed lane ids");
    assert_eq!(lane_ids.len(), 3);
    assert_eq!(
        contract["lanes"]["required_ids"],
        contract["lanes"]["closed_ids"]
    );
    assert_eq!(contract["evaluation"]["independent_denominators"], true);
    assert_eq!(contract["evaluation"]["pooled_metrics"], "forbidden");
    assert_eq!(contract["evaluation"]["required_rows"], 81);
    assert_eq!(
        contract["evaluation"]["row_formula"],
        "3_documents_x_3_lanes_x_1_reviewed_genre_x_9_regions"
    );
    assert_eq!(contract["lanes"]["silent_skip"], "forbidden");
    assert_eq!(
        contract["source_bound_genres"]["unknown_label_behavior"],
        "fail_closed"
    );
    assert_eq!(
        contract["source_bound_genres"]["missing_coverage_behavior"],
        "fail_closed"
    );

    let forbidden = contract["oracle"]["forbidden_fields"]
        .as_sequence()
        .expect("oracle redaction fields");
    for field in [
        "raw_text",
        "pdf_bytes",
        "tei_xml",
        "page_image",
        "physical_path",
        "path",
        "hostname",
        "executable",
    ] {
        assert!(
            forbidden.iter().any(|value| value == field),
            "missing redaction field {field}"
        );
    }
    assert_eq!(contract["oracle"]["independent"], true);
    assert_eq!(
        contract["oracle"]["expected_rows_not_from_sut_helpers"],
        true
    );
    assert_eq!(
        contract["oracle"]["conservation"],
        "assigned_equals_admitted"
    );
    assert_eq!(contract["oracle"]["boundary_order"], "monotonic");

    let failures = contract["failure_taxonomy"]["values"]
        .as_sequence()
        .expect("closed segmentation failure taxonomy");
    assert_eq!(contract["failure_taxonomy"]["closed"], true);
    for value in [
        "unknown_region",
        "unknown_genre",
        "missing_oracle_row",
        "duplicate_oracle_key",
        "overlapping_range",
        "denominator_erosion",
        "pooled_metric",
        "digest_mismatch",
        "raw_payload_leakage",
    ] {
        assert!(
            failures.iter().any(|candidate| candidate == value),
            "missing failure value {value}"
        );
    }
}

#[test]
fn p5_shadow_join_data_contract_is_closed() {
    let contract: serde_yaml::Value = expect_loaded("data/parse/p5_shadow_join.yaml");
    assert_eq!(contract["schema_version"], "p5-shadow-join.v1");
    assert_eq!(
        contract["join_schema_version"],
        da_parse::P5_SHADOW_JOIN_SCHEMA_VERSION
    );
    assert_eq!(contract["join_mode"], "offline_shadow_report");
    assert_eq!(contract["production_authority"], "java_http");
    assert_eq!(contract["native_accountant"], "shadow_join_only");
    assert_eq!(contract["mismatch"], "fail_closed");
    assert_eq!(contract["empty_count_not_success"], true);
    assert_eq!(contract["import_eligible"], false);
    assert_eq!(contract["graph_mutation"], "forbidden");
    assert_eq!(contract["this_wave_promotes"], "none");
    assert_eq!(
        contract["required_legs"],
        serde_yaml::from_str::<serde_yaml::Value>("[java_p5, java_p2, native_body_sequence]")
            .unwrap()
    );
    assert_eq!(
        contract["pooling_forbidden_with"],
        serde_yaml::from_str::<serde_yaml::Value>("[P2, P6, P7]").unwrap()
    );
    assert_eq!(
        contract["report"]["independently_selectable"],
        serde_yaml::from_str::<serde_yaml::Value>("[header, references]").unwrap()
    );
    assert_eq!(contract["report"]["payload_free"], true);
    assert_eq!(contract["report"]["native_status_separate"], true);
    assert_eq!(contract["report"]["java_fallback_preserved"], true);
    for field in [
        "body_text",
        "tei_xml",
        "pdf_bytes",
        "section_text",
        "java_section_n_on_p5",
        "physical_paths",
    ] {
        assert!(
            contract["forbidden_report_fields"]
                .as_sequence()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some(field)),
            "missing forbidden report field {field}"
        );
    }
    for leg in ["java_p5", "java_p2", "native_body_sequence"] {
        assert!(
            contract["required_legs"]
                .as_sequence()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some(leg)),
            "required leg {leg} eroded"
        );
    }
}

#[test]
fn p5_cross_stage_semantic_data_contract_is_closed() {
    let contract: serde_yaml::Value = expect_loaded("data/parse/p5_cross_stage.yaml");
    assert_eq!(contract["schema_version"], "p5-cross-stage.v1");
    assert_eq!(
        contract["join_schema_ref"],
        da_parse::P5_SHADOW_JOIN_SCHEMA_VERSION
    );
    assert_eq!(
        contract["report_schema_version"],
        da_parse::CROSS_STAGE_SEMANTIC_SCHEMA_VERSION
    );
    assert_eq!(contract["mode"], "offline_shadow_report");
    assert_eq!(contract["production_authority"], "java_http");
    assert_eq!(contract["native_accountant"], "shadow_join_only");
    assert_eq!(
        contract["required_legs"],
        serde_yaml::from_str::<serde_yaml::Value>(
            "[p5_shadow_join_report, java_p4, java_p6, java_p7]"
        )
        .unwrap()
    );
    assert_eq!(
        contract["optional_identity_siblings"],
        serde_yaml::from_str::<serde_yaml::Value>(
            "[native_visual_sequence, native_body_accounting]"
        )
        .unwrap()
    );
    assert_eq!(
        contract["pooling_forbidden_with"],
        serde_yaml::from_str::<serde_yaml::Value>("[P2, P4, P5, P6, P7]").unwrap()
    );
    assert_eq!(contract["order_check"], "units_monotonic_page_token");
    assert_eq!(
        contract["conservation"]["native_bucket_sum_equals_admitted"],
        true
    );
    assert_eq!(
        contract["conservation"]["cross_oracle_equality_is_not_success"],
        true
    );
    assert_eq!(contract["empty_count_not_success"], true);
    assert_eq!(contract["import_eligible"], false);
    assert_eq!(contract["this_wave_promotes"], "none");
    assert_eq!(
        contract["independently_selectable"],
        serde_yaml::from_str::<serde_yaml::Value>("[header, references]").unwrap()
    );
    assert_eq!(
        contract["body_markers_policy"],
        "missing_bucket_is_zero_not_p7"
    );
    for field in [
        "pooled_n",
        "body_text",
        "tei_xml",
        "pdf_bytes",
        "p5_success",
        "p6_success",
        "p7_success",
        "java_section_n_on_p5",
    ] {
        assert!(
            contract["forbidden_report_fields"]
                .as_sequence()
                .unwrap()
                .iter()
                .any(|value| value.as_str() == Some(field)),
            "missing forbidden field {field}"
        );
    }
    for field in [
        "java_p4.java_citation_n",
        "java_p6.java_figure_n",
        "java_p6.java_table_n",
        "java_p6.java_formula_n",
        "java_p7.java_callout_n",
        "java_p7.unresolved_n",
        "native_buckets.*",
        "native_visual.figure.metrics.*",
        "native_visual.table.metrics.*",
        "native_visual.formula.metrics.*",
    ] {
        assert!(
            contract["unpooled_fields"]
                .as_sequence()
                .unwrap()
                .iter()
                .any(|value| value.as_str() == Some(field)),
            "missing unpooled field {field}"
        );
    }
}

#[test]
fn cross_stage_semantic_policy_exact_map_is_closed() {
    let cross: serde_yaml::Value = expect_loaded("data/parse/p5_cross_stage.yaml");
    assert_eq!(cross["production_authority"], "java_http");
    assert_eq!(cross["native_accountant"], "shadow_join_only");
    assert_eq!(cross["empty_count_not_success"], true);
    assert_eq!(cross["import_eligible"], false);
    assert_eq!(cross["this_wave_promotes"], "none");
    assert_eq!(
        cross["independently_selectable"],
        serde_yaml::from_str::<serde_yaml::Value>("[header, references]").unwrap()
    );
    assert_eq!(
        cross["required_legs"],
        serde_yaml::from_str::<serde_yaml::Value>(
            "[p5_shadow_join_report, java_p4, java_p6, java_p7]"
        )
        .unwrap()
    );
    assert_eq!(
        cross["optional_identity_siblings"],
        serde_yaml::from_str::<serde_yaml::Value>(
            "[native_visual_sequence, native_body_accounting]"
        )
        .unwrap()
    );

    let shadow: serde_yaml::Value = expect_loaded("data/parse/p5_shadow_join.yaml");
    assert_eq!(
        shadow["quarantine_tokens"],
        serde_yaml::from_str::<serde_yaml::Value>(
            "[join_p5_shadow, check_cross_stage_semantic_invariants]"
        )
        .unwrap()
    );
    assert_eq!(
        shadow["boundaries"],
        serde_yaml::from_str::<serde_yaml::Value>(
            "[crates/da-cli/src/source_cli.rs, crates/da-ports/src/parser.rs]"
        )
        .unwrap()
    );
    assert_eq!(shadow["production_authority"], "java_http");
    assert_eq!(shadow["native_accountant"], "shadow_join_only");
    assert_eq!(shadow["empty_count_not_success"], true);
    assert_eq!(shadow["import_eligible"], false);
    assert_eq!(shadow["this_wave_promotes"], "none");
    assert_eq!(
        shadow["report"]["independently_selectable"],
        serde_yaml::from_str::<serde_yaml::Value>("[header, references]").unwrap()
    );

    let registry: serde_yaml::Value = expect_loaded("data/parse/grobid_compatibility.yaml");
    let p5 = registry["capabilities"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|cap| cap["id"].as_str() == Some("rest.processFulltextDocument"))
        .expect("P5/fulltext capability");
    assert_eq!(p5["baseline"], "java-grobid-http@0.9.0-crf");
    assert_eq!(p5["rollback_target"], "java-grobid-http@0.9.0-crf");
    let policy = &cross;
    assert_eq!(policy["schema_version"], "p5-cross-stage.v1");
    assert_eq!(policy["mode"], "offline_shadow_report");
    assert_eq!(policy["production_authority"], "java_http");
    assert_eq!(policy["native_accountant"], "shadow_join_only");
    assert_eq!(policy["empty_count_not_success"], true);
    assert_eq!(policy["import_eligible"], false);
    assert_eq!(policy["this_wave_promotes"], "none");
}

#[test]
fn native_body_accounting_contract() {
    let contract: serde_yaml::Value = expect_loaded("data/parse/native_body_accounting.yaml");
    let evidence = load_json_value("artifacts/parse_compat/java_grobid_body_taxonomy_source.json");
    let evidence_yaml = serde_yaml::to_value(&evidence).expect("evidence converts to YAML value");

    assert_eq!(contract["schema_version"], "native-body-accounting.v1");
    assert_eq!(
        contract["source_authority"]["evidence_artifact"],
        "artifacts/parse_compat/java_grobid_body_taxonomy_source.json"
    );
    assert_eq!(
        contract["source_authority"]["evidence_schema_version"],
        evidence_yaml["schema_version"]
    );
    assert_eq!(
        contract["source_authority"]["vendor_revision"],
        evidence_yaml["vendor_revision"]
    );
    assert_eq!(
        contract["source_authority"]["files"],
        evidence_yaml["files"]
    );
    assert_eq!(
        contract["segmentation_regions"]["labels"],
        evidence_yaml["segmentation_labels"]
    );
    assert_eq!(
        contract["fulltext_labels"]["labels"],
        evidence_yaml["fulltext_labels"]
    );
    assert_eq!(
        contract["entry_points"]["orchestration_method_lines"],
        evidence_yaml["orchestration_method_lines"]
    );

    let locks = &contract["policy_locks"];
    for (key, value) in [
        ("observation_mode", "shadow"),
        ("auto_promote", "forbidden"),
        ("production_authority", "java-grobid-http@0.9.0-crf"),
    ] {
        assert_eq!(locks[key], value, "policy lock {key}");
    }
    assert_eq!(locks["import_eligible"], false);
    assert_eq!(locks["graph_mutation"], "forbidden");
    assert_eq!(locks["raw_payloads"], "forbidden");

    let families = &contract["families"];
    assert_eq!(families["closed"], true);
    assert_eq!(families["pooling"], "forbidden");
    for family in [
        "body_blocks",
        "body_markers",
        "visual_content",
        "annex",
        "references",
        "page_furniture",
    ] {
        assert_eq!(
            families[family]["pooling"], "forbidden",
            "family {family} must not pool"
        );
    }
    assert_eq!(
        families["body_blocks"]["fulltext_labels"],
        serde_yaml::from_str::<serde_yaml::Value>("[paragraph, item, section]").unwrap()
    );
    assert_eq!(
        families["body_markers"]["fulltext_labels"],
        serde_yaml::from_str::<serde_yaml::Value>(
            "[citation_marker, figure_marker, table_marker, equation_marker, other]"
        )
        .unwrap()
    );
    assert_eq!(
        families["visual_content"]["fulltext_labels"],
        serde_yaml::from_str::<serde_yaml::Value>("[figure, table, equation, equation_label]")
            .unwrap()
    );
    assert_eq!(
        families["annex"]["fulltext_labels"],
        serde_yaml::Value::Sequence(vec![])
    );

    assert_eq!(contract["source_corpus"]["corpus_n"], 3);
    assert_eq!(contract["source_corpus"]["page_sum"], 202);
    assert_eq!(contract["denominators"]["known"]["documents"], 3);
    assert_eq!(contract["denominators"]["known"]["pages"], 202);
    assert_eq!(contract["denominators"]["known"]["lanes"], 3);
    for family in [
        "body_blocks",
        "body_markers",
        "visual_content",
        "annex",
        "references",
        "page_furniture",
    ] {
        assert_eq!(
            contract["denominators"]["pending_body_counts"][family],
            "pending_independent_oracle"
        );
    }

    let failures = contract["failure_taxonomy"]["values"]
        .as_sequence()
        .expect("closed failure values");
    for value in [
        "unknown_label",
        "missing_identity",
        "checksum_mismatch",
        "duplicate_assignment",
        "overlap",
        "denominator_erosion",
        "pooled_metric",
        "raw_payload",
        "promotion_attempt",
        "graph_attempt",
    ] {
        assert!(
            failures.iter().any(|v| v == value),
            "missing failure value {value}"
        );
    }
}

/// D358: GROBID model assets are not redistributable. Model bytes must never be
/// bundled into this repository; the native parser reads them at runtime from a
/// configured local resource root instead.
///
/// The only model-format files allowed in-tree are hand-authored synthetic
/// fixtures under a test fixture directory of the parse plane: they exist so CI
/// can decode a frozen model without touching GROBID bytes, and they are
/// provably not GROBID-derived.
///
/// S04/T04 added the `da-parse` fixture directory: the bibliography sequence
/// contract decodes `*_synthetic.wapiti` files that live beside the rows they
/// are decoded with. The exemption stays a closed list of fixture directories —
/// a `.wapiti` anywhere else in the live tree is still refused.
const SYNTHETIC_MODEL_FIXTURE_DIRS: [&str; 2] = [
    "crates/da-adapters/tests/fixtures",
    "crates/da-parse/tests/fixtures",
];

/// Extensions of the Wapiti CRF model format GROBID ships. `*.wapiti` is the
/// authoritative GROBID model filename (`models/<family>/model.wapiti`).
const WAPITI_MODEL_EXTENSION: &str = "wapiti";

/// Generic model/weight container extensions. They are tolerated elsewhere in
/// the repository (the frozen python-era archive carries `payload.bin` cache
/// blobs), but the live `data/` asset tree is YAML/JSON only, so any of these
/// appearing there signals bundled model or weight bytes.
const DATA_TREE_FORBIDDEN_EXTENSIONS: [&str; 4] = ["bin", "model", "crf", "tpl"];

/// Returns one error per path that violates the D358 bundling ban.
fn model_byte_bundle_errs(repo_relative_paths: &[String]) -> Vec<String> {
    let mut errs = Vec::new();
    for path in repo_relative_paths {
        let extension = Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let synthetic_fixture = SYNTHETIC_MODEL_FIXTURE_DIRS
            .iter()
            .any(|directory| path.starts_with(directory));
        if extension == WAPITI_MODEL_EXTENSION && !synthetic_fixture {
            errs.push(format!(
                "bundled GROBID model bytes are not redistributable (D358): {path}"
            ));
            continue;
        }
        if DATA_TREE_FORBIDDEN_EXTENSIONS.contains(&extension.as_str()) && path.starts_with("data/")
        {
            errs.push(format!(
                "model or weight bytes may not live in the data tree (D358): {path}"
            ));
        }
    }
    errs
}

/// Recursively collects repo-relative file paths below `dir`, skipping build and
/// VCS directories. Kept dependency-free on purpose: this test must run wherever
/// `cargo test` runs.
fn collect_files(root: &Path, relative: &str, out: &mut Vec<String>) {
    let dir = root.join(relative);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "target" || name == ".git" || name == "__pycache__" {
            continue;
        }
        let child = format!("{relative}/{name}");
        if entry.path().is_dir() {
            collect_files(root, &child, out);
        } else {
            out.push(child);
        }
    }
}

#[test]
fn d358_model_bytes_are_never_bundled_into_the_live_tree() {
    let root = repo_root();
    let mut files = Vec::new();
    for live in ["data", "crates", "doc", "scripts"] {
        collect_files(&root, live, &mut files);
    }
    assert!(
        files.len() > 100,
        "live tree walk must actually see files, saw {}",
        files.len()
    );

    let errs = model_byte_bundle_errs(&files);
    assert!(
        errs.is_empty(),
        "D358 bundling ban violated:\n{}",
        errs.join("\n")
    );
}

#[test]
fn d358_bundling_ban_predicate_discriminates_synthetic_fixtures() {
    let allowed = vec![
        "crates/da-adapters/tests/fixtures/synthetic_model.wapiti".to_string(),
        "crates/da-adapters/tests/fixtures/provider_pdfalto_page.json".to_string(),
        "crates/da-parse/tests/fixtures/native_bibliography_reference_synthetic.wapiti".to_string(),
        "crates/da-parse/tests/fixtures/native_bibliography_citation_synthetic.wapiti".to_string(),
        "crates/da-domain/src/scalar_crf.rs".to_string(),
        "data/parse/grobid_model_rights.yaml".to_string(),
        "archive/artifacts-legacy/catalog_sweep/pipeline_cache/parse/payload.bin".to_string(),
    ];
    assert!(
        model_byte_bundle_errs(&allowed).is_empty(),
        "synthetic fixtures and archived cache blobs must not be flagged"
    );

    let rejected = vec![
        "data/parse/models/header/model.wapiti".to_string(),
        "crates/da-adapters/src/grobid_home/lexicon/lexicon.wapiti".to_string(),
        "crates/da-parse/src/model.wapiti".to_string(),
        "crates/da-cli/tests/fixtures/model.wapiti".to_string(),
        "doc/models/model.wapiti".to_string(),
        "data/parse/weights.crf".to_string(),
        "data/parse/model.bin".to_string(),
    ];
    let errs = model_byte_bundle_errs(&rejected);
    assert_eq!(
        errs.len(),
        rejected.len(),
        "every bundled-model path must be rejected, got {errs:?}"
    );
}
