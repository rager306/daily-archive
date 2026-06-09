---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Audited source availability and found the first major deviation: 20 expansion papers lack Markdown source artifacts.

Audit local availability for the 30 selected papers: normalized Markdown, original PDF, research workspace, and known derived artifacts. Summarize missing-source patterns separately from chunking/import-model issues. External filesystem roots may be inspected during execution but only redacted status/path metadata is written.

## Inputs

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`

## Expected Output

- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json`
- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl`

## Verification

uv run python - <<'PY'
import json
from pathlib import Path
summary=json.loads(Path('.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json').read_text())
assert summary['paper_count']==30
assert summary['raw_text_included'] is False
assert summary['production_import_attempted'] is False
assert Path('.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl').stat().st_size > 0
print(summary)
PY

## Observability Impact

Availability summary records missing markdown/PDF/workspace counts and per-paper diagnostics without raw content.
