//! Composition-config contract suite for `da parse-batch` (R116,
//! M464-z84in2 S06/T04).
//!
//! Objective proofs required by the S06 plan:
//!
//! - precedence: explicit CLI value > `DA_PARSE_*` env > composition-root
//!   default, for every owned knob;
//! - closed validation BEFORE any backend open: blank values, unknown
//!   enum values, zero bounds, values over the composition-root caps,
//!   `GROBID_URL`/`DA_GROBID_URL` conflicts (fail closed — no silent
//!   pick), missing redb path while enabled, redb path overlapping the
//!   canonical cache root, and inconsistent max-in-flight/queue bounds
//!   are all typed errors;
//! - the legacy `GROBID_URL` keeps its meaning as an explicit, tested
//!   compatibility alias of the canonical `DA_GROBID_URL` (D204 identity
//!   untouched — the resolution only selects which value the unchanged
//!   backend identity is built from);
//! - the config projects NonZero batch limits; no physical default can
//!   leak into inner layers (enforced by da-parse architecture scans).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use da_cli::parse_config::{
    DA_GROBID_URL_KEY, DA_PARSE_BACKEND_KEY, DA_PARSE_BATCH_SIZE_KEY, DA_PARSE_CACHE_MODE_KEY,
    DA_PARSE_CACHE_ROOT_KEY, DA_PARSE_QUEUE_CAPACITY_KEY, DA_PARSE_REDB_INDEX_ENABLED_KEY,
    DA_PARSE_REDB_INDEX_PATH_KEY, DA_PARSE_WORKERS_KEY, DEFAULT_BATCH_DOCUMENTS,
    DEFAULT_CACHE_MODE, DEFAULT_CACHE_ROOT, DEFAULT_GROBID_URL, DEFAULT_QUEUE_CAPACITY,
    DEFAULT_WORKERS, GROBID_URL_ALIAS_KEY, MAX_BATCH_DOCUMENTS, MAX_IN_FLIGHT, MAX_QUEUE_CAPACITY,
    MAX_WORKERS, ParseBackendKind, ParseConfigError, ParseRuntimeOverrides, parse_cache_mode,
    resolve,
};
use da_parse::CacheMode;

// ───────────────────────── helpers ─────────────────────────

type Env = HashMap<String, String>;

fn env_of(pairs: &[(&str, &str)]) -> Env {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn resolve_with_env(
    overrides: &ParseRuntimeOverrides,
    env: &Env,
) -> Result<da_cli::parse_config::ParseRuntimeConfig, ParseConfigError> {
    resolve(overrides, &|key| env.get(key).cloned())
}

fn default_overrides() -> ParseRuntimeOverrides {
    ParseRuntimeOverrides::default()
}

// ───────────────────────── defaults ─────────────────────────

#[test]
fn composition_root_defaults_apply_without_env_or_overrides() {
    let config = resolve_with_env(&default_overrides(), &env_of(&[])).expect("defaults resolve");
    assert_eq!(config.backend(), ParseBackendKind::GrobidHttp);
    assert_eq!(config.grobid_url(), DEFAULT_GROBID_URL);
    assert_eq!(config.cache_mode(), DEFAULT_CACHE_MODE);
    assert_eq!(config.cache_mode(), CacheMode::WriteThrough);
    assert_eq!(config.cache_root(), Path::new(DEFAULT_CACHE_ROOT));
    assert_eq!(config.max_documents().get(), DEFAULT_BATCH_DOCUMENTS);
    assert_eq!(config.workers().get(), DEFAULT_WORKERS);
    assert_eq!(config.queue_capacity().get(), DEFAULT_QUEUE_CAPACITY);
    assert_eq!(config.max_in_flight().get(), DEFAULT_WORKERS);
    assert!(!config.redb().enabled());
    assert!(config.redb().path().is_none());
    // The config projects NonZero batch limits (bounds are excluded from
    // content identity by the frozen batch contract).
    let limits = config.batch_limits();
    assert_eq!(limits.schema_version(), 1);
    assert_eq!(limits.workers().get(), DEFAULT_WORKERS);
    assert_eq!(limits.queue_capacity().get(), DEFAULT_QUEUE_CAPACITY);
    assert_eq!(limits.max_in_flight().get(), DEFAULT_WORKERS);
    assert_eq!(limits.max_documents().get(), DEFAULT_BATCH_DOCUMENTS);
}

// ───────────────────────── precedence ─────────────────────────

#[test]
fn explicit_cli_value_beats_env_which_beats_default() {
    let env = env_of(&[
        (DA_GROBID_URL_KEY, "http://env-endpoint:9000"),
        (DA_PARSE_CACHE_MODE_KEY, "read-only"),
        (DA_PARSE_WORKERS_KEY, "7"),
        (DA_PARSE_CACHE_ROOT_KEY, "env/cache/root"),
    ]);
    let overrides = ParseRuntimeOverrides {
        grobid_url: None,
        cache_mode: Some("refresh".into()),
        workers: Some(9),
        ..ParseRuntimeOverrides::default()
    };
    let config = resolve_with_env(&overrides, &env).expect("resolve");
    // CLI beats env…
    assert_eq!(config.cache_mode(), CacheMode::Refresh);
    assert_eq!(config.workers().get(), 9);
    // …env beats default…
    assert_eq!(config.grobid_url(), "http://env-endpoint:9000");
    assert_eq!(config.cache_root(), Path::new("env/cache/root"));
    // …and unset knobs fall through to the default.
    assert_eq!(config.queue_capacity().get(), DEFAULT_QUEUE_CAPACITY);
}

#[test]
fn every_owned_env_key_applies_without_cli_overrides() {
    let env = env_of(&[
        (DA_GROBID_URL_KEY, "http://canonical:8070"),
        (DA_PARSE_BACKEND_KEY, "grobid-http"),
        (DA_PARSE_CACHE_MODE_KEY, "off"),
        (DA_PARSE_CACHE_ROOT_KEY, "ops/parse-cache"),
        (DA_PARSE_BATCH_SIZE_KEY, "17"),
        (DA_PARSE_WORKERS_KEY, "3"),
        (DA_PARSE_QUEUE_CAPACITY_KEY, "12"),
        (DA_PARSE_REDB_INDEX_ENABLED_KEY, "false"),
    ]);
    let config = resolve_with_env(&default_overrides(), &env).expect("resolve");
    assert_eq!(config.grobid_url(), "http://canonical:8070");
    assert_eq!(config.backend(), ParseBackendKind::GrobidHttp);
    assert_eq!(config.cache_mode(), CacheMode::Off);
    assert_eq!(config.cache_root(), Path::new("ops/parse-cache"));
    assert_eq!(config.max_documents().get(), 17);
    assert_eq!(config.workers().get(), 3);
    assert_eq!(config.queue_capacity().get(), 12);
    assert!(!config.redb().enabled());
}

// ───────────────────────── GROBID_URL compatibility alias ─────────────────────────

#[test]
fn legacy_grobid_url_alias_alone_resolves_like_the_canonical_key() {
    let config = resolve_with_env(
        &default_overrides(),
        &env_of(&[(GROBID_URL_ALIAS_KEY, "http://legacy-host:8070")]),
    )
    .expect("alias resolves");
    assert_eq!(config.grobid_url(), "http://legacy-host:8070");
}

#[test]
fn legacy_alias_and_canonical_agreeing_resolve_cleanly() {
    let config = resolve_with_env(
        &default_overrides(),
        &env_of(&[
            (DA_GROBID_URL_KEY, "http://same:8070"),
            (GROBID_URL_ALIAS_KEY, "http://same:8070"),
        ]),
    )
    .expect("agreeing keys resolve");
    assert_eq!(config.grobid_url(), "http://same:8070");
}

#[test]
fn conflicting_alias_values_fail_closed() {
    let error = resolve_with_env(
        &default_overrides(),
        &env_of(&[
            (DA_GROBID_URL_KEY, "http://canonical:8070"),
            (GROBID_URL_ALIAS_KEY, "http://different:9000"),
        ]),
    )
    .expect_err("conflict must fail closed");
    assert_eq!(error, ParseConfigError::EndpointAliasConflict);
    // The canonical key wins over the alias when only the alias conflicts
    // with a CLI value: the CLI value is an explicit selection.
    let config = resolve_with_env(
        &ParseRuntimeOverrides {
            grobid_url: Some("http://cli:8070".into()),
            ..ParseRuntimeOverrides::default()
        },
        &env_of(&[(GROBID_URL_ALIAS_KEY, "http://legacy:8070")]),
    )
    .expect("explicit CLI beats the alias");
    assert_eq!(config.grobid_url(), "http://cli:8070");
}

// ───────────────────────── blank values fail closed ─────────────────────────

#[test]
fn blank_env_values_are_errors_not_unsets() {
    for key in [
        DA_GROBID_URL_KEY,
        DA_PARSE_CACHE_MODE_KEY,
        DA_PARSE_CACHE_ROOT_KEY,
        DA_PARSE_BATCH_SIZE_KEY,
        DA_PARSE_WORKERS_KEY,
        DA_PARSE_QUEUE_CAPACITY_KEY,
        DA_PARSE_REDB_INDEX_ENABLED_KEY,
    ] {
        let error =
            resolve_with_env(&default_overrides(), &env_of(&[(key, "   ")])).expect_err(key);
        assert_eq!(error, ParseConfigError::BlankValue { key }, "{key}");
    }
}

#[test]
fn blank_cli_values_are_errors_too() {
    let overrides = ParseRuntimeOverrides {
        cache_mode: Some(" ".into()),
        ..ParseRuntimeOverrides::default()
    };
    let error = resolve_with_env(&overrides, &env_of(&[])).expect_err("blank CLI value");
    assert_eq!(error, ParseConfigError::BlankValue { key: "--mode" });
}

// ───────────────────────── unknown values fail closed ─────────────────────────

#[test]
fn unknown_cache_mode_backend_and_flag_values_are_typed_errors() {
    let error = resolve_with_env(
        &default_overrides(),
        &env_of(&[(DA_PARSE_CACHE_MODE_KEY, "rewind")]),
    )
    .expect_err("unknown mode");
    assert_eq!(
        error,
        ParseConfigError::UnknownValue {
            key: DA_PARSE_CACHE_MODE_KEY,
            expected: "off | read-only | write-through | refresh",
        }
    );

    let error = resolve_with_env(
        &default_overrides(),
        &env_of(&[(DA_PARSE_BACKEND_KEY, "cached-tei")]),
    )
    .expect_err("unknown backend");
    assert_eq!(
        error,
        ParseConfigError::UnknownValue {
            key: DA_PARSE_BACKEND_KEY,
            expected: "grobid-http",
        }
    );

    let error = resolve_with_env(
        &default_overrides(),
        &env_of(&[(DA_PARSE_REDB_INDEX_ENABLED_KEY, "yes")]),
    )
    .expect_err("unknown flag token");
    assert_eq!(
        error,
        ParseConfigError::UnknownValue {
            key: DA_PARSE_REDB_INDEX_ENABLED_KEY,
            expected: "true | false",
        }
    );
}

#[test]
fn closed_vocabularies_parse_exactly_the_documented_tokens() {
    assert_eq!(parse_cache_mode("off"), Some(CacheMode::Off));
    assert_eq!(parse_cache_mode("read-only"), Some(CacheMode::ReadOnly));
    assert_eq!(
        parse_cache_mode("write-through"),
        Some(CacheMode::WriteThrough)
    );
    assert_eq!(parse_cache_mode("refresh"), Some(CacheMode::Refresh));
    assert_eq!(parse_cache_mode(""), None);
    assert_eq!(parse_cache_mode("OFF"), None);
    assert_eq!(
        ParseBackendKind::parse("grobid-http"),
        Some(ParseBackendKind::GrobidHttp)
    );
    assert_eq!(ParseBackendKind::parse("cached-tei"), None);
    assert_eq!(ParseBackendKind::GrobidHttp.as_str(), "grobid-http");
}

// ───────────────────────── zero / overflow / numeric bounds ─────────────────────────

#[test]
fn zero_bounds_are_rejected() {
    for (key, value) in [
        (DA_PARSE_BATCH_SIZE_KEY, 0usize),
        (DA_PARSE_WORKERS_KEY, 0),
        (DA_PARSE_QUEUE_CAPACITY_KEY, 0),
    ] {
        let _ = value;
        let env = env_of(&[(key, "0")]);
        let error = resolve_with_env(&default_overrides(), &env).expect_err(key);
        assert_eq!(error, ParseConfigError::ZeroForbidden { key }, "{key}");
    }
    let error = resolve_with_env(
        &ParseRuntimeOverrides {
            max_in_flight: Some(0),
            ..ParseRuntimeOverrides::default()
        },
        &env_of(&[]),
    )
    .expect_err("zero max-in-flight");
    assert_eq!(
        error,
        ParseConfigError::ZeroForbidden {
            key: "--max-in-flight",
        }
    );
}

#[test]
fn bounds_over_the_composition_root_caps_are_rejected() {
    let cases = [
        (DA_PARSE_WORKERS_KEY, MAX_WORKERS + 1, MAX_WORKERS),
        (
            DA_PARSE_QUEUE_CAPACITY_KEY,
            MAX_QUEUE_CAPACITY + 1,
            MAX_QUEUE_CAPACITY,
        ),
        (
            DA_PARSE_BATCH_SIZE_KEY,
            MAX_BATCH_DOCUMENTS + 1,
            MAX_BATCH_DOCUMENTS,
        ),
    ];
    for (key, value, max) in cases {
        let env = env_of(&[(key, value.to_string().as_str())]);
        let error = resolve_with_env(&default_overrides(), &env).expect_err(key);
        assert_eq!(error, ParseConfigError::Overflow { key, max }, "{key}");
    }
    let error = resolve_with_env(
        &ParseRuntimeOverrides {
            max_in_flight: Some(MAX_IN_FLIGHT + 1),
            ..ParseRuntimeOverrides::default()
        },
        &env_of(&[]),
    )
    .expect_err("overflow max-in-flight");
    assert_eq!(
        error,
        ParseConfigError::Overflow {
            key: "--max-in-flight",
            max: MAX_IN_FLIGHT,
        }
    );
}

#[test]
fn non_numeric_bound_values_are_typed_errors() {
    for value in ["x", "3.5", "-2", "99999999999999999999"] {
        let env = env_of(&[(DA_PARSE_WORKERS_KEY, value)]);
        let error = resolve_with_env(&default_overrides(), &env).expect_err(value);
        assert_eq!(
            error,
            ParseConfigError::InvalidNumber {
                key: DA_PARSE_WORKERS_KEY,
            },
            "{value}"
        );
    }
}

#[test]
fn max_in_flight_may_not_exceed_queue_capacity() {
    let error = resolve_with_env(
        &ParseRuntimeOverrides {
            max_in_flight: Some(8),
            queue_capacity: Some(4),
            ..ParseRuntimeOverrides::default()
        },
        &env_of(&[]),
    )
    .expect_err("inconsistent bounds");
    assert_eq!(
        error,
        ParseConfigError::InconsistentBounds {
            max_in_flight: 8,
            queue_capacity: 4,
        }
    );
    // Equal bounds are consistent.
    let config = resolve_with_env(
        &ParseRuntimeOverrides {
            max_in_flight: Some(8),
            queue_capacity: Some(8),
            ..ParseRuntimeOverrides::default()
        },
        &env_of(&[]),
    )
    .expect("equal bounds");
    assert_eq!(config.max_in_flight().get(), 8);
}

// ───────────────────────── endpoint scheme ─────────────────────────

#[test]
fn endpoint_must_be_an_http_url() {
    let error = resolve_with_env(
        &default_overrides(),
        &env_of(&[(DA_GROBID_URL_KEY, "not-a-url")]),
    )
    .expect_err("non-http endpoint");
    assert_eq!(error, ParseConfigError::InvalidEndpoint);
}

// ───────────────────────── redb accelerator settings ─────────────────────────

#[test]
fn disabled_redb_mode_requires_no_path() {
    let config = resolve_with_env(&default_overrides(), &env_of(&[])).expect("disabled default");
    assert!(!config.redb().enabled());
    assert!(config.redb().path().is_none());
}

#[test]
fn enabled_redb_requires_a_path() {
    let error = resolve_with_env(
        &default_overrides(),
        &env_of(&[(DA_PARSE_REDB_INDEX_ENABLED_KEY, "true")]),
    )
    .expect_err("enabled without path");
    assert_eq!(error, ParseConfigError::MissingRedbPath);
    let config = resolve_with_env(
        &default_overrides(),
        &env_of(&[
            (DA_PARSE_REDB_INDEX_ENABLED_KEY, "true"),
            (DA_PARSE_REDB_INDEX_PATH_KEY, "ops/redb/accel.redb"),
        ]),
    )
    .expect("enabled with path");
    assert!(config.redb().enabled());
    assert_eq!(config.redb().path(), Some(Path::new("ops/redb/accel.redb")));
}

#[test]
fn redb_path_may_not_coincide_overlap_or_substitute_the_cache_root() {
    let base = |redb: &str, cache: &str| {
        resolve_with_env(
            &default_overrides(),
            &env_of(&[
                (DA_PARSE_REDB_INDEX_ENABLED_KEY, "true"),
                (DA_PARSE_REDB_INDEX_PATH_KEY, redb),
                (DA_PARSE_CACHE_ROOT_KEY, cache),
            ]),
        )
    };
    // Coincide.
    assert_eq!(
        base("data/parse-cache", "data/parse-cache").expect_err("equal paths"),
        ParseConfigError::RedbPathOverlapsCacheRoot
    );
    // Accelerator inside the cache tree.
    assert_eq!(
        base("data/parse-cache/accel.redb", "data/parse-cache").expect_err("inside cache"),
        ParseConfigError::RedbPathOverlapsCacheRoot
    );
    // Cache root lexically under the accelerator file path (substitution).
    assert_eq!(
        base("data/accel", "data/accel/cache").expect_err("cache under redb"),
        ParseConfigError::RedbPathOverlapsCacheRoot
    );
    // `..` spelling cannot fool the overlap checks: normalization makes
    // an escape INTO the cache tree an error…
    assert_eq!(
        base("data/../data/parse-cache/accel.redb", "data/parse-cache")
            .expect_err("dot-dot into cache"),
        ParseConfigError::RedbPathOverlapsCacheRoot
    );
    // …while an escape OUT to a sibling is a legitimate layout.
    assert!(
        base("data/parse-cache/../accel.redb", "data/parse-cache").is_ok(),
        "normalized sibling is not an overlap"
    );
    // Siblings are fine.
    let config = base("data/accel.redb", "data/parse-cache").expect("siblings");
    assert!(config.redb().enabled());
}

// ───────────────────────── error hygiene ─────────────────────────

#[test]
fn error_messages_name_keys_and_rules_never_values_locations() {
    let cases = [
        ParseConfigError::BlankValue {
            key: DA_PARSE_WORKERS_KEY,
        },
        ParseConfigError::UnknownValue {
            key: DA_PARSE_CACHE_MODE_KEY,
            expected: "off | read-only | write-through | refresh",
        },
        ParseConfigError::InvalidNumber {
            key: DA_PARSE_WORKERS_KEY,
        },
        ParseConfigError::ZeroForbidden {
            key: DA_PARSE_WORKERS_KEY,
        },
        ParseConfigError::Overflow {
            key: DA_PARSE_WORKERS_KEY,
            max: MAX_WORKERS,
        },
        ParseConfigError::EndpointAliasConflict,
        ParseConfigError::InvalidEndpoint,
        ParseConfigError::MissingRedbPath,
        ParseConfigError::RedbPathOverlapsCacheRoot,
        ParseConfigError::InconsistentBounds {
            max_in_flight: 8,
            queue_capacity: 4,
        },
    ];
    for error in cases {
        let message = error.to_string();
        if !matches!(error, ParseConfigError::InvalidEndpoint) {
            // Only the URL-scheme RULE names the scheme; no error text
            // ever echoes an actual endpoint value.
            assert!(!message.contains("http"), "{message}");
        }
        assert!(!message.contains("://"), "{message}");
        assert!(!message.contains(".redb"), "{message}");
        assert!(!message.contains('/'), "{message}");
    }
}

// ───────────────────────── CLI-path sanity (PathBuf override surface) ─────────────────────────

#[test]
fn pathbuf_overrides_and_u64_style_inputs_resolve() {
    let overrides = ParseRuntimeOverrides {
        cache_root: Some(PathBuf::from("cli/cache")),
        redb_enabled: Some(true),
        redb_path: Some(PathBuf::from("cli/redb/accel.redb")),
        batch_size: Some(5),
        workers: Some(2),
        queue_capacity: Some(4),
        max_in_flight: Some(2),
        ..ParseRuntimeOverrides::default()
    };
    let config = resolve_with_env(&overrides, &env_of(&[])).expect("resolve");
    assert_eq!(config.cache_root(), Path::new("cli/cache"));
    assert_eq!(config.redb().path(), Some(Path::new("cli/redb/accel.redb")));
    assert_eq!(config.max_documents().get(), 5);
    assert_eq!(config.workers().get(), 2);
    assert_eq!(config.queue_capacity().get(), 4);
    assert_eq!(config.max_in_flight().get(), 2);
}
