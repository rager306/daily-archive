//! Offline, checksum-bound acceptance evidence for the staged scholarly parse plane.
//! Never fetches, never calls Java/HTTP, never writes graph state.
//! Local TeiLifter + ConservationEngine measure no_silent_loss on preserved TEI.
use anyhow::{Context, Result, bail};
use clap::Parser;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const FALLBACK: &str = "java-grobid-http@0.9.0-crf";
const PDF_IDS: [&str; 3] = ["hub-2507.19457", "rlm-2512.24601", "reca-2605.26525v1"];
const RICH_FP: &str = "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula";
const PROXY_TEI: [(&str, &str, &str); 3] = [
    (
        "hub-2507.19457",
        "crates/da-parse/tests/fixtures/preserved/hub_2507.19457.tei.xml",
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
    ),
    (
        "rlm-2512.24601",
        "crates/da-parse/tests/fixtures/preserved/rlm_2512.24601.tei.xml",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
    ),
    (
        "reca-2605.26525v1",
        "crates/da-parse/tests/fixtures/preserved/reca_2605.26525v1.tei.xml",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
    ),
];

#[derive(Parser, Debug)]
#[command(
    name = "acceptance_evidence",
    about = "Emit redacted offline parse acceptance evidence"
)]
struct Args {
    #[arg(long, default_value = "data/parse/acceptance_manifest.yaml")]
    manifest: PathBuf,
    #[arg(long, default_value = "artifacts/parse_compat/baseline.json")]
    baseline: PathBuf,
    #[arg(long, default_value = "artifacts/parse_compat/layout_bakeoff.json")]
    layout_report: PathBuf,
    #[arg(
        long,
        default_value = "crates/da-parse/tests/fixtures/layout_oracle.yaml"
    )]
    layout_oracle: PathBuf,
    #[arg(
        long,
        default_value = "artifacts/parse_compat/acceptance_evidence.json"
    )]
    report: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: String,
    policy_locks: Locks,
    comparison_rules: Rules,
    classes: BTreeMap<String, Class>,
}
#[derive(Debug, Deserialize)]
struct Locks {
    import_eligible: bool,
    auto_promote: String,
    production_route: String,
    fallback_target: String,
}
#[derive(Debug, Deserialize)]
struct Rules {
    allowed_classes: Vec<String>,
    allowed_row_states: Vec<String>,
    allowed_comparison_kinds: Vec<String>,
    missing_behavior: String,
    same_n: String,
    catalogued_bytes_are_executed: bool,
    omnidocbench_is_tei_oracle: bool,
}
#[derive(Debug, Deserialize)]
struct Class {
    comparison_kind: String,
    rows: Vec<Row>,
    local_proxy: Proxy,
}
#[derive(Debug, Deserialize)]
struct Proxy {
    same_n: bool,
    #[serde(default)]
    page_denominator: Option<u64>,
    #[serde(default)]
    document_denominator: Option<u64>,
    #[serde(default)]
    oracle_ref: Option<String>,
    rows: Vec<ProxyRow>,
}
#[derive(Debug, Deserialize)]
struct Row {
    id: String,
    state: String,
    identity: String,
    dataset_id: String,
    source_url: String,
    expected_n: u64,
    #[serde(default)]
    gold_format: Option<String>,
    #[serde(default)]
    filter: Option<String>,
}
#[derive(Debug, Deserialize)]
struct ProxyRow {
    id: String,
    state: String,
    #[serde(default)]
    identity_ref: Option<String>,
    #[serde(default)]
    pdf_path: Option<String>,
    #[serde(default)]
    pdf_sha256: Option<String>,
    #[serde(default)]
    tei_ref: Option<String>,
    #[serde(default)]
    tei_sha256: Option<String>,
    #[serde(default)]
    page_count: Option<u64>,
}

fn measure_proxy_conservation(repo: &Path) -> Result<(bool, bool, Vec<Value>)> {
    use da_parse::{ConservationEngine, TeiEnvelope, TeiIdentity, TeiLifter};
    let engine = ConservationEngine::new();
    let lifter = TeiLifter::new();
    let mut docs = Vec::new();
    let mut all_conserved = true;
    let mut any_loss = false;
    for (id, rel, expected_sha) in PROXY_TEI {
        check_hash(repo, rel, expected_sha, "checksum_drift")?;
        let path = resolve(repo, rel)?;
        let bytes = fs::read(&path)?;
        let envelope = TeiEnvelope::new(bytes, TeiIdentity::new("grobid-http", RICH_FP));
        let ir = lifter.lift(&envelope, id, (*expected_sha).to_string());
        let delta = engine.compare(&envelope, &ir);
        let conserved = delta.no_silent_loss();
        all_conserved &= conserved;
        any_loss |= delta.loss_visible();
        docs.push(json!({
            "id": id,
            "verdict": delta.verdict.as_str(),
            "no_silent_loss": conserved,
            "delta_digest": delta.delta_digest,
        }));
    }
    Ok((all_conserved, any_loss, docs))
}

fn digest(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).with_context(|| format!("cannot read bound bytes: {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn resolve(root: &Path, raw: &str) -> Result<PathBuf> {
    let p = Path::new(raw);
    if p.is_absolute() || raw.contains('\0') {
        bail!("path_escape_absolute")
    }
    let joined = root.join(p);
    let canonical = joined
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("missing_bound_bytes"))?;
    let root_c = root.canonicalize()?;
    if !canonical.starts_with(&root_c) {
        bail!("path_escape_absolute")
    }
    Ok(canonical)
}
fn fail(code: &str, detail: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{code}: {detail}")
}
fn check_hash(root: &Path, raw: &str, expected: &str, code: &str) -> Result<()> {
    let p = resolve(root, raw)
        .map_err(|e| fail(e.to_string().split(':').next().unwrap_or("path_error"), raw))?;
    let actual = digest(&p).map_err(|_| fail("missing_bound_bytes", raw))?;
    if actual != expected {
        bail!("{code}: {}", raw);
    }
    Ok(())
}
fn json_file(root: &Path, path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(resolve(
        root,
        path.to_str().unwrap(),
    )?)?)?)
}
fn yaml_file<T: for<'a> Deserialize<'a>>(root: &Path, path: &Path) -> Result<T> {
    Ok(serde_yaml::from_slice(&fs::read(resolve(
        root,
        path.to_str().unwrap(),
    )?)?)?)
}
fn tei_path_from_ref(root: &Path, reference: &str) -> Result<String> {
    let (manifest_path, fragment) = reference
        .split_once('#')
        .ok_or_else(|| fail("malformed_reference", reference))?;
    let cases: Value = yaml_file(root, &PathBuf::from(manifest_path))?;
    let id = fragment.strip_prefix("cases.").unwrap_or(fragment);
    cases
        .get("cases")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("id").and_then(Value::as_str) == Some(id))
        })
        .and_then(|row| row.get("fixture"))
        .and_then(|fixture| fixture.get("path"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| fail("missing_bound_bytes", reference))
}

fn main() -> Result<()> {
    let _log_guard = da_cli::logging::init_pipeline_logging("acceptance_evidence");
    let args = Args::parse();
    let repo = root();
    let manifest: Manifest =
        yaml_file(&repo, &args.manifest).map_err(|e| fail("malformed_manifest", e))?;
    if manifest.schema_version != "parse-acceptance.v1" {
        bail!("malformed_manifest: schema_version");
    }
    let expected_classes = ["layout_omnidocbench", "publisher_acceptance"];
    if manifest
        .classes
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>()
        != expected_classes
    {
        bail!("malformed_taxonomy: classes");
    }
    if manifest.policy_locks.import_eligible
        || manifest.policy_locks.auto_promote != "forbidden"
        || manifest.policy_locks.production_route != "forbidden"
        || manifest.policy_locks.fallback_target != FALLBACK
    {
        bail!("lock_erosion: policy_locks");
    }
    if manifest.comparison_rules.missing_behavior != "fail_visible"
        || manifest.comparison_rules.same_n != "required"
        || manifest.comparison_rules.catalogued_bytes_are_executed
        || manifest.comparison_rules.omnidocbench_is_tei_oracle
        || manifest.comparison_rules.allowed_classes
            != ["publisher_acceptance", "layout_omnidocbench"]
        || manifest.comparison_rules.allowed_row_states != ["bound", "catalogued", "bytes_absent"]
        || manifest.comparison_rules.allowed_comparison_kinds != ["tei_family", "layout_markdown"]
    {
        bail!("malformed_taxonomy: comparison_rules");
    }
    let baseline: Value =
        json_file(&repo, &args.baseline).map_err(|e| fail("malformed_receipt", e))?;
    let layout: Value =
        json_file(&repo, &args.layout_report).map_err(|e| fail("malformed_receipt", e))?;
    let oracle: Value =
        yaml_file(&repo, &args.layout_oracle).map_err(|e| fail("malformed_receipt", e))?;
    let mut classes = BTreeMap::new();
    for class_name in expected_classes {
        let class = manifest.classes.get(class_name).unwrap();
        let expected_kind = if class_name == "publisher_acceptance" {
            "tei_family"
        } else {
            "layout_markdown"
        };
        if class.comparison_kind != expected_kind {
            bail!("role_mismatch: {class_name}");
        }
        let mut counts = BTreeMap::<String, u64>::new();
        for state in ["bound", "catalogued", "bytes_absent"] {
            counts.insert(
                state.to_string(),
                class.rows.iter().filter(|r| r.state == state).count() as u64,
            );
        }
        if class
            .rows
            .iter()
            .any(|r| !["bound", "catalogued", "bytes_absent"].contains(&r.state.as_str()))
        {
            bail!("malformed_taxonomy: row_state");
        }
        if class
            .rows
            .iter()
            .any(|r| r.state == "bound" && r.source_url.is_empty())
        {
            bail!("malformed_manifest: source_url");
        }
        let proxy_bound = class
            .local_proxy
            .rows
            .iter()
            .filter(|r| r.state == "bound")
            .count() as u64;
        if !class.local_proxy.same_n || proxy_bound == 0 {
            bail!("same_n_mismatch: {class_name}");
        }
        if class_name == "publisher_acceptance" {
            for p in &class.local_proxy.rows {
                if p.state != "bound" {
                    bail!("silent_skip: {class_name}");
                }
                let pdf = p
                    .pdf_path
                    .as_ref()
                    .ok_or_else(|| fail("missing_bound_bytes", &p.id))?;
                let sha = p
                    .pdf_sha256
                    .as_ref()
                    .ok_or_else(|| fail("missing_checksum", &p.id))?;
                check_hash(&repo, pdf, sha, "checksum_drift")?;
                let tei = p
                    .tei_ref
                    .as_ref()
                    .ok_or_else(|| fail("missing_bound_bytes", &p.id))?;
                let tsha = p
                    .tei_sha256
                    .as_ref()
                    .ok_or_else(|| fail("missing_checksum", &p.id))?;
                let tei_path =
                    tei_path_from_ref(&repo, tei).map_err(|_| fail("missing_bound_bytes", tei))?;
                let c = resolve(&repo, &tei_path).map_err(|_| fail("missing_bound_bytes", tei))?;
                if digest(&c).map_err(|_| fail("missing_bound_bytes", tei))? != *tsha {
                    bail!("checksum_drift: {tei}");
                }
            }
        }
        classes.insert(class_name.to_string(), json!({"comparison_kind": class.comparison_kind, "corpus_n": class.rows.iter().map(|r| r.expected_n).sum::<u64>(), "bound_n": counts["bound"], "catalogued_n": counts["catalogued"], "bytes_absent_n": counts["bytes_absent"], "executed_n": proxy_bound, "same_n": class.local_proxy.same_n, "missing_behavior":"fail_visible"}));
    }
    let layout_proxy = &manifest.classes["layout_omnidocbench"].local_proxy;
    let pages: u64 = layout_proxy
        .rows
        .iter()
        .map(|r| r.page_count.unwrap_or(0))
        .sum();
    if layout_proxy.document_denominator != Some(3)
        || layout_proxy.page_denominator != Some(202)
        || pages != 202
    {
        bail!("denominator_mismatch: layout_topology");
    }
    if layout
        .get("gate")
        .and_then(|x| x.get("exact_same_n"))
        .and_then(Value::as_bool)
        != Some(true)
        || layout
            .get("gate")
            .and_then(|x| x.get("oracle_complete"))
            .and_then(Value::as_bool)
            != Some(true)
    {
        bail!("same_n_mismatch: layout_receipt");
    }
    if oracle.get("schema_version").and_then(Value::as_str) != Some("layout-oracle.v1") {
        bail!("role_mismatch: layout_oracle");
    }
    if baseline.get("conservation").is_none() {
        bail!("missing_receipt: conservation");
    }
    if baseline
        .get("corpus_classes")
        .and_then(|x| x.get("used"))
        .and_then(Value::as_str)
        != Some("fast_local_fixtures")
    {
        bail!("role_mismatch: baseline");
    }
    let (no_silent_loss, loss_visible, conservation_docs) = measure_proxy_conservation(&repo)?;
    let verdict = if no_silent_loss { "PASS" } else { "FAIL" };
    let report = json!({"schema_version":"parse-acceptance-evidence.v1","manifest_id":"scholarly-parse-acceptance-s20","verdict":verdict,"classes":classes,"same_n":{"publisher_acceptance":true,"layout_omnidocbench":true},"conservation":{"source":"preserved_tei_lift","no_silent_loss":no_silent_loss,"loss_visible":loss_visible,"tei_oracle":"java_grobid_compatibility","documents":conservation_docs},"layout":{"documents":3,"pages":202,"topology":"independent_layout_oracle","same_n":true},"token_canary":{"status":"diagnostic_only","executed":false},"missing_failures":["remote_bytes_absent"],"locks":{"import_eligible":false,"auto_promote":"forbidden","production_route":"forbidden","fallback_target":FALLBACK,"promotion_state":"candidate"},"redaction":{"raw_pdf":false,"raw_tei":false,"raw_text":false,"hostnames":false,"absolute_paths":false,"secrets":false}});
    if let Some(parent) = args.report.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(&args.report, &bytes)?;
    println!(
        "acceptance_evidence verdict={verdict} no_silent_loss={no_silent_loss} report={}",
        args.report.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn constants_are_closed() {
        assert_eq!(super::FALLBACK, "java-grobid-http@0.9.0-crf");
        assert_eq!(super::PDF_IDS.len(), 3);
    }
}
