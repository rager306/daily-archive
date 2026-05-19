# S02: Baseline chunk quality measurement

**Goal:** Measure current chunking against the S01 import-ready contract and produce a baseline report of import-readiness failures without changing the chunking model or writing KG data.
**Demo:** After this slice, current chunking has measured import-readiness failures and a baseline report.

## Must-Haves

- Baseline exporter maps current artifacts/PageIndex/SemanticChunk-like chunks into S01 import contract packages without production KG writes.
- Gold corpus baseline run emits redacted per-paper package validation diagnostics.
- Report summarizes failures by paper, route, state, refusal reason, source-span coverage, parent/evidence resolution, and missing artifacts.
- Bounded review samples exist for the six-paper inner review minimum or are explicitly blocked by missing artifacts.
- Independent review confirms S02 does not overclaim import readiness from baseline counts.

## Proof Level

- This slice proves: Focused tests plus real gold-corpus baseline run plus independent artifact review.

## Integration Closure

S02 consumes S01 contract, gold corpus, and validator. It produces baseline diagnostics and review samples for S03. It must not implement improved chunking or claim final import readiness.

## Verification

- S02 adds redacted per-paper/per-chunk diagnostics: package validity, import_ready, route/state counts, source-span coverage, parent/evidence reference resolution, refusal counts, and missing-artifact blockers.

## Tasks

- [x] **T01: Build baseline package validator** `est:1d`
  Implement a read-only baseline package builder that maps current available paper/full-text/PageIndex chunk artifacts into S01 import-ready package dictionaries and runs `validate_import_ready_package`. It should be conservative: missing source spans, unresolved parents, missing artifacts, or current chunks without graph-grade metadata should become structured diagnostics rather than guessed fixes.
  - Files: `src/arxiv_archive/chunk_baseline_measurement.py`, `tests/test_chunk_baseline_measurement.py`
  - Verify: uv run pytest tests/test_chunk_baseline_measurement.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_baseline_measurement.py tests/test_chunk_baseline_measurement.py

- [x] **T02: Run baseline over gold corpus** `est:0.5d`
  Run the baseline package builder over the S01 gold corpus manifest. Emit JSON diagnostics for each paper and aggregate summary. The run must record missing artifact blockers, not silently skip papers; it must keep `raw_text_included=false`, `embeddings_included=false`, `production_import_attempted=false`, and `ladybugdb_written=false`.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl`, `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json`
  - Verify: uv run python -m arxiv_archive.chunk_baseline_measurement --manifest .gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json --output-dir .gsd/milestones/M005-dlko4z/slices/S02/run-evidence && test -s .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json && test -s .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl

- [x] **T03: Create baseline review samples** `est:0.5d`
  Generate bounded review samples for the six-paper inner review minimum where artifacts are available, and explicit blocker records where they are not. Review samples may include bounded snippets only in markdown review artifacts; machine JSON/JSONL diagnostics must remain redacted.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S02/review/baseline-review-samples.md`, `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/review-sample-index.json`
  - Verify: test -s .gsd/milestones/M005-dlko4z/slices/S02/review/baseline-review-samples.md && uv run python - <<'PY'
import json
from pathlib import Path
p=Path('.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/review-sample-index.json')
data=json.loads(p.read_text())
assert data['schema_version']=='m005-baseline-review-sample-index.v1'
assert data['raw_text_in_machine_logs'] is False
PY

- [ ] **T04: Report baseline chunk quality** `est:0.5d`
  Write the S02 baseline report and run independent review. The report must state current chunking import-readiness failures, missing-artifact blockers, route/state/refusal distributions, and explicit non-claims: no improved chunking yet, no production import, no final import readiness, no broad corpus scaling.
  - Files: `.gsd/milestones/M005-dlko4z/slices/S02/baseline-chunk-quality-report.md`, `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-review-summary.md`
  - Verify: uv run pytest tests/test_chunk_baseline_measurement.py tests/test_chunk_import_contract.py -q && test -s .gsd/milestones/M005-dlko4z/slices/S02/baseline-chunk-quality-report.md && test -s .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-review-summary.md

## Files Likely Touched

- src/arxiv_archive/chunk_baseline_measurement.py
- tests/test_chunk_baseline_measurement.py
- .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl
- .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json
- .gsd/milestones/M005-dlko4z/slices/S02/review/baseline-review-samples.md
- .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/review-sample-index.json
- .gsd/milestones/M005-dlko4z/slices/S02/baseline-chunk-quality-report.md
- .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-review-summary.md
