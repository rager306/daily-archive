//! Governed pdffigures2 subprocess adapter (M371 S02).
//!
//! The adapter stages detector output inside the canonical article directory,
//! validates JSON and rendered assets, then atomically moves retained evidence
//! into `figures/`. Temporary `renderURL` values are never persisted as truth.

use async_trait::async_trait;
use da_domain::{BoundingBox72Dpi, DetectedVisualArtifact, VisualArtifactKind};
use da_ports::{
    FigureDetectionError, FigureDetectionPort, FigureDetectionPortResult, FigureDetectionRequest,
    FigureDetectionResult,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::process::Command;

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const DEFAULT_DPI: u32 = 150;

#[derive(Debug, Clone)]
pub struct PdfFigures2Adapter {
    jar_path: PathBuf,
    java_command: PathBuf,
    timeout: Duration,
    render_dpi: u32,
    detector_fingerprint: String,
}

impl PdfFigures2Adapter {
    pub fn new(jar_path: impl Into<PathBuf>) -> Self {
        Self {
            jar_path: jar_path.into(),
            java_command: PathBuf::from("java"),
            timeout: DEFAULT_TIMEOUT,
            render_dpi: DEFAULT_DPI,
            detector_fingerprint: "pdffigures2-0.0.12-SNAPSHOT".into(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_java_command(mut self, java_command: impl Into<PathBuf>) -> Self {
        self.java_command = java_command.into();
        self
    }

    pub fn from_env() -> FigureDetectionPortResult<Self> {
        let jar = std::env::var_os("PDFFIGURES2_JAR").ok_or_else(|| {
            FigureDetectionError::Unavailable(
                "PDFFIGURES2_JAR is not configured; point it to the assembly JAR".into(),
            )
        })?;
        Ok(Self::new(jar))
    }

    fn validate_request(&self, request: &FigureDetectionRequest) -> FigureDetectionPortResult<()> {
        if !self.jar_path.is_file() {
            return Err(FigureDetectionError::Unavailable(format!(
                "pdffigures2 JAR not found at {}",
                self.jar_path.display()
            )));
        }
        if !request.source_pdf_path.is_file() {
            return Err(FigureDetectionError::InvalidRequest(format!(
                "source PDF not found at {}",
                request.source_pdf_path.display()
            )));
        }
        if request.article_id.trim().is_empty()
            || request.article_id.contains('/')
            || request.article_id.contains('\\')
            || request.article_id == ".."
        {
            return Err(FigureDetectionError::InvalidRequest(
                "article_id must be a safe path component".into(),
            ));
        }
        validate_sha256(&request.source_pdf_sha256)
            .map_err(FigureDetectionError::InvalidRequest)?;
        da_ports::validate_revision_vid(request.revision_vid.as_deref())
            .map_err(FigureDetectionError::InvalidRequest)?;
        if self.render_dpi == 0 {
            return Err(FigureDetectionError::InvalidRequest(
                "render DPI must be greater than zero".into(),
            ));
        }
        Ok(())
    }

    fn staging_dir(&self, article_dir: &Path) -> PathBuf {
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        article_dir.join(format!(
            ".pdffigures2-stage-{}-{sequence}",
            std::process::id()
        ))
    }

    fn parse_and_retain(
        &self,
        request: &FigureDetectionRequest,
        staging: &Path,
        raw_metadata_path: &Path,
    ) -> FigureDetectionPortResult<(Vec<DetectedVisualArtifact>, PathBuf, String, String)> {
        let raw_metadata =
            fs::read(raw_metadata_path).map_err(|error| io(raw_metadata_path, error))?;
        let figures: Vec<RawSavedFigure> =
            serde_json::from_slice(&raw_metadata).map_err(|error| {
                FigureDetectionError::InvalidOutput {
                    path: raw_metadata_path.display().to_string(),
                    message: error.to_string(),
                }
            })?;
        if figures.is_empty() {
            return Err(FigureDetectionError::InvalidOutput {
                path: raw_metadata_path.display().to_string(),
                message: "detector returned zero captioned visual artifacts".into(),
            });
        }

        // Hash semantic detector metadata with renderURL reduced to its stable
        // basename. Absolute staging paths change every run and must not poison
        // provenance or downstream cache keys.
        let metadata_sha256 = canonical_detector_metadata_sha256(&figures)?;

        // Validate the complete detector batch before publishing any commit marker.
        let mut totals = HashMap::new();
        for raw in &figures {
            let base = format!("{}{}", kind_name(raw.kind()?), raw.name.trim());
            *totals.entry(base).or_insert(0usize) += 1;
        }
        let mut seen = HashMap::new();
        let mut prepared = Vec::with_capacity(figures.len());
        for raw in figures {
            let kind = raw.kind()?;
            let base_id = format!("{}{}", kind_name(kind), raw.name.trim());
            let ordinal = seen.entry(base_id.clone()).or_insert(0usize);
            *ordinal += 1;
            let artifact_id = if totals[&base_id] == 1 {
                base_id
            } else {
                format!("{base_id}-{}", *ordinal)
            };
            let source_image = validate_staged_image(staging, &raw.render_url)?;
            let asset_bytes = fs::read(&source_image).map_err(|error| io(&source_image, error))?;
            if asset_bytes.is_empty() {
                return Err(invalid_output(&source_image, "rendered image is empty"));
            }
            let asset_sha256 = sha256(&asset_bytes);
            let original_stem = source_image
                .file_stem()
                .and_then(OsStr::to_str)
                .ok_or_else(|| invalid_output(&source_image, "image filename is not UTF-8"))?;
            let filename = format!("{original_stem}-{}.png", &asset_sha256[..12]);
            let relative_asset = Path::new("figures").join(&filename);
            let artifact = DetectedVisualArtifact {
                artifact_id,
                kind,
                detector_name: "pdffigures2".into(),
                detector_fingerprint: self.detector_fingerprint.clone(),
                source_pdf_sha256: request.source_pdf_sha256.clone(),
                page_index: raw.page,
                region: raw.region_boundary.into(),
                caption_region: raw.caption_boundary.into(),
                caption: raw.caption,
                image_text: raw.image_text,
                render_dpi: raw.render_dpi,
                asset_path: relative_asset.to_string_lossy().into_owned(),
                asset_sha256,
                metadata_sha256: metadata_sha256.clone(),
            };
            artifact
                .validate()
                .map_err(|error| FigureDetectionError::InvalidOutput {
                    path: raw_metadata_path.display().to_string(),
                    message: error.to_string(),
                })?;
            prepared.push((artifact, filename, asset_bytes));
        }

        let figures_dir = request.canonical_article_dir.join("figures");
        fs::create_dir_all(&figures_dir).map_err(|error| io(&figures_dir, error))?;
        for (_, filename, bytes) in &prepared {
            write_atomic(&figures_dir.join(filename), bytes)?;
        }
        let mut artifacts: Vec<_> = prepared
            .into_iter()
            .map(|(artifact, _, _)| artifact)
            .collect();
        artifacts.sort_by(|left, right| {
            left.page_index
                .cmp(&right.page_index)
                .then(left.artifact_id.cmp(&right.artifact_id))
        });

        // The canonical manifest contains only canonical paths. Writing it last
        // makes it the commit marker for the complete artifact batch.
        let canonical_metadata = figures_dir.join("pdffigures2.manifest.json");
        let manifest = CanonicalFigureManifest {
            detector_fingerprint: self.detector_fingerprint.clone(),
            source_pdf_sha256: request.source_pdf_sha256.clone(),
            detector_metadata_sha256: metadata_sha256.clone(),
            revision_vid: request.revision_vid.clone(),
            artifacts: artifacts.clone(),
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| {
            FigureDetectionError::InvalidOutput {
                path: canonical_metadata.display().to_string(),
                message: error.to_string(),
            }
        })?;
        write_atomic(&canonical_metadata, &manifest_bytes)?;
        let manifest_sha256 = sha256(&manifest_bytes);
        Ok((
            artifacts,
            canonical_metadata,
            metadata_sha256,
            manifest_sha256,
        ))
    }
}

#[async_trait]
impl FigureDetectionPort for PdfFigures2Adapter {
    fn detector_fingerprint(&self) -> &str {
        &self.detector_fingerprint
    }

    async fn detect(
        &self,
        request: FigureDetectionRequest,
    ) -> FigureDetectionPortResult<FigureDetectionResult> {
        self.validate_request(&request)?;
        fs::create_dir_all(&request.canonical_article_dir)
            .map_err(|error| io(&request.canonical_article_dir, error))?;
        let staging = self.staging_dir(&request.canonical_article_dir);
        fs::create_dir(&staging).map_err(|error| io(&staging, error))?;
        let cleanup = StagingCleanup(staging.clone());
        let stem = request
            .source_pdf_path
            .file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| {
                FigureDetectionError::InvalidRequest("PDF filename is not UTF-8".into())
            })?;
        let data_prefix = staging.join("data_");
        let image_prefix = staging.join("fig_");
        let raw_metadata_path = staging.join(format!("data_{stem}.json"));

        let started = Instant::now();
        let mut command = Command::new(&self.java_command);
        command
            .kill_on_drop(true)
            .arg("-Dsun.java.awt.headless=true")
            .arg("-jar")
            .arg(&self.jar_path)
            .arg(&request.source_pdf_path)
            .arg("-i")
            .arg(self.render_dpi.to_string())
            .arg("-d")
            .arg(&data_prefix)
            .arg("-m")
            .arg(&image_prefix)
            .arg("-f")
            .arg("png")
            .arg("-q");
        let output = tokio::time::timeout(self.timeout, command.output())
            .await
            .map_err(|_| FigureDetectionError::Timeout {
                timeout_ms: self.timeout.as_millis() as u64,
            })?
            .map_err(|error| {
                FigureDetectionError::Unavailable(format!(
                    "cannot start {}: {error}",
                    self.java_command.display()
                ))
            })?;
        if !output.status.success() {
            return Err(FigureDetectionError::Subprocess {
                status: output.status.to_string(),
                diagnostic: bounded_diagnostic(&output.stderr, &output.stdout),
            });
        }

        let (artifacts, metadata_path, detector_metadata_sha256, manifest_sha256) =
            self.parse_and_retain(&request, &staging, &raw_metadata_path)?;
        let duration_ms = started.elapsed().as_millis() as u64;
        tracing::info!(
            article_id = %request.article_id,
            detector = %self.detector_fingerprint,
            artifact_count = artifacts.len(),
            revision_vid = ?request.revision_vid,
            duration_ms,
            event = "pdffigures2_detection_complete"
        );
        drop(cleanup);
        Ok(FigureDetectionResult {
            detector_fingerprint: self.detector_fingerprint.clone(),
            metadata_path,
            detector_metadata_sha256,
            manifest_sha256,
            artifacts,
            duration_ms,
            revision_vid: request.revision_vid,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CanonicalFigureManifest {
    detector_fingerprint: String,
    source_pdf_sha256: String,
    detector_metadata_sha256: String,
    /// Revision identity bound at detection time (M400 S01). Absent (serde
    /// default) for manifests written before revision binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revision_vid: Option<String>,
    artifacts: Vec<DetectedVisualArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSavedFigure {
    render_dpi: u32,
    name: String,
    page: u32,
    #[serde(rename = "figType")]
    fig_type: String,
    region_boundary: RawBox,
    caption: String,
    #[serde(default)]
    image_text: Vec<String>,
    #[serde(rename = "renderURL")]
    render_url: String,
    caption_boundary: RawBox,
}

impl RawSavedFigure {
    fn kind(&self) -> FigureDetectionPortResult<VisualArtifactKind> {
        match self.fig_type.as_str() {
            "Figure" => Ok(VisualArtifactKind::Figure),
            "Table" => Ok(VisualArtifactKind::Table),
            other => Err(FigureDetectionError::InvalidOutput {
                path: "pdffigures2 JSON".into(),
                message: format!("unknown figType {other:?}"),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct RawBox {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

impl From<RawBox> for BoundingBox72Dpi {
    fn from(value: RawBox) -> Self {
        Self {
            x1: value.x1,
            y1: value.y1,
            x2: value.x2,
            y2: value.y2,
        }
    }
}

struct StagingCleanup(PathBuf);

impl Drop for StagingCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn canonical_detector_metadata_sha256(
    figures: &[RawSavedFigure],
) -> FigureDetectionPortResult<String> {
    let mut canonical = figures.to_vec();
    for figure in &mut canonical {
        let filename = Path::new(&figure.render_url)
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| {
                invalid_output(
                    Path::new(&figure.render_url),
                    "renderURL filename is not UTF-8",
                )
            })?;
        figure.render_url = filename.to_owned();
    }
    let bytes =
        serde_json::to_vec(&canonical).map_err(|error| FigureDetectionError::InvalidOutput {
            path: "pdffigures2 semantic metadata".into(),
            message: error.to_string(),
        })?;
    Ok(sha256(&bytes))
}

fn kind_name(kind: VisualArtifactKind) -> &'static str {
    match kind {
        VisualArtifactKind::Figure => "Figure",
        VisualArtifactKind::Table => "Table",
    }
}

fn validate_staged_image(staging: &Path, render_url: &str) -> FigureDetectionPortResult<PathBuf> {
    let path = Path::new(render_url);
    let filename = path
        .file_name()
        .ok_or_else(|| invalid_output(path, "renderURL has no filename"))?;
    let staged = staging.join(filename);
    if !staged.is_file() {
        return Err(invalid_output(
            &staged,
            "renderURL does not resolve to a staged image",
        ));
    }
    Ok(staged)
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> FigureDetectionPortResult<()> {
    let parent = destination.parent().ok_or_else(|| {
        FigureDetectionError::InvalidRequest(format!(
            "destination has no parent: {}",
            destination.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| io(parent, error))?;
    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let tmp = destination.with_extension(format!("tmp-{}-{sequence}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|error| io(&tmp, error))?;
    file.write_all(bytes).map_err(|error| io(&tmp, error))?;
    file.sync_all().map_err(|error| io(&tmp, error))?;
    fs::rename(&tmp, destination).map_err(|error| io(destination, error))
}

fn validate_sha256(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("source_pdf_sha256 must be a 64-character hex digest".into());
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn io(path: &Path, error: impl std::fmt::Display) -> FigureDetectionError {
    FigureDetectionError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    }
}

fn invalid_output(path: &Path, message: impl Into<String>) -> FigureDetectionError {
    FigureDetectionError::InvalidOutput {
        path: path.display().to_string(),
        message: message.into(),
    }
}

fn bounded_diagnostic(stderr: &[u8], stdout: &[u8]) -> String {
    const LIMIT: usize = 4096;
    let bytes = if stderr.is_empty() { stdout } else { stderr };
    let start = bytes.len().saturating_sub(LIMIT);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("da-pdffigures2-{name}-{nonce}"))
    }

    #[cfg(unix)]
    fn fake_java(root: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = root.join("fake-java.sh");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn valid_request(root: &Path) -> FigureDetectionRequest {
        let pdf = root.join("paper.pdf");
        fs::write(&pdf, b"fake-pdf").unwrap();
        FigureDetectionRequest {
            article_id: "paper".into(),
            source_pdf_path: pdf,
            source_pdf_sha256: sha256(b"fake-pdf"),
            canonical_article_dir: root.join("article"),
            revision_vid: None,
        }
    }

    #[test]
    fn semantic_metadata_hash_ignores_staging_directory() {
        let make = |render_url: &str| RawSavedFigure {
            render_dpi: 150,
            name: "3".into(),
            page: 4,
            fig_type: "Figure".into(),
            region_boundary: RawBox {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
            },
            caption: "Figure 3: GEPA".into(),
            image_text: vec!["GEPA".into()],
            render_url: render_url.into(),
            caption_boundary: RawBox {
                x1: 1.0,
                y1: 5.0,
                x2: 3.0,
                y2: 6.0,
            },
        };
        let first =
            canonical_detector_metadata_sha256(&[make("/tmp/stage-1/fig-paper-Figure3-1.png")])
                .unwrap();
        let second = canonical_detector_metadata_sha256(&[make(
            "/catalog/.stage-99/fig-paper-Figure3-1.png",
        )])
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn bounded_diagnostics_prevent_unbounded_subprocess_errors() {
        let diagnostic = bounded_diagnostic(&vec![b'x'; 5000], b"");
        assert_eq!(diagnostic.len(), 4096);
    }

    #[test]
    fn missing_jar_is_an_explicit_unavailable_error() {
        let adapter = PdfFigures2Adapter::new("/definitely/missing/pdffigures2.jar");
        let request = FigureDetectionRequest {
            article_id: "2507.19457".into(),
            source_pdf_path: "/missing.pdf".into(),
            source_pdf_sha256: "a".repeat(64),
            canonical_article_dir: temp_dir("missing"),
            revision_vid: None,
        };
        assert!(matches!(
            adapter.validate_request(&request),
            Err(FigureDetectionError::Unavailable(_))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn nonzero_subprocess_surfaces_bounded_diagnostic() {
        let root = temp_dir("nonzero");
        fs::create_dir_all(&root).unwrap();
        let jar = root.join("detector.jar");
        fs::write(&jar, b"jar").unwrap();
        let java = fake_java(&root, "printf 'detector exploded' >&2; exit 9");
        let request = valid_request(&root);
        let error = PdfFigures2Adapter::new(jar)
            .with_java_command(java)
            .detect(request)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            FigureDetectionError::Subprocess { ref diagnostic, .. }
                if diagnostic.contains("detector exploded")
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_kills_process_and_cleans_staging() {
        let root = temp_dir("timeout");
        fs::create_dir_all(&root).unwrap();
        let jar = root.join("detector.jar");
        fs::write(&jar, b"jar").unwrap();
        let java = fake_java(&root, "sleep 2");
        let request = valid_request(&root);
        let article_dir = request.canonical_article_dir.clone();
        let error = PdfFigures2Adapter::new(jar)
            .with_java_command(java)
            .with_timeout(Duration::from_millis(20))
            .detect(request)
            .await
            .unwrap_err();
        assert!(matches!(error, FigureDetectionError::Timeout { .. }));
        assert!(fs::read_dir(&article_dir).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".pdffigures2-stage-")
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fixture_mapping_publishes_only_canonical_paths_and_unique_ids() {
        let root = temp_dir("fixture");
        let staging = root.join("stage");
        fs::create_dir_all(&staging).unwrap();
        let image_one = staging.join("fig-paper-Figure3-1.png");
        let image_two = staging.join("fig-paper-Figure3-2.png");
        fs::write(&image_one, b"png-one").unwrap();
        fs::write(&image_two, b"png-two").unwrap();
        let raw_path = staging.join("data_paper.json");
        let raw = serde_json::json!([
            {
                "renderDpi": 150,
                "name": "3",
                "page": 4,
                "figType": "Figure",
                "regionBoundary": {"x1": 10.0, "y1": 20.0, "x2": 100.0, "y2": 120.0},
                "caption": "Figure 3: GEPA flowchart",
                "imageText": ["GEPA"],
                "renderURL": image_one,
                "captionBoundary": {"x1": 10.0, "y1": 125.0, "x2": 100.0, "y2": 140.0}
            },
            {
                "renderDpi": 150,
                "name": "3",
                "page": 5,
                "figType": "Figure",
                "regionBoundary": {"x1": 11.0, "y1": 21.0, "x2": 101.0, "y2": 121.0},
                "caption": "Figure 3: duplicate-name fixture",
                "imageText": [],
                "renderURL": image_two,
                "captionBoundary": {"x1": 11.0, "y1": 126.0, "x2": 101.0, "y2": 141.0}
            }
        ]);
        fs::write(&raw_path, serde_json::to_vec(&raw).unwrap()).unwrap();
        let article_dir = root.join("article");
        let request = FigureDetectionRequest {
            article_id: "paper".into(),
            source_pdf_path: root.join("paper.pdf"),
            source_pdf_sha256: "a".repeat(64),
            canonical_article_dir: article_dir.clone(),
            revision_vid: None,
        };
        let adapter = PdfFigures2Adapter::new("unused-in-parser-test.jar");
        let (artifacts, manifest_path, _, manifest_sha256) = adapter
            .parse_and_retain(&request, &staging, &raw_path)
            .unwrap();

        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].artifact_id, "Figure3-1");
        assert_eq!(artifacts[1].artifact_id, "Figure3-2");
        assert!(artifacts.iter().all(|artifact| {
            artifact.asset_path.starts_with("figures/")
                && !artifact.asset_path.contains("/tmp/")
                && article_dir.join(&artifact.asset_path).is_file()
        }));
        let manifest_bytes = fs::read(&manifest_path).unwrap();
        assert_eq!(sha256(&manifest_bytes), manifest_sha256);
        let text = String::from_utf8(manifest_bytes).unwrap();
        assert!(!text.contains("renderURL"));
        assert!(!text.contains("/tmp/"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn revision_bound_request_echoes_into_manifest_and_rejects_malformed() {
        let root = temp_dir("revision");
        let staging = root.join("stage");
        fs::create_dir_all(&staging).unwrap();
        let image = staging.join("fig-paper-Figure3-1.png");
        fs::write(&image, b"png-revision").unwrap();
        let raw_path = staging.join("data_paper.json");
        let raw = serde_json::json!([
            {
                "renderDpi": 150,
                "name": "3",
                "page": 4,
                "figType": "Figure",
                "regionBoundary": {"x1": 10.0, "y1": 20.0, "x2": 100.0, "y2": 120.0},
                "caption": "Figure 3: GEPA flowchart",
                "imageText": ["GEPA"],
                "renderURL": image,
                "captionBoundary": {"x1": 10.0, "y1": 125.0, "x2": 100.0, "y2": 140.0}
            }
        ]);
        fs::write(&raw_path, serde_json::to_vec(&raw).unwrap()).unwrap();
        let article_dir = root.join("article");
        let revision = da_domain::vid::paper_revision_vid("arxiv", "2507.19457", 1).unwrap();
        let jar = root.join("detector.jar");
        fs::write(&jar, b"jar").unwrap();
        let pdf = root.join("paper.pdf");
        fs::write(&pdf, b"fake-pdf").unwrap();
        let request = FigureDetectionRequest {
            article_id: "2507.19457".into(),
            source_pdf_path: pdf,
            source_pdf_sha256: "a".repeat(64),
            canonical_article_dir: article_dir.clone(),
            revision_vid: Some(revision.clone()),
        };
        let adapter = PdfFigures2Adapter::new(jar);
        // Malformed revision VIDs are rejected before any subprocess runs.
        let mut malformed = request.clone();
        malformed.revision_vid = Some("not-a-vid".into());
        assert!(matches!(
            adapter.validate_request(&malformed),
            Err(FigureDetectionError::InvalidRequest(_))
        ));
        // The canonical manifest persists the revision identity.
        let (_, manifest_path, _, _) = adapter
            .parse_and_retain(&request, &staging, &raw_path)
            .unwrap();
        let manifest: CanonicalFigureManifest =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest.revision_vid.as_deref(), Some(revision.as_str()));
        // Legacy manifests without the field still deserialize.
        let legacy = serde_json::json!({
            "detector_fingerprint": "pdffigures2-0.0.12-SNAPSHOT",
            "source_pdf_sha256": "a".repeat(64),
            "detector_metadata_sha256": "c".repeat(64),
            "artifacts": []
        });
        let decoded: CanonicalFigureManifest = serde_json::from_value(legacy).unwrap();
        assert!(decoded.revision_vid.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn real_gepa_pdf_yields_figures_tables_and_grounded_figure3() {
        let jar = PathBuf::from(
            "/root/vendor-source/scipdf_parser/scipdf/pdf/pdffigures2/pdffigures2-assembly-0.0.12-SNAPSHOT.jar",
        );
        let pdf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/2507.19457.pdf",
        );
        assert!(
            jar.is_file(),
            "real integration JAR missing: {}",
            jar.display()
        );
        assert!(pdf.is_file(), "real GEPA PDF missing: {}", pdf.display());
        let pdf_bytes = fs::read(&pdf).unwrap();
        let article_dir = temp_dir("gepa");
        let adapter = PdfFigures2Adapter::new(jar);
        let revision = da_domain::vid::paper_revision_vid("arxiv", "2507.19457", 1).unwrap();
        let result = adapter
            .detect(FigureDetectionRequest {
                article_id: "2507.19457".into(),
                source_pdf_path: pdf,
                source_pdf_sha256: sha256(&pdf_bytes),
                canonical_article_dir: article_dir.clone(),
                revision_vid: Some(revision.clone()),
            })
            .await
            .unwrap();

        assert_eq!(
            result.revision_vid.as_deref(),
            Some(revision.as_str()),
            "revision identity echoes into the detection result"
        );

        assert_eq!(result.artifacts.len(), 25);
        assert_eq!(
            result
                .artifacts
                .iter()
                .filter(|artifact| artifact.kind == VisualArtifactKind::Figure)
                .count(),
            22
        );
        assert_eq!(
            result
                .artifacts
                .iter()
                .filter(|artifact| artifact.kind == VisualArtifactKind::Table)
                .count(),
            3
        );
        let figure3 = result
            .artifacts
            .iter()
            .find(|artifact| artifact.artifact_id == "Figure3")
            .expect("Figure 3 detected");
        assert!(
            figure3.page_index < 10,
            "Figure 3 should remain in the opening paper pages"
        );
        assert!(figure3.caption.contains("GEPA"));
        assert!(figure3.region.x1 < figure3.region.x2);
        assert_eq!(figure3.asset_sha256.len(), 64);
        assert!(article_dir.join(&figure3.asset_path).is_file());
        assert!(result.metadata_path.is_file());
        assert_eq!(
            sha256(&fs::read(&result.metadata_path).unwrap()),
            result.manifest_sha256
        );
        assert!(
            result
                .artifacts
                .iter()
                .all(|artifact| artifact.metadata_sha256 == result.detector_metadata_sha256)
        );
        let canonical_manifest: CanonicalFigureManifest =
            serde_json::from_slice(&fs::read(&result.metadata_path).unwrap()).unwrap();
        assert_eq!(canonical_manifest.artifacts.len(), 25);
        assert_eq!(
            canonical_manifest.revision_vid.as_deref(),
            Some(revision.as_str()),
            "revision identity persists in the canonical manifest"
        );
        let manifest_text = fs::read_to_string(&result.metadata_path).unwrap();
        assert!(!manifest_text.contains("/tmp/"));
        assert!(
            canonical_manifest
                .artifacts
                .iter()
                .all(|artifact| article_dir.join(&artifact.asset_path).is_file())
        );
        fs::remove_dir_all(article_dir).unwrap();
    }
}
