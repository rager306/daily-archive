# Package Layout Migration, Verification, and Archive Plan

Generated after M085. Updated after M087: this plan is now subordinate to the `research_graph` package rename design in `artifacts/package-rename-design/`. Future migration waves should target `src/research_graph/`, not continue polishing `src/arxiv_archive/` as the final package identity.

## Current facts

Existing canonical subpackages:

```text
arxiv_archive.artifacts   # metrics, reducer, evidence_bridge, assets, minimax_boundary
arxiv_archive.assets      # non-article artifact asset package; keep distinct
arxiv_archive.chunking
arxiv_archive.identity
arxiv_archive.indexing
arxiv_archive.ingestion
arxiv_archive.llm
arxiv_archive.parsing
arxiv_archive.quality
arxiv_archive.staging
```

Already-shimmed top-level modules:

```text
article_artifact_metrics -> arxiv_archive.artifacts.metrics
article_artifact_minimax -> arxiv_archive.artifacts.minimax_boundary
article_artifact_reducer -> arxiv_archive.artifacts.reducer
article_assets -> arxiv_archive.artifacts.assets
article_evidence_bridge -> arxiv_archive.artifacts.evidence_bridge
article_loader -> arxiv_archive.ingestion.loader
candidate_locators -> arxiv_archive.staging.graph_candidates
full_text -> arxiv_archive.ingestion.loader
import_boundary_rehearsal -> arxiv_archive.staging.import_boundary
llm_provider_config -> arxiv_archive.llm.provider_config
page_index -> arxiv_archive.indexing.navigation / arxiv_archive.indexing.page_index
pdf_downloader -> arxiv_archive.ingestion.fetchers
```

Remaining top-level article-related implementation modules:

```text
article_artifact_worker.py
article_artifacts.py
article_batch_validation.py
article_links_dedup.py
article_page_index.py
article_retrieval_tables.py
```

High-risk central modules that should not be moved without their own design/verification slice:

```text
cli.py
rlm_workflow.py
universal_kb_contracts.py
models_registry.py
graph_readiness.py
article_page_index.py
article_artifacts.py
article_artifact_worker.py
```

## Migration policy

For every move:

1. Run GitNexus impact checks for public symbols that will move.
2. Run direct import search for the source module.
3. Copy/move implementation into canonical package path.
4. Replace old top-level file with explicit compatibility shim.
5. Update internal repo imports to canonical path.
6. Add or update compatibility test proving legacy path re-exports canonical objects.
7. Run targeted tests plus `python3 -m py_compile` for changed modules.
8. Run `rg` old direct import search.
9. Run `gitnexus_detect_changes`.
10. Write refactor summary.
11. Complete GSD slice/milestone.

No move should remove a shim in the same milestone.

## Phase 0 — migration guardrails

Goal: stop ad-hoc moves.

Create a migration registry artifact and/or test listing:

- old top-level module
- canonical module
- status: implementation | shim | archived
- planned tests
- owner package
- archive eligibility

Recommended file:

```text
artifacts/package-layout-migration-plan/registry.json
```

Recommended guard test:

```text
tests/test_package_layout_compatibility.py
```

Checks:

- every shim imports canonical module successfully;
- representative public objects are identical across shim/canonical path;
- `src/`, `tests/`, and `scripts/` do not import old paths except explicit compatibility tests;
- archive candidates have zero direct repo imports.

## Phase 1 — finish article artifact context

### 1A. Move worker boundary

```text
src/arxiv_archive/article_artifact_worker.py
-> src/arxiv_archive/artifacts/worker.py
```

Risk: high.

Reason: touches live/mock transport, work completion events, M050 worker/e2e tests.

Checks:

```bash
uv run pytest tests/test_m050_article_artifact_worker.py tests/test_m050_e2e_pipeline.py tests/test_article_artifact_minimax.py -q
python3 -m py_compile src/arxiv_archive/artifacts/worker.py src/arxiv_archive/article_artifact_worker.py
```

No live provider calls.

### 1B. Move artifact manifest/model module

```text
src/arxiv_archive/article_artifacts.py
-> src/arxiv_archive/artifacts/models.py
```

Risk: high.

Reason: large public surface, CLI coupling, scaffold gate coupling.

Do not split during the first move. First move whole implementation to `artifacts.models` with shim. Split later only after behavior is stable.

Checks:

```bash
uv run pytest tests/test_article_artifacts_cli.py tests/test_m023_artifact_scaffold_gate.py tests/test_article_artifact_minimax.py tests/test_m050_e2e_pipeline.py -q
python3 -m py_compile src/arxiv_archive/artifacts/models.py src/arxiv_archive/article_artifacts.py src/arxiv_archive/cli.py scripts/verify_m023_artifact_scaffold_gate.py
```

### 1C. Move batch validation

```text
src/arxiv_archive/article_batch_validation.py
-> src/arxiv_archive/artifacts/batch_validation.py
```

Risk: medium.

Checks: direct tests/import searches plus any batch validation tests discovered by `rg article_batch_validation tests scripts src`.

## Phase 2 — article indexing and retrieval context

### 2A. Move link dedup

```text
src/arxiv_archive/article_links_dedup.py
-> src/arxiv_archive/indexing/article_links_dedup.py
```

Risk: medium.

### 2B. Move retrieval tables

```text
src/arxiv_archive/article_retrieval_tables.py
-> src/arxiv_archive/indexing/retrieval_tables.py
```

Risk: medium.

### 2C. Move article page index

```text
src/arxiv_archive/article_page_index.py
-> src/arxiv_archive/indexing/article_page_index.py
```

Risk: high.

Reason: large public surface and existing `page_index` shim already points into `indexing`.

Checks:

```bash
rg -n "article_page_index|page_index|article_retrieval_tables|article_links_dedup" src tests scripts
uv run pytest <discovered indexing/page tests> -q
python3 -m py_compile src/arxiv_archive/indexing/article_page_index.py src/arxiv_archive/article_page_index.py
```

## Phase 3 — LLM helper context

### 3A. Move MiniMax structured helper

```text
src/arxiv_archive/minimax_structured.py
-> src/arxiv_archive/llm/minimax_structured.py
```

Risk: medium.

### 3B. Move MiniMax usage accounting

```text
src/arxiv_archive/minimax_usage.py
-> src/arxiv_archive/llm/minimax_usage.py
```

Risk: medium.

Checks:

- no live API calls;
- provider env policy remains namespaced;
- tests/import searches around MiniMax/GLM/LLM helpers.

## Phase 4 — graph/readiness/staging context

Do not move these piecemeal without a design slice first.

Candidate package:

```text
arxiv_archive.graph_readiness
```

or, if keeping current vocabulary:

```text
arxiv_archive.staging.readiness
```

Modules:

```text
graph_readiness.py
graph_readiness_review.py
graph_readiness_export.py
graph_readiness_manifest.py
graph_readiness_persistence.py
graph_readiness_extraction_gate.py
graph_readiness_retrieval_validation.py
rlm_graph_traversal.py
ladybug_client.py
```

Risk: high/medium.

Required first milestone: design package boundary and choose name. Do not move `graph_readiness.py` first unless verification covers review, manifest, export, persistence, and retrieval validation flows.

Mandatory check for graph-readiness review work:

```bash
uv run python -m arxiv_archive.graph_readiness_review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review
```

## Phase 5 — universal KB / queue / contracts context

Candidate packages:

```text
arxiv_archive.queue.universal_kb
arxiv_archive.contracts.universal_kb
arxiv_archive.workflows.validation_batch
```

Modules:

```text
universal_kb_queue.py
universal_kb_contracts.py
universal_kb_review_assistance.py
universal_kb_rehearsal.py
universal_kb_smoke.py
universal_kb_sidecar_boundary.py
universal_kb_substrate_rehearsal.py
validation_batch_state.py
validation_batch_workflow.py
validation_batch_provenance.py
```

Risk: medium/high.

Rule: move contracts before queue/workflow only if all importers are updated in one slice and compatibility tests exist. Otherwise move queue first with shims.

## Phase 6 — extraction/chunking/evaluation context

Candidate packages:

```text
arxiv_archive.extraction
arxiv_archive.evaluation
arxiv_archive.repair
```

Modules:

```text
scientific_extraction.py
dspy_extraction.py
extraction_benchmark.py
bounded_chunk_repair.py
chunk_repair_contract.py
chunk_import_contract.py
chunk_baseline_measurement.py
chunking_benchmark.py
evaluation.py
scoring.py
evidence.py
hybrid_retrieval.py
embedder.py
```

Risk: medium.

Rule: do not introduce DSPy/RLM optimizer behavior during layout moves. Layout-only.

## Phase 7 — CLI and workflow shells

`cli.py` and `rlm_workflow.py` should be last.

Do not move them just to reduce top-level file count. First move their dependencies. Then decide whether they remain top-level entrypoints or become thin wrappers around package commands/workflows.

Possible final shape:

```text
arxiv_archive.cli                # stays as public entrypoint
arxiv_archive.workflows.rlm      # workflow implementation
arxiv_archive.commands.*         # command implementation helpers if needed
```

## Archive policy

User policy after M086: do not keep ballast in `src/arxiv_archive` once a canonical package path is verified. Old files are not deleted; they are moved to non-importable archive directories in waves.

### Wave archive mode — default

For each wave:

1. Move implementation into canonical package path if it is not already there.
2. Move the old top-level file out of `src/arxiv_archive/` into:

```text
archive/package-layout-shims/wave-XX/src/arxiv_archive/<old_module>.py
```

3. Add a `Formerly: src/arxiv_archive/<old_module>.py` breadcrumb inside the canonical module docstring.
4. Rewrite repo imports and tests to canonical paths.
5. Replace legacy compatibility tests with archive/breadcrumb tests so future readers can see:
   - old file exists in archive;
   - old file no longer exists in importable `src`;
   - canonical module names the former location;
   - canonical behavior still passes.
6. Write/update an archive manifest for the wave.

The archive preserves exact old files for audit/history, but they are no longer part of the importable runtime package.

### What not to do

- Do not silently delete old files.
- Do not keep duplicate executable implementations under `archive/` as active code. They are historical evidence only.
- Do not leave old top-level imports in `src`, `tests`, or `scripts`.
- Do not remove or archive a module in the same wave as unrelated semantic behavior changes.

### Archive manifest requirements

Each wave manifest must list:

- old path;
- canonical path;
- archive path;
- status: archived shim | archived implementation;
- verification command/test group;
- known breakage: old import path intentionally unavailable.

## Recommended milestone sequence

```text
M086: Archive wave 01 for already-canonical artifact/LLM shims
M087: Migration registry and guard tests for remaining waves
M088: Move and archive article_artifact_worker -> artifacts.worker
M089: Move and archive article_artifacts -> artifacts.models
M090: Move and archive article_batch_validation -> artifacts.batch_validation
M091: Move and archive article_links_dedup -> indexing.article_links_dedup
M092: Move and archive article_retrieval_tables -> indexing.retrieval_tables
M093: Move and archive article_page_index -> indexing.article_page_index
M094: Move and archive minimax_structured/minimax_usage -> llm package
M095: Design graph/readiness package boundary
M096-M09X: Move and archive graph/readiness modules by sub-flow
M10X: Universal KB / queue / contracts package boundary
M11X: Extraction/chunking/evaluation package boundary
M12X: CLI/workflow shell decision and cleanup
```

## Stop conditions

Pause and replan if any of these happen:

- GitNexus impact returns high/critical on unexpected call paths.
- A move requires semantic code changes beyond imports/shim.
- Targeted tests require live provider calls or secrets.
- Old direct imports remain outside compatibility tests.
- A shim needs private helper exports that were not in the public plan.
- A package name collision appears, especially `arxiv_archive.artifacts.assets` vs `arxiv_archive.assets`.
