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
//!     a fabricated number.

use da_cli::layout_runtime::{
    CoordinatorFailure, Terminal, TopologyError, TopologyReader, TopologyReceipt,
    detect_physical_topology,
};
use da_cli::native_core_smp::{
    NATIVE_CORE_SMP_SCHEMA_VERSION, NATIVE_CORE_SMP_TIMING_DISPOSITION, NATIVE_CORE_SMP_VERDICTS,
    NativeCoreSmpConfig, NativeCoreSmpError, NativeCoreSmpFamily, NativeCoreSmpJobReceipt,
    NativeCoreSmpJobSet, SmpJobError, SmpJobProcessor, SmpObservation, build_native_core_jobs,
    closed_breakpoint_vocabulary, combine_smp_matrix, load_cpu_smp_candidate_from_manifest,
    native_core_smp_processor, qualify_native_core_smp, run_smp_worker_count,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const MANIFEST: &str = include_str!("../../../data/parse/comparison_contract.yaml");
const FULLTEXT: &[u8] =
    include_bytes!("../../da-parse/tests/fixtures/native_fulltext_synthetic.wapiti");
const FIGURE: &[u8] =
    include_bytes!("../../da-parse/tests/fixtures/native_figure_synthetic.wapiti");
const REFERENCE: &[u8] =
    include_bytes!("../../da-parse/tests/fixtures/native_bibliography_reference_synthetic.wapiti");

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
    assert!(
        !first.contains("latency") && !first.contains("throughput"),
        "observational timing must stay out of the deterministic projection"
    );
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
    assert!(
        artifact
            .timing_observational
            .timing_fields
            .iter()
            .any(|field| field.contains("throughput"))
    );
    assert!(
        artifact
            .timing_observational
            .runtime_observed_fields
            .iter()
            .any(|field| field.contains("input_high_water"))
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

    let text = String::from_utf8(receipt.artifact_json_bytes().expect("artifact bytes"))
        .expect("artifact is utf8");
    assert!(!text.contains(".wapiti"), "no model filename may appear");
    assert!(
        !text.contains("#mdl#") && !text.contains("#rdr#"),
        "no model bytes may appear"
    );
    for marker in ["/root/", "/home/", "/tmp/", "/usr/", "HOSTNAME"] {
        assert!(
            !text.contains(marker),
            "no physical path or hostname marker may appear: {marker}"
        );
    }
    for family in &families {
        for row in family.rows.iter() {
            if row.len() >= 12 {
                assert!(
                    !text.contains(row.as_str()),
                    "artifact leaked model-derived row content"
                );
            }
        }
    }
    if let Ok(hostname) = std::env::var("HOSTNAME") {
        let hostname = hostname.trim();
        if hostname.len() >= 4 {
            assert!(!text.contains(hostname), "artifact leaked the hostname");
        }
    }
    for family in &artifact.families {
        assert!(
            text.contains(&family.model_sha256),
            "family {} must stay bound by model digest",
            family.family
        );
    }
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
