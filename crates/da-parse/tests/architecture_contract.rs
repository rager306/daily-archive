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

/// Typed request/failure vocabulary surfaces: the provider-neutral protocol
/// layer and the legacy compatibility projection. `crates/da-parse/src/
/// http.rs` is deliberately NOT scanned here — the facade shell owns the
/// sanctioned env/URL resolution documented in the ledger (GROBID_URL).
const REQUEST_FAILURE_SURFACES: [&str; 3] = [
    "crates/da-parse/src/transport.rs",
    "crates/da-parse/src/backend.rs",
    "crates/da-parse/src/profile.rs",
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
    let mut hits = Vec::new();
    for (line_no, raw) in strip_line_comments(text).lines().enumerate() {
        for marker in SURFACE_FORBIDDEN_MARKERS {
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
fn request_and_failure_surfaces_stay_free_of_paths_env_and_cache_roots() {
    for rel in REQUEST_FAILURE_SURFACES {
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
        "the sanctioned proxy set must stay exactly these five capabilities"
    );
}
