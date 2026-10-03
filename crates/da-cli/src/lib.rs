//! Library surface for pure CLI helpers (tests without binary entry).

pub mod acquire_arxiv;
pub mod agent_run;
pub mod audit_fields;
pub mod background_tasks;
pub mod batch_ids;
pub mod catalog_drift;
pub mod catalog_materialize;
pub mod catalog_regression;
pub mod catalog_sweep;
pub mod gold_corpus;
pub mod gold_graph;
pub mod graph_query;
pub mod graph_tasks;
pub mod health_report;
// S03/T04 (M514-1lss2s): typed receipt for the declared Java-free vertical
// slice. Pure std/serde/serde_yaml/sha2 (no da-parse, no pdf-layout), so the
// module and its tests compile in the default feature set; the gated example
// `java_free_vertical_slice` drives it against the real donors.
pub mod java_free_slice;
// S04/T04 (M514-1lss2s): typed family-parity receipt for the native header and
// bibliography families on the declared three-document slice. Pure
// std/serde/sha2 (no da-parse, no pdf-layout), so the module and its tests
// compile in the default feature set; the gated example `native_family_parity`
// drives it against the real positioned donor and the preserved-TEI oracle.
pub mod native_family_receipt;
// S04/T01 (M513-r7dg4z): host Java-CPU counter reader (bare integer or cgroup v2
// `cpu.stat` `usage_usec`). Host/cgroup probing is infrastructure, so it lives
// here rather than in `da-parse` (onion lock); typed absence, never a fake 0.
pub mod java_cpu_counter;
// S08/T01: layout_runtime holds the qualified CPU characterization primitive
// (`characterize_cpu_kernel`, `gate_simd_candidate`) that the native core
// profile consumes. It is pure std/serde/sha2, so it stays available in the
// default feature set: the profile module, its example, and its contract test
// must compile without `pdf-layout`.
pub mod layout_runtime;
pub mod load;
pub mod load_journey;
pub mod logging;
pub mod materialization_progress;
/// S08/T01 native CRF core profile and closed SIMD/PGO disposition.
pub mod native_core_profile;
// S08/T02: bounded SMP qualification of the same native CRF core over the
// worker matrix `1..=physical_core_cap`. Pure std/serde/sha2 + layout_runtime,
// so the module, its example and its contract test compile in the default
// feature set (no `pdf-layout`).
pub mod native_core_smp;
// S08/T03: measured Java work (per-model CRF invocations) projected from a
// verified GROBID stage tape. Pure serde/sha2 over `da_parse::StageTape`, so
// the module, its example and its contract test compile in the default feature
// set (no `pdf-layout`).
pub mod native_java_work;
// S08/T04: route-recovery ledger. Joins the closed P0..P9 cascade skeleton, the
// component admission matrix and the S08/T03 measured Java-work receipt into
// per-stage recovery verdicts. Pure serde/serde_yaml/sha2, so the module, its
// example and its contract test compile in the default feature set (no
// `pdf-layout`), and nothing here is a production route.
pub mod native_route_recovery;
// S05/T05 (M514-1lss2s): OCR/scanned vs digital document groups plus the
// required-family disposition vocabulary that structurally cannot say
// `out_of_scope`. Pure std/serde over `da_domain::PdfType`, so the module and
// its contract test compile in the default feature set (no `pdf-layout`, no
// `pdf-classifier`). It partitions a declared corpus and owns no classifier.
pub mod native_document_group;
// S05/T06 (M514-1lss2s): corpus-bound content receipt for the native body,
// section/reading-order, visual and callout bands. It reuses the S04 envelope
// (`native_family_receipt`) and the T05 group/disposition vocabularies
// (`native_document_group`) and is pure std/serde/sha2, so it and its contract
// test compile in the default feature set (no `pdf-layout`, no
// `pdf-classifier`).
pub mod native_content_receipt;
// S08/T01 (M514-1lss2s): corpus-bound native acceptance measurement runner with
// a typed gap ledger. Pure std/serde/serde_yaml/serde_json/sha2 (no da-parse, no
// pdf-layout), so the module and its contract test compile in the default
// feature set; the gated example `native_corpus_acceptance` drives it against
// the real 610-document frozen holdout.
pub mod native_acceptance;
// S01/T01 (M515-v2mvmc): the shared composition-measurement surface. It owns the
// compact scanner, the source-side `ProbeSource`, the `AdmittedBand`, and the
// fixed class enumeration once, so the external-holdout probe and the corpus
// acceptance runner measure the same `admitted / source_population` aggregate.
// It uses `da_parse` (an unconditional `da-cli` dependency) for the typed
// parsers that build the admitted band, and does not use `pdf-layout`, so it
// compiles in the default feature set.
pub mod native_composition_measurement;
// S06/T03 (M514-1lss2s): route receipt envelope and independent validator for
// the end-to-end PDF->native route. It reuses the S04 envelope
// (`native_family_receipt`) and binds the T02 prospective limit freeze, and is
// pure std/serde/sha2, so it and its contract test compile in the default
// feature set (no `pdf-layout`, no `pdf-classifier`).
pub mod native_route_receipt;
// S06/T06 (M514-1lss2s): same-n resource comparison receipt. It reuses the S04
// envelope (`native_family_receipt`) and the T03 freeze binding
// (`native_route_receipt`), pairs every native metric with a pinned measured
// reference row only where a same-n source exists, and records a typed
// `not_observed` otherwise (D510). Pure std/serde/sha2, so it and its contract
// test compile in the default feature set (no `pdf-layout`, no
// `pdf-classifier`).
pub mod native_resource_same_n;
// S08/T03 (M514-1lss2s): corpus-bound same-n resource receipt. It binds the
// native resources the T01 acceptance runner measured over the frozen
// 610-document holdout and pairs each metric with a pinned same-n reference row
// only where one exists — every reference metric is a typed `not_observed`
// absence, never a restated Java number (D510). It reuses the S04 envelope
// (`native_family_receipt`), the T03 freeze binding (`native_route_receipt`) and
// the S06 pairing/absence vocabulary (`native_resource_same_n`), and is pure
// std/serde/serde_yaml/serde_json/sha2, so it and its contract test compile in
// the default feature set (no `pdf-layout`, no `pdf-classifier`).
pub mod native_corpus_resource_same_n;
// S08/T04 (M514-1lss2s): real-corpus rollback rehearsal. It runs the declared
// baseline -> switched -> returned sequence in process memory only over the 610
// assigned holdout documents, proves `returned == baseline` on the bytes of the
// committed preserved-TEI oracle and refuses a leaked native fragment. It
// reuses the S04 envelope (`native_family_receipt`), the T03 freeze binding
// (`native_route_receipt`) and the T01 assigned identity (`native_acceptance`),
// and is pure std/serde/serde_yaml/serde_json/sha2, so it and its contract test
// compile in the default feature set (no `pdf-layout`, no `pdf-classifier`).
pub mod native_corpus_rollback_rehearsal;
// S08/T05 (M514-1lss2s): GROBID retirement acceptance package. It binds every
// S08 evidence carrier by path + pointer + sha256, carries the T02 evaluation
// and enumerates every mandatory gate with a typed state, computing
// `s09_authorized` from the blocking set and refusing a GSD-lifecycle
// authorization. It reuses the S04 envelope (`native_family_receipt`) and the
// T02 value types (`da_parse`), and is pure std/serde/serde_yaml/serde_json/sha2,
// so it and its contract test compile in the default feature set.
pub mod native_acceptance_package;
// S09/T02 (M514-1lss2s): canonical blocked retirement-readiness receipt. It
// binds the S08 acceptance package, the manifest human release record, the S02
// required-scope/authorization contract and the frozen M513 release package by
// path + pointer + sha256, carries the T01 authorization verdict verbatim with
// `families_switched = []` and `goal_unmet = true`, and refuses a switched
// family, a met goal, a renamed body residual, an opened lock, a claimed human
// decision or a product-coverage claim. It reuses the S04 envelope
// (`native_family_receipt`) and the T01 value types (`da_parse`), and is pure
// std/serde/serde_json, so it and its contract test compile in the default
// feature set.
pub mod native_retirement_readiness;
// S09/T04 (M514-1lss2s): process-local Java-disabled smoke rehearsal. It
// consumes the T01 authorization verdict and a declared per-family selection and
// is deliberately not a runtime route: the live state emits a typed
// `not_authorized` refusal (`families_switched = []`, `product_scope_confirmed =
// false`), and only an authorized rehearsal with an installed identity-isolated
// guard, zero Java consults and a non-vacuous positive control switches a family.
// It reuses the S04 envelope (`native_family_receipt`) and the T01/T02 value
// types (`da_parse`, `native_retirement_readiness`), and is pure
// std/serde/sha2, so it and its contract test compile in the default feature set.
pub mod native_java_disabled_smoke;
pub mod parse_batch;
pub mod parse_config;
pub mod parse_job;
pub mod parse_profile;
/// SUT-free reference producer for the closed parse-policy ablation (S07/T04).
pub mod policy_ablation_reference;
pub mod production;
pub mod snapshot_identity;
pub mod snapshot_lineage;
pub mod source_acquire;
pub mod source_cli;
pub mod summarize_revisions;
// S01/T01 (GAR-016): bounded, machine-checked verdict lineage for the
// report-producing CLI tests. Pure std/serde/sha2 over `da_domain::ContentDigest`,
// so the module and its unit tests compile in the default feature set (no
// `pdf-layout`). Test/library provenance only: no production route, no graph
// write, `import_eligible` stays false.
pub mod uat_verdict_lineage;
// S02/T01: donor visual sidecar → native visual observation converter. Every
// type it consumes (`da_adapters::VisualObservation`,
// `da_adapters::LayoutShadowObservation`) exists only with `pdf-layout`, and
// `default = []`, so this module must stay out of the default feature set. It
// is deliberately gated (unlike `layout_runtime`, which M507/S08 ungated): the
// gate keeps pdf-inspector out of the default `cargo check -p da-cli`, while
// the existing governor step `"da-cli feature-gated example contracts"`
// (`cargo test -p da-cli --features pdf-layout --tests`) still executes the
// converted unit tests through the `unittests src/lib.rs` target.
#[cfg(feature = "pdf-layout")]
pub mod visual_adapter;

/// ADR-051 §4.4 — JSON roundtrip helper for Serialize+Deserialize types.
#[cfg(test)]
pub(crate) fn roundtrip_json<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json1 = serde_json::to_string(value).expect("serialize failed");
    let back: T = serde_json::from_str(&json1).expect("deserialize failed");
    let json2 = serde_json::to_string(&back).expect("re-serialize failed");
    assert_eq!(json1, json2, "roundtrip JSON mismatch");
}

#[cfg(test)]
mod serde_roundtrip_tests {
    use super::*;
    use crate::graph_tasks::{GraphTask, GraphTaskSuite, TaskVerdict};
    use da_domain::graph_tasks::GraphTaskClass;

    #[test]
    fn roundtrip_graph_task_basic() {
        roundtrip_json(&GraphTask {
            id: "paper_count".into(),
            title: "Paper count | Scale".into(),
            insight: "Scale check".into(),
            cypher: "MATCH (n:Paper) RETURN count(n)".into(),
            min_rows: 1,
            allow_empty: false,
            expect_rows: None,
            expect_min_value: None,
            expect_value: None,
            class: GraphTaskClass::default_for_legacy(),
        });
    }

    #[test]
    fn roundtrip_graph_task_with_bounds() {
        roundtrip_json(&GraphTask {
            id: "paper_count".into(),
            title: "Paper count | Scale".into(),
            insight: "Scale check".into(),
            cypher: "MATCH (n:Paper) RETURN count(n)".into(),
            min_rows: 1,
            allow_empty: false,
            expect_rows: Some(1),
            expect_min_value: Some(24.0),
            expect_value: None,
            class: GraphTaskClass::Structural,
        });
    }

    #[test]
    fn roundtrip_graph_task_suite() {
        roundtrip_json(&GraphTaskSuite {
            tasks: vec![
                GraphTask {
                    id: "t1".into(),
                    title: "T1 | x".into(),
                    insight: "".into(),
                    cypher: "MATCH (n) RETURN n LIMIT 1".into(),
                    min_rows: 1,
                    allow_empty: false,
                    expect_rows: None,
                    expect_min_value: None,
                    expect_value: None,
                    class: GraphTaskClass::Structural,
                },
                GraphTask {
                    id: "t2".into(),
                    title: "T2 | y".into(),
                    insight: "".into(),
                    cypher: "MATCH (n) RETURN count(n)".into(),
                    min_rows: 1,
                    allow_empty: true,
                    expect_rows: Some(1),
                    expect_min_value: None,
                    expect_value: None,
                    class: GraphTaskClass::Fact,
                },
            ],
        });
    }

    #[test]
    fn roundtrip_task_verdict() {
        for v in [TaskVerdict::Pass, TaskVerdict::Empty, TaskVerdict::Error] {
            roundtrip_json(&v);
        }
    }
}
