//! Port for loading the audited GROBID lexicon/feature contract.
//! Implementations own I/O; callers receive only validated domain data.

pub use da_domain::{
    FeatureAbi, FeatureAbiError, FeatureTemplate, FeatureToken, FeatureVector, LexiconBundle,
    LexiconChannel,
};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum LexiconLoadError {
    #[error("lexicon resource is unavailable: {0}")]
    Unavailable(String),
    #[error("lexicon resource is malformed: {0}")]
    Malformed(String),
    #[error(
        "lexicon resource exceeds the configured byte limit: {actual} bytes (maximum {maximum})"
    )]
    ResourceTooLarge { actual: u64, maximum: u64 },
    #[error("lexicon resource violates its domain contract: {0}")]
    Invalid(String),
}

pub trait LexiconLoaderPort: Send + Sync {
    fn load(&self) -> Result<LexiconBundle, LexiconLoadError>;
}

/// Shared seam contract for implementations and fakes.
pub fn validate_loaded_lexicon(
    loader: &dyn LexiconLoaderPort,
) -> Result<LexiconBundle, LexiconLoadError> {
    let bundle = loader.load()?;
    bundle
        .validate()
        .map_err(|error| LexiconLoadError::Invalid(error.to_string()))?;
    Ok(bundle)
}

/// Keeps path handling at the adapter boundary while making the intended
/// resource explicit to implementations.
pub fn resource_path(path: &Path) -> &Path {
    path
}
