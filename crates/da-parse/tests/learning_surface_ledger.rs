//! Honest learning-surface dispositions, carried separately from the frozen
//! M513 compat ledger (M514-1lss2s/S07/T05).
//!
//! Why a separate carrier instead of editing `data/parse/grobid_compatibility.yaml`:
//! that ledger is a read-only point-in-time record. It is pinned by
//! `grobid_retirement_release_decision_package.json#source_bindings`
//! (`role: parity_ledger_and_remainders`), by
//! `absorption_progress_baseline.json#historical_guard_pins`, and it is
//! re-hashed from disk by `grobid_retirement_package_contract.rs`. Rewriting it
//! to say `candidate` would trip every one of those guards. So the current,
//! honest disposition of the two project-native learning surfaces is recorded
//! here, as new evidence that *depends on* the frozen bytes rather than
//! replacing them.
//!
//! The carrier is DATA. This target is the machine check that keeps it honest:
//! it re-hashes both frozen authority files from disk, requires the frozen
//! ledger rows to still say `absent` (proving the carrier is additive, not a
//! re-pin), resolves every Rust symbol and every independent `tests/` function
//! the carrier names, and refuses on any opened lock or dropped authority.
//!
//! Run:
//!
//! ```text
//! cargo test -p da-parse --test learning_surface_ledger
//! ```

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

const CARRIER_REL: &str = "artifacts/parse_compat/learning_surface_dispositions.json";
const LEDGER_REL: &str = "data/parse/grobid_compatibility.yaml";
const RELEASE_PACKAGE_REL: &str =
    "artifacts/parse_compat/grobid_retirement_release_decision_package.json";
const BASELINE_REL: &str = "artifacts/parse_compat/absorption_progress_baseline.json";

const SCHEMA_VERSION: &str = "learning-surface-dispositions.v1";
const PARITY_NATIVE: &str = "not_applicable_no_java_donor";
const REASON_NO_DONOR: &str = "no_java_donor";
const STATE_CANDIDATE: &str = "candidate";
const STATE_ABSENT: &str = "absent";
const PARITY_NOT_CLAIMED: &str = "not_claimed_absent";
const NAMED_GAP: &str = "learning.ruvllm_ablation";
const LEDGER_SOURCE_BINDING_ROLE: &str = "parity_ledger_and_remainders";

/// The closed top-level key set of the carrier.
const TOP_LEVEL_KEYS: &[&str] = &["schema_version", "authority", "locks", "surfaces", "note"];
/// Exactly the two project-native learning surfaces this carrier covers.
const EXPECTED_SURFACES: &[&str] = &["learning.sona_trajectory", "learning.microlora_ewc"];

// --------------------------------------------------------------------------
// Carrier data model. `deny_unknown_fields` makes the closed key set a parse
// requirement, not merely an assert, so an accidental extra field is refused.
// --------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Carrier {
    schema_version: String,
    authority: Authority,
    locks: Locks,
    surfaces: Vec<Surface>,
    note: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Authority {
    frozen_ledger: FrozenRef,
    frozen_release_package: FrozenRef,
    historical_state: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct FrozenRef {
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Locks {
    deployed: bool,
    import_eligible: bool,
    promotion_state: String,
    auto_promote: String,
    production_route: String,
    graph_write: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Surface {
    id: String,
    state: String,
    parity: String,
    reason: String,
    java: Option<serde_json::Value>,
    implementation: Implementation,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Implementation {
    #[serde(rename = "crate")]
    crate_name: String,
    surface: String,
    rust: RustLink,
    test: TestLink,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct RustLink {
    path: String,
    symbol: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct TestLink {
    path: String,
    name: String,
}

// --------------------------------------------------------------------------
// Frozen-carrier subsets. Only the fields the check needs; the check must not
// become a second copy of the full ledger validator.
// --------------------------------------------------------------------------

#[derive(Deserialize)]
struct LedgerDoc {
    parity_ledger: LedgerPart,
}

#[derive(Deserialize)]
struct LedgerPart {
    rows: Vec<LedgerRow>,
    named_gaps: Vec<LedgerNamedGap>,
}

#[derive(Deserialize)]
struct LedgerRow {
    capability: String,
    state: String,
    parity: String,
}

#[derive(Deserialize)]
struct LedgerNamedGap {
    capability: String,
}

#[derive(Deserialize)]
struct ReleasePackage {
    source_bindings: Vec<SourceBinding>,
}

#[derive(Deserialize)]
struct SourceBinding {
    role: String,
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
struct Baseline {
    historical_guard_pins: GuardPins,
}

#[derive(Deserialize)]
struct GuardPins {
    pins: BTreeMap<String, String>,
}

// --------------------------------------------------------------------------
// Filesystem helpers
// --------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root above crates/da-parse")
        .to_path_buf()
}

fn read_bytes(rel: &str) -> Result<Vec<u8>, String> {
    std::fs::read(repo_root().join(rel)).map_err(|err| format!("cannot read {rel}: {err}"))
}

fn read_text(rel: &str) -> Result<String, String> {
    std::fs::read_to_string(repo_root().join(rel))
        .map_err(|err| format!("cannot read {rel}: {err}"))
}

fn sha256_of(rel: &str) -> Result<String, String> {
    read_bytes(rel).map(|bytes| format!("sha256:{}", hex::encode(Sha256::digest(&bytes))))
}

/// A carrier-declared path is acceptable only when it is repo-relative and
/// carries no `..` component, so the check can never be redirected outside the
/// workspace it is verifying.
fn is_repo_relative(rel: &str) -> bool {
    let path = Path::new(rel);
    !rel.is_empty()
        && path.is_relative()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// True when `ident` occurs as a complete identifier token, so `build_reasoning_bank`
/// does not match a longer identifier that merely contains it.
fn contains_identifier(text: &str, ident: &str) -> bool {
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

/// True when `text` contains a `fn <name>` definition (any visibility, async or
/// not) for `name`.
fn contains_fn_definition(text: &str, name: &str) -> bool {
    contains_identifier(text, &format!("fn {name}"))
}

// --------------------------------------------------------------------------
// Validation
// --------------------------------------------------------------------------

fn report(errors: &[String]) -> String {
    if errors.is_empty() {
        "no carrier violations".to_string()
    } else {
        let mut out = format!("{} violation(s):", errors.len());
        for error in errors {
            let _ = write!(out, "\n- {error}");
        }
        out
    }
}

/// Pure-ish validator: reads the frozen carriers from disk, then judges the
/// parsed `carrier` plus the raw JSON value (for the closed key set). It never
/// mutates anything and reports every violation instead of stopping at the
/// first, so one tampered field cannot mask another.
fn validate(carrier: &Carrier, raw: &serde_json::Value) -> Vec<String> {
    let mut errors = Vec::new();

    // Closed top-level key set.
    let expected_keys: BTreeSet<&str> = TOP_LEVEL_KEYS.iter().copied().collect();
    let actual_keys: BTreeSet<String> = raw
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    let actual_ref: BTreeSet<&str> = actual_keys.iter().map(String::as_str).collect();
    if actual_ref != expected_keys {
        errors.push(format!(
            "top-level key set {actual_ref:?} != closed set {expected_keys:?}"
        ));
    }

    if carrier.schema_version != SCHEMA_VERSION {
        errors.push(format!(
            "schema_version `{}` != `{SCHEMA_VERSION}`",
            carrier.schema_version
        ));
    }
    if carrier.note.trim().is_empty() {
        errors.push("note is empty".to_string());
    }

    // Every lock closed (D127 / ADR-076 §14).
    let locks = &carrier.locks;
    if locks.deployed {
        errors.push("locks.deployed must be false".to_string());
    }
    if locks.import_eligible {
        errors.push("locks.import_eligible must be false".to_string());
    }
    if locks.promotion_state != STATE_CANDIDATE {
        errors.push(format!(
            "locks.promotion_state must be `{STATE_CANDIDATE}`, got `{}`",
            locks.promotion_state
        ));
    }
    for (label, value) in [
        ("auto_promote", &locks.auto_promote),
        ("production_route", &locks.production_route),
        ("graph_write", &locks.graph_write),
    ] {
        if value != "forbidden" {
            errors.push(format!("locks.{label} must be `forbidden`, got `{value}`"));
        }
    }

    // Authority: repo-relative paths, recomputed bytes, and no silent re-pin of
    // the frozen history.
    for (label, reference) in [
        ("frozen_ledger", &carrier.authority.frozen_ledger),
        (
            "frozen_release_package",
            &carrier.authority.frozen_release_package,
        ),
    ] {
        if !is_repo_relative(&reference.path) {
            errors.push(format!(
                "authority.{label}.path `{}` is not repo-relative",
                reference.path
            ));
        }
    }

    let ledger_hash = match sha256_of(LEDGER_REL) {
        Ok(hash) => hash,
        Err(error) => {
            errors.push(error);
            String::new()
        }
    };
    let release_hash = match sha256_of(RELEASE_PACKAGE_REL) {
        Ok(hash) => hash,
        Err(error) => {
            errors.push(error);
            String::new()
        }
    };

    if carrier.authority.frozen_ledger.path != LEDGER_REL {
        errors.push(format!(
            "authority.frozen_ledger.path `{}` != `{LEDGER_REL}`",
            carrier.authority.frozen_ledger.path
        ));
    }
    if !ledger_hash.is_empty() && carrier.authority.frozen_ledger.sha256 != ledger_hash {
        errors.push(format!(
            "authority.frozen_ledger.sha256 `{}` != recomputed `{ledger_hash}`",
            carrier.authority.frozen_ledger.sha256
        ));
    }
    if carrier.authority.frozen_release_package.path != RELEASE_PACKAGE_REL {
        errors.push(format!(
            "authority.frozen_release_package.path `{}` != `{RELEASE_PACKAGE_REL}`",
            carrier.authority.frozen_release_package.path
        ));
    }
    if !release_hash.is_empty() && carrier.authority.frozen_release_package.sha256 != release_hash {
        errors.push(format!(
            "authority.frozen_release_package.sha256 `{}` != recomputed `{release_hash}`",
            carrier.authority.frozen_release_package.sha256
        ));
    }

    // The release package's own source binding must agree with the recomputed
    // ledger bytes: the carrier cannot point at a re-pinned ledger.
    match read_text(RELEASE_PACKAGE_REL)
        .ok()
        .and_then(|raw| serde_json::from_str::<ReleasePackage>(&raw).ok())
    {
        Some(package) => {
            match package
                .source_bindings
                .iter()
                .find(|binding| binding.role == LEDGER_SOURCE_BINDING_ROLE)
            {
                Some(binding) => {
                    if binding.path != LEDGER_REL {
                        errors.push(format!(
                            "release package `{LEDGER_SOURCE_BINDING_ROLE}` path `{}` != `{LEDGER_REL}`",
                            binding.path
                        ));
                    }
                    let bare = ledger_hash.strip_prefix("sha256:").unwrap_or(&ledger_hash);
                    if !ledger_hash.is_empty() && binding.sha256 != bare {
                        errors.push(format!(
                            "release package `{LEDGER_SOURCE_BINDING_ROLE}` sha256 `{}` != recomputed ledger `{bare}`",
                            binding.sha256
                        ));
                    }
                }
                None => errors.push(format!(
                    "release package has no `{LEDGER_SOURCE_BINDING_ROLE}` source binding"
                )),
            }
        }
        None => errors.push(format!("cannot parse {RELEASE_PACKAGE_REL}")),
    }

    // The baseline guard pins must still name both frozen carriers with the
    // recomputed bytes: this is the no-silent-repin check.
    match read_text(BASELINE_REL)
        .ok()
        .and_then(|raw| serde_json::from_str::<Baseline>(&raw).ok())
    {
        Some(baseline) => {
            for (rel, recomputed) in [
                (LEDGER_REL, &ledger_hash),
                (RELEASE_PACKAGE_REL, &release_hash),
            ] {
                if recomputed.is_empty() {
                    continue;
                }
                match baseline.historical_guard_pins.pins.get(rel) {
                    Some(pinned) if pinned == recomputed => {}
                    Some(pinned) => errors.push(format!(
                        "baseline historical_guard_pins[{rel}] `{pinned}` != recomputed `{recomputed}`"
                    )),
                    None => errors.push(format!(
                        "baseline historical_guard_pins has no pin for {rel}"
                    )),
                }
            }
        }
        None => errors.push(format!("cannot parse {BASELINE_REL}")),
    }

    // `historical_state` records both ids as `absent` so the carrier declares
    // it layers over an unchanged record.
    let historical: BTreeSet<&str> = carrier
        .authority
        .historical_state
        .keys()
        .map(String::as_str)
        .collect();
    let expected_history: BTreeSet<&str> = EXPECTED_SURFACES.iter().copied().collect();
    if historical != expected_history {
        errors.push(format!(
            "authority.historical_state keys {historical:?} != {expected_history:?}"
        ));
    }
    for (id, state) in &carrier.authority.historical_state {
        if state != STATE_ABSENT {
            errors.push(format!(
                "authority.historical_state[{id}] `{state}` != `{STATE_ABSENT}`"
            ));
        }
    }

    // Surfaces: exactly the two expected ids, each candidate and resolvable.
    if carrier.surfaces.len() != EXPECTED_SURFACES.len() {
        errors.push(format!(
            "surfaces has {} entries, expected {}",
            carrier.surfaces.len(),
            EXPECTED_SURFACES.len()
        ));
    }
    let declared_ids: BTreeSet<&str> = carrier.surfaces.iter().map(|s| s.id.as_str()).collect();
    if declared_ids != expected_history {
        errors.push(format!(
            "surface ids {declared_ids:?} != {expected_history:?}"
        ));
    }
    for surface in &carrier.surfaces {
        if surface.state != STATE_CANDIDATE {
            errors.push(format!(
                "`{}` state `{}` != `{STATE_CANDIDATE}`",
                surface.id, surface.state
            ));
        }
        if surface.parity != PARITY_NATIVE {
            errors.push(format!(
                "`{}` parity `{}` != `{PARITY_NATIVE}`",
                surface.id, surface.parity
            ));
        }
        if surface.reason != REASON_NO_DONOR {
            errors.push(format!(
                "`{}` reason `{}` != `{REASON_NO_DONOR}`",
                surface.id, surface.reason
            ));
        }
        if surface.java.is_some() {
            errors.push(format!(
                "`{}` declares a java donor; a project-native surface has none",
                surface.id
            ));
        }
        let implementation = &surface.implementation;
        if implementation.crate_name.trim().is_empty() {
            errors.push(format!("`{}` implementation.crate is empty", surface.id));
        }
        if implementation.surface.trim().is_empty() {
            errors.push(format!("`{}` implementation.surface is empty", surface.id));
        }
        let rust = &implementation.rust;
        if !is_repo_relative(&rust.path) {
            errors.push(format!(
                "`{}` rust.path `{}` is not repo-relative",
                surface.id, rust.path
            ));
        } else {
            match read_text(&rust.path) {
                Ok(text) => {
                    if !contains_identifier(&text, &rust.symbol) {
                        errors.push(format!(
                            "`{}` rust symbol `{}` not found in {}",
                            surface.id, rust.symbol, rust.path
                        ));
                    }
                }
                Err(error) => errors.push(error),
            }
        }
        let test = &implementation.test;
        if !test.path.starts_with("crates/") || !test.path.contains("/tests/") {
            errors.push(format!(
                "`{}` test path `{}` is not a crates/**/tests/ target",
                surface.id, test.path
            ));
        } else {
            match read_text(&test.path) {
                Ok(text) => {
                    if !contains_fn_definition(&text, &test.name) {
                        errors.push(format!(
                            "`{}` test fn `{}` not found in {}",
                            surface.id, test.name, test.path
                        ));
                    }
                }
                Err(error) => errors.push(error),
            }
        }
    }

    // Additivity: the frozen ledger rows must still say `absent` /
    // `not_claimed_absent` for both surfaces, and the single named gap must
    // remain exactly `learning.ruvllm_ablation`.
    match read_text(LEDGER_REL)
        .ok()
        .and_then(|raw| serde_yaml::from_str::<LedgerDoc>(&raw).ok())
    {
        Some(ledger) => {
            for id in EXPECTED_SURFACES {
                match ledger
                    .parity_ledger
                    .rows
                    .iter()
                    .find(|row| row.capability == *id)
                {
                    Some(row) => {
                        if row.state != STATE_ABSENT {
                            errors.push(format!(
                                "frozen ledger row `{id}` state `{}` != `{STATE_ABSENT}`; carrier must be additive",
                                row.state
                            ));
                        }
                        if row.parity != PARITY_NOT_CLAIMED {
                            errors.push(format!(
                                "frozen ledger row `{id}` parity `{}` != `{PARITY_NOT_CLAIMED}`; carrier must be additive",
                                row.parity
                            ));
                        }
                    }
                    None => errors.push(format!("frozen ledger has no row for `{id}`")),
                }
            }
            let gaps: Vec<&str> = ledger
                .parity_ledger
                .named_gaps
                .iter()
                .map(|gap| gap.capability.as_str())
                .collect();
            if gaps != [NAMED_GAP] {
                errors.push(format!(
                    "frozen ledger named gaps {gaps:?} must be exactly [`{NAMED_GAP}`]"
                ));
            }
        }
        None => errors.push(format!("cannot parse {LEDGER_REL}")),
    }

    errors
}

fn load_raw() -> serde_json::Value {
    let raw = read_text(CARRIER_REL).unwrap_or_else(|error| panic!("{error}"));
    serde_json::from_str(&raw).unwrap_or_else(|error| panic!("{CARRIER_REL} is not JSON: {error}"))
}

fn load_carrier(raw: &serde_json::Value) -> Carrier {
    serde_json::from_value(raw.clone())
        .unwrap_or_else(|error| panic!("{CARRIER_REL} does not match the carrier schema: {error}"))
}

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

/// The carrier records both project-native learning surfaces as candidates,
/// resolves every link it names, and proves it layers over an unchanged frozen
/// M513 ledger (the ledger rows still say `absent`).
#[test]
fn learning_surfaces_are_recorded_as_candidates_separate_from_the_frozen_ledger() {
    let raw = load_raw();
    let carrier = load_carrier(&raw);
    let errors = validate(&carrier, &raw);
    assert!(errors.is_empty(), "{}", report(&errors));

    // Non-vacuity: two surfaces actually present, with resolved links.
    assert_eq!(carrier.surfaces.len(), 2, "two learning surfaces expected");
    for surface in &carrier.surfaces {
        assert!(
            !surface.implementation.rust.symbol.is_empty(),
            "`{}` must name a rust symbol",
            surface.id
        );
        assert!(
            !surface.implementation.test.name.is_empty(),
            "`{}` must name a test function",
            surface.id
        );
    }
}

/// Negative proof: dropping the authority digest, opening a lock, or adding an
/// undeclared top-level key must each be refused by name. Without these, a
/// carrier that quietly stopped binding the frozen bytes could still pass.
#[test]
fn carrier_rejects_dropped_authority_opened_lock_and_extra_keys() {
    let raw = load_raw();
    let clean = load_carrier(&raw);

    let mut dropped = clean.clone();
    dropped.authority.frozen_ledger.sha256 = "sha256:".to_string();
    let errors = validate(&dropped, &raw);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("frozen_ledger.sha256")),
        "dropped authority digest must be refused: {}",
        report(&errors)
    );

    let mut opened = clean.clone();
    opened.locks.import_eligible = true;
    let errors = validate(&opened, &raw);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("locks.import_eligible")),
        "an opened import lock must be refused: {}",
        report(&errors)
    );

    let mut unpinned = clean.clone();
    unpinned.authority.frozen_release_package.sha256 = "sha256:deadbeef".to_string();
    let errors = validate(&unpinned, &raw);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("frozen_release_package.sha256")),
        "a re-pinned release package must be refused: {}",
        report(&errors)
    );

    let mut extended = raw.clone();
    extended
        .as_object_mut()
        .expect("carrier is a JSON object")
        .insert("extra_field".to_string(), serde_json::json!(true));
    let errors = validate(&clean, &extended);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("top-level key set")),
        "an undeclared top-level key must be refused: {}",
        report(&errors)
    );

    // Non-vacuity: the clean carrier trips none of the above.
    let clean_errors = validate(&clean, &raw);
    assert!(clean_errors.is_empty(), "{}", report(&clean_errors));
}
