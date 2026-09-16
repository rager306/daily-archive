//! Independent same-n oracle for the native GROBID body/fulltext lane (S05/T05).
//!
//! The expectation side of this file is a lexical scan over the preserved TEI
//! bytes plus test-local constants. It never reaches for the Java lifter, the
//! native composition entry points, the reading-order glue, the row builder or
//! the sequence decoder: those are the SUT lane, never the receipt. The two
//! lanes are separated by explicit source markers and the separation is itself
//! asserted below.
//!
//! # What the scan is, and why it is lexical
//!
//! The body region is walked once, character by character. A token is a maximal
//! whitespace-delimited run of text; a *block* is one instance of a block
//! element from the closed set below, and every token inherits the label of the
//! innermost open block element. Nested block elements therefore split the
//! enclosing element into several blocks (a `<formula>` inside a `<p>` yields a
//! paragraph block, the equation block, and the paragraph continuation) — Java
//! emits one label per token, and the nested element's tokens are labelled by
//! the nested element. `ref` is the one marker element: it always interrupts and
//! its label comes from its own `type` attribute.
//!
//! # Denominators, and why each is its own population
//!
//! * instrument tokens / blocks — 33 680 / 1 114, 18 133 / 483, 23 556 / 778.
//!   These are the body-family denominators the ledger's `pending_body_counts`
//!   names `pending_independent_oracle`; this file is that oracle.
//! * the three body families partition the labels exactly once:
//!   `body_blocks` 30 678 / 16 455 / 19 892, `body_markers` 486 / 364 / 675,
//!   `visual_content` 2 516 / 1 314 / 2 989. Family totals conserve the whole.
//! * base-name runs over the corpus label sequence — 458 / 334 / 521 — and the
//!   per-family split of the same rule (see [`FIXTURE_FAMILY_RUNS`]).
//! * body-scoped `<ref type="bibr">` markers — 123 / 95 / 158 = **376** — is a
//!   *different* population from the bibliography oracle's whole-document
//!   anchor denominator **380** (`native_bibliography_same_n.rs`). The two are
//!   never substituted for one another.
//!
//! # Named gaps — never closed by invention, never pooled
//!
//! * `<item>` and `<equation_marker>` have no lexical source in these three
//!   fixtures: `count(item) = 0` and no `<ref type="formula">` occurs, so two of
//!   the twelve ledger labels carry a zero denominator here. They are pinned as
//!   zeros rather than dropped, so the dictionary stays whole.
//! * the `<ref type="bibr"/>` self-closing spelling has no lexical source in
//!   these fixtures either (all 154 / 122 / 199 `<ref>` elements are paired and
//!   non-empty), so the marker denominators are not anchor counts. The gap stays
//!   open.
//! * `body_markers` is not the bibliography lane and never compensates it.
//!
//! # Honest offline limits (507-05 research)
//!
//! * There is no preserved ALTO/layout beside these fixtures and the TEI carries
//!   no `pb` element and no `div type` attribute, so this oracle is
//!   TEI-to-structure, not PDF-to-structure. Every page is therefore declared
//!   page 1 and the cross-page continuation behavior is proven over declared
//!   layout facts in `native_fulltext_reading_order.rs`, never here.
//! * The real GROBID fulltext model bytes are not in the tree (D358), so no
//!   label sequence can be produced from the real model here. The decode is
//!   driven over the *corpus-derived* label sequence with an in-file model whose
//!   observations are the dictionary spellings themselves, so the label each row
//!   must receive is fixed by construction and the comparison is on the tiling
//!   the runner produces. The real model is probed only opt-in
//!   (`crates/da-adapters/tests/native_fulltext_real_model.rs`).
//! * The layout facts handed to the glue (page height, block ordinates, font
//!   identity) are declared uniformly by this file, because the TEI does not
//!   carry them. They cannot make a reading order right or wrong; they only fix
//!   the identity digest, which is compared for determinism, not for a value
//!   derived from a real layout.

use da_parse::{
    FulltextCalloutType, NATIVE_FULLTEXT_FEATURE_COLUMNS, NATIVE_FULLTEXT_LABELS,
    NativeFulltextDecodeOutcome, NativeFulltextPageToken, NativeFulltextTokenText,
    decode_fulltext_sequence, glue_fulltext_reading_order,
};
use da_ports::scalar_crf::{CrfModel, CrfPattern};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

// ---- independent expectation builder (no SUT lane below this marker) ----

/// Preserved fixtures, in `FIXTURES` order.
pub const FIXTURES: [&str; 3] = [
    "hub_2507.19457.tei.xml",
    "rlm_2512.24601.tei.xml",
    "reca_2605.26525v1.tei.xml",
];

/// SHA-256 of the tracked bytes, copied from `fixtures/preserved/provenance.yaml`.
pub const FIXTURE_SHA256: [&str; 3] = [
    "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
    "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
    "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
];

/// The twelve ledger base labels, in `fulltext_labels.labels` order.
///
/// Restated on the expectation side so the oracle never reads the SUT table; the
/// data-contract test below re-derives the same list from
/// `data/parse/native_body_accounting.yaml`.
pub const BASE_LABELS: [&str; 12] = [
    "<paragraph>",
    "<item>",
    "<section>",
    "<citation_marker>",
    "<figure_marker>",
    "<table_marker>",
    "<equation_marker>",
    "<figure>",
    "<table>",
    "<equation>",
    "<equation_label>",
    "<other>",
];

/// The three body families, in the ledger's `families` key order.
pub const FAMILY_ORDER: [&str; 3] = ["body_blocks", "body_markers", "visual_content"];

/// Closed block-element set and the ledger label each element carries.
const BLOCK_ELEMENTS: [(&str, &str); 8] = [
    ("p", "<paragraph>"),
    ("head", "<section>"),
    ("item", "<item>"),
    ("label", "<equation_label>"),
    ("figure", "<figure>"),
    ("table", "<table>"),
    ("formula", "<equation>"),
    ("note", "<other>"),
];

/// `ref` marker kinds, by `type` attribute. An unknown or absent `type` is
/// `other`, which is the ledger's fail-closed bucket rather than a silent drop.
const REF_TYPES: [(&str, &str); 4] = [
    ("bibr", "<citation_marker>"),
    ("figure", "<figure_marker>"),
    ("table", "<table_marker>"),
    ("formula", "<equation_marker>"),
];
const REF_DEFAULT: &str = "<other>";

/// `(blocks, tokens)` per base label, in [`BASE_LABELS`] order, per fixture.
pub const FIXTURE_LABEL_COUNTS: [[(usize, usize); 12]; 3] = [
    [
        (809, 30342),
        (0, 0),
        (85, 336),
        (123, 455),
        (24, 24),
        (5, 5),
        (0, 0),
        (23, 2120),
        (3, 67),
        (24, 313),
        (16, 16),
        (2, 2),
    ],
    [
        (267, 16244),
        (0, 0),
        (63, 211),
        (95, 337),
        (16, 16),
        (9, 9),
        (0, 0),
        (15, 575),
        (2, 662),
        (2, 65),
        (12, 12),
        (2, 2),
    ],
    [
        (338, 19549),
        (0, 0),
        (116, 343),
        (158, 634),
        (30, 30),
        (11, 11),
        (0, 0),
        (57, 2075),
        (3, 410),
        (12, 451),
        (53, 53),
        (0, 0),
    ],
];

/// Total admitted blocks per fixture.
pub const FIXTURE_BLOCKS: [usize; 3] = [1114, 483, 778];

/// Total admitted tokens per fixture.
pub const FIXTURE_TOKENS: [usize; 3] = [33680, 18133, 23556];

/// Maximal base-name runs over each fixture's label sequence.
pub const FIXTURE_RUNS: [usize; 3] = [458, 334, 521];

/// Tokens per fixture per family, in [`FAMILY_ORDER`].
pub const FIXTURE_FAMILY_TOKENS: [[usize; 3]; 3] =
    [[30678, 486, 2516], [16455, 364, 1314], [19892, 675, 2989]];

/// Blocks per fixture per family, in [`FAMILY_ORDER`].
pub const FIXTURE_FAMILY_BLOCKS: [[usize; 3]; 3] =
    [[894, 154, 66], [330, 122, 31], [454, 199, 125]];

/// Base-name runs per fixture per family, in [`FAMILY_ORDER`].
pub const FIXTURE_FAMILY_RUNS: [[usize; 3]; 3] = [[132, 37, 36], [105, 36, 27], [125, 32, 111]];

/// Corpus totals: three fixtures, one population each.
pub const TOTAL_BLOCKS: usize = 2375;
pub const TOTAL_TOKENS: usize = 75369;

/// Body-scoped `<ref type="bibr">` markers — this lane's callout denominator.
pub const BODY_BIBR_MARKERS: [usize; 3] = [123, 95, 158];
pub const TOTAL_BODY_BIBR_MARKERS: usize = 376;

/// The bibliography oracle's *whole-document* anchor denominator, restated from
/// `native_bibliography_same_n.rs` (`bibr_total` 123 / 99 / 158). It is a
/// different population from [`TOTAL_BODY_BIBR_MARKERS`] and the difference is
/// pinned so the two can never be quietly pooled.
pub const BIBLIOGRAPHY_ANCHOR_TOTAL: usize = 380;

/// Recomputable digest of the canonical receipt text. Regenerate only when the
/// tracked fixtures themselves change; the canonicalization is documented in
/// [`canonical_receipts`].
pub const EXPECTED_RECEIPT_DIGEST: &str =
    "285a5ddd0c49a51616de2f40a48b6c9d18ce0a71e8685a73d3c15ace11069ec9";

/// One admitted block: its label and its whitespace-split tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanBlock {
    pub label: &'static str,
    pub tokens: Vec<String>,
}

/// Independent lexical inventory of the body region.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FulltextScan {
    /// The scanned body bytes, kept so the canonical receipt can recompute the
    /// same slice the counts came from.
    pub body: String,
    pub blocks: Vec<ScanBlock>,
}

impl FulltextScan {
    pub fn tokens(&self) -> usize {
        self.blocks.iter().map(|block| block.tokens.len()).sum()
    }
}

/// Slice `<body>`'s content, from the end of its opening tag to `</body>`.
///
/// A preserved TEI without a body element or without its closing tag is refused
/// rather than silently scanned as if the whole document were body.
pub fn body_element_slice(xml: &str) -> &str {
    let start = xml.find("<body").expect("preserved TEI has a body element");
    let end = xml
        .find("</body>")
        .expect("preserved TEI closes its body element");
    assert!(start < end, "body opening tag precedes its closing tag");
    let after_name = &xml[start + "<body".len()..end];
    let gt = after_name
        .find('>')
        .expect("body opening tag is closed before </body>");
    &after_name[gt + 1..]
}

/// The ledger family a label belongs to.
pub fn family_of(label: &str) -> &'static str {
    match label {
        "<paragraph>" | "<item>" | "<section>" => "body_blocks",
        "<citation_marker>" | "<figure_marker>" | "<table_marker>" | "<equation_marker>"
        | "<other>" => "body_markers",
        "<figure>" | "<table>" | "<equation>" | "<equation_label>" => "visual_content",
        other => panic!("label outside the ledger partition: {other}"),
    }
}

/// Parse a tag body into `(is_closing, name)`.
///
/// The name is the longest leading run of name characters, so `<titleStmt>` is
/// never read as `<title>`.
fn parse_tag(tag: &str) -> Option<(bool, &str)> {
    let closing = tag.starts_with('/');
    let rest = if closing { &tag[1..] } else { tag };
    let name_end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-'))
        .unwrap_or(rest.len());
    let name = &rest[..name_end];
    if !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    Some((closing, name))
}

/// The label a `ref` opening tag carries, from its own `type` attribute.
fn ref_label(tag: &str) -> &'static str {
    let needle = "type=\"";
    let Some(at) = tag.find(needle).map(|at| at + needle.len()) else {
        return REF_DEFAULT;
    };
    let rest = &tag[at..];
    let Some(end) = rest.find('"') else {
        return REF_DEFAULT;
    };
    let value = &rest[..end];
    REF_TYPES
        .iter()
        .find(|(kind, _)| *kind == value)
        .map(|(_, label)| *label)
        .unwrap_or(REF_DEFAULT)
}

/// Scan the preserved body into ordered blocks.
///
/// Tokens outside any block element are not body tokens and carry no label, so
/// they are not admitted. `ref` always interrupts; other nested block elements
/// take over the label until they close.
pub fn scan_fulltext_body(xml: &str) -> FulltextScan {
    let body = body_element_slice(xml).to_owned();
    let mut blocks: Vec<ScanBlock> = Vec::new();
    let mut stack: Vec<(&'static str, String)> = Vec::new();
    let mut buffer = String::new();

    fn flush(blocks: &mut Vec<ScanBlock>, buffer: &mut String, stack: &[(&'static str, String)]) {
        if buffer.is_empty() {
            return;
        }
        let text = std::mem::take(buffer);
        let Some((label, _)) = stack.last() else {
            return;
        };
        let tokens: Vec<String> = text.split_whitespace().map(str::to_owned).collect();
        if !tokens.is_empty() {
            blocks.push(ScanBlock { label, tokens });
        }
    }

    let mut pos = 0usize;
    while pos < body.len() {
        let c = body[pos..]
            .chars()
            .next()
            .expect("a valid character boundary");
        if c == '<' {
            let Some(relative) = body[pos..].find('>') else {
                break;
            };
            let end = pos + relative;
            let tag = &body[pos + 1..end];
            match parse_tag(tag) {
                Some((true, name)) => {
                    if let Some(index) = stack.iter().rposition(|(_, open)| open == name) {
                        flush(&mut blocks, &mut buffer, &stack);
                        stack.truncate(index);
                    }
                }
                Some((false, name)) if name == "ref" => {
                    if !tag.ends_with('/') {
                        flush(&mut blocks, &mut buffer, &stack);
                        stack.push((ref_label(tag), name.to_owned()));
                    }
                }
                Some((false, name)) => {
                    let known = BLOCK_ELEMENTS
                        .iter()
                        .find(|(element, _)| *element == name)
                        .map(|(_, label)| *label);
                    if let Some(label) = known
                        && !tag.ends_with('/')
                    {
                        flush(&mut blocks, &mut buffer, &stack);
                        stack.push((label, name.to_owned()));
                    }
                }
                None => {}
            }
            pos = end + 1;
        } else {
            buffer.push(c);
            pos += c.len_utf8();
        }
    }
    flush(&mut blocks, &mut buffer, &stack);

    FulltextScan { body, blocks }
}

/// The label sequence one block-list produces, spelled the way the decode probe
/// feeds it: the first row of every block uses the `I-` spelling and the
/// remaining rows the bare spelling.
///
/// That spelling shape is the one observed on the real 0.9.0 header model
/// (S03/T06: one `I-<author>` opening followed by bare rows) and it is the shape
/// the mirror-BIO grouping rule mis-tiles, so the probe exercises the trap
/// rather than a spelling that both rules agree on.
pub fn spelling_labels(blocks: &[ScanBlock]) -> Vec<String> {
    let mut labels = Vec::new();
    for block in blocks {
        for (index, _) in block.tokens.iter().enumerate() {
            labels.push(if index == 0 {
                format!("I-{}", block.label)
            } else {
                block.label.to_owned()
            });
        }
    }
    labels
}

/// Strip the `I-` spelling prefix.
pub fn base_of(label: &str) -> &str {
    label.strip_prefix("I-").unwrap_or(label)
}

/// The published `writeField` grouping rule: a run continues while the *base*
/// name is unchanged, and any base change opens the next run. Both spellings are
/// spellings of one field, so the prefix is never a boundary.
pub fn maximal_base_runs(labels: &[String]) -> Vec<(String, usize, usize)> {
    let mut runs: Vec<(String, usize, usize)> = Vec::new();
    for (index, label) in labels.iter().enumerate() {
        let base = base_of(label);
        match runs.last_mut() {
            Some(run) if run.0 == base => run.2 = index + 1,
            _ => runs.push((base.to_owned(), index, index + 1)),
        }
    }
    runs
}

/// The mirror-BIO rule S03 disproved: a bare label always opens a run, and an
/// `I-` label continues only an already-open run of its base.
///
/// Kept here as the *rejected* hypothesis, so the oracle can assert that its own
/// expectation is the discriminating one rather than a restatement.
pub fn mirror_bio_runs(labels: &[String]) -> Vec<(String, usize, usize)> {
    let mut runs: Vec<(String, usize, usize)> = Vec::new();
    for (index, label) in labels.iter().enumerate() {
        let prefixed = label.starts_with("I-");
        let base = base_of(label);
        if !prefixed {
            runs.push((base.to_owned(), index, index + 1));
            continue;
        }
        match runs.last_mut() {
            Some(run) if run.0 == base && run.2 == index => run.2 = index + 1,
            _ => runs.push((base.to_owned(), index, index + 1)),
        }
    }
    runs
}

/// Canonical text for the scanned receipts.
///
/// Published canonicalization, in `FIXTURES` order, one record per fixture:
///
/// ```text
/// fixture=<file name>
/// sha256=<pinned digest>
/// blocks=<n>
/// tokens=<n>
/// label=<base label>:<blocks>/<tokens>      (twelve lines, BASE_LABELS order)
/// family=<family>:<tokens>/<blocks>         (three lines, FAMILY_ORDER)
/// runs=<n>
/// ---
/// ```
///
/// The two framings of the run denominator — over the per-block labels and over
/// the spelled label sequence — must agree, which is asserted by the receipt
/// test rather than assumed here.
pub fn canonical_receipts(scans: &[FulltextScan]) -> String {
    let mut out = String::new();
    for (index, scan) in scans.iter().enumerate() {
        let mut per_label = BTreeMap::new();
        let mut per_family_blocks = BTreeMap::new();
        let mut per_family_tokens = BTreeMap::new();
        for block in &scan.blocks {
            let entry = per_label.entry(block.label).or_insert((0usize, 0usize));
            entry.0 += 1;
            entry.1 += block.tokens.len();
            let family = family_of(block.label);
            *per_family_blocks.entry(family).or_insert(0usize) += 1;
            *per_family_tokens.entry(family).or_insert(0usize) += block.tokens.len();
        }
        out.push_str(&format!("fixture={}\n", FIXTURES[index]));
        out.push_str(&format!("sha256={}\n", FIXTURE_SHA256[index]));
        out.push_str(&format!("blocks={}\n", scan.blocks.len()));
        out.push_str(&format!("tokens={}\n", scan.tokens()));
        for label in BASE_LABELS {
            let (blocks, tokens) = per_label.get(label).copied().unwrap_or((0, 0));
            out.push_str(&format!("label={label}:{blocks}/{tokens}\n"));
        }
        for family in FAMILY_ORDER {
            let tokens = per_family_tokens.get(family).copied().unwrap_or(0);
            let blocks = per_family_blocks.get(family).copied().unwrap_or(0);
            out.push_str(&format!("family={family}:{tokens}/{blocks}\n"));
        }
        out.push_str(&format!(
            "runs={}\n",
            maximal_base_runs(&scan_label_vector(scan)).len()
        ));
        out.push_str("---\n");
    }
    out
}

/// Per-block label vector — the framing the run denominator is computed over.
fn scan_label_vector(scan: &FulltextScan) -> Vec<String> {
    scan.blocks
        .iter()
        .map(|block| block.label.to_owned())
        .collect()
}

/// SHA-256 over [`canonical_receipts`], recomputable from the tracked fixtures.
pub fn receipts_digest(scans: &[FulltextScan]) -> String {
    format!("{:x}", Sha256::digest(canonical_receipts(scans).as_bytes()))
}

// ---- end independent expectation builder ----

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/preserved")
        .join(name)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Read the preserved fixtures, verify their checksums, then scan them.
fn scanned_fixtures() -> Vec<FulltextScan> {
    FIXTURES
        .iter()
        .zip(FIXTURE_SHA256)
        .map(|(name, checksum)| {
            let bytes = fs::read(fixture_path(name)).expect("preserved fixture is tracked");
            assert_eq!(digest(&bytes), checksum, "fixture checksum drift: {name}");
            let xml = String::from_utf8(bytes).expect("preserved TEI is valid UTF-8");
            scan_fulltext_body(&xml)
        })
        .collect()
}

/// Blocks of one family, in document order.
fn family_blocks(scan: &FulltextScan, family: &str) -> Vec<ScanBlock> {
    scan.blocks
        .iter()
        .filter(|block| family_of(block.label) == family)
        .cloned()
        .collect()
}

/// Lexical counts of one fixture: `(blocks, tokens)` per base label.
fn label_counts(scan: &FulltextScan) -> Vec<(usize, usize)> {
    BASE_LABELS
        .iter()
        .map(|label| {
            let blocks = scan
                .blocks
                .iter()
                .filter(|block| block.label == *label)
                .count();
            let tokens: usize = scan
                .blocks
                .iter()
                .filter(|block| block.label == *label)
                .map(|block| block.tokens.len())
                .sum();
            (blocks, tokens)
        })
        .collect()
}

/// `(blocks, tokens)` for one family of one fixture.
fn family_counts(scan: &FulltextScan, family: &str) -> (usize, usize) {
    let blocks = family_blocks(scan, family);
    let tokens = blocks.iter().map(|block| block.tokens.len()).sum();
    (blocks.len(), tokens)
}

#[test]
fn lexical_fulltext_scan_matches_independent_receipts() {
    let scans = scanned_fixtures();
    assert_eq!(scans.len(), FIXTURE_BLOCKS.len());

    for (index, scan) in scans.iter().enumerate() {
        let name = FIXTURES[index];
        assert_eq!(scan.blocks.len(), FIXTURE_BLOCKS[index], "blocks: {name}");
        assert_eq!(scan.tokens(), FIXTURE_TOKENS[index], "tokens: {name}");
        assert_eq!(
            label_counts(scan),
            FIXTURE_LABEL_COUNTS[index].to_vec(),
            "per-label (blocks, tokens): {name}"
        );
        assert!(
            FIXTURE_LABEL_COUNTS[index]
                .iter()
                .map(|(blocks, tokens)| blocks + tokens)
                .sum::<usize>()
                != 0,
            "the receipt must not be empty: {name}"
        );

        let mut family_token_sum = 0usize;
        let mut family_block_sum = 0usize;
        let mut family_run_sum = 0usize;
        for (slot, family) in FAMILY_ORDER.iter().enumerate() {
            let (blocks, tokens) = family_counts(scan, family);
            assert_eq!(
                blocks, FIXTURE_FAMILY_BLOCKS[index][slot],
                "{family} blocks: {name}"
            );
            assert_eq!(
                tokens, FIXTURE_FAMILY_TOKENS[index][slot],
                "{family} tokens: {name}"
            );
            let runs = maximal_base_runs(&scan_label_vector_of(scan, family)).len();
            assert_eq!(
                runs, FIXTURE_FAMILY_RUNS[index][slot],
                "{family} runs: {name}"
            );
            family_token_sum += tokens;
            family_block_sum += blocks;
            family_run_sum += runs;
        }
        assert_eq!(
            family_token_sum,
            scan.tokens(),
            "families must partition every token exactly once: {name}"
        );
        assert_eq!(
            family_block_sum,
            scan.blocks.len(),
            "families must partition every block exactly once: {name}"
        );
        // Dropping the other families can only merge adjacent same-base blocks,
        // so the family split can never *add* runs; it is strictly smaller here
        // because the three families are interleaved in the body rather than
        // concatenated.
        assert!(
            family_run_sum < FIXTURE_RUNS[index],
            "the families are interleaved, not concatenated: {name}"
        );

        // Reserved lines of the dictionary that these fixtures cannot reach stay
        // pinned as zeros rather than being dropped from the receipt.
        for label in ["<item>", "<equation_marker>"] {
            let (blocks, tokens) = label_counts(scan)[BASE_LABELS
                .iter()
                .position(|entry| *entry == label)
                .unwrap()];
            assert_eq!(blocks, 0, "{label} has no lexical source: {name}");
            assert_eq!(tokens, 0, "{label} has no lexical source: {name}");
        }

        // Blocks are never empty, tokens never carry whitespace, and every label
        // is inside the ledger partition.
        assert!(
            scan.blocks.iter().all(|block| !block.tokens.is_empty()),
            "no empty block is admitted: {name}"
        );
        assert!(
            scan.blocks
                .iter()
                .flat_map(|block| block.tokens.iter())
                .all(|token| !token.chars().any(char::is_whitespace)),
            "tokens carry no whitespace: {name}"
        );
        assert!(
            scan.blocks
                .iter()
                .all(|block| BASE_LABELS.contains(&block.label)),
            "every block label is a ledger base label: {name}"
        );
    }

    assert_eq!(
        scans.iter().map(FulltextScan::tokens).sum::<usize>(),
        TOTAL_TOKENS
    );
    assert_eq!(
        scans.iter().map(|scan| scan.blocks.len()).sum::<usize>(),
        TOTAL_BLOCKS
    );
    assert_eq!(TOTAL_TOKENS, FIXTURE_TOKENS.iter().sum::<usize>());
    assert_eq!(TOTAL_BLOCKS, FIXTURE_BLOCKS.iter().sum::<usize>());
    assert_ne!(TOTAL_BLOCKS, TOTAL_TOKENS);
}

impl FulltextScan {
    fn blocks_of(&self, family: &str) -> Vec<ScanBlock> {
        family_blocks(self, family)
    }
}

/// Label vector of one family's block subsequence.
fn scan_label_vector_of(scan: &FulltextScan, family: &str) -> Vec<String> {
    scan.blocks_of(family)
        .iter()
        .map(|block| block.label.to_owned())
        .collect()
}

#[test]
fn scan_negatives_fail_closed_without_inventing_denominators() {
    // (1) A document without a body element is refused rather than scanned as if
    // the whole file were body.
    assert!(std::panic::catch_unwind(|| body_element_slice("<text><p>orphan</p></text>")).is_err());
    // (2) An unterminated body is refused too: the slice has no end.
    assert!(std::panic::catch_unwind(|| body_element_slice("<body><p>x</p>")).is_err());

    // (3) Text outside any block element is not body: an orphan run is dropped
    // rather than attributed to the nearest label.
    let orphan = "<body>loose words <p>real</p></body>";
    let scan = scan_fulltext_body(orphan);
    assert_eq!(scan.blocks.len(), 1);
    assert_eq!(scan.blocks[0].tokens, vec!["real".to_owned()]);

    // (4) A self-closing block element contributes no token and therefore no
    // block, and a nested block element takes the label of its own tokens while
    // the enclosing element resumes afterwards.
    let nested = "<body><p>before <formula>x = y</formula> after</p><figure/></body>";
    let scan = scan_fulltext_body(nested);
    assert_eq!(
        scan.blocks
            .iter()
            .map(|block| (block.label, block.tokens.len()))
            .collect::<Vec<_>>(),
        vec![("<paragraph>", 1), ("<equation>", 3), ("<paragraph>", 1)]
    );

    // (5) A `ref` marker interrupts with its own label, and an unknown `type`
    // falls into the ledger's fail-closed `other` bucket. The enclosing
    // paragraph resumes between the two markers, so one `<p>` yields three
    // paragraph blocks around its two markers.
    let markers = "<body><p>see <ref type=\"bibr\">[1]</ref> and \
                   <ref type=\"foot\">note</ref></p></body>";
    let scan = scan_fulltext_body(markers);
    assert_eq!(
        scan.blocks
            .iter()
            .map(|block| (block.label, block.tokens.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("<paragraph>", vec!["see".to_owned()]),
            ("<citation_marker>", vec!["[1]".to_owned()]),
            ("<paragraph>", vec!["and".to_owned()]),
            ("<other>", vec!["note".to_owned()]),
        ]
    );
}

#[test]
fn family_partition_is_the_data_ledger() {
    let raw = fs::read_to_string(repo_root().join("data/parse/native_body_accounting.yaml"))
        .expect("FATAL the body accounting ledger must be readable");
    let ledger: serde_yaml::Value =
        serde_yaml::from_str(&raw).expect("FATAL the body accounting ledger must parse");

    // The twelve base labels are exactly the ledger's `fulltext_labels.labels`,
    // as a set; the SUT's table and this oracle's table agree with the data.
    let mut declared: Vec<String> = ledger["fulltext_labels"]["labels"]
        .as_mapping()
        .expect("fulltext_labels.labels must be a mapping")
        .values()
        .map(|value| {
            value
                .as_str()
                .expect("every ledger label is a string")
                .to_owned()
        })
        .collect();
    declared.sort_unstable();
    let mut mine: Vec<String> = BASE_LABELS
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    mine.sort_unstable();
    assert_eq!(declared, mine, "base labels drifted from the ledger");
    assert_eq!(declared.len(), 12);

    // The families partition the dictionary exactly once, and the three
    // non-empty body families are the ones this oracle probes.
    let families = ledger["families"]
        .as_mapping()
        .expect("families must be a mapping");
    let mut partition: Vec<(String, Vec<String>)> = Vec::new();
    for (key, value) in families {
        let Some(name) = key.as_str() else { continue };
        let Some(entry) = value.as_mapping() else {
            continue;
        };
        let labels: Vec<String> = entry
            .get(serde_yaml::Value::String("fulltext_labels".into()))
            .and_then(|value| value.as_sequence())
            .map(|sequence| {
                sequence
                    .iter()
                    .map(|value| format!("<{}>", value.as_str().expect("label is a string")))
                    .collect()
            })
            .unwrap_or_default();
        partition.push((name.to_owned(), labels));
    }
    assert!(
        partition.iter().any(|(name, _)| name == "body_blocks"),
        "the ledger must declare the body_blocks family"
    );

    let mut assigned: Vec<String> = partition
        .iter()
        .flat_map(|(_, labels)| labels.iter().cloned())
        .collect();
    assigned.sort_unstable();
    assert_eq!(
        assigned, declared,
        "the ledger families must partition the dictionary exactly once"
    );
    assert_eq!(assigned.len(), 12);

    for (name, labels) in &partition {
        for label in labels {
            assert_eq!(
                family_of(label),
                name,
                "this oracle's partition disagrees with the ledger for {label}"
            );
        }
    }
    for family in FAMILY_ORDER {
        assert!(
            partition.iter().any(|(name, _)| name == family),
            "the ledger must declare the {family} family"
        );
    }

    // The ledger names these very denominators as pending an oracle, which is
    // what this file supplies; the pending marker must not be quietly dropped.
    let pending = &ledger["denominators"]["pending_body_counts"];
    for family in FAMILY_ORDER {
        assert_eq!(
            pending[family].as_str(),
            Some("pending_independent_oracle"),
            "{family} denominator slot drifted"
        );
    }
    // The ledger declares accounting families this lexical scan cannot
    // populate (the annex, references and page-furniture regions carry no
    // fulltext labels), so the populated set is the three body families this
    // oracle probes — a strict subset, never a coincidence of sizes.
    let populated: Vec<&str> = partition
        .iter()
        .filter(|(_, labels)| !labels.is_empty())
        .map(|(name, _)| name.as_str())
        .collect();
    let mut observed = populated.clone();
    observed.sort_unstable();
    let mut expected = FAMILY_ORDER.to_vec();
    expected.sort_unstable();
    assert_eq!(
        observed, expected,
        "the ledger's populated families are the ones this oracle probes"
    );
    assert!(
        populated.len() < partition.len(),
        "the ledger must also declare families whose fulltext labels are empty"
    );
}

#[test]
fn body_marker_population_is_not_the_bibliography_anchor_denominator() {
    let scans = scanned_fixtures();
    let mut total = 0usize;
    for (index, scan) in scans.iter().enumerate() {
        let markers = scan
            .blocks
            .iter()
            .filter(|block| block.label == "<citation_marker>")
            .count();
        assert_eq!(
            markers, BODY_BIBR_MARKERS[index],
            "bibr markers: {}",
            FIXTURES[index]
        );
        total += markers;
    }
    assert_eq!(total, TOTAL_BODY_BIBR_MARKERS);

    // Two populations, two numbers. The bibliography oracle pins the
    // whole-document anchor denominator (123 / 99 / 158); the body-scoped
    // marker population is 123 / 95 / 158. Neither may be substituted.
    assert_ne!(TOTAL_BODY_BIBR_MARKERS, BIBLIOGRAPHY_ANCHOR_TOTAL);
    assert_ne!(BODY_BIBR_MARKERS[1], 99);
    assert_eq!(BODY_BIBR_MARKERS[0], 123);
    assert_eq!(BODY_BIBR_MARKERS[2], 158);

    // A figure/table marker is never a bibliography anchor, and the marker
    // population never absorbs the reference lane's tokens.
    for (index, scan) in scans.iter().enumerate() {
        let marker_tokens: usize = scan
            .blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.label,
                    "<citation_marker>" | "<figure_marker>" | "<table_marker>"
                )
            })
            .map(|block| block.tokens.len())
            .sum();
        assert!(
            marker_tokens > BODY_BIBR_MARKERS[index],
            "marker tokens are not a restatement of the marker count: {}",
            FIXTURES[index]
        );
        assert!(
            marker_tokens < FIXTURE_TOKENS[index],
            "markers are a strict subset of the body: {}",
            FIXTURES[index]
        );
    }
}

#[test]
fn receipt_canonicalization_is_recomputable_and_stable() {
    let scans = scanned_fixtures();
    let first = receipts_digest(&scans);
    let second = receipts_digest(&scanned_fixtures());
    assert_eq!(first, second, "canonicalization is deterministic");
    assert_eq!(first.len(), 64, "sha256 hex digest");
    assert!(
        first.chars().all(|c| c.is_ascii_hexdigit()),
        "digest is hexadecimal"
    );
    assert_eq!(
        first, EXPECTED_RECEIPT_DIGEST,
        "canonical receipt digest drifted; recomputed value"
    );

    // Swapping two fixtures changes the digest: the canonicalization is
    // order-sensitive rather than a multiset.
    let mut swapped = scans.clone();
    swapped.swap(0, 1);
    assert_ne!(
        receipts_digest(&swapped),
        first,
        "fixture order is part of the canonical receipt"
    );
}

#[test]
fn grouping_rule_tiles_the_corpus_and_the_mirror_rule_does_not() {
    let scans = scanned_fixtures();

    for (index, scan) in scans.iter().enumerate() {
        let labels = spelling_labels(&scan.blocks);
        assert_eq!(
            labels.len(),
            FIXTURE_TOKENS[index],
            "one spelling per token: {}",
            FIXTURES[index]
        );
        let runs = maximal_base_runs(&labels);
        assert!(
            runs.iter().all(|(_, start, end)| start < end),
            "runs are non-empty: {}",
            FIXTURES[index]
        );
        // The runs tile the sequence end to end.
        let mut cursor = 0usize;
        for (_, start, end) in &runs {
            assert_eq!(*start, cursor, "runs tile without a gap");
            cursor = *end;
        }
        assert_eq!(cursor, labels.len(), "runs tile without a tail");
        assert_eq!(
            runs.len(),
            FIXTURE_RUNS[index],
            "run denominator: {}",
            FIXTURES[index]
        );

        // Two framings of the same denominator must agree: over the spelled
        // sequence and over the per-block label vector.
        assert_eq!(
            runs.len(),
            maximal_base_runs(&scan_label_vector(scan)).len(),
            "run framings must agree: {}",
            FIXTURES[index]
        );

        // The expectation is the *discriminating* one: the mirror-BIO rule that
        // S03 disproved tiles this corpus differently, so a regression to it is
        // observable here rather than silently equivalent.
        let mirror = mirror_bio_runs(&labels);
        assert_ne!(
            mirror.len(),
            runs.len(),
            "the mirror-BIO rule must not reproduce the D359 tiling: {}",
            FIXTURES[index]
        );
        assert!(
            mirror.len() > runs.len(),
            "the mirror rule over-splits, never under-splits: {}",
            FIXTURES[index]
        );
    }
}

#[test]
fn expected_builder_stays_free_of_the_sut_lane() {
    let source = include_str!("native_fulltext_same_n.rs");
    let forbidden = [
        "TeiLifter",
        "compose_native_fulltext",
        "run_native_fulltext_blocks",
        "java_fulltext",
    ];

    // The file never imports the replaced lane at all.
    let imports: String = source
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect::<Vec<_>>()
        .join("\n");
    for name in forbidden {
        assert!(
            !imports.contains(name),
            "{name} must not be imported into the independent fulltext same-n oracle"
        );
    }

    // The expectation region contains no SUT symbol of any kind, not even the
    // glue or the decoder it is compared against.
    let start_marker =
        "// ---- independent expectation builder (no SUT lane below this marker) ----";
    let end_marker = "// ---- end independent expectation builder ----";
    let start = source
        .find(start_marker)
        .expect("expectation builder start marker must exist");
    let body = &source[start + start_marker.len()..];
    let end = body
        .find(end_marker)
        .expect("expectation builder end marker must exist");
    let region = &body[..end];
    assert!(
        region.len() > 8_000,
        "expectation builder must be substantive, got {} bytes",
        region.len()
    );
    for name in [
        "TeiLifter",
        "compose_native_fulltext",
        "run_native_fulltext_blocks",
        "decode_fulltext_sequence",
        "render_fulltext_features",
        "render_fulltext_feature_text",
        "glue_fulltext_reading_order",
        "cross_page_runs",
        "check_fulltext_label_block",
        "NATIVE_FULLTEXT",
    ] {
        assert!(
            !region.contains(name),
            "{name} must not appear in the independent expectation builder"
        );
    }
    for name in forbidden {
        assert!(
            !region.contains(&format!("{name}(")),
            "{name} must not be called from the independent expectation builder"
        );
    }
}

// ---- SUT lane: the corpus drives the glue, the decoder and their tiling ----

/// Uniform, declared layout facts for one block list.
///
/// The TEI carries no geometry, so this canonicalization lays the blocks out
/// top-to-bottom on a single page with a constant gap: every block starts below
/// the lowest position already reached, which is exactly the `column_change =
/// false` case of the Java `lowestPos > block.getY()` test. The heights are
/// derived from the token counts, so no block can overlap the next one.
fn layout_tokens(blocks: &[ScanBlock], texts: &[Vec<String>]) -> Vec<NativeFulltextPageToken> {
    let heights: Vec<f64> = blocks
        .iter()
        .enumerate()
        .map(|(index, _)| 12.0 * texts[index].len() as f64)
        .collect();
    let mut block_y = Vec::with_capacity(blocks.len());
    let mut cursor = 72.0;
    for height in &heights {
        block_y.push(cursor);
        cursor += height + 12.0;
    }
    let page_height = block_y
        .last()
        .map(|y| y + heights.last().copied().unwrap_or(0.0) + 72.0)
        .unwrap_or(792.0);

    let mut out = Vec::new();
    for (ordinal, tokens) in texts.iter().enumerate() {
        let count = tokens.len() as u32;
        for (within, text) in tokens.iter().enumerate() {
            out.push(NativeFulltextPageToken {
                page: 1,
                page_height,
                block: ordinal as u32,
                block_y: block_y[ordinal],
                block_height: heights[ordinal],
                within_block: within as u32,
                block_token_count: count,
                x: 72.0,
                width: 8.0,
                y: block_y[ordinal] + 12.0 * within as f64,
                font: "Times-Roman".into(),
                font_size: 10.0,
                bold: false,
                italic: false,
                bitmap_around: false,
                callout_type: FulltextCalloutType::Unknown,
                callout_known: false,
                superscript: false,
                text: NativeFulltextTokenText::Word { text: text.clone() },
            });
        }
    }
    out
}

/// The spelling of every corpus token, one per block, in document order.
fn spelling_texts(blocks: &[ScanBlock]) -> Vec<Vec<String>> {
    blocks
        .iter()
        .map(|block| {
            block
                .tokens
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    if index == 0 {
                        format!("I-{}", block.label)
                    } else {
                        block.label.to_owned()
                    }
                })
                .collect()
        })
        .collect()
}

/// A scalar CRF that fixes every row's label by its own spelling.
///
/// Observation `i` is `u:<spelling i>` and its only weighted label is `i`, so
/// the decoded sequence is the input sequence by construction: the comparison
/// below is entirely about the *tiling* the runner computes, never about what
/// the probe "asked for" and then read back.
fn spelling_model() -> CrfModel {
    let labels: Vec<String> = NATIVE_FULLTEXT_LABELS
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let label_count = labels.len();
    let observations: Vec<String> = NATIVE_FULLTEXT_LABELS
        .iter()
        .map(|label| format!("u:{label}"))
        .collect();
    let mut u_offsets = Vec::with_capacity(observations.len());
    let mut weights = BTreeMap::new();
    for index in 0..observations.len() {
        let offset = index * label_count;
        u_offsets.push(Some(offset));
        weights.insert(offset + index, 3.0);
    }
    CrfModel {
        model_type: 2,
        labels,
        observations,
        patterns: vec![CrfPattern {
            source: "u:%x[0,0]".into(),
            kind: 'u',
        }],
        u_offsets,
        b_offsets: vec![None; label_count],
        weights,
        nftr: label_count * label_count,
    }
}

fn decode(
    rows: &[da_parse::FulltextFeatureRow],
    model: &CrfModel,
) -> da_parse::NativeFulltextDecodeCandidate {
    match decode_fulltext_sequence(rows, model) {
        NativeFulltextDecodeOutcome::Accepted { decode } => decode,
        NativeFulltextDecodeOutcome::Rejected { error } => {
            panic!("corpus-derived body rows must decode, got {error}")
        }
    }
}

#[test]
fn glued_reading_order_reproduces_the_lexical_scan() {
    let scans = scanned_fixtures();
    for (index, scan) in scans.iter().enumerate() {
        let name = FIXTURES[index];
        let mut family_tokens = 0usize;
        for family in FAMILY_ORDER {
            let blocks = family_blocks(scan, family);
            let texts: Vec<Vec<String>> = blocks.iter().map(|block| block.tokens.clone()).collect();
            let tokens = layout_tokens(&blocks, &texts);
            assert_eq!(
                tokens.len(),
                FIXTURE_FAMILY_TOKENS[index][FAMILY_ORDER
                    .iter()
                    .position(|entry| *entry == family)
                    .unwrap()],
                "{family} token denominator: {name}"
            );

            let order = glue_fulltext_reading_order(&tokens)
                .unwrap_or_else(|error| panic!("{family} of {name} must glue: {error}"));

            // Reading structure: same order, same texts, same count.
            assert_eq!(
                order.tokens.len(),
                tokens.len(),
                "one glued token per laid-out token: {family} of {name}"
            );
            assert_eq!(
                order
                    .tokens
                    .iter()
                    .map(|token| token.text.clone())
                    .collect::<Vec<_>>(),
                texts.iter().flatten().cloned().collect::<Vec<_>>(),
                "document order equals the TEI order: {family} of {name}"
            );

            // Row ABI: every row is the printed 27-column vector.
            assert_eq!(order.rows.len(), tokens.len());
            assert!(
                order
                    .rows
                    .iter()
                    .all(|row| row.width() == NATIVE_FULLTEXT_FEATURE_COLUMNS),
                "every row carries the printed ABI width: {family} of {name}"
            );

            // One declared page, and the single-column layout never shifts.
            assert_eq!(order.pages.len(), 1, "{family} of {name}");
            assert_eq!(order.pages[0].page, 1);
            assert!(order.pages[0].page_break);
            assert_eq!(order.pages[0].row_count, tokens.len());
            assert_eq!(order.pages[0].first_row, 0);
            assert_eq!(order.column_shifts.len(), blocks.len());
            assert!(
                order
                    .column_shifts
                    .iter()
                    .all(|shift| !shift.column_change && shift.spacing.is_some()),
                "a top-to-bottom layout has no column shift: {family} of {name}"
            );
            for (ordinal, shift) in order.column_shifts.iter().enumerate() {
                assert_eq!(shift.page, 1, "{family} of {name}");
                assert_eq!(shift.block as usize, ordinal, "{family} of {name}");
            }
            assert_eq!(
                order.document_length as usize,
                order
                    .tokens
                    .iter()
                    .map(|token| token.text.chars().count())
                    .sum::<usize>(),
                "the document length is the admitted token length: {family} of {name}"
            );

            // The reading identity is a stable digest, and it moves with the
            // content it identifies.
            let again = glue_fulltext_reading_order(&tokens).expect("the same input glues again");
            assert_eq!(order.digest, again.digest, "the digest is deterministic");
            let hex = order
                .digest
                .strip_prefix("sha256:")
                .expect("the reading identity names its hash");
            assert_eq!(hex.len(), 64, "the identity carries a sha256 digest");
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));

            family_tokens += tokens.len();
        }
        assert_eq!(
            family_tokens, FIXTURE_TOKENS[index],
            "families conserve: {name}"
        );
    }
}

#[test]
fn corpus_driven_decode_reproduces_the_lexically_derived_runs() {
    let scans = scanned_fixtures();
    let model = spelling_model();

    for (index, scan) in scans.iter().enumerate() {
        let name = FIXTURES[index];
        for (slot, family) in FAMILY_ORDER.iter().enumerate() {
            let blocks = family_blocks(scan, family);
            let texts = spelling_texts(&blocks);
            let tokens = layout_tokens(&blocks, &texts);
            let order = glue_fulltext_reading_order(&tokens)
                .unwrap_or_else(|error| panic!("{family} of {name} must glue: {error}"));

            let decode = decode(&order.rows, &model);

            // The decoded sequence is the corpus-derived spelling sequence,
            // position for position, before any grouping is considered.
            let expected_labels = texts.iter().flatten().cloned().collect::<Vec<_>>();
            assert_eq!(
                decode.labels, expected_labels,
                "decoded label sequence: {family} of {name}"
            );
            assert_eq!(
                decode.labels.len(),
                FIXTURE_FAMILY_TOKENS[index][slot],
                "one decoded label per token: {family} of {name}"
            );

            // Grouping: the runner's tiling must equal the independently
            // computed maximal base-name runs of the same sequence.
            let expected = maximal_base_runs(&expected_labels);
            let observed: Vec<(String, usize, usize)> = decode
                .runs
                .iter()
                .map(|run| (run.label.clone(), run.start, run.end))
                .collect();
            assert_eq!(observed, expected, "run tiling: {family} of {name}");
            assert_eq!(
                observed.len(),
                FIXTURE_FAMILY_RUNS[index][slot],
                "run denominator: {family} of {name}"
            );

            // The cross-page annotation is T02's tiling annotated, not a second
            // model: with one declared page no run is continued.
            let spans = order
                .cross_page_runs(&decode)
                .unwrap_or_else(|error| panic!("{family} of {name} must annotate: {error}"));
            assert_eq!(spans.len(), observed.len());
            assert!(
                spans
                    .iter()
                    .all(|run| run.first_page == 1 && run.last_page == 1 && !run.continued),
                "a single declared page has no continued run: {family} of {name}"
            );
            assert_eq!(
                spans
                    .iter()
                    .map(|run| (run.label.clone(), run.start, run.end))
                    .collect::<Vec<_>>(),
                observed,
                "the annotation preserves the tiling: {family} of {name}"
            );
        }
    }
}

#[test]
fn stripping_the_i_spelling_does_not_move_the_corpus_runs() {
    let scans = scanned_fixtures();
    let model = spelling_model();

    for (index, scan) in scans.iter().enumerate() {
        let name = FIXTURES[index];
        for family in FAMILY_ORDER {
            let blocks = family_blocks(scan, family);
            let spelled = spelling_texts(&blocks);
            let bare: Vec<Vec<String>> = blocks
                .iter()
                .map(|block| {
                    block
                        .tokens
                        .iter()
                        .map(|_| block.label.to_owned())
                        .collect()
                })
                .collect();

            let spelled_order = glue_fulltext_reading_order(&layout_tokens(&blocks, &spelled))
                .unwrap_or_else(|error| panic!("{family} of {name} must glue: {error}"));
            let bare_order = glue_fulltext_reading_order(&layout_tokens(&blocks, &bare))
                .unwrap_or_else(|error| panic!("{family} of {name} must glue: {error}"));

            let spelled_runs = decode(&spelled_order.rows, &model)
                .runs
                .into_iter()
                .map(|run| (run.label, run.start, run.end))
                .collect::<Vec<_>>();
            let bare_runs = decode(&bare_order.rows, &model)
                .runs
                .into_iter()
                .map(|run| (run.label, run.start, run.end))
                .collect::<Vec<_>>();
            assert_eq!(
                spelled_runs, bare_runs,
                "the spelling is never a boundary: {family} of {name}"
            );
            assert_eq!(
                spelled_runs,
                maximal_base_runs(&spelled.iter().flatten().cloned().collect::<Vec<_>>()),
                "stripping cannot move the independently computed tiling: {family} of {name}"
            );
        }
    }
}

#[test]
fn whole_body_glue_refuses_at_corpus_scale_rather_than_truncating() {
    let scans = scanned_fixtures();

    // Two of the three preserved bodies fit the bounded admission whole; the hub
    // body does not, and the refusal is the typed cap rather than a silent
    // truncation to the bound.
    for (index, scan) in scans.iter().enumerate() {
        let name = FIXTURES[index];
        let texts: Vec<Vec<String>> = scan
            .blocks
            .iter()
            .map(|block| block.tokens.clone())
            .collect();
        let tokens = layout_tokens(&scan.blocks, &texts);
        let outcome = glue_fulltext_reading_order(&tokens);
        if FIXTURE_TOKENS[index] <= 32_768 {
            let order = outcome.unwrap_or_else(|error| panic!("{name} must glue: {error}"));
            assert_eq!(order.rows.len(), FIXTURE_TOKENS[index]);
        } else {
            match outcome {
                Err(da_parse::NativeFulltextReadingOrderError::TooManyTokens {
                    actual,
                    maximum,
                }) => {
                    assert_eq!(actual, FIXTURE_TOKENS[index], "{name}");
                    assert_eq!(maximum, 32_768, "{name}");
                }
                other => panic!("{name} must be refused by the cap, got {other:?}"),
            }
        }
    }

    // The corpus is genuinely larger than the bound on one side and smaller on
    // the other: the cap is exercised by real data, not by a synthetic input.
    assert!(FIXTURE_TOKENS[0] > 32_768);
    assert!(FIXTURE_TOKENS[1] < 32_768);
    assert!(FIXTURE_TOKENS[2] < 32_768);
    // ... and every family probe stays inside the bound.
    for family_slot in 0..FAMILY_ORDER.len() {
        for index in 0..FIXTURES.len() {
            assert!(
                FIXTURE_FAMILY_TOKENS[index][family_slot] <= 32_768,
                "family {} of {} must fit the bound",
                FAMILY_ORDER[family_slot],
                FIXTURES[index]
            );
        }
    }
}
