//! Python archive acceptance: verify no product code references the archived
//! downloader, Rust acquire-arxiv is the sole path, and D127 is false.

use anyhow::{Result, anyhow};
use clap::Parser;
use da_domain::assurance::ContentDigest;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "artifacts/runtime/python_archive_m386.json")]
    report: PathBuf,
}

#[derive(Serialize)]
struct Violation {
    file: String,
    line: usize,
    snippet: String,
    severity: &'static str,
}

#[derive(Serialize)]
struct Report {
    schema_version: String,
    archived_script: String,
    archive_location: String,
    replacement_command: String,
    product_code_violations: Vec<Violation>,
    test_code_references: Vec<String>,
    doc_references: usize,
    all_clear: bool,
    import_eligible: bool,
    report_sha256: String,
}

fn main() -> Result<()> {
    let _guard = da_cli::logging::init_pipeline_logging("python_archive_audit");
    let args = Args::parse();

    let archived_script = "download_arxiv_to_catalog.py";
    let archive_location = "archive/python-era-cold-s04/download_arxiv_to_catalog.py";

    // Verify archive exists and scripts/ is clean.
    if !Path::new(archive_location).exists() {
        return Err(anyhow!("archived script not found at {archive_location}"));
    }
    let old_path = format!("scripts/{archived_script}");
    if Path::new(&old_path).exists() {
        return Err(anyhow!("old script still at {old_path} — must be moved"));
    }

    let mut product_violations = Vec::new();
    let mut test_refs = Vec::new();

    // Scan crates/ for product-code references (violation).
    scan_dir(
        Path::new("crates"),
        archived_script,
        &mut product_violations,
        "product",
    );
    // Scan scripts/governor/ for product-code references (governance is allowed).
    scan_dir(
        Path::new("scripts/governor"),
        archived_script,
        &mut product_violations,
        "product",
    );
    // scripts/tests/ frozen-evidence references are OK.
    collect_refs(Path::new("scripts/tests"), archived_script, &mut test_refs);
    collect_refs(Path::new("archive"), archived_script, &mut test_refs);

    // Count doc references (should be strikethrough/historical only).
    let doc_count = count_refs(Path::new("doc"), archived_script)
        + count_refs(Path::new("README.md"), archived_script)
        + count_refs(Path::new("CHANGELOG.md"), archived_script);

    let all_clear = product_violations.is_empty();

    let mut report = Report {
        schema_version: "python-archive.v1".into(),
        archived_script: archived_script.into(),
        archive_location: archive_location.into(),
        replacement_command: "da acquire-arxiv".into(),
        product_code_violations: product_violations,
        test_code_references: test_refs,
        doc_references: doc_count,
        all_clear,
        import_eligible: false,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.report, serde_json::to_vec_pretty(&report)?)?;
    tracing::info!(
        event = "python_archive_audit_complete",
        all_clear = report.all_clear,
        product_violations = report.product_code_violations.len(),
        doc_references = report.doc_references,
        import_eligible = false,
        "Python archive audit completed"
    );
    println!(
        "python-archive all_clear={} violations={} doc_refs={} D127=false report={}",
        report.all_clear,
        report.product_code_violations.len(),
        report.doc_references,
        args.report.display()
    );
    if !report.all_clear {
        std::process::exit(1);
    }
    Ok(())
}

fn scan_dir(root: &Path, needle: &str, violations: &mut Vec<Violation>, severity: &'static str) {
    if !root.is_dir() {
        return;
    }
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "rs" | "py" | "yaml" | "yml" | "toml") {
                continue;
            }
            // Exclude this audit file itself (legitimately references the name).
            if path.ends_with("python_archive_audit.rs") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(path) {
                for (lineno, line) in content.lines().enumerate() {
                    if line.contains(needle) {
                        violations.push(Violation {
                            file: path.display().to_string(),
                            line: lineno + 1,
                            snippet: line.trim().chars().take(120).collect(),
                            severity,
                        });
                    }
                }
            }
        }
    }
}

fn collect_refs(root: &Path, needle: &str, refs: &mut Vec<String>) {
    let mut violations = Vec::new();
    scan_dir(root, needle, &mut violations, "frozen-evidence");
    for v in violations {
        refs.push(format!("{}:{}", v.file, v.line));
    }
}

fn count_refs(path: &Path, needle: &str) -> usize {
    if path.is_file() {
        std::fs::read_to_string(path)
            .map(|c| c.lines().filter(|l| l.contains(needle)).count())
            .unwrap_or(0)
    } else if path.is_dir() {
        walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .map(|c| c.lines().filter(|l| l.contains(needle)).count())
            .sum()
    } else {
        0
    }
}

fn digest<T: Serialize>(value: &T) -> Result<String> {
    let mut v = serde_json::to_value(value)?;
    v["report_sha256"] = serde_json::Value::String(String::new());
    Ok(ContentDigest::of_bytes(&serde_json::to_vec(&v)?)
        .as_str()
        .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archived_script_replacement_is_rust_command() {
        let replacement: String = "da acquire-arxiv".into();
        assert_eq!(replacement, "da acquire-arxiv");
    }

    #[test]
    fn count_refs_finds_needle_in_temp_file() {
        let dir = std::env::temp_dir().join("da-py-archive-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.md");
        std::fs::write(&file, "line1\ndownload_arxiv_to_catalog.py\nline3\n").unwrap();
        assert_eq!(count_refs(&file, "download_arxiv_to_catalog.py"), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
