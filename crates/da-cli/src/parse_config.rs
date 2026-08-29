//! `ParseRuntimeConfig` — the R116 composition-root configuration for
//! `da parse-batch` (M464-z84in2 S06/T04).
//!
//! Physical roots, endpoint, cache mode, worker/queue bounds and redb
//! accelerator settings belong HERE (da-cli) — never in da-parse,
//! da-domain or da-application (architecture contract scans enforce the
//! absence of `DA_PARSE_*` / `DA_GROBID_URL` / `GROBID_URL` markers from
//! inner layers).
//!
//! Precedence: explicit CLI value > `DA_PARSE_*` env > composition-root
//! default. Validation is closed and fails BEFORE the backend is opened:
//! blank values, unknown enum values, zero bounds, values over the
//! composition-root resource caps, a `GROBID_URL`/`DA_GROBID_URL`
//! conflict (fails closed — no silent pick), a missing redb path while
//! the accelerator is enabled, and a redb path that overlaps the
//! canonical cache root are all typed errors.
//!
//! Legacy alias: the historical `GROBID_URL` keeps its meaning as an
//! explicit, tested compatibility alias for the canonical `DA_GROBID_URL`.
//! When both are set they must agree exactly, or resolution fails closed.
//! D204 profile/cache identity is untouched — the URL selection only
//! chooses which value the (unchanged) backend identity is built from.

use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};

use da_parse::{CacheMode, ParseBatchLimits};

// ---------------------------------------------------------------------------
// Closed env-key vocabulary (documented in data/parse/grobid_compatibility.yaml
// `configuration_keys_documented_not_owned`; owned HERE per R116)
// ---------------------------------------------------------------------------

/// Canonical GROBID endpoint key.
pub const DA_GROBID_URL_KEY: &str = "DA_GROBID_URL";
/// Historical GROBID endpoint key — explicit compatibility alias for
/// [`DA_GROBID_URL_KEY`] (both set → must match, else fail closed).
pub const GROBID_URL_ALIAS_KEY: &str = "GROBID_URL";
/// Closed backend selector key.
pub const DA_PARSE_BACKEND_KEY: &str = "DA_PARSE_BACKEND";
/// Closed cache-mode key (`off` | `read-only` | `write-through` | `refresh`).
pub const DA_PARSE_CACHE_MODE_KEY: &str = "DA_PARSE_CACHE_MODE";
/// Canonical parse artifact cache root key.
pub const DA_PARSE_CACHE_ROOT_KEY: &str = "DA_PARSE_CACHE_ROOT";
/// Maximum documents per admitted batch.
pub const DA_PARSE_BATCH_SIZE_KEY: &str = "DA_PARSE_BATCH_SIZE";
/// Worker concurrency bound.
pub const DA_PARSE_WORKERS_KEY: &str = "DA_PARSE_WORKERS";
/// Admission queue capacity bound.
pub const DA_PARSE_QUEUE_CAPACITY_KEY: &str = "DA_PARSE_QUEUE_CAPACITY";
/// redb accelerator on/off (`true` | `false`).
pub const DA_PARSE_REDB_INDEX_ENABLED_KEY: &str = "DA_PARSE_REDB_INDEX_ENABLED";
/// redb accelerator file path (required only when enabled).
pub const DA_PARSE_REDB_INDEX_PATH_KEY: &str = "DA_PARSE_REDB_INDEX_PATH";

// ---------------------------------------------------------------------------
// Composition-root defaults + closed resource caps (overflow protection)
// ---------------------------------------------------------------------------

/// Default GROBID endpoint (composition-root default; identical value the
/// legacy `GrobidParser::from_env` facade has always defaulted to).
pub const DEFAULT_GROBID_URL: &str = "http://127.0.0.1:8070";
/// Default cache mode: the standard production cost strategy.
pub const DEFAULT_CACHE_MODE: CacheMode = CacheMode::WriteThrough;
/// Default parse artifact cache root (repo-relative, created on demand).
pub const DEFAULT_CACHE_ROOT: &str = "artifacts/parse-batch/cache";
/// Default maximum documents per batch.
pub const DEFAULT_BATCH_DOCUMENTS: usize = 1000;
/// Default worker count.
pub const DEFAULT_WORKERS: usize = 4;
/// Default admission queue capacity.
pub const DEFAULT_QUEUE_CAPACITY: usize = 64;

/// Upper resource caps (overflow protection at the composition root).
pub const MAX_WORKERS: usize = 64;
/// Upper resource caps (overflow protection at the composition root).
pub const MAX_IN_FLIGHT: usize = 64;
/// Upper resource caps (overflow protection at the composition root).
pub const MAX_QUEUE_CAPACITY: usize = 65_536;
/// Upper resource caps (overflow protection at the composition root).
pub const MAX_BATCH_DOCUMENTS: usize = 100_000;

// ---------------------------------------------------------------------------
// Closed backend vocabulary
// ---------------------------------------------------------------------------

/// Closed CLI backend selector (values of `DA_PARSE_BACKEND` /
/// `--backend`). T04 wires exactly one production backend; the closed
/// enum keeps unknown values a typed error instead of a silent default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseBackendKind {
    /// Java GROBID HTTP backend (`grobid-http`).
    GrobidHttp,
}

impl ParseBackendKind {
    /// Closed wire/enum value (identical to the `DA_PARSE_BACKEND` token).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GrobidHttp => "grobid-http",
        }
    }

    /// Parse the closed token.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "grobid-http" => Some(Self::GrobidHttp),
            _ => None,
        }
    }
}

/// Parse the closed cache-mode token (kebab-case, identical to the
/// `da_parse::CacheMode` serde wire form).
pub fn parse_cache_mode(value: &str) -> Option<CacheMode> {
    match value {
        "off" => Some(CacheMode::Off),
        "read-only" => Some(CacheMode::ReadOnly),
        "write-through" => Some(CacheMode::WriteThrough),
        "refresh" => Some(CacheMode::Refresh),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// redb accelerator settings
// ---------------------------------------------------------------------------

/// Optional redb accelerator settings (D295: rebuildable ONLY — never
/// authoritative; canonical terminal evidence is the versioned result
/// report plus `FilePipelineCache` payload/manifest records).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRedbIndexConfig {
    /// Whether the accelerator is enabled at all. Disabled mode requires
    /// no redb path.
    enabled: bool,
    /// Accelerator file path (required iff enabled; must not overlap the
    /// canonical cache root). Ignored while disabled.
    path: Option<PathBuf>,
}

impl ParseRedbIndexConfig {
    /// The enabled/disabled flag.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// The configured accelerator path (present only when enabled; tests
    /// and diagnostics may read it, the disabled mode never opens one).
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

// ---------------------------------------------------------------------------
// Typed closed validation errors (no secrets, no base URLs, no values echoed)
// ---------------------------------------------------------------------------

/// Closed composition-config failure vocabulary. Messages name the key and
/// the rule — never the offending value, base URL or physical root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseConfigError {
    /// The key was present but blank (whitespace-only). Blank is an
    /// invalid value, not an unset key.
    BlankValue { key: &'static str },
    /// The value is not in the closed token set for that key.
    UnknownValue {
        key: &'static str,
        expected: &'static str,
    },
    /// A numeric key is not a valid unsigned integer.
    InvalidNumber { key: &'static str },
    /// A bound key is zero (zero workers/queue/documents are degenerate).
    ZeroForbidden { key: &'static str },
    /// A bound key exceeds its composition-root resource cap.
    Overflow { key: &'static str, max: usize },
    /// `GROBID_URL` and `DA_GROBID_URL` are both set with different
    /// values — resolution fails closed (no silent pick).
    EndpointAliasConflict,
    /// The GROBID endpoint must be an http(s) URL.
    InvalidEndpoint,
    /// The redb accelerator is enabled but no path was configured.
    MissingRedbPath,
    /// The enabled redb path equals or overlaps the canonical cache root
    /// (the accelerator must never sit inside, or contain, canonical
    /// payload state).
    RedbPathOverlapsCacheRoot,
    /// `max_in_flight` exceeds `queue_capacity` — inconsistent bounds are
    /// rejected before any backend work.
    InconsistentBounds {
        max_in_flight: usize,
        queue_capacity: usize,
    },
}

impl std::fmt::Display for ParseConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlankValue { key } => {
                write!(
                    formatter,
                    "{key} is set but blank (unset the key or provide a value)"
                )
            }
            Self::UnknownValue { key, expected } => {
                write!(
                    formatter,
                    "{key} has an unknown value (accepted: {expected})"
                )
            }
            Self::InvalidNumber { key } => {
                write!(formatter, "{key} is not a valid unsigned integer")
            }
            Self::ZeroForbidden { key } => {
                write!(formatter, "{key} must be >= 1 (zero bounds are rejected)")
            }
            Self::Overflow { key, max } => {
                write!(formatter, "{key} exceeds the composition-root cap {max}")
            }
            Self::EndpointAliasConflict => write!(
                formatter,
                "{DA_GROBID_URL_KEY} and {GROBID_URL_ALIAS_KEY} are both set with different values; resolve the conflict (they must match exactly)"
            ),
            Self::InvalidEndpoint => write!(
                formatter,
                "{DA_GROBID_URL_KEY} must be an http or https URL"
            ),
            Self::MissingRedbPath => write!(
                formatter,
                "{DA_PARSE_REDB_INDEX_PATH_KEY} is required when {DA_PARSE_REDB_INDEX_ENABLED_KEY}=true"
            ),
            Self::RedbPathOverlapsCacheRoot => write!(
                formatter,
                "{DA_PARSE_REDB_INDEX_PATH_KEY} must not equal or overlap the canonical cache root ({DA_PARSE_CACHE_ROOT_KEY})"
            ),
            Self::InconsistentBounds {
                max_in_flight,
                queue_capacity,
            } => write!(
                formatter,
                "max-in-flight ({max_in_flight}) must not exceed queue-capacity ({queue_capacity})"
            ),
        }
    }
}

impl std::error::Error for ParseConfigError {}

// ---------------------------------------------------------------------------
// CLI overrides + resolution
// ---------------------------------------------------------------------------

/// Explicit CLI values (None = flag absent → fall through to env, then
/// default). Blank CLI values are rejected exactly like blank env values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseRuntimeOverrides {
    /// `--grobid-url`
    pub grobid_url: Option<String>,
    /// `--backend`
    pub backend: Option<String>,
    /// `--mode`
    pub cache_mode: Option<String>,
    /// `--cache-root`
    pub cache_root: Option<PathBuf>,
    /// `--batch-size`
    pub batch_size: Option<usize>,
    /// `--workers`
    pub workers: Option<usize>,
    /// `--queue-capacity`
    pub queue_capacity: Option<usize>,
    /// `--max-in-flight` (CLI-only bound; defaults to `workers`)
    pub max_in_flight: Option<usize>,
    /// `--redb-index <path>` (present ⇒ enabled) / `--no-redb` (present ⇒ disabled)
    pub redb_enabled: Option<bool>,
    /// `--redb-index-path`
    pub redb_path: Option<PathBuf>,
}

/// The resolved, closed-validated composition config. All physical
/// locations and bounds live here; the batch API receives none of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRuntimeConfig {
    backend: ParseBackendKind,
    grobid_url: String,
    cache_mode: CacheMode,
    cache_root: PathBuf,
    max_documents: NonZeroUsize,
    workers: NonZeroUsize,
    queue_capacity: NonZeroUsize,
    max_in_flight: NonZeroUsize,
    redb: ParseRedbIndexConfig,
}

impl ParseRuntimeConfig {
    /// Closed backend kind.
    pub const fn backend(&self) -> ParseBackendKind {
        self.backend
    }

    /// Resolved GROBID endpoint (canonical > alias > default).
    pub fn grobid_url(&self) -> &str {
        &self.grobid_url
    }

    /// Resolved cache mode (cost strategy — not part of content identity).
    pub const fn cache_mode(&self) -> CacheMode {
        self.cache_mode
    }

    /// Canonical parse artifact cache root.
    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    /// Maximum documents per admitted batch.
    pub const fn max_documents(&self) -> NonZeroUsize {
        self.max_documents
    }

    /// Worker concurrency bound.
    pub const fn workers(&self) -> NonZeroUsize {
        self.workers
    }

    /// Admission queue capacity bound.
    pub const fn queue_capacity(&self) -> NonZeroUsize {
        self.queue_capacity
    }

    /// Simultaneously executing documents (CLI-only bound; defaults to
    /// the worker count).
    pub const fn max_in_flight(&self) -> NonZeroUsize {
        self.max_in_flight
    }

    /// redb accelerator settings.
    pub const fn redb(&self) -> &ParseRedbIndexConfig {
        &self.redb
    }

    /// The batch limits these knobs project onto the provider-neutral API
    /// (bounds are excluded from content identity by contract).
    pub fn batch_limits(&self) -> ParseBatchLimits {
        ParseBatchLimits::new(
            self.workers,
            self.queue_capacity,
            self.max_in_flight,
            self.max_documents,
        )
    }
}

/// Environment accessor (injected for tests; production reads std::env).
pub type EnvLookup<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Resolve the closed composition config:
/// explicit CLI value > `DA_PARSE_*` env > composition-root default,
/// then run every closed validation rule. Fails before any backend is
/// opened — callers may treat `Ok` as "safe to open the backend".
pub fn resolve(
    overrides: &ParseRuntimeOverrides,
    env: EnvLookup<'_>,
) -> Result<ParseRuntimeConfig, ParseConfigError> {
    // Endpoint: canonical env > alias env > default, CLI above both.
    let canonical = read_env(env, DA_GROBID_URL_KEY)?;
    let alias = read_env(env, GROBID_URL_ALIAS_KEY)?;
    let env_url = match (&canonical, &alias) {
        (Some(canonical), Some(alias)) => {
            if canonical == alias {
                Some(canonical)
            } else {
                return Err(ParseConfigError::EndpointAliasConflict);
            }
        }
        (Some(canonical), None) => Some(canonical),
        (None, Some(alias)) => Some(alias),
        (None, None) => None,
    };
    let grobid_url = match override_string(overrides.grobid_url.as_deref(), "--grobid-url")? {
        Some(value) => value,
        None => match env_url {
            Some(url) => url.clone(),
            None => DEFAULT_GROBID_URL.to_string(),
        },
    };
    if !(grobid_url.starts_with("http://") || grobid_url.starts_with("https://")) {
        return Err(ParseConfigError::InvalidEndpoint);
    }

    // Backend: closed vocabulary.
    let backend = match override_string(overrides.backend.as_deref(), "--backend")? {
        Some(value) => ParseBackendKind::parse(&value).ok_or(ParseConfigError::UnknownValue {
            key: DA_PARSE_BACKEND_KEY,
            expected: "grobid-http",
        })?,
        None => match read_env(env, DA_PARSE_BACKEND_KEY)? {
            Some(value) => {
                ParseBackendKind::parse(&value).ok_or(ParseConfigError::UnknownValue {
                    key: DA_PARSE_BACKEND_KEY,
                    expected: "grobid-http",
                })?
            }
            None => ParseBackendKind::GrobidHttp,
        },
    };

    // Cache mode: closed four-mode vocabulary.
    let cache_mode = match override_string(overrides.cache_mode.as_deref(), "--mode")? {
        Some(value) => parse_cache_mode(&value).ok_or(ParseConfigError::UnknownValue {
            key: DA_PARSE_CACHE_MODE_KEY,
            expected: "off | read-only | write-through | refresh",
        })?,
        None => match read_env(env, DA_PARSE_CACHE_MODE_KEY)? {
            Some(value) => parse_cache_mode(&value).ok_or(ParseConfigError::UnknownValue {
                key: DA_PARSE_CACHE_MODE_KEY,
                expected: "off | read-only | write-through | refresh",
            })?,
            None => DEFAULT_CACHE_MODE,
        },
    };

    // Physical cache root.
    let cache_root = match override_path(overrides.cache_root.as_deref())? {
        Some(value) => value,
        None => match read_env(env, DA_PARSE_CACHE_ROOT_KEY)? {
            Some(value) => PathBuf::from(value),
            None => PathBuf::from(DEFAULT_CACHE_ROOT),
        },
    };

    // Numeric bounds (closed caps + zero rejection).
    let max_documents = resolve_bound(
        overrides.batch_size,
        DA_PARSE_BATCH_SIZE_KEY,
        env,
        DEFAULT_BATCH_DOCUMENTS,
        MAX_BATCH_DOCUMENTS,
    )?;
    let workers = resolve_bound(
        overrides.workers,
        DA_PARSE_WORKERS_KEY,
        env,
        DEFAULT_WORKERS,
        MAX_WORKERS,
    )?;
    let queue_capacity = resolve_bound(
        overrides.queue_capacity,
        DA_PARSE_QUEUE_CAPACITY_KEY,
        env,
        DEFAULT_QUEUE_CAPACITY,
        MAX_QUEUE_CAPACITY,
    )?;
    // max-in-flight is a CLI-only bound defaulting to the worker count.
    let max_in_flight = match overrides.max_in_flight {
        Some(value) => {
            let checked = check_bound(value, "--max-in-flight", MAX_IN_FLIGHT)?;
            NonZeroUsize::new(checked).expect("bound checked nonzero")
        }
        None => workers,
    };
    if max_in_flight.get() > queue_capacity.get() {
        return Err(ParseConfigError::InconsistentBounds {
            max_in_flight: max_in_flight.get(),
            queue_capacity: queue_capacity.get(),
        });
    }

    // redb accelerator: enabled flag + path (required iff enabled).
    let redb_enabled = match overrides.redb_enabled {
        Some(value) => value,
        None => match read_env(env, DA_PARSE_REDB_INDEX_ENABLED_KEY)? {
            Some(value) => match value.as_str() {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(ParseConfigError::UnknownValue {
                        key: DA_PARSE_REDB_INDEX_ENABLED_KEY,
                        expected: "true | false",
                    });
                }
            },
            None => false,
        },
    };
    let redb_path = match override_path(overrides.redb_path.as_deref())? {
        Some(path) => Some(path),
        None => read_env(env, DA_PARSE_REDB_INDEX_PATH_KEY)?.map(PathBuf::from),
    };
    let redb = match (redb_enabled, redb_path) {
        (true, Some(path)) => {
            let config = ParseRedbIndexConfig {
                enabled: true,
                path: Some(path),
            };
            validate_redb_placement(&config, &cache_root)?;
            config
        }
        (true, None) => return Err(ParseConfigError::MissingRedbPath),
        (false, path) => ParseRedbIndexConfig {
            enabled: false,
            path,
        },
    };

    Ok(ParseRuntimeConfig {
        backend,
        grobid_url,
        cache_mode,
        cache_root,
        max_documents,
        workers,
        queue_capacity,
        max_in_flight,
        redb,
    })
}

/// Production entry: resolve against the process environment.
pub fn resolve_from_process_env(
    overrides: &ParseRuntimeOverrides,
) -> Result<ParseRuntimeConfig, ParseConfigError> {
    resolve(overrides, &|key| std::env::var(key).ok())
}

// ---------------------------------------------------------------------------
// Resolution helpers
// ---------------------------------------------------------------------------

fn read_env(env: EnvLookup<'_>, key: &'static str) -> Result<Option<String>, ParseConfigError> {
    match env(key) {
        Some(value) if value.trim().is_empty() => Err(ParseConfigError::BlankValue { key }),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

fn override_string(
    value: Option<&str>,
    flag: &'static str,
) -> Result<Option<String>, ParseConfigError> {
    match value {
        Some(value) if value.trim().is_empty() => Err(ParseConfigError::BlankValue { key: flag }),
        Some(value) => Ok(Some(value.to_string())),
        None => Ok(None),
    }
}

fn override_path(value: Option<&Path>) -> Result<Option<PathBuf>, ParseConfigError> {
    match value {
        Some(path) if path.as_os_str().is_empty() => Err(ParseConfigError::BlankValue {
            key: "--redb-index-path",
        }),
        Some(path) => Ok(Some(path.to_path_buf())),
        None => Ok(None),
    }
}

fn check_bound(value: usize, key: &'static str, max: usize) -> Result<usize, ParseConfigError> {
    if value == 0 {
        return Err(ParseConfigError::ZeroForbidden { key });
    }
    if value > max {
        return Err(ParseConfigError::Overflow { key, max });
    }
    Ok(value)
}

fn resolve_bound(
    override_value: Option<usize>,
    key: &'static str,
    env: EnvLookup<'_>,
    default: usize,
    max: usize,
) -> Result<NonZeroUsize, ParseConfigError> {
    let raw = match override_value {
        Some(value) => check_bound(value, key, max)?,
        None => match read_env(env, key)? {
            Some(value) => {
                let parsed: usize = value
                    .parse()
                    .map_err(|_| ParseConfigError::InvalidNumber { key })?;
                check_bound(parsed, key, max)?
            }
            None => default,
        },
    };
    Ok(NonZeroUsize::new(raw).expect("bound checked nonzero"))
}

/// Lexical path normalization (no filesystem access): resolves `.` and
/// `..` components so overlap checks cannot be fooled by spelling.
fn normalize_path_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// The accelerator must never equal, live inside, or contain the canonical
/// cache root — it is an index, and canonical payload state must stay
/// addressable (and rebuildable) without it.
fn validate_redb_placement(
    redb: &ParseRedbIndexConfig,
    cache_root: &Path,
) -> Result<(), ParseConfigError> {
    let Some(path) = redb.path() else {
        return Err(ParseConfigError::MissingRedbPath);
    };
    let redb_file = normalize_path_lexical(path);
    let cache = normalize_path_lexical(cache_root);
    // Coincide: same path.
    if redb_file == cache {
        return Err(ParseConfigError::RedbPathOverlapsCacheRoot);
    }
    // Inside: the accelerator file lives inside the canonical cache tree.
    if redb_file.starts_with(&cache) {
        return Err(ParseConfigError::RedbPathOverlapsCacheRoot);
    }
    // Substitute: the cache root lexically lives under the accelerator
    // file path (the redb file would be an ancestor directory of canonical
    // payload state).
    if cache.starts_with(&redb_file) {
        return Err(ParseConfigError::RedbPathOverlapsCacheRoot);
    }
    Ok(())
}
