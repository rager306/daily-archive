# S02: Preflight and bounded top up source quota

**Goal:** Run init/preflight and bounded source acquisition for the M010 next +10, then use quota/top-up gates to reach source-ready quota or block scan explicitly.
**Demo:** After this slice, the selected batch has source-ready quota 10/10 or an explicit bounded shortage blocker.

## Must-Haves

- validation-batch init/preflight run on M010 manifest.
- Initial source gaps are recorded.
- Bounded acquisition attempts missing Markdown.
- Final preflight reaches ready_for_markdown_scan_count=10 or top-up/blocker artifacts explain why not.
- If top-up is needed, replacements are materialized/preflighted before scan.
- No production import/write flags are true.

## Proof Level

- This slice proves: CLI artifact guards plus source-ready quota/top-up verification.

## Integration Closure

Consumes S01 manifest and produces a final source-ready batch-state, quota/top-up evidence, or an explicit blocker for S03.

## Verification

- Adds initial/final preflight, acquisition, quota-fill, and top-up artifacts with no raw text.

## Tasks

- [x] **T01: Run initial M010 init and preflight** `est:small`
  Run validation-batch init and initial preflight for M010 S01 manifest. Persist responses and initial preflight artifacts.
  - Files: `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/init-response.json`, `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-summary.json`, `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-diagnostics.jsonl`
  - Verify: test -s .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('initial-preflight-ok')
PY

- [x] **T02: Run bounded M010 source acquisition** `est:medium`
  Run bounded fast-only source acquisition for selected M010 papers. Persist acquisition summary/diagnostics; do not use unbounded conversion.
  - Files: `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-summary.json`, `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl`
  - Verify: test -s .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
s=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-summary.json').read_text())
assert s['paper_count']==10
assert s['production_import_attempted'] is False
assert s['ladybugdb_written'] is False
print('source-acquisition-ok')
PY

- [x] **T03: Refresh preflight and quota gate** `est:medium`
  Rerun final preflight, build quota-fill evidence, and if quota remains short produce bounded top-up plan/blocker. S03 may proceed only if final source-ready quota is 10.
  - Files: `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-preflight-summary.json`, `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/quota-fill-summary.json`, `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/top-up-summary.json`, `.gsd/milestones/M010-06v9ke/slices/S02/source-readiness-report.md`
  - Verify: test -s .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/quota-fill-summary.json && uv run python - <<'PY'
import json
from pathlib import Path
q=json.loads(Path('.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/quota-fill-summary.json').read_text())
assert q['target_count']==10
assert q['raw_text_included'] is False
print('quota-gate-ok')
PY

## Files Likely Touched

- .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/init-response.json
- .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-summary.json
- .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/initial-source-preflight-diagnostics.jsonl
- .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-summary.json
- .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-acquisition-diagnostics.jsonl
- .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-preflight-summary.json
- .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/quota-fill-summary.json
- .gsd/milestones/M010-06v9ke/slices/S02/run-evidence/top-up-summary.json
- .gsd/milestones/M010-06v9ke/slices/S02/source-readiness-report.md
