//! LocalPath acquisition — operator local drop provider (ADR-056 §2.3 W9).
//!
//! `local` is a registered `source_code` (data/source_codes.yaml, reliability
//! tier 3): the operator hands over a filesystem path and the provider reads
//! (never copies) the primary bytes — a single file, or a directory whose
//! `manifest.json` (local-manifest.v1) declares the primary plus optional
//! sidecars with independent checksums.
//!
//! Fail-closed rules (ADR-056 §2.4 A.4, §2.9):
//! - listed/pointed primary missing → `MissingAsset` (hard fail, no stages);
//! - manifest unparseable / wrong schema_version / missing primary →
//!   `InvalidManifest` (hard fail);
//! - declared sha256 mismatch → `ChecksumConflict` (hard fail, never silent
//!   overwrite or silent accept);
//! - optional sidecar file missing → typed partial (disposition `Fail` on the
//!   sidecar entry), primary still ensures (§2.9 row "PDF ok, HTML fail").
//!
//! Nothing is written: no catalog materialization, no graph mutation. Second
//! ensure over identical bytes reports `Unchanged` again (idempotent by
//! construction). No network is ever used, so `allow_network=false` (CI/
//! replay) is honored trivially.

use da_domain::{AcquisitionPolicy, ArticleLocator, AssetDisposition, PrimaryContentKind};
use da_ports::{
    AcquisitionError, EnsuredAsset, EnsuredAssets, EnsuredPrimary, ResolvedArticleRef,
    SourceAcquisitionPort,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER: &str = "local";
/// DATA-bound manifest schema id; anything else fails closed.
pub const LOCAL_MANIFEST_SCHEMA: &str = "local-manifest.v1";
const MANIFEST_FILE: &str = "manifest.json";

/// Supported primary kinds by extension (ADR-056 §2.4 C.11: local primary is
/// selected by extension / declared MIME — never "first file we found").
fn primary_kind_for(path: &Path) -> Option<PrimaryContentKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => Some(PrimaryContentKind::Pdf),
        "html" | "htm" => Some(PrimaryContentKind::Html),
        "md" | "markdown" => Some(PrimaryContentKind::Markdown),
        _ => None,
    }
}

fn kind_name(kind: PrimaryContentKind) -> &'static str {
    match kind {
        PrimaryContentKind::Pdf => "pdf",
        PrimaryContentKind::Html => "html",
        PrimaryContentKind::Markdown => "markdown",
        PrimaryContentKind::Latex => "latex",
        PrimaryContentKind::Unknown => "unknown",
    }
}

/// One declared sidecar representation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LocalManifestSidecar {
    pub kind: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Declared primary content.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LocalManifestPrimary {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// The local drop manifest (`local-manifest.v1`). One directory = one load
/// unit: a primary plus optional sidecars. Multi-article drops are N units
/// and therefore N locators (ADR-056 §2.5.1 C.6: bulk = N× single load).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LocalManifest {
    pub schema_version: String,
    pub primary: LocalManifestPrimary,
    #[serde(default)]
    pub sidecars: Vec<LocalManifestSidecar>,
}

/// Filesystem-backed operator local drop provider.
#[derive(Debug, Clone)]
pub struct LocalPathAcquisition {
    /// Root used to make reported paths repo-relative when the drop lives
    /// under it (readability only — resolve/ensure work with any path).
    pub report_root: PathBuf,
}

impl Default for LocalPathAcquisition {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalPathAcquisition {
    pub fn new() -> Self {
        Self {
            report_root: PathBuf::from("."),
        }
    }

    pub fn with_report_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.report_root = root.into();
        self
    }

    fn path_for_report(&self, file: &Path) -> String {
        file.strip_prefix(&self.report_root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| file.to_string_lossy().into_owned())
    }

    /// Resolve a manifest-declared path: absolute wins, otherwise relative to
    /// the manifest's directory. `..` components are kept (operator paths are
    /// explicit inputs, not untrusted fetches).
    fn resolve_declared(manifest_dir: &Path, declared: &str) -> PathBuf {
        let p = PathBuf::from(declared);
        if p.is_absolute() {
            p
        } else {
            manifest_dir.join(p)
        }
    }

    fn hash_file(path: &Path) -> Result<(String, u64), AcquisitionError> {
        let data = fs::read(path).map_err(|e| {
            AcquisitionError::Infrastructure(format!("read {}: {e}", path.display()))
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok((hex::encode(hasher.finalize()), data.len() as u64))
    }

    /// Single-file ensure: presence + hash + primary kind by extension.
    fn ensure_single_file(&self, path: &Path) -> Result<EnsuredAssets, AcquisitionError> {
        let kind = primary_kind_for(path).ok_or_else(|| AcquisitionError::CannotResolve {
            provider: PROVIDER.into(),
            detail: format!(
                "unsupported local primary extension for {} (pdf|html|htm|md)",
                path.display()
            ),
        })?;
        if !path.is_file() {
            return Err(AcquisitionError::MissingAsset(format!(
                "local primary {} missing (fail-closed, no network fallback)",
                path.display()
            )));
        }
        let (sha, bytes) = Self::hash_file(path)?;
        let report_path = self.path_for_report(path);
        Ok(EnsuredAssets {
            primary: EnsuredPrimary {
                kind,
                path: report_path.clone(),
                sha256: Some(sha.clone()),
                bytes: Some(bytes),
            },
            assets: vec![EnsuredAsset {
                kind: kind_name(kind).into(),
                path: report_path,
                sha256: Some(sha),
                bytes: Some(bytes),
                // Read, not fetched: the bytes were already on this machine.
                disposition: AssetDisposition::Unchanged,
            }],
            // No transfer happened; ensure hit without any fetch (ADR-056
            // §2.4 D.16: offline paths must succeed with network denied).
            catalog_hit: true,
        })
    }

    /// Directory ensure: strict `manifest.json` read + per-asset verification.
    fn ensure_manifest_dir(&self, dir: &Path) -> Result<EnsuredAssets, AcquisitionError> {
        let manifest_path = dir.join(MANIFEST_FILE);
        if !manifest_path.is_file() {
            return Err(AcquisitionError::MissingAsset(format!(
                "{MANIFEST_FILE} not found under {}; directory locators require a {LOCAL_MANIFEST_SCHEMA} manifest",
                dir.display()
            )));
        }
        let raw = fs::read_to_string(&manifest_path).map_err(|e| {
            AcquisitionError::InvalidManifest(format!("read {}: {e}", manifest_path.display()))
        })?;
        let manifest: LocalManifest = serde_json::from_str(&raw).map_err(|e| {
            AcquisitionError::InvalidManifest(format!("parse {}: {e}", manifest_path.display()))
        })?;
        if manifest.schema_version != LOCAL_MANIFEST_SCHEMA {
            return Err(AcquisitionError::InvalidManifest(format!(
                "{} declares schema_version={:?}, expected {LOCAL_MANIFEST_SCHEMA}",
                manifest_path.display(),
                manifest.schema_version
            )));
        }

        let primary_path = Self::resolve_declared(dir, &manifest.primary.path);
        let primary_kind = primary_kind_for(&primary_path).ok_or_else(|| {
            AcquisitionError::InvalidManifest(format!(
                "primary {:?} has unsupported extension (pdf|html|htm|md)",
                manifest.primary.path
            ))
        })?;
        if !primary_path.is_file() {
            return Err(AcquisitionError::MissingAsset(format!(
                "manifest primary {} missing under {} (fail-closed)",
                manifest.primary.path,
                dir.display()
            )));
        }
        let (primary_sha, primary_bytes) = Self::hash_file(&primary_path)?;
        if let Some(declared) = &manifest.primary.sha256
            && declared.trim().to_ascii_lowercase() != primary_sha
        {
            // §2.4 A.4: conflicting checksum at the same id is a hard fail —
            // never a silent overwrite, never a silent accept.
            return Err(AcquisitionError::ChecksumConflict(format!(
                "manifest primary {}: declared sha256={declared}, actual={primary_sha}",
                manifest.primary.path
            )));
        }
        let primary_report_path = self.path_for_report(&primary_path);
        let mut assets = vec![EnsuredAsset {
            kind: kind_name(primary_kind).into(),
            path: primary_report_path.clone(),
            sha256: Some(primary_sha.clone()),
            bytes: Some(primary_bytes),
            disposition: AssetDisposition::Unchanged,
        }];

        for sidecar in &manifest.sidecars {
            let sidecar_path = Self::resolve_declared(dir, &sidecar.path);
            if !sidecar_path.is_file() {
                // §2.9: primary ok + sidecar fail → ensure partial, typed
                // finding rides on the asset disposition (never silent).
                assets.push(EnsuredAsset {
                    kind: sidecar.kind.clone(),
                    path: self.path_for_report(&sidecar_path),
                    sha256: None,
                    bytes: None,
                    disposition: AssetDisposition::Fail,
                });
                continue;
            }
            let (sha, bytes) = Self::hash_file(&sidecar_path)?;
            if let Some(declared) = &sidecar.sha256
                && declared.trim().to_ascii_lowercase() != sha
            {
                return Err(AcquisitionError::ChecksumConflict(format!(
                    "manifest sidecar {:?} ({}): declared sha256={declared}, actual={sha}",
                    sidecar.kind, sidecar.path
                )));
            }
            assets.push(EnsuredAsset {
                kind: sidecar.kind.clone(),
                path: self.path_for_report(&sidecar_path),
                sha256: Some(sha),
                bytes: Some(bytes),
                disposition: AssetDisposition::Unchanged,
            });
        }

        Ok(EnsuredAssets {
            primary: EnsuredPrimary {
                kind: primary_kind,
                path: primary_report_path,
                sha256: Some(primary_sha),
                bytes: Some(primary_bytes),
            },
            assets,
            catalog_hit: true,
        })
    }
}

impl SourceAcquisitionPort for LocalPathAcquisition {
    fn provider_code(&self) -> &str {
        PROVIDER
    }

    fn can_resolve(&self, locator: &ArticleLocator) -> bool {
        match locator {
            ArticleLocator::Local { path } => {
                let p = Path::new(path);
                if p.is_dir() {
                    return true;
                }
                primary_kind_for(p).is_some()
            }
            _ => false,
        }
    }

    fn resolve(&self, locator: &ArticleLocator) -> Result<ResolvedArticleRef, AcquisitionError> {
        let ArticleLocator::Local { path } = locator else {
            return Err(AcquisitionError::CannotResolve {
                provider: PROVIDER.into(),
                detail: format!("unsupported locator {}", locator.kind_name()),
            });
        };
        let p = Path::new(path);
        if !self.can_resolve(locator) {
            return Err(AcquisitionError::CannotResolve {
                provider: PROVIDER.into(),
                detail: format!(
                    "local locator {path} is neither a directory with {MANIFEST_FILE} nor a supported primary file"
                ),
            });
        }
        // Unit key: file stem for files, directory name for drops. The path
        // itself stays the raw locator for audit (ADR-056 §2.4 A.2).
        let unit_key = if p.is_dir() {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_string()
        } else {
            p.file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_string()
        };
        let source_profile = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .filter(|e| e == "pdf")
            .map(|_| "scientific_paper".to_string());
        Ok(ResolvedArticleRef {
            source_code: PROVIDER.into(),
            unit_key,
            catalog_path: None,
            locator: locator.clone(),
            raw_locator: path.clone(),
            source_profile,
            container_ref: None,
        })
    }

    fn ensure_assets(
        &self,
        resolved: &ResolvedArticleRef,
        _policy: &AcquisitionPolicy,
    ) -> Result<EnsuredAssets, AcquisitionError> {
        let ArticleLocator::Local { path } = &resolved.locator else {
            return Err(AcquisitionError::CannotResolve {
                provider: PROVIDER.into(),
                detail: format!("unsupported locator {}", resolved.locator.kind_name()),
            });
        };
        let p = Path::new(path);
        if p.is_dir() {
            self.ensure_manifest_dir(p)
        } else {
            self.ensure_single_file(p)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::AcquisitionPolicy;

    fn policy() -> AcquisitionPolicy {
        // Local never needs network — even the default (allow_network=true)
        // must not touch it; use the offline constructor to prove it.
        AcquisitionPolicy::ensure_only()
    }

    fn sha_of(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    #[test]
    fn single_pdf_ensures_unchanged_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("my-paper.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 local drop").unwrap();

        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(pdf.to_str().unwrap()).unwrap();
        assert!(matches!(loc, ArticleLocator::Local { .. }));
        assert!(acq.can_resolve(&loc));
        let resolved = acq.resolve(&loc).unwrap();
        assert_eq!(resolved.unit_key, "my-paper");
        assert_eq!(resolved.source_code, "local");

        let assets = acq.ensure_assets(&resolved, &policy()).unwrap();
        assert!(assets.catalog_hit, "no bytes transferred");
        assert_eq!(assets.primary.kind, PrimaryContentKind::Pdf);
        assert_eq!(
            assets.primary.sha256.as_deref(),
            Some(sha_of(b"%PDF-1.4 local drop").as_str())
        );
        assert_eq!(assets.assets[0].disposition, AssetDisposition::Unchanged);
    }

    #[test]
    fn missing_single_file_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("gone.pdf");
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(pdf.to_str().unwrap()).unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let err = acq.ensure_assets(&resolved, &policy()).unwrap_err();
        assert!(matches!(err, AcquisitionError::MissingAsset(_)), "{err}");
    }

    #[test]
    fn unsupported_extension_never_resolves() {
        let dir = tempfile::tempdir().unwrap();
        let txt = dir.path().join("notes.txt");
        std::fs::write(&txt, b"plain text").unwrap();
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(txt.to_str().unwrap()).unwrap();
        assert!(!acq.can_resolve(&loc));
        assert!(acq.resolve(&loc).is_err());
    }

    #[test]
    fn manifest_dir_happy_path_primary_plus_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("paper.pdf");
        let html = dir.path().join("paper.html");
        std::fs::write(&pdf, b"%PDF-1.4 manifest").unwrap();
        std::fs::write(&html, b"<html>fulltext</html>").unwrap();
        let manifest = format!(
            r#"{{"schema_version":"local-manifest.v1","primary":{{"path":"paper.pdf","sha256":"{pdf_sha}"}},"sidecars":[{{"kind":"html","path":"paper.html","sha256":"{html_sha}"}}]}}"#,
            pdf_sha = sha_of(b"%PDF-1.4 manifest"),
            html_sha = sha_of(b"<html>fulltext</html>"),
        );
        std::fs::write(dir.path().join(MANIFEST_FILE), manifest).unwrap();

        let acq = LocalPathAcquisition::new().with_report_root(dir.path());
        let loc = ArticleLocator::parse(&format!("{}/", dir.path().display())).unwrap();
        assert!(matches!(loc, ArticleLocator::Local { .. }), "{loc:?}");
        let resolved = acq.resolve(&loc).unwrap();
        let assets = acq.ensure_assets(&resolved, &policy()).unwrap();
        assert_eq!(assets.primary.kind, PrimaryContentKind::Pdf);
        assert_eq!(assets.assets.len(), 2);
        assert!(
            assets
                .assets
                .iter()
                .all(|a| a.disposition == AssetDisposition::Unchanged)
        );
        // Sidecar carries its own independent checksum (ADR-056 §2.4 C.12).
        assert_eq!(
            assets.assets[1].sha256.as_deref(),
            Some(sha_of(b"<html>fulltext</html>").as_str())
        );
    }

    #[test]
    fn manifest_dir_missing_manifest_file_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(dir.path().to_str().unwrap()).unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let err = acq.ensure_assets(&resolved, &policy()).unwrap_err();
        assert!(matches!(err, AcquisitionError::MissingAsset(_)), "{err}");
    }

    #[test]
    fn broken_manifest_json_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(MANIFEST_FILE), "{not json").unwrap();
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(dir.path().to_str().unwrap()).unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let err = acq.ensure_assets(&resolved, &policy()).unwrap_err();
        assert!(matches!(err, AcquisitionError::InvalidManifest(_)), "{err}");
    }

    #[test]
    fn wrong_schema_version_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = r#"{"schema_version":"local-manifest.v999","primary":{"path":"paper.pdf"}}"#;
        std::fs::write(dir.path().join(MANIFEST_FILE), manifest).unwrap();
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(dir.path().to_str().unwrap()).unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let err = acq.ensure_assets(&resolved, &policy()).unwrap_err();
        assert!(matches!(err, AcquisitionError::InvalidManifest(_)), "{err}");
    }

    #[test]
    fn manifest_primary_missing_file_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = r#"{"schema_version":"local-manifest.v1","primary":{"path":"absent.pdf"}}"#;
        std::fs::write(dir.path().join(MANIFEST_FILE), manifest).unwrap();
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(dir.path().to_str().unwrap()).unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let err = acq.ensure_assets(&resolved, &policy()).unwrap_err();
        assert!(matches!(err, AcquisitionError::MissingAsset(_)), "{err}");
    }

    #[test]
    fn manifest_checksum_conflict_is_hard_fail() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("paper.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 actual").unwrap();
        let manifest = format!(
            r#"{{"schema_version":"local-manifest.v1","primary":{{"path":"paper.pdf","sha256":"{declared}"}}}}"#,
            declared = "0".repeat(64),
        );
        std::fs::write(dir.path().join(MANIFEST_FILE), manifest).unwrap();
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(dir.path().to_str().unwrap()).unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let err = acq.ensure_assets(&resolved, &policy()).unwrap_err();
        assert!(
            matches!(err, AcquisitionError::ChecksumConflict(_)),
            "{err}"
        );
    }

    #[test]
    fn missing_sidecar_is_typed_partial_primary_still_ok() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("paper.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 partial").unwrap();
        let manifest = r#"{"schema_version":"local-manifest.v1","primary":{"path":"paper.pdf"},"sidecars":[{"kind":"html","path":"missing.html"}]}"#;
        std::fs::write(dir.path().join(MANIFEST_FILE), manifest).unwrap();
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(dir.path().to_str().unwrap()).unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let assets = acq.ensure_assets(&resolved, &policy()).unwrap();
        assert_eq!(assets.primary.kind, PrimaryContentKind::Pdf);
        assert_eq!(assets.assets[1].disposition, AssetDisposition::Fail);
        assert!(assets.assets[1].sha256.is_none());
    }

    #[test]
    fn sidecar_checksum_conflict_is_hard_fail() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("paper.pdf");
        let html = dir.path().join("paper.html");
        std::fs::write(&pdf, b"%PDF-1.4 ok").unwrap();
        std::fs::write(&html, b"<html>changed</html>").unwrap();
        let manifest = format!(
            r#"{{"schema_version":"local-manifest.v1","primary":{{"path":"paper.pdf"}},"sidecars":[{{"kind":"html","path":"paper.html","sha256":"{declared}"}}]}}"#,
            declared = "0".repeat(64),
        );
        std::fs::write(dir.path().join(MANIFEST_FILE), manifest).unwrap();
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(dir.path().to_str().unwrap()).unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let err = acq.ensure_assets(&resolved, &policy()).unwrap_err();
        assert!(
            matches!(err, AcquisitionError::ChecksumConflict(_)),
            "{err}"
        );
    }

    #[test]
    fn idempotent_second_ensure_is_unchanged_again() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("same.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 idempotent").unwrap();
        let acq = LocalPathAcquisition::new();
        let loc = ArticleLocator::parse(pdf.to_str().unwrap()).unwrap();
        let resolved = acq.resolve(&loc).unwrap();
        let first = acq.ensure_assets(&resolved, &policy()).unwrap();
        let second = acq.ensure_assets(&resolved, &policy()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.assets[0].disposition, AssetDisposition::Unchanged);
    }

    #[test]
    fn non_local_locators_rejected() {
        let acq = LocalPathAcquisition::new();
        for raw in ["2602.04837", "catalog:arxiv/cs-ai/2602.04837", "doi:10.1/x"] {
            let loc = ArticleLocator::parse(raw).unwrap();
            assert!(!acq.can_resolve(&loc), "raw={raw}");
            assert!(acq.resolve(&loc).is_err());
        }
    }
}
