---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Run thirty paper deviation scan

Run the 30-paper deviation scanner and persist summary plus per-paper diagnostics under S03 run-evidence. Confirm all 30 Markdown-ready papers are represented and no safety flags are enabled.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `.gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json`

## Expected Output

- `.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json`
- `.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-diagnostics.jsonl`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
summary=json.loads(Path('.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json').read_text())
assert summary['paper_count']==30
assert summary['raw_text_included'] is False
assert summary['production_import_attempted'] is False
assert Path('.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-diagnostics.jsonl').stat().st_size > 0
print(summary)
PY

## Observability Impact

Run evidence becomes the authoritative 30-paper Markdown-based deviation dataset.
