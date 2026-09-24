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
//!     test path is a `crates/**/tests/` target containing `fn <name>`;
//! (d) every Java source sha256 is 64 lowercase hex; capture-pinned sources
//!     equal the capture; both revisions are non-empty;
//! (e) every row license resolves to a `component_id` in the inventory;
//! (f) `not_claimed_absent` rows carry a scope-keyed reason and no links;
//! (g) no absent row claims parity, and no `required` capability is absent;
//! (h) the frozen denominators match what the rows actually contain.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub const CONTRACT_ID: &str = "grobid-java-rust-parity-ledger.v1";
pub const COMPATIBILITY_DOC: &str = "data/parse/grobid_compatibility.yaml";
pub const LICENSE_INVENTORY: &str = "data/parse/license_inventory.yaml";
pub const SCHEMA_DOC: &str = "data/parse/SCHEMA.yaml";

pub const PARITY_FULL: &str = "full";
pub const PARITY_PARTIAL: &str = "partial";
pub const PARITY_NOT_APPLICABLE: &str = "not_applicable_no_java_donor";
pub const PARITY_NOT_CLAIMED: &str = "not_claimed_absent";

// --------------------------------------------------------------------------
// Data model
// --------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CompatibilityDocument {
    pub capabilities: Vec<Capability>,
    pub parity_ledger: ParityLedger,
}

#[derive(Debug, Deserialize)]
pub struct Capability {
    pub id: String,
    pub state: String,
    pub product_scope: String,
    pub license: String,
}

#[derive(Debug, Deserialize)]
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
    pub java_sources: Vec<JavaSource>,
    pub rows: Vec<Row>,
    pub named_gaps: Vec<NamedGap>,
    pub denominators: Denominators,
    pub locks: Locks,
}

#[derive(Debug, Deserialize)]
pub struct JavaSource {
    pub id: String,
    pub logical_path: String,
    pub sha256: String,
    pub pinned_by: String,
    #[serde(default)]
    pub capture: Option<Capture>,
}

#[derive(Debug, Deserialize)]
pub struct Capture {
    pub artifact: String,
    pub pointer: String,
}

#[derive(Debug, Deserialize)]
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
    pub license: String,
}

#[derive(Debug, Deserialize)]
pub struct JavaLink {
    pub source_id: String,
    pub site: String,
}

#[derive(Debug, Deserialize)]
pub struct RustLink {
    pub path: String,
    pub symbol: String,
}

#[derive(Debug, Deserialize)]
pub struct TestLink {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ErrorSemantics {
    pub source: String,
    pub vocabulary: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct NamedGap {
    pub capability: String,
    pub reason: String,
    pub next: String,
}

#[derive(Debug, Deserialize)]
pub struct Denominators {
    pub capabilities: u64,
    pub scope: BTreeMap<String, u64>,
    pub implemented: u64,
    pub absent: u64,
    pub parity: BTreeMap<String, u64>,
    pub named_gaps: u64,
}

#[derive(Debug, Deserialize)]
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

fn read_required(root: &Path, relative: &str, errors: &mut Vec<String>) -> Option<String> {
    match std::fs::read_to_string(root.join(relative)) {
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

// --------------------------------------------------------------------------
// Aggregate
// --------------------------------------------------------------------------

pub struct Outcome {
    pub report: String,
    pub errors: Vec<String>,
}

pub fn evaluate(root: &Path) -> Outcome {
    let doc = load(root);
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
        let _ = writeln!(
            out,
            "  {:<50} {:<26} {:<8} {:<27} java={:<28} rust={} test={}",
            row.capability,
            row.scope,
            row.state,
            row.parity,
            java,
            row.rust.len(),
            test
        );
    }
    let _ = writeln!(out, "  named gaps:");
    for gap in &ledger.named_gaps {
        let _ = writeln!(out, "    {} -> {}", gap.capability, gap.reason);
    }
    let _ = writeln!(out, "  denominators: {:?}", ledger.denominators);
    out
}
