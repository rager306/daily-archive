//! Malformed-input and replay-invariant property suite for the conservation
//! engine (M464-z84in2 S03/T03, R106).
//!
//! Proves, over generated bounded inputs, the properties the frozen-fixture
//! contract (`conservation_contract.rs`) cannot cover exhaustively:
//!
//! - **total & non-panicking**: arbitrary bounded bytes and a closed set of
//!   malformed TEI fragments (empty, non-UTF-8, truncated tags/attrs, broken
//!   closing tags, duplicated ids, nested/invalid spans, empty elements,
//!   reordered siblings) never panic the engine or the lifter behind it;
//! - **bounded offsets**: every reported byte range satisfies
//!   `start <= end <= raw_tei.len()` — source observations and delta entries
//!   alike;
//! - **replay invariants**: comparing the same envelope/IR pair twice yields
//!   equal deltas, byte-identical serialization and an equal delta digest,
//!   with entries in the documented total order;
//! - **no silent loss**: deleting a lifted item or reordering section order
//!   can never disappear without a typed reason (`observed_but_missing` /
//!   `count_mismatch` / `reordered`) and always moves the delta digest;
//! - **malformed honesty**: structurally damaged source surfaces
//!   `malformed_source` (verdict precedence) and never fabricates
//!   preservation; verdicts stay consistent with entries;
//! - **redacting**: the serialized delta never embeds the raw TEI text.
//!
//! Offline and pure: no HTTP, no filesystem, no fixtures — inputs are
//! generated in-process; sizes and case counts are bounded for a fast,
//! deterministic PR gate. Transport outcomes (204/503/timeout) and PDF
//! conversion are S04 scope and deliberately not exercised here.

use da_parse::{
    ConservationDelta, ConservationEngine, ConservationFamily, ConservationInventory,
    ConservationReasonCode, ConservationVerdict, GrobidProfile, TeiEnvelope, TeiIdentity,
    TeiLifter, TeiParseIr,
};
use proptest::prelude::*;
use sha2::{Digest, Sha256};

/// Upper bound for generated raw inputs (bytes) — keeps every case fast.
const MAX_RAW_BYTES: usize = 512;

/// Property case budget per `proptest!` block (fast, deterministic PR gate).
const PROP_CASES: u32 = 96;

// ─── Shared harness ─────────────────────────────────────────────────────────

fn prop_config() -> ProptestConfig {
    let mut config = ProptestConfig::with_cases(PROP_CASES);
    config.max_shrink_iters = 1_024;
    config
}

fn identity() -> TeiIdentity {
    TeiIdentity::cached_tei(GrobidProfile::rich())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn lifted(bytes: &[u8]) -> (TeiEnvelope, TeiParseIr) {
    let envelope = TeiEnvelope::new(bytes.to_vec(), identity());
    let ir = TeiLifter::new().lift(&envelope, "prop-paper", "prop-pdf-sha");
    (envelope, ir)
}

/// Offsets, identity binding, verdict⇄entries consistency and total order.
fn assert_report_shape(
    envelope: &TeiEnvelope,
    source: &ConservationInventory,
    delta: &ConservationDelta,
) {
    let raw_len = envelope.raw_tei().len();

    // Identity binding mirrors the exact envelope bytes.
    assert_eq!(delta.identity.raw_tei_len as usize, raw_len);
    assert_eq!(
        delta.identity.raw_tei_sha256,
        sha256_hex(envelope.raw_tei())
    );

    // Bounded offsets everywhere: start <= end <= raw len.
    for observation in source.families.values().flatten() {
        if let Some(range) = observation.byte_range {
            assert!(
                range.is_valid(raw_len),
                "observation span {range:?} escapes raw len {raw_len}"
            );
        }
    }
    for entry in &delta.entries {
        if let Some(range) = entry.byte_range {
            assert!(
                range.is_valid(raw_len),
                "entry span {range:?} escapes raw len {raw_len}"
            );
        }
    }

    // Family counts mirror the two inventories.
    for (family, counts) in &delta.family_counts {
        assert_eq!(counts.source as usize, source.count(*family));
        assert!(counts.malformed <= counts.source);
    }

    // Verdict ⇄ entries consistency. Preserved/intentional-only reports may
    // not carry loss or malformed rows (no fabricated findings either way).
    let has_malformed = delta
        .entries
        .iter()
        .any(|e| e.reason == ConservationReasonCode::MalformedSource);
    let has_loss = delta.entries.iter().any(|e| e.reason.is_loss());
    let all_clean = delta.entries.iter().all(|e| {
        matches!(
            e.reason,
            ConservationReasonCode::Retained | ConservationReasonCode::UnsupportedThisCut
        )
    });
    // A loss report may only carry preserved/intentional rows plus typed loss
    // rows — never undefined or defensive-only reason codes.
    let benign_or_typed_loss = delta.entries.iter().all(|e| {
        matches!(
            e.reason,
            ConservationReasonCode::Retained | ConservationReasonCode::UnsupportedThisCut
        ) || e.reason.is_loss()
    });
    match delta.verdict {
        ConservationVerdict::MalformedSource => assert!(has_malformed),
        ConservationVerdict::LossDetected => {
            assert!(has_loss && !has_malformed && benign_or_typed_loss)
        }
        ConservationVerdict::Conserved => assert!(!has_loss && !has_malformed && all_clean),
    }
    assert_eq!(
        has_malformed,
        delta.verdict == ConservationVerdict::MalformedSource
    );

    // Entries come in the documented total order.
    assert!(delta.entries.windows(2).all(|pair| pair[0] <= pair[1]));
}

fn assert_replay_is_byte_identical(
    engine: &ConservationEngine,
    envelope: &TeiEnvelope,
    ir: &TeiParseIr,
    first: &ConservationDelta,
) {
    let replay = engine.compare(envelope, ir);
    assert_eq!(&replay, first, "replayed delta must be equal");
    let first_json = serde_json::to_vec(first).expect("delta serializes");
    let replay_json = serde_json::to_vec(&replay).expect("replayed delta serializes");
    assert_eq!(
        first_json, replay_json,
        "replayed serialization must be byte-identical"
    );
    assert_eq!(first.delta_digest, replay.delta_digest);
}

// ─── Closed malformed-fragment vocabulary ───────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MalformedKind {
    Empty,
    NonUtf8,
    TruncatedTag,
    TruncatedAttr,
    BrokenClose,
    StrayClose,
    DuplicateId,
    NestedSpan,
    EmptyElement,
    ReorderedSiblings,
}

const ALL_MALFORMED_KINDS: &[MalformedKind] = &[
    MalformedKind::Empty,
    MalformedKind::NonUtf8,
    MalformedKind::TruncatedTag,
    MalformedKind::TruncatedAttr,
    MalformedKind::BrokenClose,
    MalformedKind::StrayClose,
    MalformedKind::DuplicateId,
    MalformedKind::NestedSpan,
    MalformedKind::EmptyElement,
    MalformedKind::ReorderedSiblings,
];

impl MalformedKind {
    /// Kinds whose bytes structurally guarantee at least one malformed
    /// observation — hence a typed `malformed_source` entry and the verdict
    /// precedence (`NonUtf8` carries an unclosed `<head>` tail, so the byte
    /// scanner must record structural damage, not just invalid text).
    fn guarantees_malformed_entry(self) -> bool {
        matches!(
            self,
            Self::NonUtf8
                | Self::TruncatedTag
                | Self::TruncatedAttr
                | Self::BrokenClose
                | Self::StrayClose
        )
    }

    fn fragment(self, seed: usize, filler: &str) -> Vec<u8> {
        match self {
            Self::Empty => Vec::new(),
            Self::NonUtf8 => {
                let mut bytes = b"<TEI><body><head>A\xFF\xFE".to_vec();
                bytes.extend_from_slice(filler.as_bytes());
                // Invalid UTF-8 continuation bytes + unclosed <head>: total,
                // but structurally damaged at EOF.
                bytes.extend_from_slice(b"\xC3(\x80</body>");
                bytes
            }
            Self::TruncatedTag => format!(
                "<TEI><body><div><head>Section {seed}</head><p>x</p></div><head>{filler}"
            )
            .into_bytes(),
            Self::TruncatedAttr => format!(
                "<TEI><body><p>x <ref type=\"bibr\" target=\"#b{seed}{filler}</p></body>"
            )
            .into_bytes(),
            Self::BrokenClose => format!(
                "<TEI><body><div><head>Section {seed}</headx><p>{filler}</p></div></body>"
            )
            .into_bytes(),
            Self::StrayClose => b"<TEI><teiHeader></teiHeader></TEI></text>".to_vec(),
            Self::DuplicateId => format!(
                "<TEI><body><div><head>S{seed}</head></div></body><back>\
                 <biblStruct xml:id=\"b0\"><bibl>first {filler}</bibl></biblStruct>\
                 <biblStruct xml:id=\"b0\"><bibl>second</bibl></biblStruct></back></TEI>"
            )
            .into_bytes(),
            Self::NestedSpan => format!(
                "<TEI><body><div><head>Outer <head>Inner {seed}</head> tail</head>\
                 <p>{filler}</p></div></body></TEI>"
            )
            .into_bytes(),
            Self::EmptyElement => format!(
                "<TEI><teiHeader><fileDesc><titleStmt><title></title></titleStmt></fileDesc>\
                 </teiHeader><body><div><head></head><p>{filler}</p></div><table></table></body></TEI>"
            )
            .into_bytes(),
            Self::ReorderedSiblings => format!(
                "<TEI><body><div><head>Section 1</head><p>{filler}</p></div>\
                 <div><head>Section 0</head><p>later</p></div></body></TEI>"
            )
            .into_bytes(),
        }
    }
}

fn malformed_kind_strategy() -> impl Strategy<Value = MalformedKind> {
    prop_oneof![
        2 => Just(MalformedKind::Empty),
        2 => Just(MalformedKind::NonUtf8),
        2 => Just(MalformedKind::TruncatedTag),
        2 => Just(MalformedKind::TruncatedAttr),
        2 => Just(MalformedKind::BrokenClose),
        2 => Just(MalformedKind::StrayClose),
        1 => Just(MalformedKind::DuplicateId),
        1 => Just(MalformedKind::NestedSpan),
        1 => Just(MalformedKind::EmptyElement),
        1 => Just(MalformedKind::ReorderedSiblings),
    ]
}

// ─── Well-formed generator (deletion / reorder surfaces) ────────────────────

fn wellformed_tei(n_sections: usize, n_citations: usize) -> String {
    let mut tei = String::from(
        "<TEI><teiHeader><fileDesc><titleStmt><title>Property title</title></titleStmt>\
         </fileDesc><profileDesc><abstract>Property abstract.</abstract></profileDesc>\
         </teiHeader><text><body>",
    );
    for i in 0..n_sections {
        tei.push_str(&format!(
            "<div><head>Section {i}</head><p>Body text {i}.</p></div>"
        ));
    }
    tei.push_str("</body></text><back>");
    for i in 0..n_citations {
        tei.push_str(&format!(
            "<biblStruct xml:id=\"b{i}\"><bibl>Citation {i} full text</bibl></biblStruct>"
        ));
    }
    tei.push_str("</back></TEI>");
    tei
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeletionTarget {
    Title,
    Abstract,
    LastSection,
    LastCitation,
}

impl DeletionTarget {
    fn family(self) -> ConservationFamily {
        match self {
            Self::Title => ConservationFamily::Title,
            Self::Abstract => ConservationFamily::Abstract,
            Self::LastSection => ConservationFamily::Section,
            Self::LastCitation => ConservationFamily::CitationEntry,
        }
    }

    fn apply(self, ir: &mut TeiParseIr) {
        match self {
            Self::Title => ir.title.clear(),
            Self::Abstract => ir.abstract_text.clear(),
            Self::LastSection => {
                ir.sections.pop();
            }
            Self::LastCitation => {
                ir.citations.pop();
            }
        }
    }
}

fn deletion_target_strategy() -> impl Strategy<Value = DeletionTarget> {
    prop_oneof![
        1 => Just(DeletionTarget::Title),
        1 => Just(DeletionTarget::Abstract),
        2 => Just(DeletionTarget::LastSection),
        2 => Just(DeletionTarget::LastCitation),
    ]
}

// ─── Properties ─────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(prop_config())]

    /// Arbitrary bounded bytes (including non-UTF-8 and non-XML garbage):
    /// the full realistic surface — envelope → lifter → engine — stays
    /// total, offsets stay bounded, replay stays byte-identical, and the
    /// serialized delta never embeds the raw TEI text.
    #[test]
    fn arbitrary_bounded_bytes_stay_total_bounded_and_replay_stable(raw in proptest::collection::vec(any::<u8>(), 0..=MAX_RAW_BYTES)) {
        let (envelope, ir) = lifted(&raw);
        let engine = ConservationEngine::new();
        let source = engine.source_inventory(&envelope);
        let delta = engine.compare(&envelope, &ir);

        assert_report_shape(&envelope, &source, &delta);
        assert_replay_is_byte_identical(&engine, &envelope, &ir, &delta);

        // Redaction: guard to non-trivial XML-ish payloads so the substring
        // check can never pass vacuously.
        let lossy = String::from_utf8_lossy(&raw).into_owned();
        if lossy.len() >= 16 && lossy.contains('<') {
            let json = String::from_utf8(serde_json::to_vec(&delta).expect("delta serializes"))
                .expect("json is utf-8");
            prop_assert!(!json.contains(&lossy), "serialized delta embeds raw TEI");
        }
    }

    /// The closed malformed-fragment vocabulary stays total and deterministic;
    /// structurally damaging kinds must surface typed `malformed_source`.
    #[test]
    fn malformed_fragments_are_total_and_surface_typed_malformed(
        kind in malformed_kind_strategy(),
        seed in 0usize..64,
        filler in "[a-zA-Z ]{0,16}",
    ) {
        let raw = kind.fragment(seed, &filler);
        prop_assume!(raw.len() <= MAX_RAW_BYTES, "fragments stay bounded");
        let (envelope, ir) = lifted(&raw);
        let engine = ConservationEngine::new();
        let source = engine.source_inventory(&envelope);
        let delta = engine.compare(&envelope, &ir);

        assert_report_shape(&envelope, &source, &delta);
        assert_replay_is_byte_identical(&engine, &envelope, &ir, &delta);

        if kind.guarantees_malformed_entry() {
            prop_assert!(
                delta.entries.iter().any(|e| e.reason == ConservationReasonCode::MalformedSource),
                "malformed fragment {kind:?} must surface a typed malformed_source entry"
            );
            prop_assert_eq!(delta.verdict, ConservationVerdict::MalformedSource);
        }
    }

    /// Deleting a lifted item (title, abstract, last section, last citation)
    /// can never vanish silently: a typed missing/mismatch reason must appear
    /// for the exact family and the delta digest must move.
    #[test]
    fn deletion_cannot_vanish_without_typed_reason(
        n_sections in 1usize..=5,
        n_citations in 1usize..=5,
        target in deletion_target_strategy(),
    ) {
        let tei = wellformed_tei(n_sections, n_citations);
        let (envelope, mut ir) = lifted(tei.as_bytes());
        let engine = ConservationEngine::new();
        let family = target.family();

        let source_count = engine.source_inventory(&envelope).count(family);
        prop_assume!(source_count >= 1, "generator must observe the deleted family");
        let before = engine.compare(&envelope, &ir);

        target.apply(&mut ir);
        let after = engine.compare(&envelope, &ir);

        prop_assert!(
            after.delta_digest != before.delta_digest,
            "deletion must move the delta digest"
        );
        prop_assert!(
            after.entries.iter().any(|e| e.family == family
                && matches!(
                    e.reason,
                    ConservationReasonCode::ObservedButMissing | ConservationReasonCode::CountMismatch
                )),
            "deletion of {family:?} must leave a typed missing/mismatch reason"
        );
        prop_assert!(after.entries.iter().any(|e| e.reason.is_loss()));
        assert_report_shape(&envelope, &engine.source_inventory(&envelope), &after);
    }

    /// Reordering adjacent sections can never vanish silently: the digest-
    /// aligned comparator must raise the typed `reordered` structure finding
    /// with the stable structure key, and the digest must move.
    #[test]
    fn section_reorder_cannot_vanish_without_typed_reason(
        n_sections in 2usize..=6,
        swap_at in 0usize..5,
    ) {
        let tei = wellformed_tei(n_sections, 2);
        let (envelope, mut ir) = lifted(tei.as_bytes());
        prop_assume!(ir.sections.len() >= 2, "generator must yield sections");
        let index = swap_at % (ir.sections.len() - 1);
        prop_assume!(
            ir.sections[index].title != ir.sections[index + 1].title,
            "adjacent titles must differ to make the reorder observable"
        );

        let engine = ConservationEngine::new();
        let before = engine.compare(&envelope, &ir);
        ir.sections.swap(index, index + 1);
        let after = engine.compare(&envelope, &ir);

        prop_assert!(
            after.entries.iter().any(|e| e.family == ConservationFamily::Section
                && e.reason == ConservationReasonCode::Reordered
                && e.structure_key.as_deref() == Some("section_document_order")),
            "adjacent section reorder must raise the typed reordered structure finding"
        );
        prop_assert!(
            after.delta_digest != before.delta_digest,
            "reorder must move the delta digest"
        );
        assert_report_shape(&envelope, &engine.source_inventory(&envelope), &after);
        assert_replay_is_byte_identical(&engine, &envelope, &ir, &after);
    }

    /// Redaction property over lifted text: family/title/body text from the
    /// source never reaches the serialized delta (only digests/counts).
    #[test]
    fn serialized_delta_never_embeds_lifted_text(seed_text in "[a-zA-Z0-9]+ [a-zA-Z0-9]+ [a-zA-Z0-9]+") {
        let tei = format!(
            "<TEI><teiHeader><fileDesc><titleStmt><title>{seed_text}</title></titleStmt>\
             </fileDesc><profileDesc><abstract>About {seed_text}.</abstract></profileDesc>\
             </teiHeader><text><body><div><head>{seed_text}</head>\
             <p>Body mentions {seed_text}.</p></div></body></text></TEI>"
        );
        let (envelope, ir) = lifted(tei.as_bytes());
        let delta = ConservationEngine::new().compare(&envelope, &ir);
        let json = String::from_utf8(serde_json::to_vec(&delta).expect("delta serializes"))
            .expect("json is utf-8");
        prop_assert!(!json.contains(&seed_text), "delta embeds lifted text");
        prop_assert!(!json.contains(&tei), "delta embeds raw TEI");
    }
}

// ─── Deterministic coverage over the full malformed vocabulary ──────────────

/// Walk every malformed kind exactly once so per-kind guarantees hold
/// deterministically, independent of the random case distribution above.
#[test]
fn every_malformed_kind_is_deterministic_and_bounded() {
    for kind in ALL_MALFORMED_KINDS {
        let raw = kind.fragment(7, "filler text");
        let (envelope, ir) = lifted(&raw);
        let engine = ConservationEngine::new();
        let source = engine.source_inventory(&envelope);
        let delta = engine.compare(&envelope, &ir);

        assert_report_shape(&envelope, &source, &delta);
        assert_replay_is_byte_identical(&engine, &envelope, &ir, &delta);

        if kind.guarantees_malformed_entry() {
            assert!(
                delta
                    .entries
                    .iter()
                    .any(|e| e.reason == ConservationReasonCode::MalformedSource),
                "{kind:?} must surface a typed malformed_source entry"
            );
            assert_eq!(delta.verdict, ConservationVerdict::MalformedSource);
        }
    }
}
