//! Canonical cache payload wire contracts.
//!
//! `FilePipelineCache` deliberately remains byte-level and checksum-only. These
//! contracts are consumed at stage boundaries after a checksum-verified hit and
//! before the result may be reported as a cache hit.

use crate::extractor::ExtractedEntity;
use da_domain::entity::EntityType;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const EXTRACTED_ENTITY_SCHEMA_ID: &str = "extracted_entity.cache.v1";

#[derive(Debug, Error)]
pub enum CachePayloadError {
    #[error("cache payload decode error: {0}")]
    Decode(String),
    #[error("incompatible cache payload {schema_id}: {reason}")]
    Incompatible {
        schema_id: &'static str,
        reason: String,
    },
}

/// Canonical wire schema for `Vec<ExtractedEntity>` cache payloads.
///
/// `section_ordinal` is required on this wire even though the in-memory type
/// keeps an `Option`: legacy payloads that predate section grounding must not
/// silently decode as valid cache hits. Unknown fields are allowed so additive
/// optional evolution remains possible without rejecting compatible payloads.
pub fn extracted_entity_json_schema() -> Value {
    let entity_types = EntityType::all()
        .iter()
        .map(|kind| Value::String(kind.as_str().to_owned()))
        .collect::<Vec<_>>();
    let mut schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": EXTRACTED_ENTITY_SCHEMA_ID,
        "title": "Extracted entity cache payload",
        "type": "array",
        "items": {
            "type": "object",
            "required": [
                "label",
                "entity_type",
                "section_title",
                "section_ordinal",
                "char_start",
                "char_end",
                "surface"
            ],
            "additionalProperties": true,
            "properties": {
                "label": {"type": "string"},
                "entity_type": {"type": "string"},
                "section_title": {"type": "string"},
                "section_ordinal": {"type": "integer", "minimum": 0},
                "char_start": {"type": "integer", "minimum": 0},
                "char_end": {"type": "integer", "minimum": 0},
                "surface": {"type": "string"}
            }
        }
    });
    schema["items"]["properties"]["entity_type"]["enum"] = Value::Array(entity_types);
    schema
}

/// SHA-256 of the canonical serialized wire schema. Any required-field or
/// property-contract change changes this fingerprint and therefore the cache key.
pub fn extracted_entity_output_schema_fingerprint() -> String {
    let bytes = serde_json::to_vec(&extracted_entity_json_schema())
        .expect("canonical extraction schema serialization is infallible");
    hex::encode(Sha256::digest(bytes))
}

/// Decode and validate a cache payload as `Vec<ExtractedEntity>`.
///
/// This is intentionally stricter than `serde_json::from_slice` alone: serde
/// defaults keep `section_ordinal` optional for in-memory compatibility, but
/// a cache payload under the current schema identity must carry grounding.
pub fn decode_extracted_entities(bytes: &[u8]) -> Result<Vec<ExtractedEntity>, CachePayloadError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| CachePayloadError::Decode(error.to_string()))?;
    let Value::Array(items) = value else {
        return Err(CachePayloadError::Decode(
            "extracted entity payload is not a JSON array".into(),
        ));
    };

    let required = [
        "label",
        "entity_type",
        "section_title",
        "section_ordinal",
        "char_start",
        "char_end",
        "surface",
    ];
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            return Err(CachePayloadError::Incompatible {
                schema_id: EXTRACTED_ENTITY_SCHEMA_ID,
                reason: format!("item {index} is not an object"),
            });
        };
        let missing = required
            .iter()
            .filter(|field| object.get(**field).is_none_or(Value::is_null))
            .map(|field| (*field).to_string())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(CachePayloadError::Incompatible {
                schema_id: EXTRACTED_ENTITY_SCHEMA_ID,
                reason: format!("missing required fields: {}", missing.join(", ")),
            });
        }
        if let Some(ordinal) = object.get("section_ordinal")
            && ordinal
                .as_u64()
                .is_none_or(|value| value > usize::MAX as u64)
        {
            return Err(CachePayloadError::Incompatible {
                schema_id: EXTRACTED_ENTITY_SCHEMA_ID,
                reason: format!("item {index} section_ordinal is not a non-negative integer"),
            });
        }
    }

    let extracted: Vec<ExtractedEntity> = serde_json::from_value(Value::Array(items))
        .map_err(|error| CachePayloadError::Decode(error.to_string()))?;
    if extracted
        .iter()
        .any(|entity| entity.section_ordinal.is_none())
    {
        return Err(CachePayloadError::Incompatible {
            schema_id: EXTRACTED_ENTITY_SCHEMA_ID,
            reason: "section_ordinal decoded as None despite wire validation".into(),
        });
    }
    Ok(extracted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_entity_json() -> Value {
        json!({
            "label": "GEPA",
            "entity_type": "Method",
            "section_title": "Methods",
            "section_ordinal": 0,
            "char_start": 12,
            "char_end": 16,
            "surface": "GEPA"
        })
    }

    #[test]
    fn schema_binds_closed_entity_type_vocabulary() {
        let schema = extracted_entity_json_schema();
        let enum_values = schema["items"]["properties"]["entity_type"]["enum"]
            .as_array()
            .expect("entity type enum")
            .clone();
        let expected = EntityType::all()
            .iter()
            .map(|kind| Value::String(kind.as_str().to_owned()))
            .collect::<Vec<_>>();
        assert_eq!(enum_values, expected);
        for kind in EntityType::all() {
            let mut entity = valid_entity_json();
            entity["entity_type"] = Value::String(kind.as_str().to_owned());
            let decoded =
                decode_extracted_entities(serde_json::to_vec(&vec![entity]).unwrap().as_slice())
                    .unwrap();
            assert_eq!(decoded[0].entity_type, kind.clone());
        }

        let mut changed_vocabulary = extracted_entity_json_schema();
        changed_vocabulary["items"]["properties"]["entity_type"]["enum"]
            .as_array_mut()
            .unwrap()
            .push(Value::String("FutureKind".into()));
        let changed_bytes = serde_json::to_vec(&changed_vocabulary).unwrap();
        assert_ne!(
            hex::encode(Sha256::digest(changed_bytes)),
            extracted_entity_output_schema_fingerprint()
        );
    }

    #[test]
    fn fingerprint_is_stable_and_hex64() {
        let first = extracted_entity_output_schema_fingerprint();
        let second = extracted_entity_output_schema_fingerprint();
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn required_field_schema_change_changes_fingerprint() {
        let mut evolved = extracted_entity_json_schema();
        evolved["items"]["required"]
            .as_array_mut()
            .expect("required array")
            .push(Value::String("future_field".into()));
        let evolved_bytes = serde_json::to_vec(&evolved).expect("evolved schema serialization");
        let evolved_fingerprint = hex::encode(Sha256::digest(evolved_bytes));
        assert_ne!(
            evolved_fingerprint,
            extracted_entity_output_schema_fingerprint()
        );
    }

    #[test]
    fn valid_and_empty_payloads_decode() {
        let entity = valid_entity_json();
        let decoded = decode_extracted_entities(
            serde_json::to_vec(&vec![entity.clone()])
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].label, "GEPA");
        assert_eq!(decoded[0].section_ordinal, Some(0));
        assert!(decode_extracted_entities(b"[]").unwrap().is_empty());
    }

    #[test]
    fn missing_or_null_section_ordinal_is_incompatible() {
        let mut missing = valid_entity_json();
        missing.as_object_mut().unwrap().remove("section_ordinal");
        let error = decode_extracted_entities(
            serde_json::to_vec(&vec![missing.clone()])
                .unwrap()
                .as_slice(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("section_ordinal"), "{error}");
        assert!(error.to_string().contains("incompatible"), "{error}");

        let mut null_ordinal = valid_entity_json();
        null_ordinal["section_ordinal"] = Value::Null;
        let error =
            decode_extracted_entities(serde_json::to_vec(&vec![null_ordinal]).unwrap().as_slice())
                .unwrap_err();
        assert!(error.to_string().contains("section_ordinal"), "{error}");
    }

    #[test]
    fn additive_optional_field_is_accepted() {
        let mut evolved = valid_entity_json();
        evolved["future_optional"] = json!({"safe": true});
        let decoded =
            decode_extracted_entities(serde_json::to_vec(&vec![evolved]).unwrap().as_slice())
                .unwrap();
        assert_eq!(decoded[0].section_ordinal, Some(0));
    }

    #[test]
    fn non_array_payload_is_decode_error() {
        let error = decode_extracted_entities(b"{\"label\":\"GEPA\"}").unwrap_err();
        assert!(error.to_string().contains("not a JSON array"), "{error}");
    }
}
