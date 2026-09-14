use da_domain::{
    FeatureAbi, FeatureAbiError, FeatureChannel, FeatureTemplate, FeatureToken, FeatureVector,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderFixture {
    provider: String,
    page: u32,
    tokens: Vec<FeatureToken>,
}

#[derive(Debug, Error)]
pub enum ProviderParityError {
    #[error("fixture is missing: {fixture}")]
    MissingFixture { fixture: String },
    #[error("fixture is malformed: {fixture}")]
    MalformedFixture { fixture: String },
    #[error("provider fixture has no tokens: {provider}")]
    EmptyFixture { provider: String },
    #[error("feature ABI rejected provider {provider}: {source}")]
    Abi {
        provider: String,
        #[source]
        source: FeatureAbiError,
    },
    #[error("provider outputs differ: {left} vs {right}")]
    OutputMismatch { left: String, right: String },
}

pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn fixture_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("fixture")
        .to_owned()
}

pub fn load_tokens(path: &Path) -> Result<(String, Vec<FeatureToken>), ProviderParityError> {
    if !path.is_file() {
        return Err(ProviderParityError::MissingFixture {
            fixture: fixture_label(path),
        });
    }
    let bytes = std::fs::read(path).map_err(|_| ProviderParityError::MalformedFixture {
        fixture: fixture_label(path),
    })?;
    let fixture: ProviderFixture =
        serde_json::from_slice(&bytes).map_err(|_| ProviderParityError::MalformedFixture {
            fixture: fixture_label(path),
        })?;
    if fixture.page == 0 || fixture.tokens.is_empty() {
        return Err(ProviderParityError::EmptyFixture {
            provider: fixture.provider,
        });
    }
    Ok((fixture.provider, fixture.tokens))
}

pub fn demo_abi() -> FeatureAbi {
    FeatureAbi {
        channels: vec![FeatureChannel {
            key: "heading".into(),
            entries: vec!["Heading".into()],
        }],
        templates: vec![FeatureTemplate {
            key: "layout-basic".into(),
            channel_keys: vec!["heading".into()],
            feature_inputs: vec![
                "token.text".into(),
                "token.x".into(),
                "token.y".into(),
                "token.bold".into(),
            ],
        }],
        expected_dimension: None,
    }
}

pub fn extract_provider(
    abi: &FeatureAbi,
    path: &Path,
) -> Result<(String, FeatureVector), ProviderParityError> {
    let (provider, tokens) = load_tokens(path)?;
    let vector = abi
        .extract(&tokens)
        .map_err(|source| ProviderParityError::Abi {
            provider: provider.clone(),
            source,
        })?;
    Ok((provider, vector))
}

pub fn compare_providers(
    abi: &FeatureAbi,
    left: &Path,
    right: &Path,
) -> Result<(String, String, FeatureVector), ProviderParityError> {
    let (left_provider, left_vector) = extract_provider(abi, left)?;
    let (right_provider, right_vector) = extract_provider(abi, right)?;
    if left_vector != right_vector {
        return Err(ProviderParityError::OutputMismatch {
            left: left_provider,
            right: right_provider,
        });
    }
    Ok((left_provider, right_provider, left_vector))
}
