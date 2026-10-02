//! S08/T04 (M514-1lss2s) — real-corpus rollback rehearsal runner.
//!
//! This is the executable half of the rollback rehearsal. It reads committed
//! carriers only and never consults Java, never uses the network and never runs
//! a production route:
//!
//! * the frozen holdout manifest (`data/parse/holdout_manifests.yaml`, pointer
//!   `articles`) for the assigned 610-document census;
//! * the T01 acceptance carrier
//!   (`artifacts/parse_compat/native_corpus_acceptance.json`) for the native
//!   output the switched payload derives from, and for the per-document source
//!   PDF identity;
//! * the preserved-TEI provenance pins
//!   (`crates/da-parse/tests/fixtures/holdout/provenance.yaml`, pointer
//!   `documents`) for each baseline fixture path and sha256;
//! * the committed route receipt (`artifacts/parse_compat/native_route_e2e.json`)
//!   for the prospective limit freeze, whose revision is recomputed from the
//!   governor freeze carriers.
//!
//! For every assigned document it runs `baseline -> switched -> returned`
//! entirely in process memory, proves `returned == baseline` on the bytes, and
//! writes `artifacts/parse_compat/native_corpus_rollback_rehearsal.json`
//! atomically with `claim_scope = rehearsal_only`.
//!
//! ## Modes
//!
//! * `--self-check` — hermetic. No carrier is read and nothing is written.
//!   Rejects each named mutation (missing fixture claimed byte-identical,
//!   refusal without a reason, fabricated baseline on a refusal, non-identical
//!   return, leaked native fragment, vacuous switch, non-hermetic route,
//!   absolute fixture path, drifted sequence, opened lock, drifted freeze, a
//!   claim-scope drift) and proves the leak detector fires on a contaminated
//!   baseline.
//! * default — canonical run over the committed carriers, written atomically
//!   (tmp + rename).

use da_cli::native_acceptance::{
    DEFAULT_ACCEPTANCE_ARTIFACT_REL, HOLDOUT_MANIFEST_REL, load_holdout_documents,
};
use da_cli::native_corpus_resource_same_n::{
    AssignedIdentity, carrier_bare_digest, freeze_revision,
};
use da_cli::native_corpus_rollback_rehearsal::{
    CORPUS_ROLLBACK_GOVERNOR_POINTER, CORPUS_ROLLBACK_GOVERNOR_REL,
    CORPUS_ROLLBACK_ROUTE_RECEIPT_REL, CORPUS_ROLLBACK_TEI_PROVENANCE_REL,
    DEFAULT_CORPUS_ROLLBACK_REHEARSAL_ARTIFACT_REL, RollbackCorpusPins, RollbackDocumentInput,
    RollbackHermeticity, RollbackRehearsalExpectation, RollbackRehearsalInput,
    RollbackRehearsalReceipt, build_rollback_rehearsal_receipt, load_tei_fixture_pins,
    native_band_fragment, native_output_of, parse_native_measurement, run_rollback_sequence,
    validate_rollback_rehearsal_receipt,
};
use da_cli::native_family_receipt::CarrierBinding;
use da_cli::native_route_receipt::{RouteFreezeDeclaration, RouteLimitsBinding};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// ─────────────────────────────── arguments ───────────────────────────────

struct Args {
    root: PathBuf,
    report: PathBuf,
    self_check: bool,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_CORPUS_ROLLBACK_REHEARSAL_ARTIFACT_REL);
    let mut self_check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--self-check" => self_check = true,
            "--help" => {
                println!(
                    "native_corpus_rollback_rehearsal [--root DIR] [--report FILE] [--self-check]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    Args {
        root,
        report,
        self_check,
    }
}

fn main() -> ExitCode {
    let args = parse_args();
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_corpus_rollback_rehearsal: {error}");
            ExitCode::FAILURE
        }
    }
}

// ───────────────────────── carrier loading ─────────────────────────

fn read_carrier(root: &Path, rel: &str) -> Result<(Vec<u8>, String), Box<dyn Error>> {
    let bytes = fs::read(root.join(rel))?;
    let sha = da_cli::native_family_receipt::sha256_prefixed(&bytes);
    Ok((bytes, sha))
}

/// Recompute the governor freeze revision from the committed carriers the
/// `native_route_e2e.limit_policy.freeze_revision` block declares.
fn recompute_freeze(root: &Path, governor_bytes: &[u8]) -> Result<String, Box<dyn Error>> {
    let value: serde_yaml::Value = serde_yaml::from_slice(governor_bytes)?;
    let table =
        value[CORPUS_ROLLBACK_GOVERNOR_POINTER]["limit_policy"]["freeze_revision"]["carriers"]
            .as_sequence()
            .ok_or("the governor block declares no freeze carriers")?;
    let mut carriers = Vec::with_capacity(table.len());
    for carrier in table {
        let path = carrier["path"]
            .as_str()
            .ok_or("a freeze carrier has no path")?;
        let bytes = fs::read(root.join(path))?;
        carriers.push((path.to_owned(), carrier_bare_digest(&bytes)));
    }
    Ok(freeze_revision(&carriers))
}

fn assigned_from_manifest(text: &str) -> Result<Vec<AssignedIdentity>, Box<dyn Error>> {
    let documents = load_holdout_documents(text)?;
    Ok(documents
        .into_iter()
        .map(|document| AssignedIdentity {
            id: document.id,
            source_pdf: document.pdf_path,
            sha256: document.sha256,
        })
        .collect())
}

// ───────────────────────── full run ─────────────────────────

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    if args.self_check {
        return run_self_check();
    }

    // ── committed carriers ────────────────────────────────────────────────
    let (manifest_bytes, manifest_sha) = read_carrier(args.root.as_path(), HOLDOUT_MANIFEST_REL)?;
    let assigned = assigned_from_manifest(std::str::from_utf8(&manifest_bytes)?)?;

    let (t01_bytes, t01_sha) = read_carrier(args.root.as_path(), DEFAULT_ACCEPTANCE_ARTIFACT_REL)?;
    let t01 = parse_native_measurement(&t01_bytes)?;

    let (provenance_bytes, provenance_sha) =
        read_carrier(args.root.as_path(), CORPUS_ROLLBACK_TEI_PROVENANCE_REL)?;
    let tei_pins = load_tei_fixture_pins(std::str::from_utf8(&provenance_bytes)?)?;
    let tei_by_id: BTreeMap<&str, &_> = tei_pins
        .iter()
        .map(|pin| (pin.article_id.as_str(), pin))
        .collect();

    let (governor_bytes, governor_sha) =
        read_carrier(args.root.as_path(), CORPUS_ROLLBACK_GOVERNOR_REL)?;
    let recomputed = recompute_freeze(args.root.as_path(), &governor_bytes)?;

    let (route_bytes, route_sha) =
        read_carrier(args.root.as_path(), CORPUS_ROLLBACK_ROUTE_RECEIPT_REL)?;
    let route_value: serde_json::Value = serde_json::from_slice(&route_bytes)?;
    let limits: RouteLimitsBinding = serde_json::from_value(
        route_value
            .get("limits")
            .cloned()
            .ok_or("the route receipt carries no `limits` block")?,
    )?;

    // ── one rehearsal input per T01 document row ───────────────────────────
    let mut documents = Vec::with_capacity(t01.documents.len());
    for row in &t01.documents {
        let pin = tei_by_id.get(row.id.as_str()).ok_or_else(|| {
            format!(
                "the preserved-TEI provenance carries no fixture for `{}`",
                row.id
            )
        })?;
        let baseline = fs::read(args.root.join(&pin.path)).ok();
        let (native_state, layout_digest) = native_output_of(row);
        documents.push(RollbackDocumentInput {
            id: row.id.clone(),
            source_pdf: row.source_pdf.clone(),
            tei_path: pin.path.clone(),
            tei_sha256_pin: Some(pin.sha256.clone()),
            native_output_state: native_state.to_owned(),
            baseline,
            native_fragment: native_band_fragment(native_state, layout_digest),
        });
    }

    let native_measurement = CarrierBinding {
        manifest: DEFAULT_ACCEPTANCE_ARTIFACT_REL.to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: t01_sha,
    };
    let tei_provenance = CarrierBinding {
        manifest: CORPUS_ROLLBACK_TEI_PROVENANCE_REL.to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: provenance_sha,
    };

    let input = RollbackRehearsalInput {
        corpus_pins: RollbackCorpusPins {
            holdout_manifest: CarrierBinding {
                manifest: HOLDOUT_MANIFEST_REL.to_owned(),
                pointer: "articles".to_owned(),
                revision_sha256: manifest_sha,
            },
            native_measurement: native_measurement.clone(),
            tei_provenance: tei_provenance.clone(),
            route_receipt: CarrierBinding {
                manifest: CORPUS_ROLLBACK_ROUTE_RECEIPT_REL.to_owned(),
                pointer: "limits".to_owned(),
                revision_sha256: route_sha,
            },
        },
        freeze_governor: CarrierBinding {
            manifest: CORPUS_ROLLBACK_GOVERNOR_REL.to_owned(),
            pointer: format!(
                "{CORPUS_ROLLBACK_GOVERNOR_POINTER}.limit_policy.freeze_revision.carriers"
            ),
            revision_sha256: governor_sha,
        },
        freeze: RouteFreezeDeclaration {
            freeze_revision: recomputed.clone(),
            metrics: limits.metrics.clone(),
            direction_vocabulary: limits.direction_vocabulary.clone(),
        },
        limits,
        hermeticity: RollbackHermeticity {
            observed_java_calls: 0,
            network_observed: false,
            corpus_read_only: true,
            switch_in_memory_only: true,
            switch_positive_control: true,
        },
        documents,
        claim_scope: "rehearsal_only".to_owned(),
        product_coverage_effect: "none".to_owned(),
        thresholds: "not_accepted".to_owned(),
    };

    let receipt = build_rollback_rehearsal_receipt(&input)?;
    let expectation = RollbackRehearsalExpectation {
        freeze_revision: recomputed,
        native_measurement,
        tei_provenance,
        assigned,
    };
    validate_rollback_rehearsal_receipt(&receipt, &expectation)?;

    let bytes = receipt.artifact_json_bytes()?;
    write_artifact_atomically(&args.report, &bytes)?;

    Ok(format!(
        "rollback rehearsal `{}`: {} assigned documents, {} rehearsed, {} refused; \
         byte_identity_holds={} leak_free={} positive_control={}; freeze {}; \
         claim_scope={}; canonical run",
        receipt.schema_version,
        receipt.populations.assigned,
        receipt.populations.rehearsed,
        receipt.populations.refused,
        receipt.corpus_claim.byte_identity_holds,
        receipt.corpus_claim.native_fragment_leak_free,
        receipt.corpus_claim.switch_positive_control,
        receipt.limits.freeze_revision,
        receipt.claim_scope,
    ))
}

fn write_artifact_atomically(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

// ──────────────────────────────── self-check ────────────────────────────────

fn run_self_check() -> Result<String, Box<dyn Error>> {
    // The leak detector fires when a baseline already carries the native marker.
    let contaminated = run_rollback_sequence(
        format!(
            "<TEI>{}{NATIVE_BAND_FRAGMENT_MARKER}",
            NATIVE_BAND_FRAGMENT_MARKER
        )
        .as_bytes(),
        native_band_fragment("observed", Some("sha256:dead")).as_slice(),
    );
    if !contaminated.native_fragment_leaked {
        return Err("the leak detector did not fire on a contaminated baseline".into());
    }

    let (input, expectation) = synthetic_fixtures();
    let base = build_rollback_rehearsal_receipt(&input)?;
    validate_rollback_rehearsal_receipt(&base, &expectation)?;

    let mut checked = 0usize;

    // A rehearsed row whose returned bytes are not its baseline.
    let mut m = base.clone();
    m.documents[0].returned_sha256 = m.documents[0].switched_sha256.clone();
    expect_refusal(
        "returned_not_baseline",
        &m,
        &expectation,
        "RollbackNotByteIdentical",
    )?;
    checked += 1;

    // A leaked native fragment.
    let mut m = base.clone();
    m.documents[0].native_fragment_leaked = true;
    expect_refusal(
        "native_fragment_leaked",
        &m,
        &expectation,
        "NativeFragmentLeaked",
    )?;
    checked += 1;

    // A byte-identity claim that disagrees with the digests.
    let mut m = base.clone();
    m.documents[0].byte_identity = false;
    expect_refusal(
        "byte_identity_false",
        &m,
        &expectation,
        "RollbackNotByteIdentical",
    )?;
    checked += 1;

    // A refused row with no reason.
    let mut m = base.clone();
    m.documents[1].refusal_reason = None;
    expect_refusal(
        "refusal_without_reason",
        &m,
        &expectation,
        "RefusalReasonUnknown",
    )?;
    checked += 1;

    // A refused row carrying a fabricated baseline.
    let mut m = base.clone();
    m.documents[1].baseline_sha256 = Some(m.documents[0].baseline_sha256.clone().unwrap());
    expect_refusal(
        "refusal_with_baseline",
        &m,
        &expectation,
        "RefusalShapeInvalid",
    )?;
    checked += 1;

    // A missing fixture silently claimed byte-identical across the corpus.
    let mut m = base.clone();
    m.corpus_claim.byte_identity_holds = true;
    expect_refusal(
        "missing_fixture_claimed_identical",
        &m,
        &expectation,
        "CorpusClaimFalsified",
    )?;
    checked += 1;

    // A non-hermetic route.
    let mut m = base.clone();
    m.hermeticity.network_observed = true;
    expect_refusal(
        "non_hermetic_network",
        &m,
        &expectation,
        "NonHermeticRehearsal",
    )?;
    checked += 1;

    // A switch that lived outside process memory.
    let mut m = base.clone();
    m.hermeticity.switch_in_memory_only = false;
    expect_refusal(
        "switch_not_in_memory",
        &m,
        &expectation,
        "NonHermeticRehearsal",
    )?;
    checked += 1;

    // An absolute fixture path.
    let mut m = base.clone();
    m.documents[0].tei_path = "/tmp/leak.tei.xml".to_owned();
    expect_refusal(
        "absolute_fixture_path",
        &m,
        &expectation,
        "NonHermeticInput",
    )?;
    checked += 1;

    // A drifted sequence.
    let mut m = base.clone();
    m.sequence.steps = vec!["baseline".to_owned(), "returned".to_owned()];
    expect_refusal("sequence_drift", &m, &expectation, "SequenceDrift")?;
    checked += 1;

    // An opened lock.
    let mut m = base.clone();
    m.locks
        .insert("graph_write".to_owned(), "allowed".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;
    checked += 1;

    // A drifted freeze revision.
    let mut drifted = expectation.clone();
    drifted.freeze_revision = da_cli::native_family_receipt::sha256_prefixed(b"drifted");
    expect_refusal(
        "freeze_revision_drift",
        &base,
        &drifted,
        "StaleFreezeRevision",
    )?;
    checked += 1;

    // A claim-scope drift.
    let mut m = base.clone();
    m.claim_scope = "diagnostic_only".to_owned();
    expect_refusal("claim_scope_drift", &m, &expectation, "ClaimScopeDrift")?;
    checked += 1;

    Ok(format!(
        "rollback rehearsal self-check: {checked} named refusals executed plus the leak-detector \
         positive control, all refused"
    ))
}

fn expect_refusal(
    label: &str,
    receipt: &RollbackRehearsalReceipt,
    expectation: &RollbackRehearsalExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_rollback_rehearsal_receipt(receipt, expectation) {
        Ok(()) => Err(format!("{label} was admitted").into()),
        Err(error) => {
            let debug = format!("{error:?}");
            if debug.starts_with(variant) {
                Ok(())
            } else {
                Err(format!("{label} refused as `{debug}`, expected `{variant}`").into())
            }
        }
    }
}

use da_cli::native_corpus_rollback_rehearsal::NATIVE_BAND_FRAGMENT_MARKER;

/// Hermetic fixtures: two documents (one rehearsed, one refused), a valid freeze
/// and the matching receipt input/expectation. Nothing is read from disk.
fn synthetic_fixtures() -> (RollbackRehearsalInput, RollbackRehearsalExpectation) {
    let baseline = b"<TEI><text>committed baseline</text></TEI>".to_vec();
    let baseline_pin = da_cli::native_family_receipt::sha256_prefixed(&baseline);
    let freeze_digest = da_cli::native_family_receipt::sha256_prefixed(b"self-check-freeze");
    let limits = RouteLimitsBinding {
        freeze_revision: freeze_digest.clone(),
        freeze_state: "frozen".to_owned(),
        freeze_owner: "self-check".to_owned(),
        freeze_scope: "prospective_diagnostic_ceiling".to_owned(),
        tuned_after_scoring: "forbidden".to_owned(),
        zero_is_not_a_measurement: true,
        silent_clamp: "forbidden".to_owned(),
        pass_requires_frozen_threshold: true,
        claim_without_freeze: "forbidden".to_owned(),
        direction_vocabulary: vec!["floor".to_owned(), "ceiling".to_owned()],
        metrics: vec![da_cli::native_route_receipt::RouteMetricDeclaration {
            id: "java_cpu_wall_clock_ns".to_owned(),
            direction: "ceiling".to_owned(),
        }],
    };
    let native_measurement = CarrierBinding {
        manifest: "t01.json".to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: da_cli::native_family_receipt::sha256_prefixed(b"t01"),
    };
    let tei_provenance = CarrierBinding {
        manifest: "provenance.yaml".to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: da_cli::native_family_receipt::sha256_prefixed(b"prov"),
    };
    let corpus_pins = RollbackCorpusPins {
        holdout_manifest: CarrierBinding {
            manifest: "manifest.yaml".to_owned(),
            pointer: "articles".to_owned(),
            revision_sha256: da_cli::native_family_receipt::sha256_prefixed(b"manifest"),
        },
        native_measurement: native_measurement.clone(),
        tei_provenance: tei_provenance.clone(),
        route_receipt: CarrierBinding {
            manifest: "route.json".to_owned(),
            pointer: "limits".to_owned(),
            revision_sha256: da_cli::native_family_receipt::sha256_prefixed(b"route"),
        },
    };
    let documents = vec![
        RollbackDocumentInput {
            id: "d1".to_owned(),
            source_pdf: "a.pdf".to_owned(),
            tei_path: "fixtures/d1.tei.xml".to_owned(),
            tei_sha256_pin: Some(baseline_pin.clone()),
            native_output_state: "observed".to_owned(),
            baseline: Some(baseline),
            native_fragment: native_band_fragment("observed", Some("sha256:aaaa")),
        },
        RollbackDocumentInput {
            id: "d2".to_owned(),
            source_pdf: "b.pdf".to_owned(),
            tei_path: "fixtures/d2.tei.xml".to_owned(),
            tei_sha256_pin: Some(da_cli::native_family_receipt::sha256_prefixed(b"missing")),
            native_output_state: "not_run".to_owned(),
            baseline: None,
            native_fragment: native_band_fragment("not_run", None),
        },
    ];
    let input = RollbackRehearsalInput {
        corpus_pins,
        freeze_governor: CarrierBinding {
            manifest: "governor.yaml".to_owned(),
            pointer: "native_route_e2e.limit_policy.freeze_revision.carriers".to_owned(),
            revision_sha256: da_cli::native_family_receipt::sha256_prefixed(b"governor"),
        },
        freeze: RouteFreezeDeclaration {
            freeze_revision: freeze_digest.clone(),
            metrics: limits.metrics.clone(),
            direction_vocabulary: limits.direction_vocabulary.clone(),
        },
        limits,
        hermeticity: RollbackHermeticity {
            observed_java_calls: 0,
            network_observed: false,
            corpus_read_only: true,
            switch_in_memory_only: true,
            switch_positive_control: true,
        },
        documents,
        claim_scope: "rehearsal_only".to_owned(),
        product_coverage_effect: "none".to_owned(),
        thresholds: "not_accepted".to_owned(),
    };
    let expectation = RollbackRehearsalExpectation {
        freeze_revision: freeze_digest,
        native_measurement,
        tei_provenance,
        assigned: vec![
            AssignedIdentity {
                id: "d1".to_owned(),
                source_pdf: "a.pdf".to_owned(),
                sha256: "sha256:1111".to_owned(),
            },
            AssignedIdentity {
                id: "d2".to_owned(),
                source_pdf: "b.pdf".to_owned(),
                sha256: "sha256:2222".to_owned(),
            },
        ],
    };
    (input, expectation)
}
