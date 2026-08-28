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
//! The scanners' negative behavior is unit-tested in-file on synthetic
//! inputs, so the gate itself is fail-visible rather than fail-silent.

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
const FORBIDDEN_DEP_NAMES: [&str; 16] = [
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

[dev-dependencies]
da-application = { path = \"../da-application\" }
";
    let names = manifest_dependency_names(synthetic);
    assert_eq!(names.len(), 4, "scanner must read every dependency table");
    let hits = forbidden_dependency_hits(&names);
    assert_eq!(hits.len(), 3, "expected three forbidden hits, got {hits:?}");
    assert!(hits.iter().any(|h| h.contains("`samyama`")));
    assert!(hits.iter().any(|h| h.contains("`kg-storage`")));
    assert!(hits.iter().any(|h| h.contains("`da-application`")));

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
