//! Frozen Java reference capture for the S08 external holdout probe
//! (M511-j7wz2f/S08/T02).
//!
//! The S08 probe must compare a *frozen* Java reference against a frozen
//! native emission, so the reference has to be bound to exact inputs, exact
//! bytes and an exact release before any measurement happens. This example is
//! the only writer of that reference:
//!
//! - **`--rederive`** — read-only rule execution: walks the declared corpus,
//!   derives the non-gold population with the declared `selection` rule and
//!   fails with `selection_rule_drift:<detail>` when the derived sequence does
//!   not reproduce the declared `articles` list. No network, no writes.
//! - **`--preflight`** (default) — read-only. It resolves every declared input
//!   (the sha256-pinned holdout PDFs declared in the manifest, the Java
//!   endpoint, the declared Java release, the external `bytes_absent` band, the
//!   production-materialization census) and prints one
//!   `PREFLIGHT <req> -> ok|blocked:<label>` line per requirement. Any blocker
//!   exits non-zero and writes nothing.
//! - **`--capture --out <dir>`** — after a green preflight, runs each PDF
//!   through the existing typed REST transport (one
//!   `POST /api/processFulltextDocument` per document, `structural` profile per
//!   D204) and freezes the raw TEI plus a `provenance.yaml` carrying the
//!   endpoint, the probed live release, the profile and consolidation flags,
//!   the capture revision and per-document latency.
//! - **`--capture --only-new`** — incremental capture for a grown population:
//!   a declared article whose committed fixture still matches its provenance
//!   pin is adopted unchanged (pin and latency preserved verbatim) and only the
//!   remaining articles are sent to the reference. A pinned fixture whose bytes
//!   drifted blocks as `only_new_pinned_reference_drift:<id>`, so a rerun can
//!   never silently overwrite frozen bytes.
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

use anyhow::{Context, Result, bail};
use clap::Parser;
use da_parse::{GrobidHttpBackend, GrobidProfile, ParseBackend};
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// Panic/log prefix, so a failure names the tool that produced it.
const PREFIX: &str = "[external-holdout-capture]";

/// Declared holdout identity manifest (sha256-pinned non-gold arXiv PDFs; the
/// population size is read from DATA, never pinned in code).
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
/// Declared selection rule family; `selection.rule_version` selects the
/// implementation, so an unknown version is a named blocker rather than a
/// silent reuse of some other rule's semantics.
const SELECTION_RULE_VERSION: &str = "external-holdout-selection.v1";
/// The only ordering the executable rule implements: ascending `article_id`
/// (string order, so the derivation is byte-stable across environments).
const SELECTION_ORDERING: &str = "ascending_article_id";

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
    /// Adopt verified existing pins and capture only the declared articles
    /// that are not already frozen in `--out`. Existing frozen bytes and their
    /// provenance pins are kept byte-for-byte; a pinned fixture whose bytes
    /// drifted is a named blocker, never a silent overwrite.
    #[arg(long)]
    only_new: bool,
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
    /// Re-derive the declared population from the corpus with the declared
    /// selection rule (read-only: no network, no writes).
    #[arg(long)]
    rederive: bool,
    /// Corpus root override for `--rederive` (defaults to
    /// `selection.corpus_root`).
    #[arg(long, value_name = "DIR")]
    corpus: Option<PathBuf>,
    /// Manifest override for `--rederive` (defaults to the committed holdout
    /// manifest).
    #[arg(long, value_name = "PATH")]
    manifest: Option<PathBuf>,
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

// ---------------------------------------------------------------------------
// Executable selection rule (`external-holdout-selection.v1`)
//
// The `selection` block of the holdout manifest used to be prose: no code read
// it, so the declared population was only as trustworthy as a hand-kept list,
// and a rule that disagrees with the corpus (the literal "`original.pdf`
// inherits the parent directory's identity" rule resolves to `source`) could
// not be detected. `--rederive` executes the declared rule against the corpus
// and refuses a declaration the rule does not reproduce
// (`selection_rule_drift`), so the census is a derivation rather than an
// inventory (D453). The rule itself is DATA-driven: `rule_version` selects the
// algorithm, `ordering` and `exclude` are read from the manifest.
// ---------------------------------------------------------------------------

/// Resolved `selection` block: where to scan and what to exclude.
struct SelectionSpec {
    corpus_root: PathBuf,
    gold_glob: String,
    excluded_identities: BTreeSet<String>,
    take_first: Option<usize>,
}

/// One derived holdout row: the winning asset for one arXiv identity.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedRow {
    article_id: String,
    display_path: String,
}

/// Derived census: measured counters plus the ascending derived sequence.
struct Selection {
    assets: usize,
    identities: usize,
    excluded_gold: usize,
    derived: Vec<DerivedRow>,
}

fn selection_spec(
    root: &Path,
    manifest: &Value,
    corpus_override: Option<&Path>,
) -> Result<SelectionSpec> {
    let selection = field(manifest, "selection").context(format!(
        "{PREFIX} selection_block_absent: the holdout manifest carries no `selection` block"
    ))?;
    let rule_version = field_str(selection, "rule_version").unwrap_or_default();
    if rule_version != SELECTION_RULE_VERSION {
        bail!("selection_rule_version_unsupported:{rule_version}");
    }
    let ordering = field_str(selection, "ordering").unwrap_or_default();
    if ordering != SELECTION_ORDERING {
        bail!("selection_ordering_unsupported:{ordering}");
    }
    let corpus_root = match corpus_override {
        Some(dir) => dir.to_path_buf(),
        None => PathBuf::from(field_str(selection, "corpus_root").context(format!(
            "{PREFIX} selection_corpus_root_absent: `selection.corpus_root` is required unless \
             `--corpus` is passed"
        ))?),
    };
    let corpus_root = if corpus_root.is_absolute() {
        corpus_root
    } else {
        root.join(corpus_root)
    };
    let exclude = field(selection, "exclude");
    let gold_glob = exclude
        .and_then(|block| field_str(block, "gold_standard_glob"))
        .context(format!(
            "{PREFIX} selection_gold_glob_absent: `selection.exclude.gold_standard_glob` is \
             required"
        ))?
        .to_string();
    let excluded_identities = exclude
        .and_then(|block| field(block, "identities"))
        .and_then(Value::as_sequence)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<BTreeSet<String>>()
        })
        .unwrap_or_default();
    let take_first = field(selection, "take_first")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    Ok(SelectionSpec {
        corpus_root,
        gold_glob,
        excluded_identities,
        take_first,
    })
}

/// Classic `*`/`?` wildcard match over a single path component.
fn glob_match(pattern: &str, name: &str) -> bool {
    let (pattern, name) = (pattern.as_bytes(), name.as_bytes());
    let (mut p, mut n) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut star_name = 0usize;
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            star_name = n;
            p += 1;
        } else if let Some(index) = star {
            p = index + 1;
            star_name += 1;
            n = star_name;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

/// The gold identity a matched gold file denotes, per the declared
/// `gold_identity_rule`: the file name with the glob's literal suffix removed.
fn gold_identity(name: &str, pattern: &str) -> String {
    match pattern.rfind('*') {
        Some(index) => {
            let suffix = &pattern[index + 1..];
            name.strip_suffix(suffix).unwrap_or(name).to_string()
        }
        None => name.to_string(),
    }
}

/// Expand the declared gold glob into the set of gold identities. Only the
/// final component may carry wildcards, which is the declared shape
/// (`data/gold_standard/*.gold.json`); a relative glob resolves against the
/// repo root, an absolute one is used as given (hermetic tests).
fn gold_identities(root: &Path, glob: &str) -> Result<BTreeSet<String>> {
    let pattern = Path::new(glob);
    let expanded = if pattern.is_absolute() {
        pattern.to_path_buf()
    } else {
        root.join(pattern)
    };
    let dir = expanded
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .context(format!(
            "{PREFIX} selection_gold_glob_malformed: `{glob}` has no directory component"
        ))?;
    let name_pattern = expanded
        .file_name()
        .and_then(|name| name.to_str())
        .context(format!(
            "{PREFIX} selection_gold_glob_malformed: `{glob}` has no file component"
        ))?;
    let unreadable = || {
        format!(
            "{PREFIX} selection_gold_glob_unreadable: `{}` cannot be read",
            dir.display()
        )
    };
    let mut identities = BTreeSet::new();
    for entry in std::fs::read_dir(dir).with_context(unreadable)? {
        let entry = entry.with_context(unreadable)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if glob_match(name_pattern, &name) {
            identities.insert(gold_identity(&name, name_pattern));
        }
    }
    Ok(identities)
}

/// `^\d{4}\.\d{4,5}$` — the arXiv identity shape the rule recognises.
fn is_arxiv_identity(value: &str) -> bool {
    let Some((year, number)) = value.split_once('.') else {
        return false;
    };
    year.len() == 4
        && year.bytes().all(|byte| byte.is_ascii_digit())
        && (number.len() == 4 || number.len() == 5)
        && number.bytes().all(|byte| byte.is_ascii_digit())
}

/// Split a trailing `vN` (N >= 1, no leading zero) off a name, returning the
/// remainder and the revision (0 when the name carries no version suffix).
fn strip_version_suffix(name: &str) -> (&str, u32) {
    let bytes = name.as_bytes();
    let mut start = bytes.len();
    while start > 0 && bytes[start - 1].is_ascii_digit() {
        start -= 1;
    }
    if start == bytes.len() || start == 0 || bytes[start - 1] != b'v' {
        return (name, 0);
    }
    let digits = &name[start..];
    if digits.starts_with('0') {
        return (name, 0);
    }
    match digits.parse::<u32>() {
        Ok(revision) => (&name[..start - 1], revision),
        Err(_) => (name, 0),
    }
}

/// The arXiv identity of one PDF, plus its revision. The PDF stem wins when it
/// is an identity; otherwise the nearest ancestor directory that is one
/// (`<cat>/<id>/source/original.pdf` inherits `<id>`). The revision comes from
/// the identity-bearing name, so a versioned directory is honoured.
fn asset_identity(relative: &Path) -> Option<(String, u32)> {
    let mut candidates: Vec<&str> = Vec::new();
    if let Some(stem) = relative.file_stem().and_then(|stem| stem.to_str()) {
        candidates.push(stem);
    }
    let mut ancestor = relative.parent();
    while let Some(directory) = ancestor {
        if let Some(name) = directory.file_name().and_then(|name| name.to_str()) {
            candidates.push(name);
        }
        ancestor = directory.parent();
    }
    for candidate in candidates {
        let (stripped, revision) = strip_version_suffix(candidate);
        if is_arxiv_identity(stripped) {
            return Some((stripped.to_string(), revision));
        }
    }
    None
}

/// Repo-relative display path when the corpus lives inside the repo, otherwise
/// the corpus-relative path (hermetic tests use temp directories).
fn display_path(corpus_root: &Path, relative: &Path) -> String {
    let absolute = corpus_root.join(relative);
    match absolute.strip_prefix(repo_root()) {
        Ok(repo_relative) => repo_relative.to_string_lossy().replace('\\', "/"),
        Err(_) => relative.to_string_lossy().replace('\\', "/"),
    }
}

/// Path relative to the repo root when possible, else the absolute path.
fn relative_display(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => path.to_string_lossy().replace('\\', "/"),
    }
}

/// Execute the declared rule: walk the corpus, resolve each asset's identity,
/// keep the winning asset per identity (lowest revision, then lexicographically
/// smallest path), drop the declared exclusions and order ascending.
fn derive_selection(root: &Path, spec: &SelectionSpec) -> Result<Selection> {
    let gold = gold_identities(root, &spec.gold_glob)?;
    let mut by_identity: BTreeMap<String, (u32, PathBuf)> = BTreeMap::new();
    let mut assets = 0usize;
    for entry in WalkDir::new(&spec.corpus_root).follow_links(false) {
        let entry = entry.with_context(|| {
            format!(
                "{PREFIX} selection_corpus_unreadable: `{}` cannot be walked",
                spec.corpus_root.display()
            )
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let is_pdf = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
        if !is_pdf {
            continue;
        }
        assets += 1;
        let relative = path
            .strip_prefix(&spec.corpus_root)
            .with_context(|| {
                format!(
                    "{PREFIX} selection_corpus_root_escape: `{}` is outside the corpus root",
                    path.display()
                )
            })?
            .to_path_buf();
        let (identity, revision) = asset_identity(&relative).ok_or_else(|| {
            anyhow::anyhow!("selection_asset_without_identity:{}", relative.display())
        })?;
        let replace = match by_identity.get(&identity) {
            Some((best_revision, best_path)) => {
                (revision, relative.as_path()) < (*best_revision, best_path.as_path())
            }
            None => true,
        };
        if replace {
            by_identity.insert(identity, (revision, relative));
        }
    }
    let identities = by_identity.len();
    let mut excluded_gold = 0usize;
    let mut derived = Vec::new();
    for (article_id, (_revision, relative)) in by_identity {
        if gold.contains(&article_id) {
            excluded_gold += 1;
            continue;
        }
        if spec.excluded_identities.contains(&article_id) {
            continue;
        }
        derived.push(DerivedRow {
            article_id,
            display_path: display_path(&spec.corpus_root, &relative),
        });
    }
    derived.sort_by(|left, right| left.article_id.cmp(&right.article_id));
    if let Some(limit) = spec.take_first {
        derived.truncate(limit);
    }
    Ok(Selection {
        assets,
        identities,
        excluded_gold,
        derived,
    })
}

/// The declared `articles[].article_id` sequence, in file order.
fn declared_article_ids(manifest: &Value) -> Result<Vec<String>> {
    let articles = field(manifest, "articles")
        .and_then(Value::as_sequence)
        .context(format!(
            "{PREFIX} the holdout manifest carries no `articles` list"
        ))?;
    let mut ids = Vec::with_capacity(articles.len());
    for article in articles {
        ids.push(
            field_str(article, "article_id")
                .context(format!(
                    "{PREFIX} a holdout article carries no `article_id`"
                ))?
                .to_string(),
        );
    }
    Ok(ids)
}

/// The `selection_rule_drift` detail: the first diverging ids, so a failing run
/// localises the disagreement without diffing hundreds of lines by hand.
fn selection_drift(derived: &[DerivedRow], declared: &[String]) -> Option<String> {
    let derived_list: Vec<&str> = derived.iter().map(|row| row.article_id.as_str()).collect();
    let declared_list: Vec<&str> = declared.iter().map(String::as_str).collect();
    let derived_set: BTreeSet<&str> = derived_list.iter().copied().collect();
    let declared_set: BTreeSet<&str> = declared_list.iter().copied().collect();
    let missing: Vec<&str> = declared_list
        .iter()
        .copied()
        .filter(|id| !derived_set.contains(id))
        .take(3)
        .collect();
    let extra: Vec<&str> = derived_list
        .iter()
        .copied()
        .filter(|id| !declared_set.contains(id))
        .take(3)
        .collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Some(format!(
            "missing=[{}];extra=[{}]",
            missing.join(","),
            extra.join(",")
        ));
    }
    if derived_list.len() != declared_list.len() {
        return Some(format!(
            "length:{}!={}",
            derived_list.len(),
            declared_list.len()
        ));
    }
    for (index, (derived_id, declared_id)) in
        derived_list.iter().zip(declared_list.iter()).enumerate()
    {
        if derived_id != declared_id {
            return Some(format!("order@{index}:{derived_id}!={declared_id}"));
        }
    }
    None
}

/// `--rederive`: execute the declared rule and compare the derived sequence
/// with the declared one. Read-only by construction: the corpus is only walked,
/// the network is never touched and nothing is written.
fn run_rederive(root: &Path, args: &Args) -> Result<bool> {
    let manifest_path = match &args.manifest {
        Some(path) if path.is_absolute() => path.clone(),
        Some(path) => root.join(path),
        None => root.join(HOLDOUT_MANIFEST),
    };
    let manifest = read_yaml(&manifest_path)?;
    let spec = selection_spec(root, &manifest, args.corpus.as_deref())?;
    let selection = derive_selection(root, &spec)?;
    let declared = declared_article_ids(&manifest)?;

    println!(
        "REDERIVE manifest -> {}",
        relative_display(root, &manifest_path)
    );
    println!(
        "REDERIVE corpus_root -> {}",
        relative_display(root, &spec.corpus_root)
    );
    println!("REDERIVE assets -> {}", selection.assets);
    println!("REDERIVE identities -> {}", selection.identities);
    println!("REDERIVE excluded_gold -> {}", selection.excluded_gold);
    println!("REDERIVE declared -> {}", declared.len());
    println!("REDERIVE derived -> {}", selection.derived.len());
    for row in &selection.derived {
        println!(
            "REDERIVE article -> {} {}",
            row.article_id, row.display_path
        );
    }
    match selection_drift(&selection.derived, &declared) {
        None => {
            println!("REDERIVE summary -> ok");
            Ok(true)
        }
        Some(detail) => {
            println!("REDERIVE summary -> blocked:selection_rule_drift:{detail}");
            Ok(false)
        }
    }
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
    let measurable = field(lane, "reference_tei")
        .and_then(Value::as_sequence)
        .context(format!(
            "{PREFIX} reference_freeze_captured_absent: the release_probe lane carries no \
             `reference_tei` list"
        ))?;
    // The frozen population is partitioned into the measurable `reference_tei`
    // subset and the typed `reference_exceptions`. `--verify` re-hashes BOTH:
    // an exception is frozen reference bytes too, so its pins are checked rather
    // than skipped. The split is a measurement fact, not a licence to stop
    // verifying the excluded bytes.
    let exceptions: Vec<Value> = field(lane, "reference_exceptions")
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let mut pins = Vec::new();
    for document in measurable.iter().chain(exceptions.iter()) {
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

/// The surfaces a bring-up reads when it decides which GROBID image to run.
///
/// `.env` is gitignored, so a checkout may not have it; when it exists it is what the
/// local stack actually runs, so it is checked rather than ignored. The committed
/// defaults are `.env.example` and the compose file.
const BRING_UP_SURFACES: &[&str] = &[".env", ".env.example", ".docker/docker-compose.yml"];

/// Read the declared GROBID image on one bring-up surface, if the surface exists.
///
/// Returns `None` when the file is absent (legal only for the gitignored `.env`) or
/// when it declares no GROBID image at all; the caller distinguishes those.
fn bring_up_image_reference(root: &Path, surface: &str) -> Option<String> {
    let path = root.join(surface);
    let text = std::fs::read_to_string(&path).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        // dotenv surfaces declare `GROBID_IMAGE=<reference>`.
        if let Some(value) = trimmed.strip_prefix("GROBID_IMAGE=") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        // The compose surface may hardcode the image or default it inline.
        if let Some(rest) = trimmed.strip_prefix("image:") {
            let rest = rest.trim();
            let candidate = match rest.find(":-") {
                Some(index) => rest[index + 2..].trim_end_matches('}').trim(),
                None => rest,
            };
            let candidate = candidate.trim_matches('"').trim_matches('\'');
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

/// Pure decision for one bring-up surface, so the self-check can exercise the guard
/// against synthetic references without touching the filesystem.
///
/// A tag is refused even when it names the right release: a tag can be re-pointed
/// upstream, which is the same reason the pin file uses a digest.
fn admit_bring_up_reference(
    declared: &DeclaredRelease,
    surface: &str,
    reference: Option<&str>,
) -> Outcome {
    let Some(reference) = reference else {
        return Outcome::Blocked(format!("bring_up_reference_missing:{surface}"));
    };
    let Some((repository, digest)) = reference.split_once('@') else {
        return Outcome::Blocked(format!("bring_up_reference_tag_form:{surface}:{reference}"));
    };
    if repository == declared.image_repository && digest == declared.image_digest {
        Outcome::Ok
    } else {
        Outcome::Blocked(format!(
            "bring_up_reference_drift:{surface}:{repository}@{digest}"
        ))
    }
}

/// Count how many declared holdout ids are also materialized production
/// coverage. Measured, never asserted: the census is printed as
/// `n/<declared population>`, so growing the population cannot leave a stale
/// literal behind.
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
    // The pin file is only one of the surfaces that decide which image runs. The
    // bring-up defaults decide it for anyone who starts the stack without the override
    // layer, and they lagged the declared release until M513/S01/T01. A surface that is
    // absent is skipped so a checkout without the gitignored `.env` stays legal; a
    // surface that exists and disagrees blocks by name, including in tag form.
    for surface in BRING_UP_SURFACES {
        if !root.join(surface).exists() {
            checks.push((format!("java_release_bring_up:{surface}"), Outcome::Ok));
            continue;
        }
        let reference = bring_up_image_reference(root, surface);
        checks.push((
            format!("java_release_bring_up:{surface}"),
            admit_bring_up_reference(&declared, surface, reference.as_deref()),
        ));
    }

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

/// One `documents[*]` entry exactly as it appears in `provenance.yaml`.
fn reference_document_value(
    article: &ArticleIdentity,
    sha256: &str,
    size_bytes: u64,
    latency_ms: u128,
) -> Value {
    ymap(vec![
        ("article_id", ystr(&article.article_id)),
        (
            "path",
            ystr(&format!("{FIXTURE_DIR}/{}.tei.xml", article.article_id)),
        ),
        ("sha256", ystr(sha256)),
        ("size_bytes", ynum(size_bytes)),
        ("source_pdf_sha256", ystr(&article.sha256)),
        ("latency_ms", ynum(latency_ms as u64)),
    ])
}

/// Under `--only-new`, return the existing provenance entry for `article`
/// when its frozen fixture is present and still matches. A fixture that exists
/// but drifted from its own pin is a named blocker, never a silent overwrite.
/// With an empty `existing_documents` (the plain `--capture` path) this is a
/// no-op and the article is captured normally.
fn adopt_existing_pin(
    existing_documents: &[Value],
    out_dir: &Path,
    article: &ArticleIdentity,
) -> Result<Option<(Value, u64)>> {
    let Some(existing) = existing_documents
        .iter()
        .find(|document| field_str(document, "article_id") == Some(article.article_id.as_str()))
    else {
        return Ok(None);
    };
    let fixture = out_dir.join(format!("{}.tei.xml", article.article_id));
    let Ok(frozen) = std::fs::read(&fixture) else {
        // No frozen bytes yet: this article is simply not captured.
        return Ok(None);
    };
    let frozen_sha = format!("sha256:{}", sha256_hex(&frozen));
    let frozen_size = frozen.len() as u64;
    let pinned_sha = field_str(existing, "sha256").unwrap_or_default();
    let pinned_size = field_u64(existing, "size_bytes");
    if frozen_sha != pinned_sha || pinned_size != Some(frozen_size) {
        // A pinned fixture that no longer matches its own pin is never papered
        // over: refuse and name the article.
        anyhow::bail!(
            "{PREFIX} only_new_pinned_reference_drift:{}: frozen bytes \
             sha256={frozen_sha} size_bytes={frozen_size} disagree with the \
             provenance pin sha256={pinned_sha} size_bytes={}",
            article.article_id,
            pinned_size.map_or_else(|| "absent".to_string(), |s| s.to_string())
        );
    }
    println!(
        "CAPTURE {} -> adopted existing pin sha256={} size_bytes={}",
        article.article_id, pinned_sha, frozen_size
    );
    Ok(Some((existing.clone(), frozen_size)))
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

    // `--only-new`: adopt every declared article whose frozen fixture still
    // matches its provenance pin, and capture only the rest. This is what lets
    // a grown population extend the frozen reference without overwriting the
    // already-committed rows: the pilot measurement found that a re-capture of
    // the already-adopted documents is not byte-identical, so a blind `--capture`
    // would silently replace them and break their DATA pins.
    let existing_documents: Vec<Value> = if args.only_new {
        let path = out_dir.join(PROVENANCE_FILE);
        if path.is_file() {
            field(&read_yaml(&path)?, "documents")
                .and_then(Value::as_sequence)
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let mut document_pins: Vec<Value> = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut adopted: usize = 0;
    let mut captured: usize = 0;

    for article in &preflight.articles {
        if let Some((pin, size)) = adopt_existing_pin(&existing_documents, &out_dir, article)? {
            total_bytes += size;
            document_pins.push(pin);
            adopted += 1;
            continue;
        }

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
        document_pins.push(reference_document_value(
            article, &sha256, size_bytes, latency_ms,
        ));
        captured += 1;
    }

    println!(
        "CAPTURE summary -> adopted={adopted} captured={captured} declared={}",
        preflight.articles.len()
    );

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
        ("documents", Value::Sequence(document_pins)),
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

    // Bring-up surfaces (M513/S01/T02). Positive control first: the guard must accept the
    // declared reference, otherwise every rejection below would be vacuous.
    let declared_reference = format!("{}@{}", release.image_repository, release.image_digest);
    cases.push((
        "bring_up_reference_declared_is_accepted",
        admit_bring_up_reference(&release, ".env.example", Some(&declared_reference)).label(),
        "ok".to_string(),
    ));
    cases.push((
        "bring_up_reference_drift",
        admit_bring_up_reference(
            &release,
            ".env.example",
            Some(&format!(
                "{}@sha256:{}",
                release.image_repository,
                "0".repeat(64)
            )),
        )
        .label(),
        format!(
            "bring_up_reference_drift:.env.example:{}@sha256:{}",
            release.image_repository,
            "0".repeat(64)
        ),
    ));
    cases.push((
        "bring_up_reference_tag_form",
        admit_bring_up_reference(
            &release,
            ".docker/docker-compose.yml",
            Some(&format!("{}:0.9.0-crf", release.image_repository)),
        )
        .label(),
        format!(
            "bring_up_reference_tag_form:.docker/docker-compose.yml:{}:0.9.0-crf",
            release.image_repository
        ),
    ));
    cases.push((
        "bring_up_reference_missing",
        admit_bring_up_reference(&release, ".env", None).label(),
        "bring_up_reference_missing:.env".to_string(),
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

    if args.rederive {
        // Read-only rule execution: no network, no writes.
        return if run_rederive(&root, &args)? {
            Ok(())
        } else {
            std::process::exit(1);
        };
    }

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
