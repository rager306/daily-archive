//! Port for loading an audited Wapiti scalar CRF model.
//! Filesystem access remains in adapters; callers receive validated domain data.

pub use da_domain::scalar_crf::{
    CrfDecodeResult, CrfModel, CrfModelError, CrfPattern, CrfPosition, MAX_CRF_LABELS,
    MAX_CRF_MODEL_BYTES, MAX_CRF_OBSERVATIONS, MAX_CRF_PATTERNS, MAX_CRF_WEIGHTS, expand_pattern,
    expand_pattern_sequence, map_input, viterbi_decode,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CrfModelLoadError {
    #[error("CRF model resource is unavailable: {0}")]
    Unavailable(String),
    #[error("CRF model resource is malformed: {0}")]
    Malformed(String),
    #[error(
        "CRF model resource exceeds the configured byte limit: {actual} bytes (maximum {maximum})"
    )]
    ResourceTooLarge { actual: u64, maximum: u64 },
    #[error("CRF model violates its domain contract: {0}")]
    Invalid(String),
}

/// Boundary for loading a scalar CRF model from any resource provider.
pub trait CrfModelLoaderPort: Send + Sync {
    fn load(&self) -> Result<CrfModel, CrfModelLoadError>;
}

/// Shared seam validation for filesystem loaders and test fakes.
pub fn validate_loaded_crf_model(
    loader: &dyn CrfModelLoaderPort,
) -> Result<CrfModel, CrfModelLoadError> {
    let model = loader.load()?;
    if model.model_type != 2 {
        return Err(CrfModelLoadError::Invalid(
            "only scalar CRF models are supported".into(),
        ));
    }
    if model.labels.is_empty()
        || model.u_offsets.len() != model.observations.len()
        || model.b_offsets.len() != model.observations.len()
    {
        return Err(CrfModelLoadError::Invalid(
            "model label and observation tables are inconsistent".into(),
        ));
    }
    if model.weights.keys().any(|&index| index >= model.nftr)
        || model.weights.values().any(|value| !value.is_finite())
    {
        return Err(CrfModelLoadError::Invalid(
            "model weights are inconsistent".into(),
        ));
    }
    Ok(model)
}
