# S01: Import model contract and gold corpus

**Goal:** Define the import-ready chunk package contract and benchmark corpus that all M005 implementation and measurement slices will use.
**Demo:** After this slice, there is a versioned import-ready chunk package contract and a representative benchmark corpus selection with review rubric.

## Must-Haves

- Defines versioned `ImportReadyChunkPackage`, `GraphReadyChunk`, `ChunkAnnotation`, source span, route/state enums, diagnostics, and package invariants.
- Selects a representative gold corpus from existing real-paper artifacts, including known conversion/chunking edge cases and claim-candidate papers.
- Defines import eligibility/refusal rules and review rubric.
- Adds tests or validation script proving contract examples reject missing IDs, missing source spans, raw text/embedding leakage, and unresolved parent/source references.
- Produces S01 artifacts for S02 baseline measurement.

## Proof Level

- This slice proves: Documented contract plus schema/invariant tests for representative fixture packages; no production import.

## Integration Closure

S01 consumes M004/S11 research and R029, then produces the authoritative contract, corpus manifest, and review rubric for S02-S06. It does not change production chunking or write KG data.

## Verification

- S01 defines the observability fields required in later package exports: schema/contract versions, package validity, source-span coverage, route/state counts, redaction flags, warning severity, and refusal reasons.

## Tasks

- [x] **T01: Define import ready chunk contract** `est:0.5d`
  Create a versioned import-ready chunk contract document under S01 that defines package objects, required fields, enums, invariants, refusal states, redaction rules, and downstream import boundaries. Base it on M004/S11 research but make it concrete enough for code and tests.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`
  - Verify: test -s .gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md && rg "ImportReadyChunkPackage|GraphReadyChunk|ChunkAnnotation|GraphReadinessState" .gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md

- [x] **T02: Select representative gold corpus** `est:0.5d`
  Select the representative gold corpus for chunking/import benchmarks from existing real-paper artifacts. Include target paper IDs, why each paper is selected, expected hard cases, and required artifact paths. Keep this as a manifest, not a broad corpus run.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`, `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-rationale.md`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json')
data=json.loads(p.read_text())
assert data['schema_version']=='m005-gold-corpus-manifest.v1'
assert len(data['papers']) >= 6
assert all('paper_id' in paper and 'hard_case_tags' in paper for paper in data['papers'])
assert data['broad_corpus_run'] is False
PY

- [ ] **T03: Implement contract validator fixtures** `est:1d`
  Add a small contract validator module and tests for package invariants. The validator should reject raw text/embedding leakage, missing stable IDs, missing source spans for graph-eligible chunks, unresolved parent/source references, and invalid import states. It should validate synthetic fixtures only in S01.
  - Files: `src/arxiv_archive/chunk_import_contract.py`, `tests/test_chunk_import_contract.py`
  - Verify: uv run pytest tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_import_contract.py tests/test_chunk_import_contract.py

- [ ] **T04: Review import model contract** `est:0.5d`
  Write the S01 review rubric and run an independent review of the contract, corpus manifest, and validator tests. The review must check for missing import fields, overbroad claims, count-only validation, raw-text leakage risk, and whether the corpus covers hard chunking cases.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S01/import-model-review-rubric.md`, `.gsd/milestones/M005-dlko4z/slices/S01/run-evidence/contract-review-summary.md`
  - Verify: uv run pytest tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S01/run-evidence/contract-review-summary.md

## Files Likely Touched

- .gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md
- .gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json
- .gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-rationale.md
- src/arxiv_archive/chunk_import_contract.py
- tests/test_chunk_import_contract.py
- .gsd/milestones/M005-dlko4z/slices/S01/import-model-review-rubric.md
- .gsd/milestones/M005-dlko4z/slices/S01/run-evidence/contract-review-summary.md
