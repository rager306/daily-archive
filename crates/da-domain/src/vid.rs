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

/// Compute an immutable article revision expression VID.
///
/// `canonical_id` must be versionless; `revision` selects source bytes but does
/// not change [`paper_vid`]. Provider is normalized so the contract is reusable
/// beyond arXiv.
pub fn paper_revision_vid(
    source_code: &str,
    canonical_id: &str,
    revision: u32,
) -> Result<Vid, String> {
    let source = source_code.trim().to_ascii_lowercase();
    let id = canonical_id.trim();
    if source.is_empty() {
        return Err("source_code must not be empty".into());
    }
    if id.is_empty() {
        return Err("canonical article id must not be empty".into());
    }
    if revision == 0 {
        return Err("article revision must be greater than zero".into());
    }
    if source == "arxiv"
        && crate::load::try_parse_arxiv_locator(id)
            .is_some_and(|(_canonical, requested_revision)| requested_revision.is_some())
    {
        return Err(format!(
            "canonical arXiv id must be versionless, got {canonical_id:?}"
        ));
    }
    Ok(sha256_vid(
        "paper_revision",
        &format!("{source}\n{id}\nv{revision}"),
    ))
}

/// Compute a content-addressed SourceManifestation VID from a SHA256 digest.
pub fn source_manifestation_vid(content_sha256: &str) -> Result<Vid, String> {
    let digest = content_sha256.trim().to_ascii_lowercase();
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("source manifestation sha256 must be 64 hexadecimal characters".into());
    }
    Ok(sha256_vid("source_manifestation", &digest))
}

/// Compute a revision-scoped Section VID from revision identity and ordinal.
pub fn revision_section_vid(revision_vid: &str, ordinal: usize) -> Result<Vid, String> {
    let revision_vid = revision_vid.trim();
    if revision_vid.len() != 64 || !revision_vid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("revision VID must be a 64-character SHA256 VID".into());
    }
    Ok(sha256_vid2(
        "revision_section",
        revision_vid,
        &ordinal.to_string(),
    ))
}

/// Compute a revision-scoped Summary VID from revision and exact source identity.
///
/// `granularity` is part of identity so a revision-level digest and a Section
/// digest cannot collide even if they happen to share source text.
pub fn revision_summary_vid(
    revision_vid: &str,
    granularity: &str,
    source_vid: &str,
) -> Result<Vid, String> {
    let revision_vid = revision_vid.trim();
    let granularity = granularity.trim().to_ascii_lowercase();
    let source_vid = source_vid.trim();
    if revision_vid.len() != 64 || !revision_vid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("revision VID must be a 64-character SHA256 VID".into());
    }
    if source_vid.len() != 64 || !source_vid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("summary source VID must be a 64-character SHA256 VID".into());
    }
    if !matches!(granularity.as_str(), "abstract" | "section") {
        return Err(format!("unsupported summary granularity {granularity:?}"));
    }
    Ok(sha256_vid(
        "revision_summary",
        &format!("{revision_vid}\n{granularity}\n{source_vid}"),
    ))
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

/// Compute a Citation stub VID from a canonical, versionless arXiv identifier.
///
/// Citation identity is graph-global and therefore must not reuse [`paper_vid`]
/// even when both nodes refer to the same scholarly work.
pub fn citation_arxiv_vid(external_id: &str) -> Result<Vid, String> {
    let (canonical_id, _) = crate::load::try_parse_arxiv_locator(external_id)
        .ok_or_else(|| format!("invalid arXiv citation ID: {external_id:?}"))?;
    Ok(sha256_vid("citation_arxiv", &canonical_id))
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
    fn citation_arxiv_vid_is_normalized_and_disjoint_from_paper() {
        let citation = citation_arxiv_vid("arXiv:1206.6423v2").unwrap();
        assert_eq!(citation, citation_arxiv_vid("1206.6423").unwrap());
        assert_ne!(citation, paper_vid("1206.6423"));
        assert_ne!(citation, citation_vid("W12066423").unwrap());
        assert!(citation_arxiv_vid("not-an-arxiv-id").is_err());
    }

    #[test]
    fn revision_vids_are_additive_stable_and_source_scoped() {
        let paper_before = paper_vid("2506.07853");
        let v1 = paper_revision_vid("arxiv", "2506.07853", 1).unwrap();
        let v5 = paper_revision_vid("ARXIV", "2506.07853", 5).unwrap();
        assert_eq!(paper_vid("2506.07853"), paper_before);
        assert_eq!(v1.len(), 64);
        assert_eq!(v5.len(), 64);
        assert_ne!(v1, v5);
        assert_eq!(paper_revision_vid("ARXIV", "2506.07853", 1).unwrap(), v1);
        assert_ne!(paper_revision_vid("crossref", "2506.07853", 1).unwrap(), v1);
    }

    #[test]
    fn revision_vids_reject_ambiguous_or_invalid_identity() {
        assert!(paper_revision_vid("", "2506.07853", 1).is_err());
        assert!(paper_revision_vid("arxiv", "", 1).is_err());
        assert!(paper_revision_vid("arxiv", "2506.07853", 0).is_err());
        assert!(paper_revision_vid("arxiv", "2506.07853v1", 1).is_err());
    }

    #[test]
    fn manifestation_and_revision_section_vids_are_content_scoped() {
        let sha_a = "a".repeat(64);
        let sha_b = "b".repeat(64);
        let manifest_a = source_manifestation_vid(&sha_a).unwrap();
        assert_eq!(
            source_manifestation_vid(&sha_a.to_ascii_uppercase()).unwrap(),
            manifest_a
        );
        assert_ne!(manifest_a, source_manifestation_vid(&sha_b).unwrap());
        assert!(source_manifestation_vid("not-sha").is_err());

        let revision = paper_revision_vid("arxiv", "2506.07853", 1).unwrap();
        assert_ne!(
            revision_section_vid(&revision, 0).unwrap(),
            revision_section_vid(&revision, 1).unwrap()
        );
        assert!(revision_section_vid("bad", 0).is_err());
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

        /// Revision identity is deterministic, revision-separated, and never changes Paper identity.
        #[test]
        fn prop_revision_vid_additive_and_distinct(
            source in arb_key(),
            id in arb_key(),
            revision in 1u32..10_000,
        ) {
            let paper_before = paper_vid(&id);
            let current = paper_revision_vid(&source, &id, revision).unwrap();
            let next = paper_revision_vid(&source, &id, revision + 1).unwrap();
            prop_assert!(is_sha256_hex(&current));
            prop_assert_ne!(current, next);
            prop_assert_eq!(paper_vid(&id), paper_before);
        }

        /// Manifestation identity is case-insensitive on digest representation.
        #[test]
        fn prop_manifestation_vid_sha_case_stable(bytes in any::<[u8; 32]>()) {
            let lower = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            let upper = lower.to_ascii_uppercase();
            prop_assert_eq!(
                source_manifestation_vid(&lower).unwrap(),
                source_manifestation_vid(&upper).unwrap()
            );
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
