//! Cache-identity regression gate for the scholarly parse plane
//! (M464-z84in2 / S02 / T04).
//!
//! Proves the S02 seam extraction (ParseBackend / TeiEnvelope / TeiLifter /
//! ScholarlyParser) did NOT rotate any production identity:
//!
//! - D204 profile fingerprints (`GrobidProfile::configuration_fingerprint`)
//!   stay byte-stable and keep separating rich from structural; envelope
//!   identity rides the exact same strings, so envelope and cache identity
//!   cannot drift apart and transport tuning never leaks in;
//! - the frozen S01 baseline identities in the tracked
//!   `artifacts/parse_compat/baseline.json` (per-case output hashes,
//!   aggregate output/source hashes, corpus hash, conservation totals) are
//!   reproduced EXACTLY by the new composition path (CachedTeiBackend →
//!   ScholarlyParser → TeiLifter → projection) over the same preserved
//!   fixtures — cached TEI is lifted without any HTTP and yields the S01
//!   identity byte-for-byte;
//! - the bounded legacy/production cache-identity strings production relies
//!   on (`da_adapters::GrobidParser@load-v1`, `ParsedArticle.v1`, the legacy
//!   `pdf-grobid` lookup, `GrobidParser@tei-navigation-v3`,
//!   `ParsedArticle@serde-v3-unique-navigation`, the GROBID model
//!   fingerprint) are still pinned in their source surfaces;
//! - the baseline report keeps honest typed absences (the structural
//!   companion is reported, never fabricated) and never carries raw
//!   document content.
//!
//! Scope guards: no network, no GROBID, no graph store, no wall-clock
//! inputs. Fixtures and the baseline report are git-tracked files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use da_parse::{
    CachedTeiBackend, GROBID_HTTP_BACKEND_ID, GrobidProfile, ScholarlyParser, TeiEnvelope,
    TeiIdentity, TeiLifter,
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

/// Per-case output hashes frozen at S01. Any change means the lift/projection
/// output identity rotated and must be a conscious, coordinated decision —
/// never a refactor side effect.
const FROZEN_CASE_OUTPUT_HASHES: [(&str, &str); 3] = [
    (
        "reg.tei.hub-bibliography-2507.19457",
        "sha256:f621bc87809a930caf39816aace46cd52d71c4a2e6e88b7f7fbeaa8bca45b397",
    ),
    (
        "reg.tei.rlm-2512.24601",
        "sha256:e06c45fe8dc50677b7a08128790977152891a8e1dd7aac624f5795023b9b2d81",
    ),
    (
        "reg.tei.reca-bibliography-2605.26525",
        "sha256:55325dfad7f9a56624ba8eb964a038f5ec0f8fc0b5405a26cc586032ba42b2d2",
    ),
];

/// Aggregate identities frozen at S01 (`baseline.json` report_identity).
const FROZEN_AGGREGATE_OUTPUT_HASH: &str =
    "sha256:90484527edcf0f853ec25da84caef9ccec06840f5cd5d22068a7ff1cc34a0a3f";
const FROZEN_SOURCE_HASH: &str =
    "sha256:d0b2c6c7e3b4af551b1e804705b6424cd1c367832516beb383d9c3cb5ade4c49";
const FROZEN_CORPUS_HASH: &str =
    "sha256:8e142e918a2c16ecdafabd9240e5095fe2cb44d9f3539597dc9faf589cf33f87";
/// All three executed S01 cases run under the rich profile.
const FROZEN_PROFILE_FINGERPRINT: &str = RICH_FP;

/// Aggregate conservation totals frozen at S01 (family_counts).
const FROZEN_TOTALS: [(&str, u64); 4] = [
    ("citations", 207),
    ("formulas", 38),
    ("sections", 264),
    ("tables", 8),
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
fn lifted_output_preimage(article: &ParsedArticle) -> serde_json::Value {
    json!({
        "schema": "da-parse.lift-output-preimage.v1",
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
    let pins: [(&str, &[&str]); 4] = [
        (
            "crates/da-cli/src/load.rs",
            &[
                "\"da_adapters::GrobidParser@load-v1\"",
                "\"da_adapters::HtmlParser@load-v1\"",
                "\"ParsedArticle.v1\"",
                "\"pdf-grobid\"",
            ],
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
                "\"ParsedArticle@serde-v3-unique-navigation\"",
                "\"grobid/grobid:0.9.0-crf\"",
            ],
        ),
        (
            "crates/da-adapters/src/pipeline_stages.rs",
            &[
                "\"GrobidParser@tei-navigation-v3\"",
                "\"ParsedArticle@serde-v3-unique-navigation\"",
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
