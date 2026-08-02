//! Universal VID (Vertex ID) type and SHA256 hashing helper (ADR-038, ADR-050).
//!
//! Every entity gets a deterministic SHA256 ID from its canonical form.
//! This enables O(|K|) hash joins — no false merges across sources.
//!
//! Universal: `sha256_vid(prefix, key)` produces a stable VID from any
//! (namespace, identifier) pair. Project-specific functions like
//! `paper_vid(arxiv_id)`, `author_vid(name)` live in da-domain and
//! call this helper.

use sha2::{Digest, Sha256};

/// A stable vertex ID (SHA256 hex, 64 chars).
pub type Vid = String;

/// Compute a deterministic VID from (namespace_prefix, key).
///
/// # Example
/// ```
/// use kg_ontology::vid::sha256_vid;
/// let v1 = sha256_vid("paper", "1206.6423");
/// let v2 = sha256_vid("paper", "1206.6423");
/// assert_eq!(v1, v2);
/// assert_eq!(v1.len(), 64); // SHA256 hex
/// ```
pub fn sha256_vid(prefix: &str, key: &str) -> Vid {
    let canonical = key.trim();
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update(b":");
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

/// Compute a VID from (prefix, key1, key2) — for composite identifiers.
/// Keys are joined with `|` before hashing.
pub fn sha256_vid2(prefix: &str, key1: &str, key2: &str) -> Vid {
    let canonical1 = key1.trim().to_lowercase();
    let canonical2 = key2.trim();
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update(b":");
    hasher.update(canonical1.as_bytes());
    hasher.update(b"|");
    hasher.update(canonical2.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_vid_deterministic() {
        let v1 = sha256_vid("paper", "1206.6423");
        let v2 = sha256_vid("paper", "1206.6423");
        assert_eq!(v1, v2);
        assert_eq!(v1.len(), 64);
    }

    #[test]
    fn test_sha256_vid_different_prefix() {
        let paper = sha256_vid("paper", "X");
        let author = sha256_vid("author", "X");
        assert_ne!(paper, author);
    }

    #[test]
    fn test_sha256_vid_trims_whitespace() {
        let v1 = sha256_vid("paper", "1206.6423");
        let v2 = sha256_vid("paper", " 1206.6423 ");
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_sha256_vid2_deterministic() {
        let v1 = sha256_vid2("institution", "MIT", "openalex:1234");
        let v2 = sha256_vid2("institution", "MIT", "openalex:1234");
        assert_eq!(v1, v2);
        assert_eq!(v1.len(), 64);
    }

    #[test]
    fn test_sha256_vid2_case_insensitive_key1() {
        let v1 = sha256_vid2("institution", "MIT", "id1");
        let v2 = sha256_vid2("institution", "mit", "id1");
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_sha256_vid2_different_keys() {
        let v1 = sha256_vid2("institution", "MIT", "id1");
        let v2 = sha256_vid2("institution", "MIT", "id2");
        assert_ne!(v1, v2);
    }
}
