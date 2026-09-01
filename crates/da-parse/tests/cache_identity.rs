//! Cache-identity regression gate for the scholarly parse plane
//! (M464-z84in2 / S02 / T04; projection-identity rotation S07 / T05).
//!
//! Proves the S02 seam extraction (ParseBackend / TeiEnvelope / TeiLifter /
//! ScholarlyParser) did NOT rotate any production identity:
//!
//! - D204 profile fingerprints (`GrobidProfile::configuration_fingerprint`)
//!   stay byte-stable and keep separating rich from structural; envelope
//!   identity rides the exact same strings, so envelope and cache identity
//!   cannot drift apart and transport tuning never leaks in;
//! - the frozen baseline identities in the tracked
//!   `artifacts/parse_compat/baseline.json` (per-case output hashes,
//!   aggregate output/source hashes, corpus hash, conservation totals) are
//!   reproduced EXACTLY by the new composition path (CachedTeiBackend →
//!   ScholarlyParser → TeiLifter → projection) over the same preserved
//!   fixtures — cached TEI is lifted without any HTTP and yields the S01
//!   identity byte-for-byte;
//! - the bounded legacy/production cache-identity strings production relies
//!   on (`da_adapters::GrobidParser@load-v1`, `ParsedArticle.v1`, the legacy
//!   `pdf-grobid` lookup, `GrobidParser@tei-navigation-v3`, the
//!   S07/T05-rotated projection fingerprint
//!   `ParsedArticle@serde-v4-p0-tei-lift` (owned by the single da-parse
//!   const and referenced — never re-literalized — by production), the
//!   GROBID model fingerprint) are still pinned in their source surfaces;
//! - the baseline report keeps honest typed absences (the structural
//!   companion is reported, never fabricated) and never carries raw
//!   document content.
//!
//! Since S04/T04 the gate additionally pins the typed transport identity
//! guards: extended (non-legacy-equivalent) V1 dialects own deterministic,
//! pairwise-distinct `fulltext=v1;…` canonical identities that never collide
//! with the legacy D204 fingerprints, and transport tuning (timeout, opt-in
//! request/response byte bounds) stays excluded from backend envelope
//! identity.
//!
//! Scope guards: no network, no GROBID, no graph store, no wall-clock
//! inputs. Fixtures and the baseline report are git-tracked files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use da_parse::{
    CachedTeiBackend, Consolidation, CoordinateFamily, CoordinateSet, FulltextDialectV1,
    FulltextRequestVersioned, GROBID_HTTP_BACKEND_ID, GrobidHttpBackend, GrobidProfile,
    HeaderDialectV1, PageWindow, ParseBackend, ParseBackendRequest, ReferencesDialectV1,
    ScholarlyParser, TeiEnvelope, TeiIdentity, TeiLifter, TeiResultIdentity,
};
use da_ports::parser::ParsedArticle;
use serde_json::json;
use sha2::{Digest, Sha256};

const BASELINE_REL: &str = "artifacts/parse_compat/baseline.json";
const REGRESSION_MANIFEST_REL: &str = "data/parse/regression_cases.yaml";
const COMPARISON_CONTRACT_REL: &str = "data/parse/comparison_contract.yaml";

/// Canonical D204 fingerprints. The live API must keep producing exactly
/// these strings; drift is additionally caught by tests/data_contracts.rs.
const RICH_FP: &str = "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula";
const STRUCTURAL_FP: &str = "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula";

/// Per-case output hashes frozen at S01 and rotated by S07/T05: the
/// `da-parse.lift-output-preimage.v2` preimage now covers the additive P0
/// lift families (declared arXiv id, header authors, figures with graphic
/// coordinates, visual callouts). Any further change means the lift/projection
/// output identity rotated and must be a conscious, coordinated decision —
/// never a refactor side effect.
const FROZEN_CASE_OUTPUT_HASHES: [(&str, &str); 3] = [
    (
        "reg.tei.hub-bibliography-2507.19457",
        "sha256:f8ce01ec075828c93487d0803d74e328ae6758f9365a03d9eee6a474cea40e68",
    ),
    (
        "reg.tei.rlm-2512.24601",
        "sha256:f77b414af8909932599f91d4d633baf1a0a8c79e8bfbb26cdd2ee907ccd26229",
    ),
    (
        "reg.tei.reca-bibliography-2605.26525",
        "sha256:e7ac1aaa3ccd2666bc97804f3cd50f4f29d7771a4038d9793696edd33677f593",
    ),
];

/// Aggregate identities frozen at S01, output hash rotated by S07/T05 (the
/// v2 preimage covers the lifted P0 families).
const FROZEN_AGGREGATE_OUTPUT_HASH: &str =
    "sha256:3d1f0a537766de0f8003b34498b8685611a8c7df218c79c1055020cd403bdea7";
const FROZEN_SOURCE_HASH: &str =
    "sha256:d0b2c6c7e3b4af551b1e804705b6424cd1c367832516beb383d9c3cb5ade4c49";
/// Rotated by S03/T04 (per-case `conservation_delta` evidence), S07/T05
/// (hub-case note records the TEI-byte author/figure facts) and S08/T03
/// (date obligations added to the same three regression cases): the byte
/// identity of the manifest follows each sanctioned DATA edit.
const FROZEN_CORPUS_HASH: &str =
    "sha256:cd7a35fb089b6facb33c0caf70f89522ac7bf2c664182ad81ed6d84532c628fd";
/// All three executed S01 cases run under the rich profile.
const FROZEN_PROFILE_FINGERPRINT: &str = RICH_FP;

/// Aggregate lifted family totals frozen at S01; the S07 P0 families
/// (header_authors, figures, visual_callouts) joined with the S07/T05
/// rotation of the baseline report.
const FROZEN_TOTALS: [(&str, u64); 7] = [
    ("citations", 207),
    ("figures", 106),
    ("formulas", 38),
    ("header_authors", 36),
    ("sections", 264),
    ("tables", 8),
    ("visual_callouts", 106),
];

/// Pending structural companion identity (typed absence — honest).
const PENDING_STRUCTURAL_CASE: &str = "reg.tei.compact-hub-structural-identity";
const PENDING_STRUCTURAL_OWNER: &str = "S01/T03";

// ─────────────────────────── loading helpers ──────────────────────────

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn read_bytes(rel: &str) -> Vec<u8> {
    std::fs::read(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("tracked artifact unreadable: {rel}: {e}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn load_baseline() -> serde_json::Value {
    let text = String::from_utf8(read_bytes(BASELINE_REL)).expect("baseline report is utf-8");
    serde_json::from_str(&text).expect("baseline.json parses as JSON")
}

fn baseline_cases(baseline: &serde_json::Value) -> Vec<serde_json::Value> {
    baseline
        .get("cases")
        .and_then(|c| c.as_array())
        .cloned()
        .expect("baseline report carries a cases array")
}

fn str_field<'a>(case: &'a serde_json::Value, field: &str) -> &'a str {
    case.get(field)
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("baseline case lacks `{field}`"))
}

fn u64_field(case: &serde_json::Value, pointer: &str) -> u64 {
    case.pointer(pointer)
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| panic!("baseline case lacks `{pointer}`"))
}

/// Canonical digest preimage of the lifted projection — byte-identical to
/// `crates/da-cli/examples/parse_compat_baseline.rs::lifted_output_preimage`.
/// Raw `tei_xml` is deliberately bound to null: the identity covers lifted
/// content only, and raw TEI/PDF/document text never enters reports.
///
/// v2 (M464-z84in2 S07/T05): the additive P0 lift families joined the
/// preimage — the v1 digest was blind to exactly the families this slice
/// lifted, so the output-hash pin rotates with them.
fn lifted_output_preimage(article: &ParsedArticle) -> serde_json::Value {
    json!({
        "schema": "da-parse.lift-output-preimage.v2",
        "paper_id": article.paper_id,
        "pdf_hash": article.pdf_hash,
        "title": article.title,
        "abstract_text": article.abstract_text,
        "body_text": article.body_text,
        "sections": &article.sections,
        "citations": &article.citations,
        "tables": &article.tables,
        "table_missingness": &article.table_missingness,
        "formulas": &article.formulas,
        "formula_missingness": &article.formula_missingness,
        "declared_arxiv_id": &article.declared_arxiv_id,
        "declared_id_missingness": &article.declared_id_missingness,
        "header_authors": &article.header_authors,
        "figures": &article.figures,
        "visual_callouts": &article.visual_callouts,
        "tei_xml": serde_json::Value::Null,
    })
}

fn output_hash_of(article: &ParsedArticle) -> String {
    let preimage = lifted_output_preimage(article);
    let canonical = serde_json::to_string(&preimage).expect("preimage serializes");
    format!("sha256:{}", sha256_hex(canonical.as_bytes()))
}

/// One baseline case recomputed through the S02 seams: preserved TEI bytes →
/// cached-TEI envelope (no HTTP) → TeiLifter → projection. Arguments mirror
/// the baseline emitter exactly (provenance paper id + sha256 of the fixture
/// bytes as pdf_hash).
struct RecomputedCase {
    case_id: String,
    source_path: String,
    source_sha: String,
    output_hash: String,
    article: ParsedArticle,
}

fn recompute_case(case: &serde_json::Value) -> RecomputedCase {
    let case_id = str_field(case, "case_id").to_string();
    let source_path = str_field(case, "source_path").to_string();
    let source_sha = str_field(case, "source_sha256").to_string();
    let paper_id = str_field(case, "provenance_paper_id").to_string();

    let tei_bytes = read_bytes(&source_path);
    assert_eq!(
        sha256_hex(&tei_bytes),
        source_sha,
        "{case_id}: preserved fixture bytes drifted from the frozen checksum"
    );

    let envelope = TeiEnvelope::new(
        tei_bytes.clone(),
        TeiIdentity::cached_tei(GrobidProfile::rich()),
    );
    let parser = ScholarlyParser::new(CachedTeiBackend::with_identity(
        tei_bytes,
        TeiIdentity::cached_tei(GrobidProfile::rich()),
    ));
    let article = parser.lift_and_project(&paper_id, source_sha.clone(), envelope);
    let output_hash = output_hash_of(&article);
    RecomputedCase {
        case_id,
        source_path,
        source_sha,
        output_hash,
        article,
    }
}

// ─────────────────────────────── tests ────────────────────────────────

#[test]
fn d204_profile_fingerprints_are_byte_stable_and_identity_bound() {
    assert_eq!(GrobidProfile::rich().configuration_fingerprint(), RICH_FP);
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        STRUCTURAL_FP
    );
    assert_ne!(RICH_FP, STRUCTURAL_FP, "profiles must never share identity");

    // Envelope identity rides the exact same strings: envelope and cache
    // identity cannot drift apart, and the backend id is exact provenance.
    let http = TeiIdentity::grobid_http(GrobidProfile::rich());
    assert_eq!(http.backend_id(), GROBID_HTTP_BACKEND_ID);
    assert_eq!(http.profile_fingerprint(), RICH_FP);
    let cached = TeiIdentity::cached_tei(GrobidProfile::structural());
    assert_eq!(cached.backend_id(), da_parse::CACHED_TEI_BACKEND_ID);
    assert_eq!(cached.profile_fingerprint(), STRUCTURAL_FP);
    assert_ne!(http, cached, "rich/structural identities stay separated");
}

#[test]
fn s01_baseline_output_hashes_are_reproduced_through_the_cached_composition() {
    let baseline = load_baseline();
    let cases = baseline_cases(&baseline);
    assert_eq!(
        cases.len(),
        3,
        "frozen baseline executes exactly the three bound preserved cases"
    );

    for (case, (frozen_id, frozen_hash)) in cases.iter().zip(FROZEN_CASE_OUTPUT_HASHES) {
        let recomputed = recompute_case(case);
        assert_eq!(
            recomputed.case_id, frozen_id,
            "frozen case order/ids changed"
        );
        assert_eq!(
            recomputed.output_hash, frozen_hash,
            "{}: output identity rotated through the S02 composition",
            recomputed.case_id
        );
        assert_eq!(
            recomputed.output_hash,
            str_field(case, "output_hash"),
            "{}: baseline.json row disagrees with the frozen constant",
            recomputed.case_id
        );

        // Aggregate-counts-only conservation against the frozen row (no raw
        // content — counts only, matching the comparison contract).
        assert_eq!(
            u64_field(case, "/family_counts/sections"),
            recomputed.article.sections.len() as u64,
            "{}: section count drifted",
            recomputed.case_id
        );
        assert_eq!(
            u64_field(case, "/family_counts/citations"),
            recomputed.article.citations.len() as u64,
            "{}: citation count drifted",
            recomputed.case_id
        );
        assert_eq!(
            u64_field(case, "/family_counts/tables"),
            recomputed.article.tables.len() as u64,
            "{}: table count drifted",
            recomputed.case_id
        );
        assert_eq!(
            u64_field(case, "/family_counts/formulas"),
            recomputed.article.formulas.len() as u64,
            "{}: formula count drifted",
            recomputed.case_id
        );
    }
}

#[tokio::test]
async fn full_cached_composition_reproduces_a_frozen_case_output_hash() {
    let baseline = load_baseline();
    let cases = baseline_cases(&baseline);
    let hub = cases
        .iter()
        .find(|c| str_field(c, "case_id") == FROZEN_CASE_OUTPUT_HASHES[0].0)
        .expect("hub case present in frozen baseline");

    let source_path = str_field(hub, "source_path");
    let paper_id = str_field(hub, "provenance_paper_id");
    let tei_bytes = read_bytes(source_path);

    // Runtime path: CachedTeiBackend (exactly one fetch, no HTTP) →
    // envelope → lift → project. pdf_hash is derived inside the composition
    // from the same source bytes the baseline emitter hashed.
    let parser = ScholarlyParser::new(CachedTeiBackend::new(
        tei_bytes.clone(),
        GrobidProfile::rich(),
    ));
    let article = parser
        .parse(paper_id, tei_bytes)
        .await
        .expect("cached-TEI replay is total on preserved fixtures");
    assert!(
        article.tei_xml.is_some(),
        "ParsedArticle keeps tei_xml = Some(raw TEI) on the composed path"
    );
    assert_eq!(
        output_hash_of(&article),
        FROZEN_CASE_OUTPUT_HASHES[0].1,
        "full backend→envelope→lift→project runtime composition rotated the frozen identity"
    );
}

#[test]
fn s01_baseline_aggregate_identities_are_unchanged() {
    let baseline = load_baseline();
    let cases = baseline_cases(&baseline);
    let mut source_lines = String::new();
    let mut output_lines = String::new();
    let mut totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut fps: BTreeSet<String> = BTreeSet::new();

    for case in &cases {
        let recomputed = recompute_case(case);
        source_lines.push_str(&format!(
            "{}\t{}\t{}\n",
            recomputed.case_id, recomputed.source_path, recomputed.source_sha
        ));
        output_lines.push_str(&format!(
            "{}\t{}\n",
            recomputed.case_id, recomputed.output_hash
        ));
        fps.insert(str_field(case, "profile_fingerprint").to_string());
        let family = case
            .get("family_counts")
            .and_then(|f| f.as_object())
            .expect("family_counts object");
        for (key, value) in family {
            *totals.entry(key.clone()).or_insert(0) += value.as_u64().unwrap_or(0);
        }
    }

    let recomputed_source_hash = format!("sha256:{}", sha256_hex(source_lines.as_bytes()));
    let recomputed_output_hash = format!("sha256:{}", sha256_hex(output_lines.as_bytes()));
    assert_eq!(recomputed_source_hash, FROZEN_SOURCE_HASH);
    assert_eq!(recomputed_output_hash, FROZEN_AGGREGATE_OUTPUT_HASH);

    let identity = baseline
        .get("report_identity")
        .expect("report_identity object");
    assert_eq!(
        identity.get("source_hash").and_then(|v| v.as_str()),
        Some(FROZEN_SOURCE_HASH),
        "baseline source_hash drifted from the recomputation"
    );
    assert_eq!(
        identity.get("output_hash").and_then(|v| v.as_str()),
        Some(FROZEN_AGGREGATE_OUTPUT_HASH),
        "baseline aggregate output_hash drifted from the recomputation"
    );
    assert_eq!(
        identity.get("corpus_hash").and_then(|v| v.as_str()),
        Some(FROZEN_CORPUS_HASH),
        "regression manifest bytes changed — corpus identity rotated"
    );
    assert_eq!(
        sha256_hex(&read_bytes(REGRESSION_MANIFEST_REL)),
        FROZEN_CORPUS_HASH.trim_start_matches("sha256:"),
        "the tracked manifest no longer hashes to the frozen corpus identity"
    );
    assert_eq!(identity.get("corpus_n").and_then(|v| v.as_u64()), Some(3));
    assert_eq!(identity.get("same_n").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        identity.get("profile_fingerprint").and_then(|v| v.as_str()),
        Some(FROZEN_PROFILE_FINGERPRINT),
        "frozen executed-profile fingerprint rotated"
    );
    assert_eq!(
        fps.into_iter().collect::<Vec<_>>(),
        vec![FROZEN_PROFILE_FINGERPRINT.to_string()],
        "executed cases must stay uniformly rich-profile bound"
    );

    let mut expected_totals: BTreeMap<String, u64> = BTreeMap::new();
    for (k, v) in FROZEN_TOTALS {
        expected_totals.insert(k.to_string(), v);
    }
    assert_eq!(
        totals, expected_totals,
        "aggregate conservation totals drifted from S01"
    );
}

#[test]
fn baseline_keeps_honest_typed_absences_and_no_raw_content() {
    let baseline = load_baseline();

    // The structural companion stays a typed absence — reported, never
    // fabricated, and still owned.
    let pending = baseline
        .get("pending_cases")
        .and_then(|p| p.as_array())
        .expect("pending_cases array");
    assert_eq!(pending.len(), 1, "exactly one typed absence is expected");
    let pending = &pending[0];
    assert_eq!(
        pending.get("case_id").and_then(|v| v.as_str()),
        Some(PENDING_STRUCTURAL_CASE)
    );
    assert_eq!(
        pending.get("binding_owner_task").and_then(|v| v.as_str()),
        Some(PENDING_STRUCTURAL_OWNER)
    );
    assert_eq!(
        pending.get("typed_absence").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        pending.get("status").and_then(|v| v.as_str()),
        Some("not_observed")
    );
    assert_eq!(
        pending
            .get("expected_profile_fingerprint")
            .and_then(|v| v.as_str()),
        Some(STRUCTURAL_FP),
        "pending structural case must keep its own (distinct) identity"
    );

    // No raw document content anywhere in the report (forbidden field names
    // are read generically from the comparison contract).
    let contract_text =
        String::from_utf8(read_bytes(COMPARISON_CONTRACT_REL)).expect("comparison contract utf-8");
    let contract: serde_yaml::Value =
        serde_yaml::from_str(&contract_text).expect("comparison contract parses");
    let forbidden: Vec<String> = contract
        .get("forbidden_report_fields")
        .and_then(|f| f.as_sequence())
        .expect("forbidden_report_fields list")
        .iter()
        .filter_map(|v| v.as_str().map(ToString::to_string))
        .collect();
    assert!(!forbidden.is_empty(), "contract lost its forbidden fields");

    fn collect_keys(value: &serde_json::Value, keys: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    keys.push(k.clone());
                    collect_keys(v, keys);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    collect_keys(item, keys);
                }
            }
            _ => {}
        }
    }
    let mut keys = Vec::new();
    collect_keys(&baseline, &mut keys);
    for field in &forbidden {
        assert!(
            !keys.iter().any(|k| k == field),
            "forbidden field `{field}` leaked into the baseline report"
        );
    }

    // Policy locks and honest Java evidence stay verbatim.
    let locks = baseline.get("policy_locks").expect("policy_locks");
    assert_eq!(
        locks.get("import_eligible").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        locks.get("dspy_enabled").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        baseline
            .pointer("/java_grobid/selected")
            .and_then(|v| v.as_bool()),
        Some(false),
        "default baseline run never contacts GROBID"
    );
    assert_eq!(
        baseline
            .pointer("/report_identity/promotion_state")
            .and_then(|v| v.as_str()),
        Some("candidate"),
        "promotion state must stay candidate (no self-promotion)"
    );
}

#[test]
fn production_cache_identity_strings_stay_pinned_in_their_source_surfaces() {
    // Quoted needles match the exact Rust string literals — a rename or
    // fingerprint bump cannot slip through a substring accident.
    // Since S06/T03 the legacy rich-PDF `pdf-grobid` lookup lives in
    // `crates/da-adapters/src/parse_cache.rs` (extracted verbatim from
    // load.rs and used unchanged by `da load`) and the pin moved with it.
    let pins: [(&str, &[&str]); 5] = [
        (
            "crates/da-cli/src/load.rs",
            &[
                "\"da_adapters::GrobidParser@load-v1\"",
                "\"da_adapters::HtmlParser@load-v1\"",
                "\"ParsedArticle.v1\"",
            ],
        ),
        (
            "crates/da-adapters/src/parse_cache.rs",
            &["\"pdf-grobid\"", "\"html-primary\""],
        ),
        (
            "crates/da-cli/src/load_journey.rs",
            &[
                "\"da_adapters::GrobidParser@load-v1\"",
                "\"ParsedArticle.v1\"",
            ],
        ),
        (
            "crates/da-cli/src/production.rs",
            &[
                "\"GrobidParser@tei-navigation-v3\"",
                // S07/T05: the projection fingerprint lives ONLY in the
                // da-parse const — production references it instead of
                // carrying a drift-prone literal copy.
                "PROJECT_ARTICLE_SCHEMA_FINGERPRINT",
                "\"grobid/grobid:0.9.0-crf\"",
            ],
        ),
        (
            "crates/da-adapters/src/pipeline_stages.rs",
            &[
                "\"GrobidParser@tei-navigation-v3\"",
                "PROJECT_ARTICLE_SCHEMA_FINGERPRINT",
            ],
        ),
    ];
    for (rel, needles) in pins {
        let text =
            String::from_utf8(read_bytes(rel)).unwrap_or_else(|_| panic!("{rel} must be utf-8"));
        for needle in needles {
            assert!(
                text.contains(needle),
                "cache identity {needle} disappeared from {rel} — rotation requires a coordinated identity decision (D204), never a refactor side effect"
            );
        }
    }
}

/// The pure lifter stays directly addressable as the single algorithm owner:
/// lifting the same envelope bytes twice yields byte-identical IR (the
/// determinism floor behind the frozen hashes above).
#[test]
fn tei_lifter_replay_is_byte_deterministic() {
    let baseline = load_baseline();
    let cases = baseline_cases(&baseline);
    let first = &cases[0];
    let tei_bytes = read_bytes(str_field(first, "source_path"));
    let envelope = TeiEnvelope::new(tei_bytes, TeiIdentity::cached_tei(GrobidProfile::rich()));
    let lifter = TeiLifter::new();

    let ir_a = lifter.lift(
        &envelope,
        str_field(first, "provenance_paper_id"),
        str_field(first, "source_sha256"),
    );
    let ir_b = lifter.lift(
        &envelope,
        str_field(first, "provenance_paper_id"),
        str_field(first, "source_sha256"),
    );
    assert_eq!(ir_a.tei_xml, ir_b.tei_xml);
    assert_eq!(ir_a.sections.len(), ir_b.sections.len());
    assert_eq!(ir_a.citations.len(), ir_b.citations.len());
    assert_eq!(
        serde_json::to_vec(&ir_a).expect("ir serializes"),
        serde_json::to_vec(&ir_b).expect("ir serializes"),
        "TeiLifter replay must stay byte-deterministic"
    );
}

// ───────── S03/T04 conservation evidence binding ─────────

#[test]
fn s03_conservation_evidence_is_bound_per_case_and_aggregates_exactly() {
    let baseline = load_baseline();
    let cases = baseline_cases(&baseline);
    assert_eq!(cases.len(), 3, "three bound executed cases");

    let mut reason_totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut structure_totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut total_rows = 0u64;
    for case in &cases {
        let cons = case
            .get("conservation")
            .expect("every executed case carries conservation evidence");
        assert_eq!(
            cons.get("engine").and_then(|v| v.as_str()),
            Some("da_parse::ConservationEngine"),
            "the sanctioned engine must stay bound"
        );
        assert_eq!(
            cons.get("backend_id").and_then(|v| v.as_str()),
            Some("cached-tei"),
            "direct-TEI conservation identity is the cached-tei envelope identity"
        );
        assert_eq!(
            cons.get("raw_tei_sha256").and_then(|v| v.as_str()),
            Some(str_field(case, "source_sha256")),
            "delta identity must bind the exact fixture checksum"
        );
        assert_eq!(
            cons.get("profile_fingerprint").and_then(|v| v.as_str()),
            Some(str_field(case, "profile_fingerprint")),
            "delta identity must bind the exact profile fingerprint"
        );
        assert_eq!(
            cons.get("verdict").and_then(|v| v.as_str()),
            Some("loss_detected"),
            "pristine preserved baseline stays pinned loss_detected (S03/T02)"
        );
        assert_eq!(
            cons.get("malformed_rows").and_then(|v| v.as_u64()),
            Some(0),
            "pristine fixtures carry no malformed bytes"
        );
        let digest = cons
            .get("delta_digest")
            .and_then(|v| v.as_str())
            .expect("delta_digest present");
        assert_eq!(digest.len(), 64, "delta digest is plain 64-hex");
        assert_eq!(
            cons.pointer("/delta/identity/raw_tei_sha256")
                .and_then(|v| v.as_str()),
            Some(str_field(case, "source_sha256")),
            "the embedded delta carries the same exact identity"
        );
        for (reason, count) in cons
            .get("reason_counts")
            .and_then(|v| v.as_object())
            .expect("reason_counts object")
        {
            *reason_totals.entry(reason.clone()).or_insert(0) += count.as_u64().unwrap_or(0);
        }
        for (key, count) in cons
            .get("structure_counts")
            .and_then(|v| v.as_object())
            .expect("structure_counts object")
        {
            *structure_totals.entry(key.clone()).or_insert(0) += count.as_u64().unwrap_or(0);
        }
        total_rows += cons.get("total_rows").and_then(|v| v.as_u64()).unwrap_or(0);
    }

    let aggregate = baseline
        .get("conservation")
        .expect("aggregate conservation section");
    assert_eq!(aggregate.get("cases").and_then(|v| v.as_u64()), Some(3));
    assert_eq!(
        aggregate
            .pointer("/verdicts/loss_detected")
            .and_then(|v| v.as_u64()),
        Some(3)
    );
    assert_eq!(
        aggregate.get("total_rows").and_then(|v| v.as_u64()),
        Some(total_rows)
    );
    assert_eq!(
        aggregate.get("malformed_rows").and_then(|v| v.as_u64()),
        Some(0)
    );
    for (reason, total) in aggregate
        .get("reason_counts")
        .and_then(|v| v.as_object())
        .expect("aggregate reason_counts")
    {
        assert_eq!(
            total.as_u64(),
            Some(reason_totals.get(reason).copied().unwrap_or(0)),
            "aggregate reason {reason} must equal the per-case sum"
        );
    }
    for (key, total) in aggregate
        .get("structure_counts")
        .and_then(|v| v.as_object())
        .expect("aggregate structure_counts")
    {
        assert_eq!(
            total.as_u64(),
            Some(structure_totals.get(key).copied().unwrap_or(0)),
            "aggregate structure {key} must equal the per-case sum"
        );
    }
    let digests = aggregate
        .get("delta_digests")
        .and_then(|v| v.as_object())
        .expect("aggregate delta_digests map");
    assert_eq!(digests.len(), 3, "one digest per case");
}

// ───────── S04/T04 typed transport identity guards ─────────

/// Extended (non-legacy-equivalent) dialects own a versioned canonical
/// identity that is deterministic, pairwise distinct and can never collide
/// with the legacy D204 fingerprint namespace — which stays the ONLY cache
/// key for legacy presets (they claim no extended identity).
#[test]
fn extended_dialect_identities_are_deterministic_distinct_and_legacy_collision_free() {
    let extended = [
        FulltextDialectV1::legacy_rich()
            .with_consolidation(Consolidation::try_new(2, 1, 0).expect("sanctioned levels")),
        FulltextDialectV1::legacy_rich().with_raw_citations(true),
        FulltextDialectV1::legacy_rich().with_raw_affiliations(true),
        FulltextDialectV1::legacy_rich().with_segment_sentences(true),
        FulltextDialectV1::legacy_rich().with_emit_ids(true),
        FulltextDialectV1::legacy_rich()
            .with_page_window(Some(PageWindow::try_new(2, 5).expect("valid window"))),
        FulltextDialectV1::legacy_rich().with_coordinates(CoordinateSet::from_families([
            CoordinateFamily::Figure,
            CoordinateFamily::Formula,
        ])),
    ];
    let mut identities = Vec::new();
    for dialect in extended {
        let identity = dialect
            .canonical_identity()
            .expect("extended dialect owns a canonical identity");
        assert!(
            identity.starts_with("fulltext=v1;"),
            "identity must be versioned: {identity}"
        );
        assert_eq!(
            identity,
            dialect
                .canonical_identity()
                .expect("deterministic identity"),
            "canonical identity must be deterministic"
        );
        // Roundtrip through the versioned envelope preserves the identity.
        let wire = serde_json::to_string(&FulltextRequestVersioned::v1(dialect))
            .expect("envelope serializes");
        let back: FulltextRequestVersioned =
            serde_json::from_str(&wire).expect("envelope roundtrips");
        assert_eq!(
            back.into_dialect().canonical_identity(),
            Some(identity.clone())
        );
        assert_ne!(
            identity, RICH_FP,
            "extended identity must not collide with rich"
        );
        assert_ne!(
            identity, STRUCTURAL_FP,
            "extended identity must not collide with structural"
        );
        identities.push(identity);
    }
    let unique: BTreeSet<&String> = identities.iter().collect();
    assert_eq!(
        unique.len(),
        identities.len(),
        "extended identities must be pairwise distinct: {identities:?}"
    );

    // Legacy presets keep NO extended identity: the D204 fingerprint stays
    // their only cache key, so the two namespaces cannot collide.
    assert!(
        FulltextDialectV1::legacy_rich()
            .canonical_identity()
            .is_none()
    );
    assert!(
        FulltextDialectV1::legacy_structural()
            .canonical_identity()
            .is_none()
    );
    assert_eq!(
        GrobidProfile::rich().dialect(),
        FulltextDialectV1::legacy_rich()
    );
    assert_eq!(
        GrobidProfile::structural().dialect(),
        FulltextDialectV1::legacy_structural()
    );
}

/// Transport tuning (timeout, opt-in request/response byte bounds) is
/// excluded from backend envelope identity: backends differing only in
/// tuning stamp identical provenance. Offline by construction — `identity()`
/// is a pure projection and no request is sent.
#[test]
fn transport_tuning_stays_excluded_from_envelope_identity() {
    let base = GrobidHttpBackend::new(Some("http://127.0.0.1:1"));
    assert_eq!(base.identity().backend_id(), GROBID_HTTP_BACKEND_ID);

    let slow = base
        .clone()
        .with_timeout(std::time::Duration::from_secs(600));
    let fast = base
        .clone()
        .with_timeout(std::time::Duration::from_millis(1));
    assert_eq!(
        slow.identity(),
        fast.identity(),
        "timeout must never enter envelope identity"
    );

    let bounded = base
        .clone()
        .with_request_byte_limit(1024)
        .with_response_byte_limit(2048);
    assert_eq!(
        bounded.identity(),
        base.identity(),
        "opt-in byte limits must never enter envelope identity"
    );

    // The profile that DOES own the identity survives tuning, and rich vs
    // structural stay separated.
    let structural = base
        .clone()
        .with_profile(GrobidProfile::structural())
        .with_timeout(std::time::Duration::from_secs(600))
        .with_request_byte_limit(1024);
    assert_eq!(structural.identity().profile_fingerprint(), STRUCTURAL_FP);
    assert_ne!(structural.identity(), base.identity());
}

// ---------------------------------------------------------------------------
// S05/T01: endpoint result identity guards
// ---------------------------------------------------------------------------

/// The S05 endpoint result identities are deterministic, pairwise distinct
/// per profile, free of transport tuning/locations, and stamp on top of the
/// unchanged D204 identity without rotating any pre-S05 pin.
#[test]
fn s05_endpoint_result_identities_are_distinct_deterministic_and_free_of_tuning() {
    for profile in [GrobidProfile::rich(), GrobidProfile::structural()] {
        let fulltext = ParseBackendRequest::new("s05", Vec::new()).result_identity();
        let header = ParseBackendRequest::header_from_legacy_profile(&profile, "s05", Vec::new())
            .result_identity();
        let references =
            ParseBackendRequest::references_from_legacy_profile(&profile, "s05", Vec::new())
                .result_identity();

        // Pairwise distinct per profile; deterministic under repetition.
        assert_ne!(fulltext, header);
        assert_ne!(header, references);
        assert_ne!(fulltext, references);
        assert_eq!(
            ParseBackendRequest::header_from_legacy_profile(&profile, "again", Vec::new())
                .result_identity(),
            header
        );
        assert_eq!(
            ParseBackendRequest::references_from_legacy_profile(&profile, "again", Vec::new())
                .result_identity(),
            references
        );

        // The effective request is distinguishable: raw switches move only
        // their own endpoint identity.
        let raw_header = TeiResultIdentity::header(
            HeaderDialectV1::from_legacy_profile(&profile).with_include_raw_affiliations(true),
        );
        assert_ne!(raw_header, header);
        let raw_references = TeiResultIdentity::references(
            ReferencesDialectV1::from_legacy_profile(&profile).with_include_raw_citations(true),
        );
        assert_ne!(raw_references, references);

        for identity in [
            fulltext.canonical(),
            header.canonical(),
            references.canonical(),
            raw_header.canonical(),
            raw_references.canonical(),
        ] {
            for forbidden in ["http", "://", "timeout", "limit", "path", "cache", "url"] {
                assert!(
                    !identity.to_lowercase().contains(forbidden),
                    "identity `{identity}` must stay free of `{forbidden}`"
                );
            }
        }
    }

    // Stamping is additive over the byte-stable D204 fingerprints.
    for profile in [GrobidProfile::rich(), GrobidProfile::structural()] {
        let fingerprint = profile.configuration_fingerprint();
        let stamped = TeiIdentity::grobid_http(profile).with_result(TeiResultIdentity::header(
            HeaderDialectV1::from_legacy_profile(&profile),
        ));
        assert_eq!(stamped.backend_id(), GROBID_HTTP_BACKEND_ID);
        assert_eq!(stamped.profile_fingerprint(), fingerprint);
        assert!(stamped.result().is_some());
        assert_ne!(stamped, TeiIdentity::grobid_http(profile));
        // ...and the pre-S05 constructors keep owning no result component.
        assert!(TeiIdentity::grobid_http(profile).result().is_none());
        assert!(TeiIdentity::cached_tei(profile).result().is_none());
    }
}

// ---------------------------------------------------------------------------
// S05/T03: DATA-bound proxy identity regressions
// ---------------------------------------------------------------------------

/// The stamped endpoint result identity stays free of transport tuning AND
/// base URL: two backends differing only in URL/timeout/limits produce
/// identical header/references provenance, while the three endpoint
/// identities stay pairwise distinct, versioned-namespace bound and
/// collision-free with the legacy D204 fingerprint namespace (which remains
/// the ONLY fulltext cache identity for legacy presets).
#[test]
fn s05_proxy_result_identities_exclude_tuning_and_stay_d204_collision_free() {
    let tuned = GrobidHttpBackend::new(Some("http://127.0.0.99:65000"))
        .with_timeout(std::time::Duration::from_millis(1))
        .with_request_byte_limit(64)
        .with_response_byte_limit(128);
    let plain = GrobidHttpBackend::new(Some("http://127.0.0.1:1"));

    let proxies = [
        (
            "header",
            plain
                .default_header_request("p", Vec::new())
                .result_identity(),
            tuned
                .default_header_request("p", Vec::new())
                .result_identity(),
        ),
        (
            "references",
            plain
                .default_references_request("p", Vec::new())
                .result_identity(),
            tuned
                .default_references_request("p", Vec::new())
                .result_identity(),
        ),
    ];
    for (endpoint, from_plain, from_tuned) in proxies {
        assert_eq!(
            from_plain, from_tuned,
            "{endpoint}: base URL, timeout and byte limits must never enter the endpoint result identity"
        );
        assert_eq!(
            plain.identity().with_result(from_plain.clone()),
            tuned.identity().with_result(from_tuned),
            "{endpoint}: stamped provenance must be tuning- and base-URL-free"
        );
        let canonical = from_plain.canonical();
        assert!(
            canonical.starts_with(&format!("endpoint={endpoint};schema=v1;{endpoint}=v1;")),
            "{endpoint}: canonical identity must carry the versioned namespace: {canonical}"
        );
    }

    // Pairwise distinct across the three operations for the same profile,
    // collision-free with the D204 fingerprints and free of transport
    // tuning / location material.
    let structural = GrobidProfile::structural();
    let fulltext =
        ParseBackendRequest::from_legacy_profile(&structural, "p", Vec::new()).result_identity();
    let header = ParseBackendRequest::header_from_legacy_profile(&structural, "p", Vec::new())
        .result_identity();
    let references =
        ParseBackendRequest::references_from_legacy_profile(&structural, "p", Vec::new())
            .result_identity();
    assert_ne!(fulltext, header);
    assert_ne!(header, references);
    assert_ne!(fulltext, references);
    for identity in [
        fulltext.canonical(),
        header.canonical(),
        references.canonical(),
    ] {
        assert_ne!(identity, RICH_FP);
        assert_ne!(identity, STRUCTURAL_FP);
        for forbidden in ["http", "://", "timeout", "limit", "cache"] {
            assert!(
                !identity.to_lowercase().contains(forbidden),
                "identity `{identity}` must stay free of `{forbidden}`"
            );
        }
    }

    // Stamping keeps the byte-stable D204 fingerprint and backend id intact.
    let stamped = TeiIdentity::grobid_http(structural).with_result(header);
    assert_eq!(stamped.profile_fingerprint(), STRUCTURAL_FP);
    assert_eq!(stamped.backend_id(), GROBID_HTTP_BACKEND_ID);
}

// ═══════════════════════════════════════════════════════════════════════
// S06/T02: two-stage cache payload + stage-key identity gate
// ═══════════════════════════════════════════════════════════════════════

use da_parse::{
    CacheIntegrityCode, PROJECT_ARTICLE_SCHEMA_FINGERPRINT, PROJECT_ARTICLE_STAGE_ID,
    ParseCacheStage, ProjectArticlePayload, RAW_TEI_PAYLOAD_SCHEMA_VERSION, RAW_TEI_STAGE_ID,
    RawTeiEnvelopePayload, project_article_stage_key, raw_tei_stage_key,
};

fn s06_fulltext_request(paper: &str, bytes: Vec<u8>) -> da_parse::ParseBackendRequest {
    da_parse::ParseBackendRequest::with_dialect(paper, bytes, FulltextDialectV1::legacy_rich())
}

fn s06_envelope(
    tei: &str,
    dialect: FulltextDialectV1,
) -> (da_parse::TeiEnvelope, da_parse::TeiResultIdentity) {
    let identity = da_parse::TeiIdentity::new("identity-gate-backend", RICH_FP);
    let result = da_parse::TeiResultIdentity::fulltext(dialect);
    (
        da_parse::TeiEnvelope::new(tei, identity.clone().with_result(result.clone())),
        result,
    )
}

#[test]
fn s06_raw_stage_payload_identity_is_versioned_and_verbatim() {
    let (envelope, result) = s06_envelope(
        "<TEI>verbatim bytes</TEI>",
        FulltextDialectV1::legacy_rich(),
    );
    let payload = RawTeiEnvelopePayload::build(&envelope, &result);
    let wire = serde_json::to_value(&payload).unwrap();
    for field in [
        "schema_version",
        "stage_id",
        "backend_id",
        "profile_fingerprint",
        "result_identity",
        "payload_sha256",
        "tei_xml_hex",
    ] {
        assert!(
            wire.get(field).is_some(),
            "raw payload wire lost `{field}`: {wire}"
        );
    }
    assert_eq!(
        wire["schema_version"],
        json!(RAW_TEI_PAYLOAD_SCHEMA_VERSION),
        "raw payload schema version drifted"
    );
    assert_eq!(wire["stage_id"], json!(RAW_TEI_STAGE_ID));
    assert_eq!(wire["backend_id"], json!("identity-gate-backend"));
    assert_eq!(wire["profile_fingerprint"], json!(RICH_FP));
    assert_eq!(
        wire["result_identity"],
        json!(result.canonical()),
        "the stamped endpoint result identity must ride the payload"
    );
    // Verbatim bytes: hex decode reproduces the TEI byte-exactly.
    let decoded = hex::decode(wire["tei_xml_hex"].as_str().unwrap()).unwrap();
    assert_eq!(decoded, b"<TEI>verbatim bytes</TEI>");
    assert_eq!(wire["payload_sha256"], json!(sha256_hex(&decoded)));

    let (verified, sha) =
        RawTeiEnvelopePayload::decode_and_verify(&serde_json::to_vec(&payload).unwrap(), &result)
            .expect("intact payload verifies");
    assert_eq!(verified.raw_tei(), b"<TEI>verbatim bytes</TEI>");
    assert_eq!(sha, sha256_hex(b"<TEI>verbatim bytes</TEI>"));

    // Foreign schema version fails closed (corruption, not a miss).
    let mut foreign = wire.clone();
    foreign["schema_version"] = json!(RAW_TEI_PAYLOAD_SCHEMA_VERSION + 1);
    let error =
        RawTeiEnvelopePayload::decode_and_verify(&serde_json::to_vec(&foreign).unwrap(), &result)
            .unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::ManifestCorrupt);
    assert_eq!(error.stage(), ParseCacheStage::RawTei);
}

#[test]
fn s06_stage_keys_are_content_addressed_and_schema_bound() {
    let bytes_a = b"source-a".to_vec();
    let base = s06_fulltext_request("paper-key", bytes_a.clone());
    let raw_key = raw_tei_stage_key(&base);
    assert!(raw_key.starts_with(RAW_TEI_STAGE_ID), "{raw_key}");
    assert_eq!(raw_key.len(), RAW_TEI_STAGE_ID.len() + 1 + 64);

    // Content-bound: bytes and effective dialect change the key.
    let bytes_changed = s06_fulltext_request("paper-key", b"source-b".to_vec());
    assert_ne!(
        raw_key,
        raw_tei_stage_key(&bytes_changed),
        "source bytes are raw content"
    );
    let structural = s06_fulltext_request("paper-key", bytes_a.clone());
    let structural = {
        // Same bytes under the structural legacy preset are a different
        // effective dialect (different TEI), so the raw key must differ.
        let _ = structural;
        let req = da_parse::ParseBackendRequest::with_dialect(
            "paper-key",
            bytes_a.clone(),
            FulltextDialectV1::legacy_structural(),
        );
        raw_tei_stage_key(&req)
    };
    assert_ne!(
        raw_key, structural,
        "rich vs structural are distinct raw content"
    );

    // Logical paper ids do NOT affect the raw key: raw TEI depends on the
    // request content, not the id.
    let other_id = s06_fulltext_request("other-paper", bytes_a.clone());
    assert_eq!(raw_key, raw_tei_stage_key(&other_id));

    // The projector key binds every projection dependency.
    let proj = |paper: &str, pdf: &str, raw: &str, fp: &str| {
        project_article_stage_key(paper, pdf, raw, fp)
    };
    let reference = proj(
        "paper-key",
        "pdf-sha",
        "raw-sha",
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    );
    assert!(
        reference.starts_with(PROJECT_ARTICLE_STAGE_ID),
        "{reference}"
    );
    assert_ne!(
        reference,
        proj(
            "other-paper",
            "pdf-sha",
            "raw-sha",
            PROJECT_ARTICLE_SCHEMA_FINGERPRINT
        ),
        "paper ids ARE projection content"
    );
    assert_ne!(
        reference,
        proj(
            "paper-key",
            "other-pdf",
            "raw-sha",
            PROJECT_ARTICLE_SCHEMA_FINGERPRINT
        )
    );
    assert_ne!(
        reference,
        proj(
            "paper-key",
            "pdf-sha",
            "other-raw",
            PROJECT_ARTICLE_SCHEMA_FINGERPRINT
        ),
        "the verified raw payload SHA is a projection dependency"
    );
    assert_ne!(
        reference,
        proj("paper-key", "pdf-sha", "raw-sha", "ParsedArticle@rotated"),
        "a projector schema fix orphans projection entries without touching raw"
    );
}

#[test]
fn s06_raw_payload_verification_maps_integrity_codes() {
    let (envelope, result) = s06_envelope("<TEI>map</TEI>", FulltextDialectV1::legacy_rich());
    let good = RawTeiEnvelopePayload::build(&envelope, &result).encode();

    // Junk wrapper → manifest corrupt.
    let error = RawTeiEnvelopePayload::decode_and_verify(b"{junk", &result).unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::ManifestCorrupt);

    // Undecodable hex payload → payload unreadable.
    let mut unreadable: serde_json::Value = serde_json::from_slice(&good).unwrap();
    unreadable["tei_xml_hex"] = json!("not-hex-!");
    let error = RawTeiEnvelopePayload::decode_and_verify(
        &serde_json::to_vec(&unreadable).unwrap(),
        &result,
    )
    .unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::PayloadUnreadable);

    // Byte drift → checksum mismatch.
    let mut drifted: serde_json::Value = serde_json::from_slice(&good).unwrap();
    let hex_payload = drifted["tei_xml_hex"].as_str().unwrap().to_string();
    let mut bytes = hex_payload.into_bytes();
    let last = bytes.len() - 1;
    bytes[last] = if bytes[last] == b'0' { b'1' } else { b'0' };
    drifted["tei_xml_hex"] = json!(String::from_utf8(bytes).unwrap());
    let error =
        RawTeiEnvelopePayload::decode_and_verify(&serde_json::to_vec(&drifted).unwrap(), &result)
            .unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::ChecksumMismatch);

    // A payload produced for a different endpoint result → identity mismatch.
    // Fulltext rich/structural intentionally share one result schema identity;
    // tuning differences belong to the request/cache key, not result shape.
    let foreign = da_parse::TeiResultIdentity::header(HeaderDialectV1::legacy_rich());
    let error = RawTeiEnvelopePayload::decode_and_verify(&good, &foreign).unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::IdentityMismatch);
}

#[test]
fn s06_projector_payload_binds_verified_dependencies() {
    let (envelope, result) = s06_envelope("<TEI>proj</TEI>", FulltextDialectV1::legacy_rich());
    let (verified, raw_sha) = RawTeiEnvelopePayload::decode_and_verify(
        &RawTeiEnvelopePayload::build(&envelope, &result).encode(),
        &result,
    )
    .unwrap();
    let article = da_parse::lift_and_project_envelope("proj-paper", "pdf-hash", &verified);

    let build = || {
        ProjectArticlePayload::build(
            &article,
            PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
            &raw_sha,
            "proj-paper",
            "pdf-sha",
        )
    };
    let restored = ProjectArticlePayload::decode_and_verify(
        &build().encode(),
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        &raw_sha,
        "proj-paper",
        "pdf-sha",
    )
    .expect("intact projection payload verifies");
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(&article).unwrap(),
        "the cached projection is wire-equal to the fresh one"
    );

    // Foreign fingerprint → manifest corrupt.
    let error = ProjectArticlePayload::decode_and_verify(
        &build().encode(),
        "ParsedArticle@rotated",
        &raw_sha,
        "proj-paper",
        "pdf-sha",
    )
    .unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::ManifestCorrupt);
    assert_eq!(error.stage(), ParseCacheStage::ProjectArticle);

    // Different verified raw payload SHA / pdf sha / paper id → identity mismatch.
    for (raw, pdf, paper) in [
        ("other-raw", "pdf-sha", "proj-paper"),
        ("raw-sha", "other-pdf", "proj-paper"),
        ("raw-sha", "pdf-sha", "other-paper"),
    ] {
        let error = ProjectArticlePayload::decode_and_verify(
            &build().encode(),
            PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
            raw,
            paper,
            pdf,
        )
        .unwrap_err();
        assert_eq!(error.code(), CacheIntegrityCode::IdentityMismatch);
    }

    // Interpretable wrapper, undecodable article → payload unreadable.
    let mut scalar: serde_json::Value = serde_json::from_slice(&build().encode()).unwrap();
    scalar["article"] = json!(42);
    let error = ProjectArticlePayload::decode_and_verify(
        &serde_json::to_vec(&scalar).unwrap(),
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        &raw_sha,
        "proj-paper",
        "pdf-sha",
    )
    .unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::PayloadUnreadable);

    // Junk wrapper → manifest corrupt.
    let error = ProjectArticlePayload::decode_and_verify(
        b"{junk",
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        &raw_sha,
        "proj-paper",
        "pdf-sha",
    )
    .unwrap_err();
    assert_eq!(error.code(), CacheIntegrityCode::ManifestCorrupt);
}

// ───── S06/T05 batch/cache vocabulary byte pins for the DATA ledger ─────

#[test]
fn s06_batch_cache_vocabulary_stays_byte_pinned_for_the_data_ledger() {
    use da_parse::{
        CacheMode, CacheStageDisposition, PARSE_BATCH_CURSOR_SCHEMA_VERSION,
        PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION, PARSE_BATCH_SCHEMA_VERSION,
        PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION,
    };

    // Stage identities + payload schema versions: exactly the strings the
    // data/parse ledgers declare (S06/T05) and the storage adapter (T03)
    // namespaces by. Any drift here rotates the cache identity — a
    // coordinated D204-grade decision, never a silent rename.
    assert_eq!(RAW_TEI_STAGE_ID, "parse.raw_tei.v1");
    assert_eq!(PROJECT_ARTICLE_STAGE_ID, "parse.project_article.v1");
    assert_eq!(RAW_TEI_PAYLOAD_SCHEMA_VERSION, 1);
    assert_eq!(PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION, 1);
    assert_eq!(
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        "ParsedArticle@serde-v4-p0-tei-lift"
    );
    assert_eq!(ParseCacheStage::RawTei.stage_id(), RAW_TEI_STAGE_ID);
    assert_eq!(
        ParseCacheStage::ProjectArticle.stage_id(),
        PROJECT_ARTICLE_STAGE_ID
    );

    // Closed four-mode vocabulary: kebab-case wire names plus the exact
    // truth-table rows the ledgers pin (refresh bypasses hits — reads stay
    // false because the stored value is never consulted).
    let mode_rows: [(CacheMode, &str, bool, bool, bool); 4] = [
        (CacheMode::Off, "off", false, false, false),
        (CacheMode::ReadOnly, "read-only", true, false, false),
        (CacheMode::WriteThrough, "write-through", true, true, false),
        (CacheMode::Refresh, "refresh", false, true, true),
    ];
    for (mode, wire, reads, publishes, bypasses) in mode_rows {
        assert_eq!(mode.as_str(), wire, "{wire}");
        assert_eq!(mode.reads_stages(), reads, "{wire}");
        assert_eq!(mode.publishes_results(), publishes, "{wire}");
        assert_eq!(mode.bypasses_hits(), bypasses, "{wire}");
    }

    // Closed per-stage disposition vocabulary (serde snake_case wire forms).
    assert_eq!(CacheStageDisposition::Hit.as_str(), "hit");
    assert_eq!(
        CacheStageDisposition::ComputedPublished.as_str(),
        "computed_published"
    );
    assert_eq!(
        CacheStageDisposition::ComputedUnpublished.as_str(),
        "computed_unpublished"
    );
    assert_eq!(
        CacheStageDisposition::ComputedUncached.as_str(),
        "computed_uncached"
    );

    // Closed integrity-code vocabulary — the terminal per-document taxonomy
    // in the DATA ledger pins exactly these five snake_case names.
    let integrity_rows = [
        (CacheIntegrityCode::ChecksumMismatch, "checksum_mismatch"),
        (CacheIntegrityCode::ManifestCorrupt, "manifest_corrupt"),
        (CacheIntegrityCode::IdentityMismatch, "identity_mismatch"),
        (CacheIntegrityCode::PayloadUnreadable, "payload_unreadable"),
        (CacheIntegrityCode::StageWriteFailed, "stage_write_failed"),
    ];
    for (code, wire) in integrity_rows {
        assert_eq!(code.as_str(), wire);
    }

    // Schema versions of the batch / cursor / run-summary vocabularies.
    assert_eq!(PARSE_BATCH_SCHEMA_VERSION, 1);
    assert_eq!(PARSE_BATCH_CURSOR_SCHEMA_VERSION, 1);
    assert_eq!(PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION, 1);
}

// ───── S09/T05: the native personal-name shadow candidate never touches cache identity ─────

#[test]
fn s09_native_personal_name_shadow_stays_outside_cache_and_conservation_identity() {
    use da_parse::{
        CACHED_TEI_BACKEND_ID, GROBID_HTTP_BACKEND_ID, GrobidProfile,
        NATIVE_PERSONAL_NAME_OUTCOME_SCHEMA_VERSION, NATIVE_PERSONAL_NAME_PARSER_ID,
        NATIVE_PERSONAL_NAME_PARSER_VERSION, PROJECT_ARTICLE_STAGE_ID, RAW_TEI_STAGE_ID,
        project_article_stage_key,
    };

    assert_eq!(
        NATIVE_PERSONAL_NAME_PARSER_ID,
        "da-parse.native_personal_name"
    );
    assert_eq!(NATIVE_PERSONAL_NAME_PARSER_VERSION, "1.0.0");
    assert_eq!(
        NATIVE_PERSONAL_NAME_OUTCOME_SCHEMA_VERSION,
        "da-parse.native-personal-name.outcome/v1"
    );
    assert_eq!(RAW_TEI_STAGE_ID, "parse.raw_tei.v1");
    assert_eq!(PROJECT_ARTICLE_STAGE_ID, "parse.project_article.v1");
    assert_eq!(
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        "ParsedArticle@serde-v4-p0-tei-lift"
    );
    assert_eq!(GROBID_HTTP_BACKEND_ID, "grobid-http");
    assert_eq!(CACHED_TEI_BACKEND_ID, "cached-tei");
    assert_eq!(GrobidProfile::rich().configuration_fingerprint(), RICH_FP);
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        STRUCTURAL_FP
    );

    let key = project_article_stage_key(
        "reg.tei.hub-bibliography-2507.19457",
        &"a".repeat(64),
        &"b".repeat(64),
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    );
    assert!(!key.contains("native_personal_name") && !key.contains("PersonalName"));

    let baseline = load_baseline();
    let cases = baseline_cases(&baseline);
    assert_eq!(
        cases.len(),
        3,
        "native name evidence must not alter executed-case population"
    );
    for case in &cases {
        assert!(case.get("native_personal_name").is_none());
        assert!(case.get("native_personal_name_shadow").is_none());
        assert!(case.get("conservation").is_some());
    }
    let shadow = baseline
        .get("native_personal_name_shadow")
        .expect("baseline must carry additive native personal-name evidence");
    for (field, expected) in [
        ("candidate_id", NATIVE_PERSONAL_NAME_PARSER_ID),
        ("candidate_version", NATIVE_PERSONAL_NAME_PARSER_VERSION),
        (
            "outcome_schema_version",
            NATIVE_PERSONAL_NAME_OUTCOME_SCHEMA_VERSION,
        ),
        ("observation_mode", "shadow"),
        ("promotion_state", "candidate"),
        ("rollback_target", "java-grobid-http@0.9.0-crf"),
    ] {
        assert_eq!(
            shadow.get(field).and_then(|v| v.as_str()),
            Some(expected),
            "shadow field {field} drifted"
        );
    }
    assert_eq!(
        baseline
            .pointer("/policy_locks/import_eligible")
            .and_then(|v| v.as_bool()),
        Some(false),
        "candidate evidence cannot open the D127 import lock"
    );
    assert_eq!(
        baseline
            .pointer("/report_identity/promotion_state")
            .and_then(|v| v.as_str()),
        Some("candidate")
    );
    assert!(
        baseline.get("conservation").is_some(),
        "native name evidence must remain additive and preserve D299 conservation"
    );
}

// ───── S08/T04: the native date shadow candidate never touches cache identity ─────

#[test]
fn s08_native_date_shadow_stays_outside_cache_identity_and_d204_pins() {
    use da_parse::{
        CACHED_TEI_BACKEND_ID, GROBID_HTTP_BACKEND_ID, GrobidProfile,
        NATIVE_DATE_OUTCOME_SCHEMA_VERSION, NATIVE_DATE_PARSER_ID, NATIVE_DATE_PARSER_VERSION,
        PROJECT_ARTICLE_STAGE_ID, RAW_TEI_STAGE_ID, project_article_stage_key,
    };

    // Candidate identity: the exact code truth the evidence layer binds.
    assert_eq!(NATIVE_DATE_PARSER_ID, "da-parse.native_date");
    assert_eq!(NATIVE_DATE_PARSER_VERSION, "1.0.0");
    assert_eq!(
        NATIVE_DATE_OUTCOME_SCHEMA_VERSION,
        "da-parse.native-date.outcome/v1"
    );

    // Stage identities + projection fingerprint: byte-frozen with the
    // candidate in the tree (identity rotation is a coordinated D204-grade
    // decision, never a component-addition side effect).
    assert_eq!(RAW_TEI_STAGE_ID, "parse.raw_tei.v1");
    assert_eq!(PROJECT_ARTICLE_STAGE_ID, "parse.project_article.v1");
    assert_eq!(
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        "ParsedArticle@serde-v4-p0-tei-lift"
    );
    assert_eq!(GROBID_HTTP_BACKEND_ID, "grobid-http");
    assert_eq!(CACHED_TEI_BACKEND_ID, "cached-tei");
    assert_eq!(GrobidProfile::rich().configuration_fingerprint(), RICH_FP);
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        STRUCTURAL_FP
    );

    // The projector stage key stays bound to exactly its four declared
    // inputs and never absorbs a native-date term: keys for a fixed sample
    // stay byte-stable across calls, carry no candidate token, and remain
    // content-sensitive in every declared input.
    let sample = (
        "reg.tei.hub-bibliography-2507.19457",
        "a".repeat(32),
        "b".repeat(32),
    );
    let key_a = project_article_stage_key(
        sample.0,
        &sample.1,
        &sample.2,
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    );
    let key_b = project_article_stage_key(
        sample.0,
        &sample.1,
        &sample.2,
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    );
    assert_eq!(key_a, key_b, "stage key must stay deterministic");
    assert!(
        !key_a.contains("native_date") && !key_a.contains("NativeDate"),
        "the projector stage key must stay candidate-free"
    );
    assert_ne!(
        key_a,
        project_article_stage_key(
            "reg.tei.rlm-2512.24601",
            &sample.1,
            &sample.2,
            PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        )
    );
    assert_ne!(
        key_a,
        project_article_stage_key(
            sample.0,
            &"c".repeat(32),
            &sample.2,
            PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        )
    );
    assert_ne!(
        key_a,
        project_article_stage_key(
            sample.0,
            &sample.1,
            &"d".repeat(32),
            PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        )
    );

    // Historical GrobidParser readability pins survive unchanged.
    for rel in [
        "crates/da-cli/src/production.rs",
        "crates/da-adapters/src/pipeline_stages.rs",
    ] {
        let text =
            String::from_utf8(read_bytes(rel)).unwrap_or_else(|_| panic!("{rel} must be utf-8"));
        assert!(
            text.contains("GrobidParser@tei-navigation-v3"),
            "historical GrobidParser identity pin disappeared from {rel}"
        );
    }

    // The baseline keeps the additive shadow block OUTSIDE the executed-case
    // identity surface: exactly the three S01-bound cases, none carrying a
    // native-date field, and the aggregate shadow evidence complete,
    // same-n and digest-bound (173/173 strict).
    let baseline = load_baseline();
    let cases = baseline_cases(&baseline);
    assert_eq!(
        cases.len(),
        3,
        "executed-case surface must stay exactly the three S01-bound cases"
    );
    for case in &cases {
        assert!(
            case.get("native_date_shadow").is_none() && case.get("native_date").is_none(),
            "a native-date field leaked into an executed case identity — shadow evidence must stay additive"
        );
    }
    let shadow = baseline
        .get("native_date_shadow")
        .expect("baseline must carry the additive native_date_shadow block");
    assert_eq!(
        shadow.get("engine").and_then(|v| v.as_str()),
        Some("da_parse::NativeDateParser")
    );
    assert_eq!(
        shadow.get("candidate_id").and_then(|v| v.as_str()),
        Some("da-parse.native_date")
    );
    assert_eq!(
        shadow.get("candidate_version").and_then(|v| v.as_str()),
        Some("1.0.0")
    );
    assert_eq!(
        shadow
            .get("outcome_schema_version")
            .and_then(|v| v.as_str()),
        Some("da-parse.native-date.outcome/v1")
    );
    assert_eq!(
        shadow.get("observation_mode").and_then(|v| v.as_str()),
        Some("shadow")
    );
    assert_eq!(
        shadow.get("promotion_state").and_then(|v| v.as_str()),
        Some("candidate")
    );
    assert_eq!(
        shadow.get("rollback_target").and_then(|v| v.as_str()),
        Some("java-grobid-http@0.9.0-crf")
    );
    for (field, expected) in [
        ("denominator_total", 173),
        ("oracle_n", 173),
        ("candidate_n", 173),
        ("strict_n", 173),
        ("invented_component_count", 0),
        ("malformed_count", 0),
        ("unsupported_count", 0),
    ] {
        assert_eq!(
            u64_field(shadow, &format!("/{field}")),
            expected,
            "shadow aggregate field {field} drifted"
        );
    }
    assert_eq!(
        shadow.get("same_n").and_then(|v| v.as_bool()),
        Some(true),
        "strict same-n parity is the stopping gate of the shadow denominator"
    );
    let digest = shadow
        .get("aggregate_digest")
        .and_then(|v| v.as_str())
        .expect("aggregate digest");
    let digest_hex = digest.strip_prefix("sha256:").unwrap_or(digest);
    assert_eq!(
        digest_hex.len(),
        64,
        "aggregate digest must be sha-256 hex (optionally sha256:-prefixed): {digest}"
    );
    assert!(
        digest_hex.bytes().all(|b| b.is_ascii_hexdigit()),
        "aggregate digest must be hex: {digest}"
    );
    // The digest is bound to the exact per-occurrence rows: the row vector
    // carries exactly the shadow denominator as identity/hash evidence only —
    // and never any raw display text (redaction is a hard regression).
    let rows = shadow
        .get("per_occurrence")
        .and_then(|v| v.as_array())
        .expect("per_occurrence rows");
    assert_eq!(
        rows.len(),
        173,
        "per-occurrence row vector must equal the shadow denominator"
    );
    for row in rows {
        for field in [
            "case_id",
            "fixture_id",
            "ordinal",
            "span",
            "scope",
            "input_sha256",
            "oracle_when_sha256",
            "outcome",
            "precision",
        ] {
            assert!(
                row.get(field).is_some(),
                "per-occurrence row lacks identity field `{field}`"
            );
        }
        for banned in ["when_display", "source_text", "tei_fragment", "display"] {
            assert!(
                row.get(banned).is_none(),
                "per-occurrence row leaked raw-text field `{banned}`"
            );
        }
    }
}

// ───── S10/T06: native affiliation shadow cannot rotate production identity ─────

#[test]
fn s10_native_affiliation_shadow_stays_outside_cache_and_conservation_identity() {
    use da_parse::{
        NATIVE_AFFILIATION_LINK_SCHEMA_VERSION, NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION,
        NATIVE_AFFILIATION_PARSER_ID, NATIVE_AFFILIATION_PARSER_VERSION,
    };

    assert_eq!(NATIVE_AFFILIATION_PARSER_ID, "da-parse.native_affiliation");
    assert_eq!(NATIVE_AFFILIATION_PARSER_VERSION, "1.0.0");
    assert_eq!(
        NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION,
        "da-parse.native-affiliation.outcome/v1"
    );
    assert_eq!(
        NATIVE_AFFILIATION_LINK_SCHEMA_VERSION,
        "da-parse.native-affiliation-link.outcome/v1"
    );
    assert_eq!(RAW_TEI_STAGE_ID, "parse.raw_tei.v1");
    assert_eq!(PROJECT_ARTICLE_STAGE_ID, "parse.project_article.v1");
    assert_eq!(
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        "ParsedArticle@serde-v4-p0-tei-lift"
    );
    assert_eq!(GrobidProfile::rich().configuration_fingerprint(), RICH_FP);
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        STRUCTURAL_FP
    );

    let key = project_article_stage_key(
        "reg.tei.hub-bibliography-2507.19457",
        &"a".repeat(32),
        &"b".repeat(32),
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    );
    assert!(!key.contains("native_affiliation") && !key.contains("Affiliation"));

    let baseline = load_baseline();
    let shadow = baseline
        .get("native_affiliation_shadow")
        .expect("affiliation evidence is additive, not a case identity field");
    for (field, expected) in [
        ("candidate_id", NATIVE_AFFILIATION_PARSER_ID),
        ("candidate_version", NATIVE_AFFILIATION_PARSER_VERSION),
        (
            "outcome_schema_version",
            NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION,
        ),
        ("observation_mode", "shadow"),
        ("promotion_state", "candidate"),
        ("auto_promote", "forbidden"),
    ] {
        assert_eq!(shadow.get(field).and_then(|v| v.as_str()), Some(expected));
    }
    assert_eq!(
        shadow.get("import_eligible").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        shadow.get("address_observed_n").and_then(|v| v.as_u64()),
        Some(0)
    );
    for section in ["recognition", "linking"] {
        let section = shadow
            .get(section)
            .expect("recognition/linking are separate");
        assert_eq!(section.get("oracle_n").and_then(|v| v.as_u64()), Some(13));
        assert_eq!(
            section.get("candidate_n").and_then(|v| v.as_u64()),
            Some(13)
        );
        assert_eq!(section.get("same_n").and_then(|v| v.as_bool()), Some(true));
        for field in [
            "lost_count",
            "extra_count",
            "reordered_count",
            "invented_count",
        ] {
            assert_eq!(
                section.get(field).and_then(|v| v.as_u64()),
                Some(0),
                "{section} drifted: {field}"
            );
        }
    }
    assert!(
        baseline.get("conservation").is_some(),
        "D299 conservation must remain additive"
    );
    assert_eq!(
        baseline
            .pointer("/policy_locks/import_eligible")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        baseline
            .pointer("/report_identity/promotion_state")
            .and_then(|v| v.as_str()),
        Some("candidate")
    );
}

// ───── S12/T05: composition shadow is additive, redacted, and identity-neutral ─────
#[test]
fn s12_native_composition_shadow_stays_top_level_and_quarantined() {
    let baseline = load_baseline();
    let shadow = baseline
        .get("native_header_reference_composition_shadow")
        .expect("composition evidence must be present as one top-level sibling");
    assert_eq!(
        shadow.get("candidate_id").and_then(|v| v.as_str()),
        Some("da-parse.native_header_reference_composition")
    );
    assert_eq!(
        shadow.get("candidate_version").and_then(|v| v.as_str()),
        Some("1.0.0")
    );
    assert_eq!(
        shadow
            .get("outcome_schema_version")
            .and_then(|v| v.as_str()),
        Some("da-parse.native-composition.outcome/v1")
    );
    assert_eq!(
        shadow.get("observation_mode").and_then(|v| v.as_str()),
        Some("shadow")
    );
    assert_eq!(
        shadow.get("promotion_state").and_then(|v| v.as_str()),
        Some("candidate")
    );
    assert_eq!(
        shadow.get("auto_promote").and_then(|v| v.as_str()),
        Some("forbidden")
    );
    assert_eq!(
        shadow.get("independently_selectable_lanes"),
        Some(&json!(["header", "references"]))
    );
    assert_eq!(
        shadow.pointer("/rollback/header").and_then(|v| v.as_str()),
        Some("java-grobid-http@0.9.0-crf")
    );
    assert_eq!(
        shadow
            .pointer("/rollback/references")
            .and_then(|v| v.as_str()),
        Some("java-grobid-http@0.9.0-crf")
    );

    for (pointer, expected) in [
        ("/component_receipts/date/total", 173),
        ("/component_receipts/header/raw_total", 36),
        ("/component_receipts/header/accepted_total", 35),
        ("/component_receipts/header/rejected_or_incomplete", 1),
        ("/component_receipts/citation_names/total", 2198),
        ("/component_receipts/citation_names/unsupported_n", 2),
        ("/component_receipts/affiliation/recognition_total", 13),
        ("/component_receipts/affiliation/linking_total", 13),
        ("/component_receipts/affiliation/zero_link_authors", 25),
        ("/component_receipts/references/total", 207),
        ("/component_receipts/anchors/total", 380),
        ("/component_receipts/anchors/targeted", 217),
        ("/component_receipts/anchors/targetless", 163),
        ("/component_receipts/anchors/fragmented_adjacency", 168),
    ] {
        assert_eq!(
            shadow.pointer(pointer).and_then(|v| v.as_u64()),
            Some(expected),
            "receipt drift at {pointer}"
        );
    }
    assert_eq!(
        shadow
            .pointer("/component_receipts/denominator_policy")
            .and_then(|v| v.as_str()),
        Some("no_pooling_or_erosion")
    );
    assert_eq!(
        baseline
            .pointer("/policy_locks/import_eligible")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        baseline
            .pointer("/policy_locks/graph_mutation")
            .and_then(|v| v.as_str()),
        Some("forbidden")
    );
    assert_eq!(
        baseline
            .pointer("/report_identity/promotion_state")
            .and_then(|v| v.as_str()),
        Some("candidate")
    );
    assert_eq!(
        baseline
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| k.as_str() == "native_header_reference_composition_shadow")
            .count(),
        1
    );
    for case in baseline_cases(&baseline) {
        assert!(
            case.get("native_header_reference_composition_shadow")
                .is_none()
        );
        assert!(case.get("native_composition").is_none());
    }
    for banned in [
        "The title of this article",
        "raw scholarly paragraph",
        "<teiHeader>",
        "Example University",
        "Alice Smith",
        "doi:10.",
        "[1] Reference text",
    ] {
        assert!(
            !shadow.to_string().contains(banned),
            "composition shadow leaked `{banned}`"
        );
    }
    let key = project_article_stage_key(
        "reg.tei.hub-bibliography-2507.19457",
        &"a".repeat(32),
        &"b".repeat(32),
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    );
    assert!(!key.contains("native_composition"));
    assert_eq!(GrobidProfile::rich().configuration_fingerprint(), RICH_FP);
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        STRUCTURAL_FP
    );
}

#[test]
fn s11_native_citation_shadow_stays_outside_cache_identity_and_policy_locks() {
    use da_parse::{
        NATIVE_CITATION_ANCHOR_SCANNER_ID, NATIVE_CITATION_OUTCOME_SCHEMA_VERSION,
        NATIVE_CITATION_PARSER_ID, NATIVE_CITATION_PARSER_VERSION, NATIVE_REFERENCE_SEGMENTER_ID,
    };

    assert_eq!(NATIVE_CITATION_PARSER_ID, "da-parse.native_citation");
    assert_eq!(NATIVE_CITATION_PARSER_VERSION, "1.0.0");
    assert_eq!(
        NATIVE_CITATION_OUTCOME_SCHEMA_VERSION,
        "da-parse.native-citation.outcome/v1"
    );
    assert_eq!(
        NATIVE_REFERENCE_SEGMENTER_ID,
        "da-parse.native_reference_segmenter"
    );
    assert_eq!(
        NATIVE_CITATION_ANCHOR_SCANNER_ID,
        "da-parse.native_citation_anchor_scanner"
    );
    assert_eq!(RAW_TEI_STAGE_ID, "parse.raw_tei.v1");
    assert_eq!(PROJECT_ARTICLE_STAGE_ID, "parse.project_article.v1");
    assert_eq!(
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        "ParsedArticle@serde-v4-p0-tei-lift"
    );
    assert_eq!(GrobidProfile::rich().configuration_fingerprint(), RICH_FP);
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        STRUCTURAL_FP
    );

    let key = project_article_stage_key(
        "reg.tei.hub-bibliography-2507.19457",
        &"a".repeat(32),
        &"b".repeat(32),
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    );
    assert!(
        !key.contains("native_citation")
            && !key.contains("NativeCitation")
            && !key.contains("reference_segmenter")
            && !key.contains("anchor_scanner"),
        "citation shadow identity must not enter the production stage key"
    );

    let baseline = load_baseline();
    let shadow = baseline
        .get("native_citation_shadow")
        .expect("citation evidence must be present as one additive shadow block");
    for (field, expected) in [
        ("candidate_id", NATIVE_CITATION_PARSER_ID),
        ("candidate_version", NATIVE_CITATION_PARSER_VERSION),
        (
            "outcome_schema_version",
            NATIVE_CITATION_OUTCOME_SCHEMA_VERSION,
        ),
        ("observation_mode", "shadow"),
        ("promotion_state", "candidate"),
        ("rollback_target", "java-grobid-http@0.9.0-crf"),
    ] {
        assert_eq!(shadow.get(field).and_then(|v| v.as_str()), Some(expected));
    }
    assert_eq!(
        baseline
            .pointer("/policy_locks/import_eligible")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        baseline
            .pointer("/policy_locks/llm_promotion")
            .and_then(|v| v.as_str()),
        Some("forbidden")
    );
    assert!(
        baseline.get("conservation").is_some(),
        "D299 conservation must remain additive"
    );

    let cases = baseline_cases(&baseline);
    assert_eq!(
        cases.len(),
        3,
        "citation shadow must not alter executed-case population"
    );
    for case in cases {
        assert!(case.get("native_citation_shadow").is_none());
        assert!(case.get("native_citation").is_none());
        assert!(case.get("conservation").is_some());
    }

    for banned in [
        "raw_reference",
        "source_text",
        "citation_text",
        "anchor_quote",
        "tei_fragment",
        "source_text",
        "citation_text",
        "anchor_quote",
    ] {
        assert!(
            !shadow.to_string().contains(banned),
            "redacted shadow leaked `{banned}`"
        );
    }
}
