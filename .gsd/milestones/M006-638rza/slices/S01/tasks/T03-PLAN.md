---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T03: Reported 30-paper corpus readiness and found source acquisition is required for a meaningful full scan.

Write the S01 availability/rationale report, highlighting whether 30 papers are viable for S02, which missing-source gaps are expected blockers, and what deviation categories are likely to be interesting.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json`

## Expected Output

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-availability-report.md`

## Verification

test -s .gsd/milestones/M006-638rza/slices/S01/thirty-paper-availability-report.md && uv run python - <<'PY'
from pathlib import Path
text=Path('.gsd/milestones/M006-638rza/slices/S01/thirty-paper-availability-report.md').read_text()
assert '30-paper' in text or 'thirty-paper' in text
assert 'M005 overlap' in text
print('report-ok')
PY

## Observability Impact

Report gives S02 a clear go/partial-go decision and separates corpus availability deviations from model deviations.
