#![cfg(feature = "pdf-layout")]

use da_adapters::{
    LayoutDonorError, LayoutDonorLimits, parse_pdfalto_xml_shadow, reorder_shadow_with_xy_cut,
};

const XML: &[u8] = br#"<doc><page width="612" height="792"><text x="10" y="20" width="30" height="8">Heading</text><text x="10" y="40" width="30" height="8">References</text></page></doc>"#;

fn digest(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

#[test]
fn shadow_rows_are_lossless_and_bind_to_neutral_token_identity() {
    let observation =
        parse_pdfalto_xml_shadow(XML, b"pdf", "paper", LayoutDonorLimits::default()).unwrap();
    assert_eq!(observation.document.document_id, "paper");
    assert_eq!(observation.document.pages[0].tokens.len(), 2);
    assert_eq!(observation.lexemes.len(), 2);

    for (token, lexeme) in observation.document.pages[0]
        .tokens
        .iter()
        .zip(&observation.lexemes)
    {
        assert_eq!(lexeme.document_id, "paper");
        assert_eq!(lexeme.provider_id, "pdfalto");
        assert_eq!(lexeme.page_ordinal, token.page_ordinal);
        assert_eq!(lexeme.source_ordinal, token.source_ordinal);
        assert_eq!(lexeme.text_digest, token.text_digest);
        assert_eq!(lexeme.text_digest, digest(lexeme.text()));
    }
}

#[test]
fn xy_cut_reorders_rows_without_rewriting_source_identity_or_text() {
    let before =
        parse_pdfalto_xml_shadow(XML, b"pdf", "paper", LayoutDonorLimits::default()).unwrap();
    let expected = before
        .lexemes
        .iter()
        .map(|row| {
            (
                row.source_ordinal,
                row.text().to_owned(),
                row.text_digest.clone(),
            )
        })
        .collect::<Vec<_>>();
    let after = reorder_shadow_with_xy_cut(before).unwrap();

    assert_eq!(after.lexemes.len(), expected.len());
    for row in &after.lexemes {
        let original = expected
            .iter()
            .find(|(ordinal, _, _)| *ordinal == row.source_ordinal)
            .unwrap();
        assert_eq!(row.text(), original.1);
        assert_eq!(row.text_digest, original.2);
    }
    assert_eq!(
        after
            .document
            .pages
            .iter()
            .flat_map(|page| page.tokens.iter())
            .map(|token| token.source_ordinal)
            .collect::<Vec<_>>(),
        after
            .lexemes
            .iter()
            .map(|row| row.source_ordinal)
            .collect::<Vec<_>>()
    );
}

#[test]
fn shadow_lexemes_are_bounded_before_becoming_an_observation() {
    let result = parse_pdfalto_xml_shadow(
        XML,
        b"pdf",
        "paper",
        LayoutDonorLimits {
            max_output_bytes: 3,
            ..Default::default()
        },
    );
    assert!(matches!(
        result,
        Err(LayoutDonorError::Oversize {
            resource: "lexeme_bytes",
            limit: 3
        })
    ));
}

#[test]
fn shadow_rejects_duplicate_or_missing_rows_before_xy_cut() {
    let mut observation =
        parse_pdfalto_xml_shadow(XML, b"pdf", "paper", LayoutDonorLimits::default()).unwrap();
    observation.lexemes.push(observation.lexemes[0].clone());
    assert!(matches!(
        reorder_shadow_with_xy_cut(observation),
        Err(LayoutDonorError::Malformed { detail }) if detail.contains("duplicate or missing")
    ));
}

#[test]
fn shadow_rejects_digest_mismatch_before_xy_cut() {
    let mut observation =
        parse_pdfalto_xml_shadow(XML, b"pdf", "paper", LayoutDonorLimits::default()).unwrap();
    observation.lexemes[0].text_digest = "0".repeat(64);
    assert!(matches!(
        reorder_shadow_with_xy_cut(observation),
        Err(LayoutDonorError::Malformed { detail }) if detail.contains("digest")
    ));
}

#[test]
fn debug_redacts_lexeme_payload() {
    let observation =
        parse_pdfalto_xml_shadow(XML, b"pdf", "paper", LayoutDonorLimits::default()).unwrap();
    let debug = format!("{observation:?}");
    assert!(debug.contains("lexeme_count"));
    assert!(!debug.contains("Heading"));
    assert!(!debug.contains("References"));
}
