---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Report deviations against M005 baseline

Compare 30-paper distributions against M005 S06/S07 baseline. Identify new/high-frequency refusal patterns, route shifts, per-paper outliers, conversion-method caveats, source/PDF caveats, and any changed implications for remediation.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-summary.json`

## Expected Output

- `.gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md`

## Verification

test -s .gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md && uv run python - <<'PY'
from pathlib import Path
text=Path('.gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md').read_text()
assert 'M005' in text
assert '30' in text
assert 'deviation' in text.lower()
print('deviation-report-ok')
PY

## Observability Impact

Report provides human-readable pattern taxonomy and remediation candidates for review.
