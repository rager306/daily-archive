//! Mutation matrix over the GROBID Java -> Rust parity ledger (M511-j7wz2f/S07/T03).
//!
//! A fail-closed ledger is only as good as the tampering it rejects: on a clean
//! carrier the validator is indistinguishable from no validator at all. This
//! target therefore takes the DATA-declared `parity_ledger.mutations`
//! dictionary, clones the real ledger into memory, applies one mutation at a
//! time, and asserts the validator catches each one by EXACTLY the declared
//! path (the error token recorded in DATA), not merely "some error".
//!
//! Design rules enforced here:
//!
//! * The mutation registry lives in DATA, not in a second copy in this test.
//!   The applied registry must equal the declared dictionary; an id with no
//!   applier is a failure, and an applier with no declaration is unreachable.
//! * No rule of `parity_ledger/mod.rs` is weakened to let a mutation pass. A
//!   surviving or misdirected mutation is a FAIL and is reported by name.
//! * Anti-no-op: each mutation must actually change the serialized input, so a
//!   mutation that quietly does nothing cannot masquerade as a detection.
//! * Non-vacuity: the clean ledger must yield zero errors, and the scanned
//!   carrier must be non-trivial (>4096 bytes), so a zero is a real zero.
//!
//! H21-N1 = an incomplete three-link binding (a row that claims parity without
//! a Java source, a Rust symbol, or an independent test) must be rejected.
//! H21-N2 = a scope substitution (a row whose `scope` no longer equals its
//! capability's `product_scope`) must be rejected. Both are covered by named
//! mutations below.
//!
//! Run:
//!
//! ```text
//! cargo test -p da-parse --test grobid_parity_ledger_mutations
//! cargo test -p da-parse --test grobid_parity_ledger_mutations -- --nocapture
//! ```

mod parity_ledger;

use parity_ledger as pl;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    pl::workspace_root()
}

fn row_mut<'a>(
    doc: &'a mut pl::CompatibilityDocument,
    capability: &str,
) -> Result<&'a mut pl::Row, String> {
    doc.parity_ledger
        .rows
        .iter_mut()
        .find(|row| row.capability == capability)
        .ok_or_else(|| format!("row `{capability}` not found for mutation"))
}

fn java_source_mut<'a>(
    doc: &'a mut pl::CompatibilityDocument,
    id: &str,
) -> Result<&'a mut pl::JavaSource, String> {
    doc.parity_ledger
        .java_sources
        .iter_mut()
        .find(|source| source.id == id)
        .ok_or_else(|| format!("java source `{id}` not found for mutation"))
}

/// The registry: stable mutation id -> in-memory tamper of a loaded ledger.
///
/// Every id declared in `parity_ledger.mutations` must have an arm here. The
/// `other` arm is a hard error so an undeclared or misspelled id can never be
/// silently skipped.
fn apply_mutation(doc: &mut pl::CompatibilityDocument, id: &str) -> Result<(), String> {
    match id {
        // --- H21-N1: an incomplete three-link binding -----------------------
        "missing_java_source_id" => {
            row_mut(doc, "rest.isalive")?.java = None;
        }
        "missing_rust_entry" => {
            row_mut(doc, "rest.isalive")?.rust.clear();
        }
        "missing_test_link" => {
            row_mut(doc, "rest.isalive")?.test = None;
        }
        // --- H21-N2: a scope substitution ----------------------------------
        "scope_substitution_supporting_to_required" => {
            row_mut(doc, "rest.processDate")?.scope = "required".to_string();
        }
        "scope_substitution_required_to_supporting" => {
            row_mut(doc, "rest.isalive")?.scope = "supporting".to_string();
        }
        // --- absence claims -------------------------------------------------
        "absent_row_claims_parity" => {
            row_mut(doc, "rest.processFulltextAssetDocument")?.parity =
                pl::PARITY_NOT_APPLICABLE.to_string();
        }
        "full_without_links" => {
            row_mut(doc, "rest.processFulltextAssetDocument")?.parity = pl::PARITY_FULL.to_string();
        }
        "absent_row_missing_reason" => {
            row_mut(doc, "rest.processFulltextAssetDocument")?.reason = None;
        }
        // --- row identity ---------------------------------------------------
        "unknown_capability_id" => {
            row_mut(doc, "rest.isalive")?.capability = "rest.unknownCapabilityXYZ".to_string();
        }
        "missing_row" => {
            let index = doc
                .parity_ledger
                .rows
                .iter()
                .position(|row| row.capability == "rest.isalive")
                .ok_or("row `rest.isalive` not found for mutation")?;
            doc.parity_ledger.rows.remove(index);
        }
        "duplicate_row" => {
            let duplicate = {
                let row = row_mut(doc, "rest.isalive")?;
                row.clone()
            };
            doc.parity_ledger.rows.push(duplicate);
        }
        // --- java hash pinning ----------------------------------------------
        "java_sha256_mismatch_vs_capture" => {
            java_source_mut(doc, "java-grobid-fulltext-parser")?.sha256 = "0".repeat(64);
        }
        "java_sha256_not_hex" => {
            java_source_mut(doc, "java-grobid-paths")?.sha256 = "zz".to_string();
        }
        // --- rust / test link resolution ------------------------------------
        "rust_symbol_not_found_in_file" => {
            row_mut(doc, "rest.isalive")?.rust[0].symbol = "NoSuchSymbolXYZ".to_string();
        }
        "test_fn_not_found_in_target" => {
            row_mut(doc, "rest.isalive")?
                .test
                .as_mut()
                .ok_or("row `rest.isalive` has no test link")?
                .name = "no_such_test_fn_xyz".to_string();
        }
        "test_path_not_a_tests_target" => {
            row_mut(doc, "rest.isalive")?
                .test
                .as_mut()
                .ok_or("row `rest.isalive` has no test link")?
                .path = "crates/da-parse/src/lib.rs".to_string();
        }
        // --- containment ----------------------------------------------------
        // Both pass the `crates/` prefix and `/tests/` substring checks, so
        // only the containment guard can stop them.
        "rust_path_escape" => {
            row_mut(doc, "rest.isalive")?.rust[0].path =
                "crates/da-parse/tests/../../../../etc/hosts".to_string();
        }
        "test_path_absolute" => {
            row_mut(doc, "rest.isalive")?
                .test
                .as_mut()
                .ok_or("row `rest.isalive` has no test link")?
                .path = "/etc/hosts".to_string();
        }
        // --- fourth link: measurement coverage / identity (S03, D471) -------
        "measurement_missing_on_required_row" => {
            row_mut(doc, "stage.header_composition")?.measurement = None;
        }
        "measurement_on_absent_row" => {
            row_mut(doc, "rest.processFulltextAssetDocument")?.measurement =
                Some(pl::RowMeasurement {
                    state: pl::MEASURED_BY_CORPUS_PARITY.to_string(),
                    classes: vec!["AdmittedAuthorLine".to_string()],
                    reason: None,
                });
        }
        "measurement_unknown_class" => {
            let measurement = row_mut(doc, "stage.header_composition")?
                .measurement
                .as_mut()
                .ok_or("row `stage.header_composition` has no measurement")?;
            measurement.classes[0] = "NotADeclaredClassXYZ".to_string();
        }
        "measurement_unknown_reason" => {
            let measurement = row_mut(doc, "rest.isalive")?
                .measurement
                .as_mut()
                .ok_or("row `rest.isalive` has no measurement")?;
            measurement.reason = Some("not_a_declared_reason_xyz".to_string());
        }
        "measurement_artifact_sha_drift" => {
            let measurement = doc
                .parity_ledger
                .parity_measurement
                .as_mut()
                .ok_or("ledger has no parity_measurement block")?;
            measurement.artifact.sha256 = "0".repeat(64);
        }
        "measurement_identity_digest_drift" => {
            let measurement = doc
                .parity_ledger
                .parity_measurement
                .as_mut()
                .ok_or("ledger has no parity_measurement block")?;
            measurement.identity_digest =
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string();
        }
        "measurement_pooled_claim_allowed" => {
            let measurement = doc
                .parity_ledger
                .parity_measurement
                .as_mut()
                .ok_or("ledger has no parity_measurement block")?;
            measurement.pooled_quality_claim = "allowed".to_string();
        }
        // --- absent-disposition fates (S05, D478/D479) --------------------
        "absent_disposition_missing_entry" => {
            doc.parity_ledger
                .absent_dispositions
                .remove("rest.processCitationPatentTEI");
        }
        "absent_disposition_stray_entry" => {
            doc.parity_ledger.absent_dispositions.insert(
                "rest.isalive".to_string(),
                pl::AbsentDisposition {
                    disposition: pl::DISPOSITION_OUT_OF_PRODUCT_SCOPE.to_string(),
                    reason: "patent_family_out_of_scope".to_string(),
                    rationale: "A stray key that is neither an absent row nor the named gap."
                        .to_string(),
                },
            );
        }
        "absent_disposition_unknown_disposition" => {
            let entry = doc
                .parity_ledger
                .absent_dispositions
                .get_mut("rest.processCitationPatentTEI")
                .ok_or("absent disposition `rest.processCitationPatentTEI` not found")?;
            entry.disposition = "not_a_declared_disposition".to_string();
        }
        "absent_disposition_unknown_reason" => {
            let entry = doc
                .parity_ledger
                .absent_dispositions
                .get_mut("rest.processCitationPatentTEI")
                .ok_or("absent disposition `rest.processCitationPatentTEI` not found")?;
            entry.reason = "not_a_declared_reason".to_string();
        }
        "absent_disposition_empty_rationale" => {
            let entry = doc
                .parity_ledger
                .absent_dispositions
                .get_mut("rest.processCitationPatentTEI")
                .ok_or("absent disposition `rest.processCitationPatentTEI` not found")?;
            entry.rationale = "   ".to_string();
        }
        "absent_disposition_in_horizon_without_order" => {
            doc.parity_ledger
                .horizon_order
                .retain(|ordered| ordered != "learning.ruvllm_ablation");
        }
        "absent_disposition_out_of_scope_with_order" => {
            doc.parity_ledger
                .horizon_order
                .push("rest.processCitationPatentTEI".to_string());
        }
        "absent_disposition_duplicate_horizon_order" => {
            doc.parity_ledger
                .horizon_order
                .push("learning.ruvllm_ablation".to_string());
        }
        "absent_disposition_horizon_gap" => {
            doc.parity_ledger
                .horizon_order
                .retain(|ordered| ordered != "learning.sona_trajectory");
        }
        "absent_disposition_named_gap_disagreement" => {
            let entry = doc
                .parity_ledger
                .absent_dispositions
                .get_mut("learning.ruvllm_ablation")
                .ok_or("absent disposition `learning.ruvllm_ablation` not found")?;
            entry.reason = "project_native_learning_line".to_string();
        }
        // --- license identity -------------------------------------------------
        "license_identity_unresolved" => {
            row_mut(doc, "rest.isalive")?.license = "inventory:missing-component-xyz".to_string();
        }
        // --- frozen denominators ---------------------------------------------
        "denominators_drift" => {
            doc.parity_ledger.denominators.capabilities = 99;
        }
        other => {
            return Err(format!(
                "no applier registered for mutation `{other}`; the registry must equal the DATA dictionary"
            ));
        }
    }
    Ok(())
}

fn declared(root: &Path) -> Vec<pl::Mutation> {
    let doc = pl::load(root);
    doc.parity_ledger
        .mutations
        .iter()
        .map(|mutation| pl::Mutation {
            id: mutation.id.clone(),
            expected_detection: mutation.expected_detection.clone(),
        })
        .collect()
}

fn declared_token(root: &Path, id: &str) -> String {
    declared(root)
        .into_iter()
        .find(|mutation| mutation.id == id)
        .unwrap_or_else(|| panic!("mutation `{id}` is not declared in DATA"))
        .expected_detection
}

/// Apply `id` to a fresh copy of the real ledger and return the validator
/// errors plus the declared detection token.
fn detect(root: &Path, id: &str) -> (String, Vec<String>) {
    let expected = declared_token(root, id);
    let mut doc = pl::load(root);
    apply_mutation(&mut doc, id).unwrap_or_else(|error| panic!("{error}"));
    let outcome = pl::evaluate_document(root, doc);
    (expected, outcome.errors)
}

/// The matrix: every declared mutation is applied once, must really change the
/// input, and must be caught by its declared token. Nothing may survive.
#[test]
fn mutation_matrix_has_no_survivors_and_matches_data() {
    let root = root();

    // Non-vacuity preconditions: the scanned carrier must be non-trivial and
    // the clean ledger must validate, otherwise a zero survivor count would be
    // meaningless.
    let carrier = std::fs::read_to_string(root.join(pl::COMPATIBILITY_DOC))
        .expect("carrier readable for the mutation matrix");
    assert!(
        carrier.len() > 4096,
        "scanned carrier is only {} bytes; a zero-error result would be vacuous",
        carrier.len()
    );
    let clean = pl::evaluate_document(&root, pl::load(&root));
    assert!(
        clean.errors.is_empty(),
        "clean ledger must yield 0 errors before mutations mean anything: {:?}",
        clean.errors
    );
    // Read the rendered report so the aggregate `Outcome` is exercised the
    // same way the contract target uses it (and dead-code analysis sees it).
    assert!(
        clean.report.contains("denominators:"),
        "clean ledger report must render its frozen denominators"
    );

    // DATA integrity: stable, unique ids and non-empty detection tokens.
    let mut declared_ids: BTreeSet<String> = BTreeSet::new();
    for mutation in &declared(&root) {
        assert!(
            !mutation.id.trim().is_empty(),
            "a mutation id is empty in DATA"
        );
        assert!(
            !mutation.expected_detection.trim().is_empty(),
            "mutation `{}` has an empty expected_detection in DATA",
            mutation.id
        );
        assert!(
            declared_ids.insert(mutation.id.clone()),
            "mutation id `{}` is duplicated in DATA",
            mutation.id
        );
    }
    assert!(
        declared_ids.len() >= 12,
        "the mutation matrix must declare at least 12 mutations, found {}",
        declared_ids.len()
    );

    let baseline = pl::load(&root);
    let original = serde_yaml::to_string(&baseline)
        .expect("baseline ledger serializes for the anti-no-op check");

    let mut applied: BTreeSet<String> = BTreeSet::new();
    let mut detected = 0usize;
    let mut survivors: Vec<String> = Vec::new();
    let mut misdirected: Vec<String> = Vec::new();

    for mutation in &baseline.parity_ledger.mutations {
        let mut doc = pl::load(&root);
        apply_mutation(&mut doc, &mutation.id)
            .unwrap_or_else(|error| panic!("mutation `{}`: {error}", mutation.id));

        // Anti-no-op: the tamper must actually alter the input.
        let mutated = serde_yaml::to_string(&doc).expect("mutated ledger serializes");
        assert_ne!(
            mutated, original,
            "mutation `{}` did not change the ledger (no-op)",
            mutation.id
        );

        applied.insert(mutation.id.clone());

        let outcome = pl::evaluate_document(&root, doc);
        match outcome
            .errors
            .iter()
            .find(|error| error.contains(&mutation.expected_detection))
        {
            Some(error) => {
                detected += 1;
                println!(
                    "MUTATION {} -> {} (expected {})",
                    mutation.id, error, mutation.expected_detection
                );
            }
            None if outcome.errors.is_empty() => {
                survivors.push(mutation.id.clone());
                println!(
                    "MUTATION {} SURVIVED: no error at all (expected `{}`)",
                    mutation.id, mutation.expected_detection
                );
            }
            None => {
                misdirected.push(mutation.id.clone());
                println!(
                    "MUTATION {} MISDETECTED: expected `{}`, got {:?}",
                    mutation.id, mutation.expected_detection, outcome.errors
                );
            }
        }
    }

    println!(
        "{} declared mutations, {} detected, {} survived",
        declared_ids.len(),
        detected,
        survivors.len() + misdirected.len()
    );

    // Registry == DATA: exactly the declared ids, all applied.
    assert_eq!(
        applied, declared_ids,
        "the applied registry must equal the declared DATA dictionary"
    );
    assert!(
        survivors.is_empty() && misdirected.is_empty(),
        "fail-closed broken: survivors {survivors:?}, misdirected {misdirected:?}"
    );
    assert_eq!(
        detected,
        declared_ids.len(),
        "every declared mutation must be detected by its declared path"
    );
}

/// H21-N1: a row that keeps `parity: full` after losing its independent test
/// link must be rejected by the link check.
#[test]
fn h21_n1_incomplete_test_link_is_rejected_by_name() {
    let root = root();
    let (expected, errors) = detect(&root, "missing_test_link");
    assert!(
        errors.iter().any(|error| error.contains(&expected)),
        "H21-N1: missing_test_link must be detected by `{expected}`: {errors:?}"
    );
}

/// H21-N2: a scope substitution in either direction must be rejected by the
/// scope-parity check, so `scope` can never drift from `product_scope`.
#[test]
fn h21_n2_scope_substitution_is_rejected_by_name() {
    let root = root();
    for id in [
        "scope_substitution_supporting_to_required",
        "scope_substitution_required_to_supporting",
    ] {
        let (expected, errors) = detect(&root, id);
        assert!(
            errors.iter().any(|error| error.contains(&expected)),
            "H21-N2: {id} must be detected by `{expected}`: {errors:?}"
        );
    }
}

/// The mutation matrix can only be trusted if the detector it exercises is the
/// same code the contract target runs: on the untouched carrier both must
/// agree on zero errors.
#[test]
fn clean_carrier_agrees_with_the_contract_target() {
    let root = root();
    let direct = pl::evaluate_document(&root, pl::load(&root));
    let via_evaluate = pl::evaluate(&root);
    assert!(direct.errors.is_empty(), "{:?}", direct.errors);
    assert_eq!(direct.errors, via_evaluate.errors);
}
