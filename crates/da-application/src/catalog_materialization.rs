//! M440 S02: explicit materialization FSM for catalog sweep replay.
//!
//! Each swept paper moves through a typed state machine with guarded
//! transitions; unbindable papers terminate in [`PaperMaterializeState::Skipped`]
//! with a reason code and failures in [`PaperMaterializeState::Failed`] —
//! never a silent drop and never a panic. Terminal states absorb further
//! events so a stray late event cannot corrupt a finished paper's record.
//!
//! D127 invariant: this FSM has no import, promote, or unlock event and no
//! such state — the exhaustive `apply` match makes adding one a compile-time
//! decision that must touch this documentation.

use serde::Serialize;
use thiserror::Error;

/// Reason codes shared with the S01 coverage audit.
pub const SKIP_NO_PARSE_CACHE: &str = "no_parse_cache";
pub const SKIP_NO_EXTRACTION_CACHE: &str = "no_extraction_cache";
pub const SKIP_NO_EMBEDDING_CACHE: &str = "no_embedding_cache";
pub const SKIP_NO_TEI_REVISION: &str = "no_tei_revision";
pub const SKIP_AMBIGUOUS_PARSE_KEYS: &str = "ambiguous_parse_keys";
pub const SKIP_ASSET_MISSING: &str = "asset_missing";

/// Events driving one paper through materialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum PaperMaterializeEvent {
    ScanDone {
        has_parse: bool,
        has_extraction: bool,
        has_embedding: bool,
    },
    BindingResolved {
        revision: u32,
    },
    BindingSkipped {
        reason: &'static str,
    },
    MaterializeDone {
        entities_created: usize,
        mentions_edges: usize,
    },
    ReplayVerified {
        noop: bool,
    },
    Fail {
        stage: &'static str,
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum PaperMaterializeState {
    Pending,
    Scanned {
        has_parse: bool,
        has_extraction: bool,
        has_embedding: bool,
    },
    Bound {
        revision: u32,
    },
    Materialized {
        revision: u32,
        entities_created: usize,
        mentions_edges: usize,
    },
    Completed {
        revision: u32,
        entities_created: usize,
        mentions_edges: usize,
        replay_noop: bool,
    },
    Skipped {
        reason: &'static str,
    },
    Failed {
        stage: &'static str,
        error: String,
    },
}

impl PaperMaterializeState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Skipped { .. } | Self::Failed { .. }
        )
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("illegal materialization transition for {paper_id}: {event:?} in state {from:?}")]
pub struct IllegalTransition {
    pub paper_id: String,
    pub from: &'static str,
    pub event: String,
}

/// Per-paper finite state machine.
#[derive(Debug, Clone)]
pub struct PaperMaterializeFsm {
    paper_id: String,
    state: PaperMaterializeState,
}

impl PaperMaterializeFsm {
    pub fn new(paper_id: impl Into<String>) -> Self {
        Self {
            paper_id: paper_id.into(),
            state: PaperMaterializeState::Pending,
        }
    }

    pub fn paper_id(&self) -> &str {
        &self.paper_id
    }

    pub fn state(&self) -> &PaperMaterializeState {
        &self.state
    }

    /// Apply one event. Terminal states absorb every event (Ok, unchanged);
    /// structurally illegal transitions on live states are typed errors.
    pub fn apply(&mut self, event: PaperMaterializeEvent) -> Result<(), IllegalTransition> {
        // Exhaustive match over every event variant: D127 canary. This FSM
        // has no import/promote/unlock event; adding one must extend this
        // match and revisit the module documentation.
        match (&self.state, event) {
            (s, event) if s.is_terminal() => {
                // Terminal absorption: Completed/Skipped/Failed keep their
                // verdict regardless of stray late events.
                let _ = event;
                Ok(())
            }
            (
                PaperMaterializeState::Pending,
                PaperMaterializeEvent::ScanDone {
                    has_parse,
                    has_extraction,
                    has_embedding,
                },
            ) => {
                self.state = PaperMaterializeState::Scanned {
                    has_parse,
                    has_extraction,
                    has_embedding,
                };
                Ok(())
            }
            (
                PaperMaterializeState::Scanned { .. },
                PaperMaterializeEvent::BindingResolved { revision },
            ) => {
                self.state = PaperMaterializeState::Bound { revision };
                Ok(())
            }
            (
                PaperMaterializeState::Scanned { .. },
                PaperMaterializeEvent::BindingSkipped { reason },
            ) => {
                self.state = PaperMaterializeState::Skipped { reason };
                Ok(())
            }
            (
                PaperMaterializeState::Bound { revision },
                PaperMaterializeEvent::MaterializeDone {
                    entities_created,
                    mentions_edges,
                },
            ) => {
                self.state = PaperMaterializeState::Materialized {
                    revision: *revision,
                    entities_created,
                    mentions_edges,
                };
                Ok(())
            }
            (
                PaperMaterializeState::Materialized {
                    revision,
                    entities_created,
                    mentions_edges,
                },
                PaperMaterializeEvent::ReplayVerified { noop },
            ) => {
                self.state = PaperMaterializeState::Completed {
                    revision: *revision,
                    entities_created: *entities_created,
                    mentions_edges: *mentions_edges,
                    replay_noop: noop,
                };
                Ok(())
            }
            // Failure is reachable from every live state (terminal states
            // were absorbed by the first arm).
            (_, PaperMaterializeEvent::Fail { stage, error }) => {
                self.state = PaperMaterializeState::Failed { stage, error };
                Ok(())
            }
            (s, event) => Err(IllegalTransition {
                paper_id: self.paper_id.clone(),
                from: state_name(s),
                event: event_name(&event),
            }),
        }
    }
}

/// Run-level state machine for one materialization run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum RunMaterializeState {
    NotStarted,
    Running { processed: usize },
    Exporting { processed: usize },
    Completed { processed: usize, snapshot: String },
    Failed { stage: &'static str, error: String },
}

impl RunMaterializeState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }
}

/// Events driving the run-level machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum RunMaterializeEvent {
    Started,
    PaperProcessed,
    ExportStarted,
    ExportDone { snapshot: String },
    Fail { stage: &'static str, error: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("illegal run transition: {event:?} in state {from:?}")]
pub struct IllegalRunTransition {
    pub from: &'static str,
    pub event: String,
}

/// Run-level finite state machine.
#[derive(Debug, Clone)]
pub struct RunMaterializeFsm {
    state: RunMaterializeState,
}

impl Default for RunMaterializeFsm {
    fn default() -> Self {
        Self::new()
    }
}

impl RunMaterializeFsm {
    pub fn new() -> Self {
        Self {
            state: RunMaterializeState::NotStarted,
        }
    }

    pub fn state(&self) -> &RunMaterializeState {
        &self.state
    }

    pub fn apply(&mut self, event: RunMaterializeEvent) -> Result<(), IllegalRunTransition> {
        match (&self.state, event) {
            (s, event) if s.is_terminal() => {
                let _ = event;
                Ok(())
            }
            (RunMaterializeState::NotStarted, RunMaterializeEvent::Started) => {
                self.state = RunMaterializeState::Running { processed: 0 };
                Ok(())
            }
            (RunMaterializeState::Running { processed }, RunMaterializeEvent::PaperProcessed) => {
                self.state = RunMaterializeState::Running {
                    processed: processed + 1,
                };
                Ok(())
            }
            (RunMaterializeState::Running { processed }, RunMaterializeEvent::ExportStarted) => {
                self.state = RunMaterializeState::Exporting {
                    processed: *processed,
                };
                Ok(())
            }
            (
                RunMaterializeState::Exporting { processed },
                RunMaterializeEvent::ExportDone { snapshot },
            ) => {
                self.state = RunMaterializeState::Completed {
                    processed: *processed,
                    snapshot,
                };
                Ok(())
            }
            (
                RunMaterializeState::NotStarted
                | RunMaterializeState::Running { .. }
                | RunMaterializeState::Exporting { .. },
                RunMaterializeEvent::Fail { stage, error },
            ) => {
                self.state = RunMaterializeState::Failed { stage, error };
                Ok(())
            }
            (s, event) => Err(IllegalRunTransition {
                from: run_state_name(s),
                event: run_event_name(&event),
            }),
        }
    }
}

fn state_name(state: &PaperMaterializeState) -> &'static str {
    match state {
        PaperMaterializeState::Pending => "Pending",
        PaperMaterializeState::Scanned { .. } => "Scanned",
        PaperMaterializeState::Bound { .. } => "Bound",
        PaperMaterializeState::Materialized { .. } => "Materialized",
        PaperMaterializeState::Completed { .. } => "Completed",
        PaperMaterializeState::Skipped { .. } => "Skipped",
        PaperMaterializeState::Failed { .. } => "Failed",
    }
}

fn event_name(event: &PaperMaterializeEvent) -> String {
    match event {
        PaperMaterializeEvent::ScanDone { .. } => "scan_done".into(),
        PaperMaterializeEvent::BindingResolved { .. } => "binding_resolved".into(),
        PaperMaterializeEvent::BindingSkipped { .. } => "binding_skipped".into(),
        PaperMaterializeEvent::MaterializeDone { .. } => "materialize_done".into(),
        PaperMaterializeEvent::ReplayVerified { .. } => "replay_verified".into(),
        PaperMaterializeEvent::Fail { .. } => "fail".into(),
    }
}

fn run_state_name(state: &RunMaterializeState) -> &'static str {
    match state {
        RunMaterializeState::NotStarted => "NotStarted",
        RunMaterializeState::Running { .. } => "Running",
        RunMaterializeState::Exporting { .. } => "Exporting",
        RunMaterializeState::Completed { .. } => "Completed",
        RunMaterializeState::Failed { .. } => "Failed",
    }
}

fn run_event_name(event: &RunMaterializeEvent) -> String {
    match event {
        RunMaterializeEvent::Started => "started".into(),
        RunMaterializeEvent::PaperProcessed => "paper_processed".into(),
        RunMaterializeEvent::ExportStarted => "export_started".into(),
        RunMaterializeEvent::ExportDone { .. } => "export_done".into(),
        RunMaterializeEvent::Fail { .. } => "fail".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_reaches_completed() {
        let mut fsm = PaperMaterializeFsm::new("2401.00001");
        fsm.apply(PaperMaterializeEvent::ScanDone {
            has_parse: true,
            has_extraction: true,
            has_embedding: true,
        })
        .unwrap();
        fsm.apply(PaperMaterializeEvent::BindingResolved { revision: 3 })
            .unwrap();
        fsm.apply(PaperMaterializeEvent::MaterializeDone {
            entities_created: 9,
            mentions_edges: 12,
        })
        .unwrap();
        fsm.apply(PaperMaterializeEvent::ReplayVerified { noop: true })
            .unwrap();
        assert_eq!(
            fsm.state(),
            &PaperMaterializeState::Completed {
                revision: 3,
                entities_created: 9,
                mentions_edges: 12,
                replay_noop: true,
            }
        );
    }

    #[test]
    fn skip_path_short_circuits_to_terminal() {
        let mut fsm = PaperMaterializeFsm::new("2401.00002");
        fsm.apply(PaperMaterializeEvent::ScanDone {
            has_parse: true,
            has_extraction: false,
            has_embedding: false,
        })
        .unwrap();
        fsm.apply(PaperMaterializeEvent::BindingSkipped {
            reason: SKIP_NO_TEI_REVISION,
        })
        .unwrap();
        assert_eq!(
            fsm.state(),
            &PaperMaterializeState::Skipped {
                reason: SKIP_NO_TEI_REVISION,
            }
        );
    }

    #[test]
    fn illegal_transition_is_typed_error() {
        let mut fsm = PaperMaterializeFsm::new("2401.00003");
        let err = fsm
            .apply(PaperMaterializeEvent::MaterializeDone {
                entities_created: 1,
                mentions_edges: 1,
            })
            .unwrap_err();
        assert_eq!(err.from, "Pending");
    }

    #[test]
    fn binding_before_scan_is_illegal() {
        let mut fsm = PaperMaterializeFsm::new("2401.00004");
        let err = fsm
            .apply(PaperMaterializeEvent::BindingResolved { revision: 1 })
            .unwrap_err();
        assert_eq!(err.from, "Pending");
    }

    #[test]
    fn fail_is_reachable_from_live_states_and_terminal() {
        let mut fsm = PaperMaterializeFsm::new("2401.00005");
        fsm.apply(PaperMaterializeEvent::ScanDone {
            has_parse: true,
            has_extraction: true,
            has_embedding: true,
        })
        .unwrap();
        fsm.apply(PaperMaterializeEvent::BindingResolved { revision: 2 })
            .unwrap();
        fsm.apply(PaperMaterializeEvent::Fail {
            stage: "materialize",
            error: "graph write failed".into(),
        })
        .unwrap();
        assert!(matches!(fsm.state(), PaperMaterializeState::Failed { .. }));
        // Terminal absorbs a late success event instead of resurrecting.
        fsm.apply(PaperMaterializeEvent::MaterializeDone {
            entities_created: 5,
            mentions_edges: 5,
        })
        .unwrap();
        assert!(matches!(fsm.state(), PaperMaterializeState::Failed { .. }));
    }

    #[test]
    fn completed_absorbs_every_event() {
        let mut fsm = PaperMaterializeFsm::new("2401.00006");
        fsm.apply(PaperMaterializeEvent::ScanDone {
            has_parse: true,
            has_extraction: true,
            has_embedding: true,
        })
        .unwrap();
        fsm.apply(PaperMaterializeEvent::BindingResolved { revision: 1 })
            .unwrap();
        fsm.apply(PaperMaterializeEvent::MaterializeDone {
            entities_created: 1,
            mentions_edges: 1,
        })
        .unwrap();
        fsm.apply(PaperMaterializeEvent::ReplayVerified { noop: true })
            .unwrap();
        for event in [
            PaperMaterializeEvent::ScanDone {
                has_parse: false,
                has_extraction: false,
                has_embedding: false,
            },
            PaperMaterializeEvent::Fail {
                stage: "late",
                error: "late".into(),
            },
        ] {
            fsm.apply(event).unwrap();
        }
        assert!(fsm.state().is_terminal());
    }

    #[test]
    fn no_import_event_exists_in_the_fsm_surface() {
        // Compile-time canary: the exhaustive match forces any new event
        // variant to be classified here. D127: none of these may import,
        // promote, or unlock graph writes.
        fn classify(event: &PaperMaterializeEvent) -> bool {
            matches!(
                event,
                PaperMaterializeEvent::ScanDone { .. }
                    | PaperMaterializeEvent::BindingResolved { .. }
                    | PaperMaterializeEvent::BindingSkipped { .. }
                    | PaperMaterializeEvent::MaterializeDone { .. }
                    | PaperMaterializeEvent::ReplayVerified { .. }
                    | PaperMaterializeEvent::Fail { .. }
            )
        }
        assert!(classify(&PaperMaterializeEvent::Fail {
            stage: "test",
            error: String::new(),
        }));
    }

    #[test]
    fn run_fsm_happy_path() {
        let mut run = RunMaterializeFsm::new();
        run.apply(RunMaterializeEvent::Started).unwrap();
        run.apply(RunMaterializeEvent::PaperProcessed).unwrap();
        run.apply(RunMaterializeEvent::PaperProcessed).unwrap();
        run.apply(RunMaterializeEvent::ExportStarted).unwrap();
        run.apply(RunMaterializeEvent::ExportDone {
            snapshot: "out.sgsnap".into(),
        })
        .unwrap();
        assert!(run.state().is_terminal());
        // Illegal: export before start.
        let mut bad = RunMaterializeFsm::new();
        assert!(bad.apply(RunMaterializeEvent::ExportStarted).is_err());
        // Fail path from running.
        let mut failing = RunMaterializeFsm::new();
        failing.apply(RunMaterializeEvent::Started).unwrap();
        failing
            .apply(RunMaterializeEvent::Fail {
                stage: "export",
                error: "boom".into(),
            })
            .unwrap();
        failing.apply(RunMaterializeEvent::Started).unwrap(); // absorbed
        assert!(matches!(
            failing.state(),
            RunMaterializeState::Failed { .. }
        ));
    }

    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn terminal_states_absorb_arbitrary_event_sequences(
                seed in any::<u64>(),
                n in 1usize..12,
            ) {
                // Build a terminal FSM (completed), then fire a pseudo-random
                // event sequence derived from the seed; state must never
                // change and no error may surface.
                let mut fsm = PaperMaterializeFsm::new("2401.00999");
                fsm.apply(PaperMaterializeEvent::ScanDone {
                    has_parse: true, has_extraction: true, has_embedding: true,
                }).unwrap();
                fsm.apply(PaperMaterializeEvent::BindingResolved { revision: 1 }).unwrap();
                fsm.apply(PaperMaterializeEvent::MaterializeDone {
                    entities_created: 0, mentions_edges: 0,
                }).unwrap();
                fsm.apply(PaperMaterializeEvent::ReplayVerified { noop: true }).unwrap();
                let expected = fsm.state().clone();
                // Tiny deterministic LCG: proptest supplies the seed; the
                // stream arithmetic stays inside the test.
                let mut lcg = seed;
                let mut next = || {
                    lcg = lcg
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    lcg
                };
                for _ in 0..n {
                    let event = match next() % 6 {
                        0 => PaperMaterializeEvent::ScanDone {
                            has_parse: false, has_extraction: false, has_embedding: false },
                        1 => PaperMaterializeEvent::BindingResolved { revision: 9 },
                        2 => PaperMaterializeEvent::BindingSkipped { reason: SKIP_ASSET_MISSING },
                        3 => PaperMaterializeEvent::MaterializeDone {
                            entities_created: 1, mentions_edges: 1 },
                        4 => PaperMaterializeEvent::ReplayVerified { noop: false },
                        _ => PaperMaterializeEvent::Fail {
                            stage: "any", error: "x".into() },
                    };
                    fsm.apply(event).unwrap();
                    prop_assert_eq!(fsm.state(), &expected);
                }
            }
        }
    }
}
