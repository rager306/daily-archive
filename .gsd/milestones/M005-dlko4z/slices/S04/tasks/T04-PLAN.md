---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T04: Run annotation dry run on gold corpus

Run the annotation sidecar dry-run over the gold corpus and write redacted annotation summary plus package diagnostics. Confirm annotation counts and warnings are present while all import/no-write safety flags remain false.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`

## Expected Output

- `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl`

## Verification

uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl

## Observability Impact

Artifacts should include no raw text and no embeddings; report annotation coverage by paper and type.
