---
id: T04
parent: S03
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/structure_aware_chunking.py
  - tests/test_structure_aware_chunking.py
  - .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json
  - .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-package-diagnostics.jsonl
key_decisions:
  - Structure-aware dry-run output is explicitly not import-ready even though packages validate structurally.
  - The summary reports route/state/type/refusal distributions over all gold-corpus papers while preserving no raw text, no embeddings, no production import, and no LadybugDB writes.
duration: 
verification_result: passed
completed_at: 2026-05-19T07:12:20.516Z
blocker_discovered: false
---

# T04: Validated structure-aware packages over the gold corpus and wrote redacted run evidence.

**Validated structure-aware packages over the gold corpus and wrote redacted run evidence.**

## What Happened

Added a structure-aware manifest dry-run path that reads the S01 gold corpus, builds contract-shaped packages from canonical Markdown, validates each package with the existing import contract validator, and writes redacted summary/JSONL diagnostics. The gold-corpus run produced 10 valid packages and 1,831 structure-aware chunks across claim, method, citation, table, metadata, equation, figure, and retrieval-only routes. All chunks remain refused/import-ineligible, no package is import-ready, and the machine artifacts report no raw text, embeddings, production import attempts, or LadybugDB writes.

## Verification

Focused tests and ruff passed, then the real gold-corpus dry run wrote non-empty structure-aware summary and diagnostics artifacts with redaction and no-write flags verified.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py` | 0 | ✅ pass — 29 passed; ruff all checks passed | 15300ms |
| 2 | `uv run python -m arxiv_archive.structure_aware_chunking --manifest .gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json --output-dir .gsd/milestones/M005-dlko4z/slices/S03/run-evidence && validate summary/artifact presence and safety flags` | 0 | ✅ pass — paper_count=10, valid_package_count=10, chunk_count=1831, import_ready_count=0, import_eligible_chunk_count=0, safety flags false | 5000ms |

## Deviations

T04 added both callable and CLI dry-run paths. The run produced more chunks than the S02 baseline because it emits structural elements/chunks rather than SemanticChunk sections, but all remain non-importable pending later review.

## Known Issues

The structure-aware parser still uses deterministic heuristics and marks all chunks non-importable. T05 must report this boundary and run independent review before closing S03.

## Files Created/Modified

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-package-diagnostics.jsonl`
