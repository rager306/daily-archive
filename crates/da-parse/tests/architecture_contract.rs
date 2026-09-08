//! Static architecture gate for the parse plane (M464-z84in2 / S02 / T04).
//!
//! Reads the Cargo manifests and source surfaces directly (deterministic,
//! lock-free — no `cargo metadata` subprocess that could race the build
//! lock) and fails when:
//!
//! - da-parse grows a dependency on Samyama, any `kg-*`, SONA, ruVLLM, RVF,
//!   redb, a graph-store crate, or sibling `da-{application,domain,adapters,
//!   cli}` — the parse plane stays a pure leaf behind `da-ports` (ADR-075
//!   onion; da-parse must never reach application/domain or storage);
//! - `da-domain` / `da-application` invert onto `da-parse` (parse is a leaf;
//!   they consume `ParserPort` via `da-ports`, never the crate);
//! - `da-domain` / `da-application` grow parse physical-path or
//!   configuration defaults (`DA_PARSE_*` keys, GROBID URL/loopback-8070
//!   defaults, `data/parse` or parse-fixture path literals) — configuration
//!   ownership stays at the da-cli composition root (R116); the ledger
//!   records those key NAMES only.
//!
//! Since S04/T04 the gate additionally proves the typed request/failure
//! surfaces (`transport.rs`, `backend.rs`, `profile.rs`) stay free of
//! `PathBuf`/env/cache-root/URL-default material in CODE (comments may
//! document the prohibitions), and that every typed failure rendering
//! (Display / Debug / serde wire form) carries zero payload or path material.
//!
//! Since S05/T03 the gate additionally proves the closed endpoint routing:
//! the exact Java GROBID process routes are sanctioned ONLY inside the
//! static `ParseEndpoint` mapping (any other da-parse source dispatching a
//! route literal is free-form dispatch), the static routes carry no physical
//! defaults (scheme/host/port), and the registry of record keeps the D127
//! import lock, `auto_promote: forbidden`, the `da-parse` candidate identity
//! and exactly the five sanctioned proxy capabilities. The dependency ban
//! now also covers the JVM family (jni/jvm) on top of
//! Samyama/kg-*/SONA/ruVLLM/RVF/redb.
//!
//! Since S06/T01 the gate additionally scans the frozen bounded-batch and
//! cache contract modules (`batch.rs`, `cache.rs`) with the same
//! location/tuning hygiene rules and, on top, proves they grow no retry
//! surface (no retry/backoff/attempt material in code): the batch contract
//! stays an exactly-once, no-hidden-retry vocabulary (R107).
//!
//! Since S06/T05 the gate additionally proves the redb accelerator stays
//! FEATURE-GATED inside the two infrastructure crates only (optional
//! dependency in da-adapters, forwarding feature in da-cli; D295 role
//! `rebuildable_only`): no other workspace crate — the parse plane, the
//! domain/application cores and the kg-* universals included — may acquire
//! redb as a dependency or as a feature.
//!
//! The scanners' negative behavior is unit-tested in-file on synthetic
//! inputs, so the gate itself is fail-visible rather than fail-silent.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const DA_PARSE_MANIFEST: &str = "crates/da-parse/Cargo.toml";
const DOWNSTREAM_MANIFESTS: [&str; 2] = [
    "crates/da-domain/Cargo.toml",
    "crates/da-application/Cargo.toml",
];
const DA_PARSE_SOURCE_DIRS: [&str; 2] = ["crates/da-parse/src", "crates/da-parse/tests"];
const DOWNSTREAM_SOURCE_DIRS: [&str; 2] = ["crates/da-domain/src", "crates/da-application/src"];

/// Dependency names da-parse must never acquire. `kg-*` and `rvf-*` prefixes
/// are rejected in addition to these exact names.
const FORBIDDEN_DEP_NAMES: [&str; 18] = [
    "samyama",
    "kg-ontology",
    "kg-storage",
    "kg-embeddings",
    "kg-algorithms",
    "kg-pipeline",
    "sona",
    "ruvllm",
    "ruvector",
    "rvf",
    "redb",
    "jni",
    "jvm",
    "graph-store",
    "da-application",
    "da-domain",
    "da-adapters",
    "da-cli",
];

/// `use`/`extern crate` roots da-parse sources must never import. Crate
/// names use underscores in code paths.
const FORBIDDEN_USE_ROOTS: [&str; 14] = [
    "samyama",
    "kg_ontology",
    "kg_storage",
    "kg_embeddings",
    "kg_algorithms",
    "kg_pipeline",
    "redb",
    "sona",
    "ruvllm",
    "ruvector",
    "da_application",
    "da_domain",
    "da_adapters",
    "da_cli",
];

/// Physical-path / config default markers that must stay out of
/// da-domain/da-application sources (they are documented as da-cli-owned in
/// data/parse/grobid_compatibility.yaml `configuration_keys_documented_not_owned`).
const PARSE_DEFAULT_MARKERS: [&str; 6] = [
    "DA_PARSE_",
    "DA_GROBID_URL",
    "GROBID_URL",
    "127.0.0.1:8070",
    "data/parse",
    "crates/da-parse/tests",
];

// ───────────────────────────── scanners ───────────────────────────────

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn read_text(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("tracked file unreadable: {rel}: {e}"))
}

/// Extract `(section, crate_name)` pairs from Cargo.toml text. Dependency
/// tables are any `[...dependencies]` section; the key before `=` is the
/// crate name (works for plain, workspace-inherited and inline-table forms).
fn manifest_dependency_names(manifest_text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut section = String::new();
    for raw in manifest_text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !section.ends_with("dependencies") {
            continue;
        }
        if let Some((name, _rest)) = line.split_once('=') {
            let name = name.trim();
            if !name.is_empty() {
                out.push((section.clone(), name.to_string()));
            }
        }
    }
    out
}

fn forbidden_dependency_hits(names: &[(String, String)]) -> Vec<String> {
    let mut hits = Vec::new();
    for (section, name) in names {
        let forbidden = FORBIDDEN_DEP_NAMES.iter().any(|f| name == f)
            || name.starts_with("kg-")
            || name.starts_with("rvf-");
        if forbidden {
            hits.push(format!(
                "[forbidden-parse-dependency] `{name}` in [{section}]"
            ));
        }
    }
    hits
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Flag any `use <root>::` / `extern crate <root>` whose crate root is on
/// the forbidden list. Doc comments and prose never match: only import
/// positions are inspected.
fn forbidden_use_hits(text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.trim_start();
        let root = if let Some(rest) = line.strip_prefix("use ") {
            rest.split([':', ' ', '{', ';'])
                .find(|t| !t.is_empty())
                .unwrap_or("")
        } else if let Some(rest) = line.strip_prefix("extern crate ") {
            rest.split([';', ' ']).find(|t| !t.is_empty()).unwrap_or("")
        } else {
            continue;
        };
        if FORBIDDEN_USE_ROOTS.contains(&root) {
            hits.push(format!(
                "[forbidden-parse-import] line {}: `{}`",
                line_no + 1,
                raw.trim()
            ));
        }
    }
    hits
}

/// Flag parse physical-path / config default markers anywhere in the file.
fn parse_default_hits(text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in text.lines().enumerate() {
        for marker in PARSE_DEFAULT_MARKERS {
            if raw.contains(marker) {
                hits.push(format!(
                    "[parse-default-in-downstream] line {}: marker `{marker}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    hits
}

// ─────────────────────────────── tests ────────────────────────────────

#[test]
fn da_parse_manifest_declares_no_forbidden_dependencies() {
    let manifest = read_text(DA_PARSE_MANIFEST);
    let names = manifest_dependency_names(&manifest);

    // Positive controls: the scanner actually read the dependency tables.
    assert!(
        names.iter().any(|(_, n)| n == "da-ports"),
        "positive control failed: da-ports not seen — scanner or manifest drifted"
    );
    assert!(
        names.iter().any(|(_, n)| n == "reqwest"),
        "positive control failed: reqwest not seen — scanner or manifest drifted"
    );

    let hits = forbidden_dependency_hits(&names);
    assert!(
        hits.is_empty(),
        "da-parse grew a forbidden dependency (parse plane must stay a pure leaf behind da-ports):\n{}",
        hits.join("\n")
    );
}

#[test]
fn da_parse_sources_import_no_forbidden_crates() {
    let mut files = Vec::new();
    for dir in DA_PARSE_SOURCE_DIRS {
        collect_rs_files(&repo_root().join(dir), &mut files);
    }
    assert!(
        files.len() >= 10,
        "positive control failed: only {} source files scanned",
        files.len()
    );

    let mut hits = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("source unreadable {}: {e}", file.display()));
        hits.extend(forbidden_use_hits(&text));
    }
    assert!(
        hits.is_empty(),
        "da-parse sources import a forbidden crate:\n{}",
        hits.join("\n")
    );
}

#[test]
fn domain_and_application_stay_free_of_parse_defaults() {
    let mut files = Vec::new();
    for dir in DOWNSTREAM_SOURCE_DIRS {
        collect_rs_files(&repo_root().join(dir), &mut files);
    }
    assert!(
        files.len() >= 5,
        "positive control failed: only {} downstream source files scanned",
        files.len()
    );

    let mut hits = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("source unreadable {}: {e}", file.display()));
        hits.extend(parse_default_hits(&text));
    }
    assert!(
        hits.is_empty(),
        "parse physical-path/config defaults leaked into da-domain/da-application (ownership stays at the da-cli composition root, R116):\n{}",
        hits.join("\n")
    );
}

#[test]
fn domain_and_application_do_not_invert_onto_da_parse() {
    for rel in DOWNSTREAM_MANIFESTS {
        let names = manifest_dependency_names(&read_text(rel));
        let hits: Vec<String> = names
            .iter()
            .filter(|(_, name)| name == "da-parse")
            .map(|(section, name)| format!("[parse-inversion] `{name}` in [{section}] of {rel}"))
            .collect();
        assert!(
            hits.is_empty(),
            "downstream crates must consume ParserPort via da-ports, never da-parse:\n{}",
            hits.join("\n")
        );
    }
}

// ─────────────────────── scanner negative tests ───────────────────────

#[test]
fn dependency_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "\
[dependencies]
da-ports = { workspace = true }
samyama = \"0.1\"
kg-storage = { workspace = true }
jvm = \"0.1\"

[dev-dependencies]
da-application = { path = \"../da-application\" }
";
    let names = manifest_dependency_names(synthetic);
    assert_eq!(names.len(), 5, "scanner must read every dependency table");
    let hits = forbidden_dependency_hits(&names);
    assert_eq!(hits.len(), 4, "expected four forbidden hits, got {hits:?}");
    assert!(hits.iter().any(|h| h.contains("`samyama`")));
    assert!(hits.iter().any(|h| h.contains("`kg-storage`")));
    assert!(hits.iter().any(|h| h.contains("`da-application`")));
    assert!(hits.iter().any(|h| h.contains("`jvm`")));

    // Prefix families are rejected even when not enumerated exactly.
    let prefixed =
        manifest_dependency_names("[dependencies]\nkg-invented = \"1\"\nrvf-experience = \"1\"\n");
    let hits = forbidden_dependency_hits(&prefixed);
    assert_eq!(
        hits.len(),
        2,
        "kg-*/rvf-* prefixes must be caught: {hits:?}"
    );
}

#[test]
fn import_scanner_fails_visible_on_synthetic_violations() {
    // Built via format! so the raw source lines of THIS file never start
    // with a forbidden import — the gate scans its own sources too.
    let synthetic = format!(
        "use std::collections::HashMap;\nuse da_ports::parser::ParsedArticle;\nuse {}::GraphStore;\nuse {}::Paper;\nextern crate {};\n",
        "samyama", "da_domain", "redb"
    );
    let hits = forbidden_use_hits(&synthetic);
    assert_eq!(hits.len(), 3, "expected three import hits, got {hits:?}");
    assert!(hits.iter().any(|h| h.contains("samyama")));
    assert!(hits.iter().any(|h| h.contains("da_domain")));
    assert!(hits.iter().any(|h| h.contains("redb")));
    // Doc prose mentioning the crates must NOT be flagged.
    assert!(
        forbidden_use_hits("// HTML parse stays in da-adapters; see kg-storage docs.\n").is_empty()
    );
}

#[test]
fn default_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "\
// R116 composition-root defaults
const CACHE_ROOT: &str = \"DA_PARSE_CACHE_ROOT\";
let url = \"http://127.0.0.1:8070\";
let manifest = \"data/parse/regression_cases.yaml\";
let fixtures = \"crates/da-parse/tests/fixtures/preserved/x.tei.xml\";
";
    let hits = parse_default_hits(synthetic);
    assert_eq!(hits.len(), 4, "expected four default hits, got {hits:?}");
    assert!(hits.iter().any(|h| h.contains("DA_PARSE_")));
    assert!(hits.iter().any(|h| h.contains("127.0.0.1:8070")));
    assert!(hits.iter().any(|h| h.contains("data/parse")));
    // Unrelated content must stay clean.
    assert!(parse_default_hits("let stage = \"parse.stage.lift_tei\";\n").is_empty());
}

// ───────── S04/T04: request/failure surface hygiene ─────────

/// Typed request/failure/batch/cache vocabulary surfaces: the provider-
/// neutral protocol layer, the legacy compatibility projection and, since
/// S06/T01, the frozen bounded-batch and cache contract modules. `crates/
/// da-parse/src/http.rs` is deliberately NOT scanned here — the facade
/// shell owns the sanctioned env/URL resolution documented in the ledger
/// (GROBID_URL).
const TYPED_VOCABULARY_SURFACES: [&str; 5] = [
    "crates/da-parse/src/transport.rs",
    "crates/da-parse/src/backend.rs",
    "crates/da-parse/src/profile.rs",
    "crates/da-parse/src/batch.rs",
    "crates/da-parse/src/cache.rs",
];

/// Markers that must never appear in CODE of the request/failure surfaces.
/// Line comments are stripped first: the modules' doc prose intentionally
/// documents these prohibitions ("No PathBuf, no env values, no cache
/// roots"), and prose must not trip the gate.
const SURFACE_FORBIDDEN_MARKERS: [&str; 8] = [
    "PathBuf",
    "std::env",
    "env::var",
    "var_os",
    "GROBID_URL",
    "127.0.0.1:8070",
    "cache_root",
    "CACHE_ROOT",
];

fn strip_line_comments(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.trim_start().starts_with("//") {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn surface_marker_hits(text: &str) -> Vec<String> {
    surface_marker_hits_for(text, &SURFACE_FORBIDDEN_MARKERS)
}

fn surface_marker_hits_for(text: &str, markers: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        for marker in markers {
            if raw.contains(marker) {
                hits.push(format!(
                    "[request-surface-leak] line {}: marker `{marker}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    hits
}

#[test]
fn typed_vocabulary_surfaces_stay_free_of_paths_env_and_cache_roots() {
    for rel in TYPED_VOCABULARY_SURFACES {
        let text = read_text(rel);
        assert!(
            text.len() > 1000,
            "positive control failed: {rel} looks truncated"
        );
        let hits = surface_marker_hits(&text);
        assert!(
            hits.is_empty(),
            "request/failure surface {rel} leaked path/env/cache-root/URL material (physical locations must never become part of the typed vocabulary):\n{}",
            hits.join("\n")
        );
    }
}

#[test]
fn surface_scanner_fails_visible_on_synthetic_violations() {
    // Built via format! so the literals of THIS file stay clean.
    let synthetic = format!(
        "// doc prose mentioning {} must not trigger\nlet cache: {} = todo!();\nlet url = \"{}\";\nlet key = {}::var(\"X\");\n",
        "DA_PARSE_CACHE_ROOT", "std::path::PathBuf", "http://127.0.0.1:8070", "std::env"
    );
    let hits = surface_marker_hits(&synthetic);
    assert_eq!(hits.len(), 4, "expected four code hits, got {hits:?}");
    assert!(hits.iter().any(|h| h.contains("PathBuf")));
    assert!(hits.iter().any(|h| h.contains("127.0.0.1:8070")));
    assert!(hits.iter().any(|h| h.contains("std::env")));
    assert!(hits.iter().any(|h| h.contains("env::var")));
    // Comment-only mentions must stay clean.
    assert!(
        surface_marker_hits("// GROBID_URL stays in the facade shell.\n").is_empty(),
        "comment prose must not trip the scanner"
    );
}

#[test]
fn failure_renderings_never_carry_payload_or_path_material() {
    use da_parse::{FailurePhase, LimitKind, ParseBackendFailure, RetryAfter};

    let payload_markers = ["%PDF", "<TEI>", "paper.pdf", "..", "/", "\\"];
    let failures = [
        ParseBackendFailure::busy(503, Some(RetryAfter::Seconds(30))),
        ParseBackendFailure::busy(503, Some(RetryAfter::Unparsable)),
        ParseBackendFailure::timeout(FailurePhase::AwaitResponse),
        ParseBackendFailure::transport(FailurePhase::Connect),
        ParseBackendFailure::no_content(),
        ParseBackendFailure::limit(LimitKind::RequestBytes, Some(413)),
        ParseBackendFailure::limit(LimitKind::ResponseBytes, None),
        ParseBackendFailure::invalid_request(400),
        ParseBackendFailure::conversion(FailurePhase::ReadBody, Some(500)),
        ParseBackendFailure::invalid_response(FailurePhase::AwaitResponse, Some(418)),
    ];
    for failure in &failures {
        let renderings = [
            failure.to_string(),
            format!("{failure:?}"),
            serde_json::to_string(failure).expect("typed failure serializes"),
        ];
        for rendering in renderings {
            for marker in payload_markers {
                assert!(
                    !rendering.contains(marker),
                    "typed failure rendering leaked `{marker}`: {rendering}"
                );
            }
        }
    }
}

// ───────── S06/T01: batch/cache contract surface hygiene ─────────

/// The frozen bounded-batch and cache contract modules. Beyond the
/// location/tuning markers of the typed surfaces above, these two modules
/// must never grow a RETRY surface in code (no retry decisions, backoff or
/// attempt counters): availability semantics stay with the backend (an
/// honest typed Busy, exactly-once per document), and the batch contract
/// has no field that could even express a retry policy (R107).
const BATCH_CACHE_SURFACES: [&str; 2] = [
    "crates/da-parse/src/batch.rs",
    "crates/da-parse/src/cache.rs",
];

const BATCH_CACHE_FORBIDDEN_MARKERS: [&str; 10] = [
    "PathBuf",
    "std::env",
    "env::var",
    "var_os",
    "GROBID_URL",
    "127.0.0.1:8070",
    "cache_root",
    "CACHE_ROOT",
    "retry",
    "backoff",
];

#[test]
fn batch_and_cache_contract_surfaces_stay_free_of_locations_and_retry() {
    for rel in BATCH_CACHE_SURFACES {
        let text = read_text(rel);
        assert!(
            text.len() > 1000,
            "positive control failed: {rel} looks truncated"
        );
        let hits = surface_marker_hits_for(&text, &BATCH_CACHE_FORBIDDEN_MARKERS);
        assert!(
            hits.is_empty(),
            "batch/cache contract surface {rel} leaked location/tuning/retry material:\n{}",
            hits.join("\n")
        );
    }
}

#[test]
fn batch_surface_scanner_fails_visible_on_synthetic_violations() {
    // Built via format! so the literals of THIS file stay clean.
    let synthetic = format!(
        "let policy = {}::exponential();\nlet tries = {}_{}.max(1);\n",
        "backoff", "attempt", "retry"
    );
    let hits = surface_marker_hits_for(&synthetic, &BATCH_CACHE_FORBIDDEN_MARKERS);
    assert_eq!(hits.len(), 2, "expected two code hits, got {hits:?}");
    assert!(hits.iter().any(|h| h.contains("backoff")));
    assert!(hits.iter().any(|h| h.contains("retry")));
    // A clean batch/cache surface passes the same scanner.
    assert!(
        surface_marker_hits_for(
            "let mode = CacheMode::Off;\n",
            &BATCH_CACHE_FORBIDDEN_MARKERS
        )
        .is_empty()
    );
}

// ───────── S03/T04: single sanctioned conservation evidence path ─────────

#[test]
fn parse_compat_emitter_consumes_the_engine_without_a_second_implementation() {
    let emitter = read_text("crates/da-cli/examples/parse_compat_baseline.rs");
    assert!(
        emitter.contains("ConservationEngine"),
        "parse-compat emitter must run the sanctioned da-parse conservation engine (S03/T04)"
    );
    for needle in [
        "struct ConservationDelta",
        "struct ConservationEntry",
        "struct ConservationIdentity",
        "fn scan_source",
        "fn digest_delta",
    ] {
        assert!(
            !emitter.contains(needle),
            "emitter must not duplicate the conservation model/comparator (`{needle}`) — one engine, one evidence path, no second ledger"
        );
    }
}

// ───── S05/T03: closed static routing, no physical defaults, policy locks ─────

/// The exact static Java GROBID routes of the closed `ParseEndpoint` mapping.
/// These literals are sanctioned ONLY in transport.rs (the closed mapping);
/// any other da-parse source dispatching one is free-form route dispatch.
const STATIC_ROUTE_LITERALS: [&str; 3] = [
    "/api/processFulltextDocument",
    "/api/processHeaderDocument",
    "/api/processReferences",
];

/// Comment-stripped `(line_no, line)` pairs — `//`-style line comments (doc
/// comments included) are prose and must not trip the route scanner.
fn comment_stripped_lines(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .map(|(line_no, line)| (line_no + 1, line.to_string()))
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .collect()
}

/// Route-literal scanner: an occurrence of a process route OUTSIDE the closed
/// mapping file is free-form dispatch and fails the gate.
fn free_form_route_hits(files: &[(String, String)]) -> Vec<String> {
    const CLOSED_MAPPING_FILE: &str = "crates/da-parse/src/transport.rs";
    let mut hits = Vec::new();
    for (rel, text) in files {
        for (line_no, line) in comment_stripped_lines(text) {
            for literal in STATIC_ROUTE_LITERALS {
                if line.contains(literal) && rel != CLOSED_MAPPING_FILE {
                    hits.push(format!(
                        "[free-form-route-dispatch] {rel}:{line_no}: route literal `{literal}` outside the closed ParseEndpoint mapping"
                    ));
                }
            }
        }
    }
    hits
}

#[test]
fn endpoint_routes_are_closed_static_and_free_of_free_form_dispatch() {
    // The closed mapping owns the exact static Java routes...
    let transport = read_text("crates/da-parse/src/transport.rs");
    for literal in STATIC_ROUTE_LITERALS {
        assert!(
            transport.contains(literal),
            "static route `{literal}` must stay in the closed ParseEndpoint mapping"
        );
    }

    // ...and no other da-parse source dispatches one — every caller selects
    // its route through ParseEndpoint::static_path (checked below).
    let mut files = Vec::new();
    collect_rs_files(&repo_root().join("crates/da-parse/src"), &mut files);
    assert!(
        files.len() >= 9,
        "positive control failed: only {} da-parse source files scanned",
        files.len()
    );
    let mut scanned: Vec<(String, String)> = Vec::new();
    for file in &files {
        let rel = file
            .strip_prefix(repo_root())
            .unwrap_or(file)
            .to_string_lossy()
            .to_string();
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("source unreadable {}: {e}", file.display()));
        scanned.push((rel, text));
    }
    let hits = free_form_route_hits(&scanned);
    assert!(
        hits.is_empty(),
        "free-form route dispatch appeared in the parse plane (routing must stay the closed static ParseEndpoint mapping):\n{}",
        hits.join("\n")
    );

    // Positive control: the HTTP exchange really selects routes through the
    // closed mapping.
    let http = read_text("crates/da-parse/src/http.rs");
    assert!(
        http.contains(".static_path()"),
        "the HTTP exchange must select its route via ParseEndpoint::static_path, never a caller-controlled string"
    );
}

#[test]
fn route_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "let route = \"/api/processHeaderDocument\";\n".to_string();
    let files = vec![("crates/da-parse/src/invented.rs".to_string(), synthetic)];
    let hits = free_form_route_hits(&files);
    assert_eq!(hits.len(), 1, "expected one dispatch hit, got {hits:?}");
    assert!(hits[0].contains("[free-form-route-dispatch]"));
    // The closed mapping file itself is sanctioned for the exact literals...
    let mapping = vec![(
        "crates/da-parse/src/transport.rs".to_string(),
        "Self::Header => \"/api/processHeaderDocument\",\n".to_string(),
    )];
    assert!(free_form_route_hits(&mapping).is_empty());
    // ...and comment prose must not trip the scanner.
    let prose = vec![(
        "crates/da-parse/src/invented.rs".to_string(),
        "/// POST /api/processHeaderDocument is the static route.\n".to_string(),
    )];
    assert!(free_form_route_hits(&prose).is_empty());
}

#[test]
fn closed_endpoint_vocabulary_carries_no_physical_defaults() {
    use da_parse::ParseEndpoint;

    let mut paths = Vec::new();
    for endpoint in [
        ParseEndpoint::Fulltext,
        ParseEndpoint::Header,
        ParseEndpoint::References,
    ] {
        let path = endpoint.static_path();
        assert!(
            path.starts_with("/api/"),
            "{endpoint}: static route must stay a relative Java GROBID path, got `{path}`"
        );
        for marker in ["http", "://", ":", "\\", "127.0.0.1", "8070"] {
            assert!(
                !path.contains(marker),
                "{endpoint}: static route `{path}` leaked physical default `{marker}`"
            );
        }
        paths.push(path);
    }
    let unique: BTreeSet<_> = paths.iter().collect();
    assert_eq!(
        unique.len(),
        3,
        "the three closed routes stay pairwise distinct"
    );
}

#[test]
fn parse_registry_keeps_d127_import_lock_candidate_identity_and_proxy_set() {
    let registry: serde_yaml::Value = {
        let text = read_text("data/parse/grobid_compatibility.yaml");
        serde_yaml::from_str(&text).expect("registry of record parses")
    };
    // D127: the import lock stays closed — the proxy wave must not open it.
    assert_eq!(
        registry.get("import_eligible").and_then(|v| v.as_bool()),
        Some(false),
        "D127 import lock drifted open in the registry of record"
    );
    assert_eq!(
        registry.get("auto_promote").and_then(|v| v.as_str()),
        Some("forbidden"),
        "auto-promotion must stay forbidden in the registry of record"
    );
    // The candidate stays the staged native Rust crate — never a JVM, native
    // model or storage-stack claim.
    assert_eq!(
        registry
            .get("defaults")
            .and_then(|d| d.get("candidate"))
            .and_then(|c| c.get("backend"))
            .and_then(|v| v.as_str()),
        Some("da-parse"),
    );
    assert_eq!(
        registry
            .get("defaults")
            .and_then(|d| d.get("rollback_target"))
            .and_then(|v| v.as_str()),
        Some("java-grobid-http@0.9.0-crf"),
        "the rollback target is the typed-transport rollback binding of record"
    );
    // Exactly the sanctioned capabilities may claim proxy state — an
    // invented proxy operation cannot smuggle itself into the registry.
    let proxy_ids: BTreeSet<String> = registry
        .get("capabilities")
        .and_then(|c| c.as_sequence())
        .expect("capabilities sequence")
        .iter()
        .filter(|c| c.get("state").and_then(|s| s.as_str()) == Some("proxy"))
        .filter_map(|c| c.get("id").and_then(|i| i.as_str()).map(str::to_string))
        .collect();
    let expected: BTreeSet<String> = [
        "rest.isalive",
        "rest.health",
        "rest.version",
        "rest.processFulltextDocument",
        "rest.processHeaderDocument",
        "rest.processReferences",
        "dialect.teiCoordinates",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(
        proxy_ids, expected,
        "the sanctioned proxy set is Java live ops plus W0 health/version probes"
    );
}

// ───── S06/T05: redb stays a feature-gated rebuildable-only accelerator ─────

/// Infrastructure crates allowed to carry the redb accelerator (D295 role
/// `rebuildable_only`). The workspace root's [workspace.dependencies]
/// version pin is not an acquisition and is deliberately not scanned —
/// only crates/*/Cargo.toml manifests are.
const REDB_ALLOWED_CRATES: [&str; 2] = ["da-adapters", "da-cli"];

/// Scan one crate manifest for redb material. A dependency row outside the
/// infrastructure crates, a non-optional row anywhere, or a redb feature
/// outside the infrastructure crates all fail the gate.
fn redb_dependency_hits(crate_name: &str, manifest_text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let mut section = String::new();
    for (line_no, raw) in manifest_text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if section == "features" {
            if let Some((name, _value)) = line.split_once('=')
                && name.trim() == "redb"
                && !REDB_ALLOWED_CRATES.contains(&crate_name)
            {
                hits.push(format!(
                    "[redb-feature-outside-infrastructure] line {}: {crate_name}: [{section}] {line}",
                    line_no + 1
                ));
            }
            continue;
        }
        if !section.ends_with("dependencies") {
            continue;
        }
        if let Some((name, rest)) = line.split_once('=')
            && name.trim() == "redb"
        {
            if !REDB_ALLOWED_CRATES.contains(&crate_name) {
                hits.push(format!(
                    "[redb-outside-infrastructure] line {}: {crate_name}: [{section}] {line}",
                    line_no + 1
                ));
            } else if !rest.contains("optional") {
                hits.push(format!(
                    "[redb-not-feature-gated] line {}: {crate_name}: [{section}] {line} must carry optional = true",
                    line_no + 1
                ));
            }
        }
    }
    hits
}

#[test]
fn redb_stays_feature_gated_in_infrastructure_crates_only() {
    // Positive controls: the sanctioned wiring really exists.
    let adapters = read_text("crates/da-adapters/Cargo.toml");
    assert!(
        adapters.contains("redb = { workspace = true, optional = true }"),
        "positive control failed: da-adapters lost its optional redb dependency"
    );
    assert!(
        adapters.contains("redb = [\"dep:redb\"]"),
        "positive control failed: da-adapters lost its redb feature"
    );
    let cli = read_text("crates/da-cli/Cargo.toml");
    assert!(
        cli.contains("redb = [\"da-adapters/redb\"]"),
        "positive control failed: da-cli lost its redb forwarding feature"
    );

    // The whole crates/ tree stays clean: only da-adapters carries the
    // (optional) dependency row, only the two infrastructure crates carry
    // the feature, and nothing else — parse plane, domain/application cores
    // or kg-* universals — acquires redb at all.
    let crates_dir = repo_root().join("crates");
    let mut scanned = 0usize;
    let mut hits = Vec::new();
    let entries = std::fs::read_dir(&crates_dir).expect("crates directory readable");
    for entry in entries.flatten() {
        let manifest = entry.path().join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        scanned += 1;
        let crate_name = entry.file_name().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|e| panic!("manifest unreadable {manifest:?}: {e}"));
        hits.extend(redb_dependency_hits(&crate_name, &text));
    }
    assert!(
        scanned >= 8,
        "positive control failed: only {scanned} crate manifests scanned"
    );
    assert!(
        hits.is_empty(),
        "redb leaked outside the feature-gated infrastructure accelerators (D295: rebuildable_only):\n{}",
        hits.join("\n")
    );

    // da-parse stays on the explicit name-level ban list too (belt and
    // braces: the value-aware scan above must agree with the ban list).
    let parse = read_text(DA_PARSE_MANIFEST);
    assert!(
        redb_dependency_hits("da-parse", &parse).is_empty(),
        "da-parse must not acquire redb"
    );
}

#[test]
fn redb_scanner_fails_visible_on_synthetic_violations() {
    let infra_non_optional = "[dependencies]\nredb = \"2.6\"\n";
    let hits = redb_dependency_hits("da-adapters", infra_non_optional);
    assert_eq!(
        hits.len(),
        1,
        "expected the missing optional flag, got {hits:?}"
    );
    assert!(hits[0].contains("[redb-not-feature-gated]"));

    let outside = "[dependencies]\nredb = { workspace = true, optional = true }\n";
    let hits = redb_dependency_hits("kg-storage", outside);
    assert_eq!(
        hits.len(),
        1,
        "expected the outside-infrastructure hit, got {hits:?}"
    );
    assert!(hits[0].contains("[redb-outside-infrastructure]"));

    let feature_outside = "[features]\nredb = [\"dep:redb\"]\n";
    let hits = redb_dependency_hits("da-domain", feature_outside);
    assert_eq!(hits.len(), 1, "expected the feature hit, got {hits:?}");
    assert!(hits[0].contains("[redb-feature-outside-infrastructure]"));

    // Clean infrastructure wiring passes the same scanner...
    let clean = "[dependencies]\nredb = { workspace = true, optional = true }\n\n[features]\nredb = [\"dep:redb\"]\n";
    assert!(redb_dependency_hits("da-adapters", clean).is_empty());
    // ...and a clean parse-plane manifest stays clean.
    assert!(
        redb_dependency_hits(
            "da-parse",
            "[dependencies]\nda-ports = { workspace = true }\n"
        )
        .is_empty()
    );
}

// ───── S09/T05: the native personal-name candidate stays a quarantined pure leaf ─────

const NATIVE_PERSONAL_NAME_SURFACE: &str = "crates/da-parse/src/native_personal_name.rs";
const NATIVE_PERSONAL_NAME_SIDE_CHANNEL_MARKERS: [&str; 22] = [
    "std::env",
    "env::var",
    "var_os",
    "std::fs",
    "std::path",
    "PathBuf",
    "File::",
    "std::net",
    "TcpStream",
    "reqwest",
    "http",
    "std::process",
    "Command::",
    "std::time",
    "SystemTime",
    "Instant",
    "std::thread",
    "std::sync",
    "OnceLock",
    "tracing",
    "rand",
    "println!",
];
const NATIVE_PERSONAL_NAME_SCOPE_CREEP_MARKERS: [&str; 10] = [
    "TeiEnvelope",
    "TeiLifter",
    "TeiParseIr",
    "ParserPort",
    "ParsedArticle",
    "ConservationEngine",
    "Samyama",
    "GraphStore",
    "import_eligible",
    "promotion_state",
];
const NATIVE_PERSONAL_NAME_COMPOSITION_SURFACES: [&str; 12] = [
    "crates/da-parse/src/http.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/lift.rs",
    "crates/da-parse/src/ir.rs",
    "crates/da-parse/src/projector.rs",
    "crates/da-parse/src/envelope.rs",
    "crates/da-parse/src/transport.rs",
    "crates/da-parse/src/backend.rs",
    "crates/da-parse/src/batch.rs",
    "crates/da-parse/src/cache.rs",
    "crates/da-parse/src/profile.rs",
    "crates/da-parse/src/conservation.rs",
];

#[test]
fn native_personal_name_module_is_pure_and_stays_out_of_runtime_composition() {
    let text = read_text(NATIVE_PERSONAL_NAME_SURFACE);
    assert!(
        text.len() > 5000,
        "positive control failed: {NATIVE_PERSONAL_NAME_SURFACE} looks truncated"
    );
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from(["serde".to_string()])
    );
    assert!(native_date_marker_hits(&text, &NATIVE_PERSONAL_NAME_SIDE_CHANNEL_MARKERS).is_empty());
    assert!(native_date_marker_hits(&text, &NATIVE_PERSONAL_NAME_SCOPE_CREEP_MARKERS).is_empty());

    let lib = read_text("crates/da-parse/src/lib.rs");
    assert!(lib.contains("mod native_personal_name;"));
    assert!(lib.contains("pub use native_personal_name::"));

    let mut hits = Vec::new();
    for rel in NATIVE_PERSONAL_NAME_COMPOSITION_SURFACES {
        for (line_no, raw) in read_text(rel).lines().enumerate() {
            if raw.contains("native_personal_name") || raw.contains("NativePersonalName") {
                hits.push(format!(
                    "[hidden-runtime-activation] {rel}:{}: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    let mut ports_files = Vec::new();
    collect_rs_files(&repo_root().join("crates/da-ports/src"), &mut ports_files);
    for file in ports_files {
        let rel = file
            .strip_prefix(repo_root())
            .unwrap_or(&file)
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("source unreadable {}: {e}", file.display()));
        for (line_no, raw) in source.lines().enumerate() {
            if raw.contains("native_personal_name") || raw.contains("NativePersonalName") {
                hits.push(format!(
                    "[hidden-runtime-activation] {rel}:{}: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "personal-name candidate leaked into runtime composition or da-ports:\n{}",
        hits.join("\n")
    );
    assert!(
        read_text("data/parse/grobid_compatibility.yaml").contains(
            "crates/da-cli/examples/parse_compat_baseline.rs:native_personal_name_shadow"
        )
    );
}

#[test]
fn native_personal_name_boundary_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = format!(
        "use {}::GraphStore;\nlet path = {}::read_to_string(\"x\");\nlet env = {}::var(\"X\");\nfn {}() {{}}\n",
        "samyama", "std::fs", "std::env", "ParserPort"
    );
    assert!(import_roots_of(&synthetic).contains("samyama"));
    assert!(
        native_date_marker_hits(&synthetic, &NATIVE_PERSONAL_NAME_SIDE_CHANNEL_MARKERS).len() >= 2
    );
    assert!(
        !native_date_marker_hits(&synthetic, &NATIVE_PERSONAL_NAME_SCOPE_CREEP_MARKERS).is_empty()
    );
    assert!(
        native_date_marker_hits(
            "// ParserPort and std::fs are prose only\n",
            &NATIVE_PERSONAL_NAME_SIDE_CHANNEL_MARKERS
        )
        .is_empty()
    );
}

// ───── S08/T04: the native date shadow candidate stays a quarantined pure leaf ─────

/// The S08 native date component surface. Since S08/T04 the gate additionally
/// proves the date module stays a pure, side-channel-free, scope-frozen leaf
/// and that the candidate never enters the runtime parse composition, the
/// crate's identity constants or a promoted lifecycle state. The shadow
/// candidate remains fully inspectable (crate-root re-export + registry
/// evidence_ref) but never served.
const NATIVE_DATE_SURFACE: &str = "crates/da-parse/src/native_date.rs";

/// Side-channel markers banned from the date module's CODE (comments may
/// document the prohibitions — the scanner strips line comments first).
/// The module imports nothing but `serde`, so every hit is a real violation:
/// no env/filesystem/physical-path, network/HTTP, clock/time, process,
/// tracing/logging, shared-state or RNG material may even be nameable from
/// the shadow candidate.
const NATIVE_DATE_SIDE_CHANNEL_MARKERS: [&str; 24] = [
    "std::env",
    "env::var",
    "var_os",
    "std::fs",
    "std::path",
    "PathBuf",
    "File::",
    "std::net",
    "TcpStream",
    "reqwest",
    "http",
    "std::process",
    "Command::",
    "std::time",
    "SystemTime",
    "Instant",
    "std::thread",
    "std::sync",
    "OnceLock",
    "tracing",
    "log::",
    "println!",
    "eprintln!",
    "rand",
];

/// Scope-creep markers: author/personal-name/affiliation/linkage and
/// header/reference/citation composition vocabulary must never appear in the
/// date module's CODE. S08 ships a DATE parser only — name/linkage extraction
/// and header/reference composition stay with the sanctioned
/// `rest.processHeader*` / `rest.processReferences` capabilities.
const NATIVE_DATE_SCOPE_CREEP_MARKERS: [&str; 9] = [
    "author",
    "affiliation",
    "surname",
    "given_name",
    "personal_name",
    "linkage",
    "citation",
    "reference",
    "header",
];

/// Parse-plane surfaces that must never reference the candidate: the runtime
/// composition (`ParserPort` facade, `ScholarlyParser`, `TeiLifter`,
/// `TeiParseIr`, the `ParsedArticle` projector, envelope/transport/backend,
/// batch/cache contracts, profile, conservation). Hidden runtime activation
/// is a fail — the parser is observable only through the additive
/// parse-compat shadow evidence.
const NATIVE_DATE_COMPOSITION_SURFACES: [&str; 12] = [
    "crates/da-parse/src/http.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/lift.rs",
    "crates/da-parse/src/ir.rs",
    "crates/da-parse/src/projector.rs",
    "crates/da-parse/src/envelope.rs",
    "crates/da-parse/src/transport.rs",
    "crates/da-parse/src/backend.rs",
    "crates/da-parse/src/batch.rs",
    "crates/da-parse/src/cache.rs",
    "crates/da-parse/src/profile.rs",
    "crates/da-parse/src/conservation.rs",
];

/// Comment-stripped substring scan (one hit per marker per line).
fn native_date_marker_hits(text: &str, markers: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        for marker in markers {
            if raw.contains(marker) {
                hits.push(format!(
                    "[native-date-boundary] line {}: marker `{marker}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    hits
}

/// Import roots (`use <root>::…`) of a comment-stripped text.
fn import_roots_of(text: &str) -> BTreeSet<String> {
    strip_line_comments(text)
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let rest = line.strip_prefix("use ")?;
            rest.split([':', ' ', '{', ';'])
                .find(|token| !token.is_empty())
                .map(str::to_string)
        })
        .collect()
}

#[test]
fn native_date_module_imports_only_serde_with_no_side_channels() {
    let text = read_text(NATIVE_DATE_SURFACE);
    assert!(
        text.len() > 5000,
        "positive control failed: {NATIVE_DATE_SURFACE} looks truncated"
    );

    // The only importable root is serde (Serialize/Deserialize for the
    // closed wire form) — no `extern crate`, nothing else.
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from(["serde".to_string()]),
        "the date module must import nothing but serde"
    );
    assert!(
        !strip_line_comments(&text).contains("extern crate"),
        "the date module must not use `extern crate`"
    );

    // No side channel is even nameable from the module's code.
    let hits = native_date_marker_hits(&text, &NATIVE_DATE_SIDE_CHANNEL_MARKERS);
    assert!(
        hits.is_empty(),
        "the native date module acquired a side channel (env/filesystem/network/clock/process/tracing/shared state) — the shadow candidate must stay a pure function:\n{}",
        hits.join("\n")
    );
}

#[test]
fn native_date_module_stays_a_date_parser_without_composition_growth() {
    let text = read_text(NATIVE_DATE_SURFACE);
    let hits = native_date_marker_hits(&text, &NATIVE_DATE_SCOPE_CREEP_MARKERS);
    assert!(
        hits.is_empty(),
        "S08 scope creep: author/personal-name/affiliation/linkage/header/reference composition vocabulary appeared in the date module's code:\n{}",
        hits.join("\n")
    );
}

#[test]
fn native_date_candidate_never_enters_the_parse_composition_or_da_ports() {
    // Positive controls: the module stays declared AND re-exported from the
    // crate root (full inspectability)...
    let lib = read_text("crates/da-parse/src/lib.rs");
    assert!(
        lib.contains("mod native_date;"),
        "positive control failed: the date module lost its crate-root declaration"
    );
    assert!(
        lib.contains("pub use native_date::"),
        "positive control failed: the candidate lost its crate-root re-export (it must stay fully inspectable)"
    );
    // ...and the registry of record still points at the evidence surface.
    let registry = read_text("data/parse/grobid_compatibility.yaml");
    assert!(
        registry.contains(NATIVE_DATE_SURFACE),
        "the registry evidence_ref must keep pointing at the candidate surface"
    );

    // Negative guard: no composition surface and no da-ports source
    // (`ParserPort` home) may reference the candidate.
    let mut hits = Vec::new();
    for rel in NATIVE_DATE_COMPOSITION_SURFACES {
        let text = read_text(rel);
        for (line_no, raw) in text.lines().enumerate() {
            if raw.contains("native_date") || raw.contains("NativeDate") {
                hits.push(format!(
                    "[hidden-runtime-activation] {rel}:{}: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    let mut ports_files = Vec::new();
    collect_rs_files(&repo_root().join("crates/da-ports/src"), &mut ports_files);
    assert!(
        ports_files.len() >= 5,
        "positive control failed: only {} da-ports source files scanned",
        ports_files.len()
    );
    for file in &ports_files {
        let rel = file
            .strip_prefix(repo_root())
            .unwrap_or(file)
            .to_string_lossy()
            .to_string();
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("source unreadable {}: {e}", file.display()));
        for (line_no, raw) in text.lines().enumerate() {
            if raw.contains("native_date") || raw.contains("NativeDate") {
                hits.push(format!(
                    "[hidden-runtime-activation] {rel}:{}: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "the native date candidate leaked into the runtime parse composition or da-ports (shadow means never served):\n{}",
        hits.join("\n")
    );
}

#[test]
fn parse_identity_fingerprints_stay_frozen_alongside_the_candidate() {
    use da_parse::{
        CACHED_TEI_BACKEND_ID, GROBID_HTTP_BACKEND_ID, GrobidProfile, NATIVE_DATE_PARSER_ID,
        NATIVE_DATE_PARSER_VERSION, PROJECT_ARTICLE_SCHEMA_FINGERPRINT, PROJECT_ARTICLE_STAGE_ID,
        RAW_TEI_STAGE_ID,
    };

    // Projection fingerprint and stage ids: byte-frozen — the candidate must
    // never rotate cache identity as a side effect.
    assert_eq!(
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        "ParsedArticle@serde-v4-p0-tei-lift"
    );
    assert_eq!(RAW_TEI_STAGE_ID, "parse.raw_tei.v1");
    assert_eq!(PROJECT_ARTICLE_STAGE_ID, "parse.project_article.v1");
    assert_eq!(GROBID_HTTP_BACKEND_ID, "grobid-http");
    assert_eq!(CACHED_TEI_BACKEND_ID, "cached-tei");

    // D204 rich/structural fingerprints: byte-frozen profile identities.
    assert_eq!(
        GrobidProfile::rich().configuration_fingerprint(),
        "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula"
    );
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula"
    );

    // The candidate identity stays exactly the versioned code truth the
    // registry and the baseline bind against.
    assert_eq!(
        format!("{NATIVE_DATE_PARSER_ID}@{NATIVE_DATE_PARSER_VERSION}"),
        "da-parse.native_date@1.0.0"
    );
}

#[test]
fn process_date_stays_shadow_candidate_with_java_rollback_and_zero_promotion() {
    use da_parse::{
        NATIVE_DATE_OUTCOME_SCHEMA_VERSION, NATIVE_DATE_PARSER_ID, NATIVE_DATE_PARSER_VERSION,
    };

    let registry: serde_yaml::Value =
        serde_yaml::from_str(&read_text("data/parse/grobid_compatibility.yaml"))
            .expect("registry of record parses");

    // D127 stays closed at the registry root.
    assert_eq!(
        registry.get("import_eligible").and_then(|v| v.as_bool()),
        Some(false),
        "D127 import lock drifted open in the registry of record"
    );

    // rest.processDate: shadow + candidate + Java rollback, bound to the
    // exact code identity (no registry/code drift).
    let capability = registry
        .get("capabilities")
        .and_then(|c| c.as_sequence())
        .expect("capabilities sequence")
        .iter()
        .find(|c| c.get("id").and_then(|i| i.as_str()) == Some("rest.processDate"))
        .expect("rest.processDate capability must exist")
        .clone();
    assert_eq!(
        capability.get("state").and_then(|v| v.as_str()),
        Some("shadow"),
        "rest.processDate must stay shadow — promotion is a conscious coordinated decision, never a refactor side effect"
    );
    assert_eq!(
        capability.get("promotion_state").and_then(|v| v.as_str()),
        Some("candidate")
    );
    assert_eq!(
        capability
            .get("candidate")
            .and_then(|v| v.as_str())
            .expect("candidate string"),
        format!("{NATIVE_DATE_PARSER_ID}@{NATIVE_DATE_PARSER_VERSION}"),
        "the registry candidate must match the code identity exactly"
    );
    for field in ["baseline", "rollback_target"] {
        assert_eq!(
            capability.get(field).and_then(|v| v.as_str()),
            Some("java-grobid-http@0.9.0-crf"),
            "Java GROBID must stay the {field} of record for rest.processDate"
        );
    }
    assert_eq!(
        capability.get("evidence_ref").and_then(|v| v.as_str()),
        Some(NATIVE_DATE_SURFACE)
    );
    let hard: BTreeSet<String> = capability
        .get("hard_regressions")
        .and_then(|v| v.as_sequence())
        .expect("hard_regressions sequence")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    for required in ["invented_revision", "nondeterministic_output"] {
        assert!(
            hard.contains(required),
            "rest.processDate lost hard regression `{required}`"
        );
    }

    // Zero capabilities may claim promoted-grade lifecycle state today; the
    // first legitimate promotion must consciously update this guard.
    let promoted: Vec<String> = registry
        .get("capabilities")
        .and_then(|c| c.as_sequence())
        .expect("capabilities sequence")
        .iter()
        .filter(|c| {
            c.get("state").and_then(|s| s.as_str()) == Some("promoted")
                || c.get("promotion_state").and_then(|s| s.as_str()) == Some("promoted")
        })
        .filter_map(|c| c.get("id").and_then(|i| i.as_str()).map(str::to_string))
        .collect();
    assert!(
        promoted.is_empty(),
        "silent promotion appeared in the registry: {promoted:?}"
    );

    // The comparison contract's additive shadow block carries the same
    // locked lifecycle and the closed forbidden vocabulary.
    let contract: serde_yaml::Value =
        serde_yaml::from_str(&read_text("data/parse/comparison_contract.yaml"))
            .expect("comparison contract parses");
    let shadow = contract
        .get("native_date_shadow")
        .expect("native_date_shadow block must exist");
    assert_eq!(
        shadow.get("observation_mode").and_then(|v| v.as_str()),
        Some("shadow")
    );
    assert_eq!(
        shadow.get("promotion_state").and_then(|v| v.as_str()),
        Some("candidate")
    );
    assert_eq!(
        shadow.get("engine").and_then(|v| v.as_str()),
        Some("da_parse::NativeDateParser")
    );
    assert_eq!(
        shadow.get("candidate_id").and_then(|v| v.as_str()),
        Some(NATIVE_DATE_PARSER_ID)
    );
    assert_eq!(
        shadow.get("candidate_version").and_then(|v| v.as_str()),
        Some(NATIVE_DATE_PARSER_VERSION)
    );
    assert_eq!(
        shadow
            .get("outcome_schema_version")
            .and_then(|v| v.as_str()),
        Some(NATIVE_DATE_OUTCOME_SCHEMA_VERSION)
    );
    assert_eq!(
        shadow.get("rollback_target").and_then(|v| v.as_str()),
        Some("java-grobid-http@0.9.0-crf")
    );
    let forbidden: BTreeSet<String> = shadow
        .get("forbidden")
        .and_then(|v| v.as_sequence())
        .expect("forbidden sequence")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    for required in [
        "auto_promotion",
        "runtime_routing",
        "raw_display_text",
        "tei_fragments",
        "invented_bindings",
    ] {
        assert!(
            forbidden.contains(required),
            "comparison contract lost forbidden rule `{required}`"
        );
    }
}

#[test]
fn native_date_boundary_scanner_fails_visible_on_synthetic_violations() {
    // Import-root scanner: a forbidden root fires, serde stays clean.
    let synthetic = format!("use {}::GraphStore;\nuse serde::Serialize;\n", "samyama");
    assert_eq!(
        import_roots_of(&synthetic),
        BTreeSet::from(["samyama".to_string(), "serde".to_string()])
    );

    // Side-channel scan: code hits fire; comment prose never trips.
    let synthetic = format!(
        "let v = {}::var(\"X\");\nlet t = {}::now();\nlet f = {}::read_to_string(\"x\");\n// {} mention in prose stays clean\n",
        "std::env", "std::time", "std::fs", "tracing"
    );
    let hits = native_date_marker_hits(&synthetic, &NATIVE_DATE_SIDE_CHANNEL_MARKERS);
    assert_eq!(hits.len(), 4, "expected four code hits, got {hits:?}");
    for marker in ["std::env", "env::var", "std::time", "std::fs"] {
        assert!(
            hits.iter().any(|h| h.contains(&format!("`{marker}`"))),
            "missing hit for `{marker}`: {hits:?}"
        );
    }
    assert!(hits.iter().all(|h| !h.contains("tracing")));

    // Scope-creep scan: composition vocabulary fires, prose stays clean.
    let synthetic = "fn author_affiliation_composition() {}\n".to_string();
    let hits = native_date_marker_hits(&synthetic, &NATIVE_DATE_SCOPE_CREEP_MARKERS);
    assert_eq!(hits.len(), 2, "expected two creep hits, got {hits:?}");
    assert!(
        native_date_marker_hits(
            "// author header reference prose must never trip the scanner\n",
            &NATIVE_DATE_SCOPE_CREEP_MARKERS
        )
        .is_empty()
    );
}

// ───── S10/T06: affiliation recognition/linking remain quarantined pure leaves ─────

const NATIVE_AFFILIATION_SURFACE: &str = "crates/da-parse/src/native_affiliation.rs";
const NATIVE_AFFILIATION_SIDE_CHANNEL_MARKERS: [&str; 24] = [
    "std::env",
    "env::var",
    "var_os",
    "std::fs",
    "std::path",
    "PathBuf",
    "File::",
    "std::net",
    "TcpStream",
    "reqwest",
    "std::process",
    "Command::",
    "std::time",
    "SystemTime",
    "Instant",
    "std::thread",
    "std::sync",
    "OnceLock",
    "tracing",
    "log::",
    "println!",
    "eprintln!",
    "rand",
    "tokio",
];
const NATIVE_AFFILIATION_SCOPE_CREEP_MARKERS: [&str; 15] = [
    "TeiEnvelope",
    "TeiLifter",
    "TeiParseIr",
    "ParserPort",
    "ParsedArticle",
    "projector",
    "Grobid",
    "GraphStore",
    "Samyama",
    "Conservation",
    "import_eligible",
    "promotion_state",
    "HeaderAuthor",
    "filesystem",
    "network",
];
const NATIVE_AFFILIATION_RUNTIME_SURFACES: [&str; 12] = [
    "crates/da-parse/src/http.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/lift.rs",
    "crates/da-parse/src/ir.rs",
    "crates/da-parse/src/projector.rs",
    "crates/da-parse/src/envelope.rs",
    "crates/da-parse/src/transport.rs",
    "crates/da-parse/src/backend.rs",
    "crates/da-parse/src/batch.rs",
    "crates/da-parse/src/cache.rs",
    "crates/da-parse/src/profile.rs",
    "crates/da-parse/src/conservation.rs",
];

#[test]
fn native_affiliation_components_are_serde_only_and_runtime_quarantined() {
    let text = read_text(NATIVE_AFFILIATION_SURFACE);
    assert!(
        text.len() > 10_000,
        "positive control failed: affiliation surface is truncated"
    );
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from(["serde".to_string()])
    );
    assert!(native_date_marker_hits(&text, &NATIVE_AFFILIATION_SIDE_CHANNEL_MARKERS).is_empty());
    assert!(native_date_marker_hits(&text, &NATIVE_AFFILIATION_SCOPE_CREEP_MARKERS).is_empty());

    // The parser half is text-only and must not acquire a linker dependency;
    // the linker half may mention only the accepted S09 name type.
    let parser_impl = text
        .split("impl NativeAffiliationParser")
        .nth(1)
        .and_then(|rest| rest.split("fn malformed").next())
        .expect("parser implementation remains present");
    assert!(!parser_impl.contains("NativeAffiliationLinker"));
    assert!(!parser_impl.contains("RecognizedPersonalName"));
    let linker_impl = text
        .split("impl NativeAffiliationLinker")
        .nth(1)
        .and_then(|rest| rest.split("fn valid_span").next())
        .expect("linker implementation remains present");
    assert!(linker_impl.contains("RecognizedPersonalName"));
    assert!(!linker_impl.contains("NativePersonalNameParser"));

    let lib = read_text("crates/da-parse/src/lib.rs");
    assert!(lib.contains("mod native_affiliation;"));
    assert!(lib.contains("pub use native_affiliation::"));
    let mut hits = Vec::new();
    for rel in NATIVE_AFFILIATION_RUNTIME_SURFACES {
        for (line_no, raw) in read_text(rel).lines().enumerate() {
            if raw.contains("native_affiliation") || raw.contains("NativeAffiliation") {
                hits.push(format!(
                    "[hidden-runtime-activation] {rel}:{}: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    let mut ports_files = Vec::new();
    collect_rs_files(&repo_root().join("crates/da-ports/src"), &mut ports_files);
    assert!(
        ports_files.len() >= 5,
        "positive control failed: da-ports scan is too small"
    );
    for file in ports_files {
        let rel = file
            .strip_prefix(repo_root())
            .unwrap_or(&file)
            .to_string_lossy();
        for (line_no, raw) in std::fs::read_to_string(&file)
            .expect("port source readable")
            .lines()
            .enumerate()
        {
            if raw.contains("native_affiliation") || raw.contains("NativeAffiliation") {
                hits.push(format!(
                    "[hidden-runtime-activation] {rel}:{}: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "native affiliation leaked into runtime composition:\n{}",
        hits.join("\n")
    );
}

#[test]
fn native_affiliation_boundary_scanner_fails_visible_on_synthetic_violations() {
    let synthetic =
        "use std::fs::File;\nuse serde::Serialize;\nlet x = std::env::var(\"X\");\nfn wire() {}\n";
    assert!(import_roots_of(synthetic).contains("std"));
    assert!(
        native_date_marker_hits(synthetic, &NATIVE_AFFILIATION_SIDE_CHANNEL_MARKERS).len() >= 3
    );
    assert!(
        !native_date_marker_hits(
            "fn x() { ParserPort::new(); }\n",
            &NATIVE_AFFILIATION_SCOPE_CREEP_MARKERS
        )
        .is_empty()
    );
    assert!(
        native_date_marker_hits(
            "// std::fs and ParserPort are prose only\n",
            &NATIVE_AFFILIATION_SIDE_CHANNEL_MARKERS
        )
        .is_empty()
    );
}

// ───── S11/T05: citation parsing remains a quarantined, pure shadow leaf ─────

const NATIVE_CITATION_SURFACE: &str = "crates/da-parse/src/native_citation.rs";
const NATIVE_CITATION_RUNTIME_SURFACES: [&str; 12] = [
    "crates/da-parse/src/http.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/lift.rs",
    "crates/da-parse/src/ir.rs",
    "crates/da-parse/src/projector.rs",
    "crates/da-parse/src/envelope.rs",
    "crates/da-parse/src/transport.rs",
    "crates/da-parse/src/backend.rs",
    "crates/da-parse/src/batch.rs",
    "crates/da-parse/src/cache.rs",
    "crates/da-parse/src/profile.rs",
    "crates/da-parse/src/conservation.rs",
];
const NATIVE_CITATION_SIDE_CHANNEL_MARKERS: [&str; 25] = [
    "std::env",
    "env::var",
    "var_os",
    "std::fs",
    "std::path",
    "PathBuf",
    "File::",
    "std::net",
    "TcpStream",
    "reqwest",
    "std::process",
    "Command::",
    "std::time",
    "SystemTime",
    "Instant",
    "std::thread",
    "std::sync",
    "OnceLock",
    "tracing",
    "log::",
    "println!",
    "eprintln!",
    "rand",
    "tokio",
    "std::collections",
];
const NATIVE_CITATION_SCOPE_CREEP_MARKERS: [&str; 20] = [
    "TeiEnvelope",
    "TeiLifter",
    "TeiParseIr",
    "ParserPort",
    "ParsedArticle",
    "projector",
    "Grobid",
    "GraphStore",
    "Samyama",
    "Conservation",
    "import_eligible",
    "promotion_state",
    "da_application",
    "da_adapters",
    "CitationEntry",
    "extract_citations",
    "SONA",
    "ruVLLM",
    "RVF",
    "redb",
];

#[test]
fn native_citation_is_pure_and_never_enters_runtime_or_ports() {
    let text = read_text(NATIVE_CITATION_SURFACE);
    assert!(
        text.len() > 10_000,
        "citation surface unexpectedly truncated"
    );
    let imports = import_roots_of(&text);
    assert_eq!(
        imports,
        BTreeSet::from(["crate".to_string(), "serde".to_string()])
    );
    assert!(native_date_marker_hits(&text, &NATIVE_CITATION_SIDE_CHANNEL_MARKERS).is_empty());
    assert!(native_date_marker_hits(&text, &NATIVE_CITATION_SCOPE_CREEP_MARKERS).is_empty());

    let lib = read_text("crates/da-parse/src/lib.rs");
    assert!(lib.contains("mod native_citation;"));
    assert!(lib.contains("pub use native_citation::"));
    let registry = read_text("data/parse/grobid_compatibility.yaml");
    assert!(registry.contains("native_citation_shadow"));

    let mut hits = Vec::new();
    for rel in NATIVE_CITATION_RUNTIME_SURFACES {
        for (line_no, raw) in read_text(rel).lines().enumerate() {
            if raw.contains("native_citation") || raw.contains("NativeCitation") {
                hits.push(format!(
                    "[hidden-runtime-activation] {rel}:{}: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    let mut ports_files = Vec::new();
    collect_rs_files(&repo_root().join("crates/da-ports/src"), &mut ports_files);
    for file in ports_files {
        let rel = file
            .strip_prefix(repo_root())
            .unwrap_or(&file)
            .to_string_lossy();
        for (line_no, raw) in std::fs::read_to_string(&file)
            .expect("port source readable")
            .lines()
            .enumerate()
        {
            if raw.contains("native_citation") || raw.contains("NativeCitation") {
                hits.push(format!(
                    "[hidden-runtime-activation] {rel}:{}: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "citation candidate leaked into runtime composition or da-ports:\n{}",
        hits.join("\n")
    );
}

#[test]
fn native_citation_boundary_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "use std::fs::File;\nuse serde::Serialize;\nlet x = std::env::var(\"X\");\nfn x() { ParserPort::new(); }\n";
    assert!(native_date_marker_hits(synthetic, &NATIVE_CITATION_SIDE_CHANNEL_MARKERS).len() >= 3);
    assert!(!native_date_marker_hits(synthetic, &NATIVE_CITATION_SCOPE_CREEP_MARKERS).is_empty());
    assert!(
        native_date_marker_hits(
            "// std::fs and ParserPort are prose only\n",
            &NATIVE_CITATION_SIDE_CHANNEL_MARKERS
        )
        .is_empty()
    );
}

// ───── S12/T05: composition remains a quarantined, pure leaf ─────

#[test]
fn native_segmentation_is_pure_and_quarantined_from_runtime_surfaces() {
    const SURFACE: &str = "crates/da-parse/src/native_segmentation.rs";
    let text = read_text(SURFACE);
    assert!(
        text.len() > 15_000,
        "native segmentation source unexpectedly truncated"
    );
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from([
            "crate".to_string(),
            "serde".to_string(),
            "sha2".to_string(),
            "std".to_string()
        ]),
        "segmentation may use only its typed layout, serde, hashing, and formatting leaves"
    );
    let forbidden = [
        "std::env",
        "std::fs",
        "std::path",
        "PathBuf",
        "std::net",
        "reqwest",
        "std::process",
        "Command::",
        "std::time",
        "SystemTime",
        "Instant",
        "std::thread",
        "std::sync",
        "tokio",
        "tracing",
        "log::",
        "println!",
        "eprintln!",
        "rand",
        "ParserPort",
        "TeiLifter",
        "TeiParseIr",
        "ParsedArticle",
        "GraphStore",
        "Samyama",
        "compose_header",
        "compose_references",
        "http://",
        "https://",
    ];
    let hits = native_date_marker_hits(&text, &forbidden);
    assert!(
        hits.is_empty(),
        "native segmentation acquired a side effect or forbidden coupling:\n{}",
        hits.join("\n")
    );
    assert!(text.contains("pub fn segment_document"));
    for forbidden_output in [
        "FigureHead",
        "TableHead",
        "FormulaHead",
        "VisualCaption",
        "Callout",
    ] {
        assert!(
            !text.contains(forbidden_output),
            "segmentation grew visual output type {forbidden_output}"
        );
    }

    let runtime_surfaces = [
        "crates/da-parse/src/http.rs",
        "crates/da-parse/src/parser.rs",
        "crates/da-parse/src/lift.rs",
        "crates/da-parse/src/ir.rs",
        "crates/da-parse/src/projector.rs",
        "crates/da-parse/src/envelope.rs",
        "crates/da-parse/src/transport.rs",
        "crates/da-parse/src/backend.rs",
        "crates/da-parse/src/batch.rs",
        "crates/da-parse/src/cache.rs",
        "crates/da-parse/src/profile.rs",
        "crates/da-parse/src/conservation.rs",
        "crates/da-ports/src/lib.rs",
        "crates/da-ports/src/parser.rs",
        "crates/da-application/src/lib.rs",
        "crates/da-adapters/src/lib.rs",
        "crates/da-cli/src/main.rs",
    ];
    let mut leaked = Vec::new();
    for rel in runtime_surfaces {
        for (line_no, raw) in read_text(rel).lines().enumerate() {
            if raw.contains("native_segmentation")
                || raw.contains("NativeSegmenter")
                || raw.contains("segment_document")
            {
                leaked.push(format!("{rel}:{}: {}", line_no + 1, raw.trim()));
            }
        }
    }
    let mut ports = Vec::new();
    collect_rs_files(&repo_root().join("crates/da-ports/src"), &mut ports);
    for file in ports {
        let rel = file
            .strip_prefix(repo_root())
            .unwrap_or(&file)
            .display()
            .to_string();
        for (line_no, raw) in std::fs::read_to_string(&file)
            .expect("port source readable")
            .lines()
            .enumerate()
        {
            if raw.contains("native_segmentation")
                || raw.contains("NativeSegmenter")
                || raw.contains("segment_document")
            {
                leaked.push(format!("{rel}:{}: {}", line_no + 1, raw.trim()));
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "native segmentation leaked into runtime or ParserPort surfaces:\n{}",
        leaked.join("\n")
    );

    let cli_src = read_text("crates/da-cli/src/main.rs");
    assert!(!cli_src.contains("native_segmentation") && !cli_src.contains("NativeSegmenter"));
    let registry = serde_yaml::from_str::<serde_yaml::Value>(&read_text(
        "data/parse/grobid_compatibility.yaml",
    ))
    .expect("compatibility registry");
    let stage = registry
        .get("capabilities")
        .and_then(|v| v.as_sequence())
        .expect("capabilities")
        .iter()
        .find(|v| v.get("id").and_then(|v| v.as_str()) == Some("stage.document_segmentation"))
        .expect("Java segmentation stage");
    assert_eq!(
        stage.get("state").and_then(|v| v.as_str()),
        Some("shadow"),
        "W1 native segmentation is shadow with a real surface, not a silent fallback"
    );
    assert_eq!(
        stage.get("baseline").and_then(|v| v.as_str()),
        Some("java-grobid-segmentation")
    );
    assert_eq!(
        stage.get("candidate").and_then(|v| v.as_str()),
        Some("none")
    );
}

#[test]
fn native_segmentation_boundary_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "use std::fs::File;\nuse serde::Serialize;\nfn compose_header() {}\n";
    assert!(!native_date_marker_hits(synthetic, &["std::fs", "compose_header"]).is_empty());
    assert!(
        native_date_marker_hits(
            "// std::fs and compose_header are prose\n",
            &["std::fs", "compose_header"]
        )
        .is_empty()
    );
}

#[test]
fn native_composition_is_pure_and_has_independent_entry_points() {
    const SURFACE: &str = "crates/da-parse/src/native_composition.rs";
    let text = read_text(SURFACE);
    assert!(
        text.len() > 10_000,
        "composition source unexpectedly truncated"
    );
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from(["crate".to_string(), "serde".to_string()]),
        "composition may import only sanctioned typed leaves and serde"
    );
    let forbidden = [
        "std::env",
        "std::fs",
        "std::path",
        "PathBuf",
        "std::net",
        "reqwest",
        "std::process",
        "Command::",
        "std::time",
        "std::thread",
        "std::sync",
        "tokio",
        "tracing",
        "log::",
        "println!",
        "eprintln!",
        "rand",
        "ParserPort",
        "ScholarlyParser",
        "GrobidParser",
        "TeiLifter",
        "TeiParseIr",
        "ParsedArticle",
        "projector",
        "GraphStore",
        "Samyama",
        "SONA",
        "ruVLLM",
        "RVF",
        "redb",
        "http://",
        "https://",
    ];
    let hits = native_date_marker_hits(&text, &forbidden);
    assert!(
        hits.is_empty(),
        "composition acquired a runtime side channel or forbidden coupling:\n{}",
        hits.join("\n")
    );
    assert!(text.contains("pub fn compose_header"));
    assert!(text.contains("pub fn compose_references"));
    assert!(
        !text.contains("compose_header(input: ReferenceCompositionInput")
            && !text.contains("compose_references(input: HeaderCompositionInput"),
        "header and references must remain independently typed/selectable"
    );

    let lib = read_text("crates/da-parse/src/lib.rs");
    assert!(lib.contains("mod native_composition;"));
    assert!(lib.contains("pub use native_composition::"));
    let mut hits = Vec::new();
    for rel in [
        "crates/da-parse/src/parser.rs",
        "crates/da-parse/src/http.rs",
        "crates/da-parse/src/lift.rs",
        "crates/da-parse/src/ir.rs",
        "crates/da-parse/src/projector.rs",
        "crates/da-parse/src/cache.rs",
        "crates/da-parse/src/transport.rs",
        "crates/da-parse/src/backend.rs",
        "crates/da-parse/src/batch.rs",
    ] {
        for (line_no, raw) in read_text(rel).lines().enumerate() {
            if raw.contains("native_composition")
                || raw.contains("compose_header")
                || raw.contains("compose_references")
            {
                hits.push(format!(
                    "[hidden-composition-activation] {rel}:{}: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "composition leaked into runtime/parser/cache surfaces:\n{}",
        hits.join("\n")
    );
}

// ───── S18/T05: native fulltext composition remains pure and quarantined ─────

#[test]
fn native_fulltext_composition_is_pure_and_stays_outside_every_runtime_route() {
    const SURFACE: &str = "crates/da-parse/src/native_fulltext_composition.rs";
    let text = read_text(SURFACE);
    assert!(
        text.len() > 10_000,
        "native fulltext composition source unexpectedly truncated"
    );
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from([
            "crate".to_string(),
            "serde".to_string(),
            "sha2".to_string(),
            "std".to_string()
        ]),
        "fulltext composer may import only typed leaves, hashing, serde and formatting"
    );
    let forbidden = [
        "std::env",
        "std::fs",
        "std::path",
        "PathBuf",
        "std::net",
        "reqwest",
        "std::process",
        "Command::",
        "std::time",
        "SystemTime",
        "Instant",
        "std::thread",
        "std::sync",
        "tokio",
        "tracing",
        "log::",
        "println!",
        "eprintln!",
        "rand",
        "ParserPort",
        "ScholarlyParser",
        "GrobidParser",
        "TeiLifter",
        "TeiParseIr",
        "ParsedArticle",
        "GraphStore",
        "Samyama",
        "SONA",
        "ruVLLM",
        "RVF",
        "redb",
        "da_adapters",
        "http://",
        "https://",
    ];
    let hits = native_date_marker_hits(&text, &forbidden);
    assert!(
        hits.is_empty(),
        "native fulltext composer acquired a side channel or forbidden coupling:\\n{}",
        hits.join("\\n")
    );
    for required in [
        "pub fn compose_native_fulltext",
        "NativeFulltextCompositionInput",
        "NativeFulltextCompositionError",
        "NATIVE_FULLTEXT_FALLBACK_TARGET",
    ] {
        assert!(text.contains(required), "fulltext composer lost {required}");
    }

    // The composer is a public shadow API, but none of the runtime seams may
    // mention it. This includes both parse-plane stages and every composition
    // root that could otherwise turn candidate evidence into a route.
    let runtime_dirs = [
        "crates/da-parse/src",
        "crates/da-ports/src",
        "crates/da-application/src",
        "crates/da-adapters/src",
        "crates/da-cli/src",
    ];
    let mut files = Vec::new();
    for dir in runtime_dirs {
        collect_rs_files(&repo_root().join(dir), &mut files);
    }
    let mut leaked = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(repo_root())
            .unwrap_or(&file)
            .to_string_lossy()
            .to_string();
        // lib.rs is the sanctioned public re-export boundary. It exposes
        // the shadow API without wiring it into ParserPort or production
        // composition; all other runtime files must remain unaware.
        if rel == SURFACE || rel == "crates/da-parse/src/lib.rs" {
            continue;
        }
        for (line_no, raw) in std::fs::read_to_string(&file)
            .expect("runtime source readable")
            .lines()
            .enumerate()
        {
            if raw.contains("native_fulltext_composition")
                || raw.contains("NativeFulltext")
                || raw.contains("compose_native_fulltext")
            {
                leaked.push(format!("{rel}:{}: {}", line_no + 1, raw.trim()));
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "native fulltext candidate leaked into runtime or production composition roots:\\n{}",
        leaked.join("\\n")
    );

    // The immutable compatibility authority keeps the current honest state:
    // Java remains the rollback for every component and candidate evidence is
    // not allowed to mutate graph/import or become a promotion decision.
    let registry = serde_yaml::from_str::<serde_yaml::Value>(&read_text(
        "data/parse/grobid_compatibility.yaml",
    ))
    .expect("compatibility registry");
    let capability = registry["capabilities"]
        .as_sequence()
        .expect("capabilities")
        .iter()
        .find(|v| {
            v.get("id").and_then(|id| id.as_str())
                == Some("parse.component.native_fulltext_composition_shadow")
        })
        .expect("S18 fulltext shadow capability");
    assert_eq!(capability["state"].as_str(), Some("shadow"));
    assert_eq!(capability["promotion_state"].as_str(), Some("candidate"));
    assert_eq!(
        capability["baseline"].as_str(),
        Some("java-grobid-http@0.9.0-crf")
    );
    assert_eq!(registry["import_eligible"].as_bool(), Some(false));
    assert_eq!(registry["auto_promote"].as_str(), Some("forbidden"));
}

#[test]
fn native_fulltext_boundary_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "use std::fs::File;\\nuse da_adapters::x;\\nfn run() { ParserPort::new(); }\\n";
    let hits = native_date_marker_hits(synthetic, &["std::fs", "da_adapters", "ParserPort"]);
    assert_eq!(hits.len(), 3);
    assert!(
        native_date_marker_hits(
            "// std::fs and ParserPort are prose only\\n",
            &["std::fs", "ParserPort"]
        )
        .is_empty()
    );
}

// ───── S16/T05: native visual heads and observations remain shadow-only ─────

#[test]
fn native_visual_heads_and_observations_are_pure_and_quarantined() {
    let head_surface = read_text("crates/da-parse/src/native_visual.rs");
    assert!(
        head_surface.len() > 10_000,
        "native visual head source truncated"
    );
    assert_eq!(
        import_roots_of(&head_surface),
        BTreeSet::from(["crate".to_string(), "serde".to_string(), "std".to_string()]),
        "native heads may use only typed geometry, serde, and deterministic collections"
    );
    let observation_surface = read_text("crates/da-adapters/src/layout_visual.rs");
    assert!(
        observation_surface.len() > 8_000,
        "visual observation seam source truncated"
    );
    let forbidden = [
        "std::env",
        "std::fs",
        "std::path",
        "PathBuf",
        "std::net",
        "reqwest",
        "std::process",
        "Command::",
        "std::time",
        "SystemTime",
        "Instant",
        "std::thread",
        "std::sync",
        "tokio",
        "tracing",
        "log::",
        "println!",
        "eprintln!",
        "rand",
        "ParserPort",
        "TeiLifter",
        "TeiParseIr",
        "ParsedArticle",
        "GraphStore",
        "Samyama",
        "compose_header",
        "compose_references",
        "http://",
        "https://",
        "pdftotext",
        "pdffigures2",
    ];
    for (surface, text) in [
        ("native visual heads", head_surface.as_str()),
        ("visual observation seam", observation_surface.as_str()),
    ] {
        let hits = native_date_marker_hits(text, &forbidden);
        assert!(
            hits.is_empty(),
            "{surface} acquired forbidden coupling:\n{}",
            hits.join("\n")
        );
    }
    for required in [
        "detect_figure_heads",
        "detect_table_heads",
        "detect_formula_heads",
        "NativeVisualAbsence",
        "NativeVisualError",
    ] {
        assert!(
            head_surface.contains(required),
            "native head API lost {required}"
        );
    }
    assert!(observation_surface.contains("terminal_absence"));
    assert!(observation_surface.contains("source_pdf_sha256"));

    let runtime_surfaces = [
        "crates/da-parse/src/http.rs",
        "crates/da-parse/src/parser.rs",
        "crates/da-parse/src/lift.rs",
        "crates/da-parse/src/ir.rs",
        "crates/da-parse/src/projector.rs",
        "crates/da-parse/src/envelope.rs",
        "crates/da-parse/src/transport.rs",
        "crates/da-parse/src/backend.rs",
        "crates/da-parse/src/batch.rs",
        "crates/da-parse/src/cache.rs",
        "crates/da-ports/src/lib.rs",
        "crates/da-ports/src/parser.rs",
        "crates/da-application/src/lib.rs",
        "crates/da-cli/src/main.rs",
    ];
    let mut leaked = Vec::new();
    for rel in runtime_surfaces {
        for (line_no, raw) in read_text(rel).lines().enumerate() {
            if raw.contains("native_visual")
                || raw.contains("NativeVisual")
                || raw.contains("layout_visual")
                || raw.contains("VisualObservation")
            {
                leaked.push(format!("{rel}:{}: {}", line_no + 1, raw.trim()));
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "native visual capability leaked into runtime/parser/cache surfaces:\n{}",
        leaked.join("\n")
    );

    let lib = read_text("crates/da-parse/src/lib.rs");
    assert!(lib.contains("mod native_visual;"));
    assert!(lib.contains("pub use native_visual::"));
    let layout = read_text("crates/da-parse/src/layout.rs");
    assert!(layout.contains("LAYOUT_SCHEMA_VERSION"));
    let compat = read_text("data/parse/grobid_compatibility.yaml");
    assert!(compat.contains("native_visual_shadow:"));
    assert!(compat.contains("java_tei: supporting_only"));
    assert!(compat.contains("local_pdf_oracle: independent_pdf_geometry_truth"));
    assert!(compat.contains("native_head_scores: shadow_only"));
}

#[test]
fn native_visual_quarantine_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "use std::fs::File;\nfn run() { println!(\"x\"); }\nParserPort::new();\n";
    let hits = native_date_marker_hits(synthetic, &["std::fs", "println!", "ParserPort"]);
    assert_eq!(hits.len(), 3);
    assert!(
        native_date_marker_hits(
            "// std::fs and ParserPort are prose\n",
            &["std::fs", "ParserPort"]
        )
        .is_empty()
    );
}

// ───── S06/T02: header atoms stay shadow — never in parse-pdf source_cli or ParserPort ─────
//
// Mirrors the Governor header quarantine (T01) as an independent compile/test
// contract: the exact tokens `compose_header`, `run_header_sequence` and
// `NativeDateParser` must not appear in code positions of the two production
// boundaries. `//` comments (full-line and trailing) are stripped before the
// scan; `decompose` never matches because the call-site `(` is required for
// the snake_case atoms while NativeDateParser needs word boundaries.

/// Production boundaries that must stay free of header atoms.
const HEADER_QUARANTINE_BOUNDARIES: [&str; 2] = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
];

/// Comment-stripped header-atom scan over code text. One hit per token per
/// line; mirrors the Governor `_header_quarantine_tokens_present` semantics:
/// snake_case atoms require a call-site `(`, NativeDateParser is word-bound.
fn header_quarantine_hits(text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        // `strip_line_comments` blanks full-line prose; trailing `//` prose
        // is cut here so evidence comments never flag (Governor parity).
        let code = match raw.find("//") {
            Some(cut) => &raw[..cut],
            None => raw,
        };
        for token in ["compose_header", "run_header_sequence"] {
            let bytes = code.as_bytes();
            let needle = token.as_bytes();
            let mut i = 0;
            let mut found = false;
            while i + needle.len() <= bytes.len() {
                if &bytes[i..i + needle.len()] == needle {
                    let before_ok =
                        i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                    if before_ok {
                        let mut j = i + needle.len();
                        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                            j += 1;
                        }
                        if j < bytes.len() && bytes[j] == b'(' {
                            found = true;
                            break;
                        }
                    }
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
            if found {
                hits.push(format!(
                    "[header-quarantine] line {}: atom `{token}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
        let ident: &[u8] = b"NativeDateParser";
        let bytes = code.as_bytes();
        let mut i = 0;
        let mut found = false;
        while i + ident.len() <= bytes.len() {
            if &bytes[i..i + ident.len()] == ident {
                let before_ok =
                    i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                let after = i + ident.len();
                let after_ok = after >= bytes.len()
                    || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
                if before_ok && after_ok {
                    found = true;
                    break;
                }
                i += ident.len();
            } else {
                i += 1;
            }
        }
        if found {
            hits.push(format!(
                "[header-quarantine] line {}: atom `NativeDateParser`: {}",
                line_no + 1,
                raw.trim()
            ));
        }
    }
    hits
}

#[test]
fn header_quarantine_scanner_fails_visible_on_synthetic_atoms() {
    let synthetic = "fn run() {\n    compose_header(input);\n    run_header_sequence(seq);\n    let _ = NativeDateParser;\n}\n";
    let hits = header_quarantine_hits(synthetic);
    assert_eq!(hits.len(), 3, "each header atom must flag: {hits:?}");
    assert!(hits.iter().any(|h| h.contains("compose_header")));
    assert!(hits.iter().any(|h| h.contains("run_header_sequence")));
    assert!(hits.iter().any(|h| h.contains("NativeDateParser")));
    // Comments are prose, not splice; `decompose` is a different word.
    let prose = "// compose_header run_header_sequence NativeDateParser are prose\nlet x = decompose(y); // compose_header(input);\n";
    assert!(
        header_quarantine_hits(prose).is_empty(),
        "prose comments and `decompose` must not flag"
    );
}

#[test]
fn parse_pdf_source_cli_and_parser_port_stay_free_of_header_atoms() {
    let mut leaked = Vec::new();
    for rel in HEADER_QUARANTINE_BOUNDARIES {
        let text = read_text(rel);
        assert!(
            text.len() > 1_000,
            "positive control failed: {rel} looks truncated"
        );
        for hit in header_quarantine_hits(&text) {
            leaked.push(format!("{rel}: {hit}"));
        }
    }
    assert!(
        leaked.is_empty(),
        "header atoms leaked into a production boundary (shadow means never served):\n{}",
        leaked.join("\n")
    );
}

#[test]
fn parse_pdf_p3_wires_java_lifted_header_authors_not_native_splice() {
    let src = read_text("crates/da-cli/src/source_cli.rs");
    let code = strip_line_comments(&src);
    assert!(
        code.contains("stamp_p3_header_observe(&mut m, parsed.header_authors.len())"),
        "P3 must count Java-lifted ParsedArticle.header_authors"
    );
    assert!(header_quarantine_hits(&src).is_empty());
}

// ───── S07/T02: bibliography atoms stay shadow — never in parse-pdf source_cli or ParserPort ─────
//
// Mirrors the Governor bibliography quarantine (S07/T01) as an independent
// compile/test contract: the exact tokens `compose_references` and
// `run_reference_sequence` must not appear in code positions of the two
// production boundaries. `//` comments (full-line and trailing) are stripped
// before the scan; `decompose_references` never matches because a word
// boundary is required before the token and a call-site `(` (with optional
// whitespace) is required after it.

/// Production boundaries that must stay free of bibliography atoms.
const BIBLIOGRAPHY_QUARANTINE_BOUNDARIES: [&str; 2] = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
];

/// Native bibliography method/type names that must never appear in ParserPort.
const BIBLIOGRAPHY_NATIVE_PORT_TOKENS: [&str; 8] = [
    "compose_references",
    "run_reference_sequence",
    "NativeCitationAnchorScanner",
    "ReferenceCompositionInput",
    "ReferenceCompositionCandidate",
    "ReferenceCompositionOutcome",
    "ReferenceSequenceInput",
    "ReferenceCandidate",
];

/// Comment-stripped bibliography-atom scan over code text. One hit per token
/// per line; mirrors the Governor `_bibliography_quarantine_tokens_present`
/// semantics: each snake_case atom requires a word boundary before it and a
/// call-site `(` (after optional whitespace) after it.
fn bibliography_quarantine_hits(text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        // `strip_line_comments` blanks full-line prose; trailing `//` prose
        // is cut here so evidence comments never flag (Governor parity).
        let code = match raw.find("//") {
            Some(cut) => &raw[..cut],
            None => raw,
        };
        for token in ["compose_references", "run_reference_sequence"] {
            let bytes = code.as_bytes();
            let needle = token.as_bytes();
            let mut i = 0;
            let mut found = false;
            while i + needle.len() <= bytes.len() {
                if &bytes[i..i + needle.len()] == needle {
                    let before_ok =
                        i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                    if before_ok {
                        let mut j = i + needle.len();
                        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                            j += 1;
                        }
                        if j < bytes.len() && bytes[j] == b'(' {
                            found = true;
                            break;
                        }
                    }
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
            if found {
                hits.push(format!(
                    "[bibliography-quarantine] line {}: atom `{token}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    hits
}

#[test]
fn bibliography_quarantine_scanner_fails_visible_on_synthetic_atoms() {
    let synthetic =
        "fn run() {\n    compose_references(input);\n    run_reference_sequence(seq);\n}\n";
    let hits = bibliography_quarantine_hits(synthetic);
    assert_eq!(hits.len(), 2, "each bibliography atom must flag: {hits:?}");
    assert!(hits.iter().any(|h| h.contains("compose_references")));
    assert!(hits.iter().any(|h| h.contains("run_reference_sequence")));
    // Whitespace before `(` is still a call site and must flag.
    let spaced =
        "fn run() {\n    compose_references (input);\n    run_reference_sequence  (seq);\n}\n";
    let hits = bibliography_quarantine_hits(spaced);
    assert_eq!(hits.len(), 2, "whitespace call-sites must flag: {hits:?}");
    // Comments are prose, not splice; `decompose_references` is a different word;
    // prose mentions without a call-site `(` are not splices.
    let prose = "// compose_references run_reference_sequence are prose\nlet x = decompose_references(y);\nlet y = compose_references;\nfoo(); // compose_references(input);\n";
    assert!(
        bibliography_quarantine_hits(prose).is_empty(),
        "prose comments, prefixed identifiers and bare mentions must not flag"
    );
}

#[test]
fn parse_pdf_source_cli_and_parser_port_stay_free_of_bibliography_atoms() {
    let mut leaked = Vec::new();
    for rel in BIBLIOGRAPHY_QUARANTINE_BOUNDARIES {
        let text = read_text(rel);
        assert!(
            text.len() > 1_000,
            "positive control failed: {rel} looks truncated"
        );
        for hit in bibliography_quarantine_hits(&text) {
            leaked.push(format!("{rel}: {hit}"));
        }
    }
    assert!(
        leaked.is_empty(),
        "bibliography atoms leaked into a production boundary (shadow means never served):\n{}",
        leaked.join("\n")
    );
}

#[test]
fn parse_pdf_p4_wires_java_lifted_biblstruct_citations_not_native_splice() {
    let src = read_text("crates/da-cli/src/source_cli.rs");
    let code = strip_line_comments(&src);
    assert!(
        code.contains("stamp_p4_references_observe(&mut m, parsed.citations.len())"),
        "P4 must count Java-lifted ParsedArticle.citations (biblStruct receipt)"
    );
    assert!(
        code.contains("p4.insert(\"production_authority\"") && code.contains("\"java_http\""),
        "P4 production_authority must stay java_http"
    );
    assert!(
        code.contains("p4.insert(\"citation_names_not_biblstruct\"")
            && code.contains("true.into()"),
        "P4 citation_names_not_biblstruct must stay boolean true"
    );
    assert!(bibliography_quarantine_hits(&src).is_empty());
}

#[test]
fn parser_port_carries_no_native_bibliography_methods_or_types() {
    let src = read_text("crates/da-ports/src/parser.rs");
    let code = strip_line_comments(&src);
    assert!(
        code.contains("pub trait ParserPort"),
        "positive control failed: ParserPort trait vanished"
    );
    assert!(
        code.contains("async fn parse_pdf"),
        "positive control failed: ParserPort.parse_pdf vanished"
    );
    let mut leaked = Vec::new();
    for token in BIBLIOGRAPHY_NATIVE_PORT_TOKENS {
        if code.contains(token) {
            leaked.push(token.to_string());
        }
    }
    assert!(
        leaked.is_empty(),
        "native bibliography methods/types leaked into ParserPort: {leaked:?}"
    );
    assert!(
        bibliography_quarantine_hits(&src).is_empty(),
        "bibliography call-sites leaked into ParserPort"
    );
}

// ───── S08/T02: visual heads stay shadow — never in parse-pdf source_cli or ParserPort ─────
//
// Mirrors the Governor visual-heads quarantine (S08/T01, DATA-gated — the
// forbidden set and the boundaries live in
// `defaults.visual_heads_quarantine`, never hardcoded here) as an
// independent compile/test contract: the exact tokens `detect_figure_heads`,
// `detect_table_heads`, `detect_formula_heads`, `NativeVisual` and
// `LayoutProvider` must not appear in code positions of the two production
// boundaries. `//` comments (full-line and trailing) are stripped before the
// scan; `redetect_figure_heads` and prose mentions never match because a
// word boundary is required before each token, snake_case atoms additionally
// require a call-site `(` (after optional whitespace) and CamelCase atoms
// require a word boundary after the token.

/// Production boundaries that must stay free of visual-head atoms.
const VISUAL_HEADS_QUARANTINE_BOUNDARIES: [&str; 2] = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
];

/// Snake_case visual-head call-sites that must never splice into a boundary.
const VISUAL_HEADS_SNAKE_TOKENS: [&str; 4] = [
    "detect_figure_heads",
    "detect_table_heads",
    "detect_formula_heads",
    "run_visual_heads_sequence",
];

/// CamelCase visual/layout type names that must never splice into a boundary.
const VISUAL_HEADS_TYPE_TOKENS: [&str; 2] = ["NativeVisual", "LayoutProvider"];

/// Native visual method/type names that must never appear in ParserPort.
const VISUAL_HEADS_NATIVE_PORT_TOKENS: [&str; 6] = [
    "detect_figure_heads",
    "detect_table_heads",
    "detect_formula_heads",
    "run_visual_heads_sequence",
    "NativeVisual",
    "LayoutProvider",
];

/// Comment-stripped visual-head scan over code text. One hit per token per
/// line; mirrors the Governor `_visual_heads_quarantine_tokens_present`
/// semantics: snake_case atoms need a word boundary before and a call-site
/// `(` (after optional whitespace) after; CamelCase atoms need word
/// boundaries on both sides.
fn visual_heads_quarantine_hits(text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        // `strip_line_comments` blanks full-line prose; trailing `//` prose
        // is cut here so evidence comments never flag (Governor parity).
        let code = match raw.find("//") {
            Some(cut) => &raw[..cut],
            None => raw,
        };
        for token in VISUAL_HEADS_SNAKE_TOKENS {
            let bytes = code.as_bytes();
            let needle = token.as_bytes();
            let mut i = 0;
            let mut found = false;
            while i + needle.len() <= bytes.len() {
                if &bytes[i..i + needle.len()] == needle {
                    let before_ok =
                        i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                    if before_ok {
                        let mut j = i + needle.len();
                        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                            j += 1;
                        }
                        if j < bytes.len() && bytes[j] == b'(' {
                            found = true;
                            break;
                        }
                    }
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
            if found {
                hits.push(format!(
                    "[visual-heads-quarantine] line {}: atom `{token}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
        for token in VISUAL_HEADS_TYPE_TOKENS {
            let bytes = code.as_bytes();
            let needle = token.as_bytes();
            let mut i = 0;
            let mut found = false;
            while i + needle.len() <= bytes.len() {
                if &bytes[i..i + needle.len()] == needle {
                    let before_ok =
                        i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                    let after = i + needle.len();
                    let after_ok = after >= bytes.len()
                        || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
                    if before_ok && after_ok {
                        found = true;
                        break;
                    }
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
            if found {
                hits.push(format!(
                    "[visual-heads-quarantine] line {}: atom `{token}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    hits
}

#[test]
fn visual_heads_quarantine_scanner_fails_visible_on_synthetic_atoms() {
    let synthetic = "fn run() {\n    detect_figure_heads(input);\n    detect_table_heads(input);\n    detect_formula_heads(input);\n    run_visual_heads_sequence(input);\n    let _ = NativeVisual;\n    let _ = LayoutProvider;\n}\n";
    let hits = visual_heads_quarantine_hits(synthetic);
    assert_eq!(hits.len(), 6, "each visual-head atom must flag: {hits:?}");
    for token in VISUAL_HEADS_NATIVE_PORT_TOKENS {
        assert!(
            hits.iter().any(|h| h.contains(token)),
            "atom `{token}` must flag: {hits:?}"
        );
    }
    // Whitespace before `(` is still a call site and must flag.
    let spaced = "fn run() {\n    detect_figure_heads (input);\n    detect_table_heads  (input);\n    detect_formula_heads\t(input);\n    run_visual_heads_sequence (input);\n}\n";
    let hits = visual_heads_quarantine_hits(spaced);
    assert_eq!(hits.len(), 4, "whitespace call-sites must flag: {hits:?}");
    // Comments are prose, not splice; `redetect_figure_heads` is a different
    // word; prose mentions without a call-site `(` are not splices; a
    // CamelCase prefix (`NativeVisualizer`) is not the quarantined type.
    let prose = "// detect_figure_heads detect_table_heads NativeVisual LayoutProvider are prose\nlet x = redetect_figure_heads(y);\nlet y = detect_figure_heads;\nlet z = NativeVisualizer;\nfoo(); // detect_table_heads(input);\n";
    assert!(
        visual_heads_quarantine_hits(prose).is_empty(),
        "prose comments, prefixed identifiers, bare mentions and type prefixes must not flag"
    );
}

#[test]
fn parse_pdf_source_cli_and_parser_port_stay_free_of_visual_heads() {
    let mut leaked = Vec::new();
    for rel in VISUAL_HEADS_QUARANTINE_BOUNDARIES {
        let text = read_text(rel);
        assert!(
            text.len() > 1_000,
            "positive control failed: {rel} looks truncated"
        );
        for hit in visual_heads_quarantine_hits(&text) {
            leaked.push(format!("{rel}: {hit}"));
        }
    }
    assert!(
        leaked.is_empty(),
        "visual-head atoms leaked into a production boundary (shadow means never served):\n{}",
        leaked.join("\n")
    );
}

#[test]
fn parse_pdf_p6_wires_java_lifted_visual_counts_not_native_splice() {
    let src = read_text("crates/da-cli/src/source_cli.rs");
    let code = strip_line_comments(&src);
    // P6 observes the Java lift: figure/table/formula counts plus the typed
    // table/formula missingness carried on ParsedArticle — never a native
    // head call.
    assert!(
        code.contains("stamp_p6_visual_observe("),
        "P6 must keep its Java-lift observe stamp"
    );
    for arg in [
        "parsed.figures.len()",
        "parsed.tables.len()",
        "parsed.formulas.len()",
        "parsed.table_missingness.is_some()",
        "parsed.formula_missingness.is_some()",
    ] {
        assert!(
            code.contains(arg),
            "P6 must count Java-lifted ParsedArticle visual receipts ({arg})"
        );
    }
    assert!(
        code.contains("p6.insert(\"production_authority\"") && code.contains("\"java_http\""),
        "P6 production_authority must stay java_http"
    );
    assert!(
        code.contains("p6.insert(\"observe_mode\"") && code.contains("\"shadow_receipt\""),
        "P6 observe_mode must stay shadow_receipt"
    );
    assert!(
        code.contains("p6.insert(\"empty_count_not_success\"") && code.contains("true.into()"),
        "P6 empty_count_not_success must stay boolean true"
    );
    assert!(visual_heads_quarantine_hits(&src).is_empty());
}

#[test]
fn parser_port_carries_no_native_visual_methods_or_types() {
    let src = read_text("crates/da-ports/src/parser.rs");
    let code = strip_line_comments(&src);
    assert!(
        code.contains("pub trait ParserPort"),
        "positive control failed: ParserPort trait vanished"
    );
    assert!(
        code.contains("async fn parse_pdf"),
        "positive control failed: ParserPort.parse_pdf vanished"
    );
    let mut leaked = Vec::new();
    for token in VISUAL_HEADS_NATIVE_PORT_TOKENS {
        if code.contains(token) {
            leaked.push(token.to_string());
        }
    }
    assert!(
        leaked.is_empty(),
        "native visual methods/types leaked into ParserPort: {leaked:?}"
    );
    assert!(
        visual_heads_quarantine_hits(&src).is_empty(),
        "visual-head call-sites leaked into ParserPort"
    );
}

// ───── M498/S02/T01: P5 body is a Java production stamp ─────
//
// P5 (`fulltext_body_sections`) is stamped from ParsedArticle shape only;
// native accountant symbols remain quarantined from production parse-pdf.

#[test]
fn parse_pdf_p5_body_has_live_java_stamp_between_p4_and_p6() {
    let src = read_text("crates/da-cli/src/source_cli.rs");
    let code = strip_line_comments(&src);
    assert!(
        code.len() > 1_000,
        "positive control failed: source_cli.rs looks truncated"
    );
    // P5 must be live and derive both observations from Java ParsedArticle.
    let p4 = code.find("stamp_p4_references_observe").expect("P4 stamp");
    let p5 = code.find("stamp_p5_body_observe").expect("P5 stamp");
    let p6 = code.find("stamp_p6_visual_observe").expect("P6 stamp");
    assert!(p4 < p5 && p5 < p6, "P5 stamp must be between P4 and P6");
    assert!(code.contains(
        "stamp_p5_body_observe(&mut m, parsed.sections.len(), parsed.body_text.is_empty())"
    ));
    // Neighbouring receipts stay live: P4 counts Java-lifted citations,
    // P6 counts Java-lifted visual families.
    assert!(
        code.contains("stamp_p4_references_observe(&mut m, parsed.citations.len())"),
        "positive control failed: P4 live receipt vanished (S09 must not regress P4)"
    );
    assert!(
        code.contains("stamp_p6_visual_observe("),
        "positive control failed: P6 live receipt vanished (S09 must not regress P6)"
    );
}

// ───── M493 S10/T01: DecisionPoint stays six approved wire names ─────
//
// Meta/prompt freeze after the family quarantines: `DecisionPoint` in
// `crates/da-domain/src/fsm_policy.rs` stays exactly the six sanctioned
// variants, and the registry later names (`head_fallback`, `visual_bundle`,
// `critic_sample`) never become decision points. Only NEW local `s10_*`
// helpers are added here — no production symbol, no existing helper and no
// DATA file is touched.

/// Production source under contract (read-only).
const S10_DECISION_POINT_SOURCE: &str = "crates/da-domain/src/fsm_policy.rs";

/// The six sanctioned enum variants, in declaration order.
const S10_APPROVED_VARIANTS: [&str; 6] = [
    "GrammarRoute",
    "BatchAdmission",
    "ConcurrencyTune",
    "RetryPolicy",
    "SummaryDepth",
    "ProfileRoute",
];

/// The six sanctioned wire names, in declaration order.
const S10_APPROVED_WIRE: [&str; 6] = [
    "grammar_route",
    "batch_admission",
    "concurrency_tune",
    "retry_policy",
    "summary_depth",
    "profile_route",
];

/// Registry later names that must never appear as decision points.
const S10_LATER_NAMES: [&str; 3] = ["head_fallback", "visual_bundle", "critic_sample"];

/// Enum-variant spellings of the later names (must never be introduced).
const S10_LATER_VARIANTS: [&str; 3] = ["HeadFallback", "VisualBundle", "CriticSample"];

/// Local S10 helper: strip full-line AND trailing `//` comments so prose
/// mentions of later names never count as enum drift. Wire literals in the
/// scanned file carry no `//` sequences, so a plain split is sound here.
fn s10_code_without_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.split_once("//") {
            Some((code, _)) => code,
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Local S10 helper: extract the `pub enum DecisionPoint { ... }` body with
/// brace matching over comment-stripped code.
fn s10_enum_body(code: &str) -> Option<String> {
    let start = code.find("pub enum DecisionPoint")?;
    let open = code[start..].find('{')? + start;
    let mut depth = 0usize;
    for (i, ch) in code[open..].char_indices() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(code[open..open + i + 1].to_string());
            }
        }
    }
    None
}

/// Local S10 helper: accept exactly the six approved wire names.
fn s10_wire_allowed(name: &str) -> bool {
    S10_APPROVED_WIRE.contains(&name)
}

/// Local S10 helper: violations of the closed six-point contract in
/// comment-stripped source. Every violation carries
/// `[s10-decision-point-drift]`.
fn s10_decision_point_violations(code: &str) -> Vec<String> {
    const TAG: &str = "[s10-decision-point-drift]";
    let mut errs = Vec::new();
    let Some(body) = s10_enum_body(code) else {
        return vec![format!(
            "{TAG} lost `pub enum DecisionPoint` — loss, not silence"
        )];
    };
    for variant in S10_APPROVED_VARIANTS {
        if !body.contains(variant) {
            errs.push(format!(
                "{TAG} DecisionPoint lost sanctioned variant `{variant}`"
            ));
        }
    }
    for wire in S10_APPROVED_WIRE {
        let arm = format!("\"{wire}\"");
        if !code.contains(&arm) {
            errs.push(format!("{TAG} DecisionPoint lost wire arm `{wire}`"));
        }
    }
    for later in S10_LATER_NAMES {
        let lit = format!("\"{later}\"");
        if code.contains(&lit) {
            errs.push(format!(
                "{TAG} later name `{later}` leaked into DecisionPoint source — later stays later"
            ));
        }
    }
    for variant in S10_LATER_VARIANTS {
        if body.contains(variant) {
            errs.push(format!(
                "{TAG} later variant `{variant}` promoted into the DecisionPoint enum — closed set of six"
            ));
        }
    }
    errs
}

#[test]
fn s10_decision_point_stays_six_approved_wire_names() {
    let src = read_text(S10_DECISION_POINT_SOURCE);
    assert!(
        src.len() > 1_000,
        "positive control failed: fsm_policy.rs looks truncated"
    );
    let code = s10_code_without_comments(&src);
    let errs = s10_decision_point_violations(&code);
    assert!(errs.is_empty(), "DecisionPoint drift:\n{}", errs.join("\n"));
    // Accept and round-trip exactly the six approved wire names.
    for wire in S10_APPROVED_WIRE {
        assert!(
            s10_wire_allowed(wire),
            "positive control failed: approved wire `{wire}` rejected"
        );
    }
    // Later names and unknown points stay rejected.
    for bad in [
        "head_fallback",
        "visual_bundle",
        "critic_sample",
        "invented_point",
        "grammarRoute",
        "GRAMMAR_ROUTE",
        "",
    ] {
        assert!(
            !s10_wire_allowed(bad),
            "negative control failed: `{bad}` accepted as a DecisionPoint"
        );
    }
}

#[test]
fn s10_decision_point_scanner_fails_visible_on_synthetic_violations() {
    let src = read_text(S10_DECISION_POINT_SOURCE);
    let code = s10_code_without_comments(&src);
    assert!(
        s10_enum_body(&code).is_some(),
        "positive control failed: enum body not found in clean source"
    );
    // Comment-only prose about a later name must NOT trip the gate.
    let with_prose = code.clone() + "\n// visual_bundle stays later, prose only\n";
    let stripped = s10_code_without_comments(&with_prose);
    assert!(
        s10_decision_point_violations(&stripped).is_empty(),
        "comment prose must not trip the DecisionPoint gate"
    );
    // A later wire literal smuggled into code fails loudly.
    let smuggled = code.clone() + "\nlet _ = \"visual_bundle\";\n";
    let errs = s10_decision_point_violations(&smuggled);
    assert!(
        errs.iter()
            .any(|e| e.contains("[s10-decision-point-drift]") && e.contains("visual_bundle")),
        "expected [s10-decision-point-drift] for smuggled visual_bundle, got: {errs:?}"
    );
    // A later variant smuggled into the enum body fails loudly.
    let body = s10_enum_body(&code).expect("enum body must exist");
    let drifted_body = body.replacen("ProfileRoute,", "ProfileRoute,\n    VisualBundle,", 1);
    assert_ne!(
        body, drifted_body,
        "positive control failed: enum body has no ProfileRoute arm to extend"
    );
    let drifted = code.replacen(&body, &drifted_body, 1);
    let errs = s10_decision_point_violations(&drifted);
    assert!(
        errs.iter()
            .any(|e| e.contains("[s10-decision-point-drift]") && e.contains("VisualBundle")),
        "expected [s10-decision-point-drift] for VisualBundle variant, got: {errs:?}"
    );
    // A dropped approved wire arm fails loudly (replace ALL occurrences:
    // the wire legitimately appears in both `as_str` and `parse`).
    let dropped = code.replace("\"profile_route\"", "\"profile_rout\"");
    assert_ne!(
        code, dropped,
        "positive control failed: no profile_route arm to drop"
    );
    let errs = s10_decision_point_violations(&dropped);
    assert!(
        errs.iter()
            .any(|e| e.contains("[s10-decision-point-drift]") && e.contains("profile_route")),
        "expected [s10-decision-point-drift] for dropped profile_route, got: {errs:?}"
    );
    // A lost enum is loss, not silence.
    let errs = s10_decision_point_violations("pub fn nothing_here() {}");
    assert!(
        errs.iter()
            .any(|e| e.contains("[s10-decision-point-drift]")),
        "expected [s10-decision-point-drift] for lost enum, got: {errs:?}"
    );
}

// ───── S11/T02: corpus constructors stay structural, from_env stays rich ─────

/// S11 constructor-policy roots: bare `from_env` may appear ONLY as a
/// health probe (main.rs `check_health`) or as an explicit approved
/// `with_profile` construction after a ProfileRoute consult
/// (load.rs `parse_primary`, source_cli.rs `run_parse_pdf`); the parse-plane
/// helper definition and its unit tests are the remaining sanctioned sites.
/// Every real corpus PDF constructor must go through
/// `corpus_grobid_parser()` — the helper itself is CRITICAL blast radius
/// (19 symbols, 7 flows) and is NOT reworked here, only its call sites.
const S11_CORPUS_HELPER_USES: [&str; 2] = [
    "crates/da-cli/examples/batch_promote_gold.rs",
    "crates/da-cli/examples/gepa_hub_research.rs",
];

const S11_SANCTIONED_BARE_FROM_ENV: [(&str, &str); 4] = [
    // (file, required nearby role marker proving the classification)
    ("crates/da-cli/src/main.rs", "is_alive"),
    ("crates/da-cli/src/load.rs", ".with_profile"),
    ("crates/da-cli/src/source_cli.rs", ".with_profile"),
    ("crates/da-cli/src/parse_profile.rs", "with_profile"),
];

fn s11_bare_from_env_lines(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.to_string()))
        .filter(|(_, l)| {
            l.contains("GrobidParser::from_env()") && !l.trim_start().starts_with("//")
        })
        .collect()
}

#[test]
fn s11_corpus_examples_use_the_structural_helper_not_bare_from_env() {
    for rel in S11_CORPUS_HELPER_USES {
        let text = read_text(rel);
        assert!(
            text.contains("corpus_grobid_parser()"),
            "positive control failed: {rel} must construct its corpus parser via corpus_grobid_parser()"
        );
        let bare = s11_bare_from_env_lines(&text);
        assert!(
            bare.is_empty(),
            "S11: {rel} still carries bare GrobidParser::from_env() (production corpus PDF must consult corpus_grobid_parser()): {bare:?}"
        );
    }
}

#[test]
fn s11_remaining_bare_from_env_sites_stay_health_or_explicit_profile() {
    // Positive inventory: every remaining bare site must be one of the
    // sanctioned health-only / explicit-with_profile / helper-definition
    // roles — a new bare corpus parse constructor fails loudly.
    let mut seen: Vec<String> = Vec::new();
    for (rel, marker) in S11_SANCTIONED_BARE_FROM_ENV {
        let text = read_text(rel);
        let bare = s11_bare_from_env_lines(&text);
        assert!(
            !bare.is_empty(),
            "positive control failed: sanctioned {rel} lost its {marker} from_env site — inventory drifted"
        );
        // Line offset where the unit-test module starts (if any): bare
        // from_env inside `mod tests` is the Governor-sanctioned test role,
        // not a production constructor (mirrors _from_env_role).
        let test_module_from: Option<usize> = text
            .lines()
            .position(|l| l.contains("mod tests"))
            .map(|i| i + 1);
        for (line_no, line) in &bare {
            // The role marker must sit within a 12-line window so the
            // classification is local evidence, not file folklore.
            let window: Vec<&str> = text
                .lines()
                .skip(line_no.saturating_sub(7))
                .take(14)
                .collect();
            let joined = window.join("\n");
            let is_test_role = rel.ends_with("parse_profile.rs")
                && test_module_from.is_some_and(|from| *line_no >= from);
            assert!(
                joined.contains(marker) || is_test_role,
                "S11: {rel}:{line_no} bare from_env lost its `{marker}` role evidence: `{}`",
                line.trim()
            );
        }
        seen.push(rel.to_string());
    }
    // Closed world: scan the DATA-declared roots and fail on any bare site
    // outside the sanctioned inventory.
    for root in ["crates/da-cli/src", "crates/da-cli/examples"] {
        let mut files = Vec::new();
        collect_rs_files(&repo_root().join(root), &mut files);
        for file in &files {
            let rel = file
                .strip_prefix(repo_root())
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            if !seen.contains(&rel) && !S11_CORPUS_HELPER_USES.contains(&rel.as_str()) {
                let text = std::fs::read_to_string(file)
                    .unwrap_or_else(|e| panic!("source unreadable {}: {e}", file.display()));
                let bare = s11_bare_from_env_lines(&text);
                assert!(
                    bare.is_empty(),
                    "S11: unsanctioned bare GrobidParser::from_env() in {rel}: {bare:?} — route it via corpus_grobid_parser() or extend the sanctioned inventory"
                );
            }
        }
    }
}

#[test]
fn s11_rich_structural_fingerprints_stay_distinct_and_frozen() {
    use da_parse::GrobidProfile;
    // D204 exact strings: from_env identity (rich) and corpus identity
    // (structural) must never mix or collide.
    assert_eq!(
        GrobidProfile::rich().configuration_fingerprint(),
        "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula"
    );
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula"
    );
    assert_ne!(
        GrobidProfile::rich().configuration_fingerprint(),
        GrobidProfile::structural().configuration_fingerprint(),
        "S11: rich/structural fingerprint collision — D204 separation lost"
    );
    // Synthetic collision control: a single-flag mutation must not equal
    // either frozen identity.
    let mutated = "profile=structural;consolidateHeader=1;consolidateCitations=0;teiCoordinates=figure,table,note,formula";
    assert_ne!(mutated, GrobidProfile::rich().configuration_fingerprint());
    assert_ne!(
        mutated,
        GrobidProfile::structural().configuration_fingerprint()
    );
}

// ───── M493 S12/T02: composition stays a quarantined candidate leaf ─────
//
// Governor fail-closed twin (S12/T02): da-parse must not acquire a Samyama
// dependency, native compose/run atoms must not enter the parse-pdf
// boundary or ParserPort, Java stays the production oracle, P5 has a live
// shape-only stamp, and DecisionPoint stays exactly six wire variants.
// Only NEW local `s12_*` helpers are added here — no production symbol, no
// existing helper and no DATA file is touched.

/// Production boundaries that must stay free of native compose atoms.
const S12_COMPOSE_QUARANTINE_BOUNDARIES: [&str; 2] = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
];

/// Native compose/run atoms that stay shadow-only (never production).
const S12_COMPOSE_ATOMS: [&str; 4] = [
    "compose_header",
    "run_header_sequence",
    "compose_references",
    "run_reference_sequence",
];

/// Local S12 helper: call-site hits for native compose atoms in code text.
/// Mirrors the header/bibliography quarantine scanners: snake_case atoms
/// require a call-site `(` (optional whitespace allowed) with a word
/// boundary before the token; `//` comments never count.
fn s12_compose_atom_hits(text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        let code = match raw.find("//") {
            Some(cut) => &raw[..cut],
            None => raw,
        };
        for token in S12_COMPOSE_ATOMS {
            let bytes = code.as_bytes();
            let needle = token.as_bytes();
            let mut i = 0;
            let mut found = false;
            while i + needle.len() <= bytes.len() {
                if &bytes[i..i + needle.len()] == needle {
                    let before_ok =
                        i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                    if before_ok {
                        let mut j = i + needle.len();
                        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                            j += 1;
                        }
                        if j < bytes.len() && bytes[j] == b'(' {
                            found = true;
                            break;
                        }
                    }
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
            if found {
                hits.push(format!(
                    "[s12-compose-quarantine] line {}: atom `{token}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    hits
}

/// Local S12 helper: violations of the composition quarantine in code text.
/// Every violation carries `[s12-composition-quarantine]`.
fn s12_composition_quarantine_violations(code: &str) -> Vec<String> {
    const TAG: &str = "[s12-composition-quarantine]";
    let mut errs = Vec::new();
    for hit in s12_compose_atom_hits(code) {
        errs.push(format!(
            "{TAG} native compose atom in production boundary: {hit}"
        ));
    }
    errs
}

#[test]
fn s12_composition_stays_quarantined_candidate_leaf() {
    // Manifest: da-parse must not acquire a Samyama dependency; the
    // forbidden-dependency table already pins samyama, but S12 re-asserts
    // the freeze explicitly so a graph-store bypass cannot hide behind a
    // renamed helper.
    let manifest = read_text("crates/da-parse/Cargo.toml");
    assert!(
        manifest.len() > 200,
        "positive control failed: da-parse Cargo.toml looks truncated"
    );
    let names = manifest_dependency_names(&manifest);
    assert!(
        !names.iter().any(|(_, name)| name == "samyama"),
        "S12: da-parse must not depend on samyama — graph stays frozen"
    );
    assert!(
        forbidden_dependency_hits(&names).is_empty(),
        "S12: forbidden dependency entered da-parse"
    );
    // Production boundaries: no native compose/run atoms in code positions.
    for rel in S12_COMPOSE_QUARANTINE_BOUNDARIES {
        let text = read_text(rel);
        assert!(
            text.len() > 1_000,
            "positive control failed: {rel} looks truncated"
        );
        let violations = s12_composition_quarantine_violations(&text);
        assert!(
            violations.is_empty(),
            "S12: native compose atoms leaked into a production boundary (candidate stays shadow):\n{}",
            violations.join("\n")
        );
    }
    // ParserPort itself carries no compose surface.
    let port = read_text("crates/da-ports/src/parser.rs");
    assert!(
        port.contains("pub trait ParserPort"),
        "positive control failed: ParserPort trait vanished"
    );
    for atom in S12_COMPOSE_ATOMS {
        assert!(
            !port.contains(atom),
            "S12: ParserPort must not carry native compose atom `{atom}`"
        );
    }
    // Java stays the production oracle: GrobidParser construction remains
    // the production parse entry, native composition never replaces it.
    let http = read_text("crates/da-parse/src/http.rs");
    assert!(
        http.contains("GrobidParser"),
        "positive control failed: GrobidParser vanished — Java oracle lost"
    );
    // P5 has a live Java shape stamp, DecisionPoint stays six wires: reuse the
    // neighbouring contracts as positive controls so S12 cannot drift them.
    let cli = read_text("crates/da-cli/src/source_cli.rs");
    assert!(
        strip_line_comments(&cli).contains("stamp_p5_body_observe"),
        "S12: live P5 Java shape stamp disappeared"
    );
    let fsm = read_text("crates/da-domain/src/fsm_policy.rs");
    let fsm_code = s10_code_without_comments(&fsm);
    assert!(
        s10_decision_point_violations(&fsm_code).is_empty(),
        "S12: DecisionPoint drifted off the six sanctioned wires"
    );
}

// ───── M496 S05/T02: body accounting stays a test-only shadow sequence ─────
//
// The P2 -> body accountant is intentionally exposed only by da-parse's
// shadow runner. The parse-pdf CLI boundary and ParserPort must remain Java
// production surfaces; P5 remains Java shape-only and promotion/import/graph
// locks stay closed.
const S05_BODY_QUARANTINE_BOUNDARIES: [&str; 2] = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
];
const S05_BODY_QUARANTINE_ATOMS: [&str; 2] = ["NativeBodyAccountant", "run_native_body_sequence"];

fn s05_body_quarantine_hits(text: &str) -> Vec<String> {
    let code = s10_code_without_comments(text);
    S05_BODY_QUARANTINE_ATOMS
        .iter()
        .filter(|atom| {
            if atom.contains('_') {
                code.contains(&format!("{atom}("))
            } else {
                code.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .any(|part| part == **atom)
            }
        })
        .map(|atom| (*atom).to_owned())
        .collect()
}

#[test]
fn s05_body_accounting_stays_quarantined_from_production_boundaries() {
    for rel in S05_BODY_QUARANTINE_BOUNDARIES {
        let src = read_text(rel);
        assert!(
            src.len() > 1_000,
            "positive control failed: {rel} looks truncated"
        );
        assert!(
            s05_body_quarantine_hits(&src).is_empty(),
            "S05: native body accountant leaked into {rel}"
        );
    }

    let port = read_text("crates/da-ports/src/parser.rs");
    assert!(port.contains("pub trait ParserPort"));
    assert!(port.contains("async fn parse_pdf"));

    let http = read_text("crates/da-parse/src/http.rs");
    assert!(
        http.contains("GrobidParser"),
        "Java parser authority disappeared"
    );

    let cli = read_text("crates/da-cli/src/source_cli.rs");
    let cli_code = s10_code_without_comments(&cli);
    assert!(
        cli_code.contains("stamp_p5_body_observe"),
        "P5 Java shape stamp must remain live"
    );

    let fsm = read_text("crates/da-domain/src/fsm_policy.rs");
    assert!(s10_decision_point_violations(&s10_code_without_comments(&fsm)).is_empty());

    let policy = read_text("scripts/governor/checks/scholarly_parse_plane.yaml");
    assert!(policy.contains("import_eligible: false"));
    assert!(policy.contains("graph_write: forbidden"));
    assert!(policy.contains("auto_promote: forbidden"));
}

#[test]
fn s05_body_accounting_scanner_fails_visible_on_synthetic_atoms() {
    for atom in S05_BODY_QUARANTINE_ATOMS {
        let synthetic = if atom.contains('_') {
            format!("fn run() {{ {atom}(input); }}")
        } else {
            format!("let _x: {atom};")
        };
        assert_eq!(s05_body_quarantine_hits(&synthetic), vec![atom.to_owned()]);
    }
    assert!(s05_body_quarantine_hits("// NativeBodyAccountant(input)\nlet x = 1;").is_empty());
    assert!(s05_body_quarantine_hits("let NativeBodyAccountantFactory = 1;").is_empty());
}

#[test]
fn s12_composition_scanner_fails_visible_on_synthetic_atoms() {
    // Negative control: every compose atom flags, prose never does.
    for atom in S12_COMPOSE_ATOMS {
        let synthetic = format!("fn run() {{\n    {atom} (input);\n}}\n");
        let hits = s12_compose_atom_hits(&synthetic);
        assert_eq!(hits.len(), 1, "atom `{atom}` must flag: {hits:?}");
        assert!(hits[0].contains(atom));
    }
    let prose = "// compose_header run_header_sequence compose_references run_reference_sequence are prose\nlet x = decompose(y); // compose_header(input);\n";
    assert!(
        s12_compose_atom_hits(prose).is_empty(),
        "prose comments and `decompose` must not flag"
    );
    assert!(
        s12_composition_quarantine_violations("let x = 1;\n").is_empty(),
        "clean code must not violate the S12 quarantine"
    );
}
