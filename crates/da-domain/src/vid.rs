//! SHA256 stable VIDs (Identifier-Preserving Joins, ADR-038 §6 P1).
//!
//! Every entity gets a deterministic SHA256 ID from its canonical form.
//! This enables O(|K|) hash joins — no false merges across sources.

use sha2::{Digest, Sha256};

/// A stable vertex ID (SHA256 hex, 64 chars).
pub type Vid = String;

/// Compute a Paper VID from arxiv_id.
pub fn paper_vid(arxiv_id: &str) -> Vid {
    let id = arxiv_id.trim().replace("arxiv:", "");
    let mut hasher = Sha256::new();
    hasher.update(b"paper:");
    hasher.update(id.as_bytes());
    hex::encode(hasher.finalize())
}

/// Compute an Entity VID from (entity_type, canonical_label).
pub fn entity_vid(entity_type: &str, label: &str) -> Vid {
    let canonical = label.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(entity_type.as_bytes());
    hasher.update(b":");
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

/// Compute an Author VID from canonical name.
pub fn author_vid(name: &str) -> Vid {
    let canonical = name.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(b"author:");
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_vid_deterministic() {
        let v1 = paper_vid("1206.6423");
        let v2 = paper_vid("1206.6423");
        assert_eq!(v1, v2);
        assert_eq!(v1.len(), 64);
    }

    #[test]
    fn test_paper_vid_strips_prefix() {
        assert_eq!(paper_vid("arxiv:1206.6423"), paper_vid("1206.6423"));
    }

    #[test]
    fn test_entity_vid_case_insensitive() {
        assert_eq!(entity_vid("Method", "BERT"), entity_vid("Method", "bert"));
    }

    #[test]
    fn test_different_types_different_vid() {
        assert_ne!(
            entity_vid("Method", "Transformer"),
            entity_vid("Model", "Transformer")
        );
    }
}
