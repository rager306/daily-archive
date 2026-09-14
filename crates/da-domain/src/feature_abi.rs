//! Deterministic, provider-neutral feature extraction from layout tokens.
//! No I/O, clocks, IDs, randomness, or tracing belong in this module.

use crate::{FeatureTemplate, LexiconBundle};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A token and the layout facts exposed to a model feature template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureToken {
    pub text: String,
    pub page: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
}

/// The ordered ABI used to turn tokens into a flat feature vector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureAbi {
    pub channels: Vec<FeatureChannel>,
    pub templates: Vec<FeatureTemplate>,
    /// Optional fixed model input size. `None` permits the natural
    /// token-count-dependent size while still checking arithmetic overflow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_dimension: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureChannel {
    pub key: String,
    pub entries: Vec<String>,
}

pub type FeatureVector = Vec<f32>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FeatureAbiError {
    #[error("feature ABI has no channels")]
    EmptyChannels,
    #[error("feature ABI has no templates")]
    EmptyTemplates,
    #[error("feature ABI channels are not strictly ordered")]
    UnorderedChannels,
    #[error("feature ABI templates are not strictly ordered")]
    UnorderedTemplates,
    #[error("feature ABI contains an unknown input: {0}")]
    UnsupportedInput(String),
    #[error("feature ABI template {template} references unknown channel {channel}")]
    UnknownChannel { template: String, channel: String },
    #[error("feature ABI dimension overflow")]
    DimensionOverflow,
    #[error("feature ABI dimension mismatch: expected {expected}, produced {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("non-finite layout value in token {token}")]
    NonFiniteLayout { token: usize },
}

impl FeatureAbi {
    /// Build the feature ABI without copying any provider or filesystem state.
    pub fn from_lexicon(bundle: &LexiconBundle) -> Result<Self, FeatureAbiError> {
        bundle
            .validate()
            .map_err(|_| FeatureAbiError::UnorderedChannels)?;
        let abi = Self {
            channels: bundle
                .channels
                .iter()
                .map(|c| FeatureChannel {
                    key: c.key.clone(),
                    entries: c.entries.clone(),
                })
                .collect(),
            templates: bundle.feature_templates.clone(),
            expected_dimension: None,
        };
        abi.validate()?;
        Ok(abi)
    }

    pub fn validate(&self) -> Result<(), FeatureAbiError> {
        if self.channels.is_empty() {
            return Err(FeatureAbiError::EmptyChannels);
        }
        if self.templates.is_empty() {
            return Err(FeatureAbiError::EmptyTemplates);
        }
        if self.channels.windows(2).any(|w| w[0].key >= w[1].key) {
            return Err(FeatureAbiError::UnorderedChannels);
        }
        if self.templates.windows(2).any(|w| w[0].key >= w[1].key) {
            return Err(FeatureAbiError::UnorderedTemplates);
        }
        let channel_keys: std::collections::BTreeSet<&str> = self
            .channels
            .iter()
            .map(|channel| channel.key.as_str())
            .collect();
        for template in &self.templates {
            for channel in &template.channel_keys {
                if !channel_keys.contains(channel.as_str()) {
                    return Err(FeatureAbiError::UnknownChannel {
                        template: template.key.clone(),
                        channel: channel.clone(),
                    });
                }
            }
            for input in &template.feature_inputs {
                if !matches!(
                    input.as_str(),
                    "token.text"
                        | "token.x"
                        | "token.y"
                        | "token.width"
                        | "token.height"
                        | "token.page"
                        | "token.bold"
                        | "token.italic"
                ) {
                    return Err(FeatureAbiError::UnsupportedInput(input.clone()));
                }
            }
        }
        Ok(())
    }

    pub fn dimension_for(&self, token_count: usize) -> Result<usize, FeatureAbiError> {
        let inputs = self.templates.iter().try_fold(0usize, |total, template| {
            total
                .checked_add(template.feature_inputs.len())
                .ok_or(FeatureAbiError::DimensionOverflow)
        })?;
        token_count
            .checked_mul(self.channels.len())
            .and_then(|n| n.checked_mul(inputs))
            .ok_or(FeatureAbiError::DimensionOverflow)
    }

    /// Extracts values in canonical token order, then ABI channel/template/input order.
    /// Lexicon channels emit membership for `token.text`; layout inputs emit their
    /// corresponding scalar (repeated per channel as required by the ABI).
    pub fn extract(&self, tokens: &[FeatureToken]) -> Result<FeatureVector, FeatureAbiError> {
        self.validate()?;
        for (index, token) in tokens.iter().enumerate() {
            if [token.x, token.y, token.width, token.height]
                .iter()
                .any(|v| !v.is_finite())
            {
                return Err(FeatureAbiError::NonFiniteLayout { token: index });
            }
        }
        let dimension = self.dimension_for(tokens.len())?;
        if let Some(expected) = self
            .expected_dimension
            .filter(|expected| *expected != dimension)
        {
            return Err(FeatureAbiError::DimensionMismatch {
                expected,
                actual: dimension,
            });
        }
        let mut ordered = tokens.to_vec();
        ordered.sort_by(|a, b| {
            (a.page, a.y.to_bits(), a.x.to_bits(), &a.text).cmp(&(
                b.page,
                b.y.to_bits(),
                b.x.to_bits(),
                &b.text,
            ))
        });
        let mut out = Vec::with_capacity(dimension);
        for token in ordered {
            for channel in &self.channels {
                for template in &self.templates {
                    for input in &template.feature_inputs {
                        out.push(value_for(input, &token, channel));
                    }
                }
            }
        }
        Ok(out)
    }
}

fn value_for(input: &str, token: &FeatureToken, channel: &FeatureChannel) -> f32 {
    match input {
        "token.text" => channel.entries.binary_search(&token.text).is_ok() as u8 as f32,
        "token.x" => token.x,
        "token.y" => token.y,
        "token.width" => token.width,
        "token.height" => token.height,
        "token.page" => token.page as f32,
        "token.bold" => token.bold as u8 as f32,
        "token.italic" => token.italic as u8 as f32,
        _ => 0.0, // validate() rejects this before extraction.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn abi() -> FeatureAbi {
        FeatureAbi {
            channels: vec![FeatureChannel {
                key: "chemical".into(),
                entries: vec!["acid".into(), "water".into()],
            }],
            templates: vec![FeatureTemplate {
                key: "basic".into(),
                channel_keys: vec!["chemical".into()],
                feature_inputs: vec!["token.text".into(), "token.x".into()],
            }],
            expected_dimension: None,
        }
    }
    fn token(text: &str, x: f32) -> FeatureToken {
        FeatureToken {
            text: text.into(),
            page: 1,
            x,
            y: 2.0,
            width: 3.0,
            height: 4.0,
            bold: false,
            italic: false,
        }
    }

    #[test]
    fn extracts_in_fixed_order_and_is_order_independent() {
        let a = vec![token("water", 1.0), token("unknown", 0.0)];
        let b = vec![a[1].clone(), a[0].clone()];
        assert_eq!(abi().extract(&a).unwrap(), abi().extract(&b).unwrap());
        assert_eq!(abi().extract(&a).unwrap(), vec![0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn rejects_unknown_channels_and_checked_dimension_overflow() {
        let mut unknown = abi();
        unknown.templates[0].channel_keys = vec!["missing".into()];
        assert!(matches!(
            unknown.validate(),
            Err(FeatureAbiError::UnknownChannel { .. })
        ));

        let mut huge = abi();
        huge.channels.push(FeatureChannel {
            key: "zeta".into(),
            entries: vec!["acid".into()],
        });
        assert_eq!(
            huge.dimension_for(usize::MAX),
            Err(FeatureAbiError::DimensionOverflow)
        );
    }

    #[test]
    fn reports_dimension_mismatch_and_non_finite_values() {
        let mut fixed = abi();
        fixed.expected_dimension = Some(99);
        assert!(matches!(
            fixed.extract(&[token("acid", 1.0)]),
            Err(FeatureAbiError::DimensionMismatch { .. })
        ));
        let mut bad = token("acid", 1.0);
        bad.x = f32::NAN;
        assert!(matches!(
            abi().extract(&[bad]),
            Err(FeatureAbiError::NonFiniteLayout { .. })
        ));
    }

    #[test]
    fn vector_roundtrips() {
        let vector = abi().extract(&[token("acid", 1.0)]).unwrap();
        crate::roundtrip_json(&vector);
    }

    proptest! {
        #[test]
        fn same_input_is_deterministic(x in -1000.0f32..1000.0, text in "[a-z]{1,8}") {
            let t = token(&text, x);
            prop_assert_eq!(abi().extract(std::slice::from_ref(&t)), abi().extract(std::slice::from_ref(&t)));
        }
    }
}
