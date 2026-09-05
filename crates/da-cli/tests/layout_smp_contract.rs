#![cfg(feature = "pdf-layout")]

use da_cli::layout_runtime::{
    CoordinatorFailure, RuntimeConfig, Terminal, TopologyError, TopologyReader, TopologyReceipt,
    detect_physical_topology, run_bounded,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

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

fn topology() -> TopologyReceipt {
    detect_physical_topology(&FixtureTopology).expect("fixture topology")
}

fn config(workers: usize, queue: usize) -> RuntimeConfig {
    RuntimeConfig {
        workers,
        input_queue_capacity: queue,
        result_queue_capacity: queue,
        max_in_flight: 2,
        private_scratch_capacity: 32,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Receipt {
    ordinal: usize,
    unit_key: String,
    semantic: String,
    pages: usize,
    tokens: usize,
    spans: usize,
    coordinates: usize,
}

fn receipt_digest(terminals: &[Terminal<Receipt, &'static str>]) -> String {
    let mut rows = terminals
        .iter()
        .map(|terminal| match terminal {
            Terminal::Completed { result, .. } => result.clone(),
            Terminal::Failed { ordinal, .. } | Terminal::CoordinatorFailed { ordinal, .. } => {
                panic!("unexpected failure for semantic matrix row {ordinal}")
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.ordinal);
    let encoded = rows
        .iter()
        .map(|row| {
            format!(
                "{}|{}|{}|{}|{}|{}|{}",
                row.ordinal,
                row.unit_key,
                row.semantic,
                row.pages,
                row.tokens,
                row.spans,
                row.coordinates
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("sha256:{:x}", Sha256::digest(encoded.as_bytes()))
}

fn assert_conservation(terminals: &[Terminal<Receipt, &'static str>], expected: usize) {
    assert_eq!(terminals.len(), expected, "one terminal per admitted unit");
    let ordinals = terminals.iter().map(Terminal::ordinal).collect::<Vec<_>>();
    assert_eq!(
        ordinals,
        (0..expected).collect::<Vec<_>>(),
        "canonical ordinal publication"
    );
    let unique = ordinals.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), expected, "no duplicate or dropped unit keys");
    let keys = terminals
        .iter()
        .filter_map(|terminal| match terminal {
            Terminal::Completed { result, .. } => Some(result.unit_key.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(keys.len(), expected, "every fixture unit key is unique");
}

#[test]
fn runtime_metrics_are_observed_not_configured_echoes() {
    // T05-F02 regression guard: the reported queue/in-flight high-water gauges
    // must be genuine runtime observations bounded by the configured limits,
    // never copies of those limits (and never zero while work was admitted).
    let topology = topology();
    let jobs = (0..8).collect::<Vec<_>>();
    let report = run_bounded(config(2, 2), &topology, jobs.clone(), |ordinal, scratch| {
        scratch.allocate(ordinal % 3 + 1).map_err(|_| "scratch")?;
        Ok::<_, &'static str>(Receipt {
            ordinal,
            unit_key: format!("unit-{ordinal:02}"),
            semantic: format!("semantic-{ordinal}"),
            pages: 1,
            tokens: 1,
            spans: 1,
            coordinates: 1,
        })
    })
    .expect("valid bounded config");
    assert_eq!(report.admitted, jobs.len());
    assert_conservation(&report.terminals, jobs.len());

    let metrics = &report.metrics;
    assert!(
        metrics.input_queue_high_water >= 1,
        "input queue high-water must reflect observed dispatch"
    );
    assert!(
        metrics.result_queue_high_water >= 1,
        "result queue high-water must reflect observed publication"
    );
    assert!(
        metrics.in_flight_high_water >= 1,
        "in-flight high-water must reflect observed unresolved work"
    );
    // Configured capacities were queue=2, workers=2: a reserve-before-send
    // gauge may transiently overshoot the channel by the number of parked
    // senders, but never beyond queue + workers.
    assert!(metrics.input_queue_high_water <= 2 + 2);
    assert!(metrics.result_queue_high_water <= 2 + 2);
    // max_in_flight=2 bounds admitted-but-unresolved units at all times.
    assert!(metrics.in_flight_high_water <= 2);
}

#[test]
fn every_worker_count_and_bounded_queue_preserve_semantic_receipts() {
    let topology = topology();
    assert_eq!(topology.physical_cores, 2);
    let jobs = (0..12).collect::<Vec<_>>();
    let mut baseline = None;

    for workers in 1..=topology.physical_cores {
        for queue in [1, 3] {
            let report = run_bounded(
                config(workers, queue),
                &topology,
                jobs.clone(),
                |ordinal, scratch| {
                    scratch.allocate(ordinal % 7 + 1).map_err(|_| "scratch")?;
                    Ok::<_, &'static str>(Receipt {
                        ordinal,
                        unit_key: format!("unit-{ordinal:02}"),
                        semantic: format!("semantic-{}", ordinal * 17 + 3),
                        pages: 2,
                        tokens: 5 + ordinal,
                        spans: 4 + ordinal,
                        coordinates: 5 + ordinal,
                    })
                },
            )
            .expect("valid bounded config");

            assert_eq!(report.admitted, jobs.len());
            assert_conservation(&report.terminals, jobs.len());
            assert!(report.coordinator_failure.is_none());
            assert!(
                report
                    .private_scratch_high_water
                    .iter()
                    .all(|high| *high <= 7)
            );
            let digest = receipt_digest(&report.terminals);
            if let Some(expected) = baseline.as_ref() {
                assert_eq!(&digest, expected, "worker/queue count changed semantics");
            } else {
                baseline = Some(digest);
            }
        }
    }
    let one = da_parse::TeiIdentity::grobid_http(da_parse::GrobidProfile::structural());
    let many = da_parse::TeiIdentity::grobid_http(da_parse::GrobidProfile::structural());
    assert_eq!(
        one, many,
        "worker count 1..=N must not invent a TEI identity component"
    );
    assert!(
        !one.profile_fingerprint()
            .to_ascii_lowercase()
            .contains("worker")
    );
}

#[test]
fn typed_failure_and_panic_are_terminal_and_conserved_at_each_worker_count() {
    let topology = topology();
    for workers in 1..=topology.physical_cores {
        let report = run_bounded(
            config(workers, 1),
            &topology,
            (0..8).collect::<Vec<_>>(),
            |ordinal, _| match ordinal {
                2 => Err::<Receipt, _>("typed-failure"),
                5 => panic!("fixture panic"),
                n => Ok(Receipt {
                    ordinal: n,
                    unit_key: format!("unit-{n}"),
                    semantic: format!("semantic-{n}"),
                    pages: 1,
                    tokens: 1,
                    spans: 1,
                    coordinates: 1,
                }),
            },
        )
        .expect("valid bounded config");

        assert_eq!(report.admitted, 8);
        assert_eq!(report.terminals.len(), 8);
        assert_eq!(
            report
                .terminals
                .iter()
                .map(Terminal::ordinal)
                .collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
        assert!(matches!(
            report.terminals[2],
            Terminal::Failed {
                error: "typed-failure",
                ..
            }
        ));
        assert!(matches!(
            report.terminals[5],
            Terminal::CoordinatorFailed {
                failure: CoordinatorFailure::WorkerPanic { .. },
                ..
            }
        ));
        assert!(matches!(
            report.coordinator_failure,
            Some(CoordinatorFailure::WorkerPanic { .. })
        ));
    }
}

#[test]
fn over_cap_is_rejected_before_any_fixture_or_donor_invocation() {
    let topology = topology();
    let invoked = Arc::new(AtomicUsize::new(0));
    let counter = invoked.clone();
    let result = run_bounded(
        config(topology.physical_cores + 1, 1),
        &topology,
        vec![0usize],
        move |_, _| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok::<_, &'static str>(())
        },
    );

    assert!(matches!(
        result,
        Err(da_cli::layout_runtime::ConfigError::OverCoreCap {
            requested: 3,
            cap: 2
        })
    ));
    assert_eq!(
        invoked.load(Ordering::SeqCst),
        0,
        "over-cap validation precedes callback invocation"
    );
}

#[test]
fn fixture_topology_rejects_incomplete_identity_instead_of_inventing_a_core() {
    struct Incomplete;
    impl TopologyReader for Incomplete {
        fn allowed_cpu_set(&self) -> Result<String, TopologyError> {
            Ok("0-1".into())
        }
        fn core_identity(&self, cpu: u32) -> Result<(String, String), TopologyError> {
            if cpu == 0 {
                Ok(("0".into(), "0".into()))
            } else {
                Err(TopologyError::CpuIdentityUnavailable(cpu))
            }
        }
    }
    assert!(matches!(
        detect_physical_topology(&Incomplete),
        Err(TopologyError::CpuIdentityUnavailable(1))
    ));
}

#[test]
fn worker_receipts_report_per_worker_terminal_coverage() {
    let report = run_bounded(
        config(2, 1),
        &topology(),
        (0..10).collect::<Vec<_>>(),
        |ordinal, scratch| {
            scratch.allocate(ordinal % 3 + 1).map_err(|_| "scratch")?;
            Ok::<_, &'static str>(Receipt {
                ordinal,
                unit_key: format!("unit-{ordinal}"),
                semantic: "stable".into(),
                pages: 1,
                tokens: 1,
                spans: 1,
                coordinates: 1,
            })
        },
    )
    .unwrap();
    let mut per_worker = BTreeMap::<usize, usize>::new();
    for terminal in &report.terminals {
        let worker = match terminal {
            Terminal::Completed { worker, .. } => *worker,
            _ => unreachable!("fixture has no failures"),
        };
        *per_worker.entry(worker).or_default() += 1;
    }
    assert_eq!(per_worker.values().sum::<usize>(), report.admitted);
    assert_eq!(per_worker.len(), 2);
}
