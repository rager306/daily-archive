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

    #[test]
    fn test_reference_vid_deterministic() {
        let vid1 = reference_vid("Smith et al. (2023). Deep Learning.");
        let vid2 = reference_vid("Smith et al. (2023). Deep Learning.");
        assert_eq!(vid1, vid2, "same raw_text must produce same VID");
    }

    #[test]
    fn test_reference_vid_different_text() {
        let vid1 = reference_vid("Paper A");
        let vid2 = reference_vid("Paper B");
        assert_ne!(vid1, vid2);
    }
}

/// Compute a Reference VID from raw citation text.
/// Uses SHA256 of normalized raw_text for idempotent dedup.
pub fn reference_vid(raw_text: &str) -> Vid {
    let canonical = raw_text.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(b"reference:");
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

/// Compute an Institution VID from (display_name, openalex_id).
/// Uses both fields so two institutions with the same name from
/// different OpenAlex records get distinct VIDs.
pub fn institution_vid(display_name: &str, openalex_id: &str) -> Vid {
    let canonical_name = display_name.trim().to_lowercase();
    let canonical_id = openalex_id.trim();
    let mut hasher = Sha256::new();
    hasher.update(b"institution:");
    hasher.update(canonical_name.as_bytes());
    hasher.update(b"|");
    hasher.update(canonical_id.as_bytes());
    hex::encode(hasher.finalize())
}
