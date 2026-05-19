# S02: Bounded source acquisition for thirty paper scan

**Goal:** Attempt bounded source acquisition/conversion for the 20 expansion papers missing Markdown, persist redacted diagnostics, and make the 30-paper corpus ready or explicitly blocked per paper for later deviation analysis.
**Demo:** After this slice, the 20 missing-Markdown expansion papers have bounded acquisition/conversion attempts with redacted diagnostics, and the 30-paper corpus is either source-ready or explicitly blocked per paper.

## Must-Haves

- All 20 missing-Markdown expansion papers have explicit acquisition/conversion attempts or documented skip reasons.
- Per-paper diagnostics record method, phase, outcome, paths/hashes/sizes where available, and redacted errors.
- The updated availability summary shows how many of 30 are now Markdown-ready.
- No KG import, production LadybugDB writes, embeddings, vectors, raw text/chunk text, secrets, or optimizer traces are emitted.
- S03 can consume either 30 ready papers or an explicit blocked-per-paper list.

## Proof Level

- This slice proves: Automated source-acquisition/conversion dry run with artifact guards, focused tests where code is added, and summary showing readiness deltas.

## Integration Closure

Consumes S01 corpus manifest and availability diagnostics. Produces source acquisition/conversion artifacts and updated availability evidence for S03 deviation analysis.

## Verification

- Adds per-paper acquisition phase, method, outcome, output path, source path, error class, duration, and safety flags without raw text/chunk text in machine logs.

## Tasks

- [x] **T01: Define bounded source acquisition plan** `est:small`
  Inspect existing full-text/download/conversion code paths and define a bounded acquisition plan for the 20 missing-Markdown papers. The plan should prefer already-supported project mechanisms and avoid unbounded Marker/LLM/optimizer behavior.
  - Files: `.gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md`
  - Verify: test -s .gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md && grep -q 'bounded' .gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md

- [x] **T02: Implement bounded source acquisition helper** `est:large`
  Implement a reusable 30-paper source acquisition/audit helper or script that attempts Markdown/PDF acquisition for missing papers using bounded project mechanisms. It must write per-paper redacted diagnostics and avoid raw text in JSON/JSONL.
  - Files: `src/arxiv_archive/thirty_paper_source_scan.py`, `tests/test_thirty_paper_source_scan.py`
  - Verify: uv run pytest tests/test_thirty_paper_source_scan.py -q && uv run ruff check src/arxiv_archive/thirty_paper_source_scan.py tests/test_thirty_paper_source_scan.py

- [x] **T03: Run source acquisition for missing papers** `est:large`
  Run the bounded acquisition/conversion helper over the 20 missing-Markdown expansion papers. Persist updated availability summary and diagnostics under S02 run-evidence.
  - Files: `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json`, `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl`
  - Verify: uv run python - <<'PY'
import json
from pathlib import Path
summary=json.loads(Path('.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json').read_text())
assert summary['paper_count']==30
assert summary['attempted_missing_markdown_count'] == 20
assert summary['raw_text_included'] is False
assert summary['production_import_attempted'] is False
assert Path('.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl').stat().st_size > 0
print(summary)
PY

- [x] **T04: Report source readiness delta** `est:small`
  Write a readiness delta report showing what changed after acquisition attempts, which papers are still blocked, and whether S03 can run a full 30-paper deviation analysis or must separate source blockers from chunking results.
  - Files: `.gsd/milestones/M006-638rza/slices/S02/source-acquisition-report.md`
  - Verify: test -s .gsd/milestones/M006-638rza/slices/S02/source-acquisition-report.md && uv run python - <<'PY'
from pathlib import Path
text=Path('.gsd/milestones/M006-638rza/slices/S02/source-acquisition-report.md').read_text()
assert 'readiness' in text.lower()
assert '30' in text
print('source-report-ok')
PY

## Files Likely Touched

- .gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md
- src/arxiv_archive/thirty_paper_source_scan.py
- tests/test_thirty_paper_source_scan.py
- .gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json
- .gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl
- .gsd/milestones/M006-638rza/slices/S02/source-acquisition-report.md
