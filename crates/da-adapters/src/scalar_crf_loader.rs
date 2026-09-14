//! Filesystem adapter for an audited Wapiti scalar CRF model.
//! Model bytes are supplied by a caller-owned local resource path; none are bundled.

use da_domain::scalar_crf::{CrfModel, CrfModelError, MAX_CRF_MODEL_BYTES};
use da_ports::{CrfModelLoadError, CrfModelLoaderPort};
use std::path::{Path, PathBuf};

/// Filesystem loader for a text Wapiti model.
#[derive(Debug, Clone)]
pub struct FilesystemCrfModelLoader {
    path: PathBuf,
}

impl FilesystemCrfModelLoader {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl CrfModelLoaderPort for FilesystemCrfModelLoader {
    fn load(&self) -> Result<CrfModel, CrfModelLoadError> {
        let size = std::fs::metadata(&self.path)
            .map_err(|_| CrfModelLoadError::Unavailable("stat CRF model resource".into()))?
            .len();
        let maximum = MAX_CRF_MODEL_BYTES as u64;
        if size > maximum {
            return Err(CrfModelLoadError::ResourceTooLarge {
                actual: size,
                maximum,
            });
        }
        let raw = std::fs::read(&self.path)
            .map_err(|_| CrfModelLoadError::Unavailable("read CRF model resource".into()))?;
        CrfModel::parse(&raw).map_err(map_model_error)
    }
}

fn map_model_error(error: CrfModelError) -> CrfModelLoadError {
    match error {
        CrfModelError::LimitsExceeded {
            resource: "bytes",
            actual,
            maximum,
        } => CrfModelLoadError::ResourceTooLarge {
            actual: actual as u64,
            maximum: maximum as u64,
        },
        other => CrfModelLoadError::Malformed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::CrfModelLoaderPort;

    fn model_bytes() -> Vec<u8> {
        b"#mdl#2#1\n#rdr#1/0\n7:u00:foo,\n#qrk#2\n1:A,\n1:B,\n#qrk#1\n5:u:foo,\n0=0x1p+0\n".to_vec()
    }

    #[test]
    fn loads_model_without_exposing_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.wapiti");
        std::fs::write(&path, model_bytes()).unwrap();
        let model = FilesystemCrfModelLoader::new(path).load().unwrap();
        assert_eq!(model.labels, ["A", "B"]);
        assert_eq!(model.weights[&0], 1.0);
    }

    #[test]
    fn missing_resource_is_sanitized() {
        let path = "/private/path/to/missing-model.wapiti";
        let error = FilesystemCrfModelLoader::new(path).load().unwrap_err();
        assert!(matches!(error, CrfModelLoadError::Unavailable(_)));
        assert!(!error.to_string().contains(path));
    }

    #[test]
    fn malformed_model_is_typed_and_sanitized() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("malformed.wapiti");
        std::fs::write(&path, b"not-a-model").unwrap();
        let error = FilesystemCrfModelLoader::new(path).load().unwrap_err();
        assert!(matches!(error, CrfModelLoadError::Malformed(_)));
    }

    #[test]
    fn checks_metadata_limit_before_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oversized.wapiti");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_CRF_MODEL_BYTES as u64 + 1).unwrap();
        let error = FilesystemCrfModelLoader::new(path).load().unwrap_err();
        assert!(matches!(
            error,
            CrfModelLoadError::ResourceTooLarge { actual, maximum }
                if actual == MAX_CRF_MODEL_BYTES as u64 + 1
                    && maximum == MAX_CRF_MODEL_BYTES as u64
        ));
    }
}
