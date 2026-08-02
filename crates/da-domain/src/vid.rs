//! SHA256 stable VIDs (Identifier-Preserving Joins, ADR-038 §6 P1).
//!
//! Re-exports universal Vid type and sha256_vid helper from
//! kg_ontology::vid (M289-u4fzx5/S04). Project-specific functions
//! (paper_vid, author_vid, entity_vid, etc.) stay here and call
//! the universal helper.

// Re-export universal types from kg-ontology (ADR-050).
pub use kg_ontology::vid::{Vid, sha256_vid, sha256_vid2};

/// Compute a Paper VID from arxiv_id.
pub fn paper_vid(arxiv_id: &str) -> Vid {
    let id = arxiv_id.trim().replace("arxiv:", "");
    sha256_vid("paper", &id)
}

/// Compute an Entity VID from (entity_type, canonical_label).
pub fn entity_vid(entity_type: &str, label: &str) -> Vid {
    let canonical = label.trim().to_lowercase();
    sha256_vid(entity_type, &canonical)
}

/// Compute an Author VID from canonical name.
pub fn author_vid(name: &str) -> Vid {
    let canonical = name.trim().to_lowercase();
    sha256_vid("author", &canonical)
}

/// Compute a Reference VID from raw citation text.
pub fn reference_vid(raw_text: &str) -> Vid {
    let canonical = raw_text.trim().to_lowercase();
    sha256_vid("reference", &canonical)
}

/// Compute an Institution VID from (display_name, openalex_id).
pub fn institution_vid(display_name: &str, openalex_id: &str) -> Vid {
    let canonical_name = display_name.trim().to_lowercase();
    let canonical_id = openalex_id.trim();
    sha256_vid2("institution", &canonical_name, canonical_id)
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
