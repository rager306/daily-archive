---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T04: Run source asset preservation dry run

Run the source asset preservation and multimodal manifest dry-run over the 10-paper gold corpus. Write per-paper manifests, a redacted run summary, and JSONL diagnostics under S05 run evidence. Confirm all machine artifacts contain only paths/hashes/provenance/linkage/safety flags, not raw content.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl`

## Expected Output

- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl`

## Verification

uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl

## Observability Impact

Dry-run artifacts are the primary health surface for source preservation and must include missing-source diagnostics plus redaction/no-write flags.
