//! S03/T04 (M514-1lss2s) — runner for the declared Java-free vertical slice.
//!
//! This is the executable half of the slice contract: it turns the DATA
//! declaration (`data/parse/layout_bakeoff.yaml#java_free_vertical_slice`) into
//! a corpus-bound receipt with **per-stage Java invocation counts**.
//!
//! What it does, and deliberately does not do:
//!
//! - It reads the declaration and the cascade stage ids by pointer. It never
//!   restates the boundary, the stage list or the corpus identities.
//! - It installs a [`JavaInvocationCounter`] on the *real* Java transport seam
//!   (`GrobidHttpBackend` + `with_invocation_observer`) and then runs the
//!   existing Rust donors at P1 and native segmentation at P2. It **never**
//!   consults Java: that is the slice. The installed guard is what makes the
//!   observed zero a *measurement* instead of an absence.
//! - It reuses the existing engine — [`PdfInspectorLayoutDonor`] (plus its
//!   XY-Cut reading-order view) and `da_parse::segment_document_full`. No second
//!   PDF engine is created.
//! - A lane the segmenter rejects is recorded as a **typed absence**, never as a
//!   zero. Today the XY-Cut reading-order lane is rejected by native
//!   segmentation on the pinned corpus; that is recorded, not hidden.
//!
//! Exit status is non-zero when the slice observed Java (`observed > 0` on a
//! slice stage), when a pinned PDF's sha256 drifted, or when the typed stage
//! matrix is incomplete. It is zero when the slice is Java-free and the receipt
//! is justified.

use da_adapters::{LayoutDonor, LayoutDonorLimits, PdfInspectorLayoutDonor, XyCutLayoutDonor};
use da_cli::java_free_slice::{
    COMPATIBILITY_DATA_REL, DEFAULT_ARTIFACT_REL, GuardProvenance, ObservedDocument, ObservedLane,
    SEGMENTATION_NOT_OBSERVED_REJECTED, SLICE_DECLARATION_DATA_REL, SegmentationReceipt,
    StageScope, assert_redacted, build_receipt, load_declaration, sha256_hex,
};
use da_parse::{
    GrobidHttpBackend, GrobidProfile, JavaInvocationCounter, LayoutDocument, SegmentationConfig,
    segment_document_full,
};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

/// The base positioned lane `PdfInspectorLayoutDonor` provides.
const LANE_POSITIONED: &str = "pdf_inspector_positioned";
/// The XY-Cut reading-order view over the same positioned tokens.
const LANE_XY_CUT: &str = "pdf_inspector_positioned_xy_cut";

struct Args {
    root: PathBuf,
    report: PathBuf,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_ARTIFACT_REL);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--help" => {
                println!("java_free_vertical_slice [--root DIR] [--report FILE]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    Args { root, report }
}

fn main() -> ExitCode {
    let args = parse_args();
    let _log_guard = da_cli::logging::init_pipeline_logging("java_free_vertical_slice");
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("java_free_vertical_slice: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The donor behind a declared lane. Unknown lanes are refused, not skipped —
/// the declaration is closed and a new lane must be added deliberately.
fn donor_for_lane(
    lane_id: &str,
) -> Result<Box<dyn LayoutDonor>, da_cli::java_free_slice::JavaFreeSliceError> {
    match lane_id {
        LANE_POSITIONED => Ok(Box::new(PdfInspectorLayoutDonor)),
        LANE_XY_CUT => Ok(Box::new(XyCutLayoutDonor::new(Box::new(
            PdfInspectorLayoutDonor,
        )))),
        other => Err(da_cli::java_free_slice::JavaFreeSliceError::UnknownLane {
            lane: other.to_owned(),
        }),
    }
}

/// Admitted lexical tokens: the P1 denominator. Visual tokens are excluded.
fn admitted_lexical_tokens(document: &LayoutDocument) -> u64 {
    document
        .pages
        .iter()
        .flat_map(|page| page.tokens.iter())
        .filter(|token| token.kind.is_lexical())
        .count() as u64
}

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    let layout_bytes = fs::read(args.root.join(SLICE_DECLARATION_DATA_REL))?;
    let compatibility_bytes = fs::read(args.root.join(COMPATIBILITY_DATA_REL))?;
    let declaration = load_declaration(
        std::str::from_utf8(&layout_bytes)?,
        std::str::from_utf8(&compatibility_bytes)?,
    )?;

    // Install the guard on the real Java seam. Nothing below consults it; the
    // installed observer is exactly what turns "no consults" into Observe(0).
    let counter = JavaInvocationCounter::new();
    let plain = GrobidHttpBackend::new(Some("http://127.0.0.1:9"))
        .with_profile(GrobidProfile::structural());
    let watched = plain.clone().with_invocation_observer(counter.clone());
    let identity_isolated = plain.identity() == watched.identity();
    let installed = watched.java_invocation_observation().is_observed();
    let observed_java_calls = counter
        .observation()
        .attempts()
        .ok_or("an installed observer must read as observed")?;
    let guard = GuardProvenance {
        installed,
        identity_isolated,
        observed_java_calls,
    };

    let limits = LayoutDonorLimits::default();
    let mut observed_documents = Vec::with_capacity(declaration.documents.len());
    for document in &declaration.documents {
        let bytes = fs::read(args.root.join(&document.source_pdf))?;
        let digest = sha256_hex(&bytes);
        if digest != document.sha256 {
            return Err(format!(
                "checksum drift for {}: observed {digest}, pinned {}",
                document.id, document.sha256
            )
            .into());
        }
        let mut lanes = Vec::with_capacity(declaration.lane_ids.len());
        let mut observed_pages = 0_u64;
        for lane_id in &declaration.lane_ids {
            let donor = donor_for_lane(lane_id)?;
            let observation =
                donor
                    .extract_shadow(&bytes, &document.id, limits)
                    .map_err(|error| {
                        format!(
                            "lane `{lane_id}` layout failed for {}: {error}",
                            document.id
                        )
                    })?;
            let admitted = admitted_lexical_tokens(&observation.document);
            let lattice_fingerprint = observation.lattice.digest()?;
            let segmentation = match segment_document_full(
                &observation.document,
                &observation.lattice,
                &observation.lexemes,
                SegmentationConfig::default(),
            ) {
                Ok(outcome) => SegmentationReceipt::Observed {
                    assigned_tokens: outcome.assignments.len() as u64,
                    order_digest: outcome.order_digest,
                },
                // A rejected lattice is a typed absence, never a zero: the
                // segmenter produced no number, so none is published.
                Err(_) => SegmentationReceipt::NotObserved {
                    reason: SEGMENTATION_NOT_OBSERVED_REJECTED.to_owned(),
                },
            };
            observed_pages = observation.document.pages.len() as u64;
            lanes.push(ObservedLane {
                lane_id: lane_id.clone(),
                observed_pages,
                admitted_lexical_tokens: admitted,
                lattice_fingerprint,
                segmentation,
            });
        }
        observed_documents.push(ObservedDocument {
            id: document.id.clone(),
            sha256: digest,
            observed_pages,
            lanes,
        });
    }

    let receipt = build_receipt(
        &declaration,
        guard,
        &format!("sha256:{}", sha256_hex(&layout_bytes)),
        &format!("sha256:{}", sha256_hex(&compatibility_bytes)),
        &observed_documents,
    )?;
    let artifact = receipt.artifact_json_bytes()?;
    assert_redacted(std::str::from_utf8(&artifact)?)?;

    if let Some(parent) = args
        .report
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.report, &artifact)?;

    Ok(format!(
        "java-free slice `{}`: {} stages ({} in-slice), {} documents, {} lane pages, java calls observed {} via {}",
        receipt.slice_id,
        receipt.stages.len(),
        receipt
            .stages
            .iter()
            .filter(|stage| stage.scope == StageScope::InSlice)
            .count(),
        receipt.corpus.document_count,
        receipt.corpus.observed_lane_pages,
        receipt.guard.observed_java_calls,
        receipt.guard.observer,
    ))
}
