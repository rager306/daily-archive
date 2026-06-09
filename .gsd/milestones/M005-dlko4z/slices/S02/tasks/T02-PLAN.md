---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Ran the baseline chunk measurement over the gold corpus and proved current chunks are retrieval-only, not import-ready.

Run the baseline package builder over the S01 gold corpus manifest. Emit JSON diagnostics for each paper and aggregate summary. The run must record missing artifact blockers, not silently skip papers; it must keep `raw_text_included=false`, `embeddings_included=false`, `production_import_attempted=false`, and `ladybugdb_written=false`.

## Inputs

- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `src/arxiv_archive/chunk_baseline_measurement.py`

## Expected Output

- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl`
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json`

## Verification

uv run python -m arxiv_archive.chunk_baseline_measurement --manifest .gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json --output-dir .gsd/milestones/M005-dlko4z/slices/S02/run-evidence && test -s .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl

## Observability Impact

Run evidence captures per-paper valid_package/import_ready/refusal counts and blockers.
