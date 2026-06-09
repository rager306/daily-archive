---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Ran bounded source acquisition and brought the 30-paper corpus to 30/30 Markdown-ready.

Run the bounded acquisition/conversion helper over the 20 missing-Markdown expansion papers. Persist updated availability summary and diagnostics under S02 run-evidence.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `.gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md`

## Expected Output

- `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json`
- `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl`

## Verification

uv run python - <<'PY'
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

## Observability Impact

Run evidence shows per-paper outcome, readiness delta, missing counts, and no-import/no-write safety flags.
