//! Filesystem adapter for the audited GROBID lexicon contract.
//! No bundled model bytes are included; callers provide the runtime resource path.

use da_domain::LexiconBundle;
use da_ports::{LexiconLoadError, LexiconLoaderPort};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FilesystemLexiconLoader {
    path: PathBuf,
}

impl FilesystemLexiconLoader {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl LexiconLoaderPort for FilesystemLexiconLoader {
    fn load(&self) -> Result<LexiconBundle, LexiconLoadError> {
        let raw = std::fs::read_to_string(&self.path).map_err(|error| {
            LexiconLoadError::Unavailable(format!("read lexicon resource: {error}"))
        })?;
        let bundle: LexiconBundle = serde_yaml::from_str(&raw).map_err(|error| {
            LexiconLoadError::Malformed(format!("parse lexicon resource: {error}"))
        })?;
        bundle
            .validate()
            .map_err(|error| LexiconLoadError::Invalid(error.to_string()))?;
        Ok(bundle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::{
        FeatureTemplate, GROBID_LEXICON_SCHEMA_VERSION, GROBID_RESOURCE_VERSION, LexiconChannel,
    };

    fn valid_yaml() -> &'static str {
        "schema_version: grobid-lexicon.v1\nresource_version: 0.9.0-crf\nchannels:\n  - key: chemical\n    entries: [acid, water]\nfeature_templates:\n  - key: layout-basic\n    channel_keys: [chemical]\n    feature_inputs: [token.text]\n"
    }

    #[test]
    fn loader_returns_validated_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lexicon.yaml");
        std::fs::write(&path, valid_yaml()).unwrap();
        let loaded = FilesystemLexiconLoader::new(path).load().unwrap();
        assert_eq!(loaded.schema_version, GROBID_LEXICON_SCHEMA_VERSION);
        assert_eq!(loaded.resource_version, GROBID_RESOURCE_VERSION);
        assert_eq!(loaded.channels[0].entries, vec!["acid", "water"]);
    }

    #[test]
    fn domain_types_are_constructible_without_io() {
        let bundle = LexiconBundle {
            schema_version: GROBID_LEXICON_SCHEMA_VERSION.into(),
            resource_version: GROBID_RESOURCE_VERSION.into(),
            channels: vec![LexiconChannel {
                key: "a".into(),
                entries: vec!["x".into()],
            }],
            feature_templates: vec![FeatureTemplate {
                key: "t".into(),
                channel_keys: vec!["a".into()],
                feature_inputs: vec!["token".into()],
            }],
        };
        assert!(bundle.validate().is_ok());
    }

    #[test]
    fn missing_resource_is_typed_and_fail_closed() {
        let result = FilesystemLexiconLoader::new("/missing/grobid-lexicon.yaml").load();
        assert!(matches!(result, Err(LexiconLoadError::Unavailable(_))));
    }
}
