# S07: Negative isolated import boundary rehearsal — UAT

**Milestone:** M005-dlko4z
**Written:** 2026-05-19T12:36:26.197Z

# S07: Negative isolated import boundary rehearsal — UAT

**Milestone:** M005-dlko4z

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: S07 ships an isolated negative import-boundary contract, run artifacts, and review evidence. There is no live UI or production service.

## Preconditions

- S06 benchmark summary and diagnostics exist.
- S07 run evidence exists.

## Smoke Test

Run the slice verification command and confirm it prints `75 passed`, `All checks passed!`, and an artifact guard with `candidate_count=2471`, `accepted_count=0`, `rejected_count=2471`, `positive_import=blocked`, and `safety_flags_false=true`.

## Test Cases

### 1. Rehearsal tests pass

1. Run `uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q`.
2. **Expected:** 75 tests pass.

### 2. Lint passes

1. Run `uv run ruff check src/arxiv_archive/import_boundary_rehearsal.py tests/test_import_boundary_rehearsal.py`.
2. **Expected:** all checks pass.

### 3. Negative import summary is consistent

1. Read `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-summary.json`.
2. **Expected:** `candidate_count=2471`, `accepted_count=0`, `rejected_count=2471`, and `source_benchmark_summary.total_import_eligible_chunk_count=0`.

### 4. No-write and no-payload flags remain closed

1. Inspect summary safety flags.
2. **Expected:** raw/chunk text, raw binary/base64, embeddings, vectors, secrets, optimizer traces, LadybugDB writes, and production import attempts are all false.

### 5. Review scope is explicit

1. Read `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-review-summary.md`.
2. **Expected:** PASS for the narrow negative-boundary claim and BLOCK for positive trusted KG import.

## Edge Cases

### Positive import attempted accidentally

If any candidate includes `trusted_kg_import` in allowed uses or production write flags become true, validation should fail.

### Raw or embedding payload appears

If nested raw text, chunk text, embeddings, vectors, secrets, or optimizer traces appear in evidence, validation should fail without logging leaked values.

## Failure Signals

- Any accepted import appears in current S07 evidence.
- Any production write flag becomes true.
- Any payload leakage flag becomes true.
- Review wording claims positive KG import readiness.

## Requirements Proved By This UAT

- R029 — Current candidates are rejected before trusted KG import unless explicitly eligible.
- R030 — Source/asset caveats remain metadata only and no raw assets are embedded.

## Not Proven By This UAT

- Positive trusted KG import readiness.
- Entity or relation extraction.
- Semantic/vector retrieval.
- Multimodal extraction.
- Production LadybugDB persistence.
