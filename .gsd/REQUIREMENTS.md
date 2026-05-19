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
- Primary owning slice: future-validation-milestone
- Supporting slices: M003-km5fty/S01-S10
- Validation: A validation report exists for the 10-document batch and 20-document batch before any one-week full run; each report summarizes graph node/edge counts, claim/entity/relation quality, evidence-path coverage, retrieval behavior, diagnostics/failures, and go/no-go recommendations.

### R025 — Full-text acquisition and real-corpus KG validation must emit structured Loguru-based logs and persisted diagnostics for each selected paper before rerunning the 10-document validation.
- Class: operability
- Status: active
- Description: Full-text acquisition and real-corpus KG validation must emit structured Loguru-based logs and persisted diagnostics for each selected paper before rerunning the 10-document validation.
- Why it matters: The first real-data validation stopped at missing full-text inputs. Before adding the full-text bridge, future agents need reliable logs to understand per-paper acquisition/conversion decisions and failures.
- Source: user-directed M004 follow-up after S01 validation
- Primary owning slice: M004-ubh2pt/full-text-bridge
- Supporting slices: M004-ubh2pt/S01
- Validation: A rerun over the same 10-paper corpus produces machine-readable Loguru logs/diagnostics recording paper id, phase, decision, source path, conversion outcome, warnings/errors, and redacted failure details; missing full text or conversion failures are visible without rerunning with extra debug flags.
- Notes: User clarified to use `loguru` instead of unresolved `rulog`. Prefer project-managed dependency (`uv add loguru`) rather than an ad hoc environment-only pip install.

### R026 — Before scaling validation to 10, 20, or larger document batches, the real-data scientific KG pipeline must be debugged end-to-end on the current small corpus through full text, PageIndex, SemanticChunk, EvidencePath, ExtractionPatch, SCI KG persistence, and retrieval diagnostics.
- Class: quality-attribute
- Status: active
- Description: Before scaling validation to 10, 20, or larger document batches, the real-data scientific KG pipeline must be debugged end-to-end on the current small corpus through full text, PageIndex, SemanticChunk, EvidencePath, ExtractionPatch, SCI KG persistence, and retrieval diagnostics.
- Why it matters: The current validation has proven full-text/chunk/evidence readiness but not claim/entity/relation extraction, graph persistence, or retrieval quality. Scaling before debugging would produce ambiguous graph quality results.
- Source: user-directed M004 pipeline debugging gate
- Primary owning slice: M004-ubh2pt/pipeline-debug
- Supporting slices: M004-ubh2pt/S01,S02
- Validation: A debug slice produces evidence for each pipeline stage, identifies/fixes zero-chunk conversion behavior or documents exclusions, creates real or explicitly baseline ExtractionPatch outputs, persists SCI KG records for eligible papers, and samples retrieval diagnostics before any 20-document run.

### R027 — Before scientific KG validation or scaling continues, converted paper data and chunks must satisfy an explicit graph-readiness quality contract covering conversion fidelity, normalization, chunk semantics, table/figure handling, section hierarchy, and evidence provenance.
- Class: quality-attribute
- Status: active
- Description: Before scientific KG validation or scaling continues, converted paper data and chunks must satisfy an explicit graph-readiness quality contract covering conversion fidelity, normalization, chunk semantics, table/figure handling, section hierarchy, and evidence provenance.
- Why it matters: Poorly normalized conversion/chunk data will poison Claim/Entity/Relation extraction and make graph-quality results meaningless.
- Source: user-feedback-after-S10-chunk-review
- Primary owning slice: M004
- Validation: A dedicated research/benchmark slice defines metrics and acceptance thresholds, then evaluates a small representative corpus before KG validation resumes.
- Notes: This requirement gates M004/S05 and later validation. Non-zero chunks are not sufficient evidence of graph readiness.

### R028 — Validation of conversion, chunking, extraction, and graph-readiness must include an independent artifact review step where feasible, preferably via a subagent, that inspects raw outputs and test meaningfulness rather than relying only on code tests or mocked fixtures.
- Class: quality-attribute
- Status: active
- Description: Validation of conversion, chunking, extraction, and graph-readiness must include an independent artifact review step where feasible, preferably via a subagent, that inspects raw outputs and test meaningfulness rather than relying only on code tests or mocked fixtures.
- Why it matters: Passing tests can create false confidence when tests only verify plumbing, mocks, or counts. Scientific KG validation needs human-readable artifact quality checks to catch semantic failures.
- Source: user-feedback-after-S10-quality-review
- Primary owning slice: M004
- Validation: S11 defines a reusable independent-review rubric and applies it to the conversion/chunking benchmark artifacts before S05 resumes.
- Notes: The review should look at naked artifacts such as Markdown, chunk samples, quality reports, graph exports, and test assertions. It should explicitly flag empty tests, over-mocked tests, count-only validation, and outputs that pass schema checks but fail semantic usefulness.

### R029 — Before KG import continues, chunking must produce an import-ready typed chunk package with stable IDs, source spans, parent-child lineage, content routes, quality states, deterministic annotations, and independent review evidence.
- Class: quality-attribute
- Status: active
- Description: Before KG import continues, chunking must produce an import-ready typed chunk package with stable IDs, source spans, parent-child lineage, content routes, quality states, deterministic annotations, and independent review evidence.
- Why it matters: Scientific KG import quality depends on chunk semantics and provenance. If chunking mixes claims, loses table/figure structure, or lacks source traceability, downstream extraction and graph import will create false KG facts.
- Source: user-directed post-M004 chunking deepening
- Primary owning slice: M005-dlko4z
- Validation: A milestone produces a versioned import-ready chunk package contract, runs it over representative real papers, benchmarks current vs improved chunking, passes independent artifact review, and blocks KG import for chunks that are retrieval-only, repair-required, rejected, or route-excluded.
- Notes: This extends R027 from graph-readiness research into implementation. The import model must not persist raw text or embeddings in machine logs and must not promote annotations into KG facts without extraction validation.

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
| R024 | quality-attribute | active | future-validation-milestone | M003-km5fty/S01-S10 | A validation report exists for the 10-document batch and 20-document batch before any one-week full run; each report summarizes graph node/edge counts, claim/entity/relation quality, evidence-path coverage, retrieval behavior, diagnostics/failures, and go/no-go recommendations. |
| R025 | operability | active | M004-ubh2pt/full-text-bridge | M004-ubh2pt/S01 | A rerun over the same 10-paper corpus produces machine-readable Loguru logs/diagnostics recording paper id, phase, decision, source path, conversion outcome, warnings/errors, and redacted failure details; missing full text or conversion failures are visible without rerunning with extra debug flags. |
| R026 | quality-attribute | active | M004-ubh2pt/pipeline-debug | M004-ubh2pt/S01,S02 | A debug slice produces evidence for each pipeline stage, identifies/fixes zero-chunk conversion behavior or documents exclusions, creates real or explicitly baseline ExtractionPatch outputs, persists SCI KG records for eligible papers, and samples retrieval diagnostics before any 20-document run. |
| R027 | quality-attribute | active | M004 | none | A dedicated research/benchmark slice defines metrics and acceptance thresholds, then evaluates a small representative corpus before KG validation resumes. |
| R028 | quality-attribute | active | M004 | none | S11 defines a reusable independent-review rubric and applies it to the conversion/chunking benchmark artifacts before S05 resumes. |
| R029 | quality-attribute | active | M005-dlko4z | none | A milestone produces a versioned import-ready chunk package contract, runs it over representative real papers, benchmarks current vs improved chunking, passes independent artifact review, and blocks KG import for chunks that are retrieval-only, repair-required, rejected, or route-excluded. |

## Coverage Summary

- Active requirements: 9
- Mapped to slices: 9
- Validated: 20 (R001, R002, R003, R004, R005, R006, R007, R008, R009, R010, R011, R012, R013, R014, R015, R016, R017, R018, R020, R021)
- Unmapped active requirements: 0
