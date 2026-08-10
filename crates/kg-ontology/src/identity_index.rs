//! Data-driven identity property indexes.
//!
//! This module describes graph lookup keys only. It intentionally does not
//! define vertex-ID namespaces, canonicalisation algorithms, migrations,
//! filesystem paths, or storage operations.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::OntologyRegistry;

pub const IDENTITY_INDEX_SCHEMA_VERSION: &str = "ontology-v2-identity-indexes.v1";
pub const PROPERTY_INDEX_SCHEMA_VERSION: &str = "ontology-v2-property-indexes.v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IdentityIndexSpec {
    pub label: String,
    pub property: String,
    pub identity: bool,
    #[serde(default = "default_required")]
    pub required: bool,
}

fn default_required() -> bool {
    true
}

impl IdentityIndexSpec {
    pub fn key(&self) -> String {
        format!("{}.{}", self.label, self.property)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityIndexPolicy {
    schema_version: String,
    indexes: Vec<IdentityIndexSpec>,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct RawIdentityIndexPolicy {
    schema_version: String,
    indexes: Vec<IdentityIndexSpec>,
}

impl IdentityIndexPolicy {
    pub fn parse_and_validate(
        bytes: &[u8],
        registry: &OntologyRegistry,
    ) -> Result<Self, IdentityIndexError> {
        let raw: RawIdentityIndexPolicy = serde_yaml::from_slice(bytes)
            .map_err(|error| IdentityIndexError::Malformed(error.to_string()))?;
        if raw.schema_version != IDENTITY_INDEX_SCHEMA_VERSION {
            return Err(IdentityIndexError::UnsupportedSchema(raw.schema_version));
        }
        if raw.indexes.is_empty() {
            return Err(IdentityIndexError::Empty);
        }

        let mut indexes = raw.indexes;
        indexes.sort();
        let mut seen = BTreeSet::new();
        for index in &indexes {
            if index.label.trim().is_empty() || index.property.trim().is_empty() {
                return Err(IdentityIndexError::Malformed(
                    "label and property must be non-empty".into(),
                ));
            }
            if !index.identity {
                return Err(IdentityIndexError::NonIdentity(index.key()));
            }
            let schema = registry
                .get_schema(&index.label)
                .ok_or_else(|| IdentityIndexError::UnknownLabel(index.label.clone()))?;
            let property_exists = schema
                .required_fields
                .iter()
                .chain(&schema.optional_fields)
                .any(|field| field.name == index.property);
            if !property_exists {
                return Err(IdentityIndexError::UnknownProperty(index.key()));
            }
            if !seen.insert((index.label.clone(), index.property.clone())) {
                return Err(IdentityIndexError::Duplicate(index.key()));
            }
        }

        let canonical = serde_json::to_vec(&(IDENTITY_INDEX_SCHEMA_VERSION, &indexes))
            .expect("identity index policy is serializable");
        let sha256 = format!("sha256:{}", hex::encode(Sha256::digest(canonical)));
        Ok(Self {
            schema_version: IDENTITY_INDEX_SCHEMA_VERSION.into(),
            indexes,
            sha256,
        })
    }

    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub fn indexes(&self) -> &[IdentityIndexSpec] {
        &self.indexes
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PropertyIndexPolicy {
    schema_version: String,
    indexes: Vec<IdentityIndexSpec>,
    sha256: String,
}

impl PropertyIndexPolicy {
    pub fn parse_and_validate(
        bytes: &[u8],
        registry: &OntologyRegistry,
    ) -> Result<Self, IdentityIndexError> {
        let mut raw: RawIdentityIndexPolicy = serde_yaml::from_slice(bytes)
            .map_err(|error| IdentityIndexError::Malformed(error.to_string()))?;
        if raw.schema_version != PROPERTY_INDEX_SCHEMA_VERSION {
            return Err(IdentityIndexError::UnsupportedSchema(raw.schema_version));
        }
        if raw.indexes.is_empty() {
            return Err(IdentityIndexError::Empty);
        }
        raw.indexes.sort();
        let mut seen = BTreeSet::new();
        for index in &raw.indexes {
            validate_registered_property(index, registry)?;
            if index.identity {
                return Err(IdentityIndexError::IdentityInPropertyPolicy(index.key()));
            }
            if !seen.insert((index.label.clone(), index.property.clone())) {
                return Err(IdentityIndexError::Duplicate(index.key()));
            }
        }
        let canonical = serde_json::to_vec(&(PROPERTY_INDEX_SCHEMA_VERSION, &raw.indexes))
            .expect("property index policy is serializable");
        Ok(Self {
            schema_version: PROPERTY_INDEX_SCHEMA_VERSION.into(),
            indexes: raw.indexes,
            sha256: format!("sha256:{}", hex::encode(Sha256::digest(canonical))),
        })
    }

    pub fn indexes(&self) -> &[IdentityIndexSpec] {
        &self.indexes
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

fn validate_registered_property(
    index: &IdentityIndexSpec,
    registry: &OntologyRegistry,
) -> Result<(), IdentityIndexError> {
    if index.label.trim().is_empty() || index.property.trim().is_empty() {
        return Err(IdentityIndexError::Malformed(
            "label and property must be non-empty".into(),
        ));
    }
    let schema = registry
        .get_schema(&index.label)
        .ok_or_else(|| IdentityIndexError::UnknownLabel(index.label.clone()))?;
    if !schema
        .required_fields
        .iter()
        .chain(&schema.optional_fields)
        .any(|field| field.name == index.property)
    {
        return Err(IdentityIndexError::UnknownProperty(index.key()));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityIndexError {
    #[error("malformed identity index policy: {0}")]
    Malformed(String),
    #[error("unsupported identity index schema: {0}")]
    UnsupportedSchema(String),
    #[error("identity index policy is empty")]
    Empty,
    #[error("identity index references unknown label: {0}")]
    UnknownLabel(String),
    #[error("identity index references unknown property: {0}")]
    UnknownProperty(String),
    #[error("duplicate identity index: {0}")]
    Duplicate(String),
    #[error("non-identity entry in identity policy: {0}")]
    NonIdentity(String),
    #[error("identity entry in property-index policy: {0}")]
    IdentityInPropertyPolicy(String),
}
