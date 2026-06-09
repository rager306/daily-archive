# S02: S02

**Goal:** Initialize and source-preflight the new +10 batch using the M007 validation-batch CLI, then use bounded source acquisition/repair only if needed to make scan readiness explicit.
**Demo:** After this slice, the new +10 batch has M007 batch-state/source-preflight artifacts and any missing Markdown blockers are either repaired with bounded steps or explicitly block scan.

## Must-Haves

- validation-batch init runs for the new +10 manifest.
- validation-batch preflight writes batch-state/source-preflight artifacts.
- Missing Markdown is handled via bounded acquisition/repair or explicitly blocks scan.
- Final refreshed preflight states whether S03 may scan.
- Production import/write flags remain false.
- No raw paper text or chunk text is serialized.

## Proof Level

- This slice proves: CLI artifact run plus bounded acquisition/repair evidence and guard checks.

## Integration Closure

Consumes S01 new +10 manifest and produces batch-state/source-preflight artifacts for S03 scan, or an explicit blocker if sources cannot be made ready.

## Verification

- Adds init/preflight responses, source-preflight summaries/diagnostics, source-acquisition summaries/diagnostics if needed, and a source readiness report.

## Tasks

- [x] **T01: Initialized and preflighted the new +10 batch; only 1/10 is initially Markdown-ready.** `est:small`
  Run validation-batch init and initial preflight against the M008 new +10 manifest. Persist init/preflight responses and summarize initial readiness without acquisition.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/init-response.json`, `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-summary.json`, `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-diagnostics.jsonl`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('initial-preflight-ok')
PY

- [x] **T02: Bounded acquisition made the new +10 batch 10/10 Markdown-ready via arxiv2md.** `est:medium`
  If initial preflight is not source-ready, run bounded Markdown acquisition over the new +10 manifest using existing source acquisition helper. Prefer fast arxiv2md first; do not run unbounded conversion loops. Persist acquisition summary and diagnostics.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-summary.json`, `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('source-acquisition-ok')
PY

- [x] **T03: Final preflight confirms the new +10 batch is 10/10 Markdown-ready with 0 blockers.** `est:small`
  Rerun validation-batch preflight after bounded acquisition, write final preflight artifacts and source readiness report. If any Markdown remains missing, mark S03 blocked rather than scanning.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/batch-state.json`, `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-summary.json`, `.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-diagnostics.jsonl`, `.gsd/milestones/M008-c9zb94/slices/S02/source-preflight-report.md`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('final-preflight-ok')
PY

## Files Likely Touched

- .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/init-response.json
- .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-summary.json
- .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/initial-source-preflight-diagnostics.jsonl
- .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-summary.json
- .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl
- .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/batch-state.json
- .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-summary.json
- .gsd/milestones/M008-c9zb94/slices/S02/run-evidence/source-preflight-diagnostics.jsonl
- .gsd/milestones/M008-c9zb94/slices/S02/source-preflight-report.md
