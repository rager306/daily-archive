# S03: Deviation and pattern analysis

**Goal:** Run Markdown-based structure/import-model deviation analysis over the now Markdown-ready 30-paper corpus, compare against M005 10-paper baseline, and identify outliers and new patterns without authorizing KG import.
**Demo:** After this slice, there is a deviation report comparing 30-paper behavior against M005 baseline and identifying new patterns/outliers.

## Must-Haves

- 30-paper Markdown scan runs or records explicit per-paper blockers.
- Redacted artifacts include per-paper counts, aggregate route/type/state/refusal distributions, and outlier signals.
- Comparison against M005 baseline identifies new deviations and recurring patterns.
- PDF/source caveats are separated from Markdown chunking/import-model deviations.
- No raw text/chunk text, embeddings, vectors, production writes, or positive import claims are emitted.

## Proof Level

- This slice proves: Deterministic analysis script/helper, focused tests, artifact guards, and report comparing 30-paper evidence against M005 baseline.

## Integration Closure

Consumes S01/S02 corpus and source-readiness artifacts plus M005 S06/S07 baseline summaries. Produces S03 redacted deviation run evidence and analysis report for S04 independent review.

## Verification

- Adds per-paper/per-route distributions, outlier lists, baseline-vs-30 deltas, conversion/source caveats, and safety flags for the 30-paper scan.

## Tasks

- [x] **T01: Implement thirty paper deviation scanner** `est:large`
  Implement a deterministic 30-paper deviation analysis helper that consumes the M006 manifest and available Markdown sources, reuses structure-aware chunking/package diagnostics where possible, and writes redacted per-paper metrics. The helper must not serialize raw Markdown/chunk text.
  - Files: `src/arxiv_archive/thirty_paper_deviation_scan.py`, `tests/test_thirty_paper_deviation_scan.py`
  - Verify: uv run pytest tests/test_thirty_paper_deviation_scan.py tests/test_structure_aware_chunking.py tests/test_chunking_benchmark.py -q && uv run ruff check src/arxiv_archive/thirty_paper_deviation_scan.py tests/test_thirty_paper_deviation_scan.py

- [ ] **T02: Run thirty paper deviation scan** `est:medium`
  Run the 30-paper deviation scanner and persist summary plus per-paper diagnostics under S03 run-evidence. Confirm all 30 Markdown-ready papers are represented and no safety flags are enabled.
  - Files: `.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json`, `.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-diagnostics.jsonl`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
summary=json.loads(Path('.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json').read_text())
assert summary['paper_count']==30
assert summary['raw_text_included'] is False
assert summary['production_import_attempted'] is False
assert Path('.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-diagnostics.jsonl').stat().st_size > 0
print(summary)
PY

- [ ] **T03: Report deviations against M005 baseline** `est:medium`
  Compare 30-paper distributions against M005 S06/S07 baseline. Identify new/high-frequency refusal patterns, route shifts, per-paper outliers, conversion-method caveats, source/PDF caveats, and any changed implications for remediation.
  - Files: `.gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md`
  - Verify: test -s .gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md && uv run python - <<'PY'
from pathlib import Path
text=Path('.gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md').read_text()
assert 'M005' in text
assert '30' in text
assert 'deviation' in text.lower()
print('deviation-report-ok')
PY

## Files Likely Touched

- src/arxiv_archive/thirty_paper_deviation_scan.py
- tests/test_thirty_paper_deviation_scan.py
- .gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json
- .gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-diagnostics.jsonl
- .gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md
