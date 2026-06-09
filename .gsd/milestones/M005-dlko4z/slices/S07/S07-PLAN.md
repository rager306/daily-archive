# S07: S07

**Goal:** Prove the isolated import boundary rejects all current M005 package candidates safely, writes no production KG data, and emits actionable refusal diagnostics instead of attempting positive trusted KG import.
**Demo:** After this slice, an isolated import boundary rehearsal proves current packages are rejected safely with no production KG writes, and documents what remediation is required before any positive import path.

## Must-Haves

- Isolated import boundary accepts package-shaped inputs but rejects all current ineligible chunks/assets before any trusted KG write.
- Rehearsal over current S03/S04/S05/S06 artifacts reports zero accepted imports and a refusal count matching current benchmark evidence.
- No production LadybugDB writes, embeddings, vectors, raw text, raw binary/base64, secrets, or optimizer traces are emitted.
- Diagnostics identify why candidates are refused and what remediation is required before any positive import rehearsal.
- Independent review confirms the negative rehearsal proves safety boundary rather than hiding missing positive import readiness.

## Proof Level

- This slice proves: Automated tests for isolated import boundary behavior; dry-run over current M005 artifacts; redacted negative import rehearsal evidence; independent review before milestone validation.

## Integration Closure

S07 consumes S06 benchmark evidence plus S03/S04/S05 package/annotation/asset artifacts. It closes M005 by exercising the import boundary negatively: current packages must validate as structurally inspectable but import-ineligible, with no LadybugDB production writes and clear remediation signals for a future positive import slice.

## Verification

- Adds isolated import rehearsal summaries, per-method/per-package rejection diagnostics, no-write evidence, refusal reason counts, and remediation guidance for future import-positive work.

## Tasks

- [x] **T01: Defined the negative import rehearsal contract and validator for S07.** `est:medium`
  Define an isolated import rehearsal contract and validator for negative import boundary evidence. Include accepted/rejected counts, refusal reasons, package/method ids, no-write flags, redaction flags, and remediation hints. Add tests showing import-ineligible chunks/assets are rejected and raw/embedding/write leakage is blocked.
  - Files: `src/arxiv_archive/import_boundary_rehearsal.py`, `tests/test_import_boundary_rehearsal.py`
  - Verify: uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/import_boundary_rehearsal.py tests/test_import_boundary_rehearsal.py

- [x] **T02: Built S07 rehearsal candidates from S06 benchmark artifacts without raw-content access or graph writes.** `est:large`
  Implement adapters that read current S06 benchmark diagnostics and S05/S04/S03 package artifacts to create isolated import rehearsal candidates. The adapter should preserve method/package identity and refusal context but never load raw source files or attempt graph writes.
  - Files: `src/arxiv_archive/import_boundary_rehearsal.py`, `tests/test_import_boundary_rehearsal.py`
  - Verify: uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/import_boundary_rehearsal.py tests/test_import_boundary_rehearsal.py

- [x] **T03: Ran the negative import boundary rehearsal and wrote redacted S07 evidence.** `est:medium`
  Run the negative isolated import rehearsal over current M005 artifacts and write redacted run summary plus rejection diagnostics. Confirm accepted imports are zero, rejected candidates match benchmark counts, and all no-write safety flags remain false.
  - Files: `src/arxiv_archive/import_boundary_rehearsal.py`, `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-summary.json`, `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-diagnostics.jsonl`
  - Verify: uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-diagnostics.jsonl

- [x] **T04: Reported and reviewed the negative import-boundary rehearsal with positive KG import still blocked.** `est:medium`
  Write a remediation report and independent review summary for the negative rehearsal. State exactly what is proven, why positive import remains blocked, and what future slice would need to create a reviewed import-eligible subset.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S07/import-boundary-rehearsal-report.md`, `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-review-summary.md`
  - Verify: uv run pytest tests/test_import_boundary_rehearsal.py tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S07/import-boundary-rehearsal-report.md && test -s .gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-review-summary.md

## Files Likely Touched

- src/arxiv_archive/import_boundary_rehearsal.py
- tests/test_import_boundary_rehearsal.py
- .gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-summary.json
- .gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-diagnostics.jsonl
- .gsd/milestones/M005-dlko4z/slices/S07/import-boundary-rehearsal-report.md
- .gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-review-summary.md
