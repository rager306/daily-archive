//! Durable dual identity for cached stage computation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::assurance::ContentDigest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheLineageEvidence {
    pub article_id: String,
    pub stage_id: String,
    pub key_sha256: ContentDigest,
    pub payload_sha256: ContentDigest,
    pub payload_size: u64,
    pub dependency_output_sha256: BTreeMap<String, ContentDigest>,
    pub implementation_fingerprint: String,
    pub configuration_fingerprint: String,
    pub prompt_fingerprint: Option<String>,
    pub model_fingerprint: Option<String>,
    pub output_schema_fingerprint: String,
    pub cache_hit: bool,
    pub graph_materialization_skipped: bool,
    pub import_eligible: bool,
}

impl CacheLineageEvidence {
    pub fn validate(&self) -> Result<(), String> {
        let required = [
            self.article_id.as_str(),
            self.stage_id.as_str(),
            self.implementation_fingerprint.as_str(),
            self.configuration_fingerprint.as_str(),
            self.output_schema_fingerprint.as_str(),
        ];
        if required.into_iter().any(|value| value.trim().is_empty()) {
            return Err("cache lineage required identity is empty".into());
        }
        if self.payload_size == 0 {
            return Err("cache lineage cannot claim an empty payload".into());
        }
        if self
            .dependency_output_sha256
            .keys()
            .any(|value| value.trim().is_empty())
        {
            return Err("cache lineage dependency stage is empty".into());
        }
        if self.graph_materialization_skipped {
            return Err("compute cache cannot suppress graph materialization".into());
        }
        if self.import_eligible {
            return Err("cache lineage cannot authorize import".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn evidence(key: &str, payload: &str) -> CacheLineageEvidence {
        CacheLineageEvidence {
            article_id: "x".into(),
            stage_id: "parse".into(),
            key_sha256: ContentDigest::of_text(key),
            payload_sha256: ContentDigest::of_text(payload),
            payload_size: 7,
            dependency_output_sha256: BTreeMap::new(),
            implementation_fingerprint: "impl".into(),
            configuration_fingerprint: "config".into(),
            prompt_fingerprint: None,
            model_fingerprint: Some("model".into()),
            output_schema_fingerprint: "schema".into(),
            cache_hit: true,
            graph_materialization_skipped: false,
            import_eligible: false,
        }
    }
    #[test]
    fn same_payload_different_keys_remain_distinct() {
        let a = evidence("key-a", "same");
        let b = evidence("key-b", "same");
        assert_eq!(a.payload_sha256, b.payload_sha256);
        assert_ne!(a.key_sha256, b.key_sha256);
        a.validate().unwrap();
        b.validate().unwrap();
    }
    #[test]
    fn cache_cannot_claim_skipped_materialization() {
        let mut x = evidence("key", "payload");
        x.graph_materialization_skipped = true;
        assert!(x.validate().unwrap_err().contains("cannot suppress"));
    }
}
