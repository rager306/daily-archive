//! Typed, provider-neutral representation of GROBID lexical feature inputs.
//! The domain owns validation only; filesystem access belongs to an adapter.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const GROBID_LEXICON_SCHEMA_VERSION: &str = "grobid-lexicon.v1";
pub const GROBID_RESOURCE_VERSION: &str = "0.9.0-crf";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexiconBundle {
    pub schema_version: String,
    pub resource_version: String,
    pub channels: Vec<LexiconChannel>,
    pub feature_templates: Vec<FeatureTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexiconChannel {
    pub key: String,
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureTemplate {
    pub key: String,
    pub channel_keys: Vec<String>,
    pub feature_inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LexiconError {
    #[error("unsupported lexicon schema version: {0}")]
    UnsupportedSchemaVersion(String),
    #[error("unsupported GROBID resource version: {0}")]
    UnsupportedResourceVersion(String),
    #[error("lexicon must declare at least one channel")]
    EmptyChannels,
    #[error("lexicon must declare at least one feature template")]
    EmptyFeatureTemplates,
    #[error("{kind} keys must be strictly ordered and unique")]
    UnorderedKeys { kind: &'static str },
    #[error("{kind} key is blank")]
    BlankKey { kind: &'static str },
    #[error("lexicon channel {channel} has no entries")]
    EmptyEntries { channel: String },
    #[error("lexicon channel {channel} entries must be unique and ordered")]
    UnorderedEntries { channel: String },
    #[error("feature template {template} references unknown channel {channel}")]
    UnknownChannel { template: String, channel: String },
    #[error("feature template {template} must declare at least one channel and input")]
    EmptyTemplateInputs { template: String },
}

impl LexiconBundle {
    pub fn validate(&self) -> Result<(), LexiconError> {
        if self.schema_version != GROBID_LEXICON_SCHEMA_VERSION {
            return Err(LexiconError::UnsupportedSchemaVersion(
                self.schema_version.clone(),
            ));
        }
        if self.resource_version != GROBID_RESOURCE_VERSION {
            return Err(LexiconError::UnsupportedResourceVersion(
                self.resource_version.clone(),
            ));
        }
        if self.channels.is_empty() {
            return Err(LexiconError::EmptyChannels);
        }
        if self.feature_templates.is_empty() {
            return Err(LexiconError::EmptyFeatureTemplates);
        }
        validate_ordered_keys(
            self.channels.iter().map(|channel| channel.key.as_str()),
            "channel",
        )?;
        let channel_keys: BTreeSet<_> = self
            .channels
            .iter()
            .map(|channel| channel.key.as_str())
            .collect();
        for channel in &self.channels {
            if channel.entries.is_empty() {
                return Err(LexiconError::EmptyEntries {
                    channel: channel.key.clone(),
                });
            }
            if channel.entries.iter().any(|entry| entry.trim().is_empty())
                || channel.entries.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(LexiconError::UnorderedEntries {
                    channel: channel.key.clone(),
                });
            }
        }
        validate_ordered_keys(
            self.feature_templates
                .iter()
                .map(|template| template.key.as_str()),
            "feature template",
        )?;
        for template in &self.feature_templates {
            if template.channel_keys.is_empty() || template.feature_inputs.is_empty() {
                return Err(LexiconError::EmptyTemplateInputs {
                    template: template.key.clone(),
                });
            }
            if template
                .channel_keys
                .iter()
                .any(|key| !channel_keys.contains(key.as_str()))
                || template
                    .channel_keys
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || template
                    .feature_inputs
                    .iter()
                    .any(|input| input.trim().is_empty())
            {
                let unknown = template
                    .channel_keys
                    .iter()
                    .find(|key| !channel_keys.contains(key.as_str()))
                    .cloned()
                    .unwrap_or_default();
                if !unknown.is_empty() {
                    return Err(LexiconError::UnknownChannel {
                        template: template.key.clone(),
                        channel: unknown,
                    });
                }
                return Err(LexiconError::EmptyTemplateInputs {
                    template: template.key.clone(),
                });
            }
        }
        Ok(())
    }
}

fn validate_ordered_keys<'a>(
    keys: impl IntoIterator<Item = &'a str>,
    kind: &'static str,
) -> Result<(), LexiconError> {
    let mut previous = None;
    for key in keys {
        if key.trim().is_empty() {
            return Err(LexiconError::BlankKey { kind });
        }
        if previous.is_some_and(|prior: &str| prior >= key) {
            return Err(LexiconError::UnorderedKeys { kind });
        }
        previous = Some(key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn valid_bundle() -> LexiconBundle {
        LexiconBundle {
            schema_version: GROBID_LEXICON_SCHEMA_VERSION.into(),
            resource_version: GROBID_RESOURCE_VERSION.into(),
            channels: vec![
                LexiconChannel {
                    key: "chemical".into(),
                    entries: vec!["acid".into(), "water".into()],
                },
                LexiconChannel {
                    key: "unit".into(),
                    entries: vec!["kg".into(), "m".into()],
                },
            ],
            feature_templates: vec![FeatureTemplate {
                key: "layout-basic".into(),
                channel_keys: vec!["chemical".into(), "unit".into()],
                feature_inputs: vec!["token.text".into(), "token.x".into()],
            }],
        }
    }

    #[test]
    fn valid_bundle_roundtrips_and_validates() {
        let bundle = valid_bundle();
        bundle.validate().unwrap();
        let encoded = serde_json::to_string(&bundle).unwrap();
        assert_eq!(
            serde_json::from_str::<LexiconBundle>(&encoded).unwrap(),
            bundle
        );
    }

    #[test]
    fn rejects_duplicate_or_unordered_keys() {
        let mut bundle = valid_bundle();
        bundle.channels.reverse();
        assert!(matches!(
            bundle.validate(),
            Err(LexiconError::UnorderedKeys { .. })
        ));
        let mut bundle = valid_bundle();
        bundle.channels[0].entries.push("acid".into());
        assert!(matches!(
            bundle.validate(),
            Err(LexiconError::UnorderedEntries { .. })
        ));
    }

    #[test]
    fn rejects_incompatible_versions_and_unknown_channels() {
        let mut bundle = valid_bundle();
        bundle.resource_version = "0.8.0-crf".into();
        assert!(matches!(
            bundle.validate(),
            Err(LexiconError::UnsupportedResourceVersion(_))
        ));
        let mut bundle = valid_bundle();
        bundle.feature_templates[0].channel_keys = vec!["missing".into()];
        assert!(matches!(
            bundle.validate(),
            Err(LexiconError::UnknownChannel { .. })
        ));
    }
}
