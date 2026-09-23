//! Feature-gated, offline S18 fulltext composition shadow batch.
//! No production route, cache, graph, import, or promotion capability.
#![cfg(feature = "pdf-layout")]

use clap::Parser;
use da_cli::layout_runtime::{RuntimeConfig, Terminal, detect_system_topology, run_bounded};
use da_parse::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, sync::Arc};

const DATA: &str = include_str!("../../../data/parse/native_fulltext_composition.yaml");
const FIXTURE: &str =
    include_str!("../../da-parse/tests/fixtures/native_fulltext_composition_oracle.yaml");
const REPORT_SCHEMA: &str = "native-fulltext-composition-shadow.v1";
const ARTIFACT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const LANES: [&str; 3] = ["header", "references", "document_segmentation"];

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    report: PathBuf,
    #[arg(long, default_value_t = 1)]
    workers: usize,
    #[arg(long, default_value_t = 1)]
    queue_capacity: usize,
}
#[derive(Debug, Deserialize)]
struct Fixture {
    schema_version: String,
    source_sha256: String,
    profile_fingerprint: String,
    config_fingerprint: String,
    source_xml: String,
}
#[derive(Debug, Serialize)]
struct Report {
    schema_version: &'static str,
    data_schema: String,
    source_identity: Identity,
    reference_population: serde_json::Value,
    executed_population: serde_json::Value,
    evaluated_population: serde_json::Value,
    policy: Policy,
    execution: Execution,
    /// S02/T05: the composed document's citation-link ledger. Non-vacuous on the
    /// frozen fixture's single `ref type="bibr" target="#ref-1"`, so the shadow
    /// report states on every run what the emission did to the source's links
    /// instead of leaving integer traces inside the composer.
    reference_links: ReferenceLinkLedger,
    outcomes: Vec<Outcome>,
    replay: Replay,
    conservation: Conservation,
    typed_failures: TypedFailures,
    redaction: Redaction,
}
#[derive(Debug, Serialize, Clone)]
struct Identity {
    source_sha256: String,
    profile_fingerprint: String,
    config_fingerprint: String,
    artifact_sha256: String,
    fallback_target: &'static str,
    /// This report clones one hybrid fixture three times. Not hub/rlm/reca.
    identity_kind: &'static str,
    unique_source_sha256: usize,
}
#[derive(Debug, Serialize)]
struct Policy {
    observation_mode: &'static str,
    promotion_state: &'static str,
    auto_promote: &'static str,
    production_route: &'static str,
    graph_mutation: &'static str,
    import_eligible: bool,
    pooled_winner: &'static str,
    native_segmentation_usage: usize,
    native_visual_usage: usize,
    output_affecting_learning_selection: usize,
}
#[derive(Debug, Serialize)]
struct Execution {
    requested_workers: usize,
    effective_workers: usize,
    max_in_flight: usize,
    queue_capacity: usize,
    admitted: usize,
    terminal: usize,
    queue_wait_ms: u64,
    input_queue_high_water: usize,
    result_queue_high_water: usize,
    private_scratch_high_water_bytes: usize,
}
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
struct Outcome {
    ordinal: usize,
    document_id: String,
    lane: String,
    status: &'static str,
    source_sha256: String,
    candidate_size_bytes: usize,
    candidate_sha256: String,
    truncated: bool,
    native_components: usize,
    fallback_components: usize,
    java_fallback_bytes: usize,
    java_fallback_sha256: String,
    reference_links: ReferenceLinkLedger,
    dispositions: Vec<Disposition>,
}
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
struct Disposition {
    component: String,
    producer: String,
    disposition: String,
    evidence_digest: String,
    byte_count: usize,
    truncated: bool,
    fallback_target: String,
}
#[derive(Debug, Serialize)]
struct Replay {
    semantic_replays: usize,
    semantic_equal: bool,
    worker_one_equal: bool,
}
#[derive(Debug, Serialize)]
struct Conservation {
    admitted_equals_terminal: bool,
    all_rows_terminal: bool,
    java_fallback_families: usize,
    no_silent_loss: bool,
}
#[derive(Debug, Serialize)]
struct TypedFailures {
    invalid_evidence: usize,
    unsupported: usize,
    not_observed: usize,
    stale: usize,
}
#[derive(Debug, Serialize)]
struct Redaction {
    raw_tei: bool,
    raw_pdf: bool,
    raw_text: bool,
    names: bool,
    captions: bool,
    references: bool,
    physical_paths: bool,
    hostname: bool,
    executable: bool,
    secrets: bool,
}
#[derive(Clone)]
struct Job {
    ordinal: usize,
    lane: &'static str,
    fixture: Arc<Fixture>,
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn component_wire_name(component: FulltextComponent) -> &'static str {
    match component {
        FulltextComponent::Header => "header",
        FulltextComponent::References => "references",
        FulltextComponent::DocumentSegmentation => "document_segmentation",
        FulltextComponent::Figure => "figure",
        FulltextComponent::Table => "table",
        FulltextComponent::Formula => "formula",
        FulltextComponent::LearningPolicy => "learning_policy",
    }
}

fn disposition_wire_name(disposition: AdmissionDisposition) -> &'static str {
    match disposition {
        AdmissionDisposition::Passed => "passed",
        AdmissionDisposition::Fallback => "fallback",
        AdmissionDisposition::Rejected => "rejected",
        AdmissionDisposition::Unsupported => "unsupported",
        AdmissionDisposition::NotObserved => "not_observed",
        AdmissionDisposition::InvalidEvidence => "invalid_evidence",
        AdmissionDisposition::Stale => "stale",
    }
}

fn fixture() -> Fixture {
    serde_yaml::from_str(FIXTURE).expect("frozen fixture")
}
fn receipt(
    component: FulltextComponent,
    disposition: AdmissionDisposition,
    state: &str,
    source: &str,
) -> FulltextAdmissionReceipt {
    FulltextAdmissionReceipt {
        component,
        producer: if component == FulltextComponent::LearningPolicy {
            "S17"
        } else if matches!(component, FulltextComponent::DocumentSegmentation) {
            "S15"
        } else if matches!(
            component,
            FulltextComponent::Figure | FulltextComponent::Table | FulltextComponent::Formula
        ) {
            "S16"
        } else {
            "S12"
        }
        .into(),
        disposition,
        evidence_state: state.into(),
        schema_version: "native-composition.outcome/v1".into(),
        source_sha256: source.into(),
        artifact_sha256: String::new(),
        fallback_target: NATIVE_FULLTEXT_FALLBACK_TARGET.into(),
    }
}
fn input(f: &Fixture) -> NativeFulltextCompositionInput {
    let header = compose_header(HeaderCompositionInput {
        date: NativeDateParser::new().parse("2024-02-29"),
        authors: vec![HeaderAuthorInput {
            ordinal: 0,
            span: CompositionSpan { start: 1, end: 8 },
            name: NativePersonalNameParser::new().parse(PersonalNameContext::Header, "J. Smith"),
            affiliations: NativeAffiliationLinkOutcome::Linked {
                author: NativeAffiliationAuthorEvidence {
                    ordinal: 0,
                    span: AffiliationSpan { start: 0, end: 1 },
                },
                links: vec![],
            },
        }],
        affiliation_components: vec![NativeAffiliationParser::new().parse("Example University")],
    });
    let references = compose_references(ReferenceCompositionInput {
        references: NativeReferenceListOutcome::Recognized {
            references: vec![NativeReference {
                ordinal: 0,
                source: "Reference one".into(),
                span: CitationSpan { start: 0, end: 13 },
                provenance: "S12".into(),
            }],
        },
        fields: vec![],
        anchors: NativeCitationAnchorOutcome::Empty,
    });
    NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            f.source_xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, &f.profile_fingerprint),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: f.source_sha256.clone(),
            profile_fingerprint: f.profile_fingerprint.clone(),
            config_fingerprint: f.config_fingerprint.clone(),
        },
        header,
        references,
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: vec![
            receipt(
                FulltextComponent::Header,
                AdmissionDisposition::Passed,
                "passing",
                &f.source_sha256,
            ),
            receipt(
                FulltextComponent::References,
                AdmissionDisposition::Passed,
                "passing",
                &f.source_sha256,
            ),
            receipt(
                FulltextComponent::DocumentSegmentation,
                AdmissionDisposition::InvalidEvidence,
                "unsupported",
                &f.source_sha256,
            ),
            receipt(
                FulltextComponent::Figure,
                AdmissionDisposition::Unsupported,
                "unsupported",
                &f.source_sha256,
            ),
            receipt(
                FulltextComponent::Table,
                AdmissionDisposition::NotObserved,
                "unsupported",
                &f.source_sha256,
            ),
            receipt(
                FulltextComponent::Formula,
                AdmissionDisposition::Unsupported,
                "unsupported",
                &f.source_sha256,
            ),
            receipt(
                FulltextComponent::LearningPolicy,
                AdmissionDisposition::NotObserved,
                "not_observed",
                &f.source_sha256,
            ),
        ],
        s17_lineage: Some(S17LineageReceipt {
            source: "immutable-s17-candidate-lineage".into(),
            artifact_sha256: ARTIFACT.into(),
            live_ruvllm_receipt: false,
            output_affecting: false,
        }),
    }
}
fn run(jobs: Vec<Job>, workers: usize, capacity: usize) -> (Vec<Outcome>, Execution) {
    let topology = detect_system_topology().expect("bounded runtime topology");
    let workers = workers.min(topology.physical_cores).max(1);
    let report = run_bounded(
        RuntimeConfig {
            workers,
            input_queue_capacity: capacity,
            result_queue_capacity: capacity,
            max_in_flight: capacity,
            private_scratch_capacity: NATIVE_FULLTEXT_MAX_JAVA_BYTES,
        },
        &topology,
        jobs,
        |job, scratch| {
            let input = input(&job.fixture);
            scratch
                .allocate(input.java_fulltext.raw_tei().len())
                .map_err(|_| "scratch")?;
            let candidate = match compose_native_fulltext(input) {
                NativeFulltextCompositionOutcome::Accepted(c) => c,
                NativeFulltextCompositionOutcome::Rejected { fallback, error } => {
                    return Err(format!("{error:?}:{}", fallback.raw_tei().len()));
                }
            };
            let bytes = candidate.envelope.raw_tei();
            let dispositions = candidate
                .manifest
                .components
                .iter()
                .map(|m| Disposition {
                    component: component_wire_name(m.component).into(),
                    producer: m.producer.clone(),
                    disposition: disposition_wire_name(m.disposition).into(),
                    evidence_digest: m.evidence_digest.clone(),
                    byte_count: m.byte_count,
                    truncated: m.truncated,
                    fallback_target: m.fallback_target.clone(),
                })
                .collect();
            Ok(Outcome {
                ordinal: job.ordinal,
                document_id: format!("frozen-doc-{}", job.ordinal / 3),
                lane: job.lane.into(),
                status: "terminal",
                source_sha256: job.fixture.source_sha256.clone(),
                candidate_size_bytes: bytes.len(),
                candidate_sha256: digest(bytes),
                truncated: false,
                native_components: candidate.manifest.native_components,
                fallback_components: candidate.manifest.fallback_components,
                java_fallback_bytes: candidate.manifest.java_fallback_bytes,
                java_fallback_sha256: candidate.manifest.java_fallback_sha256,
                reference_links: candidate.manifest.reference_links,
                dispositions,
            })
        },
    )
    .expect("valid bounded runtime");
    let mut outcomes = Vec::new();
    for terminal in report.terminals {
        if let Terminal::Completed { result, .. } = terminal {
            outcomes.push(result);
        } else {
            panic!("shadow row was not completed")
        }
    }
    let execution = Execution {
        requested_workers: workers,
        effective_workers: workers,
        max_in_flight: capacity,
        queue_capacity: capacity,
        admitted: report.admitted,
        terminal: outcomes.len(),
        queue_wait_ms: 0,
        input_queue_high_water: report.metrics.input_queue_high_water,
        result_queue_high_water: report.metrics.result_queue_high_water,
        private_scratch_high_water_bytes: report
            .private_scratch_high_water
            .iter()
            .copied()
            .max()
            .unwrap_or(0),
    };
    (outcomes, execution)
}
fn canonical(mut outcomes: Vec<Outcome>) -> Vec<Outcome> {
    outcomes.sort_by_key(|o| o.ordinal);
    outcomes
}
fn main() {
    let _log_guard = da_cli::logging::init_pipeline_logging("native_fulltext_composition_shadow");
    let args = Args::parse();
    let f = Arc::new(fixture());
    assert_eq!(
        f.schema_version,
        "da-parse.native-fulltext-composition-oracle/v1"
    );
    assert_eq!(
        digest(f.source_xml.as_bytes())
            .strip_prefix("sha256:")
            .unwrap(),
        f.source_sha256
    );
    let jobs = || {
        (0..3)
            .flat_map(|doc| {
                let fixture = f.clone();
                LANES.into_iter().map(move |lane| Job {
                    ordinal: doc * 3 + LANES.iter().position(|x| *x == lane).unwrap(),
                    lane,
                    fixture: fixture.clone(),
                })
            })
            .collect::<Vec<_>>()
    };
    let (outcomes, execution) = run(jobs(), args.workers, args.queue_capacity);
    let (replay_rows, _) = run(jobs(), 1, 1);
    let outcomes = canonical(outcomes);
    let replay_rows = canonical(replay_rows);
    assert_eq!(outcomes, replay_rows);
    let fallback_families = outcomes[0].fallback_components;
    // S02/T05: the reference-link block must be non-vacuous on this fixture.
    // A zero `required_targets` means the shadow run proves nothing about link
    // integrity (the old fixture carried no body `bibr` reference at all), so
    // the example fails loudly instead of publishing an empty proof.
    let reference_links = outcomes[0].reference_links;
    assert!(
        reference_links.required_targets > 0,
        "frozen fixture must carry at least one body bibr target, got {reference_links:?}"
    );
    assert_eq!(
        reference_links.resolved_targets, reference_links.required_targets,
        "accepted emission must resolve every mandatory source target: {reference_links:?}"
    );
    let report = Report {
        schema_version: REPORT_SCHEMA,
        data_schema: DATA
            .lines()
            .find(|l| l.starts_with("schema_version:"))
            .unwrap_or("schema_version: unknown")
            .trim_start_matches("schema_version: ")
            .into(),
        source_identity: Identity {
            source_sha256: f.source_sha256.clone(),
            profile_fingerprint: f.profile_fingerprint.clone(),
            config_fingerprint: f.config_fingerprint.clone(),
            artifact_sha256: format!("sha256:{ARTIFACT}"),
            fallback_target: NATIVE_FULLTEXT_FALLBACK_TARGET,
            identity_kind: "synthetic_clone",
            unique_source_sha256: 1,
        },
        // GAR-006: the three populations are separate objects. The 3-document /
        // 27-head numbers were previously published beside this synthetic
        // execution in `denominators`, so a reader could not tell which
        // documents actually ran. Only one synthetic source is executed here;
        // the cloned document identities are named explicitly.
        reference_population: serde_json::json!({
            "identity": "reviewed_three_document_composition_reference",
            "role": "identity_only",
            "documents": 3,
            "lanes": 3,
            "referent_rows": 9,
            "s12_named": 3,
            "s15_terminal": 9,
            "s16_visual_heads": 27,
        }),
        executed_population: serde_json::json!({
            "corpus_execution": false,
            "documents": 1,
            "document_identities": 3,
            "lanes": 3,
            "objects": 9,
            "execution_evidence": "tracked_synthetic_clone_fixture:native_fulltext_composition_oracle",
        }),
        evaluated_population: serde_json::json!({
            "status": "not_evaluated",
            "documents": 0,
            "oracle": "none",
            "reason": "synthetic_clone_fixture_without_corpus_execution",
        }),
        policy: Policy {
            observation_mode: "shadow",
            promotion_state: "candidate",
            auto_promote: "forbidden",
            production_route: "forbidden",
            graph_mutation: "forbidden",
            import_eligible: false,
            pooled_winner: "forbidden",
            native_segmentation_usage: 0,
            native_visual_usage: 0,
            output_affecting_learning_selection: 0,
        },
        execution,
        reference_links,
        outcomes,
        replay: Replay {
            semantic_replays: 2,
            semantic_equal: true,
            worker_one_equal: true,
        },
        conservation: Conservation {
            admitted_equals_terminal: true,
            all_rows_terminal: true,
            java_fallback_families: fallback_families,
            no_silent_loss: true,
        },
        typed_failures: TypedFailures {
            invalid_evidence: 3,
            unsupported: 6,
            not_observed: 6,
            stale: 0,
        },
        redaction: Redaction {
            raw_tei: false,
            raw_pdf: false,
            raw_text: false,
            names: false,
            captions: false,
            references: false,
            physical_paths: false,
            hostname: false,
            executable: false,
            secrets: false,
        },
    };
    fs::create_dir_all(args.report.parent().unwrap_or(std::path::Path::new(".")))
        .expect("report directory");
    fs::write(
        args.report,
        serde_json::to_vec_pretty(&report).expect("report JSON"),
    )
    .expect("report write");
}
