//! Detected visual artifact contract (M371 S02, ADR-051).
//!
//! Detector output is candidate evidence, never graph truth. Coordinates follow
//! pdffigures2 semantics: zero-based page index and top-left PDF crop-box points
//! at 72 DPI. Rendered assets carry checksums and canonical catalog paths.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualArtifactKind {
    Figure,
    Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox72Dpi {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl BoundingBox72Dpi {
    pub fn validate(&self) -> Result<(), VisualArtifactError> {
        let values = [self.x1, self.y1, self.x2, self.y2];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(VisualArtifactError::InvalidBoundingBox(
                "coordinates must be finite".into(),
            ));
        }
        if values.iter().any(|value| *value < 0.0) {
            return Err(VisualArtifactError::InvalidBoundingBox(
                "coordinates must be non-negative".into(),
            ));
        }
        if self.x1 >= self.x2 || self.y1 >= self.y2 {
            return Err(VisualArtifactError::InvalidBoundingBox(
                "x1 < x2 and y1 < y2 are required".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedVisualArtifact {
    pub artifact_id: String,
    pub kind: VisualArtifactKind,
    pub detector_name: String,
    pub detector_fingerprint: String,
    pub source_pdf_sha256: String,
    /// Zero-based page index, matching pdffigures2 output.
    pub page_index: u32,
    /// Top-left crop-box coordinates at 72 DPI.
    pub region: BoundingBox72Dpi,
    /// Top-left crop-box coordinates at 72 DPI.
    pub caption_region: BoundingBox72Dpi,
    pub caption: String,
    pub image_text: Vec<String>,
    pub render_dpi: u32,
    /// Canonical path relative to the article directory, e.g. figures/Figure3.png.
    pub asset_path: String,
    pub asset_sha256: String,
    pub metadata_sha256: String,
}

impl DetectedVisualArtifact {
    pub fn validate(&self) -> Result<(), VisualArtifactError> {
        self.region.validate()?;
        self.caption_region.validate()?;
        if self.artifact_id.trim().is_empty() {
            return Err(VisualArtifactError::Missing("artifact_id"));
        }
        if self.detector_name.trim().is_empty() {
            return Err(VisualArtifactError::Missing("detector_name"));
        }
        if self.detector_fingerprint.trim().is_empty() {
            return Err(VisualArtifactError::Missing("detector_fingerprint"));
        }
        if self.caption.trim().is_empty() {
            return Err(VisualArtifactError::Missing("caption"));
        }
        if self.render_dpi == 0 {
            return Err(VisualArtifactError::InvalidRenderDpi);
        }
        if !self.asset_path.starts_with("figures/")
            || self.asset_path.contains("..")
            || self.asset_path.starts_with('/')
        {
            return Err(VisualArtifactError::InvalidAssetPath(
                self.asset_path.clone(),
            ));
        }
        validate_sha256(&self.source_pdf_sha256, "source_pdf_sha256")?;
        validate_sha256(&self.asset_sha256, "asset_sha256")?;
        validate_sha256(&self.metadata_sha256, "metadata_sha256")?;
        Ok(())
    }
}

fn validate_sha256(value: &str, field: &'static str) -> Result<(), VisualArtifactError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(VisualArtifactError::InvalidSha256(field));
    }
    Ok(())
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VisualArtifactError {
    #[error("missing visual artifact field: {0}")]
    Missing(&'static str),
    #[error("invalid visual artifact bounding box: {0}")]
    InvalidBoundingBox(String),
    #[error("render_dpi must be greater than zero")]
    InvalidRenderDpi,
    #[error("invalid canonical figure asset path: {0}")]
    InvalidAssetPath(String),
    #[error("{0} must be a 64-character SHA-256 hex digest")]
    InvalidSha256(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact() -> DetectedVisualArtifact {
        DetectedVisualArtifact {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            detector_name: "pdffigures2".into(),
            detector_fingerprint: "0.0.12-SNAPSHOT".into(),
            source_pdf_sha256: "a".repeat(64),
            page_index: 3,
            region: BoundingBox72Dpi {
                x1: 106.56,
                y1: 96.96,
                x2: 502.08,
                y2: 324.0,
            },
            caption_region: BoundingBox72Dpi {
                x1: 91.80,
                y1: 338.20,
                x2: 520.20,
                y2: 409.96,
            },
            caption: "Figure 3: Overview of the GEPA system.".into(),
            image_text: vec!["GEPA".into()],
            render_dpi: 150,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: "b".repeat(64),
            metadata_sha256: "c".repeat(64),
        }
    }

    #[test]
    fn valid_figure_contract_roundtrips() {
        let value = artifact();
        value.validate().unwrap();
        let json = serde_json::to_string(&value).unwrap();
        let decoded: DetectedVisualArtifact = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.page_index, 3, "page index remains zero-based");
    }

    #[test]
    fn rejects_reversed_nan_or_negative_bbox() {
        for bbox in [
            BoundingBox72Dpi {
                x1: 2.0,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
            BoundingBox72Dpi {
                x1: f64::NAN,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
            BoundingBox72Dpi {
                x1: -1.0,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
        ] {
            assert!(bbox.validate().is_err());
        }
    }

    #[test]
    fn rejects_temporary_or_parent_traversal_asset_paths() {
        for path in ["/tmp/Figure3.png", "figures/../source.pdf", "Figure3.png"] {
            let mut value = artifact();
            value.asset_path = path.into();
            assert!(matches!(
                value.validate(),
                Err(VisualArtifactError::InvalidAssetPath(_))
            ));
        }
    }

    #[test]
    fn rejects_missing_caption_and_invalid_hashes() {
        let mut value = artifact();
        value.caption.clear();
        assert_eq!(
            value.validate(),
            Err(VisualArtifactError::Missing("caption"))
        );
        value.caption = "Figure 3".into();
        value.asset_sha256 = "not-sha".into();
        assert_eq!(
            value.validate(),
            Err(VisualArtifactError::InvalidSha256("asset_sha256"))
        );
    }
}
