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

/// Canonical OpenAlex work ID (`W` plus ASCII digits) from a full URL or bare ID.
pub fn canonical_openalex_work_id(external_id: &str) -> Result<String, String> {
    let trimmed = external_id.trim().trim_end_matches('/');
    let candidate = trimmed
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if candidate.len() < 2
        || !candidate.starts_with('W')
        || !candidate[1..].bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("invalid OpenAlex work ID: {external_id:?}"));
    }
    Ok(candidate)
}

/// Compute a Citation stub VID from a canonical OpenAlex work ID.
pub fn citation_vid(external_id: &str) -> Result<Vid, String> {
    canonical_openalex_work_id(external_id).map(|canonical| sha256_vid("citation", &canonical))
}

/// Compute a replay-safe EvidenceBundle VID from document, section and members.
/// Member VIDs are sorted so extractor output ordering cannot change identity.
pub fn evidence_bundle_vid(paper_id: &str, section_title: &str, member_vids: &[Vid]) -> Vid {
    let mut members = member_vids.to_vec();
    members.sort();
    let canonical = format!(
        "{}\n{}\n{}",
        paper_id.trim().to_lowercase(),
        section_title.trim().to_lowercase(),
        members.join("\n")
    );
    sha256_vid("evidence_bundle", &canonical)
}

/// Compute a Claim VID from the EvidenceBundle that grounds it.
pub fn evidence_claim_vid(bundle_vid: &str) -> Vid {
    sha256_vid("claim", bundle_vid)
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
    fn citation_vid_canonicalizes_openalex_urls_and_rejects_invalid_ids() {
        assert_eq!(
            citation_vid("https://openalex.org/W123").unwrap(),
            citation_vid("w123/").unwrap()
        );
        assert_eq!(
            canonical_openalex_work_id("openalex.org/W123").unwrap(),
            "W123"
        );
        for invalid in ["", "https://openalex.org/", "123", "WABC"] {
            assert!(citation_vid(invalid).is_err());
        }
    }

    #[test]
    fn test_reference_vid_deterministic() {
        let vid1 = reference_vid("Smith et al. (2023). Deep Learning.");
        let vid2 = reference_vid("Smith et al. (2023). Deep Learning.");
        assert_eq!(vid1, vid2, "same raw_text must produce same VID");
    }

    #[test]
    fn evidence_bundle_identity_is_order_independent_and_section_safe() {
        let a = vec!["entity-a".into(), "entity-b".into()];
        let b = vec!["entity-b".into(), "entity-a".into()];
        assert_eq!(
            evidence_bundle_vid("p1", "Methods", &a),
            evidence_bundle_vid("p1", "Methods", &b)
        );
        assert_ne!(
            evidence_bundle_vid("p1", "Methods", &a),
            evidence_bundle_vid("p1", "Results", &a)
        );
    }

    #[test]
    fn evidence_claim_identity_is_bound_to_bundle() {
        assert_eq!(
            evidence_claim_vid("bundle-a"),
            evidence_claim_vid("bundle-a")
        );
        assert_ne!(
            evidence_claim_vid("bundle-a"),
            evidence_claim_vid("bundle-b")
        );
    }

    #[test]
    fn test_reference_vid_different_text() {
        let vid1 = reference_vid("Paper A");
        let vid2 = reference_vid("Paper B");
        assert_ne!(vid1, vid2);
    }
}

// ─── ADR-051 P1: Property-based tests (M355) ───

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    /// Non-empty trimmed-ish keys (avoid pure whitespace collapse edge cases).
    fn arb_key() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9._-]{1,64}".prop_map(|s| s)
    }

    fn is_sha256_hex(s: &str) -> bool {
        s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    proptest! {
        /// Property 1: paper_vid is always 64 hex chars and deterministic.
        #[test]
        fn prop_paper_vid_hex_deterministic(id in arb_key()) {
            let v1 = paper_vid(&id);
            let v2 = paper_vid(&id);
            prop_assert_eq!(&v1, &v2);
            prop_assert!(is_sha256_hex(&v1), "vid={v1}");
        }

        /// Property 2: paper_vid strips arxiv: prefix (case-sensitive strip of exact prefix).
        #[test]
        fn prop_paper_vid_strips_arxiv_prefix(id in arb_key()) {
            let bare = paper_vid(&id);
            let with = paper_vid(&format!("arxiv:{id}"));
            prop_assert_eq!(bare, with);
        }

        /// Property 3: entity_vid is case-insensitive on label and hex.
        #[test]
        fn prop_entity_vid_case_insensitive_label(
            et in arb_key(),
            label in arb_key(),
        ) {
            let lower = entity_vid(&et, &label.to_lowercase());
            let upper = entity_vid(&et, &label.to_uppercase());
            prop_assert_eq!(lower, upper);
            prop_assert!(is_sha256_hex(&entity_vid(&et, &label)));
        }

        /// Property 4: different entity_type ⇒ different vid for same label.
        #[test]
        fn prop_entity_type_separates(label in arb_key()) {
            let a = entity_vid("Method", &label);
            let b = entity_vid("Model", &label);
            prop_assert_ne!(a, b);
        }

        /// Property 5: author_vid case-insensitive and hex.
        #[test]
        fn prop_author_vid_case_insensitive(name in arb_key()) {
            let a = author_vid(&name.to_lowercase());
            let b = author_vid(&name.to_uppercase());
            prop_assert_eq!(a, b);
            prop_assert!(is_sha256_hex(&author_vid(&name)));
        }

        /// Property 6: reference_vid deterministic and hex.
        #[test]
        fn prop_reference_vid_deterministic(text in arb_key()) {
            let v1 = reference_vid(&text);
            let v2 = reference_vid(&text);
            prop_assert_eq!(v1, v2);
            prop_assert!(is_sha256_hex(&reference_vid(&text)));
        }

        /// Property 7: institution_vid hex; display_name case-insensitive.
        #[test]
        fn prop_institution_vid_case_insensitive_name(
            name in arb_key(),
            oid in arb_key(),
        ) {
            let a = institution_vid(&name.to_lowercase(), &oid);
            let b = institution_vid(&name.to_uppercase(), &oid);
            prop_assert_eq!(a, b);
            prop_assert!(is_sha256_hex(&institution_vid(&name, &oid)));
        }
    }
}
