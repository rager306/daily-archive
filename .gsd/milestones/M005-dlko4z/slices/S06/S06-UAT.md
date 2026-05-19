# S06: Benchmark chunking methods and independent review — UAT

**Milestone:** M005-dlko4z
**Written:** 2026-05-19T11:08:32.290Z

# S06: Benchmark chunking methods and independent review — UAT

**Milestone:** M005-dlko4z

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: S06 ships benchmark contracts, redacted benchmark artifacts, review samples, and an independent review report. There is no live UI or production service.

## Preconditions

- S02 baseline summary exists.
- S03 structure-aware summary exists.
- S04 annotation summary exists.
- S05 source asset summary exists.
- S06 benchmark artifacts exist.

## Smoke Test

Run the slice verification command and confirm it prints `65 passed`, `All checks passed!`, and an artifact guard with `method_count=3`, `total_chunk_count=2471`, `total_import_eligible_chunk_count=0`, `review_verdict=BLOCK for positive import`, and `safety_flags_false=true`.

## Test Cases

### 1. Benchmark tests pass

1. Run `uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q`.
2. **Expected:** 65 tests pass.

### 2. Benchmark artifacts are present

1. Check `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json`.
2. Check `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-diagnostics.jsonl`.
3. **Expected:** Both files are non-empty and parseable.

### 3. Benchmark remains import-blocked

1. Read the benchmark summary.
2. **Expected:** `method_count=3`, `total_chunk_count=2471`, `total_import_eligible_chunk_count=0`, `total_refused_chunk_count=2471`, and `recommendation_status=review_required`.

### 4. Safety flags remain closed

1. Inspect summary safety flags.
2. **Expected:** `raw_text_included=false`, `chunk_text_included=false`, `embeddings_included=false`, `vectors_included=false`, `optimizer_traces_included=false`, `ladybugdb_written=false`, and `production_import_attempted=false`.

### 5. Independent review blocks positive import

1. Read `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-review-summary.md`.
2. **Expected:** Verdict says BLOCK for S07 positive/import rehearsal.

## Edge Cases

### External library candidate not executed

1. Inspect `simple_section_window_estimate` caveats.
2. **Expected:** It states real libraries such as Chonkie/LlamaIndex/LangChain were not executed.

### Missing PDFs affect benchmark caveats

1. Inspect missing source counts.
2. **Expected:** `missing_original_pdf` caveat is visible.

## Failure Signals

- Tests fail.
- Benchmark summary or diagnostics missing.
- Any method reports non-zero import eligibility without reviewed evidence.
- Any safety flag is true.
- Review summary is missing or does not explicitly state S07 readiness/blocking status.

## Requirements Proved By This UAT

- R029 — The benchmark stage produces evidence and independent review, but validates that current candidates remain import-blocked.
- R030 — Source/asset manifests are consumed as benchmark inputs for asset-linkage and missing-source caveats.

## Not Proven By This UAT

- Positive trusted KG import readiness.
- Real external chunking library quality.
- Semantic/vector retrieval.
- Entity/relation extraction.
- Multimodal asset extraction.
- Production LadybugDB writes.
