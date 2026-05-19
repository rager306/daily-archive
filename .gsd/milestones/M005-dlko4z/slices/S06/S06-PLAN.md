# S06: Benchmark chunking methods and independent review

**Goal:** Benchmark current, structure-aware, and selected deterministic chunking candidates on the gold corpus using S05 source/asset manifests, compare quality and asset-linkage diagnostics, and obtain independent review before any isolated import rehearsal.
**Demo:** After this slice, current, structure-aware, and selected real chunking candidates are compared on real papers, including asset-linkage quality, and reviewed independently.

## Must-Haves

- Benchmark contract defines comparable metrics for baseline, structure-aware, and candidate chunking outputs, including source-span coverage, parent/reference coverage, route distribution, refusal distribution, annotation/asset-linkage coverage, and import eligibility.
- Candidate methods are deterministic and bounded; no LLM calls, embeddings, production writes, broad corpus scaling, or optimizer behavior.
- Gold-corpus run compares S02 baseline, S03 structure-aware output, and at least one additional deterministic candidate derived from preserved source structure or simple windows.
- Artifacts include per-method/per-paper diagnostics and bounded redacted review samples without raw paper text/chunk text.
- Independent review assesses semantic usefulness and flags whether S07 import rehearsal can proceed.
- Production KG writes and trusted import remain blocked unless explicitly supported by reviewed evidence.

## Proof Level

- This slice proves: Automated tests for benchmark metric calculations and artifact redaction; gold-corpus benchmark dry-run; bounded review samples; independent artifact review before slice completion.

## Integration Closure

S06 consumes S02 baseline evidence, S03 structure-aware packages, S04 annotation sidecars, and S05 source/asset manifests. It produces benchmark comparison artifacts and independent review evidence that determine whether S07 isolated import rehearsal can proceed or must remain blocked/remediated.

## Verification

- Adds run-level benchmark summaries, per-paper/method diagnostics, comparison tables, sample review artifacts, missing-source caveats, and explicit no-write/no-import flags for each candidate method.

## Tasks

- [x] **T01: Define chunking benchmark contract** `est:medium`
  Define a benchmark result contract for chunking methods. Include method id, input corpus, per-paper metrics, aggregate metrics, route/type/state/refusal counts, source-span coverage, parent/reference coverage, annotation coverage, asset-linkage coverage, import eligibility counts, missing-source caveats, and redaction/no-write flags. Add tests for metric aggregation and redaction boundaries.
  - Files: `src/arxiv_archive/chunking_benchmark.py`, `tests/test_chunking_benchmark.py`
  - Verify: uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunking_benchmark.py tests/test_chunking_benchmark.py

- [x] **T02: Implement benchmark adapters** `est:large`
  Implement deterministic benchmark adapters for existing S02 baseline evidence, S03/S04/S05 structure-aware evidence, and one bounded candidate that uses preserved normalized Markdown/source spans to estimate simple section-window chunking diagnostics. Do not add heavy dependencies or execute Chonkie/LlamaIndex/LangChain yet; record them as later benchmark candidates unless explicitly installed and bounded.
  - Files: `src/arxiv_archive/chunking_benchmark.py`, `tests/test_chunking_benchmark.py`
  - Verify: uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunking_benchmark.py tests/test_chunking_benchmark.py

- [x] **T03: Run chunking benchmark dry run** `est:medium`
  Run the benchmark over the 10-paper gold corpus and write redacted aggregate summary plus per-paper/method diagnostics. Confirm all import/no-write flags remain false and no raw text/chunk text/embeddings are serialized.
  - Files: `src/arxiv_archive/chunking_benchmark.py`, `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json`, `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl`
  - Verify: uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl

- [ ] **T04: Generate benchmark review samples** `est:medium`
  Generate bounded redacted benchmark review samples that let an independent reviewer inspect method differences without exposing raw paper text. Include representative per-paper/method rows, route/type/refusal/asset-linkage deltas, missing-source caveats, and recommendation rationale.
  - Files: `src/arxiv_archive/chunking_benchmark.py`, `.gsd/milestones/M005-dlko4z/slices/S06/review/chunking-benchmark-review-samples.md`, `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-index.json`
  - Verify: uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S06/review/chunking-benchmark-review-samples.md && test -s .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-index.json

- [ ] **T05: Review and report benchmark results** `est:medium`
  Perform independent review of benchmark artifacts and write the benchmark report. State which method, if any, is safe for S07 isolated import rehearsal; document blockers, missing PDFs, unexecuted real-library candidates, and what remains unproven.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S06/chunking-benchmark-report.md`, `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-summary.md`
  - Verify: uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S06/chunking-benchmark-report.md && test -s .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-summary.md

## Files Likely Touched

- src/arxiv_archive/chunking_benchmark.py
- tests/test_chunking_benchmark.py
- .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json
- .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl
- .gsd/milestones/M005-dlko4z/slices/S06/review/chunking-benchmark-review-samples.md
- .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-index.json
- .gsd/milestones/M005-dlko4z/slices/S06/chunking-benchmark-report.md
- .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-summary.md
