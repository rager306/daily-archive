//! S08/T04 route-recovery ledger emitter.
//!
//! Joins the closed P0..P9 cascade skeleton (`data/parse/grobid_compatibility.yaml`),
//! the component admission matrix (`data/parse/native_fulltext_composition.yaml`)
//! and the measured Java-work receipt (`artifacts/parse_compat/native_java_work.json`,
//! S08/T03) into `artifacts/parse_compat/native_route_recovery.json`: one verdict
//! per stage and the measured Java model invocations the recoverable stages
//! account for.
//!
//! Byte-reproducible for the same inputs by construction — nothing observational
//! (wall clock, host path, tape location) enters the artifact, and every
//! collection is ordered. The only filesystem observation that does enter is
//! which declared native surfaces exist, which is recorded per stage as a closed
//! `present`/`absent`/`none_declared` state rather than as a path.
//!
//! Nothing here dispatches a route: `production_route`, `auto_promote`,
//! `graph_mutation` and `selection` stay `forbidden`, `deployed` and
//! `import_eligible` stay false, and H05 is not closed.
//!
//! If the Java-work receipt carries no measurement, the ledger says so, every
//! would-be-recoverable stage becomes `not_observed`, and no total is published.
//! An absent core qualification receipt withholds recovery the same way.

use anyhow::{Context, Result};
use clap::Parser;
use da_cli::native_route_recovery::{
    JavaWorkInput, ROUTE_RECOVERY_ARTIFACT_REL, RecoverableJavaWork, RouteRecoveryLedger,
    VerdictReason, load_from_root,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
struct Args {
    /// Repository root the DATA tables and receipts are read from.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Artifact path, relative to the repository root.
    #[arg(long, default_value = ROUTE_RECOVERY_ARTIFACT_REL)]
    artifact: PathBuf,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn write_artifact(root: &Path, relative: &Path, bytes: &[u8]) -> Result<()> {
    let path = if relative.is_absolute() {
        relative.to_path_buf()
    } else {
        root.join(relative)
    };
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create artifact dir {}", parent.display()))?;
    }
    std::fs::write(&path, bytes).with_context(|| format!("write artifact {}", path.display()))
}

fn report_stdout(ledger: &RouteRecoveryLedger, bytes: &[u8], artifact: &Path) {
    println!(
        "native route recovery: schema={} corpus={} documents={}",
        ledger.schema_version, ledger.corpus.corpus_id, ledger.corpus.corpus_n
    );
    match &ledger.java_work {
        JavaWorkInput::Measured {
            unit,
            corpus_documents,
            documents_measured,
            total_invocations,
            coverage,
            tape_verification,
            ..
        } => println!(
            "  java work: measured unit={unit} documents={documents_measured}/{corpus_documents} \
             total_invocations={total_invocations} coverage={coverage:?} tape_verification={tape_verification}"
        ),
        JavaWorkInput::NotObserved { reason, .. } => {
            println!("  java work: not_observed reason={reason:?} (absence is not zero)")
        }
    }
    println!(
        "  core qualification: profile={} ({}/{}) smp={} entries={} scalar_oracle_bound={}",
        ledger.core_qualification.profile_receipt_present,
        ledger.core_qualification.profile_optimization_state,
        ledger.core_qualification.profile_pgo_disposition,
        ledger.core_qualification.smp_receipt_present,
        ledger.core_qualification.smp_matrix_entries,
        ledger.core_qualification.smp_scalar_oracle_bound,
    );
    if !ledger.core_qualification.missing_receipts.is_empty() {
        println!(
            "  core qualification missing: {}",
            ledger.core_qualification.missing_receipts.join(", ")
        );
    }
    println!("  stages:");
    for stage in &ledger.stages {
        let components = if stage.components.is_empty() {
            "-".to_string()
        } else {
            stage
                .components
                .iter()
                .map(|component| {
                    format!(
                        "{}={}",
                        component.component_id, component.matrix_disposition
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        let attributed = match stage.attributed_java_work.value() {
            Some(value) => value.to_string(),
            None => "not_observed".to_string(),
        };
        println!(
            "    {} {} authority={} surface={:?} components=[{}] conditions={}/{} attributed={} reason={:?}",
            stage.stage,
            stage.verdict.as_str(),
            stage.production_authority,
            stage.native_surface_state,
            components,
            stage
                .conditions
                .rows()
                .iter()
                .filter(|(_, state)| matches!(
                    state,
                    da_cli::native_route_recovery::ConditionState::Met
                ))
                .count(),
            stage.conditions.rows().len(),
            attributed,
            stage.verdict_reason
        );
        if stage.verdict_reason != VerdictReason::AllConditionsMet
            && stage.verdict_reason != VerdictReason::ComponentNotAdmitted
        {
            continue;
        }
        for model in &stage.java_models_observed {
            match model.invocations.value() {
                Some(value) => println!("      {} = {}", model.model, value),
                None => println!("      {} = not_observed", model.model),
            }
        }
    }
    match &ledger.recoverable_java_work {
        RecoverableJavaWork::Observed {
            unit,
            value,
            by_stage,
            claim,
            ..
        } => {
            println!("  recoverable java work: {value} {unit} claim={claim}");
            for (stage, value) in by_stage {
                println!("    {stage} = {value}");
            }
        }
        RecoverableJavaWork::NotObserved { reason, claim, .. } => println!(
            "  recoverable java work: not_observed reason={reason:?} claim={claim} \
             (no total is published from absence)"
        ),
    }
    println!(
        "  gaps: citation_names={} references={} pooling={}",
        ledger.denominators.named_gaps.citation_names,
        ledger.denominators.named_gaps.references,
        ledger.denominators.named_gaps.pooling
    );
    println!(
        "  locks: production_route={} deployed={} import_eligible={} auto_promote={} \
         graph_mutation={} selection={} decision_point={} independently_selectable={:?} h05_closure={}",
        ledger.locks.production_route,
        ledger.locks.deployed,
        ledger.locks.import_eligible,
        ledger.locks.auto_promote,
        ledger.locks.graph_mutation,
        ledger.locks.selection,
        ledger.locks.decision_point,
        ledger.locks.independently_selectable,
        ledger.h05_closure
    );
    println!(
        "artifact: {} bytes={} sha256={}",
        artifact.display(),
        bytes.len(),
        sha256_hex(bytes)
    );
}

fn main() -> Result<()> {
    let args = Args::parse();
    let _guard = da_cli::logging::init_pipeline_logging("native_route_recovery");

    let root = std::fs::canonicalize(&args.root)
        .with_context(|| format!("resolve root {}", args.root.display()))?;
    let ledger = load_from_root(&root).map_err(|error| anyhow::anyhow!("{error}"))?;
    let bytes = ledger
        .artifact_json_bytes()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    write_artifact(&root, &args.artifact, &bytes)?;
    report_stdout(&ledger, &bytes, &args.artifact);
    Ok(())
}
