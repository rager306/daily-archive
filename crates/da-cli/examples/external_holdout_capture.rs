//! Frozen Java reference capture for the S08 external holdout probe
//! (M511-j7wz2f/S08/T02).
//!
//! The S08 probe must compare a *frozen* Java reference against a frozen
//! native emission, so the reference has to be bound to exact inputs, exact
//! bytes and an exact release before any measurement happens. This example is
//! the only writer of that reference:
//!
//! - **`--preflight`** (default) — read-only. It resolves every declared input
//!   (the five sha256-pinned holdout PDFs, the Java endpoint, the declared Java
//!   release, the external `bytes_absent` band, the production-materialization
//!   census) and prints one `PREFLIGHT <req> -> ok|blocked:<label>` line per
//!   requirement. Any blocker exits non-zero and writes nothing.
//! - **`--capture --out <dir>`** — after a green preflight, runs each PDF
//!   through the existing typed REST transport (one
//!   `POST /api/processFulltextDocument` per document, `structural` profile per
//!   D204) and freezes the raw TEI plus a `provenance.yaml` carrying the
//!   endpoint, the probed live release, the profile and consolidation flags,
//!   the capture revision and per-document latency.
//! - **`--verify --in <dir>`** — re-hashes the frozen bytes and compares them
//!   with both the provenance and the DATA pins; `--offline` skips the live
//!   release re-probe so the check stays hermetic.
//! - **`--self-check`** — runs the pure preflight predicates against synthetic
//!   inputs so the "inputs unavailable" branch is *tested* by name rather than
//!   merely asserted in prose.
//!
//! Scope discipline: this is offline evidence, never a production route. No
//! graph write, no import, no promotion, and no external dataset bytes are
//! ever loaded or materialized. The external `bytes_absent` band is read, never
//! lifted.
//!
//! ## Which release is "the" Java reference?
//!
//! `data/parse/acceptance_manifest.yaml#policy_locks.fallback_target` names
//! `java-grobid-http@0.9.0-crf`, but that key is the **rollback** target:
//! `.docker/grobid-release.override.yml` documents itself as the explicit
//! production release pin, M505 provenance records
//! `state=production_switched_and_verified` with
//! `production_live_version_probe=0.9.1`, and M506 `baseline-identity.json`
//! records `production_tag=grobid/grobid:0.9.1-crf`. Comparing the live service
//! against the rollback label would block a correct environment, so the
//! expected release is declared in DATA
//! (`...release_probe.declared_release`) and its image digest is cross-checked
//! against the repo's real release pin — a drift between the declaration and the
//! pin blocks as `java_release_pin_drift`. The DATA-plane authority-label
//! divergence is itself a named open item (H21, deferred to S09) and is recorded
//! machine-visibly in the same DATA block instead of being papered over.

use anyhow::{Context, Result};
use clap::Parser;
use da_parse::{GrobidHttpBackend, GrobidProfile, ParseBackend};
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Panic/log prefix, so a failure names the tool that produced it.
const PREFIX: &str = "[external-holdout-capture]";

/// Declared holdout identity list (five sha256-pinned non-gold arXiv PDFs).
const HOLDOUT_MANIFEST: &str = "data/parse/holdout_manifests.yaml";
/// Sanctioned DATA carrier holding the frozen release-probe lane (D377).
const ACCEPTANCE_MANIFEST: &str = "data/parse/acceptance_manifest.yaml";
/// M505 production release pin (digest, not tag — see MEM1306).
const RELEASE_PIN: &str = ".docker/grobid-release.override.yml";
/// Production materialization coverage root used for the overlap census.
const PRODUCTION_COVERAGE_ROOT: &str = "data/processed";
/// Committed fixture directory for the frozen reference.
const FIXTURE_DIR: &str = "crates/da-parse/tests/fixtures/holdout";
/// Provenance file name inside the frozen directory.
const PROVENANCE_FILE: &str = "provenance.yaml";
/// Endpoint default, identical to `da_cli::parse_config::DEFAULT_GROBID_URL`.
const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8070";
/// Canonical GROBID endpoint env key (owned by `da_cli::parse_config`).
const DA_GROBID_URL_KEY: &str = "DA_GROBID_URL";
/// Historical alias env key; when both are set they must agree.
const GROBID_URL_ALIAS_KEY: &str = "GROBID_URL";
/// Above this many frozen bytes the provenance records a `size_note`.
const SIZE_NOTE_LIMIT_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Parser, Debug)]
#[command(
    about = "Frozen Java reference capture / preflight for the S08 external holdout probe",
    long_about = None
)]
struct Args {
    /// Preflight only (read-only, the default when no mode is given).
    #[arg(long)]
    preflight: bool,
    /// Capture the frozen reference into `--out`.
    #[arg(long)]
    capture: bool,
    /// Output directory for `--capture`.
    #[arg(long, value_name = "DIR", default_value = FIXTURE_DIR)]
    out: PathBuf,
    /// Verify a frozen reference directory.
    #[arg(long)]
    verify: bool,
    /// Input directory for `--verify` (defaults to the committed fixture dir).
    #[arg(long, value_name = "DIR", default_value = FIXTURE_DIR)]
    r#in: PathBuf,
    /// Skip the live release re-probe during `--verify` (hermetic mode).
    #[arg(long)]
    offline: bool,
    /// Run the pure preflight predicates on synthetic inputs.
    #[arg(long)]
    self_check: bool,
    /// Override the GROBID endpoint (otherwise `DA_GROBID_URL`/`GROBID_URL`).
    #[arg(long, value_name = "URL")]
    endpoint: Option<String>,
    /// Bounded per-document request timeout in seconds.
    #[arg(long, default_value_t = 900)]
    timeout_secs: u64,
}

// ---------------------------------------------------------------------------
// Pure preflight predicates (all of them are exercised by `--self-check`)
// ---------------------------------------------------------------------------

/// Outcome of one preflight requirement: `ok`, or a named blocker label.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    Ok,
    Blocked(String),
}

impl Outcome {
    fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    fn label(&self) -> String {
        match self {
            Self::Ok => "ok".to_string(),
            Self::Blocked(label) => label.clone(),
        }
    }
}

/// One declared holdout document and the bytes it must have.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ArticleIdentity {
    article_id: String,
    pdf_path: String,
    sha256: String,
    size_bytes: u64,
}

/// Declared Java release identity for the frozen reference.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclaredRelease {
    version: String,
    image_repository: String,
    image_digest: String,
    release_pin: String,
    rollback_version: String,
    label_divergence_item: String,
}

/// The reference bytes a single fixture must carry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReferenceDocument {
    article_id: String,
    path: String,
    sha256: String,
    size_bytes: u64,
}

/// Everything `--verify` needs from DATA.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReferenceFreeze {
    declared: DeclaredRelease,
    source_manifest_sha256: String,
    reference_endpoint: String,
    reference_java_version: String,
    reference_profile: String,
    documents: Vec<ReferenceDocument>,
}

/// Observed bytes of one declared article.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedBytes {
    exists: bool,
    sha256: Option<String>,
    size_bytes: Option<u64>,
}

/// A declared external dataset row that must stay `bytes_absent`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalRow {
    id: String,
    state: String,
    declared_local_path: Option<String>,
}

/// Does the declared article's bytes match its pinned identity?
fn admit_article_bytes(article: &ArticleIdentity, observed: &ObservedBytes) -> Outcome {
    if !observed.exists {
        return Outcome::Blocked(format!("bytes_absent:{}", article.article_id));
    }
    if observed.sha256.as_deref() != Some(article.sha256.as_str()) {
        return Outcome::Blocked(format!("bytes_drift:{}", article.article_id));
    }
    if observed.size_bytes != Some(article.size_bytes) {
        return Outcome::Blocked(format!("bytes_size_drift:{}", article.article_id));
    }
    Outcome::Ok
}

/// Is the Java endpoint alive?
fn admit_liveness(alive: bool) -> Outcome {
    if alive {
        Outcome::Ok
    } else {
        Outcome::Blocked("java_endpoint_unavailable".to_string())
    }
}

/// Does the probed live release match the declared release?
fn admit_release(declared: &DeclaredRelease, observed_version: &str) -> Outcome {
    if declared.version.trim().is_empty() {
        return Outcome::Blocked("java_release_undeclared".to_string());
    }
    if declared.version != observed_version {
        return Outcome::Blocked(format!("java_version_mismatch:{observed_version}"));
    }
    Outcome::Ok
}

/// Does the DATA declaration still agree with the repo's real release pin?
fn admit_release_pin(declared: &DeclaredRelease, pin: Option<(&str, &str)>) -> Outcome {
    match pin {
        None => Outcome::Blocked("java_release_pin_missing".to_string()),
        Some((repository, digest))
            if repository == declared.image_repository && digest == declared.image_digest =>
        {
            Outcome::Ok
        }
        Some((repository, digest)) => {
            Outcome::Blocked(format!("java_release_pin_drift:{repository}:{digest}"))
        }
    }
}

/// Do all declared external dataset rows still declare `bytes_absent` with no
/// materialized local bytes?
fn admit_external_dataset_bytes(
    rows: &[ExternalRow],
    path_exists: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut blockers = Vec::new();
    for row in rows {
        let local_bytes_exist = row.declared_local_path.as_deref().is_some_and(&path_exists);
        if row.state != "bytes_absent" || local_bytes_exist {
            blockers.push(format!("external_dataset_bytes_present:{}", row.id));
        }
    }
    blockers
}

/// Does one frozen fixture still match both the provenance and the DATA pin?
fn admit_reference(
    article_id: &str,
    exists: bool,
    observed_sha256: Option<&str>,
    provenance_sha256: &str,
    data_pin_sha256: &str,
) -> Outcome {
    if !exists {
        return Outcome::Blocked(format!("reference_absent:{article_id}"));
    }
    if observed_sha256 != Some(provenance_sha256) {
        return Outcome::Blocked(format!("reference_drift:{article_id}"));
    }
    if observed_sha256 != Some(data_pin_sha256) {
        return Outcome::Blocked(format!("data_pin_drift:{article_id}"));
    }
    Outcome::Ok
}

/// Does the live release re-probe still match the frozen record?
fn admit_live_release(
    expected_endpoint: &str,
    observed_endpoint: &str,
    expected_version: &str,
    observed_version: &str,
) -> Outcome {
    if expected_endpoint != observed_endpoint {
        return Outcome::Blocked(format!("endpoint_drift:{observed_endpoint}"));
    }
    if expected_version != observed_version {
        return Outcome::Blocked(format!("live_release_drift:{observed_version}"));
    }
    Outcome::Ok
}

// ---------------------------------------------------------------------------
// Small typed YAML helpers
// ---------------------------------------------------------------------------

fn read_yaml(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("{PREFIX} cannot read `{}`", path.display()))?;
    serde_yaml::from_str(&text)
        .with_context(|| format!("{PREFIX} `{}` is not valid YAML", path.display()))
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key)
}

fn field_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    field(value, key).and_then(Value::as_str)
}

fn field_u64(value: &Value, key: &str) -> Option<u64> {
    field(value, key).and_then(Value::as_u64)
}

fn path_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| field_str(value, key))
        .map(str::to_string)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn sha256_file_prefixed(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("{PREFIX} cannot read `{}`", path.display()))?;
    Ok(format!("sha256:{}", sha256_hex(&bytes)))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/da-cli has a workspace root")
        .to_path_buf()
}

/// Resolve the endpoint from the closed env vocabulary, mirroring
/// `da_cli::parse_config`: canonical `DA_GROBID_URL`, alias `GROBID_URL`;
/// both set and differing is a conflict, never a silent pick.
fn resolve_endpoint(override_url: Option<&str>) -> Result<String, String> {
    if let Some(url) = override_url {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err("java_endpoint_blank".to_string());
        }
        return Ok(trimmed.to_string());
    }
    let canonical = std::env::var(DA_GROBID_URL_KEY).ok();
    let alias = std::env::var(GROBID_URL_ALIAS_KEY).ok();
    let canonical = canonical.filter(|v| !v.trim().is_empty());
    let alias = alias.filter(|v| !v.trim().is_empty());
    match (canonical, alias) {
        (Some(c), Some(a)) if c.trim() != a.trim() => {
            Err("java_endpoint_alias_conflict".to_string())
        }
        (Some(c), _) => Ok(c.trim().to_string()),
        (None, Some(a)) => Ok(a.trim().to_string()),
        (None, None) => Ok(DEFAULT_ENDPOINT.to_string()),
    }
}

// ---------------------------------------------------------------------------
// DATA projections
// ---------------------------------------------------------------------------

fn declared_articles(root: &Path) -> Result<Vec<ArticleIdentity>> {
    let manifest = read_yaml(&root.join(HOLDOUT_MANIFEST))?;
    let articles = field(&manifest, "articles")
        .and_then(Value::as_sequence)
        .context(format!(
            "{PREFIX} `{HOLDOUT_MANIFEST}` has no `articles` list"
        ))?;
    let mut declared = Vec::new();
    for article in articles {
        let article_id = field_str(article, "article_id")
            .context(format!("{PREFIX} holdout article without `article_id`"))?
            .to_string();
        let pdf_path = field_str(article, "pdf_path")
            .context(format!("{PREFIX} holdout `{article_id}` has no `pdf_path`"))?
            .to_string();
        let sha256 = field_str(article, "sha256")
            .context(format!("{PREFIX} holdout `{article_id}` has no `sha256`"))?
            .to_string();
        let size_bytes = field_u64(article, "size_bytes").context(format!(
            "{PREFIX} holdout `{article_id}` has no `size_bytes`"
        ))?;
        declared.push(ArticleIdentity {
            article_id,
            pdf_path,
            sha256,
            size_bytes,
        });
    }
    Ok(declared)
}

/// The T02-declared Java release identity, cross-checked against the release pin.
fn declared_release(manifest: &Value) -> Result<DeclaredRelease> {
    let block = field(manifest, "external_holdout_contract")
        .and_then(|c| field(c, "release_probe"))
        .and_then(|p| field(p, "declared_release"))
        .context(format!(
            "{PREFIX} release_freeze_declared_absent: `{ACCEPTANCE_MANIFEST}` carries no \
             external_holdout_contract.release_probe.declared_release"
        ))?;
    Ok(DeclaredRelease {
        version: field_str(block, "version").unwrap_or_default().to_string(),
        image_repository: field_str(block, "image_repository")
            .unwrap_or_default()
            .to_string(),
        image_digest: field_str(block, "image_digest")
            .unwrap_or_default()
            .to_string(),
        release_pin: field_str(block, "release_pin")
            .unwrap_or_default()
            .to_string(),
        rollback_version: field_str(block, "rollback_version")
            .unwrap_or_default()
            .to_string(),
        label_divergence_item: field_str(block, "label_divergence_item")
            .unwrap_or_default()
            .to_string(),
    })
}

/// The frozen reference record (provenance-equivalent pins) from DATA, read by
/// `--verify` so a fixture can never drift from its declaration silently.
fn reference_freeze(manifest: &Value) -> Result<ReferenceFreeze> {
    let lane = field(manifest, "external_holdout_contract")
        .and_then(|contract| field(contract, "release_probe"))
        .context(format!(
            "{PREFIX} probe_lane_missing: `{ACCEPTANCE_MANIFEST}` carries no \
             external_holdout_contract.release_probe"
        ))?;
    let documents = field(lane, "reference_tei")
        .and_then(Value::as_sequence)
        .context(format!(
            "{PREFIX} reference_freeze_captured_absent: the release_probe lane carries no \
             `reference_tei` list"
        ))?;
    let mut pins = Vec::new();
    for document in documents {
        pins.push(ReferenceDocument {
            article_id: field_str(document, "article_id")
                .unwrap_or_default()
                .to_string(),
            path: field_str(document, "path").unwrap_or_default().to_string(),
            sha256: field_str(document, "sha256")
                .unwrap_or_default()
                .to_string(),
            size_bytes: field_u64(document, "size_bytes").unwrap_or_default(),
        });
    }
    Ok(ReferenceFreeze {
        declared: declared_release(manifest)?,
        source_manifest_sha256: field_str(lane, "source_manifest_sha256")
            .unwrap_or_default()
            .to_string(),
        reference_endpoint: field_str(lane, "reference_endpoint")
            .unwrap_or_default()
            .to_string(),
        reference_java_version: field_str(lane, "reference_java_version")
            .unwrap_or_default()
            .to_string(),
        reference_profile: field_str(lane, "reference_profile")
            .unwrap_or_default()
            .to_string(),
        documents: pins,
    })
}

/// Every declared row that carries an explicit local path key, for the
/// external `bytes_absent` guard. Rows are read, never lifted.
fn external_rows(manifest: &Value) -> Vec<ExternalRow> {
    let mut rows = Vec::new();
    let Some(classes) = field(manifest, "classes").and_then(Value::as_mapping) else {
        return rows;
    };
    for (_, class) in classes {
        collect_external_rows(class, &mut rows);
    }
    rows
}

fn collect_external_rows(value: &Value, rows: &mut Vec<ExternalRow>) {
    // A `local_proxy` sub-block is the local proxy population, not an external
    // dataset: it declares `proxy_of` and its rows are legitimately `bound`.
    let is_local_proxy = field(value, "proxy_of").is_some();
    let external_rows = field(value, "rows")
        .and_then(Value::as_sequence)
        .filter(|_| !is_local_proxy);
    if let Some(list) = external_rows {
        for row in list {
            rows.push(ExternalRow {
                id: field_str(row, "id").unwrap_or_default().to_string(),
                state: field_str(row, "state").unwrap_or_default().to_string(),
                declared_local_path: path_field(
                    row,
                    &["local_path", "bytes_path", "materialized_path", "path"],
                ),
            });
        }
    }
    if let Some(map) = value.as_mapping() {
        for (key, child) in map {
            if key.as_str() == Some("rows") || key.as_str() == Some("local_proxy") {
                continue;
            }
            collect_external_rows(child, rows);
        }
    }
}

/// Parse the `image:` reference out of the compose release-pin file.
fn release_pin_reference(root: &Path, release_pin: &str) -> Option<(String, String)> {
    let text = std::fs::read_to_string(root.join(release_pin)).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(reference) = trimmed.strip_prefix("image:") else {
            continue;
        };
        let reference = reference.trim().trim_matches('"').trim_matches('\'');
        let (repository, digest) = reference.split_once('@')?;
        return Some((repository.to_string(), digest.to_string()));
    }
    None
}

/// Count how many declared holdout ids are also materialized production
/// coverage. Measured, never asserted: the census is printed as `n/5`.
fn production_materialization_overlap(root: &Path, articles: &[ArticleIdentity]) -> usize {
    articles
        .iter()
        .filter(|article| {
            root.join(PRODUCTION_COVERAGE_ROOT)
                .join(&article.article_id)
                .is_dir()
        })
        .count()
}

/// Best-effort capture revision (git HEAD), never invented.
fn capture_revision(root: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| format!("git:{}", String::from_utf8_lossy(&output.stdout).trim()))
        .unwrap_or_else(|| "git:unknown".to_string())
}

fn observed_bytes(root: &Path, article: &ArticleIdentity) -> ObservedBytes {
    let path = root.join(&article.pdf_path);
    let Ok(bytes) = std::fs::read(&path) else {
        return ObservedBytes {
            exists: false,
            sha256: None,
            size_bytes: None,
        };
    };
    ObservedBytes {
        exists: true,
        sha256: Some(format!("sha256:{}", sha256_hex(&bytes))),
        size_bytes: Some(bytes.len() as u64),
    }
}

// ---------------------------------------------------------------------------
// Preflight
// ---------------------------------------------------------------------------

struct Preflight {
    articles: Vec<ArticleIdentity>,
    declared: DeclaredRelease,
    endpoint: String,
    observed_version: String,
    checks: Vec<(String, Outcome)>,
    overlap: usize,
}

async fn run_preflight(root: &Path, args: &Args) -> Result<Preflight> {
    let articles = declared_articles(root)?;
    let manifest = read_yaml(&root.join(ACCEPTANCE_MANIFEST))?;
    let declared = declared_release(&manifest)?;

    let mut checks: Vec<(String, Outcome)> = Vec::new();

    for article in &articles {
        let observed = observed_bytes(root, article);
        checks.push((
            format!("article:{}", article.article_id),
            admit_article_bytes(article, &observed),
        ));
    }

    let endpoint = match resolve_endpoint(args.endpoint.as_deref()) {
        Ok(endpoint) => {
            checks.push(("java_endpoint_env".to_string(), Outcome::Ok));
            endpoint
        }
        Err(label) => {
            checks.push(("java_endpoint_env".to_string(), Outcome::Blocked(label)));
            DEFAULT_ENDPOINT.to_string()
        }
    };

    let backend = GrobidHttpBackend::new(Some(&endpoint));
    let alive = backend.is_alive().await;
    checks.push(("java_endpoint".to_string(), admit_liveness(alive)));

    let (probe_outcome, observed_version) = match backend.probe_version().await {
        Ok(probe) => (admit_release(&declared, &probe.version), probe.version),
        Err(_) => (
            Outcome::Blocked("java_endpoint_unavailable".to_string()),
            String::new(),
        ),
    };
    checks.push(("java_release".to_string(), probe_outcome));
    let pin_path = if declared.release_pin.trim().is_empty() {
        RELEASE_PIN.to_string()
    } else {
        declared.release_pin.clone()
    };
    checks.push((
        "java_release_pin".to_string(),
        admit_release_pin(
            &declared,
            release_pin_reference(root, &pin_path)
                .as_ref()
                .map(|(repository, digest)| (repository.as_str(), digest.as_str())),
        ),
    ));

    let external = external_rows(&manifest);
    let guards = admit_external_dataset_bytes(&external, |path: &str| root.join(path).exists());
    if guards.is_empty() {
        checks.push(("external_dataset_bytes".to_string(), Outcome::Ok));
    } else {
        for guard in guards {
            checks.push((
                "external_dataset_bytes".to_string(),
                Outcome::Blocked(guard),
            ));
        }
    }

    let overlap = production_materialization_overlap(root, &articles);

    Ok(Preflight {
        articles,
        declared,
        endpoint,
        observed_version,
        checks,
        overlap,
    })
}

fn print_preflight(preflight: &Preflight) {
    for (req, outcome) in &preflight.checks {
        println!("PREFLIGHT {req} -> {}", outcome.label());
    }
    println!(
        "PREFLIGHT declared_release -> version={} rollback={} divergence={}",
        preflight.declared.version,
        preflight.declared.rollback_version,
        preflight.declared.label_divergence_item
    );
    println!(
        "PREFLIGHT java_release_probe -> endpoint={} version={}",
        preflight.endpoint,
        if preflight.observed_version.is_empty() {
            "unavailable"
        } else {
            preflight.observed_version.as_str()
        }
    );
    println!(
        "CENSUS production_materialization_overlap -> {}/{}",
        preflight.overlap,
        preflight.articles.len()
    );
    let blocked: Vec<String> = preflight
        .checks
        .iter()
        .filter_map(|(_, outcome)| match outcome {
            Outcome::Blocked(label) => Some(label.clone()),
            Outcome::Ok => None,
        })
        .collect();
    if blocked.is_empty() {
        println!("PREFLIGHT summary -> ok");
    } else {
        println!("PREFLIGHT summary -> blocked:{}", blocked.join(","));
    }
}

// ---------------------------------------------------------------------------
// Capture
// ---------------------------------------------------------------------------

fn ykey(key: &str) -> Value {
    Value::String(key.to_string())
}

fn ystr(value: &str) -> Value {
    Value::String(value.to_string())
}

fn ynum(value: u64) -> Value {
    Value::Number(value.into())
}

fn ymap(pairs: Vec<(&str, Value)>) -> Value {
    let mut map = Mapping::new();
    for (key, value) in pairs {
        map.insert(ykey(key), value);
    }
    Value::Mapping(map)
}

async fn run_capture(root: &Path, args: &Args, preflight: &Preflight) -> Result<()> {
    let out_dir = if args.out.is_absolute() {
        args.out.clone()
    } else {
        root.join(&args.out)
    };
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("{PREFIX} cannot create `{}`", out_dir.display()))?;

    let profile = GrobidProfile::structural();
    let backend = GrobidHttpBackend::new(Some(&preflight.endpoint))
        .with_profile(profile)
        .with_timeout(Duration::from_secs(args.timeout_secs))
        .with_backend_version(preflight.observed_version.clone());

    println!(
        "CAPTURE reference_release -> endpoint={} version={} profile={} consolidate_header={} consolidate_citations={}",
        preflight.endpoint,
        preflight.observed_version,
        profile.name(),
        u8::from(profile.consolidate_header()),
        u8::from(profile.consolidate_citations())
    );

    let mut documents: Vec<(ArticleIdentity, String, u64, u128)> = Vec::new();
    let mut total_bytes: u64 = 0;

    for article in &preflight.articles {
        let pdf_path = root.join(&article.pdf_path);
        let pdf_bytes = std::fs::read(&pdf_path)
            .with_context(|| format!("{PREFIX} cannot read `{}`", pdf_path.display()))?;
        let request = backend.default_request(article.article_id.as_str(), pdf_bytes);
        let started = Instant::now();
        let envelope = backend.fetch_tei(request).await.map_err(|failure| {
            anyhow::anyhow!(
                "{PREFIX} capture failed for `{}`: {failure}",
                article.article_id
            )
        })?;
        let latency_ms = started.elapsed().as_millis();
        let raw = envelope.into_raw_tei();
        let sha256 = format!("sha256:{}", sha256_hex(&raw));
        let size_bytes = raw.len() as u64;
        let fixture = out_dir.join(format!("{}.tei.xml", article.article_id));
        std::fs::write(&fixture, &raw)
            .with_context(|| format!("{PREFIX} cannot write `{}`", fixture.display()))?;
        total_bytes += size_bytes;
        println!(
            "CAPTURE {} -> ok sha256={} size_bytes={} latency_ms={}",
            article.article_id, sha256, size_bytes, latency_ms
        );
        documents.push((article.clone(), sha256, size_bytes, latency_ms));
    }

    let size_note = if total_bytes > SIZE_NOTE_LIMIT_BYTES {
        format!(
            "frozen reference is {total_bytes} bytes (> {SIZE_NOTE_LIMIT_BYTES}); kept in place because \
             the probe must be reproducible from committed bytes"
        )
    } else {
        "within the 4 MiB fixture budget".to_string()
    };

    let captured_at_unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();

    let document_pins = Value::Sequence(
        documents
            .iter()
            .map(|(article, sha256, size_bytes, latency_ms)| {
                ymap(vec![
                    ("article_id", ystr(&article.article_id)),
                    (
                        "path",
                        ystr(&format!("{FIXTURE_DIR}/{}.tei.xml", article.article_id)),
                    ),
                    ("sha256", ystr(sha256)),
                    ("size_bytes", ynum(*size_bytes)),
                    ("source_pdf_sha256", ystr(&article.sha256)),
                    ("latency_ms", ynum(*latency_ms as u64)),
                ])
            })
            .collect(),
    );

    let provenance = ymap(vec![
        (
            "schema_version",
            ystr("external-holdout-reference-provenance.v1"),
        ),
        ("source_manifest", ystr(HOLDOUT_MANIFEST)),
        (
            "source_manifest_sha256",
            ystr(&sha256_file_prefixed(&root.join(HOLDOUT_MANIFEST))?),
        ),
        (
            "identity_digest_ref",
            ystr("external_holdout_contract.release_probe.identity_digest"),
        ),
        ("reference_endpoint", ystr(&preflight.endpoint)),
        ("reference_java_version", ystr(&preflight.observed_version)),
        ("reference_profile", ystr(profile.name())),
        (
            "reference_consolidation_flags",
            ymap(vec![
                (
                    "consolidateHeader",
                    ystr(&u8::from(profile.consolidate_header()).to_string()),
                ),
                (
                    "consolidateCitations",
                    ystr(&u8::from(profile.consolidate_citations()).to_string()),
                ),
                ("teiCoordinates", ystr(GrobidProfile::TEI_COORDINATES)),
            ]),
        ),
        (
            "declared_release",
            ymap(vec![
                ("version", ystr(&preflight.declared.version)),
                (
                    "image_repository",
                    ystr(&preflight.declared.image_repository),
                ),
                ("image_digest", ystr(&preflight.declared.image_digest)),
                ("release_pin", ystr(&preflight.declared.release_pin)),
                (
                    "rollback_version",
                    ystr(&preflight.declared.rollback_version),
                ),
                (
                    "label_divergence_item",
                    ystr(&preflight.declared.label_divergence_item),
                ),
            ]),
        ),
        ("captured_at_revision", ystr(&capture_revision(root))),
        ("captured_at_unix_secs", ynum(captured_at_unix_secs)),
        ("capture_timeout_secs", ynum(args.timeout_secs)),
        ("documents", document_pins),
        ("total_size_bytes", ynum(total_bytes)),
        ("size_note", ystr(&size_note)),
    ]);

    let rendered = serde_yaml::to_string(&provenance)
        .with_context(|| format!("{PREFIX} cannot render provenance YAML"))?;
    let provenance_path = out_dir.join(PROVENANCE_FILE);
    std::fs::write(&provenance_path, rendered)
        .with_context(|| format!("{PREFIX} cannot write `{}`", provenance_path.display()))?;
    println!("CAPTURE provenance -> {}", provenance_path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Verify
// ---------------------------------------------------------------------------

struct VerifyReport {
    checks: Vec<(String, Outcome)>,
}

fn read_provenance(dir: &Path) -> Result<Value> {
    let path = dir.join(PROVENANCE_FILE);
    if !path.is_file() {
        anyhow::bail!(
            "{PREFIX} provenance_absent: `{}` does not exist",
            path.display()
        );
    }
    read_yaml(&path)
}

fn provenance_documents(provenance: &Value) -> Vec<ReferenceDocument> {
    field(provenance, "documents")
        .and_then(Value::as_sequence)
        .map(|list| {
            list.iter()
                .map(|document| ReferenceDocument {
                    article_id: field_str(document, "article_id")
                        .unwrap_or_default()
                        .to_string(),
                    path: field_str(document, "path").unwrap_or_default().to_string(),
                    sha256: field_str(document, "sha256")
                        .unwrap_or_default()
                        .to_string(),
                    size_bytes: field_u64(document, "size_bytes").unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Where a pinned fixture is read from during `--verify`.
///
/// The DATA pins name the committed location; `--in` names the directory whose
/// bytes are being verified. Resolving by file name lets the same integrity
/// check run against a scratch copy (so a corrupted copy is actually read),
/// while the *declared* location stays independently checked against the repo
/// tree by [`declared_path_violation`].
fn fixture_in(dir: &Path, pin_path: &str) -> PathBuf {
    dir.join(Path::new(pin_path).file_name().unwrap_or_default())
}

/// Containment check for the declared pin location: a pin may only name a
/// repo-relative path inside the checkout that signs it.
fn declared_path_violation(root: &Path, article_id: &str, path: &str) -> Option<String> {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Some(format!("reference_path_absolute:{article_id}"));
    }
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Some(format!("reference_path_escape:{article_id}"));
    }
    if !root.join(candidate).is_file() {
        return Some(format!("reference_path_missing:{article_id}"));
    }
    None
}

async fn run_verify(root: &Path, args: &Args) -> Result<VerifyReport> {
    let dir = if args.r#in.is_absolute() {
        args.r#in.clone()
    } else {
        root.join(&args.r#in)
    };
    let provenance = read_provenance(&dir)?;

    let manifest = read_yaml(&root.join(ACCEPTANCE_MANIFEST))?;
    let freeze = reference_freeze(&manifest)?;
    let mut checks: Vec<(String, Outcome)> = Vec::new();

    let provenance_version = field_str(&provenance, "reference_java_version").unwrap_or_default();
    if provenance_version != freeze.reference_java_version {
        checks.push((
            "provenance_release".to_string(),
            Outcome::Blocked(format!("provenance_release_drift:{provenance_version}")),
        ));
    } else {
        checks.push(("provenance_release".to_string(), Outcome::Ok));
    }

    let provenance_profile = field_str(&provenance, "reference_profile").unwrap_or_default();
    if provenance_profile != freeze.reference_profile {
        checks.push((
            "provenance_profile".to_string(),
            Outcome::Blocked(format!("provenance_profile_drift:{provenance_profile}")),
        ));
    } else {
        checks.push(("provenance_profile".to_string(), Outcome::Ok));
    }

    let provenance_manifest_sha =
        field_str(&provenance, "source_manifest_sha256").unwrap_or_default();
    if provenance_manifest_sha != freeze.source_manifest_sha256 {
        checks.push((
            "source_manifest".to_string(),
            Outcome::Blocked(format!("source_manifest_drift:{provenance_manifest_sha}")),
        ));
    } else {
        checks.push(("source_manifest".to_string(), Outcome::Ok));
    }

    let provenance_declared = field(&provenance, "declared_release")
        .and_then(|block| field_str(block, "version"))
        .unwrap_or_default();
    if provenance_declared != freeze.declared.version {
        checks.push((
            "declared_release".to_string(),
            Outcome::Blocked(format!("declared_release_drift:{provenance_declared}")),
        ));
    } else {
        checks.push(("declared_release".to_string(), Outcome::Ok));
    }

    let provenance_endpoint = field_str(&provenance, "reference_endpoint").unwrap_or_default();
    if provenance_endpoint != freeze.reference_endpoint {
        checks.push((
            "provenance_endpoint".to_string(),
            Outcome::Blocked(format!("provenance_endpoint_drift:{provenance_endpoint}")),
        ));
    } else {
        checks.push(("provenance_endpoint".to_string(), Outcome::Ok));
    }

    let provenance_pins = provenance_documents(&provenance);
    for pin in &freeze.documents {
        match declared_path_violation(root, &pin.article_id, &pin.path) {
            Some(label) => checks.push((
                format!("declared_path:{}", pin.article_id),
                Outcome::Blocked(label),
            )),
            None => checks.push((format!("declared_path:{}", pin.article_id), Outcome::Ok)),
        }
        let path = fixture_in(&dir, &pin.path);
        let observed = std::fs::read(&path)
            .ok()
            .map(|bytes| (format!("sha256:{}", sha256_hex(&bytes)), bytes.len() as u64));
        let provenance_pin = provenance_pins
            .iter()
            .find(|candidate| candidate.article_id == pin.article_id);
        let provenance_sha = provenance_pin
            .map(|candidate| candidate.sha256.as_str())
            .unwrap_or_default();
        checks.push((
            format!("reference:{}", pin.article_id),
            admit_reference(
                &pin.article_id,
                observed.is_some(),
                observed.as_ref().map(|(sha, _)| sha.as_str()),
                provenance_sha,
                &pin.sha256,
            ),
        ));
        let size_drift = match (observed.as_ref(), provenance_pin) {
            (Some((_, size)), Some(provenance_pin)) => *size != provenance_pin.size_bytes,
            _ => false,
        };
        if size_drift {
            checks.push((
                format!("reference_size:{}", pin.article_id),
                Outcome::Blocked(format!("reference_size_drift:{}", pin.article_id)),
            ));
        }
    }

    if !args.offline {
        let backend = GrobidHttpBackend::new(Some(&freeze.reference_endpoint));
        match backend.probe_version().await {
            Ok(probe) => checks.push((
                "live_release".to_string(),
                admit_live_release(
                    &freeze.reference_endpoint,
                    &freeze.reference_endpoint,
                    &freeze.reference_java_version,
                    &probe.version,
                ),
            )),
            Err(_) => checks.push((
                "live_release".to_string(),
                Outcome::Blocked("java_endpoint_unavailable".to_string()),
            )),
        }
    } else {
        checks.push(("live_release".to_string(), Outcome::Ok));
    }

    Ok(VerifyReport { checks })
}

// ---------------------------------------------------------------------------
// Self-check
// ---------------------------------------------------------------------------

fn synthetic_release() -> DeclaredRelease {
    DeclaredRelease {
        version: "0.9.1".to_string(),
        image_repository: "synthetic/repository".to_string(),
        image_digest: "sha256:syntheticdigest".to_string(),
        release_pin: "synthetic/pin.yml".to_string(),
        rollback_version: "0.9.0".to_string(),
        label_divergence_item: "H21".to_string(),
    }
}

fn synthetic_article() -> ArticleIdentity {
    ArticleIdentity {
        article_id: "synthetic-ref".to_string(),
        pdf_path: "synthetic/ref.pdf".to_string(),
        sha256: "sha256:synthetic-pinned".to_string(),
        size_bytes: 2048,
    }
}

/// Run every pure predicate on synthetic inputs, including a positive control
/// so the guard cannot pass by refusing everything.
fn run_self_check() -> bool {
    let release = synthetic_release();
    let article = synthetic_article();

    let present = ObservedBytes {
        exists: true,
        sha256: Some(article.sha256.clone()),
        size_bytes: Some(article.size_bytes),
    };
    let absent = ObservedBytes {
        exists: false,
        sha256: None,
        size_bytes: None,
    };
    let sha_drift = ObservedBytes {
        exists: true,
        sha256: Some("sha256:synthetic-drifted".to_string()),
        size_bytes: Some(article.size_bytes),
    };
    let size_drift = ObservedBytes {
        exists: true,
        sha256: Some(article.sha256.clone()),
        size_bytes: Some(article.size_bytes + 1),
    };

    let external_ok = vec![ExternalRow {
        id: "synthetic-external".to_string(),
        state: "bytes_absent".to_string(),
        declared_local_path: None,
    }];
    let external_present = vec![ExternalRow {
        id: "synthetic-external".to_string(),
        state: "bytes_absent".to_string(),
        declared_local_path: Some("synthetic/external".to_string()),
    }];

    let mut cases: Vec<(&str, String, String)> = Vec::new();

    let article_cases = [
        ("article_present", present.clone(), "ok"),
        (
            "article_absent",
            absent.clone(),
            "bytes_absent:synthetic-ref",
        ),
        ("article_sha_drift", sha_drift, "bytes_drift:synthetic-ref"),
        (
            "article_size_drift",
            size_drift,
            "bytes_size_drift:synthetic-ref",
        ),
    ];
    for (case, observed, expected) in article_cases {
        cases.push((
            case,
            admit_article_bytes(&article, &observed).label(),
            expected.to_string(),
        ));
    }

    cases.push((
        "endpoint_unreachable",
        admit_liveness(false).label(),
        "java_endpoint_unavailable".to_string(),
    ));
    cases.push((
        "endpoint_reachable",
        admit_liveness(true).label(),
        "ok".to_string(),
    ));
    cases.push((
        "release_match",
        admit_release(&release, "0.9.1").label(),
        "ok".to_string(),
    ));
    cases.push((
        "release_mismatch",
        admit_release(&release, "0.8.1").label(),
        "java_version_mismatch:0.8.1".to_string(),
    ));
    cases.push((
        "release_undeclared",
        admit_release(
            &DeclaredRelease {
                version: String::new(),
                ..release.clone()
            },
            "0.9.1",
        )
        .label(),
        "java_release_undeclared".to_string(),
    ));
    cases.push((
        "release_pin_match",
        admit_release_pin(
            &release,
            Some(("synthetic/repository", "sha256:syntheticdigest")),
        )
        .label(),
        "ok".to_string(),
    ));
    cases.push((
        "release_pin_drift",
        admit_release_pin(
            &release,
            Some(("synthetic/repository", "sha256:otherdigest")),
        )
        .label(),
        "java_release_pin_drift:synthetic/repository:sha256:otherdigest".to_string(),
    ));
    cases.push((
        "release_pin_missing",
        admit_release_pin(&release, None).label(),
        "java_release_pin_missing".to_string(),
    ));

    let external_ok_label =
        match admit_external_dataset_bytes(&external_ok, |_path: &str| false).as_slice() {
            [] => "ok".to_string(),
            labels => labels[0].clone(),
        };
    cases.push(("external_absent", external_ok_label, "ok".to_string()));
    let external_present_label =
        match admit_external_dataset_bytes(&external_present, |_path: &str| true).as_slice() {
            [] => "ok".to_string(),
            labels => labels[0].clone(),
        };
    cases.push((
        "external_bytes_present",
        external_present_label,
        "external_dataset_bytes_present:synthetic-external".to_string(),
    ));

    cases.push((
        "reference_present",
        admit_reference(
            "synthetic-ref",
            true,
            Some("sha256:synthetic-pinned"),
            "sha256:synthetic-pinned",
            "sha256:synthetic-pinned",
        )
        .label(),
        "ok".to_string(),
    ));
    cases.push((
        "reference_absent",
        admit_reference(
            "synthetic-ref",
            false,
            None,
            "sha256:synthetic-pinned",
            "sha256:synthetic-pinned",
        )
        .label(),
        "reference_absent:synthetic-ref".to_string(),
    ));
    cases.push((
        "reference_drift",
        admit_reference(
            "synthetic-ref",
            true,
            Some("sha256:synthetic-pinned"),
            "sha256:provenance-other",
            "sha256:synthetic-pinned",
        )
        .label(),
        "reference_drift:synthetic-ref".to_string(),
    ));
    cases.push((
        "data_pin_drift",
        admit_reference(
            "synthetic-ref",
            true,
            Some("sha256:synthetic-pinned"),
            "sha256:synthetic-pinned",
            "sha256:data-other",
        )
        .label(),
        "data_pin_drift:synthetic-ref".to_string(),
    ));
    cases.push((
        "live_release_drift",
        admit_live_release(
            "http://127.0.0.1:8070",
            "http://127.0.0.1:8070",
            "0.9.1",
            "0.9.2",
        )
        .label(),
        "live_release_drift:0.9.2".to_string(),
    ));
    cases.push((
        "endpoint_drift",
        admit_live_release(
            "http://127.0.0.1:8070",
            "http://127.0.0.1:9999",
            "0.9.1",
            "0.9.1",
        )
        .label(),
        "endpoint_drift:http://127.0.0.1:9999".to_string(),
    ));

    let mut ok = true;
    for (case, observed, expected) in cases {
        if observed == expected {
            println!("SELFCHECK {case} -> {observed}");
        } else {
            println!("SELFCHECK {case} -> MISMATCH expected={expected} observed={observed}");
            ok = false;
        }
    }
    println!(
        "SELFCHECK summary -> {}",
        if ok { "ok" } else { "mismatch" }
    );
    ok
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // Composition root: .env-backed rolling JSON logging (ADR-052). The guard
    // must live for the whole process — dropping it early silently stops file
    // logging. `--self-check` is a pure predicate matrix with no repo, network
    // or log surface, so it deliberately runs before the composition root is
    // opened.
    if args.self_check {
        // Hermetic: no repo read, no network, no write.
        return if run_self_check() {
            Ok(())
        } else {
            std::process::exit(1);
        };
    }
    let _log_guard = da_cli::logging::init_pipeline_logging("external_holdout_capture");
    let root = repo_root();

    if args.verify {
        let report = run_verify(&root, &args).await?;
        let mut blocked = Vec::new();
        for (req, outcome) in &report.checks {
            println!("VERIFY {req} -> {}", outcome.label());
            if let Outcome::Blocked(label) = outcome {
                blocked.push(label.clone());
            }
        }
        if blocked.is_empty() {
            println!("VERIFY summary -> ok");
            return Ok(());
        }
        println!("VERIFY summary -> blocked:{}", blocked.join(","));
        std::process::exit(1);
    }

    let preflight = run_preflight(&root, &args).await?;
    print_preflight(&preflight);
    if !preflight.checks.iter().all(|(_, outcome)| outcome.is_ok()) {
        // Preflight writes nothing, downloads nothing: a blocked input is a
        // named blocker, never an empty run.
        std::process::exit(1);
    }

    if args.capture {
        run_capture(&root, &args, &preflight).await?;
    } else if args.preflight {
        // Explicit read-only preflight: the gate above is the whole result.
        println!("PREFLIGHT mode -> preflight-only (zero writes)");
    }
    Ok(())
}
