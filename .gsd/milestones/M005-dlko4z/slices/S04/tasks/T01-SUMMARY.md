---
id: T01
parent: S04
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/structure_aware_chunking.py
  - tests/test_structure_aware_chunking.py
key_decisions:
  - Annotation sidecars serialize through the existing import-contract `annotations` field but always use `promoted_to_fact=false`.
  - Annotation values may carry deterministic metadata, but sidecars include redaction flags and do not include raw chunk text, embeddings, vectors, or secrets.
duration: 
verification_result: passed
completed_at: 2026-05-19T08:07:48.168Z
blocker_discovered: false
---

# T01: Defined deterministic annotation sidecars that serialize safely without becoming KG facts.

**Defined deterministic annotation sidecars that serialize safely without becoming KG facts.**

## What Happened

Defined `ChunkAnnotationSidecar` with annotation id, paper id, chunk id, deterministic method, annotation type, values, confidence class, warning codes, and hard-coded `promoted_to_fact=false`. Structure-aware packages now serialize sidecars through the S01 contract annotations field while preserving redaction flags. Tests cover redacted non-fact annotation serialization and package validation with annotation sidecars included but no import readiness granted.

## Verification

Focused structure-aware tests plus import-contract tests passed; ruff reported all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py` | 0 | ✅ pass — 32 passed; ruff all checks passed | 8500ms |

## Deviations

None.

## Known Issues

T01 only defines sidecar shape and package serialization. T02 must generate deterministic annotations from chunk metadata and summarize annotation diagnostics.

## Files Created/Modified

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`
