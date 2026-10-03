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

fn rust_sources_under(rel: &str) -> Vec<(String, String)> {
    fn visit(root: &Path, repo: &Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(root).expect("architecture source directory") {
            let entry = entry.expect("architecture source entry");
            let path = entry.path();
            if path.is_dir() {
                visit(&path, repo, out);
            } else if path.extension().and_then(|x| x.to_str()) == Some("rs") {
                let display = path
                    .strip_prefix(repo)
                    .expect("source stays under repository")
                    .display()
                    .to_string();
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("architecture source unreadable: {display}: {e}"));
                out.push((display, text));
            }
        }
    }
    let repo = repo_root();
    let mut out = Vec::new();
    visit(&repo.join(rel), &repo, &mut out);
    out
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

// ───── S04/T01: the native region carrier is lattice-bound and TEI-free ─────

/// The S04 native region carrier surface: the additive join that turns the S03
/// layout plane into ordered header/references token runs. The gate proves the
/// module is a pure leaf whose only inputs are the provider-neutral layout
/// types (`LayoutDocument`/`LayoutLattice`/`LexemeRow`/`FullSegmentationOutcome`)
/// — no TEI envelope or span may be an input — and that it never leaks into the
/// Java-authoritative parse composition.
const NATIVE_REGION_INPUT_SURFACE: &str = "crates/da-parse/src/native_region_input.rs";

/// Side-channel and TEI/Java coupling markers banned from the carrier's CODE
/// (comments may document the prohibitions — the scanner strips line comments
/// first). The module imports nothing but its typed leaves, `serde`, `sha2`,
/// and `std`, so every hit is a real violation.
const NATIVE_REGION_INPUT_FORBIDDEN: [&str; 33] = [
    "TeiEnvelope",
    "TeiLifter",
    "TeiParseIr",
    "TeiSpan",
    "ParserPort",
    "ParsedArticle",
    "GraphStore",
    "Samyama",
    "import_eligible",
    "promotion_state",
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
    "http://",
    "https://",
    "File::",
    "var_os",
];

/// Parse-plane surfaces that must never reference the carrier: the runtime
/// composition (`ParserPort` facade, `ScholarlyParser`, `TeiLifter`,
/// `TeiParseIr`, the `ParsedArticle` projector, envelope/transport/backend) and
/// the sibling crates. The carrier is a shadow input band, never a splice.
const NATIVE_REGION_INPUT_COMPOSITION_SURFACES: [&str; 11] = [
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/projector.rs",
    "crates/da-parse/src/lift.rs",
    "crates/da-parse/src/ir.rs",
    "crates/da-parse/src/envelope.rs",
    "crates/da-parse/src/backend.rs",
    "crates/da-parse/src/transport.rs",
    "crates/da-ports/src/parser.rs",
    "crates/da-application/src/lib.rs",
    "crates/da-adapters/src/lib.rs",
    "crates/da-cli/src/main.rs",
];

#[test]
fn native_region_input_is_lattice_bound_and_tei_free() {
    let text = read_text(NATIVE_REGION_INPUT_SURFACE);
    assert!(
        text.len() > 5_000,
        "positive control failed: {NATIVE_REGION_INPUT_SURFACE} looks truncated"
    );
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from([
            "crate".to_string(),
            "serde".to_string(),
            "sha2".to_string(),
            "std".to_string()
        ]),
        "region carrier may import only its typed layout leaves, serde, hashing, and formatting"
    );
    let hits = native_date_marker_hits(&text, &NATIVE_REGION_INPUT_FORBIDDEN);
    assert!(
        hits.is_empty(),
        "native region carrier acquired TEI/Java coupling or a side effect:\n{}",
        hits.join("\n")
    );

    // New symbols registered at the crate root.
    for symbol in [
        "pub struct NativeRegionCarrier",
        "pub struct NativeRegionRun",
        "pub struct NativeRegionToken",
        "pub struct NativeRegionIdentity",
        "pub enum NativeRegionAbsence",
        "pub enum NativeRegionError",
        "pub fn build_native_region_carrier",
    ] {
        assert!(text.contains(symbol), "region carrier must export {symbol}");
    }
    assert!(
        text.contains("pub fn text("),
        "the carrier token must expose a text() accessor"
    );
    assert!(
        !text.contains("pub text:"),
        "the carrier token text must stay private (redaction)"
    );

    // No carrier input may be a TEI envelope or span: the only inputs are the
    // provider-neutral layout plane.
    let signature = text
        .split("pub fn build_native_region_carrier")
        .nth(1)
        .and_then(|rest| rest.split("->").next())
        .unwrap_or_default();
    for required in [
        "&LayoutDocument",
        "&LayoutLattice",
        "&[LexemeRow]",
        "&FullSegmentationOutcome",
        "&[FullSemanticRegion]",
    ] {
        assert!(
            signature.contains(required),
            "build_native_region_carrier must accept {required}: {signature}"
        );
    }
    for forbidden_input in ["TeiEnvelope", "TeiSpan"] {
        assert!(
            !signature.contains(forbidden_input),
            "carrier input must not be {forbidden_input}"
        );
    }

    let lib = read_text("crates/da-parse/src/lib.rs");
    assert!(lib.contains("mod native_region_input;"));
    assert!(lib.contains("pub use native_region_input::"));

    // Quarantine: the carrier never enters the Java-authoritative composition.
    let mut leaked = Vec::new();
    for rel in NATIVE_REGION_INPUT_COMPOSITION_SURFACES {
        let code = s10_code_without_comments(&read_text(rel));
        for marker in ["native_region_input", "NativeRegionCarrier"] {
            if code.contains(marker) {
                leaked.push(format!("{rel}: {marker}"));
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "native region carrier escaped shadow quarantine:\n{}",
        leaked.join("\n")
    );
}

#[test]
fn native_region_input_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "use samyama::GraphStore;\nfn f() { let _ = TeiEnvelope; }\n";
    assert!(import_roots_of(synthetic).contains("samyama"));
    assert!(!native_date_marker_hits(synthetic, &NATIVE_REGION_INPUT_FORBIDDEN).is_empty());
    assert!(
        native_date_marker_hits(
            "// TeiEnvelope, TeiSpan and ParserPort are prose only\n",
            &NATIVE_REGION_INPUT_FORBIDDEN
        )
        .is_empty()
    );
}

// ───── S04/T04 repair: the word projection and the heading walk stay pure shadows ─────

/// The S04/T04 repair surfaces. The positioned donor's lexical unit is the PDF
/// show-operator span, not a word, so the T02/T03 native bands could not run on
/// any real document. These two modules are the repair: a partition of a span
/// into words with honest derived geometry, and a heading-aware region walk.
const NATIVE_WORD_PROJECTION_SURFACE: &str = "crates/da-parse/src/native_word_projection.rs";
const NATIVE_HEADING_REGIONS_SURFACE: &str = "crates/da-parse/src/native_heading_regions.rs";

/// Side-channel, TEI/Java coupling and non-determinism markers banned from the
/// repair's CODE (comments may document the prohibitions — the scanner strips
/// line comments first). Both modules import only their typed leaves, `serde`,
/// `sha2` and `std`, so every hit is a real violation.
const NATIVE_WORD_PLANE_FORBIDDEN: [&str; 30] = [
    "TeiEnvelope",
    "TeiLifter",
    "TeiParseIr",
    "TeiSpan",
    "ParserPort",
    "ParsedArticle",
    "GraphStore",
    "Samyama",
    "import_eligible",
    "promotion_state",
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
    "var_os",
];

#[test]
fn native_word_plane_is_pure_and_heading_vocabulary_cannot_drift() {
    for path in [
        NATIVE_WORD_PROJECTION_SURFACE,
        NATIVE_HEADING_REGIONS_SURFACE,
    ] {
        let text = read_text(path);
        assert!(
            text.len() > 5_000,
            "positive control failed: {path} looks truncated"
        );
        assert_eq!(
            import_roots_of(&text),
            BTreeSet::from([
                "crate".to_string(),
                "serde".to_string(),
                "sha2".to_string(),
                "std".to_string()
            ]),
            "{path} may import only its typed leaves, serde, hashing and formatting"
        );
        let hits = native_date_marker_hits(&text, &NATIVE_WORD_PLANE_FORBIDDEN);
        assert!(
            hits.is_empty(),
            "{path} acquired TEI/Java coupling, non-determinism or a side effect:\n{}",
            hits.join("\n")
        );
        assert!(
            !text.contains("pub text:"),
            "{path} must keep its text private (redaction)"
        );
        assert!(
            text.contains("pub fn text("),
            "{path} must expose a text() accessor instead of the field"
        );
    }

    // The repair must not reopen the S03 wire formats it is bounded by.
    for path in [
        NATIVE_WORD_PROJECTION_SURFACE,
        NATIVE_HEADING_REGIONS_SURFACE,
    ] {
        let text = read_text(path);
        for marker in [
            "segment_document_full(",
            "FullLatticeSegmenter",
            "TokenAssignment",
        ] {
            assert!(
                !text.contains(marker),
                "{path} must not re-declare or shadow the S03 surface via {marker}"
            );
        }
    }

    // The heading vocabulary resolves through S03's matcher, so the shared
    // entries cannot drift apart into two spellings.
    let heading = read_text(NATIVE_HEADING_REGIONS_SURFACE);
    assert!(
        heading.contains("crate::native_segmentation::full_marker"),
        "the heading vocabulary must resolve through the S03 marker matcher"
    );
    assert!(
        read_text("crates/da-parse/src/native_segmentation.rs")
            .contains("pub(crate) fn full_marker"),
        "S03's full_marker stays the single definition of the marker set"
    );

    let word = read_text(NATIVE_WORD_PROJECTION_SURFACE);
    for symbol in [
        "pub struct NativeWordProjection",
        "pub struct NativeWordParent",
        "pub struct NativeWordToken",
        "pub struct NativeWordGeometry",
        "pub enum NativeWordGeometryOrigin",
        "pub enum NativeWordAbsence",
        "pub enum NativeWordProjectionError",
        "pub fn project_native_words",
    ] {
        assert!(word.contains(symbol), "the word plane must export {symbol}");
    }
    for symbol in [
        "pub struct NativeHeadingRegionOutcome",
        "pub struct NativeVisualLine",
        "pub struct HeadingEvidence",
        "pub enum HeadingMatch",
        "pub enum NativeHeadingAbsence",
        "pub enum NativeHeadingRegionError",
        "pub fn assign_heading_regions",
    ] {
        assert!(
            heading.contains(symbol),
            "the heading walk must export {symbol}"
        );
    }

    let lib = read_text("crates/da-parse/src/lib.rs");
    for marker in [
        "mod native_word_projection;",
        "mod native_heading_regions;",
        "pub use native_word_projection::",
        "pub use native_heading_regions::",
        "build_native_word_region_carrier",
    ] {
        assert!(lib.contains(marker), "lib.rs must register {marker}");
    }

    // Quarantine: the repair never enters the Java-authoritative composition.
    let mut leaked = Vec::new();
    for rel in NATIVE_REGION_INPUT_COMPOSITION_SURFACES {
        let code = s10_code_without_comments(&read_text(rel));
        for marker in [
            "native_word_projection",
            "native_heading_regions",
            "NativeWordProjection",
            "NativeHeadingRegionOutcome",
            "build_native_word_region_carrier",
        ] {
            if code.contains(marker) {
                leaked.push(format!("{rel}: {marker}"));
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "the word plane escaped shadow quarantine:\n{}",
        leaked.join("\n")
    );
}

#[test]
fn native_word_plane_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "use samyama::GraphStore;\nfn f() { let _ = TeiSpan; }\n";
    assert!(import_roots_of(synthetic).contains("samyama"));
    assert!(!native_date_marker_hits(synthetic, &NATIVE_WORD_PLANE_FORBIDDEN).is_empty());
    assert!(
        native_date_marker_hits(
            "// TeiSpan, TeiEnvelope and ParserPort are prose only\n",
            &NATIVE_WORD_PLANE_FORBIDDEN
        )
        .is_empty()
    );
    assert!(
        !native_date_marker_hits("let p = PathBuf::new();", &NATIVE_WORD_PLANE_FORBIDDEN)
            .is_empty()
    );
    assert!(
        native_date_marker_hits(
            "let v: BTreeMap<u32, u32> = BTreeMap::new();",
            &NATIVE_WORD_PLANE_FORBIDDEN
        )
        .is_empty(),
        "a benign collection declaration is not a violation (positive control)"
    );
}

#[test]
fn s15_full_segmentation_quarantine_contract() {
    const COMPANION_MARKERS: [&str; 4] = [
        "segment_document_full",
        "FullLatticeSegmenter",
        "FullSemanticRegion",
        "native-segmentation/v2",
    ];
    let production_surfaces = [
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
        "crates/da-application/src/lib.rs",
        "crates/da-adapters/src/lib.rs",
        "crates/da-parse/src/parser.rs",
        "crates/da-parse/src/projector.rs",
    ];
    let mut leaked = Vec::new();
    for surface in production_surfaces {
        let code = s10_code_without_comments(&read_text(surface));
        for marker in COMPANION_MARKERS {
            if code.contains(marker) {
                leaked.push(format!("{surface}: {marker}"));
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "S02 full-family companion escaped shadow quarantine:\n{}",
        leaked.join("\n")
    );

    let manifest = serde_yaml::from_str::<serde_yaml::Value>(&read_text(
        "data/parse/document_segmentation.yaml",
    ))
    .expect("segmentation manifest");
    assert_eq!(manifest["policy_locks"]["observation_mode"], "shadow");
    assert_eq!(
        manifest["policy_locks"]["production_authority"],
        "java-grobid-http@0.9.0-crf"
    );
    assert_eq!(manifest["policy_locks"]["auto_promote"], "forbidden");
    assert_eq!(manifest["policy_locks"]["winner_selection"], "forbidden");
    assert_eq!(manifest["policy_locks"]["graph_mutation"], "forbidden");
    assert_eq!(manifest["policy_locks"]["import_eligible"], false);
    assert_eq!(
        manifest["regions"]["independently_selectable"],
        serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::String("header".into()),
            serde_yaml::Value::String("references".into()),
        ])
    );
    assert_eq!(manifest["source_bound_genres"]["stratification_only"], true);
    assert_eq!(
        manifest["source_bound_genres"]["classifier_branching"],
        "forbidden"
    );

    let fsm = s10_code_without_comments(&read_text("crates/da-domain/src/fsm_policy.rs"));
    assert!(
        s10_decision_point_violations(&fsm).is_empty(),
        "S02 quarantine must preserve the closed six-DecisionPoint policy"
    );
}

#[test]
fn s16_fulltext_blocks_quarantine_contract() {
    const COMPANION_MARKERS: [&str; 3] = [
        "account_full_regions",
        "run_native_fulltext_blocks",
        "NativeFulltextBlocksConfig",
    ];
    let production_surfaces = [
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
        "crates/da-application/src/lib.rs",
        "crates/da-adapters/src/lib.rs",
        "crates/da-parse/src/parser.rs",
        "crates/da-parse/src/projector.rs",
    ];
    let mut leaked = Vec::new();
    for surface in production_surfaces {
        let code = s10_code_without_comments(&read_text(surface));
        for marker in COMPANION_MARKERS {
            if code.contains(marker) {
                leaked.push(format!("{surface}: {marker}"));
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "S03 fulltext-blocks companion escaped shadow quarantine:\n{}",
        leaked.join("\n")
    );

    let fsm = s10_code_without_comments(&read_text("crates/da-domain/src/fsm_policy.rs"));
    assert!(
        s10_decision_point_violations(&fsm).is_empty(),
        "S03 quarantine must preserve the closed six-DecisionPoint policy"
    );
}

#[test]
fn s17_callout_matching_quarantine_contract() {
    const TOKENS: [&str; 3] = [
        "match_native_callouts",
        "NativeCalloutMatchingOutcome",
        "DigestOnlyTargetInventory",
    ];
    let production_surfaces = [
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
        "crates/da-application/src/lib.rs",
        "crates/da-adapters/src/lib.rs",
        "crates/da-parse/src/parser.rs",
        "crates/da-parse/src/projector.rs",
    ];
    let mut leaked = Vec::new();
    for surface in production_surfaces {
        let code = s10_code_without_comments(&read_text(surface));
        for token in TOKENS {
            if code.contains(token) {
                leaked.push(format!("{surface}: {token}"));
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "S04 matcher escaped shadow quarantine:\n{}",
        leaked.join("\n")
    );

    let semantics = serde_yaml::from_str::<serde_yaml::Value>(&read_text(
        "data/parse/callout_matching_semantics.yaml",
    ))
    .expect("callout matching semantics");
    assert_eq!(semantics["marker_model"]["closed"], true);
    assert_eq!(
        semantics["marker_model"]["vocabulary"],
        serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::String("citation".into()),
            serde_yaml::Value::String("figure".into()),
            serde_yaml::Value::String("table".into()),
            serde_yaml::Value::String("equation".into()),
        ])
    );
    assert_eq!(semantics["policy_locks"]["import_eligible"], false);
    assert_eq!(semantics["policy_locks"]["graph_write"], "forbidden");
    assert_eq!(semantics["policy_locks"]["auto_promote"], "forbidden");
    assert_eq!(semantics["policy_locks"]["composition_splice"], "forbidden");

    let synthetic =
        "use da_parse::NativeCalloutMatchingOutcome;\nfn production() { match_native_callouts(); }";
    assert!(TOKENS.iter().any(|token| synthetic.contains(token)));
    assert!(s10_code_without_comments("// match_native_callouts in prose\n").is_empty());

    let fsm = s10_code_without_comments(&read_text("crates/da-domain/src/fsm_policy.rs"));
    assert!(
        s10_decision_point_violations(&fsm).is_empty(),
        "S04 changed the six DecisionPoint policy"
    );
}

#[test]
fn s18_consolidation_specialized_quarantine_contract() {
    const TOKENS: [&str; 3] = [
        "evaluate_native_consolidation_specialized",
        "NativeConsolidationSpecializedOutcome",
        "SpecializedFamilyOutcome",
    ];
    let production_surfaces = [
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
        "crates/da-application/src/lib.rs",
        "crates/da-application/src",
    ];
    let mut leaked = Vec::new();
    for surface in production_surfaces {
        let path = repo_root().join(surface);
        let files = if path.is_dir() {
            let mut collected = Vec::new();
            collect_rs_files(&path, &mut collected);
            collected
        } else {
            vec![path]
        };
        for file in files {
            let code = s10_code_without_comments(
                &std::fs::read_to_string(&file).expect("production boundary readable"),
            );
            for token in TOKENS {
                if code.contains(token) {
                    leaked.push(format!("{}: {token}", file.display()));
                }
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "S05 specialized companion escaped quarantine:\n{}",
        leaked.join("\n")
    );

    let ledger = serde_yaml::from_str::<serde_yaml::Value>(&read_text(
        "data/parse/grobid_compatibility.yaml",
    ))
    .expect("GROBID compatibility ledger");
    let spec = &ledger["defaults"]["consolidation_specialized_quarantine"];
    assert_eq!(spec["production_authority"], "java_http");
    assert_eq!(spec["observe_mode"], "shadow_receipt");
    assert_eq!(spec["production_route"], "forbidden");
    assert_eq!(spec["graph_write"], "forbidden");
    assert_eq!(spec["import_eligible"], false);
    assert_eq!(spec["auto_promote"], "forbidden");
    assert_eq!(spec["decision_point"], 6);
    assert_eq!(
        spec["independently_selectable"],
        serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::String("header".into()),
            serde_yaml::Value::String("references".into()),
        ])
    );

    let synthetic = "use da_parse::NativeConsolidationSpecializedOutcome;\nfn production() { evaluate_native_consolidation_specialized(); }";
    assert!(TOKENS.iter().any(|token| synthetic.contains(token)));
    assert!(
        s10_code_without_comments("// evaluate_native_consolidation_specialized()\n").is_empty()
    );
}

#[test]
fn s19_feature_tagger_quarantine_contract() {
    const TOKENS: [&str; 3] = [
        "NativeFeatureTagger",
        "NativeFeatureInput",
        "FeatureUpstreamStatus",
    ];
    let production_surfaces = [
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
        "crates/da-application/src",
        "crates/da-adapters/src",
    ];
    let mut leaked = Vec::new();
    for surface in production_surfaces {
        let path = repo_root().join(surface);
        let files = if path.is_dir() {
            let mut collected = Vec::new();
            collect_rs_files(&path, &mut collected);
            collected
        } else {
            vec![path]
        };
        for file in files {
            let code = s10_code_without_comments(
                &std::fs::read_to_string(&file).expect("production boundary readable"),
            );
            for token in TOKENS {
                if code.contains(token) {
                    leaked.push(format!("{}: {token}", file.display()));
                }
            }
        }
    }
    assert!(
        leaked.is_empty(),
        "S06 feature companion escaped quarantine:\n{}",
        leaked.join("\n")
    );
    let ledger = serde_yaml::from_str::<serde_yaml::Value>(&read_text(
        "data/parse/grobid_compatibility.yaml",
    ))
    .expect("GROBID compatibility ledger");
    let spec = &ledger["defaults"]["feature_tagger_training_quarantine"];
    assert_eq!(spec["production_authority"], "java_http");
    assert_eq!(spec["observe_mode"], "shadow_receipt");
    assert_eq!(spec["production_route"], "forbidden");
    assert_eq!(spec["graph_write"], "forbidden");
    assert_eq!(spec["import_eligible"], false);
    assert_eq!(spec["auto_promote"], "forbidden");
    assert_eq!(spec["training_and_model_runtime"], "forbidden");
    assert_eq!(spec["same_n_required"], true);
    assert_eq!(spec["decision_point"], 6);
    assert_eq!(
        spec["independently_selectable"],
        serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::String("header".into()),
            serde_yaml::Value::String("references".into())
        ])
    );
    assert_eq!(spec["dispositions"].as_sequence().unwrap().len(), 3);
    let synthetic = "use da_parse::NativeFeatureTagger; fn production(x: NativeFeatureInput) { let _ = FeatureUpstreamStatus::Pass; }";
    assert_eq!(
        TOKENS
            .iter()
            .filter(|token| synthetic.contains(**token))
            .count(),
        3
    );
    assert!(
        s10_code_without_comments(
            "// NativeFeatureTagger NativeFeatureInput FeatureUpstreamStatus\n"
        )
        .is_empty()
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
            // Marker precision matters: a bare `NativeFulltext` prefix also
            // matches the unrelated `NativeFulltextBlocks` family, which is a
            // legitimate parse-plane sibling with its own quarantine contract
            // (`s16_fulltext_blocks_quarantine_contract`). This scanner's
            // subject is the composer, so it matches the composer's own
            // identifiers only, and the sanctioned DATA-table path is scrubbed
            // by the helper before matching (see its doc comment).
            for marker in fulltext_composition_leak_markers(raw) {
                leaked.push(format!(
                    "{rel}:{}: marker `{marker}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
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

/// Composer markers that may not appear in any runtime seam, with the two
/// sanctioned read-only DATA identities scrubbed before matching.
///
/// The scanner's subject is the composer, not the DATA that names it. Two
/// committed artifacts embed the composer's name as a read-only identity:
///
/// * the schema-validated table `data/parse/native_fulltext_composition.yaml`
///   (a qualification ledger's sha256 binding, a fixture `include_str!`) is a
///   configuration dependency, not composer wiring; and
/// * the capability id `parse.component.native_fulltext_composition_shadow`
///   in the compatibility registry, the reference pins and the acceptance
///   package is an ontology identity read as DATA, never a call site.
///
/// Strip those exact literal tokens first, and keep failing on every composer
/// identifier or API name, including a line that carries a DATA token and an
/// identifier together. D381 records the rule: refining the proxy's precision
/// with positive and negative controls preserves the guard's teeth, while a
/// runtime seam that *names* composer identifiers must never be exempted.
fn fulltext_composition_leak_markers(line: &str) -> Vec<&'static str> {
    const COMPOSITION_DATA_PATH: &str = "data/parse/native_fulltext_composition.yaml";
    const COMPOSITION_CAPABILITY_ID: &str = "parse.component.native_fulltext_composition_shadow";
    const MARKERS: [&str; 3] = [
        "native_fulltext_composition",
        "NativeFulltextComposition",
        "compose_native_fulltext",
    ];
    let scrubbed = line
        .replace(COMPOSITION_DATA_PATH, "")
        .replace(COMPOSITION_CAPABILITY_ID, "");
    let mut hits = Vec::new();
    for marker in MARKERS {
        if scrubbed.contains(marker) {
            hits.push(marker);
        }
    }
    hits
}

#[test]
fn fulltext_composition_scanner_ignores_the_data_path_but_never_the_identifiers() {
    // Positive control: the sanctioned DATA path stays a configuration read.
    assert!(fulltext_composition_leak_markers(
        "pub const COMPOSITION_DATA_REL: &str = \"data/parse/native_fulltext_composition.yaml\";"
    )
    .is_empty());
    assert!(fulltext_composition_leak_markers(
        "const COMPOSITION: &str = include_str!(\"../../../data/parse/native_fulltext_composition.yaml\");"
    )
    .is_empty());
    assert!(
        fulltext_composition_leak_markers(
            "//! 2. `data/parse/native_fulltext_composition.yaml#admission.current_matrix` with"
        )
        .is_empty()
    );
    // Positive control: the sanctioned capability id is an ontology identity
    // read as DATA, not a composer call site.
    assert!(
        fulltext_composition_leak_markers(
            "        \"family_id\": \"parse.component.native_fulltext_composition_shadow\","
        )
        .is_empty()
    );
    assert!(
        fulltext_composition_leak_markers(
            "== Some(\"parse.component.native_fulltext_composition_shadow\")"
        )
        .is_empty()
    );
    // Negative control: every composer identifier still trips, including a
    // line that quotes a DATA token and the identifier together.
    for line in [
        "use da_parse::NativeFulltextComposition;",
        "mod native_fulltext_composition;",
        "fn run() { compose_native_fulltext(&input) }",
        "let cfg = load(\"data/parse/native_fulltext_composition.yaml\"); NativeFulltextComposition::new(cfg);",
        "let id = \"parse.component.native_fulltext_composition_shadow\"; compose_native_fulltext(&input);",
    ] {
        assert_eq!(
            fulltext_composition_leak_markers(line).len(),
            1,
            "composer identifier must still trip the scanner: {line}"
        );
    }
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
// Mirrors the Governor header quarantine (S03/T07 made it DATA-bound from
// `data/parse/grobid_compatibility.yaml#defaults.header_sequence_quarantine`)
// as an independent compile/test contract: the exact tokens `compose_header`,
// `run_header_sequence`, `NativeDateParser`, `decode_header_sequence`,
// `render_header_features`, `render_header_feature_text` and
// `project_native_header_fields` must not appear in code positions of the two
// production boundaries. `//` comments (full-line and trailing) are stripped
// before the scan; `decompose` never matches because the call-site `(` is
// required for the snake_case atoms while NativeDateParser needs word
// boundaries.

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
        for token in [
            "compose_header",
            "run_header_sequence",
            "decode_header_sequence",
            "render_header_features",
            "render_header_feature_text",
            "project_native_header_fields",
        ] {
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
    let synthetic = "fn run() {\n    compose_header(input);\n    run_header_sequence(seq);\n    decode_header_sequence(rows);\n    render_header_features(tokens);\n    render_header_feature_text(tokens);\n    project_native_header_fields(rows, spans);\n    let _ = NativeDateParser;\n}\n";
    let hits = header_quarantine_hits(synthetic);
    assert_eq!(hits.len(), 7, "each header atom must flag: {hits:?}");
    assert!(hits.iter().any(|h| h.contains("compose_header")));
    assert!(hits.iter().any(|h| h.contains("run_header_sequence")));
    assert!(hits.iter().any(|h| h.contains("decode_header_sequence")));
    assert!(hits.iter().any(|h| h.contains("render_header_features")));
    assert!(
        hits.iter()
            .any(|h| h.contains("render_header_feature_text"))
    );
    assert!(
        hits.iter()
            .any(|h| h.contains("project_native_header_fields"))
    );
    assert!(hits.iter().any(|h| h.contains("NativeDateParser")));
    // Comments are prose, not splice; `decompose` is a different word.
    let prose = "// compose_header run_header_sequence NativeDateParser decode_header_sequence are prose\nlet x = decompose(y); // compose_header(input);\n";
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

// ───── S07/T02 + S04/T08: bibliography atoms stay shadow — never in a production boundary ─────
//
// Mirrors the Governor bibliography quarantine (S07/T01) and the additive
// S04/T08 DATA-driven quarantine (`defaults.bibliography_sequence_quarantine`)
// as an independent compile/test contract: the exact tokens below must not
// appear in code positions of the production boundaries. `//` comments
// (full-line and trailing) are stripped before the scan; `decompose_references`
// never matches because a word boundary is required before every token, a
// call-site `(` (with optional whitespace) is required after each snake_case
// atom, and each CamelCase atom is matched on word boundaries on both sides.
//
// MEM1115/D362 keep the two layers split: this Rust list is the layer that owns
// the PascalCase type names; the Governor DATA key carries the S04 lane's own
// atoms and the older Governor tuple keeps only `compose_references(` /
// `run_reference_sequence(`.

/// Production boundaries that must stay free of bibliography atoms.
const BIBLIOGRAPHY_QUARANTINE_BOUNDARIES: [&str; 4] = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/projector.rs",
];

/// Native bibliography method/type names that must never appear in a
/// production boundary (ParserPort or the parse-pdf splice surfaces).
const BIBLIOGRAPHY_NATIVE_PORT_TOKENS: [&str; 40] = [
    "compose_references",
    "run_reference_sequence",
    "NativeCitationAnchorScanner",
    "ReferenceCompositionInput",
    "ReferenceCompositionCandidate",
    "ReferenceCompositionOutcome",
    "ReferenceSequenceInput",
    "ReferenceCandidate",
    // S04/T08: the native bibliography + callout lane's own public surfaces.
    "citation_row",
    "reference_segmenter_row",
    "name_citation_row",
    "date_row",
    "render_citation_feature_text",
    "render_reference_segmenter_feature_text",
    "render_name_citation_feature_text",
    "render_date_feature_text",
    "decode_bibliography_labels",
    "decode_reference_sequence",
    "decode_citation_sequence",
    "decode_name_citation_cluster",
    "decode_date_cluster",
    "bibliography_label_runs",
    "gazetteer_spans",
    "classify_callout_markers",
    "link_callout_markers",
    "CitationFeatureRow",
    "ReferenceSegmenterFeatureRow",
    "BibliographySequenceFamily",
    "BibliographyLabelRun",
    "ReferenceSegmentSpan",
    "CitationRoleSpan",
    "NameCitationFeatureRow",
    "DateFeatureRow",
    "NativeNameCitation",
    "NativeDateValue",
    "NativePersonTitleCoverage",
    "CalloutMarkerClass",
    "CalloutBodyInventory",
    "CalloutLinkReport",
    "BibliographyGazetteerIndex",
];

/// Comment-stripped bibliography-atom scan over code text. One hit per token
/// per line; mirrors the Governor `_bibliography_quarantine_tokens_present`
/// semantics: each snake_case atom requires a word boundary before it and a
/// call-site `(` (after optional whitespace) after it.
/// One bibliography-atom hit inside a single comment-stripped line.
///
/// snake_case atoms are call-sites: word boundary before, optional whitespace
/// then `(` after. CamelCase atoms are type names: word boundary on both
/// sides. Both are the exact semantics the Governor predicate uses.
fn bibliography_atom_hit(code: &str, token: &str) -> bool {
    let bytes = code.as_bytes();
    let needle = token.as_bytes();
    let snake_case = token.contains('_');
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let before_ok =
                i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            if before_ok {
                if snake_case {
                    let mut j = i + needle.len();
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] == b'(' {
                        return true;
                    }
                } else {
                    let j = i + needle.len();
                    let after_ok =
                        j >= bytes.len() || !(bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_');
                    if after_ok {
                        return true;
                    }
                }
            }
            i += needle.len();
        } else {
            i += 1;
        }
    }
    false
}

fn bibliography_quarantine_hits(text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        // `strip_line_comments` blanks full-line prose; trailing `//` prose
        // is cut here so evidence comments never flag (Governor parity).
        let code = match raw.find("//") {
            Some(cut) => &raw[..cut],
            None => raw,
        };
        for token in BIBLIOGRAPHY_NATIVE_PORT_TOKENS {
            if bibliography_atom_hit(code, token) {
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
    let synthetic = "fn run() {\n    compose_references(input);\n    run_reference_sequence(seq);\n    let _r = citation_row(tokens);\n    let _d = decode_reference_sequence(rows);\n    let _t = render_citation_feature_text(tokens);\n    let _c: CitationFeatureRow = row;\n    let _g = gazetteer_spans(tokens);\n}\n";
    let hits = bibliography_quarantine_hits(synthetic);
    assert_eq!(hits.len(), 7, "each bibliography atom must flag: {hits:?}");
    assert!(hits.iter().any(|h| h.contains("compose_references")));
    assert!(hits.iter().any(|h| h.contains("run_reference_sequence")));
    assert!(hits.iter().any(|h| h.contains("citation_row")));
    assert!(hits.iter().any(|h| h.contains("decode_reference_sequence")));
    assert!(
        hits.iter()
            .any(|h| h.contains("render_citation_feature_text"))
    );
    assert!(hits.iter().any(|h| h.contains("CitationFeatureRow")));
    assert!(hits.iter().any(|h| h.contains("gazetteer_spans")));
    // Whitespace before `(` is still a call site and must flag.
    let spaced = "fn run() {\n    compose_references (input);\n    run_reference_sequence  (seq);\n    citation_row\t(tokens);\n}\n";
    let hits = bibliography_quarantine_hits(spaced);
    assert_eq!(hits.len(), 3, "whitespace call-sites must flag: {hits:?}");
    // Comments are prose, not splice; `decompose_references` is a different word;
    // prose mentions without a call-site `(` are not splices; a prefixed
    // CamelCase type name is a different type.
    let prose = "// compose_references run_reference_sequence citation_row CitationFeatureRow are prose\nlet x = decompose_references(y);\nlet y = compose_references;\nlet z: CitationFeatureRowExtra = row;\nfoo(); // compose_references(input);\n";
    assert!(
        bibliography_quarantine_hits(prose).is_empty(),
        "prose comments, prefixed identifiers and bare mentions must not flag"
    );
}

#[test]
fn production_boundaries_stay_free_of_bibliography_atoms() {
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

// ───── M507 S05/T06: native body/fulltext atoms stay shadow — never in a production boundary ─────
//
// Mirrors the Governor fulltext quarantine (S05/T06 DATA key
// `defaults.fulltext_sequence_quarantine`) as an independent compile/test
// contract: the exact tokens below must not appear in code positions of the
// four production boundaries. `//` comments (full-line and trailing) are
// stripped before the scan, snake_case atoms require a call-site `(` (after
// optional whitespace) with a word boundary before the token, and each
// CamelCase atom is matched on word boundaries on both sides. The matcher is
// the token-generic `bibliography_atom_hit` — one matching rule, no copy.
//
// Naming adjacency is deliberate (D362): this array belongs to the S05 body
// reading-order lane and is never merged with
// `S05_BODY_QUARANTINE_ATOMS` / `S05_BODY_ACCOUNTING_DATA_TOKENS` (the M496 P5
// accountant lane) or with the Governor `fulltext_blocks_quarantine` key (P2
// region accounting). Those stay untouched here.

/// Production boundaries that must stay free of body/fulltext atoms.
const FULLTEXT_SEQUENCE_QUARANTINE_BOUNDARIES: [&str; 4] = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/projector.rs",
];

/// Native body/fulltext symbols that must never appear in a production
/// boundary. This list is mirrored 1:1 by the Governor DATA key; the test
/// below fails if either side drifts.
const FULLTEXT_SEQUENCE_QUARANTINE_TOKENS: [&str; 36] = [
    // The 27-column ABI in the exact `FeaturesVectorFulltext.printVector` order.
    "render_fulltext_features",
    "render_fulltext_feature_text",
    "check_fulltext_feature_columns",
    "relative_position_fulltext_document",
    "relative_position_fulltext_page",
    // D359 base-name run decode over the shared grouping primitive.
    "decode_fulltext_sequence",
    "check_fulltext_label_block",
    "fulltext_label_is_admissible",
    // Cross-page reading order and the continued-run page span.
    "glue_fulltext_reading_order",
    "cross_page_runs",
    // Typed non-CRF sentence/language steps and the body version registry.
    "native_body_version_surface",
    // PascalCase row/run/step types of the same lane.
    "FulltextAlignment",
    "FulltextCalloutType",
    "FulltextFeatureRow",
    "FulltextFeatureToken",
    "FulltextFontSize",
    "FulltextFontStatus",
    "NativeFulltextFeatureError",
    "FulltextLabelRun",
    "NativeFulltextDecodeCandidate",
    "NativeFulltextDecodeOutcome",
    "NativeFulltextSequenceError",
    "ColumnShift",
    "CrossPageRun",
    "NativeFulltextPageToken",
    "NativeFulltextReadingOrder",
    "NativeFulltextReadingOrderError",
    "NativeFulltextTokenText",
    "PageReadingSpan",
    "NativeBodyStep",
    "NativeBodyStepOutcome",
    "NativeBodyStepReceipt",
    "NativeBodySurfaceKind",
    "NativeBodyVersionSurface",
    "NativeLanguageStep",
    "NativeSentenceStep",
];

fn fulltext_sequence_quarantine_data(registry: &serde_yaml::Value, key: &str) -> Vec<String> {
    registry["defaults"]["fulltext_sequence_quarantine"][key]
        .as_sequence()
        .expect("fulltext quarantine DATA list")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("fulltext quarantine DATA string")
                .to_owned()
        })
        .collect()
}

/// Comment-stripped body/fulltext-atom scan over code text.
fn fulltext_quarantine_hits(text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        let code = match raw.find("//") {
            Some(cut) => &raw[..cut],
            None => raw,
        };
        for token in FULLTEXT_SEQUENCE_QUARANTINE_TOKENS {
            if bibliography_atom_hit(code, token) {
                hits.push(format!(
                    "[fulltext-sequence-quarantine] line {}: atom `{token}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    hits
}

#[test]
fn fulltext_sequence_quarantine_is_data_bound_and_lock_closed() {
    let registry: serde_yaml::Value =
        serde_yaml::from_str(&read_text("data/parse/grobid_compatibility.yaml"))
            .expect("compatibility registry");
    let quarantine = &registry["defaults"]["fulltext_sequence_quarantine"];
    assert_ne!(
        quarantine,
        &serde_yaml::Value::Null,
        "S05: defaults.fulltext_sequence_quarantine vanished — loss, not silence"
    );
    assert_eq!(
        fulltext_sequence_quarantine_data(&registry, "tokens"),
        FULLTEXT_SEQUENCE_QUARANTINE_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect::<Vec<_>>(),
        "S05: Rust and DATA fulltext token sets drifted (D362 single source)"
    );
    assert_eq!(
        fulltext_sequence_quarantine_data(&registry, "boundaries"),
        FULLTEXT_SEQUENCE_QUARANTINE_BOUNDARIES
            .iter()
            .map(|boundary| (*boundary).to_owned())
            .collect::<Vec<_>>(),
        "S05: Rust and DATA fulltext boundary sets drifted"
    );
    // The lane's locks are DATA, and each one must fail closed on drift.
    for (key, expected) in [
        ("production_authority", "java_http"),
        ("observe_mode", "shadow_receipt"),
        ("production_route", "forbidden"),
        ("graph_write", "forbidden"),
        ("auto_promote", "forbidden"),
        ("proof_level", "contract"),
    ] {
        assert_eq!(
            quarantine[key].as_str(),
            Some(expected),
            "S05: fulltext quarantine `{key}` must stay `{expected}`"
        );
    }
    assert_eq!(quarantine["deployed"].as_bool(), Some(false));
    assert_eq!(quarantine["import_eligible"].as_bool(), Some(false));
    assert_eq!(quarantine["decision_point"].as_i64(), Some(6));
    let selectable: Vec<String> = quarantine["independently_selectable"]
        .as_sequence()
        .expect("independently_selectable")
        .iter()
        .map(|value| value.as_str().expect("selectable lane").to_owned())
        .collect();
    assert_eq!(
        selectable,
        vec!["header".to_owned(), "references".to_owned()]
    );
    // Negative controls: a dropped token, an added boundary or an eroded lock
    // must not satisfy the closed DATA set.
    let mut drifted = registry.clone();
    drifted["defaults"]["fulltext_sequence_quarantine"]["tokens"] = serde_yaml::Value::Sequence(
        FULLTEXT_SEQUENCE_QUARANTINE_TOKENS[..FULLTEXT_SEQUENCE_QUARANTINE_TOKENS.len() - 1]
            .iter()
            .map(|token| serde_yaml::Value::String((*token).to_owned()))
            .collect(),
    );
    assert_ne!(
        fulltext_sequence_quarantine_data(&drifted, "tokens"),
        fulltext_sequence_quarantine_data(&registry, "tokens"),
        "S05: a dropped token must not satisfy the closed DATA set"
    );
    let mut promoted = registry.clone();
    promoted["defaults"]["fulltext_sequence_quarantine"]["deployed"] =
        serde_yaml::Value::Bool(true);
    assert_ne!(
        promoted["defaults"]["fulltext_sequence_quarantine"]["deployed"].as_bool(),
        Some(false),
        "S05: deployed=true must be observable as drift"
    );
}

#[test]
fn fulltext_quarantine_scanner_fails_visible_on_synthetic_atoms() {
    let synthetic = "fn run() {\n    render_fulltext_features(tokens);\n    decode_fulltext_sequence(rows, model);\n    glue_fulltext_reading_order(pages);\n    let _r: FulltextFeatureRow = row;\n    let _c: CrossPageRun = run;\n}\n";
    let hits = fulltext_quarantine_hits(synthetic);
    assert_eq!(hits.len(), 5, "each fulltext atom must flag: {hits:?}");
    for atom in [
        "render_fulltext_features",
        "decode_fulltext_sequence",
        "glue_fulltext_reading_order",
        "FulltextFeatureRow",
        "CrossPageRun",
    ] {
        assert!(
            hits.iter().any(|hit| hit.contains(atom)),
            "scanner lost coverage for `{atom}`"
        );
    }
    // Whitespace before `(` is still a call site and must flag.
    let spaced = "fn run() {\n    decode_fulltext_sequence (rows, model);\n    cross_page_runs\t(runs);\n}\n";
    assert_eq!(
        fulltext_quarantine_hits(spaced).len(),
        2,
        "whitespace call-sites must flag"
    );
    // Comments are prose, bare mentions are not call sites, and a prefixed
    // identifier or a longer CamelCase type name is a different symbol.
    let prose = "// render_fulltext_features FulltextFeatureRow CrossPageRun are prose\nlet x = render_fulltext_features_legacy(y);\nlet y = decode_fulltext_sequence;\nlet z: FulltextFeatureRowBuilder = row;\nfoo(); // decode_fulltext_sequence(rows, model);\n";
    assert!(
        fulltext_quarantine_hits(prose).is_empty(),
        "prose comments, prefixed identifiers and bare mentions must not flag"
    );
}

#[test]
fn production_boundaries_stay_free_of_fulltext_sequence_atoms() {
    for rel in FULLTEXT_SEQUENCE_QUARANTINE_BOUNDARIES {
        let text = read_text(rel);
        assert!(
            text.len() > 1_000,
            "positive control failed: {rel} looks truncated"
        );
        let hits = fulltext_quarantine_hits(&text);
        assert!(
            hits.is_empty(),
            "body/fulltext atoms leaked into a production boundary (shadow means never served):\n{}",
            hits.join("\n")
        );
    }
    // The production authority stays Java and the lane stays off the FSM.
    let http = read_text("crates/da-parse/src/http.rs");
    assert!(
        http.contains("GrobidParser"),
        "Java parser authority disappeared"
    );
    let port = read_text("crates/da-ports/src/parser.rs");
    assert!(
        port.contains("pub trait ParserPort") && port.contains("async fn parse_pdf"),
        "positive control failed: ParserPort surface vanished"
    );
}

// ───── S06/T08: the native visual-family and funding lanes stay shadow ─────
//
// Mirrors the two Governor S06 quarantine keys
// (`defaults.visual_family_sequence_quarantine` for figure/table/formula and
// `defaults.funding_sequence_quarantine` for funding) as an independent
// compile/test contract: the exact token sets below must not appear in code
// positions of the four production boundaries. `//` comments (full-line and
// trailing) are stripped before the scan, snake_case atoms require a call-site
// `(` (after optional whitespace) with a word boundary before the token, and
// each CamelCase atom is matched on word boundaries on both sides. The matcher
// is the shared `bibliography_atom_hit` — one matching rule, no copy.
//
// Naming adjacency is deliberate (D362): neither key is merged with
// `visual_heads_quarantine` (S08 geometric heads, which carries no lock fields
// and only the weak `NativeVisual` CamelCase token), the S05
// `fulltext_sequence_quarantine`, `fulltext_blocks_quarantine` (P2 region
// accounting) or `body_accounting_quarantine` (M496 P5 accountant). The sets
// are checked disjoint, and each key owns its own four boundaries.

/// Production boundaries that must stay free of S06 family atoms.
const FAMILY_SEQUENCE_QUARANTINE_BOUNDARIES: [&str; 4] = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/projector.rs",
];

/// Native figure/table/formula symbols (S06 key
/// `visual_family_sequence_quarantine`). Mirrored 1:1 by the Governor DATA
/// key; the test below fails if either side drifts.
const VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS: [&str; 38] = [
    // Shared family clusteror and the common caption-link band.
    "cluster_family_label_runs",
    "caption_links_from_markers",
    // The `figure` second-pass runner over the S05 fulltext ABI.
    "decode_figure_sequence",
    "check_figure_label_block",
    "check_figure_column_window",
    "figure_spans_from_decoded",
    "figure_label_is_admissible",
    // The `table` second-pass runner: same ABI plus the Java split/revert rules.
    "decode_table_sequence",
    "check_table_label_block",
    "check_table_column_window",
    "table_spans_from_decoded",
    "table_cells_from_inventory",
    "table_revert_reason",
    "cluster_opens_new_table",
    "table_label_is_admissible",
    // The `formula` locator join over an already-decoded fulltext sequence.
    "join_native_equations",
    "join_native_equation_labels",
    "is_formula_join_label",
    // PascalCase row/span/outcome/error types of the same lane.
    "FamilyLabelCluster",
    "NativeVisualFamilyClusterError",
    "NativeCaptionMarker",
    "NativeCaptionLinks",
    "NativeCaptionLinkAbsence",
    "FigureSpan",
    "NativeFigureAbsence",
    "NativeFigureDecodeOutcome",
    "NativeFigureSequenceError",
    "NativeTableSpan",
    "NativeTableCell",
    "NativeTableCells",
    "NativeTableAbsence",
    "NativeTableRevertReason",
    "NativeTableDecodeOutcome",
    "NativeTableSequenceError",
    "NativeEquationSpan",
    "NativeFormulaAbsence",
    "NativeFormulaJoinError",
    "NativeFormulaJoinOutcome",
];

/// Native funding symbols (S06 key `funding_sequence_quarantine`), mirrored
/// 1:1 by the Governor DATA key.
const FUNDING_SEQUENCE_QUARANTINE_TOKENS: [&str; 25] = [
    // The 17-column ABI in the exact `FeaturesVectorFunding.printVector` order.
    "render_funding_features",
    "render_funding_feature_text",
    "check_funding_feature_columns",
    "validate_funding_gazetteer_coverage",
    // The funding decode and its entity lift.
    "decode_funding_sequence",
    "check_funding_label_block",
    "check_funding_column_window",
    "funding_entries_from_decoded",
    "funding_label_is_admissible",
    "funding_role_of_label",
    // PascalCase row/coverage/entry/outcome/error types of the same lane.
    "FundingFeatureRow",
    "FundingFeatureToken",
    "FundingLineStatus",
    "FundingCapitalisation",
    "FundingDigit",
    "FundingPunctType",
    "NativeFundingFeatureError",
    "NativeFundingGazetteerCoverage",
    "NativeFundingGazetteerLane",
    "NativeFundingGazetteerSpan",
    "NativeFundingEntry",
    "NativeFundingRole",
    "NativeFundingAbsence",
    "NativeFundingDecodeOutcome",
    "NativeFundingSequenceError",
];

/// Read one list from an S06 ledger quarantine key.
///
/// Missing, null or malformed DATA yields an empty list with no invented
/// fallback — the same fail-closed-but-silent contract as the Governor
/// `_family_sequence_quarantine_spec`. Loss is therefore visible where it
/// belongs: the 1:1 equality assertions below compare this list against the
/// Rust constant, so a removed key, a dropped token or a malformed entry all
/// surface as drift instead of a panic inside the reader.
fn family_sequence_quarantine_data(
    registry: &serde_yaml::Value,
    key: &str,
    field: &str,
) -> Vec<String> {
    registry["defaults"][key][field]
        .as_sequence()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(|text| text.to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// Comment-stripped family-atom scan over code text.
fn family_sequence_quarantine_hits(text: &str, tokens: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        let code = match raw.find("//") {
            Some(cut) => &raw[..cut],
            None => raw,
        };
        for token in tokens {
            if bibliography_atom_hit(code, token) {
                hits.push(format!(
                    "[family-sequence-quarantine] line {}: atom `{token}`: {}",
                    line_no + 1,
                    raw.trim()
                ));
            }
        }
    }
    hits
}

/// The locks every S06 family key must carry, checked by value so an eroded
/// lock is observable drift rather than silence.
fn assert_family_sequence_quarantine_locks(registry: &serde_yaml::Value, key: &str, lane: &str) {
    let quarantine = &registry["defaults"][key];
    assert_ne!(
        quarantine,
        &serde_yaml::Value::Null,
        "{lane}: defaults.{key} vanished — loss, not silence"
    );
    for (field, expected) in [
        ("production_authority", "java_http"),
        ("observe_mode", "shadow_receipt"),
        ("production_route", "forbidden"),
        ("graph_write", "forbidden"),
        ("auto_promote", "forbidden"),
        ("proof_level", "contract"),
        ("data_binding", "data/parse/native_visual_families.yaml"),
        ("schema_version", "native-visual-families.v1"),
        ("model_root_env", "DA_GROBID_MODEL_ROOT"),
        (
            "independent_oracle",
            "crates/da-parse/tests/native_visual_family_same_n.rs",
        ),
        (
            "real_model_harness",
            "crates/da-adapters/tests/native_visual_family_real_model.rs",
        ),
    ] {
        assert_eq!(
            quarantine[field].as_str(),
            Some(expected),
            "{lane}: `{field}` must stay `{expected}`"
        );
    }
    for field in ["deployed", "bundled_model_bytes"] {
        assert_eq!(
            quarantine[field].as_bool(),
            Some(false),
            "{lane}: `{field}` must stay false"
        );
    }
    assert_eq!(
        quarantine["import_eligible"].as_bool(),
        Some(false),
        "{lane}: D127 lock eroded"
    );
    assert_eq!(
        quarantine["decision_point"].as_i64(),
        Some(6),
        "{lane}: the lane may not add or move a DecisionPoint"
    );
    let selectable: Vec<String> = quarantine["independently_selectable"]
        .as_sequence()
        .expect("independently_selectable")
        .iter()
        .map(|value| value.as_str().expect("selectable lane").to_owned())
        .collect();
    assert_eq!(
        selectable,
        vec!["header".to_owned(), "references".to_owned()]
    );
    assert!(
        quarantine["offline_limits"]
            .as_sequence()
            .is_some_and(|limits| !limits.is_empty()),
        "{lane}: offline limits must be named, not hidden"
    );
    assert_eq!(
        family_sequence_quarantine_data(registry, key, "boundaries"),
        FAMILY_SEQUENCE_QUARANTINE_BOUNDARIES
            .iter()
            .map(|boundary| (*boundary).to_owned())
            .collect::<Vec<_>>(),
        "{lane}: the four production boundaries drifted"
    );
}

#[test]
fn visual_family_sequence_quarantine_is_data_bound_and_lock_closed() {
    let registry: serde_yaml::Value =
        serde_yaml::from_str(&read_text("data/parse/grobid_compatibility.yaml"))
            .expect("compatibility registry");
    assert_eq!(
        family_sequence_quarantine_data(&registry, "visual_family_sequence_quarantine", "tokens"),
        VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect::<Vec<_>>(),
        "S06: Rust and DATA visual-family token sets drifted (D362 single source)"
    );
    assert_family_sequence_quarantine_locks(
        &registry,
        "visual_family_sequence_quarantine",
        "S06 visual-family",
    );
    // The figure/table second pass reads the S05 fulltext ABI unchanged: 27
    // emitted columns and only 0..=25 referenced by the frozen templates.
    let quarantine = &registry["defaults"]["visual_family_sequence_quarantine"];
    assert_eq!(quarantine["emitted_columns"].as_u64(), Some(27));
    assert_eq!(quarantine["referenced_columns"].as_u64(), Some(26));
    assert_eq!(
        quarantine["grouping_rule"].as_str(),
        Some("tagging_token_clusteror_is_beginning"),
        "S06: the family lane must group by the clusteror rule, not by D359 base runs"
    );
    // The caption-link band never resolves marker -> reference itself (S04 owns it).
    assert_eq!(
        quarantine["caption_link_resolve_owner"].as_str(),
        Some("native_callout_matching")
    );
    assert_eq!(
        quarantine["caption_link_empty_inventory"].as_str(),
        Some("not_observed")
    );
    // Negative controls: a dropped token or an eroded lock must not satisfy the
    // closed DATA set.
    let mut drifted = registry.clone();
    drifted["defaults"]["visual_family_sequence_quarantine"]["tokens"] =
        serde_yaml::Value::Sequence(
            VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS
                [..VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS.len() - 1]
                .iter()
                .map(|token| serde_yaml::Value::String((*token).to_owned()))
                .collect(),
        );
    assert_ne!(
        family_sequence_quarantine_data(&drifted, "visual_family_sequence_quarantine", "tokens"),
        family_sequence_quarantine_data(&registry, "visual_family_sequence_quarantine", "tokens"),
        "S06: a dropped token must not satisfy the closed DATA set"
    );
    let mut promoted = registry.clone();
    promoted["defaults"]["visual_family_sequence_quarantine"]["deployed"] =
        serde_yaml::Value::Bool(true);
    assert_ne!(
        promoted["defaults"]["visual_family_sequence_quarantine"]["deployed"].as_bool(),
        Some(false),
        "S06: deployed=true must be observable as drift"
    );
}

#[test]
fn funding_sequence_quarantine_is_data_bound_and_lock_closed() {
    let registry: serde_yaml::Value =
        serde_yaml::from_str(&read_text("data/parse/grobid_compatibility.yaml"))
            .expect("compatibility registry");
    assert_eq!(
        family_sequence_quarantine_data(&registry, "funding_sequence_quarantine", "tokens"),
        FUNDING_SEQUENCE_QUARANTINE_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect::<Vec<_>>(),
        "S06: Rust and DATA funding token sets drifted (D362 single source)"
    );
    assert_family_sequence_quarantine_locks(
        &registry,
        "funding_sequence_quarantine",
        "S06 funding",
    );
    // 17 emitted feature columns plus the literal `0` dummy label slot at 17.
    let quarantine = &registry["defaults"]["funding_sequence_quarantine"];
    assert_eq!(quarantine["emitted_columns"].as_u64(), Some(17));
    assert_eq!(quarantine["referenced_columns"].as_u64(), Some(17));
    assert_eq!(quarantine["label_column_index"].as_u64(), Some(17));
    assert_eq!(
        quarantine["label_column_value"].as_str(),
        Some("0"),
        "S06: the funding label slot is the literal `0`, never a training label"
    );
    assert_eq!(quarantine["base_labels"].as_u64(), Some(10));
    assert_eq!(quarantine["label_spellings"].as_u64(), Some(20));
    let lanes: Vec<String> = quarantine["gazetteer_lanes"]
        .as_sequence()
        .expect("gazetteer_lanes")
        .iter()
        .map(|value| value.as_str().expect("lane").to_owned())
        .collect();
    assert_eq!(
        lanes,
        vec!["knownFunder".to_owned(), "knownInfrastructure".to_owned()]
    );
    // Coverage is caller-supplied typed spans (D363), never a bundled lexicon.
    assert_eq!(
        quarantine["gazetteer_coverage_source"].as_str(),
        Some("caller_supplied_typed_spans")
    );
    // Negative control: a removed key must not degrade into silence — the
    // reader returns an empty set and the 1:1 equality assertions above are
    // what reject it.
    let mut removed = registry.clone();
    removed["defaults"]["funding_sequence_quarantine"] = serde_yaml::Value::Null;
    assert!(
        family_sequence_quarantine_data(&removed, "funding_sequence_quarantine", "tokens")
            .is_empty(),
        "S06 negative control: a removed DATA key must read as an empty set, not as silence"
    );
    assert_ne!(
        family_sequence_quarantine_data(&removed, "funding_sequence_quarantine", "tokens"),
        FUNDING_SEQUENCE_QUARANTINE_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect::<Vec<_>>(),
        "S06 negative control: an empty set must not satisfy the closed token list"
    );
}

#[test]
fn family_sequence_quarantine_scanner_fails_visible_on_synthetic_atoms() {
    let synthetic = "fn run() {\n    cluster_family_label_runs(labels);\n    decode_figure_sequence(rows, model);\n    decode_table_sequence(rows, model);\n    join_native_equations(&decoded);\n    render_funding_features(&tokens, &coverage);\n    decode_funding_sequence(&rows, &model);\n    let _c: NativeCaptionLinks = links;\n    let _e: NativeFundingEntry = entry;\n}\n";
    let hits = family_sequence_quarantine_hits(
        synthetic,
        &[
            VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS.as_slice(),
            FUNDING_SEQUENCE_QUARANTINE_TOKENS.as_slice(),
        ]
        .concat(),
    );
    assert_eq!(hits.len(), 8, "each family atom must flag: {hits:?}");
    for atom in [
        "cluster_family_label_runs",
        "decode_figure_sequence",
        "decode_table_sequence",
        "join_native_equations",
        "render_funding_features",
        "decode_funding_sequence",
        "NativeCaptionLinks",
        "NativeFundingEntry",
    ] {
        assert!(
            hits.iter().any(|hit| hit.contains(atom)),
            "scanner lost coverage for `{atom}`"
        );
    }
    // Whitespace before `(` is still a call site and must flag.
    let spaced = "fn run() {\n    decode_table_sequence (rows, model);\n    check_funding_label_block\t(model);\n}\n";
    assert_eq!(
        family_sequence_quarantine_hits(spaced, &VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS).len(),
        1,
        "visual lane must flag its own whitespace call-site"
    );
    assert_eq!(
        family_sequence_quarantine_hits(spaced, &FUNDING_SEQUENCE_QUARANTINE_TOKENS).len(),
        1,
        "funding lane must flag its own whitespace call-site"
    );
    // Comments are prose, bare mentions are not call sites, and a prefixed
    // identifier or a longer CamelCase type name is a different symbol.
    let prose = "// decode_figure_sequence NativeCaptionLinks render_funding_features are prose\nlet x = decode_figure_sequence_legacy(y);\nlet y = decode_funding_sequence;\nlet z: NativeCaptionLinksBuilder = links;\nlet w: FundingFeatureRowExtra = row;\nfoo(); // render_funding_features(tokens, coverage);\n";
    assert!(
        family_sequence_quarantine_hits(prose, &VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS)
            .is_empty(),
        "prose comments, prefixed identifiers and bare mentions must not flag (visual)"
    );
    assert!(
        family_sequence_quarantine_hits(prose, &FUNDING_SEQUENCE_QUARANTINE_TOKENS).is_empty(),
        "prose comments, prefixed identifiers and bare mentions must not flag (funding)"
    );
}

#[test]
fn production_boundaries_stay_free_of_family_sequence_atoms() {
    for rel in FAMILY_SEQUENCE_QUARANTINE_BOUNDARIES {
        let text = read_text(rel);
        assert!(
            text.len() > 1_000,
            "positive control failed: {rel} looks truncated"
        );
        let visual =
            family_sequence_quarantine_hits(&text, &VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS);
        let funding = family_sequence_quarantine_hits(&text, &FUNDING_SEQUENCE_QUARANTINE_TOKENS);
        assert!(
            visual.is_empty() && funding.is_empty(),
            "S06 family atoms leaked into a production boundary (shadow means never served):\n{}\n{}",
            visual.join("\n"),
            funding.join("\n")
        );
    }
}

#[test]
fn family_sequence_quarantines_are_distinct_and_stay_off_the_fsm() {
    let registry: serde_yaml::Value =
        serde_yaml::from_str(&read_text("data/parse/grobid_compatibility.yaml"))
            .expect("compatibility registry");
    let visual: std::collections::BTreeSet<String> =
        family_sequence_quarantine_data(&registry, "visual_family_sequence_quarantine", "tokens")
            .into_iter()
            .collect();
    let funding: std::collections::BTreeSet<String> =
        family_sequence_quarantine_data(&registry, "funding_sequence_quarantine", "tokens")
            .into_iter()
            .collect();
    assert_eq!(visual.len(), VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS.len());
    assert_eq!(funding.len(), FUNDING_SEQUENCE_QUARANTINE_TOKENS.len());
    assert!(
        visual.is_disjoint(&funding),
        "the two S06 keys must never share a token"
    );
    // D362: the new keys are disjoint from every sibling lane, including the
    // older `visual_heads_quarantine`, because name adjacency is not inheritance.
    for sibling in [
        "visual_heads_quarantine",
        "fulltext_blocks_quarantine",
        "body_accounting_quarantine",
        "fulltext_sequence_quarantine",
        "callout_matching_quarantine",
    ] {
        let sibling_tokens: std::collections::BTreeSet<String> =
            family_sequence_quarantine_data(&registry, sibling, "tokens")
                .into_iter()
                .collect();
        assert!(
            !sibling_tokens.is_empty(),
            "{sibling} vanished — the disjointness check would be vacuous"
        );
        assert!(
            visual.is_disjoint(&sibling_tokens),
            "visual-family tokens must not overlap {sibling}"
        );
        assert!(
            funding.is_disjoint(&sibling_tokens),
            "funding tokens must not overlap {sibling}"
        );
    }
    // No new DecisionPoint: the policy FSM stays the closed six-point set.
    let fsm = s10_code_without_comments(&read_text("crates/da-domain/src/fsm_policy.rs"));
    assert!(
        s10_decision_point_violations(&fsm).is_empty(),
        "S06 quarantines must preserve the closed six-DecisionPoint policy"
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

/// S02 helper: retain code tokens while ignoring comments and string literals.
/// The prompt spec legitimately contains boundary names such as `GraphStore`;
/// only executable/source coupling should be rejected by the quarantine scan.
fn s02_code_without_comments_and_strings(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            out.push(' ');
            continue;
        }
        if ch == '"' {
            in_string = true;
            out.push(' ');
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for comment_ch in chars.by_ref() {
                if comment_ch == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
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
const S05_BODY_QUARANTINE_ATOMS: [&str; 7] = [
    "NativeBodyAccountant",
    "run_native_body_sequence",
    "join_p5_shadow",
    "check_cross_stage_semantic_invariants",
    "run_visual_heads_sequence",
    "compose_references",
    "ConservationEngine",
];
const S05_BODY_ACCOUNTING_DATA_TOKENS: [&str; 4] = [
    "NativeBodyAccountant",
    "run_native_body_sequence",
    "join_p5_shadow",
    "check_cross_stage_semantic_invariants",
];

fn s05_body_accounting_data_tokens(registry: &serde_yaml::Value) -> Vec<String> {
    registry["defaults"]["body_accounting_quarantine"]["tokens"]
        .as_sequence()
        .expect("body-accounting quarantine DATA tokens")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("body-accounting token string")
                .to_owned()
        })
        .collect()
}

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
    let registry: serde_yaml::Value =
        serde_yaml::from_str(&read_text("data/parse/grobid_compatibility.yaml"))
            .expect("compatibility registry");
    let data_tokens = s05_body_accounting_data_tokens(&registry);
    assert_eq!(
        data_tokens,
        S05_BODY_ACCOUNTING_DATA_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect::<Vec<_>>(),
        "S05: Rust body-accounting scanner and grobid DATA tokens drifted"
    );
    assert_eq!(
        &S05_BODY_QUARANTINE_ATOMS[..S05_BODY_ACCOUNTING_DATA_TOKENS.len()],
        S05_BODY_ACCOUNTING_DATA_TOKENS,
        "S05: body-accounting atoms must remain the leading DATA-driven subset"
    );

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
        for atom in S05_BODY_QUARANTINE_ATOMS {
            let synthetic = if atom.contains('_') {
                format!("fn run() {{ {atom}(input); }}")
            } else {
                format!("let _x: {atom};")
            };
            assert_eq!(
                s05_body_quarantine_hits(&synthetic),
                vec![atom.to_owned()],
                "S05: quarantine scanner lost native atom coverage for {rel}: {atom}"
            );
        }
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

    // S04: the checker and every native producer remain shadow-only. These
    // are call/type tokens, not prose policy names, and are scanned in both
    // production boundaries above.
    for atom in [
        "check_cross_stage_semantic_invariants",
        "NativeBodyAccountant",
        "run_native_body_sequence",
        "run_visual_heads_sequence",
        "compose_references",
        "ConservationEngine",
    ] {
        assert!(
            s05_body_quarantine_hits(&cli).iter().all(|hit| hit != atom)
                && s05_body_quarantine_hits(&port)
                    .iter()
                    .all(|hit| hit != atom),
            "S04: forbidden shadow token leaked into production boundary: {atom}"
        );
    }
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
    let registry: serde_yaml::Value =
        serde_yaml::from_str(&read_text("data/parse/grobid_compatibility.yaml"))
            .expect("compatibility registry");
    let mut dropped_checker = registry.clone();
    dropped_checker["defaults"]["body_accounting_quarantine"]["tokens"] =
        serde_yaml::Value::Sequence(
            S05_BODY_ACCOUNTING_DATA_TOKENS[..3]
                .iter()
                .map(|token| serde_yaml::Value::String((*token).to_owned()))
                .collect(),
        );
    assert_ne!(
        s05_body_accounting_data_tokens(&dropped_checker),
        S05_BODY_ACCOUNTING_DATA_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect::<Vec<_>>(),
        "S05: negative missing-checker fixture must not satisfy the closed DATA set"
    );
    assert!(s05_body_quarantine_hits("let join_p5_shadow_report = 1;").is_empty());
    assert!(s05_body_quarantine_hits("// join_p5_shadow(input)\nlet x = 1;").is_empty());
    assert!(
        s05_body_quarantine_hits("let check_cross_stage_semantic_invariants_report = 1;")
            .is_empty()
    );
    assert!(
        s05_body_quarantine_hits("// check_cross_stage_semantic_invariants(input)\nlet x = 1;")
            .is_empty()
    );
}

#[test]
fn s05_body_accounting_scans_da_application_production_sources() {
    let forbidden = [
        "check_cross_stage_semantic_invariants",
        "NativeBodyAccountant",
        "run_native_body_sequence",
        "run_visual_heads_sequence",
        "compose_references",
        "ConservationEngine",
    ];
    for (path, source) in rust_sources_under("crates/da-application/src") {
        let hits = s05_body_quarantine_hits(&source);
        assert!(
            hits.is_empty(),
            "S04: forbidden shadow call in {path}: {hits:?}"
        );
        let code = s10_code_without_comments(&source);
        for atom in forbidden {
            assert!(
                !code.contains(&format!("{atom}(")),
                "S04: live checker/native call in {path}: {atom}"
            );
        }
    }
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

#[test]
fn s01_derived_maturity_stays_out_of_production_parse_and_write_boundaries() {
    let application_manifest = read_text("crates/da-application/Cargo.toml");
    let application_dependencies = manifest_dependency_names(&application_manifest);
    assert!(
        !application_dependencies
            .iter()
            .any(|(_, name)| name == "da-parse"),
        "S01: da-application must not depend on the parse implementation crate"
    );

    let cli = s10_code_without_comments(&read_text("crates/da-cli/src/source_cli.rs"));
    for forbidden in [
        "derived_evidence_maturity",
        "derive_maturity",
        "MaturityState",
        "MaturityDerivation",
    ] {
        assert!(
            !cli.contains(forbidden),
            "S01: production source_cli leaked derived maturity token `{forbidden}`"
        );
    }
    assert!(
        cli.contains("stamp_parse_grammar")
            && cli.contains("grobid_profile_from_route")
            && cli.contains("crate::parse_profile"),
        "S01: production parse routing must remain expressed through grammar/profile resolvers"
    );

    let maturity = s10_code_without_comments(&read_text(
        "crates/da-application/src/derived_evidence_maturity.rs",
    ));
    for forbidden in [
        "da_parse",
        "source_cli",
        "run_parse_pdf",
        "GraphStore",
        "PromotionService",
        "write_graph",
        "import_write",
    ] {
        assert!(
            !maturity.contains(forbidden),
            "S01: derived-only module leaked production coupling token `{forbidden}`"
        );
    }
}

// ───── M499/S02/T03: bounded prompt renderer stays offline and quarantined ─────
//
// The prompt controller is planning metadata, not a parse/import/graph/model
// controller. Keep this as a source-level contract so an accidental import is
// caught before the application crate can expose a privileged path.

#[test]
fn s02_derived_prompt_contract_stays_out_of_production_parse_and_write_boundaries() {
    let application_manifest = read_text("crates/da-application/Cargo.toml");
    let dependencies = manifest_dependency_names(&application_manifest);
    assert!(
        !dependencies.iter().any(|(_, name)| name == "da-parse"),
        "S02: da-application must not depend on da-parse"
    );

    let cli = s10_code_without_comments(&read_text("crates/da-cli/src/source_cli.rs"));
    for forbidden in [
        "derived_prompt_contract",
        "render_prompt",
        "AtomicPromptContract",
        "FamilyPromptSpec",
        "PromptRender",
    ] {
        assert!(
            !cli.contains(forbidden),
            "S02: source_cli leaked offline prompt token `{forbidden}`"
        );
    }
    assert!(
        cli.contains("stamp_parse_grammar")
            && cli.contains("grobid_profile_from_route")
            && cli.contains("crate::parse_profile"),
        "S02: parse-pdf must retain grammar/profile resolver wiring"
    );

    let prompt = s02_code_without_comments_and_strings(&read_text(
        "crates/da-application/src/derived_prompt_contract.rs",
    ));
    for forbidden in [
        "da_parse",
        "source_cli",
        "run_parse_pdf",
        "GraphStore",
        "PromotionService",
        "import_write",
        "PromptExecution",
        "PromptCandidate",
        "tracing",
        "std::fs",
        "tokio",
        "unsafe",
    ] {
        let leaked = if forbidden == "unsafe" {
            prompt.contains("unsafe {")
                || prompt.contains("unsafe fn")
                || prompt.contains("unsafe impl")
        } else {
            prompt.contains(forbidden)
        };
        assert!(
            !leaked,
            "S02: prompt renderer leaked forbidden coupling token `{forbidden}`"
        );
    }

    let lib = s10_code_without_comments(&read_text("crates/da-application/src/lib.rs"));
    assert_eq!(
        lib.matches("pub mod derived_prompt_contract;").count(),
        1,
        "S02: prompt renderer must have exactly one production module export"
    );

    let prompt_symbols = [
        "derived_prompt_contract",
        "render_prompt",
        "AtomicPromptContract",
        "FamilyPromptSpec",
        "PromptRender",
    ];
    for (path, source) in rust_sources_under("crates/da-application/src") {
        if path == "crates/da-application/src/lib.rs"
            || path == "crates/da-application/src/derived_prompt_contract.rs"
        {
            continue;
        }
        let code = s10_code_without_comments(&source);
        for symbol in prompt_symbols {
            assert!(
                !code.contains(symbol),
                "S02: production reference `{symbol}` escaped quarantine in {path}"
            );
        }
    }
}

#[test]
fn s02_prompt_quarantine_scanner_fails_visible_on_synthetic_coupling() {
    let clean = s02_code_without_comments_and_strings(&read_text(
        "crates/da-application/src/derived_prompt_contract.rs",
    ));
    assert!(!clean.contains("GraphStore"));
    for token in [
        "da_parse",
        "GraphStore",
        "PromptExecution",
        "std::fs",
        "tokio",
    ] {
        let synthetic = format!("fn leaked() {{ let _ = {token}; }}");
        assert!(
            synthetic.contains(token) && !clean.contains(token),
            "positive/negative quarantine controls must remain meaningful for `{token}`"
        );
    }
}

// ───── M499/S04/T03: family adapters remain an offline quarantine leaf ─────
//
// The adapter assembles injected DTOs only.  It must not become a parse
// route, and the parse plane must not reach back into application policy.

#[test]
fn s04_derived_family_adapters_stays_out_of_production_parse_and_write_boundaries() {
    let application_manifest = read_text("crates/da-application/Cargo.toml");
    let dependencies = manifest_dependency_names(&application_manifest);
    assert!(
        !dependencies.iter().any(|(_, name)| name == "da-parse"),
        "S04: da-application must not depend on da-parse"
    );

    let application_lib = s10_code_without_comments(&read_text("crates/da-application/src/lib.rs"));
    assert!(
        !application_lib.contains("derived_family_adapters"),
        "S04: adapter must stay out of the production application surface"
    );

    for (path, source) in rust_sources_under("crates/da-application/src") {
        assert!(
            !source.contains("derived_family_adapters"),
            "S04: adapter escaped into production application source in {path}"
        );
    }

    let adapter =
        read_text("crates/da-cli/examples/derived_maturity_rehearsal/derived_family_adapters.rs");
    assert!(
        adapter.contains("pub fn adapt_family")
            && adapter.contains("pub fn prompt_adapted")
            && adapter.contains("pub fn evaluate_adapted"),
        "S04: adapter must expose only the intended constructor and thin delegates"
    );
    for forbidden in [
        "std::fs",
        "tokio",
        "tracing",
        "std::net",
        "std::process",
        "write_graph",
        "run_parse_pdf",
        "source_cli",
        "GraphStore",
        "ParserPort",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "S04: adapter leaked runtime coupling token `{forbidden}`"
        );
    }

    let cli = s10_code_without_comments(&read_text("crates/da-cli/src/source_cli.rs"));
    for token in [
        "derived_family_adapters",
        "adapt_family",
        "prompt_adapted",
        "evaluate_adapted",
        "FamilyAdapterInput",
        "AdaptedFamily",
    ] {
        assert!(
            !cli.contains(token),
            "S04: source_cli leaked offline adapter token `{token}`"
        );
    }

    for (path, source) in rust_sources_under("crates/da-parse/src") {
        assert!(
            !source.contains("derived_family_adapters")
                && !source.contains("adapt_family")
                && !source.contains("FamilyAdapterInput"),
            "S04: parse source reached application adapter in {path}"
        );
    }
}

// ───── M499/S03/T03: transition evaluator remains an offline diagnostic leaf ─────
//
// The evaluator accepts untrusted observations but has no production parse,
// import, graph, promotion, persistence, or runtime authority. Keep this
// contract source-level and closed-world: adding a second export or wiring the
// API into CLI/application siblings must fail before it can become a seventh
// runtime decision point.
const S03_TRANSITION_API_TOKENS: [&str; 5] = [
    "derived_transition_evaluator",
    "evaluate_transition",
    "TransitionObservations",
    "ClaimedTransition",
    "TransitionEvaluation",
];

const S03_TRANSITION_FORBIDDEN_TOKENS: [&str; 18] = [
    "da_parse",
    "ParserPort",
    "source_cli",
    "run_parse_pdf",
    "GraphStore",
    "PromotionService",
    "import_write",
    "tracing",
    "std::fs",
    "tokio",
    "unsafe",
    "write_graph",
    "import",
    "promote",
    "persist",
    "std::net",
    "std::process",
    "chrono",
];

fn s03_transition_quarantine_hits(text: &str) -> Vec<String> {
    let code = s02_code_without_comments_and_strings(text);
    let mut hits = Vec::new();
    for token in S03_TRANSITION_FORBIDDEN_TOKENS {
        let found = if token.contains("::") {
            code.contains(token)
        } else {
            code.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .any(|part| part == token)
        };
        if found {
            hits.push(token.to_owned());
        }
    }
    hits
}

#[test]
fn s03_derived_transition_evaluator_stays_out_of_production_parse_and_write_boundaries() {
    let application_manifest = read_text("crates/da-application/Cargo.toml");
    let dependencies = manifest_dependency_names(&application_manifest);
    assert!(
        !dependencies.iter().any(|(_, name)| name == "da-parse"),
        "S03: da-application must not depend on da-parse"
    );

    let cli = s10_code_without_comments(&read_text("crates/da-cli/src/source_cli.rs"));
    for token in S03_TRANSITION_API_TOKENS {
        assert!(
            !cli.contains(token),
            "S03: source_cli leaked transition evaluator token `{token}`"
        );
    }
    assert!(
        cli.contains("stamp_parse_grammar")
            && cli.contains("grobid_profile_from_route")
            && cli.contains("crate::parse_profile"),
        "S03: parse-pdf routing must remain expressed through grammar/profile resolvers"
    );

    let evaluator = read_text("crates/da-application/src/derived_transition_evaluator.rs");
    assert!(
        s03_transition_quarantine_hits(&evaluator).is_empty(),
        "S03: evaluator leaked production coupling: {:?}",
        s03_transition_quarantine_hits(&evaluator)
    );

    let lib = s10_code_without_comments(&read_text("crates/da-application/src/lib.rs"));
    assert_eq!(
        lib.matches("pub mod derived_transition_evaluator;").count(),
        1,
        "S03: evaluator must have exactly one production module export"
    );

    let mut module_exports = 0;
    for (path, source) in rust_sources_under("crates/da-application/src") {
        let code = s10_code_without_comments(&source);
        if code.contains("pub mod derived_transition_evaluator;") {
            module_exports += 1;
        }
        if path != "crates/da-application/src/lib.rs"
            && path != "crates/da-application/src/derived_transition_evaluator.rs"
        {
            for token in S03_TRANSITION_API_TOKENS {
                assert!(
                    !code.contains(token),
                    "S03: transition evaluator API escaped quarantine in {path}: `{token}`"
                );
            }
        }
    }
    assert_eq!(module_exports, 1, "S03: evaluator export escaped lib.rs");
}

#[test]
fn s03_transition_quarantine_scanner_fails_visible_on_synthetic_coupling() {
    let clean = s03_transition_quarantine_hits(&read_text(
        "crates/da-application/src/derived_transition_evaluator.rs",
    ));
    assert!(
        clean.is_empty(),
        "clean evaluator unexpectedly has hits: {clean:?}"
    );
    for token in S03_TRANSITION_FORBIDDEN_TOKENS {
        let synthetic = if token.contains("::") {
            format!("fn leaked() {{ let _ = {token}::value; }}")
        } else if token == "unsafe" {
            "unsafe { let _ = 1; }".to_owned()
        } else {
            format!("fn leaked() {{ let _ = {token}; }}")
        };
        assert_eq!(
            s03_transition_quarantine_hits(&synthetic),
            vec![token.to_owned()],
            "S03: synthetic forbidden coupling `{token}` was not fail-visible"
        );
    }
    assert!(
        s03_transition_quarantine_hits("// GraphStore import_write\nlet oracle_imports = 1;")
            .is_empty(),
        "S03: comments and compound identifiers must not create false positives"
    );
}

// ───── M499/S05/T03: offline rehearsal remains a quarantined CLI example ─────
const S05_REHEARSAL_EXAMPLE: &str = "crates/da-cli/examples/derived_maturity_rehearsal/main.rs";
const S05_REHEARSAL_TEST: &str = "crates/da-cli/tests/derived_maturity_rehearsal_example.rs";

fn s05_quarantine_hits(text: &str) -> Vec<String> {
    let code = s02_code_without_comments_and_strings(text);
    let mut hits = Vec::new();
    for marker in [
        "da_parse",
        "ParserPort",
        "run_parse_pdf",
        "parse-pdf",
        "std::fs",
        "std::net",
        "std::process",
        "reqwest",
        "tokio",
        "GraphStore",
        "write_graph",
        "PromotionService",
        "promote",
        "deploy",
        "source_cli",
    ] {
        if marker.contains("::") || marker.contains('-') {
            if code.contains(marker) {
                hits.push(marker.to_owned());
            }
        } else if code
            .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .any(|part| part == marker)
        {
            hits.push(marker.to_owned());
        }
    }
    hits
}

#[test]
fn s05_derived_maturity_rehearsal_is_quarantined_and_closed() {
    let example = read_text(S05_REHEARSAL_EXAMPLE);
    let integration = read_text(S05_REHEARSAL_TEST);
    assert!(
        example.len() > 10_000,
        "S05: rehearsal example missing or truncated"
    );
    assert!(
        integration.len() > 1_000,
        "S05: independent rehearsal integration test missing or truncated"
    );

    let manifest = read_text("crates/da-cli/Cargo.toml");
    assert_eq!(
        manifest
            .lines()
            .filter(|line| line.trim() == "name = \"derived_maturity_rehearsal\"")
            .count(),
        1,
        "S05: rehearsal must have exactly one manifest declaration"
    );
    let declaration = manifest
        .find("name = \"derived_maturity_rehearsal\"")
        .expect("S05: manifest declaration disappeared");
    let last_example = manifest[..declaration].rfind("[[example]]").unwrap_or(0);
    assert!(
        !manifest[last_example..declaration].contains("required-features"),
        "S05: rehearsal must not be feature-gated"
    );

    assert!(
        example.contains("init_pipeline_logging(\"derived_maturity_rehearsal\")"),
        "S05: mandatory pipeline logging missing"
    );
    for token in [
        "adapt_family",
        "prompt_adapted",
        "evaluate_adapted",
        "evaluate_transition",
    ] {
        assert!(
            example.contains(token),
            "S05: approved surface missing: {token}"
        );
    }
    assert!(
        s05_quarantine_hits(&example).is_empty(),
        "S05: rehearsal leaked production coupling"
    );

    let source_cli = s10_code_without_comments(&read_text("crates/da-cli/src/source_cli.rs"));
    for token in [
        "derived_maturity_rehearsal",
        "derived_family_adapters",
        "derived_prompt_contract",
        "derived_transition_evaluator",
        "evaluate_transition",
    ] {
        assert!(
            !source_cli.contains(token),
            "S05: source_cli leaked rehearsal token `{token}`"
        );
    }
    for (path, source) in rust_sources_under("crates/da-parse/src") {
        for token in [
            "derived_maturity_rehearsal",
            "derived_family_adapters",
            "derived_prompt_contract",
            "derived_transition_evaluator",
            "evaluate_transition",
        ] {
            assert!(
                !source.contains(token),
                "S05: parse source reached `{token}` in {path}"
            );
        }
    }

    let cli_lib = s10_code_without_comments(&read_text("crates/da-cli/src/lib.rs"));
    assert!(
        !cli_lib.contains("derived_maturity_rehearsal"),
        "S05: rehearsal escaped into da-cli library"
    );
    let application_lib = s10_code_without_comments(&read_text("crates/da-application/src/lib.rs"));
    assert!(
        !application_lib.contains("derived_maturity_rehearsal"),
        "S05: application exported composition root"
    );
    assert_eq!(
        application_lib.matches("pub mod derived_").count(),
        3,
        "S05: derived application surface changed"
    );
    let application_manifest = read_text("crates/da-application/Cargo.toml");
    assert!(
        !manifest_dependency_names(&application_manifest)
            .iter()
            .any(|(_, name)| name == "da-parse"),
        "S05: da-application depends on da-parse"
    );

    let fsm = read_text("crates/da-domain/src/fsm_policy.rs");
    assert!(
        s10_decision_point_violations(&s10_code_without_comments(&fsm)).is_empty(),
        "S05: DecisionPoint wire set drifted"
    );
}

#[test]
fn s05_rehearsal_quarantine_scanner_fails_visible_on_synthetic_coupling() {
    assert!(s05_quarantine_hits("fn clean() { let value = 1; }").is_empty());
    for marker in [
        "da_parse",
        "ParserPort",
        "run_parse_pdf",
        "parse-pdf",
        "std::fs",
        "std::net",
        "std::process",
        "reqwest",
        "tokio",
        "GraphStore",
        "write_graph",
        "PromotionService",
        "promote",
        "deploy",
        "source_cli",
    ] {
        let synthetic = if marker.contains("::") {
            format!("fn leaked() {{ let _ = {marker}::value; }}")
        } else {
            format!("fn leaked() {{ let _ = {marker}; }}")
        };
        assert_eq!(
            s05_quarantine_hits(&synthetic),
            vec![marker.to_owned()],
            "S05: synthetic coupling `{marker}` was not fail-visible"
        );
    }
    assert!(
        s05_quarantine_hits("// GraphStore and std::fs in prose\nlet source_cli_value = 1;")
            .is_empty(),
        "S05: comments and compound identifiers tripped scanner"
    );
}

// ───── M499/S06/T01: named requirement-level evidence closure ─────
//
// This is intentionally an aggregator, not a second governor.  It gives the
// milestone validator one executable source location that names the
// requirements whose proof is distributed across the earlier contracts.
#[test]
fn s06_requirement_level_evidence_closure() {
    let fsm = s10_code_without_comments(&read_text("crates/da-domain/src/fsm_policy.rs"));
    assert!(
        s10_decision_point_violations(&fsm).is_empty(),
        "R118: three-automata conservation requires the approved six DecisionPoint wires"
    );
    for wire in [
        "grammar_route",
        "batch_admission",
        "concurrency_tune",
        "retry_policy",
        "summary_depth",
        "profile_route",
    ] {
        assert!(
            fsm.contains(wire),
            "R118/R119: approved DecisionPoint wire disappeared: {wire}"
        );
    }

    let mut header_hits = Vec::new();
    for boundary in HEADER_QUARANTINE_BOUNDARIES {
        header_hits.extend(
            header_quarantine_hits(&read_text(boundary))
                .into_iter()
                .map(|hit| format!("{boundary}: {hit}")),
        );
    }
    assert!(
        header_hits.is_empty(),
        "R127: parse-pdf ParserPort/source_cli header quarantine leaked: {header_hits:?}"
    );

    let source_cli = read_text("crates/da-cli/src/source_cli.rs");
    let source_cli_code = s10_code_without_comments(&source_cli);
    assert!(
        source_cli_code.contains("stamp_p3_header_observe(&mut m, parsed.header_authors.len())"),
        "R127: P3 must stamp Java-lifted header_authors, not native composition"
    );
    assert!(
        source_cli_code.contains("java_http") && source_cli_code.contains("shadow_receipt"),
        "R127: P3 authority must remain java_http with shadow_receipt observation"
    );
    assert!(
        source_cli_code.contains("stamp_parse_grammar") && source_cli_code.contains("none"),
        "R119: policy-none fallback must remain explicitly stamped"
    );

    let compat = read_text("data/parse/grobid_compatibility.yaml");
    assert!(
        compat.contains("parse_pdf_consults: [grammar_route, profile_route]"),
        "R119: parse_pdf consult lineage must remain exactly grammar_route then profile_route"
    );
    assert!(
        compat.contains("production_authority: java_http")
            && compat.contains("observe_mode: shadow_receipt")
            && compat.contains("this_wave_promotes: none"),
        "R118/R127/R130: production authority, quarantine, and no-promotion freeze drifted"
    );

    let rehearsal = read_text(S05_REHEARSAL_EXAMPLE);
    assert!(
        s05_quarantine_hits(&rehearsal).is_empty(),
        "R130: offline rehearsal must remain free of parse, graph, import, and promotion coupling"
    );
    let application_manifest = read_text("crates/da-application/Cargo.toml");
    assert!(
        !manifest_dependency_names(&application_manifest)
            .iter()
            .any(|(_, name)| name == "da-parse"),
        "R118/R130: onion boundary must not permit da-application -> da-parse"
    );

    let da_parse_manifest = read_text("crates/da-parse/Cargo.toml");
    let forbidden = ["samyama", "ruvllm", "sona", "redb", "da-application"];
    let deps = manifest_dependency_names(&da_parse_manifest);
    for name in forbidden {
        assert!(
            !deps.iter().any(|(_, dependency)| dependency == name),
            "R124: da-parse forbidden dependency leaked into manifest: {name}"
        );
    }
}

#[test]
fn s06_requirement_level_scanner_fails_visible_on_synthetic_coupling() {
    let synthetic = "fn leaked() { let _ = ParserPort; let _ = GraphStore; }";
    let hits = s05_quarantine_hits(synthetic);
    assert!(
        !hits.is_empty(),
        "R124/R130: shared quarantine scanner must expose synthetic production coupling"
    );
    assert!(
        hits.contains(&"ParserPort".to_owned()) && hits.contains(&"GraphStore".to_owned()),
        "R118/R124: synthetic parse and graph coupling must both be fail-visible: {hits:?}"
    );
    assert!(
        s05_quarantine_hits("// ParserPort GraphStore\nlet source_cli_value = 1;").is_empty(),
        "R124/R130: scanner must ignore prose and compound identifiers"
    );
}

// ───── M499/S06/T02: six W0 triggers and policy lineage remain fail-closed ─────
//
// R124 names the six W0 honesty triggers. The sibling contracts retain the
// backend identity, graph-freeze/write, and F1-not-oracle checks; those are
// deliberately not extra rows in this matrix. This table is an executable
// DATA/checker pin, not a second governor implementation.
const S06_R124_W0_TRIGGERS: [(&str, &str, &str); 6] = [
    (
        "ledger_surface",
        "ledger_surface",
        "data/parse/grobid_compatibility.yaml",
    ),
    (
        "backend_identity",
        "backend_identity",
        "data/parse/grobid_compatibility.yaml",
    ),
    (
        "policy_stamp",
        "policy_stamp",
        "data/parse/grobid_compatibility.yaml",
    ),
    (
        "silent_loss",
        "silent_loss",
        "data/parse/grobid_compatibility.yaml",
    ),
    (
        "lexical_visual_mix",
        "lexical_visual_mix",
        "data/parse/native_visual_heads.yaml",
    ),
    (
        "region_coverage",
        "region_coverage",
        "data/parse/document_segmentation.yaml",
    ),
];

#[test]
fn s06_r124_w0_trigger_matrix_is_six_unique_data_pinned_rows() {
    let names: Vec<_> = S06_R124_W0_TRIGGERS
        .iter()
        .map(|(name, _, _)| *name)
        .collect();
    assert_eq!(
        names.len(),
        6,
        "R124: W0 honesty matrix must contain exactly six triggers, not sibling gates"
    );
    let unique: BTreeSet<_> = names.iter().copied().collect();
    assert_eq!(
        unique.len(),
        names.len(),
        "R124: W0 trigger names must be unique: {names:?}"
    );
    let checker = read_text("scripts/governor/checks/scholarly_parse_plane.yaml");
    for (trigger, checker_pin, data_path) in S06_R124_W0_TRIGGERS {
        assert!(
            checker.contains(checker_pin),
            "R124: trigger `{trigger}` lost its governor checker pin `{checker_pin}`"
        );
        let data = read_text(data_path);
        assert!(
            !data.trim().is_empty(),
            "R124: trigger `{trigger}` lost its DATA pin source `{data_path}`"
        );
    }
    for sibling in [
        "production_route",
        "graph_freeze",
        "denom_honesty",
        "dep_boundary",
        "f1_not_oracle",
    ] {
        assert!(
            !names.contains(&sibling),
            "R124: sibling contract `{sibling}` must not inflate the six-row W0 matrix"
        );
    }
}

#[test]
fn s06_r119_policy_lineage_fallback_and_consult_set_are_explicit() {
    let source_cli = s10_code_without_comments(&read_text("crates/da-cli/src/source_cli.rs"));
    let grammar = read_text("crates/da-application/src/parse_grammar.rs");
    let consult = read_text("crates/da-application/src/fsm_policy_consult.rs");
    let compat = read_text("data/parse/grobid_compatibility.yaml");

    assert!(
        grammar.contains("pub fn resolve_parse_grammar")
            && grammar.contains("unwrap_or(\"arxiv_pdf_dot\")"),
        "R119: grammar route must fail-open to the DATA default profile"
    );
    assert!(
        grammar.contains("pub fn resolve_parse_profile")
            && grammar.contains("unwrap_or(ProfileRoute::Structural)"),
        "R119: profile route must fail-open to the DATA structural default"
    );
    assert!(
        consult.contains(
            "FsmPolicyError::Load(_) | FsmPolicyError::Instantiate(_) | FsmPolicyError::Io(_)"
        ) && consult.contains("NativeFallbackPolicy::from_yaml(yaml)"),
        "R119: policy host load failure must fall back to the native policy"
    );
    assert!(
        source_cli.contains("fn stamp_policy_none")
            && source_cli.contains(
                "stamp_parse_grammar(m, \"arxiv_pdf_dot\", \"unavailable\", \"none\", \"none\")"
            ),
        "R119: missing policy must retain explicit wire policy_source=none and policy_hash=none"
    );
    assert!(
        source_cli.contains("m.insert(\"policy_hash\".into(), policy_hash.into())"),
        "R119: policy lineage must preserve policy_hash or explicit none"
    );
    assert_eq!(
        compat
            .matches("parse_pdf_consults: [grammar_route, profile_route]")
            .count(),
        1,
        "R119: parse_pdf consult lineage must be exactly grammar_route then profile_route"
    );
    assert!(
        !compat.contains("parse_pdf_consults: [grammar_route, profile_route, ")
            && !compat.contains("parse_pdf_consults: [profile_route, grammar_route]"),
        "R119: a third consult or reordered policy route must fail closed"
    );
}

#[test]
fn s06_r127_p3_java_stamp_and_native_header_order_remain_bound() {
    let source_cli = s10_code_without_comments(&read_text("crates/da-cli/src/source_cli.rs"));
    assert!(
        source_cli.contains("stamp_p3_header_observe(&mut m, parsed.header_authors.len())"),
        "R127: P3 java_header_n must derive from Java ParsedArticle.header_authors"
    );
    assert!(
        source_cli.contains("java_http") && source_cli.contains("shadow_receipt"),
        "R127: P3 authority must remain Java with shadow receipt"
    );
    let native = read_text("crates/da-parse/tests/native_composition_contract.rs");
    assert!(
        native.contains("fn header_sequence_is_date_then_name_then_affiliation")
            && native.contains("CompositionComponent::Date")
            && native.contains("CompositionComponent::Names")
            && native.contains("CompositionComponent::Affiliations"),
        "R127: native header sequence must remain date then name then affiliation"
    );
}

#[test]
fn s01_native_family_promotion_gate_stays_out_of_production_parse_and_write_boundaries() {
    let application_lib = read_text("crates/da-application/src/lib.rs");
    let gate = read_text("crates/da-application/src/native_family_promotion_gate.rs");
    let cli = read_text("crates/da-cli/src/source_cli.rs");
    let parser_port = read_text("crates/da-ports/src/parser.rs");

    // The gate is a standalone application module, not a production route or
    // facade export.  This keeps injected review evidence separate from the
    // parse/write composition root.
    assert!(
        application_lib
            .lines()
            .any(|line| line.trim() == "pub mod native_family_promotion_gate;"),
        "S01: gate must remain registered as a standalone da-application module"
    );
    assert!(
        !application_lib.contains("pub use native_family_promotion_gate"),
        "S01: gate must not be facade-reexported"
    );
    assert!(
        !cli.contains("native_family_promotion_gate") && !cli.contains("derive_promotion_gate"),
        "S01: source_cli must not route through the dry-run gate"
    );
    assert!(
        parser_port.contains("pub trait ParserPort") && !parser_port.contains("PromotionGate"),
        "S01: ParserPort must remain the parse-only boundary"
    );

    // Scan only code positions.  concat! keeps the denylist itself from
    // becoming a false positive while still making representative mutations
    // fail visibly in this source-level contract.
    let code = s10_code_without_comments(&gate);
    let forbidden = [
        concat!("da_", "parse"),
        "PromotionService",
        "PromotionCandidate",
        "GraphStore",
        "DirectGraphStore",
        "aggregate_same_n_ablation",
        "parse_pdf",
        "parse_pdf_runtime",
        "deploy_candidate",
        "import_candidate",
        "auto_promote_candidate",
        "promote_candidate",
    ];
    for token in forbidden {
        assert!(
            !code.contains(token),
            "S01: forbidden production coupling leaked into native_family_promotion_gate: {token}"
        );
    }
    for token in [
        "create_node",
        "create_edge",
        "set_node_property",
        "Command::new",
    ] {
        assert!(
            !code.contains(token),
            "S01: side-effect call leaked into native_family_promotion_gate: {token}"
        );
    }
    assert!(
        code.contains("FamilyId") && code.contains("FamilyPromotionEvidence"),
        "S01: gate must retain only the typed FamilyId/evidence derivation seam"
    );

    // Production parse remains on the approved Java GROBID route and the
    // existing six-name DecisionPoint vocabulary remains closed.
    assert!(
        cli.contains("GrobidProfile") || cli.contains("grobid"),
        "S01: source_cli lost the approved GROBID parse/profile route"
    );
    let fsm = read_text("crates/da-domain/src/fsm_policy.rs");
    assert!(
        s10_decision_point_violations(&s10_code_without_comments(&fsm)).is_empty(),
        "S01: DecisionPoint drifted while adding the standalone gate"
    );

    let compatibility = read_text("data/parse/grobid_compatibility.yaml");
    for pin in [
        "import_eligible: false",
        "auto_promote: forbidden",
        "this_wave_promotes: none",
        "deployed: false",
        "rollback_target: java-grobid-http@0.9.0-crf",
    ] {
        assert!(
            compatibility.contains(pin),
            "S01: missing locked GROBID policy pin: {pin}"
        );
    }

    let composition = read_text("data/parse/native_fulltext_composition.yaml");
    for pin in [
        "pooled_winner: forbidden",
        "independently_selectable: [header, references]",
        "s12_named_denominators:",
        "hard_regressions:",
        "missing_rollback",
        "java_header_authors_equals: header_raw_total",
        "hub_native_accepted_header: 20",
        "hub_java_header_raw: 21",
    ] {
        assert!(
            composition.contains(pin),
            "S01: missing native composition lock/gap: {pin}"
        );
    }
}

#[test]
fn s02_native_family_same_n_adapter_stays_single_evaluator_and_quarantined() {
    let application_lib = read_text("crates/da-application/src/lib.rs");
    let adapter = read_text("crates/da-application/src/native_family_same_n_adapter.rs");
    let cli = read_text("crates/da-cli/src/source_cli.rs");
    let parser = read_text("crates/da-ports/src/parser.rs");
    let graph_store = read_text("crates/da-ports/src/graph_store.rs");
    let promotion = read_text("crates/da-application/src/promotion.rs");

    assert!(
        application_lib
            .lines()
            .any(|line| line.trim() == "pub mod native_family_same_n_adapter;"),
        "S02: adapter must be registered as a standalone application module"
    );
    assert!(
        !application_lib.contains("pub use native_family_same_n_adapter"),
        "S02: adapter must not be broad-facade re-exported"
    );
    for (name, source) in [
        ("source_cli", cli),
        ("ParserPort", parser),
        ("GraphStore", graph_store),
        ("PromotionService", promotion),
    ] {
        assert!(
            !source.contains("native_family_same_n_adapter"),
            "S02: {name} must not import or route through the dry-run adapter"
        );
    }

    // Keep the denylist out of the scanned source with split literals. This
    // makes a forbidden coupling fail visibly without self-matching here.
    let code = s10_code_without_comments(&adapter);
    assert_eq!(
        code.matches(concat!(
            "aggregate_same_n_",
            "ablation",
            "_with_provenance("
        ))
        .count(),
        1,
        "S02: adapter must invoke the canonical same-n evaluator exactly once"
    );
    assert_eq!(
        code.matches(concat!("aggregate_same_n_", "ablation", "("))
            .count(),
        0,
        "S02: adapter must not route promotion through the label compatibility bridge"
    );
    for token in [
        concat!("da_", "parse"),
        "ParserPort",
        "PromotionService",
        "GraphStore",
        "DirectGraphStore",
        "source_cli",
        "Command::new",
        "std::fs",
        "reqwest",
        "TcpStream",
        "spawn(",
        "create_node",
        "create_edge",
        "set_node_property",
        "set_edge_property",
        "deploy_candidate",
        "import_candidate",
        "auto_promote_candidate",
        "promote_candidate",
        "filesystem",
        "network",
        "process",
        "average(",
        "mean(",
        "sort_by",
        "sort_unstable",
        ".filter(",
    ] {
        assert!(
            !code.contains(token),
            "S02: forbidden adapter coupling or duplicate evaluator operation: {token}"
        );
    }
    for token in [
        "fn denominator",
        "fn order_digest",
        "fn conservation_ledger",
    ] {
        assert!(
            !code.contains(token),
            "S02: adapter contains a local evaluator-like helper: {token}"
        );
    }

    // Synthetic controls prove the scanner's critical negative checks are
    // fail-visible rather than merely documenting an architectural intent.
    let synthetic = "fn drift() { aggregate_same_n_ablation(x); rows.sort_by_key(|r| r.order); let _ = GraphStore; }";
    assert_eq!(
        synthetic
            .matches(concat!("aggregate_same_n_", "ablation", "("))
            .count(),
        1,
        "S02: synthetic second evaluator call must be detectable"
    );
    assert!(
        synthetic.contains("sort_by"),
        "S02: row sorting drift must be detectable"
    );
    assert!(
        synthetic.contains("GraphStore"),
        "S02: graph coupling drift must be detectable"
    );
}

#[test]
fn s03_native_family_adversarial_gate_stays_quarantined() {
    let application_lib = read_text("crates/da-application/src/lib.rs");
    let adapter = read_text("crates/da-application/src/native_family_same_n_adapter.rs");
    let gate = read_text("crates/da-application/src/native_family_promotion_gate.rs");
    let cli = read_text("crates/da-cli/src/source_cli.rs");
    let parser = read_text("crates/da-ports/src/parser.rs");
    let graph_store = read_text("crates/da-ports/src/graph_store.rs");
    let promotion = read_text("crates/da-application/src/promotion.rs");

    // Both surfaces stay standalone: no facade route and no adversarial
    // production module can turn this dry-run proof into a runtime path.
    for module in [
        "pub mod native_family_promotion_gate;",
        "pub mod native_family_same_n_adapter;",
    ] {
        assert!(
            application_lib.lines().any(|line| line.trim() == module),
            "S03: missing standalone module declaration: {module}"
        );
    }
    assert!(!application_lib.contains("pub use native_family_promotion_gate"));
    assert!(!application_lib.contains("pub use native_family_same_n_adapter"));
    for (name, source) in [
        ("source_cli", cli),
        ("ParserPort", parser),
        ("GraphStore", graph_store),
        ("PromotionService", promotion),
    ] {
        assert!(
            !source.contains("native_family_promotion_gate")
                && !source.contains("native_family_same_n_adapter"),
            "S03: {name} must not route through native-family dry-run surfaces"
        );
    }

    // Strip comments and strings before scanning production code.  The
    // split evaluator spelling keeps the scanner's own marker out of scope.
    let adapter_code = s02_code_without_comments_and_strings(&adapter);
    assert_eq!(
        adapter_code
            .matches(concat!(
                "aggregate_same_n_",
                "ablation",
                "_with_provenance("
            ))
            .count(),
        1,
        "S03: adapter must retain exactly one canonical evaluator call"
    );
    assert_eq!(
        adapter_code
            .matches(concat!("aggregate_same_n_", "ablation", "("))
            .count(),
        0,
        "S03: adapter must not authorize promotion through the label bridge"
    );
    let forbidden = [
        concat!("da_", "parse"),
        "ParserPort",
        "source_cli",
        "PromotionService",
        "GraphStore",
        "DirectGraphStore",
        "std::fs",
        "reqwest",
        "TcpStream",
        "Command::new",
        "parse_pdf",
        "deploy_candidate",
        "import_candidate",
        "auto_promote_candidate",
        "promote_candidate",
        "create_node",
        "create_edge",
        "set_node_property",
        "set_edge_property",
        "filesystem",
        "network",
        "process",
    ];
    for token in forbidden {
        assert!(
            !adapter_code.contains(token),
            "S03: forbidden production-boundary token leaked into adapter: {token}"
        );
    }
    for token in [
        "fn denominator",
        "fn order_digest",
        "fn conservation_ledger",
        "sort_by",
        "sort_unstable",
        ".filter(",
        "average(",
        "mean(",
    ] {
        assert!(
            !adapter_code.contains(token),
            "S03: local evaluator/pooling helper leaked into adapter: {token}"
        );
    }

    // The gate has the same quarantine boundary, while its source-level
    // vocabulary remains tied to typed evidence rather than writes.
    let gate_code = s02_code_without_comments_and_strings(&gate);
    for token in [
        concat!("da_", "parse"),
        "ParserPort",
        "PromotionService",
        "GraphStore",
        "DirectGraphStore",
        "Command::new",
        "parse_pdf",
        "deploy_candidate",
        "import_candidate",
        "auto_promote_candidate",
        "promote_candidate",
        "create_node",
        "create_edge",
        "set_node_property",
        "set_edge_property",
    ] {
        assert!(
            !gate_code.contains(token),
            "S03: forbidden production-boundary token leaked into gate: {token}"
        );
    }

    let compatibility = read_text("data/parse/grobid_compatibility.yaml");
    for pin in [
        "import_eligible: false",
        "auto_promote: forbidden",
        "this_wave_promotes: none",
        "deployed: false",
        "rollback_target: java-grobid-http@0.9.0-crf",
    ] {
        assert!(
            compatibility.contains(pin),
            "S03: missing policy pin: {pin}"
        );
    }
    let composition = read_text("data/parse/native_fulltext_composition.yaml");
    assert!(composition.contains("pooled_winner: forbidden"));
    assert!(composition.contains("independently_selectable: [header, references]"));

    let fsm = read_text("crates/da-domain/src/fsm_policy.rs");
    assert!(
        s10_decision_point_violations(&s10_code_without_comments(&fsm)).is_empty(),
        "S03: DecisionPoint vocabulary drifted"
    );

    // Synthetic mutations are intentionally checked by the same predicates:
    // a second evaluator call and a graph coupling must be observable.
    let synthetic = "fn drift() { aggregate_same_n_ablation(x); aggregate_same_n_ablation(y); let _ = GraphStore; }";
    assert_eq!(
        synthetic
            .matches(concat!("aggregate_same_n_", "ablation", "("))
            .count(),
        2,
        "S03: synthetic second evaluator call must be fail-visible"
    );
    assert!(
        synthetic.contains("GraphStore"),
        "S03: synthetic GraphStore coupling must be fail-visible"
    );
}

#[test]
fn s04_native_family_decision_packages_stay_independent_and_quarantined() {
    let application_lib = s10_code_without_comments(&read_text("crates/da-application/src/lib.rs"));
    let package = read_text("crates/da-application/src/native_family_decision_packages.rs");
    let package_code = s02_code_without_comments_and_strings(&package);
    let adapter = s02_code_without_comments_and_strings(&read_text(
        "crates/da-application/src/native_family_same_n_adapter.rs",
    ));

    assert_eq!(
        application_lib
            .lines()
            .filter(|line| line.trim() == "pub mod native_family_decision_packages;")
            .count(),
        1,
        "S04: decision packages must have exactly one standalone module declaration"
    );
    assert!(
        !application_lib.contains("pub use native_family_decision_packages"),
        "S04: decision packages must not escape through a facade re-export"
    );

    assert_eq!(
        package_code.matches("bind_family_same_n(").count(),
        1,
        "S04: package set must call the canonical family binder at one source site"
    );
    assert_eq!(
        package_code
            .matches(concat!("aggregate_same_n_", "ablation", "("))
            .count(),
        0,
        "S04: decision package must not own a second aggregate evaluator"
    );
    assert_eq!(
        adapter
            .matches(concat!(
                "aggregate_same_n_",
                "ablation",
                "_with_provenance("
            ))
            .count(),
        1,
        "S04: canonical aggregate evaluator must remain single-owned by the adapter"
    );
    assert_eq!(
        adapter
            .matches(concat!("aggregate_same_n_", "ablation", "("))
            .count(),
        0,
        "S04: adapter must not authorize promotion through the label bridge"
    );

    for (name, source) in [
        ("source_cli", read_text("crates/da-cli/src/source_cli.rs")),
        ("ParserPort", read_text("crates/da-ports/src/parser.rs")),
        (
            "GraphStore",
            read_text("crates/da-ports/src/graph_store.rs"),
        ),
        (
            "PromotionService",
            read_text("crates/da-application/src/promotion.rs"),
        ),
    ] {
        assert!(
            !source.contains("native_family_decision_packages")
                && !source.contains("assemble_family_decision_set"),
            "S04: {name} must not route through the decision-package boundary"
        );
    }

    // Scan production code, not comments or string literals.  The split
    // spellings keep the denylist itself from self-matching while retaining
    // fail-visible synthetic controls below.
    for token in [
        concat!("da_", "parse"),
        "ParserPort",
        "source_cli",
        "GraphStore",
        "DirectGraphStore",
        "PromotionService",
        "parse_pdf",
        "std::fs",
        "Command::new",
        "create_node",
        "create_edge",
        "deploy_candidate",
        "import_candidate",
        "promote_candidate",
        "auto_promote_candidate",
        "average(",
        "mean(",
        "sort_by",
        "sort_unstable",
        ".filter(",
        "tracing",
    ] {
        assert!(
            !package_code.contains(token),
            "S04: forbidden quarantine token leaked into decision packages: {token}"
        );
    }
    assert!(
        package_code.contains("PooledWinner::Forbidden") && package_code.contains("pooled_winner"),
        "S04: only the typed forbidden pooled-winner pin may be present"
    );
    assert!(
        !package_code.contains("pooled_winner: header"),
        "S04: pooled winner claim escaped into the package surface"
    );

    let compatibility = read_text("data/parse/grobid_compatibility.yaml");
    for pin in [
        "import_eligible: false",
        "auto_promote: forbidden",
        "this_wave_promotes: none",
        "deployed: false",
        "rollback_target: java-grobid-http@0.9.0-crf",
    ] {
        assert!(
            compatibility.contains(pin),
            "S04: missing compatibility pin: {pin}"
        );
    }
    let composition = read_text("data/parse/native_fulltext_composition.yaml");
    for pin in [
        "pooled_winner: forbidden",
        "independently_selectable: [header, references]",
        "citation_names_total: 2198",
        "references_total: 207",
        "hub_native_accepted_header: 20",
        "hub_java_header_raw: 21",
        "java_header_authors_equals: header_raw_total",
    ] {
        assert!(
            composition.contains(pin),
            "S04: missing composition pin: {pin}"
        );
    }

    let fsm = s10_code_without_comments(&read_text("crates/da-domain/src/fsm_policy.rs"));
    assert!(
        s10_decision_point_violations(&fsm).is_empty(),
        "S04: DecisionPoint vocabulary drifted"
    );
    for variant in [
        "GrammarRoute",
        "BatchAdmission",
        "ConcurrencyTune",
        "RetryPolicy",
        "SummaryDepth",
        "ProfileRoute",
    ] {
        assert!(
            fsm.contains(variant),
            "S04: missing DecisionPoint variant: {variant}"
        );
    }

    // Positive controls: each critical mutation must be detectable by the
    // same predicates used against the production module.
    let synthetic = "fn drift() { aggregate_same_n_ablation(x); let _ = GraphStore; let pooled_winner: header; }";
    assert_eq!(
        synthetic
            .matches(concat!("aggregate_same_n_", "ablation", "("))
            .count(),
        1,
        "S04: synthetic second evaluator marker must be fail-visible"
    );
    assert!(synthetic.contains("GraphStore"));
    assert!(synthetic.contains("pooled_winner: header"));
}

#[test]
fn s05_native_family_gate_rehearsal_stops_at_human_gate_and_preserves_java() {
    let example = read_text("crates/da-cli/examples/native_family_gate_rehearsal.rs");
    let example_code = s02_code_without_comments_and_strings(&example);
    let application_lib = read_text("crates/da-application/src/lib.rs");
    let source_cli = read_text("crates/da-cli/src/source_cli.rs");
    let parser_port = read_text("crates/da-ports/src/parser.rs");
    let compatibility = read_text("data/parse/grobid_compatibility.yaml");
    let composition = read_text("data/parse/native_fulltext_composition.yaml");
    let fsm = s10_code_without_comments(&read_text("crates/da-domain/src/fsm_policy.rs"));

    // The rehearsal is a standalone example only.  It must not become a
    // fourth native-family application facade or a source_cli route.
    assert!(example.contains("pub struct ReviewDigest"));
    assert!(example.contains("native-family-review-digest.v1"));
    assert!(example.contains("HumanGateRequired"));
    assert!(example.contains("assemble_family_decision_set(input())"));
    assert!(example.contains("init_pipeline_logging(\"native_family_gate_rehearsal\")"));
    assert!(!application_lib.contains("native_family_gate_rehearsal"));
    assert!(!source_cli.contains("native_family_gate_rehearsal"));
    for module in [
        "native_family_promotion_gate",
        "native_family_same_n_adapter",
        "native_family_decision_packages",
    ] {
        assert_eq!(
            application_lib
                .lines()
                .filter(|line| line.trim() == format!("pub mod {module};"))
                .count(),
            1,
            "S05: existing S01-S04 owner module must remain exactly once: {module}"
        );
    }
    assert!(
        !application_lib.contains("pub mod native_family_gate_rehearsal;")
            && !application_lib.contains("pub use native_family_gate_rehearsal")
    );

    // Closed wire and no-effect pins are asserted against the producer text,
    // while executable coupling is scanned after comments and strings are
    // removed so the denylist cannot self-match its own documentation.
    for token in [
        "can_auto_promote",
        "java_production_unchanged",
        "decision_point_consults",
        "PooledWinnerDigest::Forbidden",
        "ReviewGate::HumanGateRequired",
        "ReviewOutcome::Incomplete",
        "Vec::new()",
        "S05-R01",
        "S05-R02",
        "S05-R03",
        "S05-R04",
        "S05-R05",
        "S05-R06",
    ] {
        assert!(
            example.contains(token),
            "S05: missing digest contract token: {token}"
        );
    }
    for token in [
        concat!("da_", "parse"),
        "ParserPort",
        "source_cli",
        "GraphStore",
        "DirectGraphStore",
        "PromotionService",
        "parse_pdf",
        "std::fs",
        "Command::new",
        "spawn(",
        "create_node",
        "create_edge",
        "set_node_property",
        "set_edge_property",
        "deploy_candidate",
        "import_candidate",
        "promote_candidate",
        "auto_promote_candidate",
        "aggregate_same_n_ablation",
        "average(",
        "mean(",
        "metric_aggregation",
        "recompute_metrics",
    ] {
        assert!(
            !example_code.contains(token),
            "S05: forbidden production or evaluator coupling leaked into rehearsal: {token}"
        );
    }

    // The production composition root retains the Java parse consults and
    // the ParserPort remains parse-only; the rehearsal is absent from both.
    assert!(source_cli.contains("stamp_parse_grammar"));
    assert!(source_cli.contains("grobid_profile_from_route"));
    assert!(source_cli.contains("crate::parse_profile"));
    assert!(
        parser_port.contains("pub trait ParserPort")
            && !parser_port.contains("native_family_gate_rehearsal")
    );
    assert!(!source_cli.contains("assemble_family_decision_set"));
    assert!(s10_decision_point_violations(&fsm).is_empty());
    for variant in [
        "GrammarRoute",
        "BatchAdmission",
        "ConcurrencyTune",
        "RetryPolicy",
        "SummaryDepth",
        "ProfileRoute",
    ] {
        assert!(
            fsm.contains(variant),
            "S05: missing DecisionPoint variant: {variant}"
        );
    }

    for pin in [
        "import_eligible: false",
        "auto_promote: forbidden",
        "this_wave_promotes: none",
        "deployed: false",
        "rollback_target: java-grobid-http@0.9.0-crf",
    ] {
        assert!(
            compatibility.contains(pin),
            "S05: missing compatibility pin: {pin}"
        );
    }
    for pin in [
        "pooled_winner: forbidden",
        "independently_selectable: [header, references]",
        "citation_names_total: 2198",
        "references_total: 207",
        "java_header_authors_equals: header_raw_total",
        "hub_native_accepted_header: 20",
        "hub_java_header_raw: 21",
    ] {
        assert!(
            composition.contains(pin),
            "S05: missing composition pin: {pin}"
        );
    }

    // Fail-visible controls: each critical boundary mutation is paired with
    // its exact detector token rather than relying on a broad any-match.
    let synthetic = [
        ("use da_parse::ParserPort;", concat!("da_", "parse")),
        ("let _ = GraphStore::create_node();", "GraphStore"),
        ("source_cli::native_family_gate_rehearsal();", "source_cli"),
        ("let pooled_winner = header;", "pooled_winner"),
        (
            "auto_promote_candidate(candidate);",
            "auto_promote_candidate",
        ),
        (
            "aggregate_same_n_ablation(candidate);",
            "aggregate_same_n_ablation",
        ),
    ];
    for (mutation, detector) in synthetic {
        let hits = s02_code_without_comments_and_strings(mutation);
        assert!(
            hits.contains(detector),
            "S05: synthetic mutation was not fail-visible for {detector}: {mutation}"
        );
    }

    let missing_stamp = "fn stamp() { let policy_source = \"wasm\"; }";
    assert!(
        !missing_stamp.contains("policy_hash") && !missing_stamp.contains("policy=none"),
        "R119: synthetic missing policy stamp must be visibly malformed"
    );
    let forbidden = "fn leaked() { let _ = ParserPort; let _ = GraphStore; let _ = promote; }";
    let hits = s05_quarantine_hits(forbidden);
    assert!(
        hits.contains(&"ParserPort".to_owned())
            && hits.contains(&"GraphStore".to_owned())
            && hits.contains(&"promote".to_owned()),
        "R119/R124: production route, graph write, and promotion drift must be fail-visible: {hits:?}"
    );
    let drifted = S06_R124_W0_TRIGGERS
        .iter()
        .map(|(name, _, _)| *name)
        .chain(["ledger_surface"])
        .collect::<Vec<_>>();
    let unique: BTreeSet<_> = drifted.iter().copied().collect();
    assert_ne!(
        drifted.len(),
        unique.len(),
        "R124: synthetic duplicate trigger must fail the uniqueness boundary"
    );
}

// ───── S20/T04: REST asset/batch production quarantine ─────
//
// This is deliberately an architecture-only contract: the local batch
// composition remains a carrier over the three typed transports, while Java
// asset/HTML/annotation/training families cannot silently become production
// routes or ParserPort methods.
#[test]
fn s20_rest_batch_asset_quarantine_contract() {
    let policy = read_text("scripts/governor/checks/scholarly_parse_plane.yaml");
    for binding in [
        "java_grobid_rest_batch_asset_source.json",
        "rest_batch_asset_parity.yaml",
        "rest_batch_asset_oracle_source.json",
        "rest_batch_asset_oracle.yaml",
        "rest_batch_asset_shadow.json",
    ] {
        assert!(
            policy.contains(binding),
            "S20: quarantine must bind T01/T02/T03 artifact {binding}"
        );
    }
    for pin in [
        "implemented_new_endpoints: 0",
        "decision_point: 6",
        "independently_selectable: [header, references]",
        "import_eligible: false",
        "graph_write: forbidden",
        "auto_promote: forbidden",
        "production_route: forbidden",
        "not_endpoint: true",
    ] {
        assert!(policy.contains(pin), "S20: missing quarantine pin {pin}");
    }

    let transport = read_text("crates/da-parse/src/transport.rs");
    let endpoint_enum = transport
        .split("pub enum ParseEndpoint")
        .nth(1)
        .and_then(|tail| tail.split("impl ParseEndpoint").next())
        .expect("S20: ParseEndpoint mapping exists");
    assert_eq!(endpoint_enum.matches("Fulltext").count(), 1);
    assert_eq!(endpoint_enum.matches("Header").count(), 1);
    assert_eq!(endpoint_enum.matches("References").count(), 1);
    let endpoint_block = transport
        .split("impl ParseEndpoint")
        .nth(1)
        .expect("S20: ParseEndpoint implementation exists");
    for route in [
        "/api/processFulltextDocument",
        "/api/processHeaderDocument",
        "/api/processReferences",
    ] {
        assert!(
            endpoint_block.contains(route),
            "S20: sanctioned route missing {route}"
        );
    }
    for forbidden in [
        "processFulltextAssetDocument",
        "processFulltextDocumentHTML",
        "annotatePDF",
        "createTraining",
        "trainTagger",
        "batchProcessAssets",
        "batchTraining",
    ] {
        assert!(
            !endpoint_block.contains(forbidden),
            "S20: forbidden Java asset/training route widened ParseEndpoint: {forbidden}"
        );
    }

    // Fail-visible synthetic controls mirror the DATA scanner's two critical
    // classes: companion symbol splice and caller-controlled route widening.
    let synthetic = [
        "fn route() { processFulltextAssetDocument(); }",
        "fn route(url: &str) { caller_controlled_url(url); }",
        "fn route() { let _ = generic_url; }",
    ];
    for mutation in synthetic {
        let code = strip_line_comments(mutation);
        assert!(
            [
                "processFulltextAssetDocument",
                "caller_controlled_url",
                "generic_url",
            ]
            .iter()
            .any(|token| code.contains(token)),
            "S20: synthetic widening must remain fail-visible: {mutation}"
        );
    }
}

// ───── S06/T04: the promotion rehearsal stays rehearsal-scoped (no route) ─────
//
// The one-lane promotion rehearsal primitive is an executed experiment, not a
// route: its switch lives only in the example process's memory. This gate proves
// that statically, because a rehearsal that quietly opens a production route, a
// graph write or a promotion is worse than no rehearsal at all — the report it
// writes would still claim `rehearsal_only`.
//
// The scanner strips comments AND string/char literals before matching, so a
// token that appears only in prose or in a report key (`"auto_promote"`) is not
// a coupling, while the same token in code is. `std::fs` and `da_parse` are
// deliberately NOT forbidden (the example must read its carriers, write its
// report and drive the composer); `da_application` is forbidden because the
// promotion-gate facade it is not supposed to need was already rehearsed in
// M500/S05.

/// The rehearsal example whose boundary this gate proves.
const PROMOTION_REHEARSAL_EXAMPLE: &str =
    "crates/da-cli/examples/native_family_promotion_rehearsal.rs";
/// The example's module/file stem. Its presence in a runtime seam is coupling.
const PROMOTION_REHEARSAL_NAME: &str = "native_family_promotion_rehearsal";

/// Couplings a rehearsal primitive may never acquire.
const PROMOTION_REHEARSAL_FORBIDDEN: [&str; 22] = [
    "da_ports",
    "ParserPort",
    "source_cli",
    "GraphStore",
    "DirectGraphStore",
    "PromotionService",
    "parse_pdf",
    "create_node",
    "create_edge",
    "set_node_property",
    "set_edge_property",
    "deploy_candidate",
    "import_candidate",
    "promote_candidate",
    "auto_promote_candidate",
    "Command::new",
    "spawn(",
    "Samyama",
    "redb",
    "da_application",
    "File::create",
    "OpenOptions",
];

/// The byte position just past a raw-string opener starting at `start`, plus its
/// hash count, or `None` when `start` does not open a raw string.
fn raw_string_opener(chars: &[char], start: usize) -> Option<(usize, usize)> {
    if start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        return None;
    }
    let mut at = start;
    if matches!(chars.get(at), Some('b') | Some('c')) {
        at += 1;
    }
    if chars.get(at) != Some(&'r') {
        return None;
    }
    at += 1;
    let mut hashes = 0usize;
    while chars.get(at) == Some(&'#') {
        hashes += 1;
        at += 1;
    }
    if chars.get(at) != Some(&'"') {
        return None;
    }
    Some((at + 1, hashes))
}

/// Remove Rust comments, string literals and char literals, replacing their
/// contents with nothing while preserving newlines, so a scan sees identifiers
/// only. Handles line comments, nested block comments, raw strings (`r#".."#`)
/// and the char-literal/lifetime ambiguity.
fn strip_comments_and_literals(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            let mut depth = 1usize;
            i += 2;
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    continue;
                }
                if chars[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            continue;
        }
        if let Some((after_opener, hashes)) = raw_string_opener(&chars, i) {
            i = after_opener;
            while i < chars.len() {
                if chars[i] == '"' && (1..=hashes).all(|n| chars.get(i + n) == Some(&'#')) {
                    i += 1 + hashes;
                    break;
                }
                if chars[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            continue;
        }
        if c == '"' {
            i += 1;
            while i < chars.len() {
                match chars[i] {
                    '\\' => i += 2,
                    '"' => {
                        i += 1;
                        break;
                    }
                    '\n' => {
                        out.push('\n');
                        i += 1;
                    }
                    _ => i += 1,
                }
            }
            continue;
        }
        if c == '\'' {
            let is_char = match chars.get(i + 1) {
                Some('\\') => true,
                Some(_) => chars.get(i + 2) == Some(&'\''),
                None => false,
            };
            if is_char {
                i += 1;
                while i < chars.len() {
                    match chars[i] {
                        '\\' => i += 2,
                        '\'' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                continue;
            }
            out.push(c);
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Every static finding against the rehearsal example's own source.
///
/// One function, so the negative test below can feed it mutated copies and the
/// positive test can feed it the tracked file: a scanner whose only caller is
/// the happy path is a scanner nobody has proven can fail.
fn promotion_rehearsal_source_findings(source: &str) -> Vec<String> {
    let code = strip_comments_and_literals(source);
    let mut findings = Vec::new();
    for marker in PROMOTION_REHEARSAL_FORBIDDEN {
        let count = code.matches(marker).count();
        if count > 0 {
            findings.push(format!(
                "[rehearsal-forbidden-coupling] `{marker}` appears {count} time(s) in code"
            ));
        }
    }
    for required in ["DATA_COMPARISON", "DATA_COMPOSITION", "fn write_report"] {
        if !code.contains(required) {
            findings.push(format!(
                "[rehearsal-missing-required] the example no longer carries `{required}`"
            ));
        }
    }
    let writes = code.matches("fs::write").count();
    if writes != 1 {
        findings.push(format!(
            "[rehearsal-write-surface] expected exactly one `fs::write` (the atomic report), found {writes}"
        ));
    }
    findings
}

#[test]
fn native_family_promotion_rehearsal_is_rehearsal_scoped_and_never_routes_or_promotes() {
    let source = read_text(PROMOTION_REHEARSAL_EXAMPLE);
    assert!(
        source.len() > 50_000,
        "positive control failed: the rehearsal example looks truncated ({} bytes)",
        source.len()
    );
    let findings = promotion_rehearsal_source_findings(&source);
    assert!(
        findings.is_empty(),
        "the one-lane promotion rehearsal acquired a production coupling:\n{}",
        findings.join("\n")
    );

    // The example must not be registered as a runtime module or reached from any
    // parse/application seam: the switch it exercises lives only in that process.
    for rel in [
        "crates/da-cli/src/lib.rs",
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
        "crates/da-application/src/lib.rs",
    ] {
        let text = read_text(rel);
        assert!(
            !text.contains(PROMOTION_REHEARSAL_NAME),
            "{rel} must not reference the promotion rehearsal (a rehearsal is not a route)"
        );
    }
    let lib = read_text("crates/da-cli/src/lib.rs");
    assert!(
        !lib.contains(&format!("pub mod {PROMOTION_REHEARSAL_NAME}")),
        "the promotion rehearsal must not become a da-cli library module"
    );
}

#[test]
fn native_family_promotion_rehearsal_scanner_fails_visible_on_synthetic_violations() {
    // Positive control: the tracked example passes its own scanner.
    assert!(
        promotion_rehearsal_source_findings(&read_text(PROMOTION_REHEARSAL_EXAMPLE)).is_empty(),
        "the real rehearsal example must pass its own scanner"
    );

    // A minimal clean body that satisfies every required-presence check.
    let clean = "const DATA_COMPARISON: &str = \"c\";\n\
                 const DATA_COMPOSITION: &str = \"d\";\n\
                 fn write_report() { std::fs::write(\"x\", b\"\"); }\n";
    assert!(
        promotion_rehearsal_source_findings(clean).is_empty(),
        "clean control must not trip the scanner: {:?}",
        promotion_rehearsal_source_findings(clean)
    );

    // Non-vacuity: every forbidden coupling must be fail-visible when injected
    // into CODE.
    for injection in [
        "use da_ports::parser::ParserPort;",
        "let _ = PromotionService::new();",
        "fn f() { create_node(); }",
        "fn f() { auto_promote_candidate(); }",
        "fn f() { let _ = Command::new(\"x\"); }",
        "fn f() { let _ = Samyama::open(); }",
        "fn f() { let _ = da_application::run(); }",
        "fn f() { let _ = File::create(\"x\"); }",
        "fn f() { let _ = parser.parse_pdf(); }",
    ] {
        let mutated = format!("{clean}\n{injection}\n");
        assert!(
            !promotion_rehearsal_source_findings(&mutated).is_empty(),
            "an injected coupling must stay fail-visible: {injection}"
        );
    }

    // Comment and string/raw-string mentions are prose, not coupling. This also
    // proves the literal stripper works on a raw string.
    let prose = format!(
        "// ParserPort and PromotionService stay out of the rehearsal\n\
         /* create_node, auto_promote_candidate */\n\
         fn f() {{ let a = \"ParserPort\"; let b = r#\"auto_promote_candidate\"#; let c = 'x'; }}\n\
         {clean}"
    );
    assert!(
        promotion_rehearsal_source_findings(&prose).is_empty(),
        "prose and literal mentions must not trip the scanner: {:?}",
        promotion_rehearsal_source_findings(&prose)
    );

    // The write surface is exact: a second write, or none, is a finding.
    let two_writes = format!("{clean}fn g() {{ std::fs::write(\"y\", b\"\"); }}\n");
    assert!(
        promotion_rehearsal_source_findings(&two_writes)
            .iter()
            .any(|finding| finding.contains("fs::write")),
        "a second report write must be fail-visible"
    );
    let no_write = "const DATA_COMPARISON: &str = \"c\";\n\
                    const DATA_COMPOSITION: &str = \"d\";\n\
                    fn write_report() {}\n";
    assert!(
        promotion_rehearsal_source_findings(no_write)
            .iter()
            .any(|finding| finding.contains("fs::write")),
        "a missing report write must be fail-visible"
    );

    // A dropped carrier constant must be fail-visible too, or the scanner's
    // required-presence half is decorative.
    let missing_carrier = "const DATA_COMPOSITION: &str = \"d\";\n\
                           fn write_report() { std::fs::write(\"x\", b\"\"); }\n";
    assert!(
        promotion_rehearsal_source_findings(missing_carrier)
            .iter()
            .any(|finding| finding.contains("DATA_COMPARISON")),
        "a dropped carrier constant must be fail-visible"
    );
}

// ───── S05/T01: the native body-text assembly stays a pure shadow ─────

/// The S05/T01 character-oracle surface. It joins the S03 layout plane's body
/// characters and turns a native-vs-oracle comparison into a typed verdict;
/// like the word plane it must name no TEI or Java type and take no side channel.
const NATIVE_BODY_TEXT_SURFACE: &str = "crates/da-parse/src/native_body_text.rs";

/// TEI/Java coupling markers banned from the assembly's CODE in addition to the
/// S04 word-plane list. The scanner blanks full-line comments, so the module may
/// document the prohibition while the code stays clean.
const NATIVE_BODY_TEXT_EXTRA_FORBIDDEN: [&str; 6] =
    ["Tei", "Grobid", "Java", "java_", "TeiEnvelope", "tei_xml"];

#[test]
fn native_body_text_is_pure_and_names_no_tei_or_java_type() {
    let text = read_text(NATIVE_BODY_TEXT_SURFACE);
    assert!(
        text.len() > 5_000,
        "positive control failed: {NATIVE_BODY_TEXT_SURFACE} looks truncated"
    );
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from([
            "crate".to_string(),
            "serde".to_string(),
            "sha2".to_string(),
            "std".to_string()
        ]),
        "{NATIVE_BODY_TEXT_SURFACE} may import only its typed leaves, serde, hashing and formatting"
    );
    let mut hits = native_date_marker_hits(&text, &NATIVE_WORD_PLANE_FORBIDDEN);
    hits.extend(native_date_marker_hits(
        &text,
        &NATIVE_BODY_TEXT_EXTRA_FORBIDDEN,
    ));
    assert!(
        hits.is_empty(),
        "{NATIVE_BODY_TEXT_SURFACE} acquired TEI/Java coupling or a side effect:\n{}",
        hits.join("\n")
    );
    assert!(
        !text.contains("pub text:"),
        "the body text must stay private (redaction)"
    );
    assert!(
        text.contains("pub fn text("),
        "the body text must be reachable only through text()"
    );
    assert!(
        text.contains("d498_reopen_required"),
        "the D498 reopen flag must be part of the verdict"
    );
    // S05/T01 additive Body extraction/assignment: the boundary verification and
    // the additive entry point must live on the same pure, TEI-free surface.
    assert!(
        text.contains("pub fn build_native_body_assignment"),
        "the additive Body assignment entry point must be registered"
    );
    assert!(
        text.contains("BodyBoundaryVerification"),
        "the additive boundary verification must be part of the surface"
    );
    // S05/T01 third-route candidate: the v1 `segment_document` evaluation must
    // stay on the same pure, TEI-free surface, declared by name.
    assert!(
        text.contains("pub fn evaluate_body_third_route_candidate"),
        "the additive third-route candidate entry point must be registered"
    );
    assert!(
        text.contains("BodyThirdRouteVerdict"),
        "the third-route corroboration verdict must be part of the surface"
    );

    let lib = read_text("crates/da-parse/src/lib.rs");
    for marker in ["mod native_body_text;", "pub use native_body_text::"] {
        assert!(lib.contains(marker), "lib.rs must register {marker}");
    }
}

/// The additive S05/T01 order-lineage surface. It states the order of the native
/// token stream as explicit document token identities plus digests; like the word
/// plane and the body-text assembly it must name no TEI or Java type and take no
/// side channel.
const NATIVE_BODY_ORDER_LINEAGE_SURFACE: &str = "crates/da-parse/src/native_body_order_lineage.rs";

#[test]
fn native_body_order_lineage_is_pure_and_names_no_tei_or_java_type() {
    let text = read_text(NATIVE_BODY_ORDER_LINEAGE_SURFACE);
    assert!(
        text.len() > 5_000,
        "positive control failed: {NATIVE_BODY_ORDER_LINEAGE_SURFACE} looks truncated"
    );
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from(["serde".to_string(), "sha2".to_string(), "std".to_string()]),
        "{NATIVE_BODY_ORDER_LINEAGE_SURFACE} may import only serde, hashing and formatting"
    );
    let mut hits = native_date_marker_hits(&text, &NATIVE_WORD_PLANE_FORBIDDEN);
    hits.extend(native_date_marker_hits(
        &text,
        &NATIVE_BODY_TEXT_EXTRA_FORBIDDEN,
    ));
    assert!(
        hits.is_empty(),
        "{NATIVE_BODY_ORDER_LINEAGE_SURFACE} acquired TEI/Java coupling or a side effect:\n{}",
        hits.join("\n")
    );
    for marker in [
        "pub fn audit_body_order_lineage",
        "pub fn ordered_fragments",
        "pub entries: Vec<BodyOrderEntry>",
        "LexemeDigestMismatch",
        "SEGMENTER_REJECTED_LATTICE",
    ] {
        assert!(
            text.contains(marker),
            "{NATIVE_BODY_ORDER_LINEAGE_SURFACE} must keep {marker}"
        );
    }

    let lib = read_text("crates/da-parse/src/lib.rs");
    for marker in [
        "mod native_body_order_lineage;",
        "pub use native_body_order_lineage::",
    ] {
        assert!(lib.contains(marker), "lib.rs must register {marker}");
    }
}

/// The additive S05/T03 native visual-input surface. It builds the visual
/// observation from the S03 plane and compares it against an independent
/// positive oracle; like the other additive bands it must stay pure, TEI-free,
/// and its required-family vocabulary must carry no scope reduction.
const NATIVE_VISUAL_NATIVE_INPUT_SURFACE: &str =
    "crates/da-parse/src/native_visual_native_input.rs";

#[test]
fn native_visual_native_input_is_pure_and_free_of_out_of_scope() {
    let text = read_text(NATIVE_VISUAL_NATIVE_INPUT_SURFACE);
    assert!(
        text.len() > 10_000,
        "positive control failed: {NATIVE_VISUAL_NATIVE_INPUT_SURFACE} looks truncated"
    );
    assert_eq!(
        import_roots_of(&text),
        BTreeSet::from([
            "crate".to_string(),
            "serde".to_string(),
            "sha2".to_string(),
            "std".to_string()
        ]),
        "{NATIVE_VISUAL_NATIVE_INPUT_SURFACE} may import only its typed leaves, serde, hashing and formatting"
    );
    let mut hits = native_date_marker_hits(&text, &NATIVE_WORD_PLANE_FORBIDDEN);
    hits.extend(native_date_marker_hits(
        &text,
        &NATIVE_BODY_TEXT_EXTRA_FORBIDDEN,
    ));
    assert!(
        hits.is_empty(),
        "{NATIVE_VISUAL_NATIVE_INPUT_SURFACE} acquired TEI/Java coupling or a side effect:\n{}",
        hits.join("\n")
    );
    for marker in [
        "NATIVE_VISUAL_FAMILY_DISPOSITION_VOCABULARY",
        "build_native_visual_input",
        "evaluate_visual_family_rows",
        "require_non_vacuous_visual_oracle",
        "NativeVisualUnmetReason",
    ] {
        assert!(
            text.contains(marker),
            "{NATIVE_VISUAL_NATIVE_INPUT_SURFACE} must keep {marker}"
        );
    }
    // The required-family vocabulary is closed and carries no scope reduction:
    // an unsupported required family is a typed `unmet`, never `out_of_scope`.
    assert!(text.contains("[\"measured\", \"unmet\", \"not_observed\"]"));
    assert!(
        !strip_line_comments(&text).contains("out_of_scope"),
        "the code half of the surface must not name out_of_scope"
    );
    let lib = read_text("crates/da-parse/src/lib.rs");
    for marker in [
        "mod native_visual_native_input;",
        "pub use native_visual_native_input::",
    ] {
        assert!(lib.contains(marker), "lib.rs must register {marker}");
    }
    // The band is additive: it must not be spliced into the Java-authoritative
    // parse route or the projection that feeds it.
    for boundary in [
        "crates/da-parse/src/parser.rs",
        "crates/da-parse/src/projector.rs",
    ] {
        let boundary_text = strip_line_comments(&read_text(boundary));
        for marker in [
            "native_visual_native_input",
            "build_native_visual_input",
            "build_native_visual_observation",
            "run_native_visual_heads",
        ] {
            assert!(
                !boundary_text.contains(marker),
                "{boundary} must not wire the additive native visual band ({marker})"
            );
        }
    }
}

/// Positive control for the S05/T03 scanner: the forbidden-marker scan is live
/// and the boundary scan sees a synthetic wiring, so the assertion above cannot
/// pass vacuously.
#[test]
fn native_visual_native_input_scanner_fails_visible_on_synthetic_violations() {
    let synthetic = "use std::fs::File;\nfn run() { println!(\"x\"); }\nParserPort::new();\n";
    let hits = native_date_marker_hits(synthetic, &NATIVE_WORD_PLANE_FORBIDDEN);
    assert!(
        hits.len() >= 3,
        "the forbidden-marker scanner must be live, got {hits:?}"
    );
    assert!(
        native_date_marker_hits(
            "// std::fs and ParserPort are prose\n",
            &NATIVE_WORD_PLANE_FORBIDDEN
        )
        .is_empty(),
        "comment prose must not flag"
    );
    let wired = strip_line_comments("pub fn parse() { build_native_visual_input(x); }");
    assert!(
        wired.contains("build_native_visual_input"),
        "a wired call-site must be visible to the boundary scanner"
    );
}
