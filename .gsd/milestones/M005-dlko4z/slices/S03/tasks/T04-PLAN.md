---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T04: Validated structure-aware packages over the gold corpus and wrote redacted run evidence.

Build S01 contract-shaped packages from structure-aware chunks and validate them with the existing import contract validator. Add a CLI or callable dry-run path that writes redacted structure-aware package diagnostics for the gold corpus without writing production KG data.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `src/arxiv_archive/chunk_import_contract.py`

## Expected Output

- `src/arxiv_archive/structure_aware_chunking.py`
- `tests/test_structure_aware_chunking.py`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-package-diagnostics.jsonl`

## Verification

uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-package-diagnostics.jsonl

## Observability Impact

Run evidence should record no raw text, no embeddings, no production import attempts, and no LadybugDB writes.
