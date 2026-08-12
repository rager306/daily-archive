//! Profile real cached article artifacts without claiming unobserved stages.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_adapters::{
    context_conservation_policy::load_context_conservation_policy,
    pipeline_ledger::load_pipeline_stage_ledger,
};
use da_application::context_conservation::{observe_extracted_entities, observe_parsed_article};
use da_domain::{
    assurance::{ContentDigest, MissingnessReason},
    context_conservation::StageContextSnapshot,
};
use da_ports::{extractor::ExtractedEntity, parser::ParsedArticle};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Parser)]
struct Args {
    #[arg(long, env = "DA_PIPELINE_CACHE_ROOT")]
    cache_root: PathBuf,
    #[arg(long, default_value_t = 30)]
    max_articles: usize,
    #[arg(long, default_value = "data/ontology/pipeline_stage_contracts.yaml")]
    ledger: PathBuf,
    #[arg(long, default_value = "data/ontology/context_conservation_policy.yaml")]
    policy: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/article_context/revision_graph_2506.07853.report.json"
    )]
    exact_report: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/runtime/context_conservation_m387.json"
    )]
    report: PathBuf,
}
#[derive(Serialize)]
struct StageRow {
    stage_id: String,
    articles_observed: u64,
    no_claim: Option<MissingnessReason>,
    aggregate: Option<StageContextSnapshot>,
}
#[derive(Serialize)]
struct Profile {
    schema_version: String,
    ledger_sha256: ContentDigest,
    policy_id: String,
    policy_sha256: ContentDigest,
    cache_root: String,
    requested_articles: usize,
    profiled_articles: usize,
    exact_revision_report_sha256: Option<ContentDigest>,
    exact_revision_report_import_eligible: Option<bool>,
    stages: Vec<StageRow>,
    diagnostic_only: bool,
    independent_admission_verdict: Option<String>,
    import_eligible: bool,
    report_sha256: String,
}
fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("context_conservation_profile");
    let a = Args::parse();
    let lb = std::fs::read(&a.ledger)?;
    let ledger = load_pipeline_stage_ledger(&a.ledger).map_err(|e| anyhow!(e))?;
    let pb = std::fs::read(&a.policy)?;
    let policy = load_context_conservation_policy(&a.policy).map_err(|e| anyhow!(e))?;
    let mut parses = BTreeMap::new();
    collect_payloads(&a.cache_root, "parse", &mut parses)?;
    let ids = parses
        .keys()
        .take(a.max_articles)
        .cloned()
        .collect::<Vec<_>>();
    let noise = policy.navigation_noise_titles.clone();
    let mut extracts = BTreeMap::new();
    collect_payloads(&a.cache_root, "extract", &mut extracts)?;
    let mut parse_agg = None;
    let mut extract_agg = None;
    let mut extract_observed = 0;
    for id in &ids {
        let parsed: ParsedArticle = serde_json::from_slice(&std::fs::read(&parses[id])?)?;
        let p = observe_parsed_article(&parsed, std::fs::metadata(&parses[id])?.len(), &noise)
            .map_err(|e| anyhow!(e))?;
        merge(&mut parse_agg, p).map_err(|error| anyhow!(error))?;
        if let Some(path) = extracts.get(id) {
            let entities: Vec<ExtractedEntity> = serde_json::from_slice(&std::fs::read(path)?)?;
            let x = observe_extracted_entities(&parsed, &entities).map_err(|e| anyhow!(e))?;
            merge(&mut extract_agg, x).map_err(|error| anyhow!(error))?;
            extract_observed += 1;
        }
    }
    let exact = std::fs::read(&a.exact_report).ok();
    let exact_import = exact
        .as_ref()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(b).ok())
        .and_then(|v| v.get("import_eligible").and_then(|x| x.as_bool()));
    let mut stages = Vec::new();
    for s in &ledger.stages {
        let (n, agg) = match s.id.as_str() {
            "parse" => (ids.len() as u64, parse_agg.clone()),
            "extraction_compute" => (extract_observed, extract_agg.clone()),
            _ => (0, None),
        };
        stages.push(StageRow {
            stage_id: s.id.clone(),
            articles_observed: n,
            no_claim: if agg.is_none() {
                Some(MissingnessReason::NotMaterialized)
            } else {
                None
            },
            aggregate: agg,
        });
    }
    let mut r = Profile {
        schema_version: "context-conservation-profile.v1".into(),
        ledger_sha256: ContentDigest::of_bytes(&lb),
        policy_id: policy.policy_id,
        policy_sha256: ContentDigest::of_bytes(&pb),
        cache_root: a.cache_root.display().to_string(),
        requested_articles: a.max_articles,
        profiled_articles: ids.len(),
        exact_revision_report_sha256: exact.as_ref().map(|b| ContentDigest::of_bytes(b)),
        exact_revision_report_import_eligible: exact_import,
        stages,
        diagnostic_only: true,
        independent_admission_verdict: None,
        import_eligible: false,
        report_sha256: String::new(),
    };
    r.report_sha256 = digest_report(&r)?;
    if let Some(parent) = a.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&a.report, serde_json::to_vec_pretty(&r)?)?;
    tracing::info!(
        event = "context_conservation_profile_complete",
        profiled_articles = ids.len(),
        extraction_articles = extract_observed,
        stage_rows = r.stages.len(),
        report = %a.report.display(),
        import_eligible = false,
        "Context conservation diagnostic profile completed"
    );
    println!(
        "context-profile articles={} extract={} stages={} diagnostic_only=true D127=false report={}",
        ids.len(),
        extract_observed,
        r.stages.len(),
        a.report.display()
    );
    Ok(())
}
fn collect_payloads(root: &Path, stage: &str, out: &mut BTreeMap<String, PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let p = entry?.path();
        if p.is_dir() {
            if p.file_name().and_then(|x| x.to_str()) == Some(stage) {
                if let Some(id) = p
                    .parent()
                    .and_then(|x| x.file_name())
                    .and_then(|x| x.to_str())
                {
                    find_payload(&p, id, out)?;
                }
            } else {
                collect_payloads(&p, stage, out)?;
            }
        }
    }
    Ok(())
}
fn find_payload(dir: &Path, id: &str, out: &mut BTreeMap<String, PathBuf>) -> Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            let q = p.join("payload.bin");
            if q.is_file() {
                out.entry(id.into()).or_insert(q);
            }
        }
    }
    Ok(())
}
fn merge(slot: &mut Option<StageContextSnapshot>, x: StageContextSnapshot) -> Result<(), String> {
    if let Some(a) = slot {
        a.source_bytes += x.source_bytes;
        a.text_chars += x.text_chars;
        a.sections += x.sections;
        a.citations += x.citations;
        a.figures += x.figures;
        a.tables += x.tables;
        a.grounded_spans += x.grounded_spans;
        a.grounded_section_vids += x.grounded_section_vids;
        a.entities += x.entities;
        a.evidence_items += x.evidence_items;
        a.duplicate_section_titles += x.duplicate_section_titles;
        a.navigation_noise_sections += x.navigation_noise_sections;
        for (k, v) in x.missingness {
            a.missingness.entry(k).or_insert(v);
        }
        a.artifact_sha256 =
            StageContextSnapshot::digest_json(&(a.text_chars, a.sections, a.entities))?;
    } else {
        *slot = Some(x);
    }
    Ok(())
}
fn digest_report(r: &Profile) -> Result<String> {
    let mut v = serde_json::to_value(r)?;
    v["report_sha256"] = serde_json::Value::String(String::new());
    Ok(ContentDigest::of_bytes(&serde_json::to_vec(&v)?)
        .as_str()
        .into())
}
