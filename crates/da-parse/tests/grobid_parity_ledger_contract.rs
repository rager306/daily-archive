//! Fail-closed contract for the GROBID Java -> Rust parity ledger (H21).
//!
//! Slice: M511-j7wz2f/S07. The ledger lives as DATA in the sanctioned carrier
//! `data/parse/grobid_compatibility.yaml::parity_ledger`; this integration
//! target is the machine check that keeps it honest. It is split into one
//! test per invariant so a failure names the exact rule that regressed, and
//! the aggregate test prints the per-row report with `--nocapture`.
//!
//! Why a separate target instead of a unit test: the ledger binds Rust
//! symbols and independent `tests/` targets across crates, so the check must
//! see the repository the way a reviewer does — from the filesystem — not the
//! way the library crate sees its own private modules.
//!
//! Environment independence is deliberate. The contract never opens
//! `/root/vendor-source/grobid`; it validates DATA against DATA (a
//! capture-pinned hash must equal the hash recorded in the capture artifact it
//! names). Recomputation of the pinned hashes against the vendor checkout is
//! S07/T06.
//!
//! Run:
//!
//! ```text
//! cargo test -p da-parse --test grobid_parity_ledger_contract
//! cargo test -p da-parse --test grobid_parity_ledger_contract -- --nocapture
//! ```

mod parity_ledger;

use parity_ledger as pl;

fn root() -> std::path::PathBuf {
    pl::workspace_root()
}

fn report(errors: &[String]) -> String {
    if errors.is_empty() {
        "no contract violations".to_string()
    } else {
        format!("{} violation(s):\n- {}", errors.len(), errors.join("\n- "))
    }
}

/// (a) Every capability has exactly one parity row and no stray row exists.
#[test]
fn row_ids_equal_capability_ids_exactly_once() {
    let errors = pl::check_row_identity(&pl::load(&root()));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// (b) A row may not restate scope or state differently from its capability;
/// the ledger is a projection, not a second source of truth.
#[test]
fn row_scope_and_state_match_the_capability() {
    let errors = pl::check_scope_parity(&pl::load(&root()));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// Scope vocabulary is read from `SCHEMA.yaml`; parity vocabulary is closed.
#[test]
fn scope_and_parity_vocabularies_are_closed() {
    let root = root();
    let doc = pl::load(&root);
    let schema = pl::load_schema(&root);
    let errors = pl::check_parity_vocabulary(&doc, &schema);
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// (c) Every `full` row resolves three links: a Java source with a non-empty
/// site, a Rust symbol that exists as a separate identifier in the named
/// file, and an independent `crates/**/tests/` target that defines the named
/// function. A row that cannot do this must be downgraded, not claimed.
#[test]
fn full_rows_resolve_java_rust_and_test_links() {
    let root = root();
    let errors = pl::check_links(&root, &pl::load(&root));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// (c) containment: a `rust.path` or `test.path` that carries a `..`
/// component (or is absolute) must be rejected by name, never followed, even
/// when it still looks like a `crates/**/tests/` target. The carrier holds
/// only repo-relative paths, so the guard is proven in memory — this is the
/// negative proof for Q3/S07 threat-surface gap 1 and the prerequisite for
/// the `rust_path_escape` / `test_path_absolute` mutations in T03.
#[test]
fn traversal_and_absolute_paths_are_rejected_by_name() {
    let root = root();
    // Passes both the `crates/` prefix and the `/tests/` substring checks, so
    // only the containment guard can stop it.
    let traversal = "crates/da-parse/tests/../../../../etc/hosts";

    let mut doc = pl::load(&root);
    let row = doc
        .parity_ledger
        .rows
        .iter_mut()
        .find(|row| row.parity == pl::PARITY_FULL && !row.rust.is_empty())
        .expect("at least one full row carries a rust link");
    row.rust[0].path = traversal.to_string();
    let errors = pl::check_links(&root, &doc);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("escapes the repository root")),
        "traversal rust.path must be rejected by name: {}",
        report(&errors)
    );

    let mut doc = pl::load(&root);
    let row = doc
        .parity_ledger
        .rows
        .iter_mut()
        .find(|row| row.test.is_some())
        .expect("at least one row carries a test link");
    row.test.as_mut().expect("test link present").path = traversal.to_string();
    let errors = pl::check_links(&root, &doc);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("escapes the repository root")),
        "traversal test.path must be rejected by name: {}",
        report(&errors)
    );

    // Non-vacuity: the carrier as written trips no containment error.
    let clean = pl::check_links(&root, &pl::load(&root));
    assert!(
        !clean
            .iter()
            .any(|error| error.contains("escapes the repository root")),
        "clean ledger must not trip the containment guard: {}",
        report(&clean)
    );
}

/// (d) Pinned Java content: 64 lowercase hex, capture-pinned hashes equal to
/// the capture they cite, and both revision fields populated. The observed
/// checkout revision differs from the capture-declared revision by design and
/// that discrepancy is recorded in `revision_discrepancy_note`.
#[test]
fn java_sources_are_pinned_and_capture_bound() {
    let root = root();
    let errors = pl::check_java_sources(&root, &pl::load(&root));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// Every declared failure vocabulary identifier really occurs in the source
/// file the row names, so `error_semantics` cannot rot into a claim.
#[test]
fn error_semantics_vocabulary_occurs_in_its_source() {
    let root = root();
    let errors = pl::check_error_semantics(&root, &pl::load(&root));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// (e) Each row's license identifier resolves to a `component_id` in the
/// license inventory and equals the capability's own license.
#[test]
fn every_row_license_resolves_in_the_inventory() {
    let root = root();
    let doc = pl::load(&root);
    let license = pl::load_license(&root);
    let errors = pl::check_license_resolution(&doc, &license);
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// (f)+(g) Absent rows are named by a scope-keyed reason and carry no links;
/// no absent row claims parity and no `required` capability is absent.
#[test]
fn absent_rows_are_named_and_claim_nothing() {
    let errors = pl::check_absent_rows(&pl::load(&root()));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// (h) The frozen denominators match the rows as written, including the
/// cross-check that every non-absent row is one of the three implemented
/// parities (full, partial, not_applicable_no_java_donor).
#[test]
fn frozen_denominators_match_the_rows() {
    let errors = pl::check_denominators(&pl::load(&root()));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// Named gaps are exactly the rows that are neither full nor not-claimed, and
/// each names a next step. Nothing is hidden.
#[test]
fn named_gaps_cover_every_non_full_non_absent_row() {
    let errors = pl::check_named_gaps(&pl::load(&root()));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// D127 / ADR-076 §14 locks are unchanged: no import, no deploy, and no
/// production route, graph write, or auto-promote.
#[test]
fn locks_are_unchanged() {
    let errors = pl::check_locks(&pl::load(&root()));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// Contract identity and the non-empty revision/prose fields.
#[test]
fn contract_identity_is_declared() {
    let errors = pl::check_contract_identity(&pl::load(&root()));
    assert!(errors.is_empty(), "{}", report(&errors));
}

/// Aggregate gate: run every check at once and print the 38-row report plus
/// the frozen denominators. This is the check the slice's verify command
/// relies on; with `--nocapture` it is the human-readable ledger.
#[test]
fn parity_ledger_is_fail_closed_and_prints_its_report() {
    let outcome = pl::evaluate(&root());
    println!("{}", outcome.report);
    assert!(outcome.errors.is_empty(), "{}", report(&outcome.errors));

    // A ledger that validates but is empty would be vacuous; assert the
    // frozen shape explicitly so the test cannot pass on an empty carrier.
    let doc = pl::load(&root());
    assert_eq!(doc.parity_ledger.rows.len(), 38, "row denominator");
    assert_eq!(doc.capabilities.len(), 38, "capability denominator");
    assert_eq!(
        doc.parity_ledger.named_gaps.len(),
        2,
        "named gap denominator"
    );
    assert!(
        outcome.report.contains("denominators:"),
        "report must print the frozen denominators"
    );
    assert!(
        pl::workspace_root().join(pl::COMPATIBILITY_DOC).exists(),
        "carrier must be the sanctioned data/parse/grobid_compatibility.yaml"
    );
}
