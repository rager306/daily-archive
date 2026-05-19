---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T04: Review import model contract

Write the S01 review rubric and run an independent review of the contract, corpus manifest, and validator tests. The review must check for missing import fields, overbroad claims, count-only validation, raw-text leakage risk, and whether the corpus covers hard chunking cases.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`
- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `tests/test_chunk_import_contract.py`

## Expected Output

- `.gsd/milestones/M005-dlko4z/slices/S01/import-model-review-rubric.md`
- `.gsd/milestones/M005-dlko4z/slices/S01/run-evidence/contract-review-summary.md`

## Verification

uv run pytest tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S01/run-evidence/contract-review-summary.md

## Observability Impact

Independent review produces PASS/FLAG findings before S02 uses the contract.
