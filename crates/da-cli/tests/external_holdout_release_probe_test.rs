#![cfg(feature = "pdf-layout")]

//! M511-j7wz2f / S08 / T04 — the runtime artifact of the frozen external holdout
//! probe is frozen, non-vacuous, reproducible and redacted (H20 / GAR-015).
//!
//! The structural, independent oracle for this probe is the dev-only
//! `da-parse` test `external_holdout_probe_semantics.rs` (S08/T03) and its DATA
//! carrier `data/parse/acceptance_manifest.yaml#external_holdout_contract.release_probe`
//! (S08/T01/T02).  This test owns the *runtime* half: the example
//! `examples/external_holdout_release_probe.rs` re-measures the same frozen
//! sha256-pinned TEI (the measurable subset of the frozen population; the
//! remainder is the lane's named typed exceptions), fails FATAL on
//! drift/absence/non-UTF-8, and emits
//! `artifacts/parse_compat/external_holdout_release_probe.json`.
//!
//! The artifact is the same-n surface the governor and H20 need, so the audit
//! below pins the identity (one distinct sha256 per frozen document, the probe digest recomputed
//! from the source manifest by the declared canonicalization handles), the
//! GAR-006 population split (a distinguishable reference/executed/evaluated
//! population with no legacy `denominators`/`corpus` block), the measured
//! per-document profile S08/T03 froze, the counts-only aggregate, the margin
//! verdicts recomputed from the DATA thresholds (a re-fitted verdict is a
//! failure), the catalog-overlap census as a measurement, the policy locks and
//! the redaction.  Denominator erosion, identity collapse, a fabricated band, a
//! spliced latency source, a lifted external band and a leaked payload are all
//! rejected by the same audit, which the negative test proves one mutation at a
//! time.
//!
//! `cargo test --test NAME` does not build examples, so when the runner is
//! missing this test builds it through cargo with the same target directory
//! (cargo releases the build lock while tests run, so this cannot deadlock).

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

/// The committed runtime artifact under test.
const ARTIFACT: &str = "artifacts/parse_compat/external_holdout_release_probe.json";
const SCHEMA_VERSION: &str = "external-holdout-release-probe.v1";
const MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
const SOURCE_MANIFEST_REL: &str = "data/parse/holdout_manifests.yaml";
const PROVENANCE_REL: &str = "crates/da-parse/tests/fixtures/holdout/provenance.yaml";
const LATENCY_SOURCE_REF: &str =
    "crates/da-parse/tests/fixtures/holdout/provenance.yaml#documents[].latency_ms";
const EXECUTION_EVIDENCE: &str = "tracked_offline_fixture:external_holdout_release_probe_reference";

/// The declared probe population.  DATA is the only carrier of the population
/// size: the source manifest's article list.  No literal governs it, and the
/// frozen per-document table below must keep covering exactly this population.
fn declared_population() -> usize {
    load_yaml(SOURCE_MANIFEST_REL)["articles"]
        .as_sequence()
        .expect("the source manifest must declare the article list")
        .len()
}

/// One frozen per-document row: `(document_id, identity_sha256,
/// source_population, admitted, agreement_micros, uncertainty_low_micros,
/// uncertainty_high_micros, uncertainty_width_micros, latency_ms)`.
type FrozenDocument = (
    &'static str,
    &'static str,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
);

/// The frozen per-document measurement of 2026-09-24, identical to the S08/T03
/// oracle's table.
const FROZEN_DOCUMENTS: &[FrozenDocument] = &[
    (
        "1203.2295",
        "a230094fff20d5434e1871ee9511ce61d71777b6ff0454faeaedc79cdfef6629",
        57,
        56,
        1000000,
        935804,
        1000000,
        64196,
        46341,
    ),
    (
        "2401.17167",
        "7c9c42adc0ddbc22a9bf889c31dd3387b26d6dd8cd007909fb3127c29628c262",
        283,
        268,
        1000000,
        985868,
        1000000,
        14132,
        10255,
    ),
    (
        "2411.07228",
        "97a3f9f0eab21f512a2fed28a7e98058d002c3a056297d86f7bc4e025b4ec0db",
        128,
        121,
        1000000,
        969228,
        1000000,
        30772,
        5420,
    ),
    (
        "2505.16270",
        "2d09c727aa300bf8446efbdf55a94d7713b4d7d39e9fd3f84ee84b12453426bc",
        221,
        219,
        1000000,
        982761,
        1000000,
        17239,
        8089,
    ),
    (
        "2608.22118",
        "635dc993492fbb3e83b9d8478fe85be56042bcacf435a627b8e254f1761a2714",
        42,
        41,
        1000000,
        914330,
        1000000,
        85670,
        3883,
    ),
    (
        "1409.0473",
        "11c46c00dc4b97b47c9591356d29eedd888e467d0d042fa104d2583ee6f3bb55",
        61,
        59,
        1000000,
        938869,
        1000000,
        61131,
        3376,
    ),
    (
        "1410.4615",
        "4236917ee79e85b48908ea242f848d34043f1ca9e86586c7892e1a77182f94ce",
        79,
        78,
        1000000,
        953061,
        1000000,
        46939,
        3280,
    ),
    (
        "1410.5401",
        "a73daefd41d7f04a390515ec22f0a56cc87d8aaf668205d72308bec2181b382b",
        84,
        83,
        1000000,
        955763,
        1000000,
        44237,
        3378,
    ),
    (
        "1506.02438",
        "6a6a0633e0a451642dbb61878b23774adf0a4ff312c6d98e6a090178963f5be1",
        64,
        63,
        1000000,
        942527,
        1000000,
        57473,
        2947,
    ),
    (
        "1511.08228",
        "6b19ce1f5702fe3e34a3061e911ab3dede76acd8652827d923e995a51cf0764c",
        80,
        79,
        1000000,
        953627,
        1000000,
        46373,
        2431,
    ),
    (
        "1606.01540",
        "4fb94fb9b986409621b0392d637b276d37edde6638d7c3fff5da47700247e1f6",
        44,
        36,
        1000000,
        903578,
        1000000,
        96422,
        1590,
    ),
    (
        "1703.00050",
        "fe95602ca1b0eaadff08f99e7e79e9767be668dab3b92ce75da1425cebed4b42",
        53,
        52,
        1000000,
        931205,
        1000000,
        68795,
        2877,
    ),
    (
        "1706.03741",
        "311948c84b492cceff43e03fadbdd45c5671eab6bd0c12db28fd3e4da421a4f4",
        100,
        97,
        1000000,
        961905,
        1000000,
        38095,
        3140,
    ),
    (
        "1712.00600",
        "44dff1010d73b0ad72897dfe58ffce7dc666c380175081588717fa14a996ade2",
        23,
        22,
        1000000,
        851340,
        1000000,
        148660,
        1469,
    ),
    (
        "1902.04043",
        "03d0ef3b738435b16be7c4aaf95e6cb9c4a2245fb7b11343dacb57dd2df900a1",
        98,
        97,
        1000000,
        961905,
        1000000,
        38095,
        3003,
    ),
    (
        "2010.03768",
        "60fbb3f794ac17b64482cfc57878feed03d0b6767cdad95182bc1b6e9a562e4a",
        124,
        123,
        1000000,
        969713,
        1000000,
        30287,
        3804,
    ),
    (
        "2010.09776",
        "7ad0356afe67dc9be7232cdd65aa2488071ae2067a7865f87e5e613a377ad8b7",
        248,
        210,
        1000000,
        982035,
        1000000,
        17965,
        3548,
    ),
    (
        "2107.06857",
        "d7baa798e5cee89fbd5202c54d822662b5ea8fdc60ef3ef22d935151ee6c6ee3",
        177,
        176,
        1000000,
        978639,
        1000000,
        21361,
        6445,
    ),
    (
        "2112.09332",
        "1463ee2772e04070d919c1e4fc3572756deac27d2d98433d023635acf51563ed",
        73,
        72,
        1000000,
        949347,
        1000000,
        50653,
        5235,
    ),
    (
        "2201.11903",
        "c1cd041d3aed55a92698c80d9d94d0c7a62572924b4ec67cbd48b7cb01121f10",
        169,
        168,
        1000000,
        977645,
        1000000,
        22355,
        6986,
    ),
    (
        "2203.07540",
        "c0aaa035976e5d94a00305d6f9b1646ba115eb1d0f5c573205a6f6921de5970d",
        139,
        138,
        1000000,
        972916,
        1000000,
        27084,
        4629,
    ),
    (
        "2205.10625",
        "c884f2c28a17331bc3c39d20ff912a65745daf216f63ba874e8cb7ba8cc10aa3",
        89,
        88,
        1000000,
        958171,
        1000000,
        41829,
        10247,
    ),
    (
        "2205.11916",
        "5dc63bb5d2d16833ebaaed3b1b6968e57fc4ea4c1b27c1ae4cee3ed663b1de2b",
        152,
        149,
        1000000,
        974865,
        1000000,
        25135,
        7170,
    ),
    (
        "2206.09889",
        "de03a7562632711e7010ba511534ac2ad883a99621ef05b53ababf059d3b811f",
        101,
        93,
        1000000,
        960331,
        1000000,
        39669,
        3217,
    ),
    (
        "2206.10498",
        "0a50b6d4c4dab11120fc539c12f052a301bf98e29393ec1a800229769d42aa6b",
        83,
        82,
        1000000,
        955248,
        1000000,
        44752,
        6893,
    ),
    (
        "2206.10944",
        "7c91c6d6a0669dea3cca44c176855b82809352b8283a9e73af018c2ff024b764",
        42,
        37,
        1000000,
        905939,
        1000000,
        94061,
        2021,
    ),
    (
        "2207.01206",
        "e68cf8cba1bb3ac3f1cb20e8666242d47c95628f1007a5cd7c784bac065772c6",
        127,
        126,
        1000000,
        970413,
        1000000,
        29587,
        5120,
    ),
    (
        "2207.01780",
        "de39d4ec3079735cb939c1d3be9a2d16a9c7e00a1591e4cb843d36772e9c131c",
        179,
        178,
        1000000,
        978874,
        1000000,
        21126,
        6025,
    ),
    (
        "2210.03350",
        "937361d23baab081d241b8c4cb76579bc7654d9ce294e2cfb67f099f172524f7",
        117,
        115,
        1000000,
        967675,
        1000000,
        32325,
        4231,
    ),
    (
        "2210.03629",
        "dd4335ea14681bf25f3c28bf15ba82760b8b7521d92217bbfcf75d55bc371958",
        106,
        105,
        1000000,
        964705,
        1000000,
        35295,
        5208,
    ),
    (
        "2212.04088",
        "7a828241c0b2d5d1bb498ca6a1a019181e3e6d355f5cfb8cbd9974161ef1f3d8",
        92,
        91,
        1000000,
        959495,
        1000000,
        40505,
        3685,
    ),
    (
        "2212.08073",
        "558b12fdaf7201b84fd4d59899bb7ab95fb1770e5d22370e5f0a07ab921c2887",
        80,
        79,
        1000000,
        953627,
        1000000,
        46373,
        5336,
    ),
    (
        "2212.10509",
        "b5d1469fcbabfde6f06d63c90bb1ba44234a1886faabf1963c391fa98e510a35",
        91,
        90,
        1000000,
        959063,
        1000000,
        40937,
        4260,
    ),
    (
        "2212.14024",
        "85b977cbabb26499378efbd79311bf31d0305d46ece192ad7cb08c7fb5412653",
        155,
        154,
        1000000,
        975662,
        1000000,
        24338,
        3860,
    ),
    (
        "2302.00923",
        "1d6e7e56485e1686f232fc84befaa130c47d789eda9087cf3249a58fcd1b6219",
        133,
        132,
        1000000,
        971720,
        1000000,
        28280,
        4489,
    ),
    (
        "2302.01560",
        "e4d5bf5937e01fd1c0622f89e3bb877fafb1bd84f090a54826848f8285de009c",
        115,
        113,
        1000000,
        967121,
        1000000,
        32879,
        5965,
    ),
    (
        "2302.06590",
        "a890ab82ba968ff39ebbe14641ba317cb5af7a7206eea5e1d8dd1700fdf97a0b",
        41,
        37,
        1000000,
        905939,
        1000000,
        94061,
        1801,
    ),
    (
        "2302.08468",
        "f867b907733e2a59325c887ba540f1231f3d1fb9963b1ed6db7105270d9f1c2f",
        180,
        179,
        1000000,
        978989,
        1000000,
        21011,
        4401,
    ),
    (
        "2303.06689",
        "8386fea1bcc7bb6455be55d54374bb32b53afdd1c0634322f205666d691546d4",
        154,
        142,
        1000000,
        973659,
        1000000,
        26341,
        4801,
    ),
    (
        "2303.08268",
        "1b94db6d6c0de92021e93330579262b1d64987b636a3eb5813e36071e0f30bd5",
        61,
        60,
        1000000,
        939826,
        1000000,
        60174,
        2672,
    ),
];

/// The frozen counts-only aggregate: `(source_population, admitted, tp, fn, fp,
/// agreeing_documents, margin_pass, margin_fail, no_denominator_documents)`.
/// The population-derived counter (`agreeing_documents`) is recomputed from the
/// frozen per-document rows, so a re-measured table cannot silently keep the old
/// population number; every other value is the recorded aggregate measurement.
fn frozen_aggregate() -> (u64, u64, u64, u64, u64, u64, u64, u64, u64) {
    (
        4445,
        4308,
        4308,
        0,
        0,
        FROZEN_DOCUMENTS
            .iter()
            .filter(|(_, _, _, _, agreement, _, _, _, _)| *agreement == 1_000_000)
            .count() as u64,
        80,
        0,
        0,
    )
}

/// The frozen named admission narrowings, identical to the S08/T03 table:
/// `(document_id, class, source_population, admitted)`.  Every class whose
/// admitted band is smaller than its real carrier population must appear here.
const FROZEN_NARROWINGS: [(&str, &str, u64, u64); 59] = [
    ("1203.2295", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2401.17167", "AdmittedAuthorLine", 239, 235),
    ("2401.17167", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2401.17167", "SemanticFieldLoss::AuthorAffiliation", 16, 6),
    ("2411.07228", "AdmittedAuthorLine", 11, 8),
    ("2411.07228", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2411.07228", "SemanticFieldLoss::AuthorAffiliation", 11, 8),
    ("2505.16270", "AdmittedAuthorLine", 10, 9),
    ("2505.16270", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2608.22118", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1409.0473", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1409.0473", "SemanticFieldLoss::AuthorAffiliation", 3, 2),
    ("1410.4615", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1410.5401", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1506.02438", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1511.08228", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1606.01540", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1606.01540", "SemanticFieldLoss::AuthorAffiliation", 7, 0),
    ("1703.00050", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1706.03741", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1706.03741", "SemanticFieldLoss::AuthorAffiliation", 2, 0),
    ("1712.00600", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("1902.04043", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2010.03768", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2010.09776", "AdmittedAuthorLine", 38, 36),
    ("2010.09776", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2010.09776", "SemanticFieldLoss::AuthorAffiliation", 44, 9),
    ("2107.06857", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2112.09332", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2201.11903", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2203.07540", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2205.10625", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2205.11916", "AdmittedAuthorLine", 7, 6),
    ("2205.11916", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2205.11916", "SemanticFieldLoss::AuthorAffiliation", 7, 6),
    ("2206.09889", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2206.09889", "SemanticFieldLoss::AuthorAffiliation", 7, 0),
    ("2206.10498", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2206.10944", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2206.10944", "SemanticFieldLoss::AuthorAffiliation", 4, 0),
    ("2207.01206", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2207.01780", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2210.03350", "AdmittedAuthorLine", 11, 10),
    ("2210.03350", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2210.03629", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2212.04088", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2212.08073", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2212.10509", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2212.14024", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2302.00923", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2302.01560", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2302.01560", "SemanticFieldLoss::AuthorAffiliation", 9, 8),
    ("2302.06590", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2302.06590", "SemanticFieldLoss::AuthorAffiliation", 4, 1),
    ("2302.08468", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2303.06689", "AdmittedAuthorLine", 9, 8),
    ("2303.06689", "SemanticFieldLoss::HeaderTitle", 2, 1),
    ("2303.06689", "SemanticFieldLoss::AuthorAffiliation", 10, 0),
    ("2303.08268", "SemanticFieldLoss::HeaderTitle", 2, 1),
];

/// The two release labels the artifact may spell.  Any other `@`-bearing string
/// is a leaked address or a raw value, not an identity.
const PINNED_RELEASE_LABELS: [&str; 2] = ["java-grobid-http@0.9.1", "java-grobid-http@0.9.0-crf"];

/// Every redaction flag the artifact must carry, all `false`.
const REDACTION_FLAGS: [&str; 16] = [
    "raw_tei",
    "raw_pdf",
    "raw_text",
    "field_values",
    "names",
    "email_addresses",
    "institution_names",
    "captions",
    "references",
    "physical_paths",
    "hostname",
    "executable",
    "secrets",
    "pdf_bytes",
    "graph_payload",
    "fulltext",
];

/// Payload keys that may only ever appear as redaction *flags*.
const FORBIDDEN_KEYS: [&str; 12] = [
    "raw_tei",
    "tei_xml",
    "pdf_bytes",
    "body_text",
    "text",
    "captions",
    "references",
    "physical_path",
    "path",
    "hostname",
    "executable",
    "secret",
];

/// The guards the runner declares.  The self-check must prove each one rejects
/// its own violating input, so a guard that silently stops firing is visible.
const GUARD_LABELS: [&str; 32] = [
    "schema_version_drift",
    "reference_population_role",
    "reference_documents",
    "reference_identity_missing",
    "reference_identity_digest_drift",
    "source_manifest_sha_drift",
    "reference_byte_pins_drift",
    "outcome_count",
    "identity_sha_collapse",
    "identity_pin_drift",
    "executed_population_not_executed",
    "execution_evidence_unbound",
    "accepted_documents",
    "evaluated_status",
    "aggregate_zero_admitted_population",
    "aggregate_conservation_mismatch",
    "margin_verdict_absent",
    "margin_verdict_refitted",
    "latency_source_substituted",
    "latency_unmeasured",
    "legacy_block_present",
    "external_lane_state_flipped",
    "policy_lock_open",
    "redaction_flag_raised",
    "redaction_scope_replaced",
    "raw_payload_leak",
    "cost_unknown_read_as_zero",
    "production_catalog_overlap_absent",
    "named_limitations_missing",
    "class_narrowings_incomplete",
    "reference_measurable_documents",
    "reference_exceptions_drift",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn artifact_path() -> PathBuf {
    repo_root().join(ARTIFACT)
}

fn load_committed_artifact() -> Value {
    let path = artifact_path();
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "the canonical artifact {} must be regenerated by `cargo run -p da-cli --features pdf-layout --example external_holdout_release_probe -- --report {ARTIFACT}` ({error})",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("the canonical artifact is valid JSON")
}

fn example_binary() -> PathBuf {
    // Provision the runner once per test process. `cargo test --test NAME` does
    // not build examples, so a stale binary from an earlier run would silently
    // shadow the source under test; an unconditional (incremental) build makes
    // the population proof reproducible. `OnceLock` keeps the parallel tests
    // from racing on cargo's target lock.
    static BINARY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BINARY
        .get_or_init(|| {
            let binary = repo_root().join("target/debug/examples/external_holdout_release_probe");
            let build = Command::new("cargo")
                .args([
                    "build",
                    "-q",
                    "-p",
                    "da-cli",
                    "--features",
                    "pdf-layout",
                    "--example",
                    "external_holdout_release_probe",
                ])
                .current_dir(repo_root())
                .output()
                .expect("build feature-gated release probe example");
            assert!(
                build.status.success(),
                "example build failed: {}",
                String::from_utf8_lossy(&build.stderr)
            );
            assert!(
                binary.is_file(),
                "the example binary must exist after a successful build"
            );
            binary
        })
        .clone()
}

// ─────────────────────── the independent expectations ───────────────────────

fn load_yaml(rel: &str) -> serde_yaml::Value {
    let text = fs::read_to_string(repo_root().join(rel)).expect("DATA file must be readable");
    serde_yaml::from_str(&text).expect("DATA file must be valid YAML")
}

fn manifest_yaml() -> serde_yaml::Value {
    load_yaml(MANIFEST_REL)
}

fn yaml_at<'a>(root: &'a serde_yaml::Value, key: &str) -> &'a serde_yaml::Value {
    root.get(key).expect("DATA key must exist")
}

/// Recompute the probe identity digest from the source manifest using ONLY the
/// declared canonicalization handles.  An independent implementation: the
/// artifact's declared digest is never trusted.
fn recompute_identity_digest(lane: &serde_yaml::Value) -> String {
    let fields: Vec<String> = lane["digest_input_fields"]
        .as_sequence()
        .expect("digest_input_fields")
        .iter()
        .map(|field| field.as_str().expect("field name").to_string())
        .collect();
    assert_eq!(
        lane["digest_input_order"].as_str(),
        Some("as_listed"),
        "the identity handles must be frozen in order"
    );
    let field_separator = lane["digest_field_separator"].as_str().expect("separator");
    let record_separator = lane["digest_record_separator"].as_str().expect("separator");
    let source = load_yaml(SOURCE_MANIFEST_REL);
    let mut records = Vec::new();
    for article in source["articles"].as_sequence().expect("articles") {
        let parts: Vec<String> = fields
            .iter()
            .map(|field| match &article[field.as_str()] {
                serde_yaml::Value::String(text) => text.clone(),
                other => panic!("field `{field}` must be a scalar string, got {other:?}"),
            })
            .collect();
        records.push(parts.join(field_separator));
    }
    format!(
        "sha256:{:x}",
        Sha256::digest(records.join(record_separator).as_bytes())
    )
}

/// The declared margins by metric, read from `acceptance_margins` (the single
/// place thresholds live).
fn declared_margins(manifest: &serde_yaml::Value) -> BTreeMap<String, (String, f64)> {
    let contract = yaml_at(manifest, "external_holdout_contract");
    let mut margins = BTreeMap::new();
    for entry in yaml_at(contract, "acceptance_margins")["margins"]
        .as_sequence()
        .expect("margins")
    {
        margins.insert(
            entry["metric"].as_str().expect("metric").to_string(),
            (
                entry["direction"].as_str().expect("direction").to_string(),
                entry["margin"].as_f64().expect("margin"),
            ),
        );
    }
    margins
}

fn provenance_latency(document_id: &str) -> u64 {
    let provenance = load_yaml(PROVENANCE_REL);
    provenance["documents"]
        .as_sequence()
        .expect("provenance documents")
        .iter()
        .find(|row| row["article_id"].as_str() == Some(document_id))
        .expect("the provenance declares every frozen document")["latency_ms"]
        .as_u64()
        .expect("latency_ms")
}

// ─────────────────────────────── the audit ──────────────────────────────────

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn collect_strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::String(text) => out.push(text),
        Value::Array(items) => items.iter().for_each(|item| collect_strings(item, out)),
        Value::Object(map) => map.values().for_each(|child| collect_strings(child, out)),
        _ => {}
    }
}

/// Payload keys may only appear as redaction flags.
fn raw_key_errors(report: &Value) -> Vec<String> {
    fn visit(value: &Value, inside_redaction: bool, errors: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let in_redaction = inside_redaction || key == "redaction";
                    if !in_redaction && FORBIDDEN_KEYS.contains(&key.as_str()) {
                        errors.push(format!("probe:raw_payload.key:{key}"));
                    }
                    visit(child, in_redaction, errors);
                }
            }
            Value::Array(items) => items
                .iter()
                .for_each(|item| visit(item, inside_redaction, errors)),
            _ => {}
        }
    }
    let mut errors = Vec::new();
    visit(report, false, &mut errors);
    errors
}

/// Every reason the runtime probe artifact is unacceptable.  A pure predicate so
/// the negative test can prove it rejects one violation at a time.
fn artifact_errors(
    report: &Value,
    expected_digest: &str,
    margins: &BTreeMap<String, (String, f64)>,
    declared: usize,
) -> Vec<String> {
    let mut errors = Vec::new();
    // The DATA surfaces this audit compares against, parsed once.
    let manifest = manifest_yaml();
    let contract = yaml_at(&manifest, "external_holdout_contract");
    let lane = &contract["release_probe"];

    // The frozen population is a DATA partition: the measurable `reference_tei`
    // rows plus the typed `reference_exceptions` rows.  The frozen reference
    // surface (`reference_population`, `production_catalog_overlap`) is audited
    // against `declared`; the measured surface (`executed_population`,
    // `evaluated_population`, `outcomes`) is audited against the measurable
    // subset.  If DATA moved the population off the frozen table, or the frozen
    // table no longer covers the measurable subset, the artifact cannot be
    // audited against it: refuse by name rather than compare against a stale
    // table.
    let measurable = lane["reference_tei"]
        .as_sequence()
        .map(Vec::len)
        .unwrap_or(0);
    let exceptions = lane["reference_exceptions"]
        .as_sequence()
        .map(Vec::len)
        .unwrap_or(0);
    if declared != measurable + exceptions || FROZEN_DOCUMENTS.len() != measurable {
        errors.push("probe:reference_population.frozen_table_drift".into());
        return errors;
    }

    if report["schema_version"].as_str() != Some(SCHEMA_VERSION) {
        errors.push("probe:schema_version".into());
    }

    // ── identity: distinct sha256 per frozen document, recomputed digest ──────
    let reference = &report["reference_population"];
    if reference["role"].as_str() != Some("identity_only") {
        errors.push("probe:reference_population.role".into());
    }
    if reference["identity"]
        .as_str()
        .map(str::is_empty)
        .unwrap_or(true)
    {
        errors.push("probe:reference_population.identity".into());
    }
    if reference["identity_sha256"].as_str() != Some(expected_digest) {
        errors.push("probe:reference_population.identity_digest".into());
    }
    let declared_digest = lane["identity_digest"]
        .as_str()
        .expect("declared identity digest");
    if expected_digest != declared_digest {
        errors.push("probe:identity_digest_not_declared".into());
    }
    if reference["documents"].as_u64() != Some(declared as u64) {
        errors.push("probe:reference_population.documents".into());
    }
    let pins = reference["byte_pins"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if pins.len() != declared {
        errors.push("probe:reference_population.byte_pins.cardinality".into());
    }
    let pin_shas: BTreeSet<String> = pins
        .iter()
        .filter_map(|pin| pin["identity_sha256"].as_str().map(str::to_string))
        .collect();
    if pin_shas.len() != declared || pin_shas.iter().any(|sha| !is_sha256(sha)) {
        errors.push("probe:reference_population.byte_pins.shape".into());
    }
    for (index, pin) in pins.iter().enumerate() {
        let matches_lane =
            reference_pin_from_manifest(pin["document_id"].as_str().unwrap_or_default())
                .map(|(sha, size)| {
                    pin["identity_sha256"].as_str() == Some(sha.as_str())
                        && pin["size_bytes"].as_u64() == Some(size)
                })
                .unwrap_or(false);
        if !matches_lane {
            errors.push(format!("probe:reference_population.byte_pins[{index}]"));
        }
    }
    // The release attribution must be the declared one.
    let release = &reference["reference_release"];
    let declared_release = lane["declared_release"].clone();
    if release["version"].as_str() != declared_release["version"].as_str() {
        errors.push("probe:reference_population.release.version".into());
    }
    if release["profile"].as_str() != lane["reference_profile"].as_str() {
        errors.push("probe:reference_population.release.profile".into());
    }
    if release["captured_at_revision"].as_str()
        != load_yaml(PROVENANCE_REL)["captured_at_revision"].as_str()
    {
        errors.push("probe:reference_population.release.captured_at_revision".into());
    }

    // ── executed population: really executed, evidence named ───────────────
    let executed = &report["executed_population"];
    if executed["corpus_execution"].as_bool() != Some(true) {
        errors.push("probe:executed_population.corpus_execution".into());
    }
    if executed["execution_evidence"].as_str() != Some(EXECUTION_EVIDENCE) {
        errors.push("probe:executed_population.execution_evidence".into());
    }
    if executed["documents"].as_u64() != Some(measurable as u64)
        || executed["accepted"].as_u64() != Some(measurable as u64)
    {
        errors.push("probe:executed_population.documents".into());
    }

    // ── GAR-006: the split is real, and the legacy blocks are gone ─────────
    for legacy in ["denominators", "corpus"] {
        if report.get(legacy).is_some() {
            errors.push(format!("probe:legacy_blocks.{legacy}"));
        }
    }
    if reference == executed
        || reference == &report["evaluated_population"]
        || executed == &report["evaluated_population"]
    {
        errors.push("probe:populations.indistinguishable".into());
    }

    // ── the external band stays where it is ────────────────────────────────
    let external = &report["external_holdout_band"];
    if external["execution_status"].as_str() != contract["execution_status"].as_str()
        || external["external_bytes"].as_str() != contract["byte_policy"]["external_bytes"].as_str()
    {
        errors.push("probe:external_holdout_band.flipped".into());
    }

    // ── evaluated population: measured status, counts-only aggregate ───────
    let evaluated = &report["evaluated_population"];
    if evaluated["status"].as_str() != Some("evaluated") {
        errors.push("probe:evaluated_population.status".into());
    }
    if evaluated["documents"].as_u64() != Some(measurable as u64) {
        errors.push("probe:evaluated_population.documents".into());
    }
    let aggregate = &evaluated["aggregate"];
    let observed_aggregate = (
        aggregate["source_population"].as_u64().unwrap_or(0),
        aggregate["admitted"].as_u64().unwrap_or(0),
        aggregate["tp"].as_u64().unwrap_or(0),
        aggregate["fn"].as_u64().unwrap_or(0),
        aggregate["fp"].as_u64().unwrap_or(0),
        aggregate["agreeing_documents"].as_u64().unwrap_or(0),
        aggregate["margin_pass"].as_u64().unwrap_or(0),
        aggregate["margin_fail"].as_u64().unwrap_or(0),
        aggregate["no_denominator_documents"].as_u64().unwrap_or(0),
    );
    let frozen = frozen_aggregate();
    if observed_aggregate != frozen {
        errors.push("probe:aggregate.frozen".into());
    }
    if aggregate["counts_only"].as_bool() != Some(true) {
        errors.push("probe:aggregate.counts_only".into());
    }
    if aggregate["admitted"].as_u64().unwrap_or(0) == 0 {
        errors.push("probe:aggregate.zero_denominator".into());
    }
    let aggregated_from_rows: (u64, u64, u64, u64, u64) = report["outcomes"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .fold((0, 0, 0, 0, 0), |acc, outcome| {
            (
                acc.0 + outcome["source_population"].as_u64().unwrap_or(0),
                acc.1 + outcome["admitted"].as_u64().unwrap_or(0),
                acc.2 + outcome["conserved"].as_u64().unwrap_or(0),
                acc.3 + outcome["lost"].as_u64().unwrap_or(0),
                acc.4 + outcome["fabricated"].as_u64().unwrap_or(0),
            )
        });
    if aggregated_from_rows != (frozen.0, frozen.1, frozen.2, frozen.3, frozen.4) {
        errors.push("probe:aggregate.row_sum".into());
    }

    // ── outcomes: the frozen per-document measurement ──────────────────────
    let outcomes = report["outcomes"].as_array().cloned().unwrap_or_default();
    if outcomes.len() != measurable {
        errors.push("probe:outcomes.cardinality".into());
    }
    let mut measured_ids: BTreeSet<&str> = BTreeSet::new();
    for &(document_id, sha, source_population, admitted, agreement, low, high, width, latency) in
        FROZEN_DOCUMENTS
    {
        let row = outcomes
            .iter()
            .find(|row| row["document_id"].as_str() == Some(document_id))
            .unwrap_or_else(|| panic!("the artifact must carry the frozen document {document_id}"));
        measured_ids.insert(document_id);
        if row["identity_sha256"].as_str() != Some(sha) {
            errors.push(format!("probe:outcomes.{document_id}.identity"));
        }
        if row["state"].as_str() != Some("evaluated") {
            errors.push(format!("probe:outcomes.{document_id}.state"));
        }
        let observed = (
            row["source_population"].as_u64(),
            row["admitted"].as_u64(),
            row["conserved"].as_u64(),
            row["lost"].as_u64(),
            row["fabricated"].as_u64(),
            row["agreement_ratio_micros"].as_u64(),
            row["uncertainty_low"].as_u64(),
            row["uncertainty_high"].as_u64(),
            row["uncertainty_width_micros"].as_u64(),
        );
        if observed
            != (
                Some(source_population),
                Some(admitted),
                Some(admitted),
                Some(0),
                Some(0),
                Some(agreement),
                Some(low),
                Some(high),
                Some(width),
            )
        {
            errors.push(format!("probe:outcomes.{document_id}.profile"));
        }
        if width != high.saturating_sub(low) {
            errors.push(format!("probe:outcomes.{document_id}.interval"));
        }
        if row["latency_ms"].as_u64() != Some(latency)
            || row["latency_ms"].as_u64() != Some(provenance_latency(document_id))
        {
            errors.push(format!("probe:outcomes.{document_id}.latency"));
        }
        if row["latency_source"].as_str() != Some(LATENCY_SOURCE_REF) {
            errors.push(format!("probe:outcomes.{document_id}.latency_source"));
        }
        if !row["cost_lower_bound"].is_null() || !row["cost_upper_bound"].is_null() {
            errors.push(format!("probe:outcomes.{document_id}.cost_unknown"));
        }
        // The declared per-document reporting fields must all be present.
        for field in [
            "document_id",
            "state",
            "identity_sha256",
            "uncertainty_low",
            "uncertainty_high",
            "cost_lower_bound",
            "cost_upper_bound",
        ] {
            if row.get(field).is_none() {
                errors.push(format!("probe:outcomes.{document_id}.field:{field}"));
            }
        }
        // ── margin verdicts: present, and not re-fitted ────────────────────
        let margins_rows = row["margins"].as_array().cloned().unwrap_or_default();
        if margins_rows.len() != margins.len() {
            errors.push(format!("probe:outcomes.{document_id}.margins.cardinality"));
        }
        for margin in &margins_rows {
            let metric = margin["metric"].as_str().unwrap_or_default();
            let Some((direction, threshold)) = margins.get(metric) else {
                errors.push(format!(
                    "probe:outcomes.{document_id}.margins.unknown:{metric}"
                ));
                continue;
            };
            let observed_threshold = margin["threshold_micros"].as_u64();
            if observed_threshold != Some((threshold * 1_000_000.0).round() as u64) {
                errors.push(format!(
                    "probe:outcomes.{document_id}.margins.threshold:{metric}"
                ));
            }
            let value = margin["value_micros"].as_u64();
            let expected_verdict = match value {
                None => "no_denominator",
                Some(value) => {
                    let value = value as f64 / 1_000_000.0;
                    let satisfied = if direction == "at_least" {
                        value >= *threshold
                    } else {
                        value <= *threshold
                    };
                    if satisfied { "PASS" } else { "FAIL" }
                }
            };
            if margin["verdict"].as_str() != Some(expected_verdict) {
                errors.push(format!(
                    "probe:outcomes.{document_id}.margins.refitted:{metric}"
                ));
            }
        }
        // ── typed gaps: every narrowing is explained ───────────────────────
        for gap in row["typed_gaps"].as_array().cloned().unwrap_or_default() {
            let admitted = gap["admitted"].as_u64().unwrap_or(u64::MAX);
            let source_population = gap["source_population"].as_u64().unwrap_or(0);
            if admitted >= source_population || source_population == 0 {
                errors.push(format!(
                    "probe:outcomes.{document_id}.typed_gaps.not_a_narrowing"
                ));
            }
            if gap["reason"]
                .as_str()
                .map(str::trim)
                .filter(|reason| !reason.is_empty())
                .is_none()
            {
                errors.push(format!("probe:outcomes.{document_id}.typed_gaps.reason"));
            }
        }
    }
    if outcomes
        .iter()
        .filter_map(|row| row["document_id"].as_str())
        .any(|document_id| !measured_ids.contains(document_id))
    {
        errors.push("probe:outcomes.unexpected_document".into());
    }

    // ── class narrowings: the frozen table, and nothing silent ─────────────
    let declared_narrowings: BTreeSet<(String, String, u64, u64)> = report["class_narrowings"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|row| {
            Some((
                row["document_id"].as_str()?.to_string(),
                row["class"].as_str()?.to_string(),
                row["source_population"].as_u64()?,
                row["admitted"].as_u64()?,
            ))
        })
        .collect();
    let expected_narrowings: BTreeSet<(String, String, u64, u64)> = FROZEN_NARROWINGS
        .iter()
        .map(|(document_id, class, source_population, admitted)| {
            (
                (*document_id).to_string(),
                (*class).to_string(),
                *source_population,
                *admitted,
            )
        })
        .collect();
    if declared_narrowings != expected_narrowings {
        errors.push("probe:class_narrowings.frozen".into());
    }
    // Every class profile must derive exactly the declared narrowing set.
    let mut derived: BTreeSet<(String, String, u64, u64)> = BTreeSet::new();
    for document in report["class_profiles"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let document_id = document["document_id"].as_str().unwrap_or_default();
        for class_row in document["classes"].as_array().cloned().unwrap_or_default() {
            let admitted = class_row["admitted"].as_u64().unwrap_or(0);
            let source_population = class_row["source_population"].as_u64().unwrap_or(0);
            if admitted < source_population {
                derived.insert((
                    document_id.to_string(),
                    class_row["class"].as_str().unwrap_or_default().to_string(),
                    source_population,
                    admitted,
                ));
            }
        }
    }
    if derived != expected_narrowings {
        errors.push("probe:class_profiles.narrowings".into());
    }

    // ── catalog overlap: a real measurement, not an assertion ──────────────
    let census = &report["production_catalog_overlap"];
    let locators = census["locators"].as_array().cloned().unwrap_or_default();
    if census["census_kind"].as_str() != Some("filesystem_presence_of_the_holdout_identities")
        || census["documents"].as_u64() != Some(declared as u64)
        || locators.len() != declared
    {
        errors.push("probe:production_catalog_overlap.shape".into());
    }
    // The non-gold claim is the one this census exists to measure.
    if census["gold_standard_present_count"].as_u64() != Some(0) {
        errors.push("probe:production_catalog_overlap.gold_standard".into());
    }
    if census["canonical_catalog_present_count"].as_u64() != Some(0) {
        errors.push("probe:production_catalog_overlap.canonical_catalog".into());
    }
    if census["production_corpus_identity_overlap"].as_str() != Some("none") {
        errors.push("probe:production_catalog_overlap.overlap_label".into());
    }
    // Each locator row must be a measurement: declared-present must agree with
    // the manifest's declared path, and the flags must be booleans.
    let absent_expected = locators
        .iter()
        .filter(|row| {
            row["declared_pdf_present"].as_bool() != Some(true)
                && row["canonical_catalog_present"].as_bool() != Some(true)
                && row["sources_archive_present"].as_bool() != Some(true)
                && row["gold_standard_present"].as_bool() != Some(true)
        })
        .count() as u64;
    if census["absent_from_every_locator_count"].as_u64() != Some(absent_expected) {
        errors.push("probe:production_catalog_overlap.absent_count".into());
    }
    for row in &locators {
        if row["document_id"]
            .as_str()
            .map(str::is_empty)
            .unwrap_or(true)
        {
            errors.push("probe:production_catalog_overlap.locator".into());
        }
        for flag in [
            "declared_pdf_present",
            "canonical_catalog_present",
            "sources_archive_present",
            "gold_standard_present",
        ] {
            if row[flag].as_bool().is_none() {
                errors.push(format!("probe:production_catalog_overlap.flag:{flag}"));
            }
        }
    }

    // ── cost and latency accounting: bounded, never measured, never pooled ─
    let cost = &report["cost_accounting"];
    if cost["status"].as_str() != Some("bounded_not_measured")
        || cost["unknown_is_not_zero"].as_bool() != Some(true)
        || !cost["cost_lower_bound"].is_null()
        || !cost["cost_upper_bound"].is_null()
        || cost["pooled_cost_claim"].as_str() != Some("forbidden")
    {
        errors.push("probe:cost_accounting".into());
    }
    let latency = &report["latency_accounting"];
    if latency["measurement_source"].as_str() != Some("recorded_wall_clock_per_document")
        || latency["latency_source"].as_str() != Some(LATENCY_SOURCE_REF)
        || latency["pooled_latency_claim"].as_str() != Some("forbidden")
        || latency["wall_clock_at_run_time"].as_bool() != Some(false)
    {
        errors.push("probe:latency_accounting".into());
    }

    // ── policy locks: shadow only, D127 closed ─────────────────────────────
    let policy = &report["policy_locks"];
    for (key, expected) in [
        ("auto_promote", "forbidden"),
        ("production_route", "forbidden"),
        ("graph_write", "forbidden"),
        ("pooled_claim", "forbidden"),
    ] {
        if policy[key].as_str() != Some(expected) {
            errors.push(format!("probe:policy_locks.{key}"));
        }
    }
    if policy["import_eligible"].as_bool() != Some(false)
        || policy["deployed"].as_bool() != Some(false)
    {
        errors.push("probe:policy_locks.bool".into());
    }

    // ── redaction: declared flags only, all false, no payload anywhere ─────
    let redaction = &report["redaction"];
    let declared_flags: BTreeSet<&str> = redaction
        .as_object()
        .map(|flags| flags.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let expected_flags: BTreeSet<&str> = REDACTION_FLAGS.iter().copied().collect();
    if declared_flags != expected_flags {
        errors.push("probe:redaction.scope".into());
    }
    if let Some(flags) = redaction.as_object() {
        for (flag, value) in flags {
            if value.as_bool() != Some(false) {
                errors.push(format!("probe:redaction.{flag}"));
            }
        }
    }
    errors.extend(raw_key_errors(report));
    if serde_json::to_string(report)
        .unwrap_or_default()
        .contains('<')
    {
        errors.push("probe:raw_payload.xml".into());
    }
    let mut strings = Vec::new();
    collect_strings(report, &mut strings);
    for text in strings.into_iter().filter(|text| text.contains('@')) {
        if !PINNED_RELEASE_LABELS.contains(&text) {
            errors.push("probe:raw_payload.at".into());
        }
    }

    // ── the limitations are declared next to the numbers ───────────────────
    if report["named_limitations"]
        .as_array()
        .map(Vec::is_empty)
        .unwrap_or(true)
    {
        errors.push("probe:named_limitations".into());
    }
    errors
}

/// A lane byte pin from the manifest: `(identity sha256 without prefix, size)`.
fn reference_pin_from_manifest(document_id: &str) -> Option<(String, u64)> {
    let manifest = manifest_yaml();
    let contract = yaml_at(&manifest, "external_holdout_contract");
    let lane = &contract["release_probe"];
    // The frozen population is the measurable pins plus the typed exceptions:
    // the artifact's `byte_pins` covers all 50, so the manifest lookup must too.
    let mut rows: Vec<&serde_yaml::Value> = Vec::new();
    for key in ["reference_tei", "reference_exceptions"] {
        if let Some(entries) = lane[key].as_sequence() {
            rows.extend(entries.iter());
        }
    }
    rows.into_iter()
        .find(|row| row["article_id"].as_str() == Some(document_id))
        .and_then(|row| {
            Some((
                row["sha256"].as_str()?.strip_prefix("sha256:")?.to_string(),
                row["size_bytes"].as_u64()?,
            ))
        })
}

// ──────────────────────────────── positives ─────────────────────────────────

#[test]
fn committed_artifact_is_a_frozen_probe_measurement() {
    let report = load_committed_artifact();
    let expected_digest = recompute_identity_digest(
        &yaml_at(&manifest_yaml(), "external_holdout_contract")["release_probe"],
    );
    let errors = artifact_errors(
        &report,
        &expected_digest,
        &declared_margins(&manifest_yaml()),
        declared_population(),
    );
    assert!(
        errors.is_empty(),
        "the committed {} failed its own audit: {errors:#?}",
        artifact_path().display()
    );
    // Non-vacuity, stated once outside the audit.
    assert!(
        report["evaluated_population"]["aggregate"]["tp"]
            .as_u64()
            .unwrap_or(0)
            > 0,
        "the probe metric must be non-vacuous"
    );
}

#[test]
fn external_band_and_human_release_decision_stay_open() {
    // H20 is not closed by this artifact: the external bytes band stays absent
    // and the release decision stays with a real person.
    let manifest = manifest_yaml();
    let contract = yaml_at(&manifest, "external_holdout_contract");
    assert_eq!(contract["execution_status"].as_str(), Some("not_executed"));
    assert_eq!(
        contract["byte_policy"]["external_bytes"].as_str(),
        Some("bytes_absent")
    );
    assert_eq!(
        contract["byte_policy"]["external_rows_are_evidence"].as_bool(),
        Some(false)
    );
    let review = yaml_at(&manifest, "release_decision_package")["human_release_review"].clone();
    assert_eq!(review["decision"].as_str(), Some("pending"));
    assert!(review["reviewer"].is_null());
    assert_eq!(
        review["prose_alone_authorizes_release"].as_bool(),
        Some(false)
    );
}

// ──────────────────────────────── negatives ────────────────────────────────

#[test]
fn artifact_audit_rejects_every_declared_violation() {
    let valid = load_committed_artifact();
    let manifest = manifest_yaml();
    let expected_digest = recompute_identity_digest(
        &yaml_at(&manifest, "external_holdout_contract")["release_probe"],
    );
    let margins = declared_margins(&manifest);
    assert!(
        artifact_errors(&valid, &expected_digest, &margins, declared_population()).is_empty(),
        "the unmutated artifact must pass the audit"
    );

    let first_sha = valid["outcomes"][0]["identity_sha256"].clone();
    let mut mutations: Vec<(&str, Value)> = Vec::new();

    let mut synthetic = valid.clone();
    synthetic["schema_version"] = json!("external-holdout-release-probe.v0");
    mutations.push(("schema_version_rewritten", synthetic));

    let mut role = valid.clone();
    role["reference_population"]["role"] = json!("identity_and_execution");
    mutations.push(("reference_population_role_rewritten", role));

    let mut digest = valid.clone();
    digest["reference_population"]["identity_sha256"] = json!("sha256:drift");
    mutations.push(("identity_digest_drifted", digest));

    let mut pin_drift = valid.clone();
    pin_drift["reference_population"]["byte_pins"][0]["size_bytes"] = json!(1);
    mutations.push(("byte_pin_drifted", pin_drift));

    let mut missing_pin = valid.clone();
    missing_pin["reference_population"]["byte_pins"]
        .as_array_mut()
        .expect("byte pins")
        .pop();
    mutations.push(("byte_pin_removed", missing_pin));

    let mut lost_documents = valid.clone();
    lost_documents["reference_population"]["documents"] = json!(1);
    mutations.push(("document_denominator_lost", lost_documents));

    let mut denied_execution = valid.clone();
    denied_execution["executed_population"]["corpus_execution"] = json!(false);
    mutations.push(("corpus_execution_denied", denied_execution));

    let mut unbound_evidence = valid.clone();
    unbound_evidence["executed_population"]["execution_evidence"] = json!("n/a");
    mutations.push(("execution_evidence_unbound", unbound_evidence));

    let mut legacy = valid.clone();
    legacy["denominators"] = json!({"documents": declared_population()});
    mutations.push(("legacy_denominators_block_reintroduced", legacy));

    let mut legacy_corpus = valid.clone();
    legacy_corpus["corpus"] = json!({"documents": declared_population()});
    mutations.push(("legacy_corpus_block_reintroduced", legacy_corpus));

    let mut split_collapsed = valid.clone();
    split_collapsed["executed_population"] = valid["reference_population"].clone();
    mutations.push(("populations_collapsed", split_collapsed));

    let mut flipped_band = valid.clone();
    flipped_band["external_holdout_band"]["execution_status"] = json!("executed");
    mutations.push(("external_band_lifted", flipped_band));

    let mut unstated_status = valid.clone();
    unstated_status["evaluated_population"]["status"] = json!("not_evaluated");
    mutations.push(("evaluated_status_removed", unstated_status));

    let mut vacuous = valid.clone();
    vacuous["evaluated_population"]["aggregate"]["admitted"] = json!(0);
    vacuous["evaluated_population"]["aggregate"]["tp"] = json!(0);
    vacuous["evaluated_population"]["aggregate"]["source_population"] = json!(0);
    mutations.push(("vacuous_metric", vacuous));

    let mut fabricated = valid.clone();
    fabricated["evaluated_population"]["aggregate"]["fp"] = json!(7);
    mutations.push(("fabricated_band_admitted", fabricated));

    let mut non_counts = valid.clone();
    non_counts["evaluated_population"]["aggregate"]["counts_only"] = json!(false);
    mutations.push(("counts_only_dropped", non_counts));

    let mut doc_drift = valid.clone();
    doc_drift["outcomes"][0]["admitted"] = json!(1);
    mutations.push(("outcome_admitted_drifted", doc_drift));

    let mut latency_swap = valid.clone();
    latency_swap["outcomes"][0]["latency_source"] = json!("std::time::Instant::now()");
    mutations.push(("latency_source_substituted", latency_swap));

    let mut latency_zero = valid.clone();
    latency_zero["outcomes"][0]["latency_ms"] = json!(0);
    mutations.push(("latency_unmeasured", latency_zero));

    let mut sha_collapse = valid.clone();
    sha_collapse["outcomes"][1]["identity_sha256"] = first_sha;
    mutations.push(("outcome_identity_collapsed", sha_collapse));

    let mut dropped_field = valid.clone();
    dropped_field["outcomes"][0]
        .as_object_mut()
        .expect("outcome")
        .remove("uncertainty_low");
    mutations.push(("declared_reporting_field_dropped", dropped_field));

    let mut dropped_margin = valid.clone();
    dropped_margin["outcomes"][0]["margins"]
        .as_array_mut()
        .expect("margins")
        .pop();
    mutations.push(("margin_verdict_missing", dropped_margin));

    let mut refitted = valid.clone();
    refitted["outcomes"][0]["margins"][0]["verdict"] = json!("FAIL");
    mutations.push(("margin_verdict_refitted", refitted));

    let mut lowered_threshold = valid.clone();
    lowered_threshold["outcomes"][0]["margins"][0]["threshold_micros"] = json!(1);
    mutations.push(("margin_threshold_duplicated", lowered_threshold));

    let mut gap_without_reason = valid.clone();
    gap_without_reason["outcomes"][0]["typed_gaps"] = json!([{
        "class": "SemanticFieldLoss::HeaderTitle",
        "source_population": 2,
        "admitted": 1,
        "reason": "",
    }]);
    mutations.push(("typed_gap_reason_removed", gap_without_reason));

    let mut fake_gap = valid.clone();
    fake_gap["outcomes"][0]["typed_gaps"] = json!([{
        "class": "SemanticFieldLoss::HeaderTitle",
        "source_population": 2,
        "admitted": 2,
        "reason": "not a narrowing",
    }]);
    mutations.push(("non_narrowing_declared_as_gap", fake_gap));

    let mut narrowings_removed = valid.clone();
    narrowings_removed["class_narrowings"] = json!([]);
    mutations.push(("class_narrowings_removed", narrowings_removed));

    let mut narrowing_drift = valid.clone();
    narrowing_drift["class_narrowings"][0]["document_id"] = json!("unknown");
    mutations.push(("class_narrowing_drifted", narrowing_drift));

    let mut census_emptied = valid.clone();
    census_emptied["production_catalog_overlap"]["locators"] = json!([]);
    mutations.push(("census_locators_removed", census_emptied));

    let mut gold_member = valid.clone();
    gold_member["production_catalog_overlap"]["gold_standard_present_count"] = json!(1);
    mutations.push(("gold_standard_membership_claimed", gold_member));

    let mut census_label = valid.clone();
    census_label["production_catalog_overlap"]["production_corpus_identity_overlap"] =
        json!("present");
    mutations.push(("overlap_label_rewritten", census_label));

    let mut cost_zero = valid.clone();
    cost_zero["cost_accounting"]["cost_lower_bound"] = json!(0);
    mutations.push(("unknown_cost_read_as_zero", cost_zero));

    let mut cost_measured = valid.clone();
    cost_measured["cost_accounting"]["status"] = json!("measured");
    mutations.push(("cost_claimed_as_measured", cost_measured));

    let mut latency_clock = valid.clone();
    latency_clock["latency_accounting"]["wall_clock_at_run_time"] = json!(true);
    mutations.push(("run_time_clock_used", latency_clock));

    let mut unlocked_import = valid.clone();
    unlocked_import["policy_locks"]["import_eligible"] = json!(true);
    mutations.push(("import_lock_removed", unlocked_import));

    let mut unlocked_promotion = valid.clone();
    unlocked_promotion["policy_locks"]["auto_promote"] = json!("allowed");
    mutations.push(("auto_promote_unlocked", unlocked_promotion));

    let mut redaction_lie = valid.clone();
    redaction_lie["redaction"]["raw_tei"] = json!(true);
    mutations.push(("redaction_flag_raised", redaction_lie));

    let mut redaction_scope = valid.clone();
    redaction_scope["redaction"]["tei_xml"] = json!(false);
    mutations.push(("redaction_scope_replaced", redaction_scope));

    let mut raw_payload = valid.clone();
    raw_payload["outcomes"][0]["raw_tei"] = json!("<teiHeader/>");
    mutations.push(("raw_payload_injected", raw_payload));

    let mut leaked_email = valid.clone();
    leaked_email["outcomes"][0]["contact"] = json!("someone@example.org");
    mutations.push(("email_injected", leaked_email));

    let mut limitations_removed = valid.clone();
    limitations_removed["named_limitations"] = json!([]);
    mutations.push(("named_limitations_removed", limitations_removed));

    for (name, mutant) in mutations {
        let errors = artifact_errors(&mutant, &expected_digest, &margins, declared_population());
        assert!(
            !errors.is_empty(),
            "the audit accepted the mutation `{name}`"
        );
    }
}

#[test]
fn guard_self_check_rejects_every_declared_violation() {
    let output = Command::new(example_binary())
        .arg("--self-check")
        .current_dir(repo_root())
        .output()
        .expect("run the release probe guard self-check");
    assert!(
        output.status.success(),
        "self-check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for guard in GUARD_LABELS {
        assert!(
            stdout.contains(&format!("guard {guard}: rejected")),
            "the self-check did not prove `{guard}` rejects its violating input:\n{stdout}"
        );
    }
    assert!(
        stdout.contains(&format!(
            "guard self-check: {} guards rejected their violating input",
            GUARD_LABELS.len()
        )),
        "the self-check must report every declared guard:\n{stdout}"
    );
}

/// The lane's guard surface must follow the DATA, not a compiled-in population:
/// one binary, two populations, no code change between the runs.  The synthetic
/// fixture is code-built and lives only for the run, never written into the
/// repository, so it cannot dilute the declared pilot.
#[test]
fn guard_self_check_scales_to_any_population_without_a_code_change() {
    let binary = example_binary();
    let base = declared_population();
    let populations = [base, base + 2]; // N=5 and N=7 on the frozen manifest.
    let mut recorded: BTreeMap<usize, String> = BTreeMap::new();
    for population in populations {
        let output = Command::new(&binary)
            .args(["--self-check", "--documents", &population.to_string()])
            .current_dir(repo_root())
            .output()
            .expect("run the release probe guard self-check");
        assert!(
            output.status.success(),
            "self-check failed at population {population}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert!(
            stdout.contains(&format!(
                "guard self-check population: {population} documents"
            )),
            "the self-check must record the population it exercised:\n{stdout}"
        );
        for guard in GUARD_LABELS {
            assert!(
                stdout.contains(&format!("guard {guard}: rejected")),
                "population {population} did not prove `{guard}` rejects its violating input:\n{stdout}"
            );
        }
        assert!(
            stdout.contains(&format!(
                "guard self-check: {} guards rejected their violating input",
                GUARD_LABELS.len()
            )),
            "population {population} must report every declared guard:\n{stdout}"
        );
        recorded.insert(population, stdout);
    }

    assert_ne!(populations[0], populations[1]);
    // Strip the population line: the two recorded runs must otherwise be equal,
    // which is what "the same binary, only the data changed" means.
    let fingerprint = |text: &str| {
        text.lines()
            .filter(|line| !line.starts_with("guard self-check population:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        fingerprint(&recorded[&populations[0]]),
        fingerprint(&recorded[&populations[1]]),
        "the two populations must expose the same guard surface"
    );
}

#[test]
fn documents_flag_is_refused_outside_the_self_check() {
    let output = Command::new(example_binary())
        .args(["--documents", "7"])
        .current_dir(repo_root())
        .output()
        .expect("run the release probe example with --documents but no --self-check");
    assert!(
        !output.status.success(),
        "--documents outside --self-check must fail closed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--documents is only meaningful together with --self-check"),
        "the refusal must name the argument:\n{stderr}"
    );
}

#[test]
fn zero_document_self_check_is_refused() {
    let output = Command::new(example_binary())
        .args(["--self-check", "--documents", "0"])
        .current_dir(repo_root())
        .output()
        .expect("run the release probe guard self-check at population 0");
    assert!(
        !output.status.success(),
        "a zero-document population must be refused, not reported as a vacuous pass"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("a zero-document population is not a probe"),
        "the refusal must name the reason:\n{stdout}"
    );
}

#[test]
fn artifact_audit_refuses_a_data_population_that_left_the_frozen_table() {
    let report = load_committed_artifact();
    let manifest = manifest_yaml();
    let expected_digest = recompute_identity_digest(
        &yaml_at(&manifest, "external_holdout_contract")["release_probe"],
    );
    let margins = declared_margins(&manifest);
    let declared = declared_population();
    assert!(
        artifact_errors(&report, &expected_digest, &margins, declared).is_empty(),
        "the committed artifact must pass at the declared population"
    );
    // DATA moved off the frozen table: the audit refuses by name instead of
    // comparing the artifact against a stale table.
    assert_eq!(
        artifact_errors(&report, &expected_digest, &margins, declared + 2),
        vec!["probe:reference_population.frozen_table_drift".to_string()],
        "a population that left the frozen table must be refused by name"
    );
}

#[test]
fn committed_artifact_equals_two_independent_runs() {
    let binary = example_binary();
    let committed = fs::read(artifact_path()).expect("committed artifact");
    let mut streams: Vec<Vec<u8>> = Vec::new();
    for _ in 0..2 {
        // A different working directory proves the artifact is not a function of
        // the invoking cwd: the runner resolves everything from the repo root.
        let dir = tempdir().expect("temporary run directory");
        let report = dir.path().join("report.json");
        let output = Command::new(&binary)
            .args(["--report", report.to_str().expect("report path")])
            .current_dir(dir.path())
            .output()
            .expect("run the release probe example");
        assert!(
            output.status.success(),
            "runner failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        streams.push(fs::read(&report).expect("generated report bytes"));
    }
    assert_eq!(
        streams[0], streams[1],
        "two independent runs must be byte-identical"
    );
    assert_eq!(
        streams[0], committed,
        "the committed artifact must equal a fresh canonical run"
    );
}

#[test]
fn missing_report_fails_closed_without_writing_an_artifact() {
    let output = Command::new(example_binary())
        .current_dir(repo_root())
        .output()
        .expect("run the release probe example without --report");
    assert!(
        !output.status.success(),
        "the canonical run must refuse to start without --report"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--report <path> is required for the canonical run"),
        "the refusal must name the missing argument:\n{stderr}"
    );
}

/// The recorded measurement tables are DERIVED FROM the committed artifact, not
/// hand-typed: this guard re-reads
/// `artifacts/parse_compat/external_holdout_release_probe.json` and asserts that
/// the compiled `FROZEN_DOCUMENTS`, `frozen_aggregate` and `FROZEN_NARROWINGS`
/// are exactly its `outcomes`, aggregate and `class_narrowings`.  A hand edit, a
/// stale artifact, or a table regenerated without the other is a failure here.
/// The audit itself still compares a *mutated* artifact against these static
/// tables, so this guard does not make the mutation matrix self-referential.
#[test]
fn frozen_tables_are_derived_from_the_committed_artifact() {
    let report = load_committed_artifact();
    let outcomes = report["outcomes"].as_array().expect("outcomes");
    assert_eq!(outcomes.len(), FROZEN_DOCUMENTS.len());
    for (row, frozen) in outcomes.iter().zip(FROZEN_DOCUMENTS.iter()) {
        let (document_id, sha, source_population, admitted, agreement, low, high, width, latency) =
            *frozen;
        assert_eq!(row["document_id"].as_str(), Some(document_id));
        assert_eq!(row["identity_sha256"].as_str(), Some(sha));
        assert_eq!(row["source_population"].as_u64(), Some(source_population));
        assert_eq!(row["admitted"].as_u64(), Some(admitted));
        assert_eq!(row["agreement_ratio_micros"].as_u64(), Some(agreement));
        assert_eq!(row["uncertainty_low"].as_u64(), Some(low));
        assert_eq!(row["uncertainty_high"].as_u64(), Some(high));
        assert_eq!(row["uncertainty_width_micros"].as_u64(), Some(width));
        assert_eq!(row["latency_ms"].as_u64(), Some(latency));
    }

    let aggregate = &report["evaluated_population"]["aggregate"];
    let frozen = frozen_aggregate();
    assert_eq!(aggregate["source_population"].as_u64(), Some(frozen.0));
    assert_eq!(aggregate["admitted"].as_u64(), Some(frozen.1));
    assert_eq!(aggregate["tp"].as_u64(), Some(frozen.2));
    assert_eq!(aggregate["fn"].as_u64(), Some(frozen.3));
    assert_eq!(aggregate["fp"].as_u64(), Some(frozen.4));
    assert_eq!(aggregate["agreeing_documents"].as_u64(), Some(frozen.5));
    assert_eq!(aggregate["margin_pass"].as_u64(), Some(frozen.6));
    assert_eq!(aggregate["margin_fail"].as_u64(), Some(frozen.7));
    assert_eq!(
        aggregate["no_denominator_documents"].as_u64(),
        Some(frozen.8)
    );

    let artifact_narrowings: BTreeSet<(String, String, u64, u64)> = report["class_narrowings"]
        .as_array()
        .expect("class_narrowings")
        .iter()
        .map(|row| {
            (
                row["document_id"]
                    .as_str()
                    .expect("document_id")
                    .to_string(),
                row["class"].as_str().expect("class").to_string(),
                row["source_population"].as_u64().expect("source"),
                row["admitted"].as_u64().expect("admitted"),
            )
        })
        .collect();
    let frozen_narrowings: BTreeSet<(String, String, u64, u64)> = FROZEN_NARROWINGS
        .iter()
        .map(|(document_id, class, source_population, admitted)| {
            (
                (*document_id).to_string(),
                (*class).to_string(),
                *source_population,
                *admitted,
            )
        })
        .collect();
    assert_eq!(artifact_narrowings, frozen_narrowings);
}
