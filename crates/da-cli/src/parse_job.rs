//! Opt-in local parse-job lifecycle commands.
//!
//! This surface owns only job admission and durable control-plane operations;
//! the existing `parse-batch` command remains unchanged and unmanaged.

use clap::{Args, Subcommand};
use da_adapters::{SamyamaJobLedger, parse_cache::FileParseArtifactCache};
use da_application::ParseJobRunner;
use da_domain::{
    ContentDigest,
    parse_job::{JobRecord, JobSpec},
};
use da_parse::{
    GrobidHttpBackend, GrobidProfile, ParseBackendRequest, ParseBatch, ParseBatchBudget,
    ParseBatchCancellation, ParseBatchCollector, ParseBatchExecutor, ParseBatchItem,
    ordered_input_fingerprint,
};
use da_ports::JobLedger;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Subcommand)]
pub enum ParseJobCommand {
    /// Submit an immutable manifest as a durable queued job.
    Submit(ParseJobArgs),
    /// Claim/reclaim one queued or expired job for this local owner.
    Run(ParseJobRunArgs),
    /// Read the durable job record and event history.
    Status(ParseJobIdArgs),
    /// Durably request cancellation (works before or during a claim).
    Cancel(ParseJobIdArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ParseJobArgs {
    /// Existing parse-batch manifest. Source paths are read only to form identity.
    #[arg(long)]
    pub manifest: PathBuf,
    /// Operational ledger directory (separate from scientific graph storage).
    #[arg(long, default_value = "data/parse-jobs")]
    pub ledger: PathBuf,
    /// Backend identity (immutable; default matches parse-batch).
    #[arg(long, default_value = "grobid-http")]
    pub backend: String,
    /// Projector identity (immutable).
    #[arg(long, default_value = "native-v1")]
    pub projector: String,
    /// Frozen GROBID profile identity.
    #[arg(long, default_value = "rich")]
    pub profile: String,
    /// Tuning values are operational and excluded from identity.
    #[arg(long = "tuning", value_parser = parse_pair)]
    pub tuning: Vec<(String, String)>,
}

#[derive(Debug, Clone, Args)]
pub struct ParseJobIdArgs {
    #[arg(long)]
    pub job_id: String,
    #[arg(long, default_value = "data/parse-jobs")]
    pub ledger: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct ParseJobRunArgs {
    #[command(flatten)]
    pub id: ParseJobIdArgs,
    /// Opaque owner label. Defaults to the current process identity.
    #[arg(long)]
    pub owner: Option<String>,
}

fn parse_pair(value: &str) -> Result<(String, String), String> {
    let (key, value) = value.split_once('=').ok_or("expected KEY=VALUE")?;
    if key.is_empty() || value.is_empty() {
        return Err("tuning key and value must be non-empty".into());
    }
    Ok((key.to_string(), value.to_string()))
}

fn open(path: &PathBuf) -> Result<Arc<SamyamaJobLedger>, String> {
    SamyamaJobLedger::open(path)
        .map(Arc::new)
        .map_err(|error| error.to_string())
}

fn spec_from_manifest(args: &ParseJobArgs) -> Result<JobSpec, String> {
    let entries = crate::parse_batch::load_manifest(&args.manifest).map_err(|e| e.to_string())?;
    let mut content = Vec::with_capacity(entries.len());
    let mut locations = Vec::with_capacity(entries.len());
    for entry in entries {
        let bytes = std::fs::read(&entry.pdf_path)
            .map_err(|error| format!("source read for {} failed: {error}", entry.paper_id))?;
        content.push(format!(
            "{}|{}:{}",
            entry.paper_id,
            entry.endpoint.as_str(),
            ContentDigest::of_bytes(&bytes).as_str()
        ));
        // Locations are diagnostic hints only and never enter identity.
        locations.push(entry.pdf_path);
    }
    let mut spec = JobSpec {
        key: ContentDigest::of_text("pending"),
        content,
        backend: args.backend.clone(),
        projector: args.projector.clone(),
        profile: args.profile.clone(),
        tuning: args.tuning.clone(),
        locations,
    };
    spec.key = spec.identity_key().map_err(|e| e.to_string())?;
    Ok(spec)
}

pub async fn run(command: ParseJobCommand) -> Result<(), String> {
    match command {
        ParseJobCommand::Submit(args) => {
            let spec = spec_from_manifest(&args)?;
            let key = spec.key.as_str().to_string();
            let record = open(&args.ledger)?
                .submit(spec)
                .await
                .map_err(|e| e.to_string())?;
            print_record("submitted", &record);
            println!("parse-job: key={key}");
        }
        ParseJobCommand::Status(args) => {
            let ledger = open(&args.ledger)?;
            let record = ledger.get(&args.job_id).await.map_err(|e| e.to_string())?;
            print_record("status", &record);
            for event in ledger
                .events(&args.job_id)
                .await
                .map_err(|e| e.to_string())?
            {
                println!(
                    "  event revision={} status={:?} name={}",
                    event.revision, event.status, event.event
                );
            }
        }
        ParseJobCommand::Cancel(args) => {
            let record = open(&args.ledger)?
                .cancel(&args.job_id)
                .await
                .map_err(|e| e.to_string())?;
            print_record("cancel-requested", &record);
        }
        ParseJobCommand::Run(args) => {
            run_managed(args).await?;
        }
    }
    Ok(())
}

async fn run_managed(args: ParseJobRunArgs) -> Result<(), String> {
    let owner = args
        .owner
        .unwrap_or_else(|| format!("pid-{}", std::process::id()));
    let ledger = open(&args.id.ledger)?;
    let spec = ledger
        .spec(&args.id.job_id)
        .await
        .map_err(|error| error.to_string())?;
    let record = ledger
        .get(&args.id.job_id)
        .await
        .map_err(|error| error.to_string())?;
    let requests = requests_from_spec(&spec)?;
    let items: Vec<ParseBatchItem> = requests
        .iter()
        .enumerate()
        .map(|(ordinal, request)| ParseBatchItem::new(ordinal as u32, request.clone()))
        .collect();
    let fingerprint = ordered_input_fingerprint(&items);
    let cursor = da_parse::BatchCursor::at(fingerprint, record.cursor as u32);
    let config = crate::parse_config::resolve(
        &crate::parse_config::ParseRuntimeOverrides::default(),
        &|key| std::env::var(key).ok(),
    )
    .map_err(|error| error.to_string())?;
    let profile = match spec.profile.as_str() {
        "rich" => GrobidProfile::rich(),
        "structural" => GrobidProfile::structural(),
        other => return Err(format!("frozen parse profile is unsupported: {other}")),
    };
    let backend = GrobidHttpBackend::new(Some(config.grobid_url()))
        .with_profile(profile)
        .with_timeout(Duration::from_secs(config.timeout_secs()));
    let batch = ParseBatch::admit(
        config.cache_mode(),
        config.batch_limits(),
        requests,
        Some(cursor),
    )
    .map_err(|error| error.to_string())?;
    let cache = FileParseArtifactCache::new(config.cache_root());
    let executor = ParseBatchExecutor::new(backend, da_parse::TwoStageParseCache::new(cache))
        .with_publication_window(config.publication_window())
        .with_shared_budget(Arc::new(ParseBatchBudget::new(
            config.publication_window(),
            config.execution_capacity(),
        )));
    let sink = Arc::new(ParseBatchCollector::new());
    let cancellation = ParseBatchCancellation::new();
    let runner = ParseJobRunner::new(ledger.clone());
    let run = runner.run(
        &args.id.job_id,
        &owner,
        &executor,
        batch,
        sink,
        cancellation.clone(),
    );
    tokio::pin!(run);
    let report = tokio::select! {
        result = &mut run => result.map_err(|error| error.to_string())?,
        signal = tokio::signal::ctrl_c() => {
            signal.map_err(|error| format!("SIGINT handler failed: {error}"))?;
            runner.cancel(&args.id.job_id, &cancellation).await.map_err(|error| error.to_string())?;
            return Err("parse job cancelled by SIGINT".into());
        }
    };
    print_record("completed", &report.record);
    println!(
        "parse-job: cursor={} result_digest={}",
        report.summary.cursor().next_ordinal(),
        report
            .record
            .result_digest
            .as_ref()
            .map_or("-", ContentDigest::as_str)
    );
    Ok(())
}

fn requests_from_spec(spec: &JobSpec) -> Result<Vec<ParseBackendRequest>, String> {
    if spec.content.len() != spec.locations.len() {
        return Err("frozen job spec content and locations have different lengths".into());
    }
    let profile = match spec.profile.as_str() {
        "rich" => GrobidProfile::rich(),
        "structural" => GrobidProfile::structural(),
        other => return Err(format!("frozen parse profile is unsupported: {other}")),
    };
    spec.locations
        .iter()
        .zip(&spec.content)
        .map(|(location, content)| {
            let (paper_and_endpoint, expected_digest) = content
                .split_once(':')
                .ok_or_else(|| "frozen job content has invalid identity framing".to_string())?;
            let (paper_id, endpoint) = paper_and_endpoint
                .split_once('|')
                .ok_or_else(|| "frozen job content has no endpoint identity".to_string())?;
            let bytes = std::fs::read(location)
                .map_err(|error| format!("frozen source read failed: {error}"))?;
            let actual = ContentDigest::of_bytes(&bytes);
            if actual.as_str() != expected_digest {
                return Err(format!("frozen source digest mismatch for {paper_id}"));
            }
            Ok(match endpoint {
                "fulltext" => ParseBackendRequest::from_legacy_profile(&profile, paper_id, bytes),
                "header" => {
                    ParseBackendRequest::header_from_legacy_profile(&profile, paper_id, bytes)
                }
                "references" => {
                    ParseBackendRequest::references_from_legacy_profile(&profile, paper_id, bytes)
                }
                other => return Err(format!("frozen endpoint is unsupported: {other}")),
            })
        })
        .collect()
}

fn print_record(action: &str, record: &JobRecord) {
    let attempt = record.attempt.as_ref();
    println!(
        "parse-job: action={action} job_id={} status={:?} revision={} epoch={} cursor={} result_digest={} last_error={}",
        record.job_id,
        record.status,
        record.revision,
        attempt.map_or(0, |value| value.epoch),
        record.cursor,
        record
            .result_digest
            .as_ref()
            .map_or("-", ContentDigest::as_str),
        record.last_error.as_deref().unwrap_or("-")
    );
}

#[cfg(test)]
mod tests {
    use super::{parse_pair, requests_from_spec};
    use da_domain::{ContentDigest, parse_job::JobSpec};

    #[test]
    fn tuning_parser_rejects_unstructured_values() {
        assert_eq!(
            parse_pair("workers=2").unwrap(),
            ("workers".into(), "2".into())
        );
        assert!(parse_pair("workers").is_err());
        assert!(parse_pair("=2").is_err());
    }

    #[test]
    fn frozen_spec_restores_endpoint_and_checks_source_digest() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("paper.pdf");
        let bytes = b"fixture-pdf";
        std::fs::write(&path, bytes).unwrap();
        let digest = ContentDigest::of_bytes(bytes);
        let spec = JobSpec {
            key: ContentDigest::of_text("ignored"),
            content: vec![format!("paper|header:{}", digest.as_str())],
            backend: "grobid-http".into(),
            projector: "native-v1".into(),
            profile: "structural".into(),
            tuning: vec![],
            locations: vec![path.to_string_lossy().into_owned()],
        };
        let requests = requests_from_spec(&spec).unwrap();
        assert_eq!(requests[0].paper_id(), "paper");
        assert_eq!(requests[0].endpoint(), da_parse::ParseEndpoint::Header);
    }
}
