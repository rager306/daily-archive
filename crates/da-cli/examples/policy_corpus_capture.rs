//! Freeze both parse arms without touching production state or choosing a winner.

#[path = "policy_corpus/shared.rs"]
mod shared;

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use da_application::parse_policy_pair::ParseArm;
use da_application::parse_policy_table::PolicyOutcomeEvidence;
use da_application::pdf_toc_stage::{PdfTocStageError, TocObservations, build_pdf_toc_stage};
use da_domain::fsm_policy::ArticleFeatures;
use da_ports::{PdfClassificationPort, parser::ParserPort};
use shared::{
    ArmCapture, Artifact, CAPTURE_SCHEMA, Capture, CaptureStatus, ExporterIdentity,
    ReferenceCapture,
};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    pdf: PathBuf,
    #[arg(long)]
    paper_id: String,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value = "python3")]
    python: String,
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

fn source_artifact(path: &Path) -> Result<Artifact> {
    Ok(Artifact {
        path: path.to_str().context("artifact path is not UTF-8")?.into(),
        sha256: sha256(&fs::read(path)?),
    })
}

fn write_artifact(out: &Path, relative: &str, bytes: &[u8]) -> Result<Artifact> {
    let path = out.join(relative);
    fs::create_dir_all(path.parent().context("artifact has no parent")?)?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?
        .write_all(bytes)?;
    Ok(Artifact {
        path: relative.into(),
        sha256: sha256(bytes),
    })
}

fn new_arm(arm: ParseArm) -> ArmCapture {
    ArmCapture {
        arm,
        decision_code: arm.decision_code().into(),
        grammar_profile: match arm {
            ParseArm::Toc => "page_outline",
            ParseArm::Grobid => "arxiv_pdf_dot",
        }
        .into(),
        backend_profile: (arm == ParseArm::Grobid).then(|| "structural".into()),
        status: CaptureStatus::Failed,
        error: None,
        article: None,
        evidence: None,
        backend_version: None,
        latency_ms: 0,
    }
}

fn article_evidence(value: &serde_json::Value, backend: &str) -> Result<PolicyOutcomeEvidence> {
    let sections = value
        .get("sections")
        .and_then(serde_json::Value::as_array)
        .context("parsed article has no sections array")?;
    let body_chars = sections.iter().try_fold(0usize, |count, section| {
        let text = section
            .get("text")
            .and_then(serde_json::Value::as_str)
            .context("parsed section has no text")?;
        Ok::<_, anyhow::Error>(count + text.chars().count())
    })?;
    Ok(PolicyOutcomeEvidence {
        ok: !sections.is_empty() && body_chars > 0,
        section_count: sections.len(),
        body_chars,
        latency_ms: 0,
        backend: Some(backend.into()),
    })
}

fn finish_arm(mut arm: ArmCapture, started: Instant, result: Result<()>) -> ArmCapture {
    arm.latency_ms = started.elapsed().as_millis() as u64;
    if let Some(evidence) = &mut arm.evidence {
        evidence.latency_ms = arm.latency_ms;
    }
    if let Err(error) = result {
        arm.status = CaptureStatus::Failed;
        arm.error = Some(format!("{error:#}"));
    } else if arm.status == CaptureStatus::Success
        && arm.evidence.as_ref().is_some_and(|evidence| !evidence.ok)
    {
        arm.status = CaptureStatus::Failed;
        arm.error = Some("parsed article has no nonempty section text".into());
    }
    arm
}

fn record_toc_article(
    arm: &mut ArmCapture,
    paper_id: &str,
    observations: &str,
    out: &Path,
    source_hash: &str,
    backend_version: &str,
) -> Result<()> {
    let parsed = match build_pdf_toc_stage(paper_id, observations) {
        Ok(parsed) => parsed,
        Err(PdfTocStageError::EmptyToc(_)) => {
            arm.status = CaptureStatus::NotApplicable;
            arm.error = Some("empty_toc".into());
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let value: serde_json::Value = serde_json::from_str(&parsed.parsed_json)?;
    ensure!(
        value["pdf_hash"].as_str() == Some(source_hash),
        "TOC output PDF hash mismatch"
    );
    arm.article = Some(write_artifact(
        out,
        "toc/article.json",
        parsed.parsed_json.as_bytes(),
    )?);
    arm.evidence = Some(article_evidence(&value, backend_version)?);
    arm.status = CaptureStatus::Success;
    Ok(())
}

fn exporter_versions(out: &Path) -> Result<serde_json::Value> {
    serde_json::from_slice(
        &fs::read(out.join("toc-versions.json")).context("read TOC exporter versions")?,
    )
    .context("TOC exporter versions")
}

fn capture_toc(
    args: &Args,
    pdf: &Path,
    out: &Path,
    source_hash: &str,
    exporter: &mut ExporterIdentity,
) -> (ArmCapture, Option<u32>) {
    let started = Instant::now();
    let mut arm = new_arm(ParseArm::Toc);
    let mut bookmarks = None;
    let result = (|| -> Result<()> {
        let code = r#"
import importlib.util, json, pathlib, sys
spec = importlib.util.spec_from_file_location('toc_exporter', sys.argv[1])
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
m.OUT = pathlib.Path(sys.argv[4])
m.export(pathlib.Path(sys.argv[2]), sys.argv[3])
with pathlib.Path(sys.argv[5]).open('x', encoding='utf-8') as versions:
    json.dump({'python': sys.version, 'pymupdf': m.fitz.VersionBind}, versions)
"#;
        let export = Command::new(&args.python)
            .arg("-c")
            .arg(code)
            .arg(&exporter.script.path)
            .arg(pdf)
            .arg(&args.paper_id)
            .arg(out.join("toc-observations"))
            .arg(out.join("toc-versions.json"))
            .output()
            .context("start TOC exporter")?;
        ensure!(
            export.status.success(),
            "TOC exporter failed: {}",
            String::from_utf8_lossy(&export.stderr)
        );
        write_artifact(out, "toc-exporter.stdout", &export.stdout)?;
        write_artifact(out, "toc-exporter.stderr", &export.stderr)?;
        let versions = exporter_versions(out)?;
        let python_version = versions["python"]
            .as_str()
            .filter(|version| !version.trim().is_empty())
            .context("Python version absent")?;
        let pymupdf_version = versions["pymupdf"]
            .as_str()
            .filter(|version| !version.trim().is_empty())
            .context("PyMuPDF version absent")?;
        exporter.python_version = Some(python_version.into());
        exporter.pymupdf_version = Some(pymupdf_version.into());
        arm.backend_version = Some(pymupdf_version.into());

        let relative = format!("toc-observations/{}.json", args.paper_id);
        let bytes = fs::read(out.join(&relative)).context("read TOC observations")?;
        exporter.observations = Some(Artifact {
            path: relative,
            sha256: sha256(&bytes),
        });
        let observations: TocObservations = serde_json::from_slice(&bytes)?;
        ensure!(
            observations.pdf_sha256.as_deref() == Some(source_hash),
            "TOC exporter PDF hash does not match input"
        );
        ensure!(
            observations.total_pages > 0,
            "TOC exporter measured zero pages"
        );
        bookmarks = Some(u32::try_from(observations.toc.len())?);
        record_toc_article(
            &mut arm,
            &args.paper_id,
            std::str::from_utf8(&bytes)?,
            out,
            source_hash,
            pymupdf_version,
        )
    })();
    (finish_arm(arm, started, result), bookmarks)
}

async fn capture_grobid(pdf: &Path, paper_id: &str, out: &Path, source_hash: &str) -> ArmCapture {
    let started = Instant::now();
    let mut arm = new_arm(ParseArm::Grobid);
    let result = async {
        let parser =
            da_parse::GrobidParser::from_env().with_profile(da_parse::GrobidProfile::structural());
        let probe = parser
            .probe_version()
            .await
            .context("GROBID release probe")?;
        ensure!(!probe.version.trim().is_empty(), "GROBID release absent");
        arm.backend_version = Some(probe.version.clone());
        let parsed = parser
            .parse_pdf(pdf.to_str().context("PDF path is not UTF-8")?, paper_id)
            .await
            .context("GROBID structural parse")?;
        ensure!(
            parsed.pdf_hash == source_hash,
            "GROBID output PDF hash mismatch"
        );
        let bytes = serde_json::to_vec_pretty(&parsed)?;
        arm.article = Some(write_artifact(out, "grobid/article.json", &bytes)?);
        arm.evidence = Some(article_evidence(
            &serde_json::to_value(&parsed)?,
            &probe.version,
        )?);
        arm.status = CaptureStatus::Success;
        Ok::<_, anyhow::Error>(())
    }
    .await;
    finish_arm(arm, started, result)
}

fn capture_reference(pdf: &Path, out: &Path) -> ReferenceCapture {
    let mut reference = ReferenceCapture {
        tool: "pdftotext".into(),
        version: None,
        artifact: None,
        char_count: None,
        error: None,
    };
    let result = (|| -> Result<()> {
        let probe = Command::new("pdftotext")
            .arg("-v")
            .output()
            .context("pdftotext version")?;
        ensure!(probe.status.success(), "pdftotext version probe failed");
        let version = format!(
            "{}{}",
            String::from_utf8(probe.stdout)?,
            String::from_utf8(probe.stderr)?
        );
        ensure!(!version.trim().is_empty(), "pdftotext version absent");
        reference.version = Some(version.trim().into());
        let extraction = Command::new("pdftotext")
            .args(["-enc", "UTF-8"])
            .arg(pdf)
            .arg("-")
            .output()
            .context("pdftotext extraction")?;
        ensure!(
            extraction.status.success(),
            "pdftotext failed: {}",
            String::from_utf8_lossy(&extraction.stderr)
        );
        let text = std::str::from_utf8(&extraction.stdout)?;
        reference.char_count = Some(text.chars().count());
        reference.artifact = Some(write_artifact(
            out,
            "reference/pdftotext.txt",
            &extraction.stdout,
        )?);
        ensure!(!text.trim().is_empty(), "pdftotext reference is empty");
        Ok(())
    })();
    if let Err(error) = result {
        reference.error = Some(format!("{error:#}"));
    }
    reference
}

async fn measured_features(bytes: &[u8], bookmarks: Option<u32>) -> Result<ArticleFeatures> {
    let classification = da_adapters::PdfInspectorClassifier::new()
        .classify(bytes)
        .await?;
    ensure!(
        classification.page_count > 0,
        "classifier measured zero pages"
    );
    let mut features =
        da_application::parse_policy_observation::features_from_classification(&classification, "");
    features.toc_bookmarks = bookmarks.context("outline bookmark count was not measured")?;
    Ok(features)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let _logging_guard = da_cli::logging::init_pipeline_logging("policy_corpus_capture");
    tokio::runtime::Runtime::new()?.block_on(capture(args))
}

async fn capture(args: Args) -> Result<()> {
    ensure!(
        !args.paper_id.is_empty()
            && ![".", ".."].contains(&args.paper_id.as_str())
            && args
                .paper_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)),
        "paper-id must contain only ASCII letters, digits, dot, underscore or hyphen"
    );
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let pdf = args.pdf.canonicalize().context("source PDF")?;
    let bytes = fs::read(&pdf)?;
    let source = source_artifact(&pdf)?;
    ensure!(
        sha256(&bytes) == source.sha256,
        "PDF changed while reading input"
    );
    let mut exporter = ExporterIdentity {
        script: source_artifact(&repo.join("scripts/m451_pdf_toc_observations.py"))?,
        furniture_policy: source_artifact(&repo.join("data/ontology/furniture_detectors.yaml"))?,
        python: args.python.clone(),
        python_version: None,
        pymupdf_version: None,
        observations: None,
    };
    let parent = args
        .out
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let out = parent
        .canonicalize()
        .context("output parent must exist")?
        .join(
            args.out
                .file_name()
                .context("output directory name missing")?,
        );
    ensure!(
        !out.starts_with(repo.join("data"))
            && !out.starts_with(repo.join("artifacts/pipeline_cache")),
        "capture output must be outside production data and pipeline caches"
    );
    fs::create_dir(&out).context("capture output must be a new directory")?;

    let (toc, bookmarks) = capture_toc(&args, &pdf, &out, &source.sha256, &mut exporter);
    let grobid = capture_grobid(&pdf, &args.paper_id, &out, &source.sha256).await;
    let reference = capture_reference(&pdf, &out);
    let (features, features_error) = match measured_features(&bytes, bookmarks).await {
        Ok(features) => (Some(features), None),
        Err(error) => (None, Some(format!("{error:#}"))),
    };
    for artifact in [&source, &exporter.script, &exporter.furniture_policy] {
        ensure!(
            source_artifact(Path::new(&artifact.path))?.sha256 == artifact.sha256,
            "input changed during capture: {}",
            artifact.path
        );
    }
    let capture = Capture {
        schema_version: CAPTURE_SCHEMA.into(),
        paper_id: args.paper_id,
        source,
        source_bytes: bytes.len() as u64,
        features,
        features_error,
        exporter,
        reference,
        arms: [toc, grobid],
        candidate_only: true,
        deployed: false,
        import_eligible: false,
        graph_write: false,
    };
    write_artifact(&out, "capture.json", &serde_json::to_vec_pretty(&capture)?)?;
    println!("{}", out.join("capture.json").display());
    if capture.features.is_none()
        || capture.reference.error.is_some()
        || capture
            .arms
            .iter()
            .any(|arm| arm.status == CaptureStatus::Failed)
    {
        bail!("capture retained failures; inspect capture.json before evaluating");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exporter_metadata_is_not_decoded_from_native_diagnostic_output() {
        let out = tempfile::tempdir().unwrap();
        fs::write(
            out.path().join("toc-exporter.stdout"),
            "MuPDF error: invalid glyph\n",
        )
        .unwrap();
        fs::write(
            out.path().join("toc-versions.json"),
            br#"{"python":"3.12","pymupdf":"1.28"}"#,
        )
        .unwrap();
        let versions = exporter_versions(out.path()).unwrap();
        assert_eq!(versions["python"], "3.12");
        assert_eq!(versions["pymupdf"], "1.28");
        fs::write(out.path().join("toc-versions.json"), b"not JSON").unwrap();
        assert!(exporter_versions(out.path()).is_err());
    }

    #[test]
    fn only_typed_empty_toc_is_not_applicable() {
        let out = tempfile::tempdir().unwrap();
        let mut empty = new_arm(ParseArm::Toc);
        record_toc_article(
            &mut empty,
            "paper",
            r#"{"toc":[],"total_pages":1}"#,
            out.path(),
            "source-sha",
            "version",
        )
        .unwrap();
        assert_eq!(empty.status, CaptureStatus::NotApplicable);
        assert_eq!(empty.error.as_deref(), Some("empty_toc"));
        assert!(empty.article.is_none());
        assert!(empty.evidence.is_none());

        let mut malformed = new_arm(ParseArm::Toc);
        let started = Instant::now();
        let result = record_toc_article(
            &mut malformed,
            "paper",
            "not JSON",
            out.path(),
            "source-sha",
            "version",
        );
        let failed = finish_arm(malformed, started, result);
        assert_eq!(failed.status, CaptureStatus::Failed);
        assert!(failed.error.is_some());
    }

    #[test]
    fn toc_source_hash_mismatch_fails_before_writing_article() {
        let out = tempfile::tempdir().unwrap();
        let observations = serde_json::json!({
            "toc":[{"level":1,"title":"Chapter","page":1}],
            "pages_text":{"1":"body"},"total_pages":1,"pdf_sha256":"changed"
        });
        let mut arm = new_arm(ParseArm::Toc);
        assert!(
            record_toc_article(
                &mut arm,
                "paper",
                &observations.to_string(),
                out.path(),
                "original",
                "version"
            )
            .is_err()
        );
        assert!(!out.path().join("toc/article.json").exists());
    }

    #[test]
    fn artifacts_refuse_overwrite_and_hash_detects_tampering() {
        let out = tempfile::tempdir().unwrap();
        let artifact = write_artifact(out.path(), "reference/test.txt", b"original").unwrap();
        assert!(write_artifact(out.path(), &artifact.path, b"overwrite").is_err());
        fs::write(out.path().join(&artifact.path), b"changed").unwrap();
        assert_ne!(
            artifact.sha256,
            source_artifact(&out.path().join(artifact.path))
                .unwrap()
                .sha256
        );
    }
}
