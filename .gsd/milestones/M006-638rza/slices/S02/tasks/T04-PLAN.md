---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T04: Report source readiness delta

Write a readiness delta report showing what changed after acquisition attempts, which papers are still blocked, and whether S03 can run a full 30-paper deviation analysis or must separate source blockers from chunking results.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json`
- `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json`

## Expected Output

- `.gsd/milestones/M006-638rza/slices/S02/source-acquisition-report.md`

## Verification

test -s .gsd/milestones/M006-638rza/slices/S02/source-acquisition-report.md && uv run python - <<'PY'
from pathlib import Path
text=Path('.gsd/milestones/M006-638rza/slices/S02/source-acquisition-report.md').read_text()
assert 'readiness' in text.lower()
assert '30' in text
print('source-report-ok')
PY

## Observability Impact

Report provides S03 go/partial-go decision and distinguishes source acquisition deviations from chunking deviations.
