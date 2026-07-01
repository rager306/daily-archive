# Requirements

This file is the explicit capability and coverage contract for the project.

## Active

### R019 — Hybrid retrieval must return traceable evidence contexts with vector, graph, fusion score metadata, and EvidencePath references.
- Class: core-capability
- Status: active
- Description: Hybrid retrieval must return traceable evidence contexts with vector, graph, fusion score metadata, and EvidencePath references.
- Why it matters: The system must retrieve grounded scientific context, not just similar text, and must expose why each result was returned.
- Source: M003 requirements restoration after S03
- Primary owning slice: M003-km5fty/S06
- Supporting slices: S03,S05,S07,S10
- Validation: Pending S06 retrieval and ablation tests comparing vector-only, graph expansion, and fused retrieval over fixtures.
- Notes: Restores planned M003 hybrid retrieval requirement from the missing historical R026-R035 range using current GSD auto-assigned IDs.

### R022 — RLM document navigation and workflow-in-code prototypes must be read-only, bounded, and return typed draft outputs plus trajectories validated by deterministic code.
- Class: core-capability
- Status: active
- Description: RLM document navigation and workflow-in-code prototypes must be read-only, bounded, and return typed draft outputs plus trajectories validated by deterministic code.
- Why it matters: RLM can be useful for navigation and workflow drafting only if it cannot mutate storage directly and its outputs are inspectable and validated.
- Source: M003 requirements restoration after S03
- Primary owning slice: M003-km5fty/S09
- Supporting slices: S02,S03,S04,S08
- Validation: Pending S09 fixture tests or mocked-interpreter tests for bounded tools, typed draft patch output, trajectory capture, and deterministic validation.
- Notes: Restores planned M003 RLM document/workflow requirement from the missing historical R026-R035 range using current GSD auto-assigned IDs.

### R023 — RLM graph traversal must be benchmarked against vector-only, one-hop graph expansion, and heuristic BFS before adoption recommendations are made.
- Class: differentiator
- Status: active
- Description: RLM graph traversal must be benchmarked against vector-only, one-hop graph expansion, and heuristic BFS before adoption recommendations are made.
- Why it matters: Adaptive RLM traversal should be used only where it beats cheaper deterministic baselines on scattered-evidence scientific questions.
- Source: M003 requirements restoration after S03
- Primary owning slice: M003-km5fty/S10
- Supporting slices: S05,S06,S07,S09
- Validation: Pending S10 comparative benchmark with traversal path, tool usage, cost/latency, candidate set, and evidence recall metrics.
- Notes: Restores planned M003 adaptive graph traversal requirement from the missing historical R026-R035 range using current GSD auto-assigned IDs.

### R024 — Before expanding beyond M003, the system must validate current scientific KG behavior on staged real article batches of 10 documents, 20 documents, and then a one-week corpus, with analysis of graph quality at each stage.
- Class: quality-attribute
- Status: active
- Description: Before expanding beyond M003, the system must validate current scientific KG behavior on staged real article batches of 10 documents, 20 documents, and then a one-week corpus, with analysis of graph quality at each stage.
- Why it matters: Fixture-level passing tests do not prove that the knowledge graph captures useful claims, entities, relations, evidence paths, and retrieval behavior on real articles. Staged validation is needed before further architecture or optimizer work.
- Source: user-directed post-M003 validation plan
- Primary owning slice: M121-kd3kzr/S06
- Supporting slices: M003-km5fty/S01-S10
- Validation: M121 expanded canonical catalog to 221 article records and validated 219 source-backed records fail-closed. Evidence: data/r024-218-document-corpus-v1/R024-COVERAGE.md; ingest-summary.json (166 M056 PDFs SHA256 verified, index_entries=221); parser-chunking/summary.json (total=221, ok=219, skipped=2, errors=0, chunk_count_total=2576); networkx-probe/summary.json (corpus_size=219, skipped_metadata_only=2, n_nodes=3891, n_edges=10102, citation_relations_count=6212); targeted tests test_catalog_expansion_m121.py 8 pass, test_r024_218_document_parser_chunking.py 12 pass, test_r024_218_document_networkx_probe.py 10 pass, test_r024_218_document_coverage_report.py 10 pass. Production graph readiness is still not claimed.
- Notes: M121 advances R024 from 53-record catalog-limited validation to 221 catalog records / 219 source-backed parser+graph-probe records while preserving no-network/no-production-write fail-closed invariants. Two metadata-only records remain excluded until future local source acquisition: arxiv/mixed-source/2605.29548 and stanford/cs224n/gradient-notes. R024 remains active for future production KG readiness work: real semantic extraction, real citation extraction, and side-by-side production graph DB validation before any write activation.

### R027 — Before scientific KG validation or scaling continues, converted paper data and chunks must satisfy an explicit graph-readiness quality contract covering conversion fidelity, normalization, chunk semantics, table/figure handling, section hierarchy, and evidence provenance.
- Class: quality-attribute
- Status: active
- Description: Before scientific KG validation or scaling continues, converted paper data and chunks must satisfy an explicit graph-readiness quality contract covering conversion fidelity, normalization, chunk semantics, table/figure handling, section hierarchy, and evidence provenance.
- Why it matters: Poorly normalized conversion/chunk data will poison Claim/Entity/Relation extraction and make graph-quality results meaningless.
- Source: user-feedback-after-S10-chunk-review
- Primary owning slice: M004
- Validation: Partial bounded advancement in M022: S01-S05 produced stable source/locator/span IDs, source-hash coverage, route/review/repair diagnostics, reviewer packet artifacts, and final no-import guard with import_allowed=false and semantic_ready_for_kg=false. Full validation still requires a dedicated graph-readiness quality benchmark/acceptance pass before KG validation or scaling resumes.
- Notes: M025 S11 scope reconciliation: advanced by preprocessing diagnostics, redaction checks, and boundary traceability; remains active because positive graph-readiness acceptance is still not proven by M025. M031 S07 scope reconciliation: M031 advances graph-readiness quality only as a fail-closed refusal boundary: parser-ready converted artifact, refusal diagnostics, chunk/evidence package, pending reviewer packet, and no import eligibility without completed independent review. It does not prove full graph-readiness acceptance, so R027 remains active.

### R029 — Before KG import continues, chunking must produce an import-ready typed chunk package with stable IDs, source spans, parent-child lineage, content routes, quality states, deterministic annotations, and independent review evidence.
- Class: quality-attribute
- Status: active
- Description: Before KG import continues, chunking must produce an import-ready typed chunk package with stable IDs, source spans, parent-child lineage, content routes, quality states, deterministic annotations, and independent review evidence.
- Why it matters: Scientific KG import quality depends on chunk semantics and provenance. If chunking mixes claims, loses table/figure structure, or lacks source traceability, downstream extraction and graph import will create false KG facts.
- Source: user-directed post-M004 chunking deepening
- Primary owning slice: M005-dlko4z
- Validation: Partial bounded validation in M022: typed reviewer packet handoff/final gate verified stable packet/review/repair/route diagnostics for six targets with final schema m022-final-gate.v1, pending_review=6, importable_count=0, semantic_ready_count=0, raw_text_embedded_count=0, and unsafe_counters_zero=True. This is not a positive import-ready package validation.
- Notes: M025 S11 scope reconciliation: advanced by traceable smoke-corpus chunks and PageIndex/source-provenance diagnostics; remains active because import-ready typed packages, independent semantic review, and import eligibility are still not proven. M031 S07 scope reconciliation: M031 produces graph-readiness handoff packages and verifies import_eligible_count=0 under a no-write boundary. This is a negative/refusal proof, not a positive import-ready typed chunk package validation, so R029 remains active.

### R031 — Before drawing broader chunking/import-readiness conclusions, validation must expand from the 10-paper gold corpus to a 30-paper deviation scan that compares distributions, outliers, missing-source rates, route/refusal patterns, and new failure modes without authorizing production KG import.
- Class: quality-attribute
- Status: active
- Description: Before drawing broader chunking/import-readiness conclusions, validation must expand from the 10-paper gold corpus to a 30-paper deviation scan that compares distributions, outliers, missing-source rates, route/refusal patterns, and new failure modes without authorizing production KG import.
- Why it matters: The 10-paper corpus was useful for safety gates but is too small to reveal corpus-level deviations and recurring patterns. A 30-paper scan can expose new conversion/chunking/source/asset/refusal behaviors before any positive import or broader scaling work.
- Source: user-directed M006 expansion request
- Primary owning slice: M006-638rza
- Supporting slices: M005-dlko4z
- Validation: A 30-paper dry-run report exists with redacted aggregate/per-paper diagnostics, deviation analysis against M005 10-paper evidence, new-pattern taxonomy, and explicit no-go/go recommendations for remediation.

### R032 — Corpus validation must support an automated +10-paper iterative loop up to 100 papers, with resumable batch state, source acquisition, deviation analysis, remediation recommendations, and strict no-import/no-write safety gates.
- Class: operability
- Status: active
- Description: Corpus validation must support an automated +10-paper iterative loop up to 100 papers, with resumable batch state, source acquisition, deviation analysis, remediation recommendations, and strict no-import/no-write safety gates.
- Why it matters: Manually planning each 10-paper expansion will not scale to 100 papers. The workflow needs repeatable automation to discover deviations, improve source/chunking automation, and preserve evidence quality without unsafe KG import.
- Source: user-directed M006 scale-loop request
- Primary owning slice: future-validation-automation
- Supporting slices: M006-638rza
- Validation: A CLI or equivalent command can run batches of +10 papers, persist per-batch manifests/diagnostics/reports, resume after failures, compare each batch against prior baselines, and stop at review gates without production KG writes.

### R033 — Provide a deterministic, resumable CLI workflow for iterative +10-paper validation batches toward a 100-paper diagnostic corpus.
- Class: operability
- Status: active
- Description: Provide a deterministic, resumable CLI workflow for iterative +10-paper validation batches toward a 100-paper diagnostic corpus.
- Why it matters: Manual per-batch orchestration worked for 30 papers but will not scale safely to 100 without deterministic state, guards, and evidence artifacts.
- Source: M006-638rza S04 final recommendation
- Primary owning slice: M007-opaont
- Supporting slices: M007/S01,S02,S03,S04
- Validation: A local CLI can select the next batch, preflight/acquire sources, run redacted deviation scans, compare route/refusal deltas, flag outliers/contradictions, and persist resumable batch state without raw/chunk text or KG writes.
- Notes: The CLI must automate operational evidence collection and review gating only. It must not promote trusted KG facts, write production LadybugDB data, or enable embeddings/vector retrieval claims.

### R035 — Validation batches must fill the target accepted-paper quota by drawing deterministic replacement candidates when selected papers cannot become source-ready within bounded acquisition rules.
- Class: quality-attribute
- Status: active
- Description: Validation batches must fill the target accepted-paper quota by drawing deterministic replacement candidates when selected papers cannot become source-ready within bounded acquisition rules.
- Why it matters: A +10 batch is only useful if it actually reaches 10 accepted scan-ready papers or produces an explicit shortage blocker after bounded attempts; stopping at an underfilled first selection would silently weaken validation coverage.
- Source: user feedback during M008 after S02
- Primary owning slice: M008-c9zb94
- Supporting slices: M008/S03,M008/S04
- Validation: Partial validation: top-up pass sample final_accepted_ready_count=target_count and scan_allowed=true; blocked sample remaining_shortage_count=2 and scan_allowed=false. Missing: automatic acquisition/preflight integration for accepted replacements.
- Notes: M009/S04 implemented bounded top-up planning with pass and blocked shortage artifacts. Replacement materialization and preflight remain required during the next batch runbook before scan.

### R040 — New infrastructure must be researched, compatibility-probed, and safety-wrapped before it is enabled in the main Scientific KG process.
- Class: constraint
- Status: active
- Description: New infrastructure must be researched, compatibility-probed, and safety-wrapped before it is enabled in the main Scientific KG process.
- Why it matters: The pipeline depends on strict provenance, redaction, no-import/no-write safety, and reproducibility. Unprepared infrastructure can introduce hidden incompatibilities, secret handling risks, cost/rate failures, optimizer side effects, or false semantic readiness.
- Source: user principle during M012 planning
- Primary owning slice: project
- Validation: Future milestones that introduce infrastructure must include research/probe artifacts, failure-mode analysis, artifact/redaction boundaries, and an explicit go/no-go decision before process activation.
- Notes: M025 S11 scope reconciliation: advanced by five-article local preprocessing replay and readiness-for-larger-preprocessing evidence only; M025 does not introduce new infrastructure (only reuses existing M105 onion-organized packages), so research/probe artifacts and go/no-go decisions remain applicable for future R040-active infrastructure (DSPy/RLM/production-LadybugDB), which is explicitly out of M025 scope. M025 also does not enable new infrastructure in the main Scientific KG process; it remains preprocessing-only with no-network/no-import/no-write safety flags.

### R050 — Provide a deterministic CLI for detecting article structure artifacts and candidate KG scaffold links from preserved paper sources without performing KG import.
- Class: core-capability
- Status: active
- Description: Provide a deterministic CLI for detecting article structure artifacts and candidate KG scaffold links from preserved paper sources without performing KG import.
- Why it matters: The project needs an automated pre-KG layer that turns paper structure into typed, reviewable artifact candidates such as figures, tables, equations, datasets, methods, metrics, experiments, claims, citations, spans, and candidate relationships before any knowledge graph write is considered.
- Source: user-directed post-M022 artifact detection plan
- Primary owning slice: M023-vk5wb2/S02
- Supporting slices: M023-vk5wb2/S01,M023-vk5wb2/S04,M023-vk5wb2/S05
- Validation: A CLI command can process bounded source manifests or validation batch state, produce per-paper artifact manifests and run summaries with stable IDs, source spans, candidate links, review states, provenance, and explicit kg_import_allowed=false.
- Notes: Traceability cleanup: M023-vk5wb2 is the planned milestone that implements the former next-milestone article artifact detection requirement. S02 owns the deterministic CLI; S01 supplies the contract, S04 supplies metrics, and S05 supplies the final no-import scaffold gate. This requirement remains explicitly pre-import: production LadybugDB writes, trusted fact promotion, embeddings/vector readiness, and unattended scaling remain blocked. M031 S07 scope reconciliation: M031 advances continuity and refusal-boundary evidence for selected refs but does not globally close the deterministic artifact-detection CLI requirement.

### R051 — MiniMax may assist article artifact detection only as a bounded structured helper with forced tool calls, local schema validation, redacted inputs, and non-authoritative outputs.
- Class: integration
- Status: active
- Description: MiniMax may assist article artifact detection only as a bounded structured helper with forced tool calls, local schema validation, redacted inputs, and non-authoritative outputs.
- Why it matters: MiniMax can help classify and link structural artifacts at scale, but the project safety model requires it to remain a helper rather than a source of truth or writer.
- Source: user-directed post-M022 artifact detection plan
- Primary owning slice: M023-vk5wb2/S03
- Supporting slices: M023-vk5wb2/S01,M023-vk5wb2/S02,M023-vk5wb2/S05
- Validation: A bounded MiniMax adapter is wired into the artifact detection CLI behind an explicit flag, validated by tests and fixture runs proving forced tool-call request shape, local schema validation, refusal of unsafe payloads, redacted diagnostics, and no KG import authorization.
- Notes: Traceability cleanup: M023-vk5wb2/S03 owns bounded MiniMax helper integration. S01 supplies the artifact safety contract, S02 supplies the CLI surface, and S05 verifies MiniMax-derived candidates remain non-authoritative and non-importable. MiniMax output must set minimax_source_of_truth=false and review_state=pending_review or repair_required; raw paper payloads, secrets, raw responses, production import, and production writes remain blocked.

### R052 — DSPy prompt optimization for artifact detection must remain gated until benchmark fixtures, metrics, and baseline MiniMax or deterministic outputs exist.
- Class: quality-attribute
- Status: active
- Description: DSPy prompt optimization for artifact detection must remain gated until benchmark fixtures, metrics, and baseline MiniMax or deterministic outputs exist.
- Why it matters: Self-improving prompts are only useful if the system can measure whether artifact detection is improving without leaking raw corpus text or promoting false KG facts.
- Source: user-directed post-M022 artifact detection plan
- Primary owning slice: M023-vk5wb2/S04
- Supporting slices: M023-vk5wb2/S02,M023-vk5wb2/S03,M023-vk5wb2/S05
- Validation: A benchmark fixture set and metric report exist for artifact detection precision, recall, span coverage, link correctness, section lineage correctness, raw leakage rate, and review burden; the final gate either blocks or explicitly scopes any DSPy optimizer activation.
- Notes: Traceability cleanup: M023-vk5wb2/S04 owns benchmark fixtures and metrics before any DSPy or optimizer recommendation. S02 and S03 provide deterministic and MiniMax-assisted outputs for comparison, and S05 consumes the metric evidence for a DSPy readiness/no-go gate. DSPy or optimizer behavior must not be activated before evaluation fixtures and metrics exist.

### R054 — Provide a durable lazy async sidecar pipeline for article processing jobs.
- Class: core-capability
- Status: active
- Description: Provide a durable lazy async sidecar pipeline for article processing jobs.
- Why it matters: M033 introduced multiple sidecar candidates that run at different speeds and can fail independently. The project needs persistent queues/statuses so source acquisition, GROBID, OpenDataLoader, Adaptix mapping, validation, review packet creation, and graph-readiness review can run lazily, resume after failures, and avoid unnecessary recomputation.
- Source: M033 follow-up discussion
- Primary owning slice: future pipeline orchestration milestone
- Validation: A future milestone defines and verifies persisted job/artifact state with statuses, input/output hashes, dependency readiness, stale detection, and resume/retry behavior.

### R055 — Track sidecar job lifecycle, retries, typed blockers, and backend/cache health explicitly.
- Class: failure-visibility
- Status: active
- Description: Track sidecar job lifecycle, retries, typed blockers, and backend/cache health explicitly.
- Why it matters: GROBID, OpenDataLoader, Adaptix, and future review stages have distinct latency, runtime dependencies, and failure modes. Failures such as unhealthy backend, missing model cache, unstable network, stale source hash, adapter mapping failure, or low-quality output must become typed state, not lost in logs or in-memory batches.
- Source: M033 follow-up discussion
- Primary owning slice: future pipeline orchestration milestone
- Validation: A future verifier proves per-job status, attempt count, retry_after, last_error_code, output_paths, backend/cache health, and dead-letter/terminal blocker states are persisted and queryable.

### R056 — Parser sidecar outputs must remain candidate evidence until daily-archive validators, review packets, and graph-readiness review pass.
- Class: constraint
- Status: active
- Description: Parser sidecar outputs must remain candidate evidence until daily-archive validators, review packets, and graph-readiness review pass.
- Why it matters: M033 confirmed that GROBID, OpenDataLoader, Adaptix, and quant-mind patterns are useful but not graph-ready or import-eligible by themselves. The orchestration layer must preserve no-write/no-import safety and prevent parser success from becoming semantic KG promotion.
- Source: M033 follow-up discussion
- Primary owning slice: future pipeline orchestration milestone
- Validation: All future sidecar pipeline artifacts keep graph_import_allowed=false, ladybugdb_written=false, production_import_attempted=false, and import_eligible=false until a separately authorized graph-readiness/import milestone changes those flags with evidence.

### R079 — Every pipeline service that touches external or local I/O (ArxivClient, TEI Embedder, Markdown Converter, SemanticScholar) must wrap network/local calls in typed error handling (HTTPError, TimeoutException, ConnectError, 429-rate-limit) and surface typed diagnostics (error code, service name, redacted message, retry count, final outcome) instead of propagating opaque exceptions.
- Class: quality-attribute
- Status: active
- Description: Every pipeline service that touches external or local I/O (ArxivClient, TEI Embedder, Markdown Converter, SemanticScholar) must wrap network/local calls in typed error handling (HTTPError, TimeoutException, ConnectError, 429-rate-limit) and surface typed diagnostics (error code, service name, redacted message, retry count, final outcome) instead of propagating opaque exceptions.
- Why it matters: Opaque exception propagation (e.g. bare httpx.HTTPStatusError from ArxivClient) makes unattended pipeline failures undiagnosable and prevents operators from distinguishing transient rate-limiting from permanent failures.
- Source: M199 live pipeline audit 2026-07-01
- Primary owning slice: M199-4rex3i
- Supporting slices: S01, S02, S03, S04
- Validation: Each service module raises a typed error on exhausted retries carrying error code, service name, redacted message, retry count; integration tests with a mock transport verify typed diagnostics for 429/5xx/timeout.

### R080 — Transient failures (429, 5xx, timeout) for pipeline services must retry with bounded backoff honoring Retry-After where present. The mature catalog_ingest retry/backoff/Retry-After pattern (ARXIV_BACKOFF_SECONDS=(1,5,15,60,300)) is the established project pattern and must be reused rather than reinvented.
- Class: quality-attribute
- Status: active
- Description: Transient failures (429, 5xx, timeout) for pipeline services must retry with bounded backoff honoring Retry-After where present. The mature catalog_ingest retry/backoff/Retry-After pattern (ARXIV_BACKOFF_SECONDS=(1,5,15,60,300)) is the established project pattern and must be reused rather than reinvented.
- Why it matters: ArxivClient currently has zero retry on transient failures, while catalog_ingest has mature retry+backoff for the same arxiv API — a duplicated-but-divergent pattern that contradicts Ponytail 'already-installed pattern solves it'.
- Source: M199 live pipeline audit 2026-07-01
- Primary owning slice: M199-4rex3i
- Supporting slices: S01, S03
- Validation: Integration tests verify the retry schedule and Retry-After honour for each retried service (ArxivClient, Markdown Converter arxiv2md). No service silently skips transient failures.

### R081 — Graceful degradation (e.g. TEI Embedder returning zero vectors when the circuit-breaker is open) is allowed only when the degraded output is explicitly marked via a typed signal or safety flag, and downstream consumers fail closed on or annotate its use. No service may silently degrade to corrupt data.
- Class: operability
- Status: active
- Description: Graceful degradation (e.g. TEI Embedder returning zero vectors when the circuit-breaker is open) is allowed only when the degraded output is explicitly marked via a typed signal or safety flag, and downstream consumers fail closed on or annotate its use. No service may silently degrade to corrupt data.
- Why it matters: TEI Embedder currently returns 1024-dim zero vectors on circuit-breaker open, and downstream retrieval/semantic steps receive garbage they cannot detect — silent data corruption in the KG.
- Source: M199 live pipeline audit 2026-07-01
- Primary owning slice: M199-4rex3i
- Supporting slices: S02, S04
- Validation: Live test demonstrates that TEI outage produces an explicit degraded signal; downstream embedding-stamping path refuses or annotates; no unmarked-good zero vectors reach artifacts.

### R082 — The ScoringEngine recency score must document and enforce its same-day-run contract: a retrospective scoring run must either use an explicit run_date parameter or be rejected, never silently score all papers recency=0.5 by comparing against date.today().
- Class: constraint
- Status: active
- Description: The ScoringEngine recency score must document and enforce its same-day-run contract: a retrospective scoring run must either use an explicit run_date parameter or be rejected, never silently score all papers recency=0.5 by comparing against date.today().
- Why it matters: _recency_score compares against date.today(), so any historical/replay run silently scores all papers 0.5 recency (20% of score weight), distorting the ranking without any signal.
- Source: M199 live pipeline audit 2026-07-01
- Primary owning slice: M199-4rex3i
- Supporting slices: S03
- Validation: Contract test confirms a retrospective run without explicit run_date is corrected or rejected rather than producing uniform recency=0.5.

### R083 — Error diagnostics across all pipeline services must never include raw payload, raw text, embeddings, vectors, or secrets — only error code, service name, redacted message, retry count, and final outcome.
- Class: compliance/security
- Status: active
- Description: Error diagnostics across all pipeline services must never include raw payload, raw text, embeddings, vectors, or secrets — only error code, service name, redacted message, retry count, and final outcome.
- Why it matters: Services handle arxiv API responses, embedding vectors, and conversion text. Diagnostic leakage of these would violate the project's EvidencePath and fail-closed safety invariants.
- Source: M199 live pipeline audit 2026-07-01
- Primary owning slice: M199-4rex3i
- Supporting slices: S01, S02, S03, S04
- Validation: Review of diagnostics surfaces confirms no raw payload or secret leakage; redaction verified via contract test.

## Validated

### R001 — CLI help info
- Class: core-capability
- Status: validated
- Description: CLI help info
- Why it matters: Hermes and cron agents must discover usage without reading source code.
- Source: M001
- Validation: S01 implemented a Typer app exposing project purpose, artifact paths, exit codes, and non-goals in help output; S05 verified help contracts via subprocess tests.
- Notes: Typer app exposes project purpose, artifact paths, exit codes, and non-goals

### R002 — CLI `--date` analysis
- Class: core-capability
- Status: validated
- Description: CLI `--date` analysis
- Why it matters: Hermes needs deterministic daily analysis for a selected date.
- Source: M001
- Validation: S02 wired explicit-date CLI analysis to ArxivClient and normalized DailyAnalysis output.
- Notes: Wires `--date` to ArxivClient producing `DailyAnalysis` result

### R003 — JSON result in sessions
- Class: primary-user-loop
- Status: validated
- Description: JSON result in sessions
- Why it matters: Hermes needs a stable machine-readable result artifact after each run.
- Source: M001
- Validation: S03 implemented and verified Hermes-readable session JSON under `~/research/ops/sessions/YYYY-MM-DD.json`.
- Notes: `write_session_json()` writes `~/research/ops/sessions/YYYY-MM-DD.json`

### R004 — Save full list of papers
- Class: core-capability
- Status: validated
- Description: Save full list of papers
- Why it matters: Future calibration and graph ingestion require the complete daily paper set.
- Source: M001
- Validation: S03 daily artifacts save full raw and scored paper lists instead of only top-N outputs.
- Notes: `write_daily_artifacts()` saves full `papers.json` and `scored.json`

### R005 — Per-paper artifacts
- Class: primary-user-loop
- Status: validated
- Description: Per-paper artifacts
- Why it matters: Reusable per-paper artifacts are needed for later enrichment, graph ingestion, and preference calibration.
- Source: M001
- Validation: S04 added idempotent per-paper raw and scored JSON artifacts under `~/research/papers/{arxiv-id}/`.
- Notes: `write_paper_artifacts()` creates `paper.json` and `scored.json` per arxiv-id

### R006 — Topic overview aggregates
- Class: primary-user-loop
- Status: validated
- Description: Topic overview aggregates
- Why it matters: Interest calibration needs a quick overview of topics and signals for each day.
- Source: M001
- Validation: S04 populated overview category counts, keyword counts, top papers, and deterministic aggregates.
- Notes: `build_overview_payload()` aggregates categories, keywords, top papers

### R007 — Transparent score breakdown
- Class: quality-attribute
- Status: validated
- Description: Transparent score breakdown
- Why it matters: Scores must be interpretable for calibration and debugging.
- Source: M001
- Validation: S04 includes detailed score breakdown statistics in per-paper scored JSON and daily overview JSON.
- Notes: `score_breakdown` statistics in per-paper and daily overview

### R008 — Queue state file
- Class: operability
- Status: validated
- Description: Queue state file
- Why it matters: Cron/Hermes runs need observable state even when a run fails.
- Source: M001
- Validation: S05 implemented queue state persistence in `~/research/ops/queue/YYYY-MM-DD.json` across running, done, empty, and failed states.
- Notes: `write_queue_state()` tracks running → done/empty/failed lifecycle

### R009 — Idempotent reruns
- Class: operability
- Status: validated
- Description: Idempotent reruns
- Why it matters: Scheduled jobs and manual reruns must be safe.
- Source: M001
- Validation: S03, S04, and S05 verified same-date rerun overwrites are deterministic and do not duplicate artifacts.
- Notes: Last-writer-wins overwrite across all artifact writers

### R010 — Rust-portable contracts
- Class: integration
- Status: validated
- Description: Rust-portable contracts
- Why it matters: Hermes and a possible future Rust rewrite need language-neutral file and CLI contracts.
- Source: M001
- Validation: S01 established portable exit codes; S03 introduced explicit JSON-native serializers with ISO strings and no Python repr/dataclass metadata.
- Notes: Explicit serializer functions, portable exit-code vocabulary

### R011 — Follow style guide/lint
- Class: quality-attribute
- Status: validated
- Description: Follow style guide/lint
- Why it matters: The cron-safe foundation should remain maintainable and regression-tested.
- Source: M001
- Validation: S05 confirmed zero Ruff lint findings and a passing full pytest suite for M001.
- Notes: Zero Ruff lint findings, full pytest suite passes

### R012 — Empty day handling
- Class: core-capability
- Status: validated
- Description: Empty day handling
- Why it matters: No-paper days are normal scheduler outcomes, not failures.
- Source: M001
- Validation: S02 explicitly returns `empty` status with exit 0 for zero-paper days; S03/S05 verified valid empty JSON outputs.
- Notes: `status="empty"` with exit 0, valid empty JSON arrays

### R013 — Pytest contract coverage
- Class: quality-attribute
- Status: validated
- Description: Pytest contract coverage
- Why it matters: CLI/file contracts must be protected by tests that do not depend on live services.
- Source: M001
- Validation: S05 added offline subprocess tests for help output, JSON schemas, empty results, failure states, and rerun idempotency.
- Notes: Offline subprocess tests for help, JSON, empty, failure, rerun

### R014 — Local full-text ingestion must produce deterministic, provenance-rich ingestion results for markdown and plain-text paper artifacts.
- Class: core-capability
- Status: validated
- Description: Local full-text ingestion must produce deterministic, provenance-rich ingestion results for markdown and plain-text paper artifacts.
- Why it matters: PageIndex, chunking, evidence paths, and later scientific KG layers need a stable local text boundary with explicit diagnostics before any PDF/network/LLM behavior is introduced.
- Source: M003 requirements restoration after S03
- Primary owning slice: M003-km5fty/S01
- Supporting slices: S02,S03
- Validation: Validated by S01: `uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` passed during S01 closeout.
- Notes: Restores M003 full-text prerequisite from the missing historical R026-R035 range using current GSD auto-assigned IDs.

### R015 — PageIndex document navigation must represent fixture papers as deterministic PageIndexNode hierarchies with parent, child, NEXT, path, and validation diagnostics.
- Class: core-capability
- Status: validated
- Description: PageIndex document navigation must represent fixture papers as deterministic PageIndexNode hierarchies with parent, child, NEXT, path, and validation diagnostics.
- Why it matters: Semantic chunks, evidence paths, graph storage, and RLM document navigation all need stable intra-paper structure and navigable node IDs.
- Source: M003 requirements restoration after S03
- Primary owning slice: M003-km5fty/S02
- Supporting slices: S03,S09
- Validation: Validated by S02: `uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` passed during S02 closeout.
- Notes: Restores M003 PageIndex requirement from the missing historical R026-R035 range using current GSD auto-assigned IDs.

### R016 — SemanticChunk records and EvidencePath objects must provide deterministic traceability from Paper to PageIndexNode to chunk.
- Class: core-capability
- Status: validated
- Description: SemanticChunk records and EvidencePath objects must provide deterministic traceability from Paper to PageIndexNode to chunk.
- Why it matters: Claims, retrieval contexts, and graph persistence need evidence anchors that can be checked without re-parsing full text or PageIndex internals.
- Source: M003 requirements restoration after S03
- Primary owning slice: M003-km5fty/S03
- Supporting slices: S04,S05,S06,S07,S09
- Validation: Validated by S03: `uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` passed with 44 tests.
- Notes: Restores M003 SemanticChunk/EvidencePath requirement from the missing historical R026-R035 range using current GSD auto-assigned IDs.

### R017 — Claim, ScientificEntity, and ScientificRelation contracts must reference EvidencePath records or explicit validation errors.
- Class: core-capability
- Status: validated
- Description: Claim, ScientificEntity, and ScientificRelation contracts must reference EvidencePath records or explicit validation errors.
- Why it matters: Scientific KG extraction must be typed, traceable, and validator-backed before storage, retrieval, or DSPy/RLM workflows rely on it.
- Source: M003 requirements restoration after S03
- Primary owning slice: M003-km5fty/S04
- Supporting slices: S03,S05,S07,S08
- Validation: S04 verification passed: `uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` => 50 passed; Ruff all checks passed; CLI help smoke exit 0; LSP diagnostics clean; Pyrefly 0 errors; Ty all checks passed.
- Notes: Validated by M003-km5fty/S04. S04 added deterministic Claim, ScientificEntity, ScientificRelation, and ExtractionPatch contracts backed by EvidencePath fields plus explicit validation diagnostics. The slice intentionally remains local-only and does not add LLM/DSPy/storage/retrieval behavior.

### R018 — LadybugDB SCI KG schema must store Paper, PageIndexNode, SemanticChunk, Claim, ScientificEntity, ScientificRelation, EvidencePath, and required edges idempotently and transaction-safely.
- Class: integration
- Status: validated
- Description: LadybugDB SCI KG schema must store Paper, PageIndexNode, SemanticChunk, Claim, ScientificEntity, ScientificRelation, EvidencePath, and required edges idempotently and transaction-safely.
- Why it matters: Hybrid retrieval and graph traversal require a durable scientific graph schema whose writes are safe, repeatable, and observable.
- Source: M003 requirements restoration after S03
- Primary owning slice: M003-km5fty/S05
- Supporting slices: S02,S03,S04,S06,S10
- Validation: Validated by M003-km5fty S05 post-review verification: `uv run pytest tests/test_ladybug_scientific_kg.py tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_ladybug_client_property.py tests/test_scientific_kg_e2e.py tests/test_cli_contract.py -q` passed with 42 tests; Ruff passed on touched files; Pyrefly reported 0 errors; Ty passed on `src/` plus S05 test; CLI help smoke exited 0; LSP diagnostics were clean; GitNexus detect_changes was reviewed with expected high scope for the uncommitted S05 persistence expansion.
- Notes: S05 added post-review regression coverage requiring patch-embedded claim/entity/relation EvidencePath references to be present in the persisted evidence_paths list before any write transaction opens.

### R020 — Evaluation fixtures and metrics must exist before scale, optimizer, DSPy, RLM, or retrieval-quality claims are made.
- Class: quality-attribute
- Status: validated
- Description: Evaluation fixtures and metrics must exist before scale, optimizer, DSPy, RLM, or retrieval-quality claims are made.
- Why it matters: Without metrics and benchmark fixtures, DSPy/RLM or hybrid retrieval changes would create unverifiable quality claims.
- Source: M003 requirements restoration after S03 and D001
- Primary owning slice: M003-km5fty/S07
- Supporting slices: S03,S04,S06,S08,S09,S10
- Validation: Validated by M003-km5fty/S07: focused closeout verification passed with `uv run pytest tests/test_evaluation_benchmark.py tests/test_hybrid_retrieval.py tests/test_ladybug_scientific_kg.py tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py -q` => 34 passed; Ruff lint and format checks passed; `ty`, `pyrefly`, and CLI help smoke passed. S07 added typed evaluation contracts and fixture-backed benchmark tests for schema validity, groundedness proxy counts, evidence-path hit rate, retrieval recall, and vector-only/graph-only/hybrid ablation metrics over deterministic local fixtures before DSPy/RLM/optimizer claims.
- Notes: Validated by M003-km5fty/S07. Diagnostics are intentionally text-safe and local-only: benchmark outputs expose IDs, counts, modes, query text, metric values, and existing S06 diagnostics without full paper/chunk body text, embeddings, live services, DSPy, RLM traversal, or optimizer behavior.

### R021 — DSPy extraction boundaries must remain disabled or non-optimizing until evaluation metrics and benchmark fixtures are verified.
- Class: constraint
- Status: validated
- Description: DSPy extraction boundaries must remain disabled or non-optimizing until evaluation metrics and benchmark fixtures are verified.
- Why it matters: DSPy optimizers or LM modules without verified metrics would create false confidence and couple extraction quality to unmeasured prompts.
- Source: D001 and M003 requirements restoration after S03
- Primary owning slice: M003-km5fty/S08
- Supporting slices: S04,S07
- Validation: Validated by M003-km5fty/S08 closeout: `uv run python -c "from arxiv_archive.dspy_extraction import BaselineDspyExtractionModule, DspyExtractionInput, DspyExtractionOutput"` exited 0; focused verification passed with `uv run pytest tests/test_dspy_extraction_boundary.py tests/test_evaluation_benchmark.py tests/test_scientific_extraction_contracts.py -q` => 23 passed; Ruff lint and format checks passed; `ty` passed; `pyrefly` reported 0 errors; CLI help smoke exited 0; `gitnexus detect-changes --repo daily-archive` reported no changes detected. S08 added a deterministic DSPy-compatible `forward()` boundary around existing `ExtractionPatch` callables, reusing S04 extraction contracts and S07 schema/groundedness metric gates while keeping optimizer/runtime behavior disabled and diagnostics text-safe.
- Notes: Validated by M003-km5fty/S08. The boundary is dependency-light and does not import DSPy on the normal runtime path, does not modify storage schema, rejects requested optimizer configuration, and exposes ID/count/status diagnostics plus explicit non-optimizer metadata for downstream S09 read-only RLM workflow use.

### R025 — Full-text acquisition and real-corpus KG validation must emit structured Loguru-based logs and persisted diagnostics for each selected paper before rerunning the 10-document validation.
- Class: operability
- Status: validated
- Description: Full-text acquisition and real-corpus KG validation must emit structured Loguru-based logs and persisted diagnostics for each selected paper before rerunning the 10-document validation.
- Why it matters: The first real-data validation stopped at missing full-text inputs. Before adding the full-text bridge, future agents need reliable logs to understand per-paper acquisition/conversion decisions and failures.
- Source: user-directed M004 follow-up after S01 validation
- Primary owning slice: M004-ubh2pt/full-text-bridge
- Supporting slices: M004-ubh2pt/S01
- Validation: Validated by M004-ubh2pt/S02. S02 implemented and verified Loguru-based structured diagnostics via a narrow ValidationLogger/JSONL event stream, used it during the selected ten-paper full-text bridge, and reran the 10-document structural validation with per-paper outcomes and redacted failure details recorded. Evidence: S02-SUMMARY.md and S02-UAT.md explicitly prove R025.
- Notes: Validated as the observability bridge for rerunning the selected 10-document validation. This does not imply broad graph quality or semantic KG readiness; it proves structured Loguru diagnostics existed and were used for the full-text bridge/rerun.

### R026 — Before scaling validation to 10, 20, or larger document batches, the real-data scientific KG pipeline must be debugged end-to-end on the current small corpus through full text, PageIndex, SemanticChunk, EvidencePath, ExtractionPatch, SCI KG persistence, and retrieval diagnostics.
- Class: quality-attribute
- Status: validated
- Description: Before scaling validation to 10, 20, or larger document batches, the real-data scientific KG pipeline must be debugged end-to-end on the current small corpus through full text, PageIndex, SemanticChunk, EvidencePath, ExtractionPatch, SCI KG persistence, and retrieval diagnostics.
- Why it matters: The current validation has proven full-text/chunk/evidence readiness but not claim/entity/relation extraction, graph persistence, or retrieval quality. Scaling before debugging would produce ambiguous graph quality results.
- Source: user-directed M004 pipeline debugging gate
- Primary owning slice: M004-ubh2pt/pipeline-debug
- Supporting slices: M004-ubh2pt/S01,S02
- Validation: Validated narrowly as an end-to-end real-data pipeline debug/plumbing gate by M004-ubh2pt/S03 and M004 validation. S03 exercised converted full text through PageIndex, SemanticChunk, EvidencePath, explicitly labeled debug-baseline ExtractionPatch generation, SCI KG persistence plumbing for eligible papers, and retrieval diagnostics, while documenting conversion/extraction blockers and preventing 20-document scaling.
- Notes: Validated only as a debug-before-scaling gate, not as proof of real semantic KG quality, production persistence readiness, broad retrieval quality, or corpus scaling. M004 explicitly kept broader KG readiness, semantic/vector retrieval, entity/relation extraction, and production LadybugDB persistence blocked.

### R028 — Validation of conversion, chunking, extraction, and graph-readiness must include an independent artifact review step where feasible, preferably via a subagent, that inspects raw outputs and test meaningfulness rather than relying only on code tests or mocked fixtures.
- Class: quality-attribute
- Status: validated
- Description: Validation of conversion, chunking, extraction, and graph-readiness must include an independent artifact review step where feasible, preferably via a subagent, that inspects raw outputs and test meaningfulness rather than relying only on code tests or mocked fixtures.
- Why it matters: Passing tests can create false confidence when tests only verify plumbing, mocks, or counts. Scientific KG validation needs human-readable artifact quality checks to catch semantic failures.
- Source: user-feedback-after-S10-quality-review
- Primary owning slice: M004
- Validation: M022 S04/S05 produced and verified six bounded reviewer packets plus an independent assessment and final no-import gate. Evidence: reviewer packet verifier reported packets=6, pending_review=6, assessment_verdict=blocked_pending_semantic_acceptance, unsafe_counters_zero=True; final recommendation maps R028 as validated for bounded artifact review.
- Notes: Validated for the bounded M022 reviewer surface. This does not authorize KG import, semantic acceptance, broad scaling, embeddings/vectors, or production LadybugDB writes.

### R030 — Article ingestion must preserve source artifacts alongside derived text, including the original PDF, normalized Markdown, extracted figures, tables, image assets, hashes, provenance, and redacted asset manifests for future multimodal retrieval.
- Class: continuity
- Status: validated
- Description: Article ingestion must preserve source artifacts alongside derived text, including the original PDF, normalized Markdown, extracted figures, tables, image assets, hashes, provenance, and redacted asset manifests for future multimodal retrieval.
- Why it matters: Scientific papers often contain figures, plots, tables, and diagrams that are necessary for future multimodal search and evidence review. Losing raw source artifacts during ingestion would make later multimodal KG and retrieval work incomplete or non-reproducible.
- Source: user-direction
- Primary owning slice: M005-dlko4z/S05
- Validation: M024 S04 closeout verified metadata-only asset preservation contract and bridge integration with fixture manifests for figures/tables/equation images, fail-closed forbidden-payload validation, stable IDs/provenance/status summaries, and full regression/property suite: `uv run pytest tests/test_article_loader.py tests/test_article_artifacts.py tests/test_source_asset_manifest.py tests/test_article_evidence_bridge.py tests/test_property_article_evidence_bridge.py tests/test_article_page_index.py tests/test_property_article_page_index.py tests/test_article_assets.py tests/test_property_article_assets.py -q` passed 132 tests.
- Notes: M025 S11 scope reconciliation: already validated by M024 asset-preservation evidence and supported by M025 metadata-preserving local replay; no new validation status change needed.

### R034 — Run the first genuinely new +10-paper validation batch through the deterministic M007 validation-batch workflow.
- Class: primary-user-loop
- Status: validated
- Description: Run the first genuinely new +10-paper validation batch through the deterministic M007 validation-batch workflow.
- Why it matters: M007 proved the workflow over the existing 30-paper corpus. The next risk is whether the workflow handles newly selected papers with real source availability, readiness contradictions, scan deltas, and review gates.
- Source: M007-opaont S04 final recommendation
- Primary owning slice: M008-c9zb94
- Supporting slices: M008/S01,S02,S03,S04
- Validation: M008 evidence: selected_count=10, m006_overlap_count=0, final source_ready=10, quota accepted_ready_count=10, scan paper_count=10, chunk_count=1591, outlier_count=6, import_eligible_chunk_count=0, review verdict FLAG with next-batch gate.
- Notes: Validated by M008: one genuinely new +10 batch was selected, source-ready, quota-gated, scanned, and independently reviewed. The review requires bounded top-up automation before another +10, but R034's one-batch goal is satisfied.

### R036 — Validation CLI runs must produce replay/audit provenance logs tying each generated artifact to the exact command, inputs, output hashes, exit code, cwd, git commit, and active milestone/batch context.
- Class: failure-visibility
- Status: validated
- Description: Validation CLI runs must produce replay/audit provenance logs tying each generated artifact to the exact command, inputs, output hashes, exit code, cwd, git commit, and active milestone/batch context.
- Why it matters: Current validation artifacts can be checked for consistency, but they do not fully prove that each artifact was freshly produced by a specific CLI run. Provenance is required to detect stale artifacts and metadata mismatches such as an M008 artifact carrying M006 milestone metadata.
- Source: user feedback after M008 completion
- Primary owning slice: next validation hardening milestone
- Validation: M027-aakeky S06 validated R036-style provenance for replay/gate artifacts via `uv run python scripts/verify_m027_end_to_end_mixed_replay.py && uv run python scripts/verify_m027_provenance_and_riskratchet_gate.py --validate-only && uv run python -m pytest tests/test_m027_provenance_and_riskratchet_gate.py tests/test_riskratchet_gate.py tests/test_m027_end_to_end_mixed_replay.py -q` (gsd_exec 7ac737ec-e48b-45a2-bf86-18fa892e9c51, exit 0, 32 passed). The S06 summary records command, cwd, git commit, input/output artifact hashes, exit code/status, milestone/slice context, self-hash exclusion rationale, and fail-closed safety/riskratchet flags.
- Notes: M027 S06 advances provenance on the mixed-source replay artifacts without graph import, LadybugDB writes, trusted fact promotion, production import, network replay, or using riskratchet as a blocking correctness gate.

### R037 — Run the next reviewed +10 validation batch using M009 runbook gates: active scan lineage, real provenance entry, artifact freshness verification, and bounded top-up handling before scan.
- Class: core-capability
- Status: validated
- Description: Run the next reviewed +10 validation batch using M009 runbook gates: active scan lineage, real provenance entry, artifact freshness verification, and bounded top-up handling before scan.
- Why it matters: M009 allows exactly one next +10 only under explicit gates. This validates whether the hardening works on a real batch rather than only synthetic evidence.
- Source: post-M009 user approval
- Primary owning slice: next reviewed +10 milestone
- Validation: M010 final guard: review_verdict=PASS; selected_count=10; prior_overlap_count=0; quota_ready_count=10; paper_count=10; chunk_count=1477; freshness_verdict=fresh; import_eligible_chunk_count=0; positive_import_blocked=true; production_writes_blocked=true; unattended_scaling_blocked=true.
- Notes: Validated by M010-06v9ke: one reviewed next +10 batch was selected with prior_overlap_count=0, source quota was materialized to 10/10 after bounded replacements, scan used active M010 lineage, real provenance/freshness returned fresh for run_id=m010-s03-scan-002, independent review verdict PASS, and import/write/scaling gates remained blocked.

### R038 — Before any positive KG import, a reviewed semantic evidence gate must evaluate a small subset of scanned chunks/outliers for factual extraction readiness without writing to production LadybugDB.
- Class: quality-attribute
- Status: validated
- Description: Before any positive KG import, a reviewed semantic evidence gate must evaluate a small subset of scanned chunks/outliers for factual extraction readiness without writing to production LadybugDB.
- Why it matters: Operational scan counts and zero import-eligible chunks do not prove semantic KG quality. The project needs human/reviewer-visible evidence about whether chunks can support trusted scientific claims before import work resumes.
- Source: M010 final recommendation
- Primary owning slice: M011
- Validation: M011 final guard: review_verdict=PASS; gate_result=pass_negative_readiness_gate; target_count=10; source_hash_missing_count=0; repair_required_count=7; retrieval_only_count=3; import_candidate_count=0; raw_payload_key_count=0; positive_import_blocked=true; production_writes_blocked=true; chunk_span_provenance_required_next=true.
- Notes: M011 validates the negative semantic gate: 10 M010-derived targets were selected with source paths/hashes and no payload leakage, rubric judged 7 repair_required and 3 retrieval_only with zero import candidates, independent review verdict PASS, and final guard keeps positive import, production writes, semantic KG readiness claims, and unattended scaling blocked. Next evidence required: chunk-level span provenance and candidate locators before any positive import rehearsal.

### R039 — Before enabling DSPy or MiniMax in the Scientific KG pipeline, the project must complete parallel compatibility research proving version/API requirements, minimal invocation paths, failure modes, artifact boundaries, and no-import/no-write safety constraints.
- Class: constraint
- Status: validated
- Description: Before enabling DSPy or MiniMax in the Scientific KG pipeline, the project must complete parallel compatibility research proving version/API requirements, minimal invocation paths, failure modes, artifact boundaries, and no-import/no-write safety constraints.
- Why it matters: DSPy and MiniMax could be valuable, but enabling them without compatibility probes risks discovering API, dependency, cost, modality, or safety incompatibilities during the critical import-readiness path.
- Source: post-M011 planning
- Primary owning slice: M012
- Validation: M012 final guard: review_verdict=PASS; dspy_verdict=conditional_go_optional_dev_probe_only; minimax_verdict=conditional_go_optional_helper_probe_only; production_import_allowed=false; dspy_optimizer_allowed=false; minimax_orchestrator_allowed=false; next_safe_options=[dspy_optional_dev_dependency_no_lm_probe, minimax_explicit_synthetic_auth_smoke_test, chunk_span_provenance_candidate_locator_packet].
- Notes: Validated by M012-a7v8fw as compatibility research, not activation. DSPy: local/vendor and 2026 best-practice research complete; import currently blocked_missing_dependencies (`cloudpickle`), optional/dev no-LM probe allowed later, production runtime and optimizers blocked. MiniMax: official-doc research and no-call synthetic payload dry run complete; key presence recorded without value; live call not attempted; optional helper probe allowed later with explicit approval; orchestration/source-of-truth/direct PDF ingestion blocked. Integration guard keeps positive import, production writes, and unattended scaling blocked.

### R041 — Before any DSPy optimizer or MiniMax helper is used in the Scientific KG process, the project must prove detailed optimizer applicability, dependency/install feasibility, bounded invocation, and explicit blocked/allowed scopes.
- Class: constraint
- Status: validated
- Description: Before any DSPy optimizer or MiniMax helper is used in the Scientific KG process, the project must prove detailed optimizer applicability, dependency/install feasibility, bounded invocation, and explicit blocked/allowed scopes.
- Why it matters: M012 proved only high-level compatibility. The user needs concrete DSPy optimizer details, dependency proof, and next-step MiniMax callability boundaries before adoption decisions can be safe.
- Source: post-M012 user request
- Primary owning slice: M013
- Validation: M013 final guard: review_verdict=PASS; dspy_dependency_verdict=pass_isolated_optional_dev_probe_ready; dspy_install_succeeded=true; dspy_import_succeeded=true; dspy_predict_failed_closed_without_lm=true; dspy_evaluate_static_program_succeeded=true; dspy_possible_dev_optimizers=[KNNFewShot,LabeledFewShot]; dspy_optimizer_execution_allowed=false; minimax_smoke_verdict=pass_synthetic_callability_only; minimax_http_status=200; minimax_orchestrator_allowed=false; production_import_allowed=false.
- Notes: Validated by M013: DSPy installed/imported in isolated temp venv; Predict without LM failed closed; static Evaluate succeeded; project deps were not modified; optimizer inventory/applicability catalog completed with KNNFewShot and LabeledFewShot as possible-dev future first candidates and advanced optimizers future-only/blocked; MiniMax synthetic OpenAI-compatible smoke test returned HTTP 200; raw MiniMax response/model content was removed from persisted artifacts; independent review PASS after evidence-hygiene fixes. Production import, production writes, DSPy optimizer execution, DSPy production runtime adoption, MiniMax orchestration/source-of-truth, and raw paper/PDF/chunk text calls remain blocked.

### R042 — MiniMax advancement must use real bounded API tests, document Token Plan quota/limit visibility, and keep external calls redacted and non-authoritative.
- Class: integration
- Status: validated
- Description: MiniMax advancement must use real bounded API tests, document Token Plan quota/limit visibility, and keep external calls redacted and non-authoritative.
- Why it matters: The project needs MiniMax callability and limit-operability evidence before any helper integration can be safely considered.
- Source: user-request
- Primary owning slice: M014-65dlgp
- Validation: M014 final guard: review_verdict=PASS; subscription_budget_non_blocking=true; platform_limits_still_apply=true; weekly_usage_quota_documented=10x the 5-hour quota; live_call_count=4; successful_http_count=4; redacted_helper_success_count=1; raw_response_persisted=false; raw_model_content_persisted=false; secrets_logged=false; production_import_allowed=false; ladybugdb_written=false; minimax_orchestrator_allowed=false; source_of_truth_allowed=false.
- Notes: Validated by M014: MiniMax Token Plan usage visibility documented from official docs; subscription budget recorded as non-blocking per user instruction; platform limits still apply, including request windows, RPM/TPM, daily quotas, dynamic peak-hour traffic guidance, and weekly usage quota of 10x 5-hour quota where applicable. Remains endpoint was probed safely but returned HTTP 403 with current key, likely not authorized/not Token Plan key; no raw body or credential persisted. Four real MiniMax live calls completed over synthetic/redacted metadata only; all HTTP 200; strict JSON passed; one redacted helper attempt truncated, high-budget retry parsed; edge failure recorded. Production import/write/source-of-truth/orchestrator/unattended use remain blocked.

### R043 — MiniMax remediation must prove Token Plan limit-check access and structured JSON behavior using the correct API surfaces before any helper verdict is accepted.
- Class: integration
- Status: validated
- Description: MiniMax remediation must prove Token Plan limit-check access and structured JSON behavior using the correct API surfaces before any helper verdict is accepted.
- Why it matters: A single remains 403 and prompt-only JSON parse failures are insufficient evidence; incorrect API surface or key type can produce false negative conclusions.
- Source: user-correction
- Primary owning slice: M015-ktorc7
- Validation: M015 final guard: review_verdict=PASS; structured_output_verdict=tool_call_recommended; recommended_structured_interfaces=[anthropic_forced_tool_call,openai_response_format_json_schema,openai_response_format_json_object]; anthropic_forced_tool_schema_validated=true; schema_validated_count=1; token_plan_limit_check_verdict=ui_only_or_session_required; token_plan_api_remains_verified=false; true_remains_success_count=0; raw_response_persisted=false; secrets_logged=false; production_import_allowed=false; source_of_truth_allowed=false.
- Notes: Validated by M015 remediation. Corrected M014: MiniMax structured JSON should not be judged from prompt-only OpenAI JSON. Anthropic-compatible forced tool calls with input_schema succeeded and schema-validated; OpenAI response_format json_schema/json_object also parsed in the live matrix. Corrected Token Plan limits: API remains access was tested through a 32-row matrix; no true remains success occurred because available MINIMAX_TOKEN_PLAN_KEY matched MINIMAX_API_KEY and base_resp-only HTTP 200 responses had non-zero status codes. Reliable current limit check is Billing > Token Plan UI; programmatic remains requires a distinct authorized Token Plan Key or session-supported endpoint. Production import/write/source-of-truth/orchestration remain blocked.

### R044 — MiniMax limit checking must follow the 9router endpoint/fallback/parsing algorithm before declaring Token Plan remains unresolved.
- Class: integration
- Status: validated
- Description: MiniMax limit checking must follow the 9router endpoint/fallback/parsing algorithm before declaring Token Plan remains unresolved.
- Why it matters: Previous M015 matrix missed at least one 9router global fallback endpoint and did not derive the full parsing semantics from known working implementation.
- Source: user-correction-9router
- Primary owning slice: M016-9819d1
- Validation: M016 final guard: limit_check_verdict=api_remains_verified; used_9router_algorithm=true; m015_limit_verdict_overturned=true; working_endpoint=https://api.minimax.io/v1/api/openplatform/coding_plan/remains; count_means_remaining=true; true_success_count=1; quota_row_count_total=8; raw_response_persisted=false; exact_quota_values_persisted=false; credential_values_logged=false.
- Notes: Validated by M016. 9router was cloned to /root/vendor-source/9router and indexed as GitNexus repo `9router`. Its MiniMax usage implementation showed the correct global fallback endpoint `https://api.minimax.io/v1/api/openplatform/coding_plan/remains`, GET with Authorization Bearer, and parser rules requiring base_resp.status_code=0 plus model_remains quota rows. The corrected live probe verified API remains for global MiniMax via that fallback: true_success_count=1, model_remains_count=11, quota_row_count_total=8. Raw response, exact quota values, and credential values were not persisted. M015 limit verdict is overturned for global MiniMax; CN endpoints remain unverified with the current global key.

### R045 — MiniMax helper implementation must turn the verified MiniMax findings into dev-only, tested code before MiniMax is used in any Scientific KG workflow.
- Class: integration
- Status: validated
- Description: MiniMax helper implementation must turn the verified MiniMax findings into dev-only, tested code before MiniMax is used in any Scientific KG workflow.
- Why it matters: The MiniMax endpoint/auth/thinking/limit behavior is now proven and documented, but still needs a reusable project implementation to prevent future agents from reintroducing M014/M015 errors.
- Source: post-M016 planning
- Primary owning slice: M017-cf3fd0
- Validation: M017 final guard: tests_passed=9; ruff_passed=true; reviewer_verdict=PASS; security_final_verdict=PASS_WITH_NOTED_DEPENDENCY_DEBT; raw_response_persisted=false; exact_quota_values_persisted=false; credential_values_logged=false; raw_corpus_payload_allowed=false; raw_model_content_persisted=false; production_import_allowed=false; ladybugdb_write_allowed=false; minimax_source_of_truth=false; live_call_performed=false.
- Notes: Validated by M017. S01 attempted requested Manus/Jina research and recorded it as not currently extractable, so implementation proceeded from global minimax-safe-helper, official docs, M015, and M016. S02 implemented `arxiv_archive.minimax_usage` with canonical key alias resolution, 9router endpoint order, provider status/model_remains checks, token_plan/coding_plan count semantics, and sanitized diagnostics. S03 implemented `arxiv_archive.minimax_structured` with Anthropic-compatible forced tool request building, local schema validation, prompt-only JSON rejection, raw corpus blocking, temperature fail-closed behavior, and non-authoritative helper results. S04 review found and remediated repr leakage/raw-corpus marker risks; dependency audit debt was noted as broader out-of-scope project debt. Final guard preserves production_import_allowed=false, ladybugdb_write_allowed=false, minimax_source_of_truth=false, and raw/secret persistence blocks.

### R046 — Triage ML dependency vulnerabilities before enabling any runtime path that uses vulnerable ML packages.
- Class: compliance/security
- Status: validated
- Description: Triage ML dependency vulnerabilities before enabling any runtime path that uses vulnerable ML packages.
- Why it matters: Known vulnerable ML dependencies can become exploitable if used on untrusted PDFs/images/model artifacts or unattended workflows; they must be assessed separately from MiniMax helper safety.
- Source: M017 security review
- Primary owning slice: M018-gyff0h
- Validation: M018 final guard: vulnerable_dependency_count=2; total_vulnerability_count=19; direct_torch_imports_in_project_source=0; direct_transformers_imports_in_project_source=0; source_acquisition_helper_exposure_found=true; active_cli_exposure_found=false; immediate_hotfix_required=false; broad_dependency_upgrade_now=false; next_milestone=Docling fallback safety gate; independent_security_review=PASS; dependencies_changed=false; raw_audit_json_persisted=false; secrets_logged=false; raw_corpus_payload_logged=false.
- Notes: Validated by M018. S01 found vulnerable transitive ML packages via docling: torch 2.12.0 (11 findings) and transformers 5.8.1 (8 findings), with no direct project dependency declaration and no pip-audit fix versions. S02 found zero direct torch/transformers imports in project source and one lazy Docling import in `MDConverter._try_docling`, reachable from source-acquisition helpers when arxiv2md fails/low-quality and Marker is unavailable. S03 independent security review agreed: no immediate main-CLI hotfix is required, but Docling fallback must be explicitly gated/isolated before new broad source-acquisition runs on external PDFs. Broad ML-stack upgrade is deferred to a separate milestone after the gate.

### R047 — Compare selected open-source research-agent systems before adopting any research-agent patterns into the Scientific KG workflow.
- Class: core-capability
- Status: validated
- Description: Compare selected open-source research-agent systems before adopting any research-agent patterns into the Scientific KG workflow.
- Why it matters: The user-provided research-agent dialogue identified promising systems, but they need repo-level verification before influencing Scientific KG architecture.
- Source: post-M017 roadmap and user confirmation after M018
- Primary owning slice: M019-221lb7
- Validation: M019 final guard: all four targets source_found=true and profile_complete=true; primary_positive_pattern_source=prismAId; secondary_pattern_source=GPT Researcher; primary_cautionary_examples=[The AI Scientist, AI-Researcher]; next_milestone=KG Candidate Locator and Chunk-Span Provenance Protocol; adopt_external_code_now=false; adopt_new_dependencies_now=false; enable_production_kg_import=false; enable_ladybugdb_writes=false; enable_autonomous_scientist_behavior=false; independent_recommendation_review=PASS.
- Notes: Validated by M019. S01 identified authoritative sources for GPT Researcher, AI-Researcher, The AI Scientist, and Open-and-Sustainable/prismAId, including disambiguating prismAId from Prismer. S02 created evidence-backed profiles for all four systems. S03 synthesized a comparative matrix and independent review PASS. Final recommendation: use prismAId as the primary positive pattern source for protocol-bound review workflows, GPT Researcher as a secondary source for bounded orchestration/source tracking, and AI-Researcher/The AI Scientist as cautionary examples for autonomy non-goals. Next recommended milestone: KG Candidate Locator and Chunk-Span Provenance Protocol. No code/dependency adoption, production KG import, LadybugDB writes, or autonomous scientist behavior was enabled.

### R048 — Define and validate KG candidate locators with chunk-span provenance before any positive Scientific KG import is allowed.
- Class: core-capability
- Status: validated
- Description: Define and validate KG candidate locators with chunk-span provenance before any positive Scientific KG import is allowed.
- Why it matters: M011 blocked semantic import until chunk-level spans and candidate locators exist. M019 recommended a protocol-bound source-ledger/review-gated approach before returning to KG readiness.
- Source: M011 negative semantic gate and M019 comparative spike recommendation
- Primary owning slice: M020-uh5kvt
- Validation: Validated by M020 S01-S04 artifacts: candidate locator protocol/schema/guard, one-paper fixture and guard, 10-paper small-batch rehearsal with 35 locators, final guard m020-s04-final-guard-ok, and independent semantic review. Evidence supports candidate-locator protocol continuation but explicitly defers positive import-gate work.
- Notes: M020 validated the protocol and bounded rehearsal evidence. Independent review returned FLAG for positive import readiness because 27/35 locators were ambiguous, so next work should implement deterministic locator generation plus ambiguity diagnostics. Positive KG import and LadybugDB writes remain blocked.

### R049 — Implement deterministic candidate locator generation with schema validation, source hash checks, coordinate validation, safety guards, and ambiguity diagnostics while keeping KG import disabled.
- Class: core-capability
- Status: validated
- Description: Implement deterministic candidate locator generation with schema validation, source hash checks, coordinate validation, safety guards, and ambiguity diagnostics while keeping KG import disabled.
- Why it matters: M020 validated the locator protocol but independent review flagged 27/35 ambiguous spans and recommended deterministic implementation plus diagnostics before any positive import gate.
- Source: M020-uh5kvt final recommendation and independent semantic review
- Primary owning slice: M021-xcfj4p
- Validation: Validated by src/arxiv_archive/candidate_locators.py, tests/test_candidate_locators.py (12 focused tests), S02 module guard, S03 deterministic 10-paper batch guard, independent review, remediation verification, and final guard m021-final-guard-ok. Final batch: 10 papers, 26 locators, 20 ambiguous spans, 10 overlap diagnostics, 0 import-eligible locators, 0 fact promotions.
- Notes: M021 implemented deterministic candidate locator generation and bounded batch rehearsal. Independent review initially flagged path-dependent span hashes and missing overlap diagnostics; both were remediated before final closeout. Positive KG import and LadybugDB writes remain blocked.

### R053 — External PDF/parser tool evaluation must compare current daily-archive outputs with opendataloader-pdf and GROBID on a bounded local PDF corpus before any parser integration or graph-readiness claim is accepted.
- Class: quality-attribute
- Status: validated
- Description: External PDF/parser tool evaluation must compare current daily-archive outputs with opendataloader-pdf and GROBID on a bounded local PDF corpus before any parser integration or graph-readiness claim is accepted.
- Why it matters: The project needs better scientific article parsing, layout/table extraction, bibliography handling, and evidence traceability, but M031 proved that parser output must remain candidate evidence until reviewed and fail-closed.
- Source: user-directed M032 external parser research plan after M031
- Primary owning slice: M032
- Validation: Validated by completed milestone M033-732r1t: S01 baseline, S02 GROBID study, S03 OpenDataLoader hybrid probe, S04 quant-mind pattern study, S07 Adaptix adapter probe, S05 combined recommendation, and S06 bounded quality plan. Fresh verifiers/tests/Ruff and milestone validation passed; all graph/import/write safety flags remained false.
- Notes: M033 recommends `recommended-bounded-combined-sidecar-architecture` as bounded research output only. No production parser integration, dependency adoption, graph import, LadybugDB write, or import eligibility is authorized.

### R057 — Future sidecar-pipeline roadmap must include explicit architecture brainstorm and decision gates at key irreversible points.
- Class: constraint
- Status: validated
- Description: Future sidecar-pipeline roadmap must include explicit architecture brainstorm and decision gates at key irreversible points.
- Why it matters: Post-M033 work has several high-coupling choices: durable state model, queue/worker model, artifact dependency graph, retry semantics, sidecar lifecycle, review boundary, and later agent-worker boundary. These should be decided deliberately before implementation slices lock in the wrong abstraction.
- Source: Post-M033 architecture discussion
- Primary owning slice: future pipeline orchestration milestone
- Validation: Validated by M034: ROADMAP-GATES.md defines mandatory architecture gates for universal KB scope, GraphDB evaluation, state model, queue semantics, artifact dependency graph, failure taxonomy, sidecar lifecycle, review boundary, graph-readiness handoff, and agent boundary; verify_m034_roadmap_gates.py passes.
- Notes: M034 satisfies the roadmap-gate documentation requirement. Future implementation still needs to execute the gates.

### R058 — Post-M033 ADR package must root every sidecar and orchestration decision in the overall daily-archive mission: local-first scientific paper evidence chains before any Scientific KG or LadybugDB import.
- Class: constraint
- Status: validated
- Description: Post-M033 ADR package must root every sidecar and orchestration decision in the overall daily-archive mission: local-first scientific paper evidence chains before any Scientific KG or LadybugDB import.
- Why it matters: A pipeline-only ADR set can drift into implementation mechanics and forget the project's core purpose: traceable, reviewed, fail-closed progression from article sources to candidate evidence to graph-readiness, not parser adoption for its own sake.
- Source: User correction during M034 planning
- Primary owning slice: M034-kuei9y/S01
- Validation: Validated by M034: ADR-000 Universal KB North Star grounds M034 decisions in the project mission, separates generic universal-KB primitives from scientific-paper first-domain adapters, and final verification passes.
- Notes: Superseded/narrowed paper-only wording by universal-KB-with-scientific-articles-first framing.

### R059 — Do not lock the future knowledge graph database choice to LadybugDB before a dedicated GraphDB evaluation compares viable local-first candidates.
- Class: constraint
- Status: validated
- Description: Do not lock the future knowledge graph database choice to LadybugDB before a dedicated GraphDB evaluation compares viable local-first candidates.
- Why it matters: The project needs to consider license terms, local operation, performance, graph-vector capabilities, Python/tooling ergonomics, persistence model, query language, deployment complexity, and future scalability across LadybugDB, FalkorDB, HelixDB, and other candidates before choosing the durable knowledge substrate.
- Source: User correction during M034 planning
- Primary owning slice: M034-kuei9y/S01
- Validation: Validated by M034: ADR-002 Defer Final GraphDB Selection explicitly keeps LadybugDB/FalkorDB/HelixDB/other selection open and requires future comparison before any final substrate choice; final package verifier passes.
- Notes: This validates non-lock-in, not GraphDB evaluation itself. A future milestone must perform the actual comparison.

### R060 — Frame the architecture around a universal local-first knowledge base, with scientific articles as the primary current domain and proving ground.
- Class: core-capability
- Status: validated
- Description: Frame the architecture around a universal local-first knowledge base, with scientific articles as the primary current domain and proving ground.
- Why it matters: Parser, provenance, evidence, review, and knowledge-card/tree contracts should not be overfit to arXiv papers if the intended direction is a broader knowledge base. Scientific articles remain the main near-term focus, but architecture should separate generic knowledge-ingestion primitives from paper-specific adapters and quality gates.
- Source: User correction during M034 planning
- Primary owning slice: M034-kuei9y/S01
- Validation: Validated by M034: ADR-000, PRD.md, FUNCTIONAL-REQUIREMENTS.md, NON-FUNCTIONAL-REQUIREMENTS.md, and CONTRACTS.md frame the architecture as a local-first universal KB with scientific articles as primary first domain; final verifier passes.
- Notes: Scientific-article domain remains the first proving ground; future non-paper domains remain open questions.

### R061 — M034 must audit all existing GSD requirements and decisions for mutual consistency with the universal knowledge-base ADR package before closeout.
- Class: quality-attribute
- Status: validated
- Description: M034 must audit all existing GSD requirements and decisions for mutual consistency with the universal knowledge-base ADR package before closeout.
- Why it matters: The project has accumulated many requirements and decisions across scientific KG, parser quality, MiniMax/DSPy, validation batches, LadybugDB, and sidecar pipeline work. A new ADR package can only prevent drift if it reconciles existing Rxxx and Dxxx records, identifies contradictions or superseded assumptions, and routes corrections through explicit discussion or new superseding decisions rather than silently rewriting history.
- Source: User correction during M034 planning
- Primary owning slice: M034-kuei9y/S06
- Validation: Validated by M034 S01 and final package verification: all 61 requirements and 67 decisions were inventoried and classified across 128 records; 15 needs-clarification routes were captured; verify_m034_rd_consistency_audit.py and verify_m034_decision_package.py pass.
- Notes: No immediate blocking conflict-needs-user-decision records remained after false-positive refinement; future ADRs/implementation must continue honoring the route plan.

### R062 — Governance memory must provide a fast codebase-memory ADR/R/D mirror generated from canonical GSD and ADR artifacts without becoming the source of truth.
- Class: operability
- Status: validated
- Description: Governance memory must provide a fast codebase-memory ADR/R/D mirror generated from canonical GSD and ADR artifacts without becoming the source of truth.
- Why it matters: Agents need quick recall of requirements, decisions, ADR relationships, and safety constraints, but stale memory must not override GSD registers or documented ADRs.
- Source: M038 planning
- Primary owning slice: M038-hdx112
- Supporting slices: M034,M035,M036,M037
- Validation: M038 generated `.codebase-memory/adr.md` from canonical GSD/ADR artifacts via `scripts/sync_codebase_memory_governance.py`; tests verify D075/R062/ADR-005 markers, source-of-truth warning, stale check behavior, and secret/payload rejection. MCP readback via `codebase-memory-mcp cli manage_adr '{"project":"root-daily-archive","action":"list"}'` contains D075, R062, ADR-005, GSD canonical warning, and codebase-memory MCP mirror wording.
- Notes: Validated as a non-canonical fast recall mirror only. GSD remains canonical for requirements/decisions; ADR docs remain canonical for architecture; GitNexus remains mandatory for code-impact and change-scope safety.

### R063 — Governance memory must expose a typed ADR/R/D graph projection with verifiable nodes and edges while preserving GSD and ADR files as canonical.
- Class: operability
- Status: validated
- Description: Governance memory must expose a typed ADR/R/D graph projection with verifiable nodes and edges while preserving GSD and ADR files as canonical.
- Why it matters: Agents need graph-shaped navigation across requirements, decisions, ADRs, milestones, safety constraints, and validation evidence without losing source-of-truth boundaries or relying on unsupported MCP graph writes.
- Source: M039 planning
- Primary owning slice: M039-7o4yf1
- Supporting slices: M038-hdx112
- Validation: M039 generated `.codebase-memory/governance-graph.json` from canonical GSD/ADR artifacts with typed nodes and edges. Verifier evidence: sync/check passed for markdown and graph artifacts, 10 graph tests passed, ruff passed, JSON parsed, required D075/D076/R062/R063/ADR-005/M038/M039 nodes and D076/D075/R063/ADR-005 edges were asserted, codebase-memory MCP readback/search found graph projection markers after fast index refresh, and `ingest_traces` confirmed runtime edge creation is not implemented so native custom graph claims are avoided.
- Notes: Validated as artifact-first typed projection only. GSD and ADR docs remain canonical; codebase-memory custom edge ingestion remains future work pending MCP support.

### R064 — The real-corpus no-write smoke must support a mixed 20-30 article batch containing retained baseline articles, fresh articles, and reference-linked candidates when safely discoverable.
- Class: core-capability
- Status: validated
- Description: The real-corpus no-write smoke must support a mixed 20-30 article batch containing retained baseline articles, fresh articles, and reference-linked candidates when safely discoverable.
- Why it matters: The next validation stage needs to test both scale and early connectivity while preserving no-write/no-import safety boundaries.
- Source: M041 planning
- Primary owning slice: M041-8k3kv4
- Supporting slices: M040-4flhk6
- Validation: M041 generated and ran a mixed 20-article no-write smoke: 10 retained baseline articles, 5 articles linked from already loaded local sources, and 5 Hermes review-section articles. Evidence: M041 manifest category counts, M041 run summary with 20 completed handoffs, M041 audit with 20 continuity artifacts and empty blockers, all graph/import/promotion flags false, and README/report documenting arXiv deferred metadata caveat.
- Notes: Validated for no-write connectivity smoke only. It does not authorize graph import, fact promotion, production writes, or treating deferred linked metadata as graph-ready evidence.

### R065 — Provide a unified project trajectory check that summarizes architecture decisions, requirements, module/code movement, evidence maturity, safety boundaries, and next blockers so agents can detect drift before planning or execution.
- Class: operability
- Status: validated
- Description: Provide a unified project trajectory check that summarizes architecture decisions, requirements, module/code movement, evidence maturity, safety boundaries, and next blockers so agents can detect drift before planning or execution.
- Why it matters: The project has many local verifiers and decisions, but agents can still lose the overall trajectory across compressed sessions. A single trajectory check should optimize existing controls rather than proliferating narrow guardrails.
- Source: user request after M044
- Validation: M045 implemented `scripts/check_project_trajectory.py`, tests, codebase-memory MCP snapshot support, real JSON/Markdown trajectory reports, README preflight documentation, and D080. The report covers architecture, functionality, module_code, evidence, safety, operations, and next_gate dimensions; flags drift risks; verifies no-write boundaries; and treats codebase-memory as non-canonical recall/navigation evidence.
- Notes: Validated as a derived trajectory preflight, not a replacement for GSD, ADRs, GitNexus, or governance mirrors.

### R066 — Recurring corpus pipeline logic must live behind hexagonal onion application use cases and ports, with milestone scripts reduced to thin CLI or compatibility wrappers.
- Class: quality-attribute
- Status: validated
- Description: Recurring corpus pipeline logic must live behind hexagonal onion application use cases and ports, with milestone scripts reduced to thin CLI or compatibility wrappers.
- Why it matters: M121 proved the pipeline with GSD-era scripts, but long-term maintainability requires business rules to be testable inside package boundaries rather than embedded in milestone scripts.
- Source: M122-qq2mfe planning
- Primary owning slice: M122-qq2mfe
- Validation: Validated by M122-qq2mfe S01-S06. Final acceptance runner generated passing `data/pipeline-script-architecture/acceptance-summary.json`; onion layering guard clean; fresh milestone verification passed 94 tests. Migrated catalog ingest, parser replay, coverage report, and graph probe wrappers delegate through application use cases and infrastructure adapters/writers while preserving M121/R024 artifact compatibility.
- Notes: Quality-metrics remains a first-class category and is documented out of the S06 integrated M121-style acceptance path rather than folded into coverage/reporting.

### R067 — Pipeline must expose typed, resumable, observable jobs for intake, acquisition, parsing, chunking, evidence, and graph-candidate preparation before graph import claims.
- Class: core-capability
- Status: validated
- Description: Pipeline must expose typed, resumable, observable jobs for intake, acquisition, parsing, chunking, evidence, and graph-candidate preparation before graph import claims.
- Why it matters: Architecture crystallization requires the pipeline to be a governed application workflow rather than disconnected scripts before any graph transition.
- Source: M195 planning after D109 and D110
- Primary owning slice: M195
- Validation: M195 final validation: final focused suite passed (98 passed, gsd_exec[315c75c2-2dcf-4d85-99d9-513809a8c276]); final no-write runtime smoke passed (gsd_exec[3886e84d-75e6-4489-86ee-e6492799d327]); S12 proved queue-to-schema-to-projection no-write rehearsal artifacts with false write/import flags.
- Notes: Validated for no-write pipeline/projection rehearsal only; production graph import remains out of scope.

### R068 — Pipeline external dependency failures must be typed and fail closed for network, arXiv availability, resource limits, LLM limits, stale hashes, and partial artifacts.
- Class: failure-visibility
- Status: validated
- Description: Pipeline external dependency failures must be typed and fail closed for network, arXiv availability, resource limits, LLM limits, stale hashes, and partial artifacts.
- Why it matters: The system cannot safely prepare graph candidates if upstream failure modes can masquerade as successful ingestion or conversion.
- Source: M195 planning after D109
- Primary owning slice: M195
- Validation: M195 final validation: schema gate tests passed and final suite passed (98 passed, gsd_exec[315c75c2-2dcf-4d85-99d9-513809a8c276]); runtime smoke confirmed schema_gate.diagnostics=[schema_versions_current] and import_eligible=false.
- Notes: Validated as schema governance and placeholder migration readiness only; no migration execution is enabled.

### R069 — Graph readiness work must pass through a graph projection boundary with schema versioning, evidence paths, and lightweight NetworkX rehearsal before any production graph backend write.
- Class: integration
- Status: validated
- Description: Graph readiness work must pass through a graph projection boundary with schema versioning, evidence paths, and lightweight NetworkX rehearsal before any production graph backend write.
- Why it matters: NetworkX, LadybugDB, and FalkorDB should be interchangeable downstream adapters without coupling domain/application code to backend-specific infrastructure.
- Source: M195 planning after D110
- Primary owning slice: M195
- Validation: M195 final validation: projection port, NetworkX adapter, disabled backend seams, governance ratchets, and no-write rehearsal passed in final suite (98 passed, gsd_exec[315c75c2-2dcf-4d85-99d9-513809a8c276]); runtime smoke confirmed projection_backend=networkx and import_eligible=false.
- Notes: Validated as no-write graph projection boundary only; LadybugDB/FalkorDB writes remain disabled and unvalidated.

### R070 — Pipeline production hardening must provide staged validation evidence before any production promotion.
- Class: operability
- Status: validated
- Description: Pipeline production hardening must provide staged validation evidence before any production promotion.
- Why it matters: M195 proved no-write projection rehearsal, but production hardening needs repeatable staged runs, acceptance thresholds, and rollback-friendly evidence before broader use.
- Source: M196 planning after M195
- Primary owning slice: M196-0nrede
- Validation: M196 final validation: staged validation contract tests passed and final focused suite passed with 111 passed (gsd_exec[c0d190c3-387c-4f58-a928-04a4dabc6cb4]); final runtime smoke passed (gsd_exec[7f187fc6-091e-46dc-b851-d15cb05a1bfb]).
- Notes: Validated for bounded staged production hardening contract and repeated local verification only; production graph import remains out of scope.

### R071 — Pipeline hardening must persist operator-readable run state, failure diagnostics, retry counts, and artifact lineage.
- Class: failure-visibility
- Status: validated
- Description: Pipeline hardening must persist operator-readable run state, failure diagnostics, retry counts, and artifact lineage.
- Why it matters: Future agents need enough state to diagnose failed or partial pipeline runs without re-running blindly or inspecting raw payloads.
- Source: M196 planning after M195
- Primary owning slice: M196-0nrede
- Validation: M196 final validation: queue resilience tests, run artifact observability tests, and final runtime smoke passed; artifacts expose queue status/events, schema gate diagnostics, projection diagnostics, summary linkage, and false write/import flags without checked payload leakage.
- Notes: Validated for metadata-only operator diagnostics and artifact lineage in no-write rehearsal/hardening context.

### R072 — Pipeline production hardening must not enable graph backend writes, migration execution, or import eligibility.
- Class: constraint
- Status: validated
- Description: Pipeline production hardening must not enable graph backend writes, migration execution, or import eligibility.
- Why it matters: M195 intentionally kept graph import blocked; hardening should improve pipeline reliability without bypassing graph readiness gates.
- Source: M196 planning after M195
- Primary owning slice: M196-0nrede
- Validation: M196 final validation: M196/M195 governance ratchets passed in final suite (111 passed); final runtime smoke confirmed import_eligible=false and no graph backend write path was enabled.
- Notes: Validated as a no-write/no-import constraint; LadybugDB/FalkorDB writes, migration execution, and import_eligible=true remain blocked.

### R073 — Pipeline orchestration should support bounded asynchronous execution for independent I/O-bound stages while preserving deterministic sync domain contracts.
- Class: quality-attribute
- Status: validated
- Description: Pipeline orchestration should support bounded asynchronous execution for independent I/O-bound stages while preserving deterministic sync domain contracts.
- Why it matters: Async should improve throughput and responsiveness without making extraction, validation, schema, and no-write contracts nondeterministic or harder to test.
- Source: Post-M196 reactive architecture inventory
- Primary owning slice: Future async/reactive pipeline milestone
- Validation: Validated by M197 reactive runner, dry-run script, realistic no-write rehearsal, and final compatibility sweep. Evidence: data/architecture-assessment/m197-requirement-outcomes.md and data/architecture-assessment/m197-s14-final-compatibility-evidence.md.
- Notes: M197 validates additive async/reactive pilot behavior only; production graph import remains out of scope.

### R074 — Reactive pipeline jobs must expose observable state transitions, correlation IDs, retries, cancellation, timeout, and artifact lineage.
- Class: operability
- Status: validated
- Description: Reactive pipeline jobs must expose observable state transitions, correlation IDs, retries, cancellation, timeout, and artifact lineage.
- Why it matters: Async systems fail by hanging, racing, or partially completing; operators and agents need durable state to diagnose work without inspecting raw payloads.
- Source: Post-M196 reactive architecture inventory
- Primary owning slice: Future async/reactive pipeline milestone
- Validation: Validated by M197 reactive event contract, lifecycle/failure/retry/lineage metadata, operator handoff, governance ratchets, and final compatibility sweep. Evidence: data/architecture-assessment/m197-requirement-outcomes.md.
- Notes: Event surface remains contract-first and metadata-only.

### R075 — Async/reactive adoption must preserve fail-closed graph/import governance boundaries.
- Class: constraint
- Status: validated
- Description: Async/reactive adoption must preserve fail-closed graph/import governance boundaries.
- Why it matters: Concurrency must not accidentally bypass staged validation, write gates, schema migration guards, or import eligibility controls.
- Source: Post-M196 reactive architecture inventory
- Primary owning slice: Future async/reactive pipeline milestone
- Validation: Validated by no-write/import-blocked reactive event flags, queue compatibility tests, governance ratchets, final safety audit, and M195/M196 compatibility. Evidence: data/architecture-assessment/m197-requirement-outcomes.md and data/architecture-assessment/m197-s14-final-safety-audit.md.
- Notes: No production graph import, schema migration, queue dependency semantic change, or smoke/rehearsal semantic change was enabled.

### R076 — Reactive no-write pilot must gain production-readiness preconditions that compare dry-run, sync rehearsal, smoke, and graph readiness evidence without enabling production writes.
- Class: core-capability
- Status: validated
- Description: Reactive no-write pilot must gain production-readiness preconditions that compare dry-run, sync rehearsal, smoke, and graph readiness evidence without enabling production writes.
- Why it matters: M197 proved a safe reactive pilot, but future production orchestration needs objective readiness gates before any write/import transition.
- Source: M198-t5wlml planning
- Primary owning slice: M198-t5wlml
- Validation: Validated by M198 final verification: readiness evidence/index/diagnostics/report/rehearsal/package chain passed final suite with 82 tests, Ruff, Pyrefly, and GitNexus detect_changes LOW. Evidence: data/architecture-assessment/m198-final-validation-evidence.md and m198-requirement-outcomes.md.
- Notes: M198 produced metadata-only readiness precondition evidence; production import remains out of scope.

### R077 — Reactive readiness workflows must expose operator-readable failure diagnostics, drift deltas, and evidence indexes for dry-run versus sync/smoke parity.
- Class: failure-visibility
- Status: validated
- Description: Reactive readiness workflows must expose operator-readable failure diagnostics, drift deltas, and evidence indexes for dry-run versus sync/smoke parity.
- Why it matters: Future agents need clear failure surfaces before promoting reactive orchestration beyond no-write mode.
- Source: M198-t5wlml planning
- Primary owning slice: M198-t5wlml
- Validation: Validated by M198 drift classifier, operator diagnostics, readiness report, rehearsal command log, smoke parity audit, disabled backend safety audit, validation package, and operator runbook. Final suite passed with 82 tests and package-level failure visibility. Evidence: data/architecture-assessment/m198-final-validation-evidence.md.
- Notes: Failure visibility is operator/package-level; remediation ownership remains for future milestones.

### R078 — No production graph import, schema migration, queue dependency semantic change, or smoke/rehearsal semantic change may be enabled by M198 readiness work.
- Class: compliance/security
- Status: validated
- Description: No production graph import, schema migration, queue dependency semantic change, or smoke/rehearsal semantic change may be enabled by M198 readiness work.
- Why it matters: Queue dependency impact is HIGH and M197 explicitly validated only a no-write reactive pilot.
- Source: M198-t5wlml planning
- Primary owning slice: M198-t5wlml
- Validation: Validated by no-write/import-blocked contract, S11 governance ratchets, S12 GitNexus impact gates, S13-S16 boundary confirmations, M195-M197 governance ratchets, and final GitNexus detect_changes LOW. Evidence: data/architecture-assessment/m198-final-validation-evidence.md and m198-requirement-outcomes.md.
- Notes: No production graph import, schema migration, queue semantic change, smoke/rehearsal semantic change, retired shim restoration, or import eligibility promotion was enabled.

## Deferred

## Out of Scope

## Traceability

| ID | Class | Status | Primary owner | Supporting | Proof |
|---|---|---|---|---|---|
| R001 | core-capability | validated | none | none | S01 implemented a Typer app exposing project purpose, artifact paths, exit codes, and non-goals in help output; S05 verified help contracts via subprocess tests. |
| R002 | core-capability | validated | none | none | S02 wired explicit-date CLI analysis to ArxivClient and normalized DailyAnalysis output. |
| R003 | primary-user-loop | validated | none | none | S03 implemented and verified Hermes-readable session JSON under `~/research/ops/sessions/YYYY-MM-DD.json`. |
| R004 | core-capability | validated | none | none | S03 daily artifacts save full raw and scored paper lists instead of only top-N outputs. |
| R005 | primary-user-loop | validated | none | none | S04 added idempotent per-paper raw and scored JSON artifacts under `~/research/papers/{arxiv-id}/`. |
| R006 | primary-user-loop | validated | none | none | S04 populated overview category counts, keyword counts, top papers, and deterministic aggregates. |
| R007 | quality-attribute | validated | none | none | S04 includes detailed score breakdown statistics in per-paper scored JSON and daily overview JSON. |
| R008 | operability | validated | none | none | S05 implemented queue state persistence in `~/research/ops/queue/YYYY-MM-DD.json` across running, done, empty, and failed states. |
| R009 | operability | validated | none | none | S03, S04, and S05 verified same-date rerun overwrites are deterministic and do not duplicate artifacts. |
| R010 | integration | validated | none | none | S01 established portable exit codes; S03 introduced explicit JSON-native serializers with ISO strings and no Python repr/dataclass metadata. |
| R011 | quality-attribute | validated | none | none | S05 confirmed zero Ruff lint findings and a passing full pytest suite for M001. |
| R012 | core-capability | validated | none | none | S02 explicitly returns `empty` status with exit 0 for zero-paper days; S03/S05 verified valid empty JSON outputs. |
| R013 | quality-attribute | validated | none | none | S05 added offline subprocess tests for help output, JSON schemas, empty results, failure states, and rerun idempotency. |
| R014 | core-capability | validated | M003-km5fty/S01 | S02,S03 | Validated by S01: `uv run pytest tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` passed during S01 closeout. |
| R015 | core-capability | validated | M003-km5fty/S02 | S03,S09 | Validated by S02: `uv run pytest tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` passed during S02 closeout. |
| R016 | core-capability | validated | M003-km5fty/S03 | S04,S05,S06,S07,S09 | Validated by S03: `uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` passed with 44 tests. |
| R017 | core-capability | validated | M003-km5fty/S04 | S03,S05,S07,S08 | S04 verification passed: `uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` => 50 passed; Ruff all checks passed; CLI help smoke exit 0; LSP diagnostics clean; Pyrefly 0 errors; Ty all checks passed. |
| R018 | integration | validated | M003-km5fty/S05 | S02,S03,S04,S06,S10 | Validated by M003-km5fty S05 post-review verification: `uv run pytest tests/test_ladybug_scientific_kg.py tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_ladybug_client_property.py tests/test_scientific_kg_e2e.py tests/test_cli_contract.py -q` passed with 42 tests; Ruff passed on touched files; Pyrefly reported 0 errors; Ty passed on `src/` plus S05 test; CLI help smoke exited 0; LSP diagnostics were clean; GitNexus detect_changes was reviewed with expected high scope for the uncommitted S05 persistence expansion. |
| R019 | core-capability | active | M003-km5fty/S06 | S03,S05,S07,S10 | Pending S06 retrieval and ablation tests comparing vector-only, graph expansion, and fused retrieval over fixtures. |
| R020 | quality-attribute | validated | M003-km5fty/S07 | S03,S04,S06,S08,S09,S10 | Validated by M003-km5fty/S07: focused closeout verification passed with `uv run pytest tests/test_evaluation_benchmark.py tests/test_hybrid_retrieval.py tests/test_ladybug_scientific_kg.py tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py -q` => 34 passed; Ruff lint and format checks passed; `ty`, `pyrefly`, and CLI help smoke passed. S07 added typed evaluation contracts and fixture-backed benchmark tests for schema validity, groundedness proxy counts, evidence-path hit rate, retrieval recall, and vector-only/graph-only/hybrid ablation metrics over deterministic local fixtures before DSPy/RLM/optimizer claims. |
| R021 | constraint | validated | M003-km5fty/S08 | S04,S07 | Validated by M003-km5fty/S08 closeout: `uv run python -c "from arxiv_archive.dspy_extraction import BaselineDspyExtractionModule, DspyExtractionInput, DspyExtractionOutput"` exited 0; focused verification passed with `uv run pytest tests/test_dspy_extraction_boundary.py tests/test_evaluation_benchmark.py tests/test_scientific_extraction_contracts.py -q` => 23 passed; Ruff lint and format checks passed; `ty` passed; `pyrefly` reported 0 errors; CLI help smoke exited 0; `gitnexus detect-changes --repo daily-archive` reported no changes detected. S08 added a deterministic DSPy-compatible `forward()` boundary around existing `ExtractionPatch` callables, reusing S04 extraction contracts and S07 schema/groundedness metric gates while keeping optimizer/runtime behavior disabled and diagnostics text-safe. |
| R022 | core-capability | active | M003-km5fty/S09 | S02,S03,S04,S08 | Pending S09 fixture tests or mocked-interpreter tests for bounded tools, typed draft patch output, trajectory capture, and deterministic validation. |
| R023 | differentiator | active | M003-km5fty/S10 | S05,S06,S07,S09 | Pending S10 comparative benchmark with traversal path, tool usage, cost/latency, candidate set, and evidence recall metrics. |
| R024 | quality-attribute | active | M121-kd3kzr/S06 | M003-km5fty/S01-S10 | M121 expanded canonical catalog to 221 article records and validated 219 source-backed records fail-closed. Evidence: data/r024-218-document-corpus-v1/R024-COVERAGE.md; ingest-summary.json (166 M056 PDFs SHA256 verified, index_entries=221); parser-chunking/summary.json (total=221, ok=219, skipped=2, errors=0, chunk_count_total=2576); networkx-probe/summary.json (corpus_size=219, skipped_metadata_only=2, n_nodes=3891, n_edges=10102, citation_relations_count=6212); targeted tests test_catalog_expansion_m121.py 8 pass, test_r024_218_document_parser_chunking.py 12 pass, test_r024_218_document_networkx_probe.py 10 pass, test_r024_218_document_coverage_report.py 10 pass. Production graph readiness is still not claimed. |
| R025 | operability | validated | M004-ubh2pt/full-text-bridge | M004-ubh2pt/S01 | Validated by M004-ubh2pt/S02. S02 implemented and verified Loguru-based structured diagnostics via a narrow ValidationLogger/JSONL event stream, used it during the selected ten-paper full-text bridge, and reran the 10-document structural validation with per-paper outcomes and redacted failure details recorded. Evidence: S02-SUMMARY.md and S02-UAT.md explicitly prove R025. |
| R026 | quality-attribute | validated | M004-ubh2pt/pipeline-debug | M004-ubh2pt/S01,S02 | Validated narrowly as an end-to-end real-data pipeline debug/plumbing gate by M004-ubh2pt/S03 and M004 validation. S03 exercised converted full text through PageIndex, SemanticChunk, EvidencePath, explicitly labeled debug-baseline ExtractionPatch generation, SCI KG persistence plumbing for eligible papers, and retrieval diagnostics, while documenting conversion/extraction blockers and preventing 20-document scaling. |
| R027 | quality-attribute | active | M004 | none | Partial bounded advancement in M022: S01-S05 produced stable source/locator/span IDs, source-hash coverage, route/review/repair diagnostics, reviewer packet artifacts, and final no-import guard with import_allowed=false and semantic_ready_for_kg=false. Full validation still requires a dedicated graph-readiness quality benchmark/acceptance pass before KG validation or scaling resumes. |
| R028 | quality-attribute | validated | M004 | none | M022 S04/S05 produced and verified six bounded reviewer packets plus an independent assessment and final no-import gate. Evidence: reviewer packet verifier reported packets=6, pending_review=6, assessment_verdict=blocked_pending_semantic_acceptance, unsafe_counters_zero=True; final recommendation maps R028 as validated for bounded artifact review. |
| R029 | quality-attribute | active | M005-dlko4z | none | Partial bounded validation in M022: typed reviewer packet handoff/final gate verified stable packet/review/repair/route diagnostics for six targets with final schema m022-final-gate.v1, pending_review=6, importable_count=0, semantic_ready_count=0, raw_text_embedded_count=0, and unsafe_counters_zero=True. This is not a positive import-ready package validation. |
| R030 | continuity | validated | M005-dlko4z/S05 | none | M024 S04 closeout verified metadata-only asset preservation contract and bridge integration with fixture manifests for figures/tables/equation images, fail-closed forbidden-payload validation, stable IDs/provenance/status summaries, and full regression/property suite: `uv run pytest tests/test_article_loader.py tests/test_article_artifacts.py tests/test_source_asset_manifest.py tests/test_article_evidence_bridge.py tests/test_property_article_evidence_bridge.py tests/test_article_page_index.py tests/test_property_article_page_index.py tests/test_article_assets.py tests/test_property_article_assets.py -q` passed 132 tests. |
| R031 | quality-attribute | active | M006-638rza | M005-dlko4z | A 30-paper dry-run report exists with redacted aggregate/per-paper diagnostics, deviation analysis against M005 10-paper evidence, new-pattern taxonomy, and explicit no-go/go recommendations for remediation. |
| R032 | operability | active | future-validation-automation | M006-638rza | A CLI or equivalent command can run batches of +10 papers, persist per-batch manifests/diagnostics/reports, resume after failures, compare each batch against prior baselines, and stop at review gates without production KG writes. |
| R033 | operability | active | M007-opaont | M007/S01,S02,S03,S04 | A local CLI can select the next batch, preflight/acquire sources, run redacted deviation scans, compare route/refusal deltas, flag outliers/contradictions, and persist resumable batch state without raw/chunk text or KG writes. |
| R034 | primary-user-loop | validated | M008-c9zb94 | M008/S01,S02,S03,S04 | M008 evidence: selected_count=10, m006_overlap_count=0, final source_ready=10, quota accepted_ready_count=10, scan paper_count=10, chunk_count=1591, outlier_count=6, import_eligible_chunk_count=0, review verdict FLAG with next-batch gate. |
| R035 | quality-attribute | active | M008-c9zb94 | M008/S03,M008/S04 | Partial validation: top-up pass sample final_accepted_ready_count=target_count and scan_allowed=true; blocked sample remaining_shortage_count=2 and scan_allowed=false. Missing: automatic acquisition/preflight integration for accepted replacements. |
| R036 | failure-visibility | validated | next validation hardening milestone | none | M027-aakeky S06 validated R036-style provenance for replay/gate artifacts via `uv run python scripts/verify_m027_end_to_end_mixed_replay.py && uv run python scripts/verify_m027_provenance_and_riskratchet_gate.py --validate-only && uv run python -m pytest tests/test_m027_provenance_and_riskratchet_gate.py tests/test_riskratchet_gate.py tests/test_m027_end_to_end_mixed_replay.py -q` (gsd_exec 7ac737ec-e48b-45a2-bf86-18fa892e9c51, exit 0, 32 passed). The S06 summary records command, cwd, git commit, input/output artifact hashes, exit code/status, milestone/slice context, self-hash exclusion rationale, and fail-closed safety/riskratchet flags. |
| R037 | core-capability | validated | next reviewed +10 milestone | none | M010 final guard: review_verdict=PASS; selected_count=10; prior_overlap_count=0; quota_ready_count=10; paper_count=10; chunk_count=1477; freshness_verdict=fresh; import_eligible_chunk_count=0; positive_import_blocked=true; production_writes_blocked=true; unattended_scaling_blocked=true. |
| R038 | quality-attribute | validated | M011 | none | M011 final guard: review_verdict=PASS; gate_result=pass_negative_readiness_gate; target_count=10; source_hash_missing_count=0; repair_required_count=7; retrieval_only_count=3; import_candidate_count=0; raw_payload_key_count=0; positive_import_blocked=true; production_writes_blocked=true; chunk_span_provenance_required_next=true. |
| R039 | constraint | validated | M012 | none | M012 final guard: review_verdict=PASS; dspy_verdict=conditional_go_optional_dev_probe_only; minimax_verdict=conditional_go_optional_helper_probe_only; production_import_allowed=false; dspy_optimizer_allowed=false; minimax_orchestrator_allowed=false; next_safe_options=[dspy_optional_dev_dependency_no_lm_probe, minimax_explicit_synthetic_auth_smoke_test, chunk_span_provenance_candidate_locator_packet]. |
| R040 | constraint | active | project | none | Future milestones that introduce infrastructure must include research/probe artifacts, failure-mode analysis, artifact/redaction boundaries, and an explicit go/no-go decision before process activation. |
| R041 | constraint | validated | M013 | none | M013 final guard: review_verdict=PASS; dspy_dependency_verdict=pass_isolated_optional_dev_probe_ready; dspy_install_succeeded=true; dspy_import_succeeded=true; dspy_predict_failed_closed_without_lm=true; dspy_evaluate_static_program_succeeded=true; dspy_possible_dev_optimizers=[KNNFewShot,LabeledFewShot]; dspy_optimizer_execution_allowed=false; minimax_smoke_verdict=pass_synthetic_callability_only; minimax_http_status=200; minimax_orchestrator_allowed=false; production_import_allowed=false. |
| R042 | integration | validated | M014-65dlgp | none | M014 final guard: review_verdict=PASS; subscription_budget_non_blocking=true; platform_limits_still_apply=true; weekly_usage_quota_documented=10x the 5-hour quota; live_call_count=4; successful_http_count=4; redacted_helper_success_count=1; raw_response_persisted=false; raw_model_content_persisted=false; secrets_logged=false; production_import_allowed=false; ladybugdb_written=false; minimax_orchestrator_allowed=false; source_of_truth_allowed=false. |
| R043 | integration | validated | M015-ktorc7 | none | M015 final guard: review_verdict=PASS; structured_output_verdict=tool_call_recommended; recommended_structured_interfaces=[anthropic_forced_tool_call,openai_response_format_json_schema,openai_response_format_json_object]; anthropic_forced_tool_schema_validated=true; schema_validated_count=1; token_plan_limit_check_verdict=ui_only_or_session_required; token_plan_api_remains_verified=false; true_remains_success_count=0; raw_response_persisted=false; secrets_logged=false; production_import_allowed=false; source_of_truth_allowed=false. |
| R044 | integration | validated | M016-9819d1 | none | M016 final guard: limit_check_verdict=api_remains_verified; used_9router_algorithm=true; m015_limit_verdict_overturned=true; working_endpoint=https://api.minimax.io/v1/api/openplatform/coding_plan/remains; count_means_remaining=true; true_success_count=1; quota_row_count_total=8; raw_response_persisted=false; exact_quota_values_persisted=false; credential_values_logged=false. |
| R045 | integration | validated | M017-cf3fd0 | none | M017 final guard: tests_passed=9; ruff_passed=true; reviewer_verdict=PASS; security_final_verdict=PASS_WITH_NOTED_DEPENDENCY_DEBT; raw_response_persisted=false; exact_quota_values_persisted=false; credential_values_logged=false; raw_corpus_payload_allowed=false; raw_model_content_persisted=false; production_import_allowed=false; ladybugdb_write_allowed=false; minimax_source_of_truth=false; live_call_performed=false. |
| R046 | compliance/security | validated | M018-gyff0h | none | M018 final guard: vulnerable_dependency_count=2; total_vulnerability_count=19; direct_torch_imports_in_project_source=0; direct_transformers_imports_in_project_source=0; source_acquisition_helper_exposure_found=true; active_cli_exposure_found=false; immediate_hotfix_required=false; broad_dependency_upgrade_now=false; next_milestone=Docling fallback safety gate; independent_security_review=PASS; dependencies_changed=false; raw_audit_json_persisted=false; secrets_logged=false; raw_corpus_payload_logged=false. |
| R047 | core-capability | validated | M019-221lb7 | none | M019 final guard: all four targets source_found=true and profile_complete=true; primary_positive_pattern_source=prismAId; secondary_pattern_source=GPT Researcher; primary_cautionary_examples=[The AI Scientist, AI-Researcher]; next_milestone=KG Candidate Locator and Chunk-Span Provenance Protocol; adopt_external_code_now=false; adopt_new_dependencies_now=false; enable_production_kg_import=false; enable_ladybugdb_writes=false; enable_autonomous_scientist_behavior=false; independent_recommendation_review=PASS. |
| R048 | core-capability | validated | M020-uh5kvt | none | Validated by M020 S01-S04 artifacts: candidate locator protocol/schema/guard, one-paper fixture and guard, 10-paper small-batch rehearsal with 35 locators, final guard m020-s04-final-guard-ok, and independent semantic review. Evidence supports candidate-locator protocol continuation but explicitly defers positive import-gate work. |
| R049 | core-capability | validated | M021-xcfj4p | none | Validated by src/arxiv_archive/candidate_locators.py, tests/test_candidate_locators.py (12 focused tests), S02 module guard, S03 deterministic 10-paper batch guard, independent review, remediation verification, and final guard m021-final-guard-ok. Final batch: 10 papers, 26 locators, 20 ambiguous spans, 10 overlap diagnostics, 0 import-eligible locators, 0 fact promotions. |
| R050 | core-capability | active | M023-vk5wb2/S02 | M023-vk5wb2/S01,M023-vk5wb2/S04,M023-vk5wb2/S05 | A CLI command can process bounded source manifests or validation batch state, produce per-paper artifact manifests and run summaries with stable IDs, source spans, candidate links, review states, provenance, and explicit kg_import_allowed=false. |
| R051 | integration | active | M023-vk5wb2/S03 | M023-vk5wb2/S01,M023-vk5wb2/S02,M023-vk5wb2/S05 | A bounded MiniMax adapter is wired into the artifact detection CLI behind an explicit flag, validated by tests and fixture runs proving forced tool-call request shape, local schema validation, refusal of unsafe payloads, redacted diagnostics, and no KG import authorization. |
| R052 | quality-attribute | active | M023-vk5wb2/S04 | M023-vk5wb2/S02,M023-vk5wb2/S03,M023-vk5wb2/S05 | A benchmark fixture set and metric report exist for artifact detection precision, recall, span coverage, link correctness, section lineage correctness, raw leakage rate, and review burden; the final gate either blocks or explicitly scopes any DSPy optimizer activation. |
| R053 | quality-attribute | validated | M032 | none | Validated by completed milestone M033-732r1t: S01 baseline, S02 GROBID study, S03 OpenDataLoader hybrid probe, S04 quant-mind pattern study, S07 Adaptix adapter probe, S05 combined recommendation, and S06 bounded quality plan. Fresh verifiers/tests/Ruff and milestone validation passed; all graph/import/write safety flags remained false. |
| R054 | core-capability | active | future pipeline orchestration milestone | none | A future milestone defines and verifies persisted job/artifact state with statuses, input/output hashes, dependency readiness, stale detection, and resume/retry behavior. |
| R055 | failure-visibility | active | future pipeline orchestration milestone | none | A future verifier proves per-job status, attempt count, retry_after, last_error_code, output_paths, backend/cache health, and dead-letter/terminal blocker states are persisted and queryable. |
| R056 | constraint | active | future pipeline orchestration milestone | none | All future sidecar pipeline artifacts keep graph_import_allowed=false, ladybugdb_written=false, production_import_attempted=false, and import_eligible=false until a separately authorized graph-readiness/import milestone changes those flags with evidence. |
| R057 | constraint | validated | future pipeline orchestration milestone | none | Validated by M034: ROADMAP-GATES.md defines mandatory architecture gates for universal KB scope, GraphDB evaluation, state model, queue semantics, artifact dependency graph, failure taxonomy, sidecar lifecycle, review boundary, graph-readiness handoff, and agent boundary; verify_m034_roadmap_gates.py passes. |
| R058 | constraint | validated | M034-kuei9y/S01 | none | Validated by M034: ADR-000 Universal KB North Star grounds M034 decisions in the project mission, separates generic universal-KB primitives from scientific-paper first-domain adapters, and final verification passes. |
| R059 | constraint | validated | M034-kuei9y/S01 | none | Validated by M034: ADR-002 Defer Final GraphDB Selection explicitly keeps LadybugDB/FalkorDB/HelixDB/other selection open and requires future comparison before any final substrate choice; final package verifier passes. |
| R060 | core-capability | validated | M034-kuei9y/S01 | none | Validated by M034: ADR-000, PRD.md, FUNCTIONAL-REQUIREMENTS.md, NON-FUNCTIONAL-REQUIREMENTS.md, and CONTRACTS.md frame the architecture as a local-first universal KB with scientific articles as primary first domain; final verifier passes. |
| R061 | quality-attribute | validated | M034-kuei9y/S06 | none | Validated by M034 S01 and final package verification: all 61 requirements and 67 decisions were inventoried and classified across 128 records; 15 needs-clarification routes were captured; verify_m034_rd_consistency_audit.py and verify_m034_decision_package.py pass. |
| R062 | operability | validated | M038-hdx112 | M034,M035,M036,M037 | M038 generated `.codebase-memory/adr.md` from canonical GSD/ADR artifacts via `scripts/sync_codebase_memory_governance.py`; tests verify D075/R062/ADR-005 markers, source-of-truth warning, stale check behavior, and secret/payload rejection. MCP readback via `codebase-memory-mcp cli manage_adr '{"project":"root-daily-archive","action":"list"}'` contains D075, R062, ADR-005, GSD canonical warning, and codebase-memory MCP mirror wording. |
| R063 | operability | validated | M039-7o4yf1 | M038-hdx112 | M039 generated `.codebase-memory/governance-graph.json` from canonical GSD/ADR artifacts with typed nodes and edges. Verifier evidence: sync/check passed for markdown and graph artifacts, 10 graph tests passed, ruff passed, JSON parsed, required D075/D076/R062/R063/ADR-005/M038/M039 nodes and D076/D075/R063/ADR-005 edges were asserted, codebase-memory MCP readback/search found graph projection markers after fast index refresh, and `ingest_traces` confirmed runtime edge creation is not implemented so native custom graph claims are avoided. |
| R064 | core-capability | validated | M041-8k3kv4 | M040-4flhk6 | M041 generated and ran a mixed 20-article no-write smoke: 10 retained baseline articles, 5 articles linked from already loaded local sources, and 5 Hermes review-section articles. Evidence: M041 manifest category counts, M041 run summary with 20 completed handoffs, M041 audit with 20 continuity artifacts and empty blockers, all graph/import/promotion flags false, and README/report documenting arXiv deferred metadata caveat. |
| R065 | operability | validated | none | none | M045 implemented `scripts/check_project_trajectory.py`, tests, codebase-memory MCP snapshot support, real JSON/Markdown trajectory reports, README preflight documentation, and D080. The report covers architecture, functionality, module_code, evidence, safety, operations, and next_gate dimensions; flags drift risks; verifies no-write boundaries; and treats codebase-memory as non-canonical recall/navigation evidence. |
| R066 | quality-attribute | validated | M122-qq2mfe | none | Validated by M122-qq2mfe S01-S06. Final acceptance runner generated passing `data/pipeline-script-architecture/acceptance-summary.json`; onion layering guard clean; fresh milestone verification passed 94 tests. Migrated catalog ingest, parser replay, coverage report, and graph probe wrappers delegate through application use cases and infrastructure adapters/writers while preserving M121/R024 artifact compatibility. |
| R067 | core-capability | validated | M195 | none | M195 final validation: final focused suite passed (98 passed, gsd_exec[315c75c2-2dcf-4d85-99d9-513809a8c276]); final no-write runtime smoke passed (gsd_exec[3886e84d-75e6-4489-86ee-e6492799d327]); S12 proved queue-to-schema-to-projection no-write rehearsal artifacts with false write/import flags. |
| R068 | failure-visibility | validated | M195 | none | M195 final validation: schema gate tests passed and final suite passed (98 passed, gsd_exec[315c75c2-2dcf-4d85-99d9-513809a8c276]); runtime smoke confirmed schema_gate.diagnostics=[schema_versions_current] and import_eligible=false. |
| R069 | integration | validated | M195 | none | M195 final validation: projection port, NetworkX adapter, disabled backend seams, governance ratchets, and no-write rehearsal passed in final suite (98 passed, gsd_exec[315c75c2-2dcf-4d85-99d9-513809a8c276]); runtime smoke confirmed projection_backend=networkx and import_eligible=false. |
| R070 | operability | validated | M196-0nrede | none | M196 final validation: staged validation contract tests passed and final focused suite passed with 111 passed (gsd_exec[c0d190c3-387c-4f58-a928-04a4dabc6cb4]); final runtime smoke passed (gsd_exec[7f187fc6-091e-46dc-b851-d15cb05a1bfb]). |
| R071 | failure-visibility | validated | M196-0nrede | none | M196 final validation: queue resilience tests, run artifact observability tests, and final runtime smoke passed; artifacts expose queue status/events, schema gate diagnostics, projection diagnostics, summary linkage, and false write/import flags without checked payload leakage. |
| R072 | constraint | validated | M196-0nrede | none | M196 final validation: M196/M195 governance ratchets passed in final suite (111 passed); final runtime smoke confirmed import_eligible=false and no graph backend write path was enabled. |
| R073 | quality-attribute | validated | Future async/reactive pipeline milestone | none | Validated by M197 reactive runner, dry-run script, realistic no-write rehearsal, and final compatibility sweep. Evidence: data/architecture-assessment/m197-requirement-outcomes.md and data/architecture-assessment/m197-s14-final-compatibility-evidence.md. |
| R074 | operability | validated | Future async/reactive pipeline milestone | none | Validated by M197 reactive event contract, lifecycle/failure/retry/lineage metadata, operator handoff, governance ratchets, and final compatibility sweep. Evidence: data/architecture-assessment/m197-requirement-outcomes.md. |
| R075 | constraint | validated | Future async/reactive pipeline milestone | none | Validated by no-write/import-blocked reactive event flags, queue compatibility tests, governance ratchets, final safety audit, and M195/M196 compatibility. Evidence: data/architecture-assessment/m197-requirement-outcomes.md and data/architecture-assessment/m197-s14-final-safety-audit.md. |
| R076 | core-capability | validated | M198-t5wlml | none | Validated by M198 final verification: readiness evidence/index/diagnostics/report/rehearsal/package chain passed final suite with 82 tests, Ruff, Pyrefly, and GitNexus detect_changes LOW. Evidence: data/architecture-assessment/m198-final-validation-evidence.md and m198-requirement-outcomes.md. |
| R077 | failure-visibility | validated | M198-t5wlml | none | Validated by M198 drift classifier, operator diagnostics, readiness report, rehearsal command log, smoke parity audit, disabled backend safety audit, validation package, and operator runbook. Final suite passed with 82 tests and package-level failure visibility. Evidence: data/architecture-assessment/m198-final-validation-evidence.md. |
| R078 | compliance/security | validated | M198-t5wlml | none | Validated by no-write/import-blocked contract, S11 governance ratchets, S12 GitNexus impact gates, S13-S16 boundary confirmations, M195-M197 governance ratchets, and final GitNexus detect_changes LOW. Evidence: data/architecture-assessment/m198-final-validation-evidence.md and m198-requirement-outcomes.md. |
| R079 | quality-attribute | active | M199-4rex3i | S01, S02, S03, S04 | Each service module raises a typed error on exhausted retries carrying error code, service name, redacted message, retry count; integration tests with a mock transport verify typed diagnostics for 429/5xx/timeout. |
| R080 | quality-attribute | active | M199-4rex3i | S01, S03 | Integration tests verify the retry schedule and Retry-After honour for each retried service (ArxivClient, Markdown Converter arxiv2md). No service silently skips transient failures. |
| R081 | operability | active | M199-4rex3i | S02, S04 | Live test demonstrates that TEI outage produces an explicit degraded signal; downstream embedding-stamping path refuses or annotates; no unmarked-good zero vectors reach artifacts. |
| R082 | constraint | active | M199-4rex3i | S03 | Contract test confirms a retrospective run without explicit run_date is corrected or rejected rather than producing uniform recency=0.5. |
| R083 | compliance/security | active | M199-4rex3i | S01, S02, S03, S04 | Review of diagnostics surfaces confirms no raw payload or secret leakage; redaction verified via contract test. |

## Coverage Summary

- Active requirements: 22
- Mapped to slices: 7
- Validated: 61 (R001, R002, R003, R004, R005, R006, R007, R008, R009, R010, R011, R012, R013, R014, R015, R016, R017, R018, R020, R021, R025, R026, R028, R030, R034, R036, R037, R038, R039, R041, R042, R043, R044, R045, R046, R047, R048, R049, R053, R057, R058, R059, R060, R061, R062, R063, R064, R065, R066, R067, R068, R069, R070, R071, R072, R073, R074, R075, R076, R077, R078)
- Unmapped active requirements: 5
