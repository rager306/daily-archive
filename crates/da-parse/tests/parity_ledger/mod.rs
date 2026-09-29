//! Fail-closed validator for the GROBID Java -> Rust parity ledger.
//!
//! The ledger is DATA: `data/parse/grobid_compatibility.yaml::parity_ledger`
//! (M511-j7wz2f/S07, H21). This module holds the loader and the invariant
//! checks; `tests/grobid_parity_ledger_contract.rs` drives them as separate
//! tests so a regression names the exact rule that broke.
//!
//! Deliberate non-goals:
//!
//! * The validator never reads `/root/vendor-source/grobid`. Verifying the
//!   pinned sha256 against the vendor tree is S07/T06, because a test that
//!   needs the checkout is a test that fails wherever the checkout is absent.
//!   What this module does check is DATA -> DATA: a capture-pinned hash must
//!   equal the hash stored in the capture artifact it names.
//! * The validator never mutates the carrier. It is read-only and additive.
//!
//! Invariants implemented (a)-(h) from the slice plan:
//!
//! (a) the row id set equals the capability id set, each exactly once;
//! (b) each row's `scope` equals that capability's `product_scope`;
//! (c) every `full` row resolves a Java source + non-empty site, each Rust
//!     path exists and contains the symbol as a separate identifier, and the
//!     test path is a `crates/**/tests/` target containing `fn <name>`. Every
//!     repo-relative path the ledger names (`rust.path`, `test.path`,
//!     `error_semantics.source`, `capture.artifact`) is also contained: an
//!     absolute path, a `..` component, or a resolved path outside the
//!     workspace root is rejected by name instead of read (Q3/S07 gap 1,
//!     matching the `acceptance-path-escape` precedent in data_contracts.rs);
//! (d) every Java source sha256 is 64 lowercase hex; capture-pinned sources
//!     equal the capture; both revisions are non-empty;
//! (e) every row license resolves to a `component_id` in the inventory;
//! (f) `not_claimed_absent` rows carry a scope-keyed reason and no links;
//! (g) no absent row claims parity, and no `required` capability is absent;
//! (h) the frozen denominators match what the rows actually contain.
//!
//! The fourth link (M513-r7dg4z/S03, D471) adds a fail-closed measurement
//! projection: every `required` row binds a typed reference to the S02
//! Java-relative corpus parity, and no other row may claim one. The binding is
//! checked twice: `check_measurement_coverage` is pure DATA -> DATA (vocabularies
//! plus a closed class cover), and `check_measurement_identity` re-hashes the
//! pinned artifact and frozen tables and reconciles the mirrored tokens. This is
//! a coverage inventory, not a correctness proof (MEM1732): correctness stays
//! with the third link and the S02 oracle.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

/// Token a `pinned_by` command uses instead of repeating the physical vendor
/// root on every row. The root itself is declared once as
/// `parity_ledger.java_source_vendor_root` (Q3/S07 gap 2: redaction hygiene).
pub const VENDOR_ROOT_TOKEN: &str = "$GROBID_VENDOR_ROOT";

pub const CONTRACT_ID: &str = "grobid-java-rust-parity-ledger.v1";
pub const COMPATIBILITY_DOC: &str = "data/parse/grobid_compatibility.yaml";
pub const LICENSE_INVENTORY: &str = "data/parse/license_inventory.yaml";
pub const SCHEMA_DOC: &str = "data/parse/SCHEMA.yaml";

pub const PARITY_FULL: &str = "full";
pub const PARITY_PARTIAL: &str = "partial";
pub const PARITY_NOT_APPLICABLE: &str = "not_applicable_no_java_donor";
pub const PARITY_NOT_CLAIMED: &str = "not_claimed_absent";

/// Measurement states (M513-r7dg4z/S03, D471). A required row either binds the
/// corpus parity (`measured_by_corpus_parity`) or names a typed reason why the
/// measured lane does not cover it (`outside_corpus_parity`).
pub const MEASURED_BY_CORPUS_PARITY: &str = "measured_by_corpus_parity";
pub const OUTSIDE_CORPUS_PARITY: &str = "outside_corpus_parity";
pub const AGGREGATE_KIND_DECLARED_POPULATION_COUNT: &str = "declared_population_count";
pub const POOLED_QUALITY_CLAIM_FORBIDDEN: &str = "forbidden";

/// Absent-capability fate registry (M513-r7dg4z/S05, D478/D479). A recorded
/// fate is a decision the scope-derived `Row.reason` cannot express, so it
/// lives in its own block where the Rust name `reason` is free.
pub const DISPOSITION_OUT_OF_PRODUCT_SCOPE: &str = "out_of_product_scope";
pub const DISPOSITION_IN_HORIZON_PROJECT_NATIVE: &str = "in_horizon_project_native";
/// The one reason a named gap may carry: no Java donor exists, so parity is
/// explicitly not claimed rather than silently unimplemented.
pub const NAMED_GAP_REASON_NO_DONOR: &str = "project_native_no_grobid_donor";

// --------------------------------------------------------------------------
// Data model
// --------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CompatibilityDocument {
    pub capabilities: Vec<Capability>,
    pub parity_ledger: ParityLedger,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Capability {
    pub id: String,
    pub state: String,
    pub product_scope: String,
    pub license: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ParityLedger {
    pub contract_id: String,
    pub authority: String,
    pub observed_checkout_revision: String,
    pub capture_declared_revision: String,
    pub revision_discrepancy_note: String,
    pub scope_vocabulary: Vec<String>,
    pub parity_level_vocabulary: Vec<String>,
    pub absent_reason_vocabulary: BTreeMap<String, String>,
    pub error_semantics_policy: String,
    /// Documented physical vendor root that `pinned_by` commands refer to via
    /// [`VENDOR_ROOT_TOKEN`]. Declared once instead of repeating an absolute
    /// path on every java source.
    #[serde(default)]
    pub java_source_vendor_root: Option<String>,
    pub java_sources: Vec<JavaSource>,
    pub rows: Vec<Row>,
    pub named_gaps: Vec<NamedGap>,
    pub denominators: Denominators,
    pub locks: Locks,
    /// DATA-declared negative fixtures for the mutation matrix (S07/T03).
    /// Each entry names a stable mutation id and the exact validator error
    /// token that must detect it; `grobid_parity_ledger_mutations.rs` owns the
    /// appliers and fails when the applied registry diverges from this list.
    #[serde(default)]
    pub mutations: Vec<Mutation>,
    /// The fourth link (S03, D471): the pinned S02 measurement every required
    /// row refers to. `#[serde(default)]` keeps the mutation matrix able to
    /// deserialize a clone whose sub-block was mutated away.
    #[serde(default)]
    pub parity_measurement: Option<ParityMeasurement>,
    /// Typed refusal for every named gap (S03, D471): a named gap is never a
    /// required row and always carries a kind, a decision reference and a
    /// residual owner.
    #[serde(default)]
    pub named_gap_disposition: Option<NamedGapDisposition>,
    /// Closed vocabulary the absent-disposition registry is judged against
    /// (S05, D478/D479). `#[serde(default)]` is mandatory: without it the
    /// mutation matrix cannot deserialize a clone whose sub-block was tampered
    /// away, and the new DATA would be silently ignored rather than judged.
    #[serde(default)]
    pub absent_disposition_vocabulary: Option<AbsentDispositionVocabulary>,
    /// The recorded fate of every `state: absent` row plus every named gap
    /// (S05, D478/D479). Keys are capability ids.
    #[serde(default)]
    pub absent_dispositions: BTreeMap<String, AbsentDisposition>,
    /// Ordered subset of the `in_horizon_project_native` keys (S05, D478/D479).
    #[serde(default)]
    pub horizon_order: Vec<String>,
}

/// The pinned S02 measurement: two content-addressed files, the identity and
/// release binding, the closed class/state/reason vocabularies, the mirrored
/// class-parity declaration, and the D442/D469 reconciliation tokens.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ParityMeasurement {
    pub artifact: MeasurementArtifactRef,
    pub frozen_tables: FrozenTablesRef,
    pub identity_digest: String,
    pub declared_release: DeclaredRelease,
    pub claim_scope: String,
    pub aggregation: String,
    pub aggregate_kind: String,
    pub pooled_quality_claim: String,
    pub classes: Vec<String>,
    pub state_vocabulary: Vec<String>,
    pub outside_reason_vocabulary: Vec<String>,
    pub class_parity_declaration: ClassParityDeclaration,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MeasurementArtifactRef {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FrozenTablesRef {
    pub path: String,
    pub sha256: String,
    /// Internal chain: the artifact bytes these frozen tables were derived from.
    pub artifact_sha256: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeclaredRelease {
    pub label: String,
    pub image_digest: String,
    pub rollback_label: String,
    pub label_divergence_item: String,
}

/// Mirror of the artifact's `class_parity_declaration`, compared token for
/// token against the pinned artifact by `check_measurement_identity`.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct ClassParityDeclaration {
    pub aggregation: String,
    pub claim_scope: String,
    pub denominator: String,
    pub extra: String,
    pub f1: String,
    pub missing: String,
    pub numerator: String,
    pub precision: String,
    pub recall: String,
    pub schema_version: String,
    pub source_consumed_classes: Vec<String>,
    pub source_consumed_rule: String,
    pub zero_zero_denominator: String,
}

/// Per-required-row measurement link. `#[serde(default)]` on the parent keeps
/// the mutation matrix deserializing clones whose row link was removed.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RowMeasurement {
    pub state: String,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NamedGapDisposition {
    pub required_named_gaps: u64,
    pub kind_vocabulary: Vec<String>,
    #[serde(default)]
    pub gaps: Vec<NamedGapRefusal>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NamedGapRefusal {
    pub capability: String,
    pub kind: String,
    pub accepted_as_residual: bool,
    pub decision_ref: String,
    pub residual_owner: String,
}

/// Closed vocabulary for [`AbsentDisposition`]: the disposition classes and the
/// reasons an absent capability may be assigned (S05, D478).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AbsentDispositionVocabulary {
    pub dispositions: Vec<String>,
    pub reasons: Vec<String>,
}

/// A recorded fate for one absent capability or named gap (S05, D478/D479).
/// `rationale` states the product fact; it must never be a restatement of the
/// capability's `product_scope`.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AbsentDisposition {
    pub disposition: String,
    pub reason: String,
    pub rationale: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Mutation {
    pub id: String,
    pub expected_detection: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JavaSource {
    pub id: String,
    pub logical_path: String,
    pub sha256: String,
    pub pinned_by: String,
    #[serde(default)]
    pub capture: Option<Capture>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Capture {
    pub artifact: String,
    pub pointer: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Row {
    pub capability: String,
    pub scope: String,
    pub state: String,
    pub parity: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub java: Option<JavaLink>,
    #[serde(default)]
    pub java_secondary: Vec<JavaLink>,
    #[serde(default)]
    pub rust: Vec<RustLink>,
    #[serde(default)]
    pub test: Option<TestLink>,
    #[serde(default)]
    pub error_semantics: Option<ErrorSemantics>,
    /// The fourth link (S03, D471), present iff `scope == required`.
    #[serde(default)]
    pub measurement: Option<RowMeasurement>,
    pub license: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JavaLink {
    pub source_id: String,
    pub site: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RustLink {
    pub path: String,
    pub symbol: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TestLink {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ErrorSemantics {
    pub source: String,
    pub vocabulary: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NamedGap {
    pub capability: String,
    pub reason: String,
    pub next: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Denominators {
    pub capabilities: u64,
    pub scope: BTreeMap<String, u64>,
    pub implemented: u64,
    pub absent: u64,
    pub parity: BTreeMap<String, u64>,
    pub named_gaps: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Locks {
    pub import_eligible: bool,
    pub production_route: String,
    pub graph_write: String,
    pub auto_promote: String,
    pub deployed: bool,
}

#[derive(Debug, Deserialize)]
pub struct SchemaDoc {
    pub ledger_document: SchemaLedger,
}

#[derive(Debug, Deserialize)]
pub struct SchemaLedger {
    pub allowed_product_scope: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct LicenseDoc {
    pub components: Vec<LicenseComponent>,
}

#[derive(Debug, Deserialize)]
pub struct LicenseComponent {
    pub component_id: String,
}

// --------------------------------------------------------------------------
// Loading
// --------------------------------------------------------------------------

/// Workspace root: `crates/da-parse` -> `crates` -> repo root.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root above crates/da-parse")
        .to_path_buf()
}

pub fn load(root: &Path) -> CompatibilityDocument {
    let raw = std::fs::read_to_string(root.join(COMPATIBILITY_DOC))
        .expect("grobid_compatibility.yaml readable");
    serde_yaml::from_str(&raw).expect("grobid_compatibility.yaml parses")
}

pub fn load_schema(root: &Path) -> SchemaDoc {
    let raw = std::fs::read_to_string(root.join(SCHEMA_DOC)).expect("SCHEMA.yaml readable");
    serde_yaml::from_str(&raw).expect("SCHEMA.yaml parses")
}

pub fn load_license(root: &Path) -> LicenseDoc {
    let raw = std::fs::read_to_string(root.join(LICENSE_INVENTORY))
        .expect("license_inventory.yaml readable");
    serde_yaml::from_str(&raw).expect("license_inventory.yaml parses")
}

// --------------------------------------------------------------------------
// Identifier occurrence
// --------------------------------------------------------------------------

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// True when `ident` occurs in `text` as a complete identifier token, so
/// `VersionProbe` does not match `VersionProbeError` and vice versa.
pub fn contains_identifier(text: &str, ident: &str) -> bool {
    if ident.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    text.match_indices(ident).any(|(start, _)| {
        let end = start + ident.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_ident_byte(bytes[end]);
        before_ok && after_ok
    })
}

/// True when the source text contains a `fn <name>` definition (any
/// visibility, `async` or not) for `name`.
pub fn contains_fn_definition(text: &str, name: &str) -> bool {
    let needle = format!("fn {name}");
    let bytes = text.as_bytes();
    text.match_indices(&needle).any(|(start, _)| {
        let end = start + needle.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_ident_byte(bytes[end]);
        before_ok && after_ok
    })
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// True when a ledger-declared path can leave the repository root before any
/// resolution: it is absolute, or it carries a `..` component.
fn escapes_repo_root(relative: &str) -> bool {
    let path = Path::new(relative);
    path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::RootDir | Component::Prefix(_) | Component::ParentDir
            )
        })
}

fn read_required(root: &Path, relative: &str, errors: &mut Vec<String>) -> Option<String> {
    if escapes_repo_root(relative) {
        errors.push(format!(
            "{relative}: absolute path or `..` component escapes the repository root"
        ));
        return None;
    }
    let joined = root.join(relative);
    // A symlinked intermediate directory can still leave the root, so check
    // the resolved path whenever both ends exist. A missing file falls through
    // to the read below, which records the read failure instead.
    let resolved_outside_root = root
        .canonicalize()
        .ok()
        .zip(joined.canonicalize().ok())
        .is_some_and(|(root_canonical, joined_canonical)| {
            !joined_canonical.starts_with(root_canonical)
        });
    if resolved_outside_root {
        errors.push(format!(
            "{relative}: resolved path escapes the repository root"
        ));
        return None;
    }
    match std::fs::read_to_string(&joined) {
        Ok(text) => Some(text),
        Err(err) => {
            errors.push(format!("cannot read {relative}: {err}"));
            None
        }
    }
}

fn json_pointer<'a>(value: &'a serde_json::Value, pointer: &str) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in pointer.split('/').filter(|segment| !segment.is_empty()) {
        current = current.get(segment)?;
    }
    Some(current)
}

// --------------------------------------------------------------------------
// Checks
// --------------------------------------------------------------------------

pub fn check_contract_identity(doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let ledger = &doc.parity_ledger;
    if ledger.contract_id != CONTRACT_ID {
        errors.push(format!(
            "contract_id is `{}`, expected `{CONTRACT_ID}`",
            ledger.contract_id
        ));
    }
    for (label, text) in [
        ("authority", &ledger.authority),
        (
            "observed_checkout_revision",
            &ledger.observed_checkout_revision,
        ),
        (
            "capture_declared_revision",
            &ledger.capture_declared_revision,
        ),
        (
            "revision_discrepancy_note",
            &ledger.revision_discrepancy_note,
        ),
        ("error_semantics_policy", &ledger.error_semantics_policy),
    ] {
        if text.trim().is_empty() {
            errors.push(format!("{label} is empty"));
        }
    }
    errors
}

/// (a) row id set equals the capability id set, each exactly once.
pub fn check_row_identity(doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let caps: BTreeSet<&str> = doc.capabilities.iter().map(|cap| cap.id.as_str()).collect();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for row in &doc.parity_ledger.rows {
        *seen.entry(row.capability.as_str()).or_insert(0) += 1;
    }
    for (id, count) in &seen {
        if *count != 1 {
            errors.push(format!(
                "capability `{id}` appears {count} times, expected 1"
            ));
        }
    }
    for id in &caps {
        if !seen.contains_key(id) {
            errors.push(format!("capability `{id}` has no parity row"));
        }
    }
    for id in seen.keys() {
        if !caps.contains(id) {
            errors.push(format!("parity row `{id}` has no capability"));
        }
    }
    errors
}

/// (b) each row's scope equals the capability's product_scope.
pub fn check_scope_parity(doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let caps: BTreeMap<&str, &Capability> = doc
        .capabilities
        .iter()
        .map(|cap| (cap.id.as_str(), cap))
        .collect();
    for row in &doc.parity_ledger.rows {
        if let Some(cap) = caps.get(row.capability.as_str()) {
            if row.scope != cap.product_scope {
                errors.push(format!(
                    "`{}` scope `{}` != product_scope `{}`",
                    row.capability, row.scope, cap.product_scope
                ));
            }
            if row.state != cap.state {
                errors.push(format!(
                    "`{}` state `{}` != capability state `{}`",
                    row.capability, row.state, cap.state
                ));
            }
        }
    }
    errors
}

/// Scope and parity vocabularies are DATA-driven from SCHEMA.yaml.
pub fn check_parity_vocabulary(doc: &CompatibilityDocument, schema: &SchemaDoc) -> Vec<String> {
    let mut errors = Vec::new();
    let ledger = &doc.parity_ledger;
    let schema_scope: BTreeSet<&str> = schema
        .ledger_document
        .allowed_product_scope
        .iter()
        .map(String::as_str)
        .collect();
    let declared_scope: BTreeSet<&str> =
        ledger.scope_vocabulary.iter().map(String::as_str).collect();
    if schema_scope != declared_scope {
        errors.push(format!(
            "scope_vocabulary {declared_scope:?} != SCHEMA allowed_product_scope {schema_scope:?}"
        ));
    }
    let expected_parity: BTreeSet<&str> = [
        PARITY_FULL,
        PARITY_PARTIAL,
        PARITY_NOT_APPLICABLE,
        PARITY_NOT_CLAIMED,
    ]
    .into_iter()
    .collect();
    let declared_parity: BTreeSet<&str> = ledger
        .parity_level_vocabulary
        .iter()
        .map(String::as_str)
        .collect();
    if expected_parity != declared_parity {
        errors.push(format!(
            "parity_level_vocabulary {declared_parity:?} != {expected_parity:?}"
        ));
    }
    for row in &ledger.rows {
        if !declared_parity.contains(row.parity.as_str()) {
            errors.push(format!(
                "`{}` parity `{}` is outside the declared vocabulary",
                row.capability, row.parity
            ));
        }
        if !declared_scope.contains(row.scope.as_str()) {
            errors.push(format!(
                "`{}` scope `{}` is outside the declared vocabulary",
                row.capability, row.scope
            ));
        }
    }
    errors
}

/// (c) full rows bind three links that actually resolve.
pub fn check_links(root: &Path, doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let ledger = &doc.parity_ledger;
    let source_ids: BTreeSet<&str> = ledger.java_sources.iter().map(|s| s.id.as_str()).collect();
    for row in &ledger.rows {
        let is_full = row.parity == PARITY_FULL;
        if is_full {
            if row.java.is_none() {
                errors.push(format!("full row `{}` has no java link", row.capability));
            }
            if row.rust.is_empty() {
                errors.push(format!("full row `{}` has no rust link", row.capability));
            }
            if row.test.is_none() {
                errors.push(format!("full row `{}` has no test link", row.capability));
            }
            if row.error_semantics.is_none() {
                errors.push(format!(
                    "full row `{}` has no error_semantics",
                    row.capability
                ));
            }
        }
        let mut java_links: Vec<&JavaLink> = Vec::new();
        if let Some(link) = &row.java {
            java_links.push(link);
        }
        java_links.extend(row.java_secondary.iter());
        for link in java_links {
            if !source_ids.contains(link.source_id.as_str()) {
                errors.push(format!(
                    "`{}` java source_id `{}` does not resolve",
                    row.capability, link.source_id
                ));
            }
            if link.site.trim().is_empty() {
                errors.push(format!("`{}` java site is empty", row.capability));
            }
        }
        for link in &row.rust {
            // `unwrap_or(true)` when the file is unreadable: the read failure
            // is already recorded in `errors`, so the symbol check is skipped
            // rather than reported twice.
            let present = read_required(root, &link.path, &mut errors)
                .map(|text| contains_identifier(&text, &link.symbol))
                .unwrap_or(true);
            if !present {
                errors.push(format!(
                    "`{}` rust symbol `{}` not found in {}",
                    row.capability, link.symbol, link.path
                ));
            }
        }
        if let Some(test) = &row.test {
            if !test.path.starts_with("crates/") || !test.path.contains("/tests/") {
                errors.push(format!(
                    "`{}` test path `{}` is not a crates/**/tests/ target",
                    row.capability, test.path
                ));
            }
            let defined = read_required(root, &test.path, &mut errors)
                .map(|text| contains_fn_definition(&text, &test.name))
                .unwrap_or(true);
            if !defined {
                errors.push(format!(
                    "`{}` test fn `{}` not found in {}",
                    row.capability, test.name, test.path
                ));
            }
        }
    }
    errors
}

/// (d) pinned Java hashes are well formed, capture-pinned hashes match their
/// capture, and both revision fields are populated.
pub fn check_java_sources(root: &Path, doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let ledger = &doc.parity_ledger;
    if ledger.observed_checkout_revision.trim().is_empty() {
        errors.push("observed_checkout_revision is empty".to_string());
    }
    if ledger.capture_declared_revision.trim().is_empty() {
        errors.push("capture_declared_revision is empty".to_string());
    }
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    for source in &ledger.java_sources {
        if !ids.insert(source.id.as_str()) {
            errors.push(format!("java source id `{}` is duplicated", source.id));
        }
        if source.logical_path.trim().is_empty() {
            errors.push(format!(
                "java source `{}` has an empty logical_path",
                source.id
            ));
        }
        if !is_sha256_hex(&source.sha256) {
            errors.push(format!(
                "java source `{}` sha256 `{}` is not 64 lowercase hex",
                source.id, source.sha256
            ));
        }
        if source.pinned_by.trim().is_empty() {
            errors.push(format!(
                "java source `{}` has an empty pinned_by",
                source.id
            ));
        }
        if source.pinned_by.contains(VENDOR_ROOT_TOKEN)
            && ledger
                .java_source_vendor_root
                .as_deref()
                .is_none_or(|hint| hint.trim().is_empty())
        {
            errors.push(format!(
                "java source `{}` pinned_by references {VENDOR_ROOT_TOKEN} but java_source_vendor_root is not declared",
                source.id
            ));
        }
        let capture_bound = source.capture.as_ref().and_then(|capture| {
            read_required(root, &capture.artifact, &mut errors).map(|raw| (capture, raw))
        });
        if let Some((capture, raw)) = capture_bound {
            match serde_json::from_str::<serde_json::Value>(&raw) {
                Ok(value) => match json_pointer(&value, &capture.pointer)
                    .and_then(serde_json::Value::as_str)
                {
                    // guard arm is deliberately empty: the capture and the
                    // ledger agree, so there is nothing to report.
                    Some(expected) if expected == source.sha256 => {}
                    Some(expected) => errors.push(format!(
                        "java source `{}` sha256 `{}` != capture {}#{} `{}`",
                        source.id, source.sha256, capture.artifact, capture.pointer, expected
                    )),
                    None => errors.push(format!(
                        "java source `{}` capture pointer {}#{} is not a string",
                        source.id, capture.artifact, capture.pointer
                    )),
                },
                Err(err) => errors.push(format!(
                    "java source `{}` capture {} is not JSON: {err}",
                    source.id, capture.artifact
                )),
            }
        }
    }
    errors
}

/// (error semantics) every full row binds a source that really contains the
/// declared failure vocabulary.
pub fn check_error_semantics(root: &Path, doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    for row in &doc.parity_ledger.rows {
        let Some(semantics) = &row.error_semantics else {
            continue;
        };
        if !semantics.source.ends_with(".rs") {
            errors.push(format!(
                "`{}` error_semantics source `{}` is not a .rs file",
                row.capability, semantics.source
            ));
            continue;
        }
        if semantics.vocabulary.is_empty() {
            errors.push(format!(
                "`{}` error_semantics vocabulary is empty",
                row.capability
            ));
        }
        if let Some(text) = read_required(root, &semantics.source, &mut errors) {
            for identifier in &semantics.vocabulary {
                if !contains_identifier(&text, identifier) {
                    errors.push(format!(
                        "`{}` error identifier `{}` not found in {}",
                        row.capability, identifier, semantics.source
                    ));
                }
            }
        }
    }
    errors
}

/// (e) every row license resolves to a license inventory component.
pub fn check_license_resolution(doc: &CompatibilityDocument, license: &LicenseDoc) -> Vec<String> {
    let mut errors = Vec::new();
    let ids: BTreeSet<&str> = license
        .components
        .iter()
        .map(|component| component.component_id.as_str())
        .collect();
    let caps: BTreeMap<&str, &Capability> = doc
        .capabilities
        .iter()
        .map(|cap| (cap.id.as_str(), cap))
        .collect();
    for row in &doc.parity_ledger.rows {
        let resolved = row
            .license
            .strip_prefix("inventory:")
            .unwrap_or(row.license.as_str());
        if !ids.contains(resolved) {
            errors.push(format!(
                "`{}` license `{}` does not resolve in {LICENSE_INVENTORY}",
                row.capability, row.license
            ));
        }
        let capability_license = caps
            .get(row.capability.as_str())
            .map(|cap| cap.license.as_str());
        if capability_license.is_some_and(|expected| expected != row.license) {
            errors.push(format!(
                "`{}` license `{}` != capability license `{}`",
                row.capability,
                row.license,
                capability_license.unwrap_or_default()
            ));
        }
    }
    errors
}

/// (f)+(g) absent rows are named by a scope-keyed reason and claim no links.
pub fn check_absent_rows(doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let ledger = &doc.parity_ledger;
    for row in &ledger.rows {
        let absent = row.state == "absent";
        let not_claimed = row.parity == PARITY_NOT_CLAIMED;
        if absent != not_claimed {
            errors.push(format!(
                "`{}` state `{}` and parity `{}` disagree on absence",
                row.capability, row.state, row.parity
            ));
        }
        if not_claimed {
            match ledger.absent_reason_vocabulary.get(&row.scope) {
                Some(expected) => match &row.reason {
                    Some(reason) if reason == expected => {}
                    Some(reason) => errors.push(format!(
                        "`{}` reason `{reason}` != `{expected}` for scope `{}`",
                        row.capability, row.scope
                    )),
                    None => errors.push(format!("`{}` has no absent reason", row.capability)),
                },
                None => errors.push(format!(
                    "scope `{}` has no absent_reason_vocabulary entry",
                    row.scope
                )),
            }
            if row.java.is_some() || !row.rust.is_empty() || row.test.is_some() {
                errors.push(format!(
                    "`{}` is not_claimed_absent but declares links",
                    row.capability
                ));
            }
            if row.error_semantics.is_some() {
                errors.push(format!(
                    "`{}` is not_claimed_absent but declares error_semantics",
                    row.capability
                ));
            }
        }
        if row.state == "absent" && row.parity == PARITY_FULL {
            errors.push(format!(
                "absent row `{}` claims full parity",
                row.capability
            ));
        }
        if row.state == "absent" && row.scope == "required" {
            errors.push(format!(
                "required capability `{}` is absent",
                row.capability
            ));
        }
    }
    errors
}

/// (h) frozen denominators match the rows as written.
pub fn check_denominators(doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let ledger = &doc.parity_ledger;
    let mut scope: BTreeMap<String, u64> = BTreeMap::new();
    let mut parity: BTreeMap<String, u64> = BTreeMap::new();
    let mut implemented = 0u64;
    let mut absent = 0u64;
    for row in &ledger.rows {
        *scope.entry(row.scope.clone()).or_insert(0) += 1;
        *parity.entry(row.parity.clone()).or_insert(0) += 1;
        if row.state == "absent" {
            absent += 1;
        } else {
            implemented += 1;
        }
    }
    let denominators = &ledger.denominators;
    if denominators.capabilities != ledger.rows.len() as u64 {
        errors.push(format!(
            "denominators.capabilities {} != {} rows",
            denominators.capabilities,
            ledger.rows.len()
        ));
    }
    if denominators.capabilities != doc.capabilities.len() as u64 {
        errors.push(format!(
            "denominators.capabilities {} != {} capabilities",
            denominators.capabilities,
            doc.capabilities.len()
        ));
    }
    if denominators.scope != scope {
        errors.push(format!(
            "denominators.scope {:?} != actual {scope:?}",
            denominators.scope
        ));
    }
    if denominators.parity != parity {
        errors.push(format!(
            "denominators.parity {:?} != actual {parity:?}",
            denominators.parity
        ));
    }
    if denominators.implemented != implemented {
        errors.push(format!(
            "denominators.implemented {} != actual {implemented}",
            denominators.implemented
        ));
    }
    if denominators.absent != absent {
        errors.push(format!(
            "denominators.absent {} != actual {absent}",
            denominators.absent
        ));
    }
    if denominators.named_gaps != ledger.named_gaps.len() as u64 {
        errors.push(format!(
            "denominators.named_gaps {} != {} named gaps",
            denominators.named_gaps,
            ledger.named_gaps.len()
        ));
    }
    // Every non-absent row must be one of the three implemented parities:
    // full, partial, or not_applicable_no_java_donor. `not_claimed_absent`
    // is the only absent parity, so this is an exact partition.
    let linked = ledger
        .rows
        .iter()
        .filter(|row| {
            row.parity == PARITY_FULL
                || row.parity == PARITY_PARTIAL
                || row.parity == PARITY_NOT_APPLICABLE
        })
        .count() as u64;
    if implemented != linked {
        errors.push(format!(
            "implemented {implemented} != full+partial+not-applicable rows {linked}"
        ));
    }
    errors
}

/// Named gaps are exactly the rows that are neither full nor not-claimed.
pub fn check_named_gaps(doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let ledger = &doc.parity_ledger;
    let rows: BTreeMap<&str, &Row> = ledger
        .rows
        .iter()
        .map(|row| (row.capability.as_str(), row))
        .collect();
    let mut expected: BTreeSet<&str> = BTreeSet::new();
    for row in &ledger.rows {
        if row.parity == PARITY_PARTIAL || row.parity == PARITY_NOT_APPLICABLE {
            expected.insert(row.capability.as_str());
        }
    }
    let mut named: BTreeSet<&str> = BTreeSet::new();
    for gap in &ledger.named_gaps {
        if !named.insert(gap.capability.as_str()) {
            errors.push(format!("named gap `{}` is duplicated", gap.capability));
        }
        match rows.get(gap.capability.as_str()) {
            Some(row) => {
                let row_reason = row.reason.as_deref().unwrap_or("");
                if !row_reason.is_empty() && row_reason != gap.reason {
                    errors.push(format!(
                        "named gap `{}` reason `{}` != row reason `{row_reason}`",
                        gap.capability, gap.reason
                    ));
                }
            }
            None => errors.push(format!("named gap `{}` has no parity row", gap.capability)),
        }
        if gap.next.trim().is_empty() {
            errors.push(format!(
                "named gap `{}` has an empty next step",
                gap.capability
            ));
        }
    }
    if named != expected {
        errors.push(format!(
            "named gaps {named:?} != rows needing a name {expected:?}"
        ));
    }
    errors
}

/// Fail-closed judge for the absent-capability fate registry (S05, D478/D479).
///
/// Pure DATA -> DATA, no file IO. Every violation names the capability (or the
/// disposition key) and the check never stops at the first error, so one
/// tampered entry cannot mask the others. The registry is total over the 16
/// `state: absent` rows plus the single named gap, and no capability may
/// acquire a fate it is not allowed to hold.
///
/// Invariants (exact error tokens in the slice plan):
///
/// * the key set equals the absent rows union the named gaps;
/// * `disposition` and `reason` come from the declared vocabulary;
/// * a decision always carries a non-empty `rationale`;
/// * the horizon is exactly the `in_horizon_project_native` set, is unique and
///   is contiguous with `horizon_order`;
/// * a named gap's decision is the typed `project_native_no_grobid_donor`
///   refusal and agrees with the gap's `named_gap_disposition` entry.
pub fn check_absent_dispositions(doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let ledger = &doc.parity_ledger;

    // (a) totality: the key set equals the absent rows union the named gaps.
    let mut expected: BTreeSet<&str> = ledger
        .rows
        .iter()
        .filter(|row| row.state == "absent")
        .map(|row| row.capability.as_str())
        .collect();
    expected.extend(ledger.named_gaps.iter().map(|gap| gap.capability.as_str()));
    let actual: BTreeSet<&str> = ledger
        .absent_dispositions
        .keys()
        .map(String::as_str)
        .collect();
    for capability in expected.difference(&actual) {
        errors.push(format!("`{capability}` has no disposition"));
    }
    for key in actual.difference(&expected) {
        errors.push(format!("`{key}` has no capability"));
    }

    let vocabulary = ledger.absent_disposition_vocabulary.as_ref();
    if vocabulary.is_none() {
        errors.push("absent_disposition_vocabulary is not declared".to_string());
    }
    let dispositions: BTreeSet<&str> = vocabulary
        .map(|vocabulary| vocabulary.dispositions.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let reasons: BTreeSet<&str> = vocabulary
        .map(|vocabulary| vocabulary.reasons.iter().map(String::as_str).collect())
        .unwrap_or_default();

    let mut in_horizon: BTreeSet<&str> = BTreeSet::new();
    for (capability, entry) in &ledger.absent_dispositions {
        // (b) the disposition is drawn from the declared vocabulary.
        if !dispositions.contains(entry.disposition.as_str()) {
            errors.push(format!(
                "`{capability}` disposition `{}` is not in the declared absent disposition vocabulary",
                entry.disposition
            ));
        }
        // (c) the reason is drawn from the declared vocabulary.
        if !reasons.contains(entry.reason.as_str()) {
            errors.push(format!(
                "`{capability}` reason `{}` is not in the declared absent reason vocabulary",
                entry.reason
            ));
        }
        // (d) a recorded decision always states a rationale.
        if entry.rationale.trim().is_empty() {
            errors.push(format!("`{capability}` has no rationale"));
        }
        // (e) the horizon is exactly the in-horizon set, nothing more.
        if entry.disposition == DISPOSITION_IN_HORIZON_PROJECT_NATIVE {
            in_horizon.insert(capability.as_str());
            if !ledger
                .horizon_order
                .iter()
                .any(|ordered| ordered == capability)
            {
                errors.push(format!("`{capability}` has no horizon order"));
            }
        } else if entry.disposition == DISPOSITION_OUT_OF_PRODUCT_SCOPE
            && ledger
                .horizon_order
                .iter()
                .any(|ordered| ordered == capability)
        {
            errors.push(format!("`{capability}` claims a horizon order"));
        }
    }

    // (f) the horizon order is unique, resolvable and contiguous with the set.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for ordered in &ledger.horizon_order {
        if !seen.insert(ordered.as_str()) {
            errors.push(format!("duplicate horizon order `{ordered}`"));
        }
        if !ledger.absent_dispositions.contains_key(ordered) {
            errors.push(format!(
                "horizon_order is not contiguous: `{ordered}` is not a disposition key"
            ));
        }
    }
    let ordered_set: BTreeSet<&str> = ledger.horizon_order.iter().map(String::as_str).collect();
    if ordered_set != in_horizon {
        errors.push(format!(
            "horizon_order is not contiguous: {ordered_set:?} != {in_horizon:?}"
        ));
    }

    // (g) a named gap's fate must be the typed refusal it already carries.
    for gap in &ledger.named_gaps {
        let Some(entry) = ledger.absent_dispositions.get(&gap.capability) else {
            // (a) already named the missing key; do not double-report here.
            continue;
        };
        let refusal_agrees = ledger
            .named_gap_disposition
            .as_ref()
            .and_then(|disposition| {
                disposition
                    .gaps
                    .iter()
                    .find(|refusal| refusal.capability == gap.capability)
                    .map(|refusal| {
                        disposition
                            .kind_vocabulary
                            .iter()
                            .any(|kind| kind == &refusal.kind)
                            && !refusal.residual_owner.trim().is_empty()
                            && !refusal.decision_ref.trim().is_empty()
                    })
            })
            .unwrap_or(false);
        if entry.disposition != DISPOSITION_IN_HORIZON_PROJECT_NATIVE
            || entry.reason != NAMED_GAP_REASON_NO_DONOR
            || !refusal_agrees
        {
            errors.push(format!(
                "`{}` disagrees with the named gap refusal",
                gap.capability
            ));
        }
    }

    errors
}

/// Locks are unchanged (D127 / ADR-076 §14).
pub fn check_locks(doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let locks = &doc.parity_ledger.locks;
    if locks.import_eligible {
        errors.push("locks.import_eligible must be false".to_string());
    }
    if locks.deployed {
        errors.push("locks.deployed must be false".to_string());
    }
    for (label, value) in [
        ("production_route", &locks.production_route),
        ("graph_write", &locks.graph_write),
        ("auto_promote", &locks.auto_promote),
    ] {
        if value != "forbidden" {
            errors.push(format!("locks.{label} must be `forbidden`, got `{value}`"));
        }
    }
    errors
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Class-token union of the S02 artifact's `class_profiles[].classes[].class`.
fn artifact_class_union(value: &serde_json::Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for profile in value
        .get("class_profiles")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        for class in profile
            .get("classes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(name) = class.get("class").and_then(serde_json::Value::as_str) {
                out.insert(name.to_string());
            }
        }
    }
    out
}

/// Class-token union of the frozen tables' `documents[].profile[].class`.
fn frozen_class_union(value: &serde_json::Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for document in value
        .get("documents")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        for class in document
            .get("profile")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(name) = class.get("class").and_then(serde_json::Value::as_str) {
                out.insert(name.to_string());
            }
        }
    }
    out
}

/// (measurement coverage, pure DATA -> DATA) every `required` row carries a
/// typed measurement link, no other row does, the states/classes/reasons come
/// from closed vocabularies, and every declared class is claimed by at least
/// one measured row. Named gaps are typed refusals, never required rows.
pub fn check_measurement_coverage(doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let ledger = &doc.parity_ledger;
    let Some(measurement) = &ledger.parity_measurement else {
        errors.push("parity_ledger has no parity_measurement block".to_string());
        return errors;
    };
    let states: BTreeSet<&str> = measurement
        .state_vocabulary
        .iter()
        .map(String::as_str)
        .collect();
    let reasons: BTreeSet<&str> = measurement
        .outside_reason_vocabulary
        .iter()
        .map(String::as_str)
        .collect();
    let classes: BTreeSet<&str> = measurement.classes.iter().map(String::as_str).collect();
    let mut claimed_classes: BTreeSet<&str> = BTreeSet::new();

    for row in &ledger.rows {
        let required = row.scope == "required";
        match (&row.measurement, required) {
            (None, true) => errors.push(format!(
                "required row `{}` has no measurement",
                row.capability
            )),
            (Some(_), false) => errors.push(format!(
                "non-required row `{}` declares a measurement",
                row.capability
            )),
            _ => {}
        }
        let Some(row_measurement) = &row.measurement else {
            continue;
        };
        if !states.contains(row_measurement.state.as_str()) {
            errors.push(format!(
                "`{}` measurement state `{}` is outside the declared measurement state vocabulary",
                row.capability, row_measurement.state
            ));
            continue;
        }
        match row_measurement.state.as_str() {
            MEASURED_BY_CORPUS_PARITY => {
                if row_measurement.classes.is_empty() {
                    errors.push(format!(
                        "measured row `{}` declares no classes",
                        row.capability
                    ));
                }
                for class in &row_measurement.classes {
                    if classes.contains(class.as_str()) {
                        claimed_classes.insert(class.as_str());
                    } else {
                        errors.push(format!(
                            "`{}` measurement class `{class}` is not in the declared class set",
                            row.capability
                        ));
                    }
                }
            }
            OUTSIDE_CORPUS_PARITY => match &row_measurement.reason {
                Some(reason) if reasons.contains(reason.as_str()) => {}
                Some(reason) => errors.push(format!(
                    "`{}` measurement reason `{reason}` is not in the declared measurement reason vocabulary",
                    row.capability
                )),
                None => errors.push(format!(
                    "outside row `{}` has no measurement reason",
                    row.capability
                )),
            },
            _ => {}
        }
    }

    for class in &measurement.classes {
        if !claimed_classes.contains(class.as_str()) {
            errors.push(format!(
                "declared class `{class}` is claimed by no measured row"
            ));
        }
    }

    // Named-gap typed refusal: a required row is never named a gap, and every
    // gap carries a kind from the closed vocabulary plus its decision reference
    // and residual owner.
    let rows: BTreeMap<&str, &Row> = ledger
        .rows
        .iter()
        .map(|row| (row.capability.as_str(), row))
        .collect();
    let kinds: BTreeSet<&str> = ledger
        .named_gap_disposition
        .as_ref()
        .map(|disposition| {
            disposition
                .kind_vocabulary
                .iter()
                .map(String::as_str)
                .collect()
        })
        .unwrap_or_default();
    let mut required_gaps = 0u64;
    for gap in &ledger.named_gaps {
        if rows
            .get(gap.capability.as_str())
            .is_some_and(|row| row.scope == "required")
        {
            required_gaps += 1;
            errors.push(format!("named gap `{}` is a required row", gap.capability));
        }
        let refusal = ledger
            .named_gap_disposition
            .as_ref()
            .and_then(|disposition| {
                disposition
                    .gaps
                    .iter()
                    .find(|refusal| refusal.capability == gap.capability)
            });
        let typed = refusal.is_some_and(|refusal| {
            kinds.contains(refusal.kind.as_str())
                && !refusal.decision_ref.trim().is_empty()
                && !refusal.residual_owner.trim().is_empty()
        });
        if !typed {
            errors.push(format!(
                "named gap `{}` has no typed refusal",
                gap.capability
            ));
        }
    }
    match &ledger.named_gap_disposition {
        None => errors.push("parity_ledger has no named_gap_disposition block".to_string()),
        Some(disposition) if disposition.required_named_gaps != required_gaps => {
            errors.push(format!(
                "required named gaps {required_gaps} != declared {}",
                disposition.required_named_gaps
            ));
        }
        Some(_) => {}
    }
    errors
}

/// (measurement identity) the declared link is measured, not declared: both
/// content-addressed files re-hash to the declared bytes, the internal chain
/// holds, the identity/release/class bindings match the pinned artifact, the
/// mirrored class-parity declaration is exact, and the D442/D469 reconciliation
/// keeps the corpus-wide micro number from reading as a pooled quality claim.
///
/// This reads only repo-relative content-addressed files through
/// `read_required`, so it preserves the module's environment independence: no
/// vendor tree, no network, no new dependency.
pub fn check_measurement_identity(root: &Path, doc: &CompatibilityDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(measurement) = &doc.parity_ledger.parity_measurement else {
        errors.push("parity_ledger has no parity_measurement block".to_string());
        return errors;
    };

    let artifact_raw = read_required(root, &measurement.artifact.path, &mut errors);
    let frozen_raw = read_required(root, &measurement.frozen_tables.path, &mut errors);
    let artifact = artifact_raw.as_deref().and_then(|raw| {
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(value) => Some(value),
            Err(err) => {
                errors.push(format!(
                    "parity_measurement artifact {} is not JSON: {err}",
                    measurement.artifact.path
                ));
                None
            }
        }
    });
    let frozen = frozen_raw.as_deref().and_then(|raw| {
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(value) => Some(value),
            Err(err) => {
                errors.push(format!(
                    "parity_measurement frozen tables {} is not JSON: {err}",
                    measurement.frozen_tables.path
                ));
                None
            }
        }
    });

    if let Some(raw) = artifact_raw.as_deref() {
        let actual = sha256_hex(raw.as_bytes());
        if actual != measurement.artifact.sha256 {
            errors.push(format!(
                "parity_measurement artifact sha256 `{}` != artifact bytes `{actual}`",
                measurement.artifact.sha256
            ));
        }
    }
    if let Some(raw) = frozen_raw.as_deref() {
        let actual = sha256_hex(raw.as_bytes());
        if actual != measurement.frozen_tables.sha256 {
            errors.push(format!(
                "parity_measurement frozen_tables sha256 `{}` != frozen bytes `{actual}`",
                measurement.frozen_tables.sha256
            ));
        }
        if let Some(value) = frozen.as_ref() {
            match value
                .get("artifact_sha256")
                .and_then(serde_json::Value::as_str)
            {
                Some(actual) if actual == measurement.artifact.sha256 => {}
                Some(actual) => errors.push(format!(
                    "parity_measurement frozen_tables.artifact_sha256 `{actual}` != artifact_sha256 `{}`",
                    measurement.artifact.sha256
                )),
                None => errors.push(
                    "parity_measurement frozen_tables has no artifact_sha256".to_string(),
                ),
            }
        }
    }

    if let Some(value) = artifact.as_ref() {
        match value
            .pointer("/reference_population/identity_sha256")
            .and_then(serde_json::Value::as_str)
        {
            Some(actual) if actual == measurement.identity_digest => {}
            Some(actual) => errors.push(format!(
                "parity_measurement identity_digest `{}` != artifact identity `{actual}`",
                measurement.identity_digest
            )),
            None => errors.push(
                "parity_measurement artifact has no reference_population.identity_sha256"
                    .to_string(),
            ),
        }
        match value
            .pointer("/reference_population/reference_release/image_digest")
            .and_then(serde_json::Value::as_str)
        {
            Some(actual) if actual == measurement.declared_release.image_digest => {}
            Some(actual) => errors.push(format!(
                "parity_measurement declared_release.image_digest `{}` != artifact release digest `{actual}`",
                measurement.declared_release.image_digest
            )),
            None => errors.push(
                "parity_measurement artifact has no reference_release.image_digest".to_string(),
            ),
        }

        let artifact_classes = artifact_class_union(value);
        let declared_classes: BTreeSet<String> = measurement.classes.iter().cloned().collect();
        if declared_classes != artifact_classes {
            errors.push(format!(
                "parity_measurement classes {declared_classes:?} != artifact class set {artifact_classes:?}"
            ));
        }
        if let Some(frozen_value) = frozen.as_ref() {
            let frozen_classes = frozen_class_union(frozen_value);
            if declared_classes != frozen_classes {
                errors.push(format!(
                    "parity_measurement classes {declared_classes:?} != frozen-tables class set {frozen_classes:?}"
                ));
            }
        }

        match value.get("class_parity_declaration") {
            Some(artifact_declaration) => {
                let declared = serde_json::to_value(&measurement.class_parity_declaration)
                    .unwrap_or(serde_json::Value::Null);
                if &declared != artifact_declaration {
                    errors.push(format!(
                        "parity_measurement class_parity_declaration {declared} != artifact class_parity_declaration {artifact_declaration}"
                    ));
                }
            }
            None => errors
                .push("parity_measurement artifact has no class_parity_declaration".to_string()),
        }
    }

    // D442/D469 reconciliation: the aggregate is a count over the declared
    // population, explicitly not a pooled statistical claim.
    if measurement.aggregate_kind != AGGREGATE_KIND_DECLARED_POPULATION_COUNT {
        errors.push(format!(
            "parity_measurement.aggregate_kind `{}` != {AGGREGATE_KIND_DECLARED_POPULATION_COUNT}",
            measurement.aggregate_kind
        ));
    }
    if measurement.pooled_quality_claim != POOLED_QUALITY_CLAIM_FORBIDDEN {
        errors.push(format!(
            "parity_measurement.pooled_quality_claim `{}` must be forbidden",
            measurement.pooled_quality_claim
        ));
    }
    if let Some(value) = artifact.as_ref() {
        if value
            .pointer("/evaluated_population/aggregate/counts_only")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            errors.push(
                "parity_measurement artifact evaluated_population.aggregate.counts_only must be true"
                    .to_string(),
            );
        }
        match value
            .pointer("/policy_locks/pooled_claim")
            .and_then(serde_json::Value::as_str)
        {
            Some(actual) if actual == POOLED_QUALITY_CLAIM_FORBIDDEN => {}
            Some(actual) => errors.push(format!(
                "parity_measurement artifact policy_locks.pooled_claim `{actual}` must be forbidden"
            )),
            None => errors.push(
                "parity_measurement artifact has no policy_locks.pooled_claim; it must be forbidden"
                    .to_string(),
            ),
        }
    }

    if measurement.declared_release.label.trim().is_empty() {
        errors.push("parity_measurement declared_release.label is empty".to_string());
    }
    if measurement
        .declared_release
        .rollback_label
        .trim()
        .is_empty()
    {
        errors.push("parity_measurement declared_release.rollback_label is empty".to_string());
    }
    if measurement
        .declared_release
        .label_divergence_item
        .trim()
        .is_empty()
    {
        errors
            .push("parity_measurement declared_release.label_divergence_item is empty".to_string());
    }
    errors
}

// --------------------------------------------------------------------------
// Aggregate
// --------------------------------------------------------------------------

pub struct Outcome {
    pub report: String,
    pub errors: Vec<String>,
}

pub fn evaluate(root: &Path) -> Outcome {
    evaluate_document(root, load(root))
}

/// Run every invariant against an already-loaded document. This is what
/// `evaluate` uses for the carrier on disk and what the mutation matrix
/// (`grobid_parity_ledger_mutations.rs`) uses for an in-memory mutated clone:
/// the mutation target must judge a tampered document through exactly the same
/// checks, never through a parallel reimplementation that could drift.
///
/// Additive only: the running order and the set of checks are byte-identical to
/// the previous `evaluate`, so no rule is weakened.
pub fn evaluate_document(root: &Path, doc: CompatibilityDocument) -> Outcome {
    let schema = load_schema(root);
    let license = load_license(root);
    let mut errors = Vec::new();
    errors.extend(check_contract_identity(&doc));
    errors.extend(check_row_identity(&doc));
    errors.extend(check_scope_parity(&doc));
    errors.extend(check_parity_vocabulary(&doc, &schema));
    errors.extend(check_links(root, &doc));
    errors.extend(check_java_sources(root, &doc));
    errors.extend(check_error_semantics(root, &doc));
    errors.extend(check_license_resolution(&doc, &license));
    errors.extend(check_absent_rows(&doc));
    errors.extend(check_denominators(&doc));
    errors.extend(check_named_gaps(&doc));
    errors.extend(check_locks(&doc));
    // M513-r7dg4z/S03 (D471): the fourth link. Appended additively so the
    // existing check order and set are unchanged.
    errors.extend(check_measurement_coverage(&doc));
    errors.extend(check_measurement_identity(root, &doc));
    // M513-r7dg4z/S05 (D478/D479): the recorded fate of every absent capability
    // and the single named gap. Appended last so no existing check is
    // reordered or weakened.
    errors.extend(check_absent_dispositions(&doc));
    Outcome {
        report: render_report(&doc),
        errors,
    }
}

pub fn render_report(doc: &CompatibilityDocument) -> String {
    let mut out = String::new();
    let ledger = &doc.parity_ledger;
    let _ = writeln!(
        out,
        "parity ledger `{}` — {} rows",
        ledger.contract_id,
        ledger.rows.len()
    );
    for row in &ledger.rows {
        let java = match &row.java {
            Some(link) => format!("{}:{}", link.source_id, link.site),
            None => "-".to_string(),
        };
        let test = match &row.test {
            Some(link) => format!("{}::{}", link.path, link.name),
            None => "-".to_string(),
        };
        let measurement = match &row.measurement {
            Some(link) if link.state == MEASURED_BY_CORPUS_PARITY => {
                format!("measured[{}]", link.classes.len())
            }
            Some(link) => format!("outside[{}]", link.reason.as_deref().unwrap_or("-")),
            None => "-".to_string(),
        };
        let disposition = match ledger.absent_dispositions.get(&row.capability) {
            Some(entry) => format!("{}/{}", entry.disposition, entry.reason),
            None => "-".to_string(),
        };
        let _ = writeln!(
            out,
            "  {:<50} {:<26} {:<8} {:<27} java={:<28} rust={} test={} measurement={} disposition={}",
            row.capability,
            row.scope,
            row.state,
            row.parity,
            java,
            row.rust.len(),
            test,
            measurement,
            disposition
        );
    }
    let _ = writeln!(out, "  named gaps:");
    for gap in &ledger.named_gaps {
        let refusal = ledger
            .named_gap_disposition
            .as_ref()
            .and_then(|disposition| {
                disposition
                    .gaps
                    .iter()
                    .find(|refusal| refusal.capability == gap.capability)
            });
        match refusal {
            Some(refusal) => {
                let _ = writeln!(
                    out,
                    "    {} -> {} kind={} decision_ref={} residual_owner={}",
                    gap.capability,
                    gap.reason,
                    refusal.kind,
                    refusal.decision_ref,
                    refusal.residual_owner
                );
            }
            None => {
                let _ = writeln!(out, "    {} -> {}", gap.capability, gap.reason);
            }
        }
    }
    let _ = writeln!(out, "  denominators: {:?}", ledger.denominators);
    out
}
