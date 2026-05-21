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

### R030 — Article ingestion must preserve source artifacts alongside derived text, including the original PDF, normalized Markdown, extracted figures, tables, image assets, hashes, provenance, and redacted asset manifests for future multimodal retrieval.
- Class: continuity
- Status: active
- Description: Article ingestion must preserve source artifacts alongside derived text, including the original PDF, normalized Markdown, extracted figures, tables, image assets, hashes, provenance, and redacted asset manifests for future multimodal retrieval.
- Why it matters: Scientific papers often contain figures, plots, tables, and diagrams that are necessary for future multimodal search and evidence review. Losing raw source artifacts during ingestion would make later multimodal KG and retrieval work incomplete or non-reproducible.
- Source: user-direction
- Primary owning slice: M005-dlko4z/S05
- Validation: A future asset-preservation slice writes per-paper PDF/MD/assets plus an assets-manifest with stable asset ids, paths, hashes, page/bbox/caption-span/linkage metadata, and safety flags proving no raw binary/base64/embeddings are included in machine logs.
- Notes: Raw binary/image assets must be stored as files referenced by path/hash/provenance manifests, not embedded in machine JSON/JSONL logs. Assets are not KG facts and do not authorize production import, embeddings, or multimodal claims until later review gates pass.

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

### R036 — Validation CLI runs must produce replay/audit provenance logs tying each generated artifact to the exact command, inputs, output hashes, exit code, cwd, git commit, and active milestone/batch context.
- Class: failure-visibility
- Status: active
- Description: Validation CLI runs must produce replay/audit provenance logs tying each generated artifact to the exact command, inputs, output hashes, exit code, cwd, git commit, and active milestone/batch context.
- Why it matters: Current validation artifacts can be checked for consistency, but they do not fully prove that each artifact was freshly produced by a specific CLI run. Provenance is required to detect stale artifacts and metadata mismatches such as an M008 artifact carrying M006 milestone metadata.
- Source: user feedback after M008 completion
- Primary owning slice: next validation hardening milestone
- Validation: Partial validation: S01/S02/S03 artifacts prove fresh/stale/missing/hash/metadata verification. Final guard: freshness_pass=fresh, freshness_stale=stale, lineage_mismatch=stale. Missing: automatic provenance emission for real validation-batch commands.
- Notes: M009 implemented provenance/freshness primitives, `validation-batch verify-artifacts`, active lineage metadata checks, and pass/stale/mismatch sample evidence. Automatic provenance emission from init/preflight/scan remains future work, so R036 stays active until real runs produce provenance without manual wrapper steps.

### R040 — New infrastructure must be researched, compatibility-probed, and safety-wrapped before it is enabled in the main Scientific KG process.
- Class: constraint
- Status: active
- Description: New infrastructure must be researched, compatibility-probed, and safety-wrapped before it is enabled in the main Scientific KG process.
- Why it matters: The pipeline depends on strict provenance, redaction, no-import/no-write safety, and reproducibility. Unprepared infrastructure can introduce hidden incompatibilities, secret handling risks, cost/rate failures, optimizer side effects, or false semantic readiness.
- Source: user principle during M012 planning
- Primary owning slice: project
- Validation: Future milestones that introduce infrastructure must include research/probe artifacts, failure-mode analysis, artifact/redaction boundaries, and an explicit go/no-go decision before process activation.

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
| R030 | continuity | active | M005-dlko4z/S05 | none | A future asset-preservation slice writes per-paper PDF/MD/assets plus an assets-manifest with stable asset ids, paths, hashes, page/bbox/caption-span/linkage metadata, and safety flags proving no raw binary/base64/embeddings are included in machine logs. |
| R031 | quality-attribute | active | M006-638rza | M005-dlko4z | A 30-paper dry-run report exists with redacted aggregate/per-paper diagnostics, deviation analysis against M005 10-paper evidence, new-pattern taxonomy, and explicit no-go/go recommendations for remediation. |
| R032 | operability | active | future-validation-automation | M006-638rza | A CLI or equivalent command can run batches of +10 papers, persist per-batch manifests/diagnostics/reports, resume after failures, compare each batch against prior baselines, and stop at review gates without production KG writes. |
| R033 | operability | active | M007-opaont | M007/S01,S02,S03,S04 | A local CLI can select the next batch, preflight/acquire sources, run redacted deviation scans, compare route/refusal deltas, flag outliers/contradictions, and persist resumable batch state without raw/chunk text or KG writes. |
| R034 | primary-user-loop | validated | M008-c9zb94 | M008/S01,S02,S03,S04 | M008 evidence: selected_count=10, m006_overlap_count=0, final source_ready=10, quota accepted_ready_count=10, scan paper_count=10, chunk_count=1591, outlier_count=6, import_eligible_chunk_count=0, review verdict FLAG with next-batch gate. |
| R035 | quality-attribute | active | M008-c9zb94 | M008/S03,M008/S04 | Partial validation: top-up pass sample final_accepted_ready_count=target_count and scan_allowed=true; blocked sample remaining_shortage_count=2 and scan_allowed=false. Missing: automatic acquisition/preflight integration for accepted replacements. |
| R036 | failure-visibility | active | next validation hardening milestone | none | Partial validation: S01/S02/S03 artifacts prove fresh/stale/missing/hash/metadata verification. Final guard: freshness_pass=fresh, freshness_stale=stale, lineage_mismatch=stale. Missing: automatic provenance emission for real validation-batch commands. |
| R037 | core-capability | validated | next reviewed +10 milestone | none | M010 final guard: review_verdict=PASS; selected_count=10; prior_overlap_count=0; quota_ready_count=10; paper_count=10; chunk_count=1477; freshness_verdict=fresh; import_eligible_chunk_count=0; positive_import_blocked=true; production_writes_blocked=true; unattended_scaling_blocked=true. |
| R038 | quality-attribute | validated | M011 | none | M011 final guard: review_verdict=PASS; gate_result=pass_negative_readiness_gate; target_count=10; source_hash_missing_count=0; repair_required_count=7; retrieval_only_count=3; import_candidate_count=0; raw_payload_key_count=0; positive_import_blocked=true; production_writes_blocked=true; chunk_span_provenance_required_next=true. |
| R039 | constraint | validated | M012 | none | M012 final guard: review_verdict=PASS; dspy_verdict=conditional_go_optional_dev_probe_only; minimax_verdict=conditional_go_optional_helper_probe_only; production_import_allowed=false; dspy_optimizer_allowed=false; minimax_orchestrator_allowed=false; next_safe_options=[dspy_optional_dev_dependency_no_lm_probe, minimax_explicit_synthetic_auth_smoke_test, chunk_span_provenance_candidate_locator_packet]. |
| R040 | constraint | active | project | none | Future milestones that introduce infrastructure must include research/probe artifacts, failure-mode analysis, artifact/redaction boundaries, and an explicit go/no-go decision before process activation. |
| R041 | constraint | validated | M013 | none | M013 final guard: review_verdict=PASS; dspy_dependency_verdict=pass_isolated_optional_dev_probe_ready; dspy_install_succeeded=true; dspy_import_succeeded=true; dspy_predict_failed_closed_without_lm=true; dspy_evaluate_static_program_succeeded=true; dspy_possible_dev_optimizers=[KNNFewShot,LabeledFewShot]; dspy_optimizer_execution_allowed=false; minimax_smoke_verdict=pass_synthetic_callability_only; minimax_http_status=200; minimax_orchestrator_allowed=false; production_import_allowed=false. |
| R042 | integration | validated | M014-65dlgp | none | M014 final guard: review_verdict=PASS; subscription_budget_non_blocking=true; platform_limits_still_apply=true; weekly_usage_quota_documented=10x the 5-hour quota; live_call_count=4; successful_http_count=4; redacted_helper_success_count=1; raw_response_persisted=false; raw_model_content_persisted=false; secrets_logged=false; production_import_allowed=false; ladybugdb_written=false; minimax_orchestrator_allowed=false; source_of_truth_allowed=false. |
| R043 | integration | validated | M015-ktorc7 | none | M015 final guard: review_verdict=PASS; structured_output_verdict=tool_call_recommended; recommended_structured_interfaces=[anthropic_forced_tool_call,openai_response_format_json_schema,openai_response_format_json_object]; anthropic_forced_tool_schema_validated=true; schema_validated_count=1; token_plan_limit_check_verdict=ui_only_or_session_required; token_plan_api_remains_verified=false; true_remains_success_count=0; raw_response_persisted=false; secrets_logged=false; production_import_allowed=false; source_of_truth_allowed=false. |
| R044 | integration | validated | M016-9819d1 | none | M016 final guard: limit_check_verdict=api_remains_verified; used_9router_algorithm=true; m015_limit_verdict_overturned=true; working_endpoint=https://api.minimax.io/v1/api/openplatform/coding_plan/remains; count_means_remaining=true; true_success_count=1; quota_row_count_total=8; raw_response_persisted=false; exact_quota_values_persisted=false; credential_values_logged=false. |
| R045 | integration | validated | M017-cf3fd0 | none | M017 final guard: tests_passed=9; ruff_passed=true; reviewer_verdict=PASS; security_final_verdict=PASS_WITH_NOTED_DEPENDENCY_DEBT; raw_response_persisted=false; exact_quota_values_persisted=false; credential_values_logged=false; raw_corpus_payload_allowed=false; raw_model_content_persisted=false; production_import_allowed=false; ladybugdb_write_allowed=false; minimax_source_of_truth=false; live_call_performed=false. |

## Coverage Summary

- Active requirements: 16
- Mapped to slices: 16
- Validated: 29 (R001, R002, R003, R004, R005, R006, R007, R008, R009, R010, R011, R012, R013, R014, R015, R016, R017, R018, R020, R021, R034, R037, R038, R039, R041, R042, R043, R044, R045)
- Unmapped active requirements: 0
