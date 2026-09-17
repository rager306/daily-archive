//! S08/T02 contract gate for the bounded SMP qualification (M507-4fk6m6).
//!
//! Deliberately NOT feature-gated: the module, its example and this gate must
//! compile and run in the default feature set, otherwise a governor `da-cli
//! tests` step would cover only an empty target.
//!
//! What this gate proves:
//! 1. the semantic receipts are identical at EVERY worker count `1..=cap`, and
//!    the serial `workers = 1` digest is the oracle for all of them;
//! 2. a request above the physical core cap fails before a single job runs and
//!    is never silently clamped;
//! 3. exactly one terminal is published per admitted unit, admitted equals
//!    terminal, and the canonical ordinals are preserved;
//! 4. a panicking job becomes a typed coordinator failure without corrupting
//!    the other terminals;
//! 5. the runtime gauges are observed, not echoes of the configuration;
//! 6. per-worker receipts give real per-worker coverage;
//! 7. every breakpoint verdict is a member of the closed DATA vocabulary;
//! 8. a multi-worker semantic divergence is a hard typed error;
//! 9. the artifact carries the timing/observational declaration and stays
//!    redacted;
//! 10. the DATA gate refuses a weakened ledger and typed absence never becomes
//!     a fabricated number;
//! 11. the deterministic surface carries NO observational leaf path at all —
//!     proved by structurally enumerating every leaf path instead of scanning
//!     for the substring `latency` — while every declared observational field
//!     does exist on the observation surface, so the declaration is not
//!     decorative;
//! 12. perturbing each observational leaf of a cloned receipt, one at a time
//!     (latency percentiles, run wall time, throughput, queue and private
//!     scratch high-water marks, breakpoint verdict and reason, process peak
//!     RSS), changes the observation bytes and leaves the deterministic bytes
//!     byte-identical;
//! 13. two independent receipts over unchanged inputs share deterministic
//!     bytes but not observation bytes;
//! 14. `deterministic_projection.{digest,scope}` is persisted inside the
//!     artifact and recomputes from `deterministic_projection()`, with no
//!     self-reference onto the bytes that carry the digest;
//! 15. the observation surface declares itself a run snapshot rather than a
//!     receipt, and is absent from the DATA receipt list and from the route
//!     registry's `source_bindings` — only deterministic bytes are pinned.

use da_cli::layout_runtime::{
    CoordinatorFailure, Terminal, TopologyError, TopologyReader, TopologyReceipt,
    detect_physical_topology,
};
use da_cli::native_core_smp::{
    NATIVE_CORE_SMP_ARTIFACT_REL, NATIVE_CORE_SMP_DETERMINISTIC_PROJECTION_SCOPE,
    NATIVE_CORE_SMP_OBSERVATIONS_ARTIFACT_REL, NATIVE_CORE_SMP_OBSERVATIONS_SCHEMA_VERSION,
    NATIVE_CORE_SMP_SCHEMA_VERSION, NATIVE_CORE_SMP_TIMING_DISPOSITION, NATIVE_CORE_SMP_VERDICTS,
    NativeCoreSmpConfig, NativeCoreSmpError, NativeCoreSmpFamily, NativeCoreSmpJobReceipt,
    NativeCoreSmpJobSet, NativeCoreSmpReceipt, SMP_RUNTIME_OBSERVED_FIELDS, SMP_TIMING_FIELDS,
    SmpBreakpointVerdict, SmpJobError, SmpJobProcessor, SmpObservation, build_native_core_jobs,
    closed_breakpoint_vocabulary, combine_smp_matrix, load_cpu_smp_candidate_from_manifest,
    native_core_smp_processor, qualify_native_core_smp, run_smp_worker_count,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const MANIFEST: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const FULLTEXT: &[u8] =
    include_bytes!("../../da-parse/tests/fixtures/native_fulltext_synthetic.wapiti");
const FIGURE: &[u8] =
    include_bytes!("../../da-parse/tests/fixtures/native_figure_synthetic.wapiti");
const REFERENCE: &[u8] =
    include_bytes!("../../da-parse/tests/fixtures/native_bibliography_reference_synthetic.wapiti");

/// The DATA receipt list. It is read as text, not as a path, so the gate never
/// depends on the test process working directory.
const COMPOSITION_DATA: &str = include_str!("../../../data/parse/native_fulltext_composition.yaml");
/// The emitted route registry. Its `source_bindings` are what a downstream
/// reader treats as pinned inputs, so the observation surface must be absent
/// from them.
const ROUTE_REGISTRY: &str =
    include_str!("../../../artifacts/parse_compat/native_route_recovery.json");

const DOCUMENTS: usize = 3;
const ROWS_PER_DOCUMENT: usize = 8;

struct FixtureTopology;

impl TopologyReader for FixtureTopology {
    fn allowed_cpu_set(&self) -> Result<String, TopologyError> {
        Ok("0-3".into())
    }

    fn core_identity(&self, cpu: u32) -> Result<(String, String), TopologyError> {
        match cpu {
            0 | 1 => Ok(("0".into(), "0".into())),
            2 | 3 => Ok(("0".into(), "1".into())),
            other => Err(TopologyError::CpuIdentityUnavailable(other)),
        }
    }
}

fn fixture_topology() -> TopologyReceipt {
    detect_physical_topology(&FixtureTopology).expect("fixture topology")
}

fn fixture_families(rows_per_family: usize) -> Vec<NativeCoreSmpFamily> {
    [
        ("fulltext", FULLTEXT),
        ("figure", FIGURE),
        ("bibliography_reference", REFERENCE),
    ]
    .into_iter()
    .map(|(family, bytes)| {
        NativeCoreSmpFamily::from_frozen_model(family, bytes, rows_per_family)
            .expect("frozen synthetic model is part of the slice contract")
    })
    .collect()
}

fn config(repetitions: usize) -> NativeCoreSmpConfig {
    NativeCoreSmpConfig::new(DOCUMENTS, ROWS_PER_DOCUMENT, repetitions)
}

fn job_set() -> NativeCoreSmpJobSet {
    build_native_core_jobs(&fixture_families(DOCUMENTS * ROWS_PER_DOCUMENT), &config(1))
        .expect("fixture job set")
}

fn real_processor(job_set: &NativeCoreSmpJobSet) -> SmpJobProcessor {
    native_core_smp_processor(&fixture_families(
        job_set.documents_per_family * job_set.rows_per_document,
    ))
    .expect("fixture processor")
}

// ─────────── leaf-level determinism helpers (S09/T02) ───────────

/// One fixture receipt over the shared fixtures with an injected process peak
/// RSS. The value is injectable on purpose: a process gauge may only land on the
/// observation surface, so two calls with different values must still agree on
/// the deterministic bytes.
fn fixture_receipt(peak_rss_mib: u64) -> NativeCoreSmpReceipt {
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let topology = fixture_topology();
    let families = fixture_families(DOCUMENTS * ROWS_PER_DOCUMENT);
    let mut peak_rss = move || Some(peak_rss_mib * 1024 * 1024);
    qualify_native_core_smp(&data, &families, &topology, &config(1), &mut peak_rss)
        .expect("fixture matrix qualifies")
}

/// Recursively flatten a JSON value into its leaf paths, normalising every array
/// index segment to `[]`.
///
/// A scalar array contributes one `<path>[]` leaf; an array of objects or arrays
/// recurses per element, so identical element shapes collapse onto a single
/// canonical path. This is what makes the exclusion claim structural: the S08
/// guard was `!text.contains("latency")`, and a substring scan cannot see a
/// value that lives on a path whose name it does not literally spell.
fn collect_leaf_paths(value: &serde_json::Value, path: &str, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                collect_leaf_paths(child, &child_path, out);
            }
        }
        serde_json::Value::Array(items) => {
            let scalar_array = items.iter().all(|item| {
                !matches!(
                    item,
                    serde_json::Value::Object(_) | serde_json::Value::Array(_)
                )
            });
            if items.is_empty() || scalar_array {
                out.insert(format!("{path}[]"));
            } else {
                for item in items {
                    collect_leaf_paths(item, &format!("{path}[]"), out);
                }
            }
        }
        _ => {
            out.insert(path.to_string());
        }
    }
}

fn leaf_paths(value: &serde_json::Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    collect_leaf_paths(value, "", &mut out);
    out
}

/// Whether `path` is `name` itself or a descendant of it at segment granularity:
/// `...latency_ms.p50_ms` and `...latency_ms[]` are hits, while `...latency_ms_foo`
/// is not.
fn path_is_under(path: &str, name: &str) -> bool {
    match path.strip_prefix(name) {
        None => false,
        Some(rest) => rest.is_empty() || rest.starts_with('.') || rest.starts_with('['),
    }
}

/// The deterministic projection is a *subset view* of the deterministic
/// artifact, not a copy of it: `matrix.entries[].latency_ms` in artifact space is
/// `entries[].latency_ms` in projection space. A guard that reuses the
/// artifact-rooted names verbatim never matches the projection at all, which is
/// how a reintroduced observational field would survive a green test — the same
/// failure mode as the S08 substring guard, just one level down. So the declared
/// name is checked in both namespaces.
fn projection_path_is_observational(path: &str, declared: &str) -> bool {
    let relative = declared.strip_prefix("matrix.").unwrap_or(declared);
    path_is_under(path, declared) || path_is_under(path, relative)
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// The redaction policy, asserted identically on both emitted surfaces.
fn assert_surface_is_redacted(label: &str, text: &str, families: &[NativeCoreSmpFamily]) {
    assert!(
        !text.contains(".wapiti"),
        "{label}: no model filename may appear"
    );
    assert!(
        !text.contains("#mdl#") && !text.contains("#rdr#"),
        "{label}: no model bytes may appear"
    );
    for marker in ["/root/", "/home/", "/tmp/", "/usr/", "HOSTNAME"] {
        assert!(
            !text.contains(marker),
            "{label}: no physical path or hostname marker may appear: {marker}"
        );
    }
    for family in families {
        for row in family.rows.iter() {
            if row.len() >= 12 {
                assert!(
                    !text.contains(row.as_str()),
                    "{label}: leaked model-derived row content"
                );
            }
        }
    }
    if let Ok(hostname) = std::env::var("HOSTNAME") {
        let hostname = hostname.trim();
        if hostname.len() >= 4 {
            assert!(!text.contains(hostname), "{label}: leaked the hostname");
        }
    }
}

/// A processor that panics on its `panic_on`-th invocation and otherwise
/// executes the real kernel, so the coordinator behaviour is exercised on top
/// of real work.
fn panicking_processor(job_set: &NativeCoreSmpJobSet, panic_on: usize) -> SmpJobProcessor {
    let inner = real_processor(job_set);
    let calls = Arc::new(AtomicUsize::new(0));
    Arc::new(move |job, scratch| {
        let call = calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == panic_on {
            panic!("fixture panic at invocation {call}");
        }
        inner(job, scratch)
    })
}

#[test]
fn semantic_receipts_are_identical_at_every_worker_count() {
    let topology = fixture_topology();
    assert_eq!(topology.physical_cores, 2, "fixture cap");
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let mut peak_rss = || Some(64 * 1024 * 1024);

    let receipt = qualify_native_core_smp(
        &data,
        &fixture_families(DOCUMENTS * ROWS_PER_DOCUMENT),
        &topology,
        &config(2),
        &mut peak_rss,
    )
    .expect("fixture matrix qualifies");

    assert_eq!(receipt.workload.jobs, 9, "3 families x 3 documents");
    assert!(receipt.selection_forbidden);
    assert!(receipt.cpu_only);
    assert!(!receipt.subprocess_io_included);

    let oracle = receipt.matrix.scalar_oracle_digest.clone();
    assert!(oracle.starts_with("sha256:"));
    for entry in &receipt.matrix.entries {
        assert_eq!(
            entry.semantic_digest, oracle,
            "worker count {} changed the native core semantics",
            entry.workers
        );
        assert_eq!(entry.scalar_oracle_digest, oracle);
        assert!(entry.semantic_equal_to_scalar);
        assert_eq!(entry.repetitions, 2);
        assert!(entry.decoded_positions > 0);
        assert_eq!(entry.rows_decoded, 9 * ROWS_PER_DOCUMENT as u64);
    }

    // The two independent matrix runs of the emitter must agree on the
    // deterministic projection; the module already fails hard on divergence,
    // so here we assert the projection is stable across worker counts.
    let first = receipt.deterministic_projection().expect("projection");
    let second = receipt.deterministic_projection().expect("projection");
    assert_eq!(first, second);
    // Leaf-level, not substring. The S08 guard was
    // `!first.contains("latency") && !first.contains("throughput")`, which
    // cannot see an observational value living on a path whose name it does not
    // literally spell — and that is exactly how two declared-observational
    // fields survived on the deterministic surface. Parse the projection and
    // prove no declared observational name (or any descendant of it) is a leaf,
    // re-rooting the artifact-space names into projection space: the projection
    // has no `matrix.` prefix, so comparing the names verbatim is vacuous.
    let projection: serde_json::Value =
        serde_json::from_str(&first).expect("the deterministic projection is JSON");
    let projection_paths = leaf_paths(&projection);
    for name in SMP_TIMING_FIELDS
        .iter()
        .chain(SMP_RUNTIME_OBSERVED_FIELDS.iter())
    {
        assert!(
            !projection_paths
                .iter()
                .any(|path| projection_path_is_observational(path, name)),
            "observational field `{name}` appears as a leaf of the deterministic projection"
        );
    }
}

#[test]
fn over_cap_is_refused_before_any_job_runs_and_never_clamped() {
    let topology = fixture_topology();
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let jobs = job_set();
    let invoked = Arc::new(AtomicUsize::new(0));
    let counter = invoked.clone();
    let processor = real_processor(&jobs);
    let counting: SmpJobProcessor = Arc::new(move |job, scratch| {
        counter.fetch_add(1, Ordering::SeqCst);
        processor(job, scratch)
    });

    let result = run_smp_worker_count(
        &topology,
        topology.physical_cores + 1,
        &data,
        &jobs,
        &config(1),
        &counting,
    );

    assert!(matches!(
        result,
        Err(NativeCoreSmpError::OverCoreCap {
            requested: 3,
            cap: 2
        })
    ));
    assert_eq!(
        invoked.load(Ordering::SeqCst),
        0,
        "over-cap validation must precede the first job invocation, and no clamp may hide it"
    );
}

#[test]
fn one_terminal_per_admitted_unit_with_canonical_ordinals() {
    let topology = fixture_topology();
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let jobs = job_set();
    let processor = real_processor(&jobs);

    for workers in 1..=topology.physical_cores {
        let entry = run_smp_worker_count(&topology, workers, &data, &jobs, &config(3), &processor)
            .expect("fixture worker count");
        assert_eq!(entry.admitted, jobs.jobs.len());
        assert_eq!(entry.terminals, jobs.jobs.len());
        assert_eq!(entry.terminals, entry.ordinals.len());
        assert_eq!(
            entry.ordinals,
            (0..jobs.jobs.len()).collect::<Vec<_>>(),
            "canonical ordinal publication at {workers} workers"
        );
        assert_eq!(entry.completed, jobs.jobs.len());
        assert_eq!(entry.failed, 0);
        assert_eq!(entry.coordinator_failed, 0);
        assert!(entry.coordinator_failure.is_none());
        let per_worker_total: usize = entry
            .per_worker_terminal_counts
            .iter()
            .map(|counts| counts.completed + counts.failed + counts.coordinator_failed)
            .sum();
        assert_eq!(per_worker_total, entry.terminals);
        assert!(entry.scratch_high_water_bytes > 0);
        assert!(entry.scratch_high_water_bytes <= entry.scratch_capacity_bytes);
        assert!(entry.scratch_capacity_bytes >= jobs.scratch_capacity);
    }
}

#[test]
fn worker_panic_is_a_typed_coordinator_failure_and_spares_the_other_terminals() {
    let topology = fixture_topology();
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let jobs = job_set();
    let processor = panicking_processor(&jobs, 3);

    let entry = run_smp_worker_count(&topology, 2, &data, &jobs, &config(1), &processor)
        .expect("a coordinator failure is recorded, not promoted to an error here");

    assert_eq!(entry.admitted, jobs.jobs.len());
    assert_eq!(entry.terminals, jobs.jobs.len(), "no terminal was lost");
    assert_eq!(entry.ordinals, (0..jobs.jobs.len()).collect::<Vec<_>>());
    assert_eq!(entry.completed, jobs.jobs.len() - 1);
    assert_eq!(entry.coordinator_failed, 1);
    assert_eq!(entry.failed, 0, "a panic is not a typed job failure");
    let failure = entry
        .coordinator_failure
        .as_ref()
        .expect("the panic must not be silent");
    assert_eq!(failure.kind, "worker_panic");
    assert!(
        data.terminal_receipts
            .failure_vocabulary
            .iter()
            .any(|kind| kind == &failure.kind)
    );
    let per_worker_total: usize = entry
        .per_worker_terminal_counts
        .iter()
        .map(|counts| counts.completed + counts.failed + counts.coordinator_failed)
        .sum();
    assert_eq!(per_worker_total, entry.terminals);

    // The same panic through the coordinator must never be reported as a clean
    // qualification.
    let mut entries = vec![
        run_smp_worker_count(
            &topology,
            1,
            &data,
            &jobs,
            &config(1),
            &real_processor(&jobs),
        )
        .expect("serial oracle"),
        entry,
    ];
    entries[1].workers = 2;
    let combined = combine_smp_matrix(&data, entries, &SmpObservation::observed(1024));
    assert!(matches!(
        combined,
        Err(NativeCoreSmpError::CoordinatorFailed { kind, .. }) if kind == "worker_panic"
    ));
}

#[test]
fn runtime_gauges_are_observed_not_configuration_echoes() {
    let topology = fixture_topology();
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    // One document only: a gauge that echoed its configured capacity would
    // report 2 for workers=2, which this assertion refuses.
    let single = NativeCoreSmpConfig::new(1, ROWS_PER_DOCUMENT, 1);
    let families = vec![
        NativeCoreSmpFamily::from_frozen_model("fulltext", FULLTEXT, ROWS_PER_DOCUMENT)
            .expect("fixture family"),
    ];
    let jobs = build_native_core_jobs(&families, &single).expect("single-document job set");
    let processor = real_processor(&jobs);

    let entry = run_smp_worker_count(&topology, 2, &data, &jobs, &single, &processor)
        .expect("single-document run");

    assert_eq!(entry.jobs, 1);
    assert_eq!(entry.queue.input_capacity, 2);
    assert_eq!(entry.queue.result_capacity, 2);
    assert_eq!(entry.queue.max_in_flight, 2);
    assert_eq!(entry.queue.input_high_water, 1);
    assert_eq!(entry.queue.result_high_water, 1);
    assert_eq!(entry.queue.in_flight_high_water, 1);
    assert!(
        entry.queue.in_flight_high_water <= entry.queue.max_in_flight,
        "in-flight work is bounded by the configured admission budget"
    );
    // The private scratch gauge reflects the rendered document, not the arena.
    assert!(entry.scratch_high_water_bytes > 0);
    assert!(entry.scratch_high_water_bytes <= entry.scratch_capacity_bytes);
}

#[test]
fn per_worker_receipts_report_per_worker_coverage() {
    let topology = fixture_topology();
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let jobs = job_set();
    let processor = real_processor(&jobs);

    let entry = run_smp_worker_count(&topology, 2, &data, &jobs, &config(1), &processor)
        .expect("two-worker run");

    assert_eq!(entry.per_worker_terminal_counts.len(), 2);
    let workers = entry
        .per_worker_terminal_counts
        .iter()
        .map(|counts| counts.worker)
        .collect::<Vec<_>>();
    assert_eq!(workers, vec![0, 1]);
    assert!(
        entry
            .per_worker_terminal_counts
            .iter()
            .all(|counts| counts.completed > 0)
    );
    assert_eq!(
        entry.per_worker_scratch_high_water_bytes.len(),
        2,
        "one private scratch high-water per worker"
    );
    assert!(
        entry
            .per_worker_scratch_high_water_bytes
            .iter()
            .all(|high| *high > 0)
    );
}

#[test]
fn every_breakpoint_verdict_is_a_member_of_the_closed_vocabulary() {
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let closed = closed_breakpoint_vocabulary();
    let topology = fixture_topology();
    let mut entries = Vec::new();
    let jobs = job_set();
    let processor = real_processor(&jobs);
    for workers in 1..=topology.physical_cores {
        entries.push(
            run_smp_worker_count(&topology, workers, &data, &jobs, &config(1), &processor)
                .expect("fixture worker count"),
        );
    }
    let matrix = combine_smp_matrix(&data, entries, &SmpObservation::observed(32 * 1024 * 1024))
        .expect("combined matrix");

    assert_eq!(matrix.breakpoints.len(), topology.physical_cores);
    assert_eq!(matrix.breakpoint_vocabulary, data.breakpoints.verdicts);
    for record in &matrix.breakpoints {
        let name = record.verdict.as_str();
        assert!(
            closed.contains(name),
            "verdict `{name}` is outside the closed vocabulary"
        );
        assert!(
            data.breakpoints.verdicts.iter().any(|v| v == name),
            "verdict `{name}` is not declared by the DATA ledger"
        );
        assert!(!record.reason.is_empty());
    }
    assert_eq!(matrix.entries[0].workers, 1);

    // The DATA vocabulary is itself closed: dropping a member is refused.
    let mut weakened = data.clone();
    weakened.breakpoints.verdicts.retain(|v| v != "saturation");
    assert!(matches!(
        weakened.validate(),
        Err(NativeCoreSmpError::DataContractViolation { .. })
    ));
}

#[test]
fn semantic_divergence_across_worker_counts_is_a_hard_error() {
    let topology = fixture_topology();
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let jobs = job_set();
    let processor = real_processor(&jobs);

    let oracle =
        run_smp_worker_count(&topology, 1, &data, &jobs, &config(1), &processor).expect("oracle");
    let mut drifted =
        run_smp_worker_count(&topology, 2, &data, &jobs, &config(1), &processor).expect("second");
    drifted.semantic_digest = format!("sha256:{:064x}", 0xdead_beef_u64);

    let combined = combine_smp_matrix(
        &data,
        vec![oracle, drifted],
        &SmpObservation::observed(1024),
    );
    assert!(
        matches!(
            combined,
            Err(NativeCoreSmpError::SemanticDivergence { workers: 2, .. })
        ),
        "a worker-count-dependent digest must fail loudly, got {combined:?}"
    );
}

#[test]
fn artifact_declares_observational_timing_and_stays_redacted() {
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let topology = fixture_topology();
    let families = fixture_families(DOCUMENTS * ROWS_PER_DOCUMENT);
    let mut peak_rss = || Some(48 * 1024 * 1024);
    let receipt = qualify_native_core_smp(&data, &families, &topology, &config(1), &mut peak_rss)
        .expect("fixture matrix qualifies");
    let artifact = receipt.artifact();
    assert_eq!(artifact.schema_version, NATIVE_CORE_SMP_SCHEMA_VERSION);
    assert_eq!(
        artifact.timing_observational.disposition,
        NATIVE_CORE_SMP_TIMING_DISPOSITION
    );
    assert!(
        artifact
            .timing_observational
            .excluded_from
            .iter()
            .any(|field| field == "acceptance_denominators")
    );
    // Exact membership, never a substring: `field.contains("throughput")` is
    // the same weak shape as the S08 guard and would accept any field whose name
    // merely mentions the token. The declaration must equal the implemented set;
    // the leaf-level exclusion of exactly those fields is proved below and in
    // `deterministic_artifact_has_no_observational_leaf_path`.
    assert_eq!(
        artifact.timing_observational.timing_fields,
        SMP_TIMING_FIELDS.to_vec()
    );
    assert_eq!(
        artifact.timing_observational.runtime_observed_fields,
        SMP_RUNTIME_OBSERVED_FIELDS.to_vec()
    );
    assert!(artifact.denominators.acceptance.fixed);
    assert_eq!(artifact.denominators.acceptance.total_lane_pages, 606);
    assert!(
        artifact.denominators.benchmark.separate_from_acceptance,
        "benchmark repetitions must stay separate from acceptance"
    );
    assert_eq!(
        artifact.denominators.benchmark.pooled, "forbidden",
        "benchmark repetitions must never be pooled into acceptance"
    );
    assert_eq!(
        artifact.denominators.benchmark.repetitions_per_worker_count,
        config(1).repetitions
    );
    assert_eq!(artifact.h05_closure, "not_claimed");
    assert_eq!(artifact.promotion.selected, "forbidden");
    assert_eq!(artifact.promotion.promoted, "forbidden");
    assert_eq!(artifact.promotion.auto_promote, "forbidden");
    assert!(artifact.selection_forbidden);
    assert!(artifact.cpu_only);
    assert!(!artifact.subprocess_io_included);
    for metric in &data.metrics.required {
        assert!(
            artifact
                .metrics
                .required
                .iter()
                .any(|entry| entry == metric),
            "DATA metric `{metric}` must be echoed by the artifact"
        );
    }

    // The declaration is carried, but the *values* are not — proved at leaf
    // level, because `timing_fields` itself literally spells the observational
    // names, so any substring scan still "finds" them here.
    assert_eq!(
        artifact.timing_observational.observations_artifact,
        NATIVE_CORE_SMP_OBSERVATIONS_ARTIFACT_REL,
        "the deterministic surface must name where the observational values live"
    );
    assert_eq!(
        artifact.timing_observational.observations_schema_version,
        NATIVE_CORE_SMP_OBSERVATIONS_SCHEMA_VERSION
    );
    let artifact_json: serde_json::Value =
        serde_json::from_slice(&receipt.artifact_json_bytes().expect("artifact bytes"))
            .expect("artifact is JSON");
    let deterministic_paths = leaf_paths(&artifact_json);
    for name in SMP_TIMING_FIELDS
        .iter()
        .chain(SMP_RUNTIME_OBSERVED_FIELDS.iter())
    {
        assert!(
            !deterministic_paths
                .iter()
                .any(|path| path_is_under(path, name)),
            "the deterministic surface carries a leaf for the observational field `{name}`"
        );
    }

    let text = String::from_utf8(receipt.artifact_json_bytes().expect("artifact bytes"))
        .expect("artifact is utf8");
    assert_surface_is_redacted("deterministic surface", &text, &families);
    for family in &artifact.families {
        assert!(
            text.contains(&family.model_sha256),
            "family {} must stay bound by model digest",
            family.family
        );
    }

    // The same redaction policy must hold on the observation surface: it is the
    // second half of the boundary claim, and an unredacted snapshot would leak
    // exactly what the deterministic surface refuses to carry.
    let deterministic_sha256 = sha256_prefixed(&receipt.artifact_json_bytes().expect("bytes"));
    let observations_text = String::from_utf8(
        receipt
            .observations_json_bytes(&deterministic_sha256)
            .expect("observation bytes"),
    )
    .expect("observation surface is utf8");
    assert_surface_is_redacted("observation surface", &observations_text, &families);
}

#[test]
fn typed_absence_is_serialized_without_a_number() {
    let absent: SmpObservation<u64> = SmpObservation::not_observed("not measured here");
    let json = serde_json::to_value(&absent).expect("serialize");
    assert_eq!(json["state"], "not_observed");
    assert!(json.get("value").is_none());

    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let topology = fixture_topology();
    let families = fixture_families(DOCUMENTS * ROWS_PER_DOCUMENT);
    // The injected reader reports absence: the receipt must say so instead of
    // inventing a zero.
    let mut no_rss = || None;
    let receipt = qualify_native_core_smp(&data, &families, &topology, &config(1), &mut no_rss)
        .expect("fixture matrix qualifies");
    assert!(!receipt.peak_rss_bytes.is_observed());
    assert!(
        !receipt.memory_pressure_ceiling_bytes.is_observed(),
        "no DATA ceiling means memory_pressure is not observable"
    );
    for entry in &receipt.matrix.entries {
        assert!(
            !entry.queue_wait_ms.is_observed(),
            "queue_wait_ms is declared but not instrumented; it must not be fabricated"
        );
    }
    let json = serde_json::to_value(receipt.artifact()).expect("artifact json");
    assert!(json["peak_rss_bytes"].get("value").is_none());
}

#[test]
fn weakened_ledger_contracts_are_refused_at_the_loader() {
    let tampered = MANIFEST.replace("silent_clamp: forbidden", "silent_clamp: allowed");
    assert!(
        matches!(
            load_cpu_smp_candidate_from_manifest(&tampered),
            Err(NativeCoreSmpError::DataContractViolation { .. })
        ),
        "a clamp-permitting ledger must be refused"
    );

    let tampered = MANIFEST.replace(
        "    verdicts: [not_observed, saturation, first_regression, queue_pressure, memory_pressure, inconclusive]",
        "    verdicts: [not_observed, saturation, first_regression]",
    );
    assert!(
        matches!(
            load_cpu_smp_candidate_from_manifest(&tampered),
            Err(NativeCoreSmpError::DataContractViolation { .. })
        ),
        "an opened breakpoint vocabulary must be refused"
    );

    let tampered = MANIFEST.replace("    auto_promote: forbidden", "    auto_promote: allowed");
    assert!(
        matches!(
            load_cpu_smp_candidate_from_manifest(&tampered),
            Err(NativeCoreSmpError::DataContractViolation { .. })
        ),
        "an auto-promoting ledger must be refused"
    );

    assert!(matches!(
        load_cpu_smp_candidate_from_manifest("not: [a, yaml, mapping"),
        Err(NativeCoreSmpError::DataUnreadable { .. })
    ));
}

#[test]
fn missing_ledger_block_is_a_violation_not_silence() {
    assert!(matches!(
        load_cpu_smp_candidate_from_manifest("other_block: {}\n"),
        Err(NativeCoreSmpError::DataContractViolation { path, .. }) if path == "cpu_smp_candidate"
    ));
}

#[test]
fn real_topology_matrix_is_green_on_this_machine() {
    let Ok(topology) = da_cli::layout_runtime::detect_system_topology() else {
        // A platform without the Linux topology reader is honestly skipped
        // rather than faked.
        return;
    };
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let families = fixture_families(2 * 4);
    let small = NativeCoreSmpConfig::new(2, 4, 1);
    let mut peak_rss = || None;

    let receipt = qualify_native_core_smp(&data, &families, &topology, &small, &mut peak_rss)
        .expect("the real worker matrix must qualify on this machine");

    assert_eq!(receipt.topology.physical_cores, topology.physical_cores);
    assert_eq!(
        receipt.worker_matrix(),
        (1..=topology.physical_cores).collect::<Vec<_>>(),
        "the sweep must cover 1..=physical_core_cap"
    );
    assert_eq!(
        receipt.matrix.scalar_oracle_digest,
        receipt.matrix.entries[0].semantic_digest
    );
    for entry in &receipt.matrix.entries {
        assert_eq!(entry.semantic_digest, receipt.matrix.scalar_oracle_digest);
        assert_eq!(entry.admitted, entry.terminals);
    }
    assert!(
        NATIVE_CORE_SMP_VERDICTS
            .iter()
            .any(|verdict| receipt.matrix.breakpoints[0].verdict.as_str() == *verdict)
    );
}

#[test]
fn job_scratch_exhaustion_is_a_typed_job_error() {
    let data = load_cpu_smp_candidate_from_manifest(MANIFEST).expect("cpu_smp_candidate block");
    let topology = fixture_topology();
    let families = fixture_families(DOCUMENTS * ROWS_PER_DOCUMENT);
    let jobs = build_native_core_jobs(&families, &config(1)).expect("job set");
    // A processor that always fails in a typed way: the qualification must
    // report the failure instead of publishing an empty success.
    let failing: SmpJobProcessor = Arc::new(|_job, _scratch| {
        Err::<NativeCoreSmpJobReceipt, SmpJobError>(SmpJobError::Kernel {
            family: "fixture".to_string(),
            document: 0,
            reason: "fixture".to_string(),
        })
    });
    let result = run_smp_worker_count(&topology, 1, &data, &jobs, &config(1), &failing);
    assert!(
        matches!(
            result,
            Err(NativeCoreSmpError::JobFailed { workers: 1, .. })
        ),
        "a typed job failure must not be reported as a clean run, got {result:?}"
    );

    // A panic through the same path is a coordinator failure, never a job error.
    let panicking = panicking_processor(&jobs, usize::MAX);
    let entry = run_smp_worker_count(&topology, 1, &data, &jobs, &config(1), &panicking)
        .expect("no panic is triggered");
    assert!(entry.coordinator_failure.is_none());
    assert_eq!(entry.terminals, jobs.jobs.len());
}

// ─────────── leaf-level determinism boundary (S09/T02) ───────────

/// The S08 defect, reproduced as a gate: every field the artifact itself
/// declares observational may change freely on a clone and the *deterministic*
/// bytes must not move — while the observation bytes must move, so no case is
/// vacuous. One field is perturbed at a time, for every matrix entry and every
/// breakpoint, not just the first.
#[test]
fn perturbing_every_observational_field_leaves_the_deterministic_bytes_unchanged() {
    let receipt = fixture_receipt(48);
    let deterministic = receipt.artifact_json_bytes().expect("deterministic bytes");
    let deterministic_sha256 = sha256_prefixed(&deterministic);
    let observations = receipt
        .observations_json_bytes(&deterministic_sha256)
        .expect("observation bytes");
    assert_eq!(receipt.matrix.entries.len(), 2, "fixture cap is 2 cores");
    assert_eq!(receipt.matrix.breakpoints.len(), 2, "one verdict per entry");

    let mut entry_indices: BTreeSet<usize> = BTreeSet::new();
    entry_indices.insert(0);
    entry_indices.insert(receipt.matrix.entries.len() - 1);
    let mut breakpoint_indices: BTreeSet<usize> = BTreeSet::new();
    breakpoint_indices.insert(0);
    breakpoint_indices.insert(receipt.matrix.breakpoints.len() - 1);

    type Perturbation = Box<dyn Fn(&mut NativeCoreSmpReceipt)>;
    let mut cases: Vec<(String, Perturbation)> = Vec::new();
    for index in &entry_indices {
        let index = *index;
        cases.push((
            format!("matrix.entries[{index}].latency_ms.p50_ms"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index].latency_ms.p50_ms += 7;
            }),
        ));
        cases.push((
            format!("matrix.entries[{index}].latency_ms.p95_ms"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index].latency_ms.p95_ms += 11;
            }),
        ));
        cases.push((
            format!("matrix.entries[{index}].latency_ms.p99_ms"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index].latency_ms.p99_ms += 13;
            }),
        ));
        cases.push((
            format!("matrix.entries[{index}].run_wall_ms"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index].run_wall_ms += 17;
            }),
        ));
        cases.push((
            format!("matrix.entries[{index}].throughput_units_per_second"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index].throughput_units_per_second += 19;
            }),
        ));
        cases.push((
            format!("matrix.entries[{index}].queue.input_high_water"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index].queue.input_high_water += 1;
            }),
        ));
        cases.push((
            format!("matrix.entries[{index}].queue.result_high_water"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index].queue.result_high_water += 1;
            }),
        ));
        cases.push((
            format!("matrix.entries[{index}].queue.in_flight_high_water"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index].queue.in_flight_high_water += 1;
            }),
        ));
        cases.push((
            format!("matrix.entries[{index}].scratch_high_water_bytes"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index].scratch_high_water_bytes += 1;
            }),
        ));
        cases.push((
            format!("matrix.entries[{index}].per_worker_scratch_high_water_bytes"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.entries[index]
                    .per_worker_scratch_high_water_bytes
                    .push(1);
            }),
        ));
    }
    for index in &breakpoint_indices {
        let index = *index;
        cases.push((
            format!("matrix.breakpoints[{index}].verdict"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                let current = r.matrix.breakpoints[index].verdict;
                r.matrix.breakpoints[index].verdict =
                    if current == SmpBreakpointVerdict::Inconclusive {
                        SmpBreakpointVerdict::Saturation
                    } else {
                        SmpBreakpointVerdict::Inconclusive
                    };
            }),
        ));
        cases.push((
            format!("matrix.breakpoints[{index}].reason"),
            Box::new(move |r: &mut NativeCoreSmpReceipt| {
                r.matrix.breakpoints[index]
                    .reason
                    .push_str(" | perturbed by the leaf gate");
            }),
        ));
    }
    cases.push((
        "peak_rss_bytes".to_string(),
        Box::new(|r: &mut NativeCoreSmpReceipt| {
            r.peak_rss_bytes = SmpObservation::observed(64 * 1024 * 1024);
        }),
    ));

    // A silently shortened case list would weaken the proof without failing it.
    assert_eq!(
        cases.len(),
        entry_indices.len() * 10 + breakpoint_indices.len() * 2 + 1
    );

    for (name, perturb) in &cases {
        let mut perturbed = receipt.clone();
        perturb(&mut perturbed);
        let perturbed_deterministic = perturbed.artifact_json_bytes().expect("perturbed bytes");
        assert!(
            perturbed_deterministic == deterministic,
            "perturbing `{name}` changed the deterministic artifact bytes"
        );
        let perturbed_observations = perturbed
            .observations_json_bytes(&deterministic_sha256)
            .expect("perturbed observation bytes");
        assert!(
            perturbed_observations != observations,
            "perturbing `{name}` left the observation surface byte-identical, so the case proves \
nothing"
        );
    }
}

/// The exclusion, proved by enumerating every leaf path of the emitted artifact
/// rather than by scanning for a substring. `matrix.entries[].queue_wait_ms` is
/// explicitly *not* an exception: it is a declared absence, and it must stay on
/// the deterministic side carrying a state, not a number.
#[test]
fn deterministic_artifact_has_no_observational_leaf_path() {
    let receipt = fixture_receipt(48);
    let artifact_json: serde_json::Value =
        serde_json::from_slice(&receipt.artifact_json_bytes().expect("deterministic bytes"))
            .expect("the deterministic artifact is JSON");
    let paths = leaf_paths(&artifact_json);

    // Declaration and implementation must name the same set, otherwise this
    // gate could enumerate one list while the artifact declares another.
    let declared = receipt.artifact().timing_observational;
    assert_eq!(declared.timing_fields, SMP_TIMING_FIELDS.to_vec());
    assert_eq!(
        declared.runtime_observed_fields,
        SMP_RUNTIME_OBSERVED_FIELDS.to_vec()
    );

    for name in SMP_TIMING_FIELDS
        .iter()
        .chain(SMP_RUNTIME_OBSERVED_FIELDS.iter())
    {
        let offenders: Vec<&String> = paths
            .iter()
            .filter(|path| path_is_under(path, name))
            .collect();
        assert!(
            offenders.is_empty(),
            "the deterministic surface carries leaf path(s) under the observational field `{name}`: \
{offenders:?}"
        );
    }

    // Breakpoints are classifications of observational throughput: neither the
    // verdicts nor their reasons may be reachable from this surface.
    for name in [
        "matrix.breakpoints[].verdict",
        "matrix.breakpoints[].reason",
        "matrix.breakpoints",
    ] {
        let offenders: Vec<&String> = paths
            .iter()
            .filter(|path| path_is_under(path, name))
            .collect();
        assert!(
            offenders.is_empty(),
            "the deterministic surface exposes `{name}`: {offenders:?}"
        );
    }

    // The queue high-water marks are gone while the capacities they are
    // measured against stay: those are bounded by the validated runtime
    // configuration and therefore reproduce.
    assert!(paths.contains("matrix.entries[].queue.input_capacity"));
    assert!(paths.contains("matrix.entries[].queue.result_capacity"));
    assert!(paths.contains("matrix.entries[].queue.max_in_flight"));

    // `queue_wait_ms` is not an exception to the exclusion — it is a declared
    // absence, which is itself deterministic, and it carries no value leaf.
    assert!(paths.contains("matrix.entries[].queue_wait_ms.state"));
    assert!(paths.contains("matrix.entries[].queue_wait_ms.reason"));
    assert!(
        !paths
            .iter()
            .any(|path| path.starts_with("matrix.entries[].queue_wait_ms.value")),
        "a typed absence must never carry a fabricated number"
    );
    let queue_wait = &artifact_json["matrix"]["entries"][0]["queue_wait_ms"];
    assert_eq!(queue_wait["state"], "not_observed");
    assert!(queue_wait.get("value").is_none());

    // Positive polarity for the same boundary: the private scratch *capacity*
    // is deterministic, its high-water marks are not.
    assert!(paths.contains("matrix.entries[].scratch_capacity_bytes"));
    assert!(!paths.contains("matrix.entries[].scratch_high_water_bytes"));
}

/// Every declared observational field must be reachable on the observation
/// surface. A declaration with no corresponding leaf would be decorative.
#[test]
fn every_declared_observational_field_exists_on_the_observation_surface() {
    let receipt = fixture_receipt(48);
    let artifact = receipt.artifact();
    let declared = &artifact.timing_observational;
    assert_eq!(
        declared.observations_artifact,
        NATIVE_CORE_SMP_OBSERVATIONS_ARTIFACT_REL
    );
    assert_eq!(
        declared.observations_schema_version,
        NATIVE_CORE_SMP_OBSERVATIONS_SCHEMA_VERSION
    );
    assert_eq!(declared.timing_fields, SMP_TIMING_FIELDS.to_vec());
    assert_eq!(
        declared.runtime_observed_fields,
        SMP_RUNTIME_OBSERVED_FIELDS.to_vec()
    );
    assert!(!declared.excluded_from.is_empty());

    let deterministic_sha256 = sha256_prefixed(&receipt.artifact_json_bytes().expect("bytes"));
    let observation_json: serde_json::Value = serde_json::from_slice(
        &receipt
            .observations_json_bytes(&deterministic_sha256)
            .expect("observation bytes"),
    )
    .expect("the observation surface is JSON");
    let observation_paths = leaf_paths(&observation_json);

    for name in declared
        .timing_fields
        .iter()
        .chain(declared.runtime_observed_fields.iter())
    {
        assert!(
            observation_paths
                .iter()
                .any(|path| path_is_under(path, name)),
            "declared observational field `{name}` has no leaf path on the observation surface, so \
the declaration is decorative"
        );
    }
}

/// Two independent receipts over unchanged inputs: identical deterministic
/// bytes, different observation bytes. This is the property S08 claimed and did
/// not have.
///
/// Stated rather than hidden: with sub-millisecond fixture jobs the wall-clock
/// fields can coincide, so this byte equality is *empirical* evidence. It is the
/// structural gates beside it — `perturbing_every_observational_field_...` and
/// `deterministic_artifact_has_no_observational_leaf_path` — that make the claim
/// robust; they are what fail when a declared-observational field is
/// reintroduced onto this surface.
#[test]
fn two_independent_receipts_share_deterministic_bytes_but_not_observations() {
    let first = fixture_receipt(48);
    let second = fixture_receipt(64);

    let first_deterministic = first.artifact_json_bytes().expect("first bytes");
    let second_deterministic = second.artifact_json_bytes().expect("second bytes");
    assert!(
        first_deterministic == second_deterministic,
        "two independent receipts over unchanged inputs must agree byte-for-byte on the \
deterministic surface"
    );

    let deterministic_sha256 = sha256_prefixed(&first_deterministic);
    let first_observations = first
        .observations_json_bytes(&deterministic_sha256)
        .expect("first observation bytes");
    let second_observations = second
        .observations_json_bytes(&deterministic_sha256)
        .expect("second observation bytes");
    assert!(
        first_observations != second_observations,
        "the observation surfaces must differ: they carry wall-clock gauges and the injected peak RSS"
    );

    let first_json: serde_json::Value =
        serde_json::from_slice(&first_observations).expect("first observation JSON");
    let second_json: serde_json::Value =
        serde_json::from_slice(&second_observations).expect("second observation JSON");
    assert_eq!(first_json["peak_rss_bytes"]["value"], 48 * 1024 * 1024_u64);
    assert_eq!(second_json["peak_rss_bytes"]["value"], 64 * 1024 * 1024_u64);
    // Both snapshots nevertheless bind the *same* deterministic object.
    assert_eq!(
        first_json["source_artifact"]["sha256"],
        second_json["source_artifact"]["sha256"]
    );
    assert_eq!(
        first_json["source_artifact"]["sha256"],
        deterministic_sha256
    );
}

/// The projection digest is *persisted*, not printed once and dropped, and it
/// recomputes from the projection — never from the bytes that carry it, so there
/// is no self-reference.
#[test]
fn declared_deterministic_projection_digest_recomputes() {
    let receipt = fixture_receipt(48);
    let artifact_json: serde_json::Value =
        serde_json::from_slice(&receipt.artifact_json_bytes().expect("bytes"))
            .expect("artifact is JSON");
    let declared = artifact_json["deterministic_projection"]["digest"]
        .as_str()
        .expect("the artifact must persist `deterministic_projection.digest`")
        .to_string();
    let scope: Vec<String> = artifact_json["deterministic_projection"]["scope"]
        .as_array()
        .expect("the artifact must persist `deterministic_projection.scope`")
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .expect("scope entries are strings")
                .to_string()
        })
        .collect();

    let projection = receipt.deterministic_projection().expect("projection");
    assert_eq!(declared, sha256_prefixed(projection.as_bytes()));
    assert_eq!(declared, receipt.deterministic_projection_digest);
    assert_eq!(
        scope,
        NATIVE_CORE_SMP_DETERMINISTIC_PROJECTION_SCOPE
            .iter()
            .map(|group| (*group).to_string())
            .collect::<Vec<_>>()
    );

    // No self-reference: the digest covers the projection, not the artifact
    // bytes that embed it.
    let artifact_digest = sha256_prefixed(&receipt.artifact_json_bytes().expect("bytes"));
    assert_ne!(declared, artifact_digest);
}

/// The observation surface is a declared run snapshot, not an acceptance
/// receipt, and it is never pinned: not in the DATA receipt list, not in the
/// route registry's `source_bindings`. Only the deterministic bytes are pinned.
#[test]
fn observations_surface_is_declared_a_run_snapshot_not_a_receipt() {
    let receipt = fixture_receipt(48);
    let deterministic_bytes = receipt.artifact_json_bytes().expect("deterministic bytes");
    let deterministic_sha256 = sha256_prefixed(&deterministic_bytes);
    let observation_json: serde_json::Value = serde_json::from_slice(
        &receipt
            .observations_json_bytes(&deterministic_sha256)
            .expect("observation bytes"),
    )
    .expect("the observation surface is JSON");

    assert_eq!(
        observation_json["schema_version"],
        NATIVE_CORE_SMP_OBSERVATIONS_SCHEMA_VERSION
    );
    assert_eq!(observation_json["reproducible"], false);
    assert_eq!(observation_json["not_a_receipt"], true);
    assert_eq!(
        observation_json["source_artifact"]["path"],
        NATIVE_CORE_SMP_ARTIFACT_REL
    );
    assert_eq!(
        observation_json["source_artifact"]["sha256"],
        deterministic_sha256
    );
    assert_eq!(
        deterministic_sha256,
        format!("sha256:{:x}", Sha256::digest(&deterministic_bytes)),
        "the binding must digest the bytes that were actually emitted"
    );
    assert!(
        !observation_json["non_reproducible_reason"]
            .as_str()
            .expect("a non-reproducible claim needs a typed reason")
            .is_empty()
    );

    // Not a DATA receipt…
    assert!(
        !COMPOSITION_DATA.contains(NATIVE_CORE_SMP_OBSERVATIONS_ARTIFACT_REL),
        "the observation snapshot must not be declared as a DATA receipt"
    );
    // …and not a pinned input of the emitted route registry.
    let registry: serde_json::Value =
        serde_json::from_str(ROUTE_REGISTRY).expect("the route registry is JSON");
    let bindings = &registry["source_bindings"];
    let bindings_json = serde_json::to_string(bindings).expect("source_bindings serialize");
    assert!(
        !bindings_json.contains(NATIVE_CORE_SMP_OBSERVATIONS_ARTIFACT_REL),
        "the observation surface must never appear in the route registry's source_bindings"
    );
    // Positive polarity, so the exclusion above is a boundary and not an
    // accident: the deterministic surface is the object that *is* pinned.
    assert!(
        bindings
            .as_array()
            .expect("source_bindings is a list")
            .iter()
            .any(|binding| binding["path"] == NATIVE_CORE_SMP_ARTIFACT_REL),
        "the deterministic surface must be pinned by the route registry"
    );
}

/// Keep the unused-import guard honest: the fixture failure vocabulary is
/// asserted through the typed enum in the panic test.
#[allow(dead_code)]
fn coordinator_failure_kinds(failure: &CoordinatorFailure) -> &'static str {
    match failure {
        CoordinatorFailure::WorkerPanic { .. } => "worker_panic",
        CoordinatorFailure::ChannelClosed { .. } => "channel_closed",
        CoordinatorFailure::Interrupted => "interrupted",
    }
}

/// `Terminal` is part of the layout_runtime contract this module reuses; the
/// gate pins that the reused type still carries the ordinals it relies on.
#[allow(dead_code)]
fn terminal_ordinal(terminal: &Terminal<u8, &'static str>) -> usize {
    terminal.ordinal()
}
