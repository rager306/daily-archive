# S04: S04

**Goal:** Independently review the quota-filled first new +10 batch evidence and produce a recommendation for the next validation step.
**Demo:** After this slice, independent review says whether the quota-filled first new +10 batch is good enough to continue another +10, needs fixes, or blocks progression.

## Must-Haves

- Review checks quota-fill evidence, source readiness, scan artifacts, safety flags, and PDF caveat.
- Review verdict distinguishes operational workflow readiness from semantic KG readiness.
- Final recommendation says whether to run another +10, implement top-up automation, or block.
- Positive KG import and production writes remain blocked.
- No raw paper/chunk text is embedded in review artifacts.

## Proof Level

- This slice proves: Independent artifact review plus final guard checks.

## Integration Closure

Consumes S01 selection, S02 source preflight/acquisition, and S03 quota/scan artifacts. Produces review summary and final recommendation for milestone validation.

## Verification

- Adds independent review summary and final recommendation that explicitly calls out quota-fill behavior, source caveats, scan results, and remaining import blocks.

## Tasks

- [x] **T01: Independent review flagged top-up automation and stale metadata gaps while accepting current M008 scan as safe operational evidence.** `est:medium`
  Run independent review of M008 S01-S03 artifacts. Focus on whether quota-fill evidence is meaningful, source readiness is honestly represented, scan artifacts are redacted, and claims do not exceed evidence.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S04/run-evidence/new-plus-ten-review-summary.md`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S04/run-evidence/new-plus-ten-review-summary.md && grep -Fq 'Verdict:' .gsd/milestones/M008-c9zb94/slices/S04/run-evidence/new-plus-ten-review-summary.md

- [x] **T02: Wrote final recommendation: close M008, but add bounded top-up automation before another +10.** `est:small`
  Write final recommendation based on review: continue another +10, add bounded top-up automation first, or block progression. Keep import and production writes blocked.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S04/new-plus-ten-final-recommendation.md`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S04/new-plus-ten-final-recommendation.md && grep -Fq 'positive KG import remains blocked' .gsd/milestones/M008-c9zb94/slices/S04/new-plus-ten-final-recommendation.md

- [x] **T03: Final S04 guard passed and records the FLAG review plus next-batch top-up requirement.** `est:small`
  Run final artifact guards for M008 S04 and milestone-ready status: quota accepted count, scan count, import gate, no-write/no-import flags, and review/recommendation presence.
  - Files: `.gsd/milestones/M008-c9zb94/slices/S04/run-evidence/final-review-guard.json`
  - Verify: test -s .gsd/milestones/M008-c9zb94/slices/S04/run-evidence/final-review-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M008-c9zb94/slices/S04/run-evidence/final-review-guard.json').read_text())
assert g['quota_ready']==10
assert g['paper_count']==10
assert g['import_eligible_chunk_count']==0
assert g['production_import_attempted'] is False
assert g['ladybugdb_written'] is False
print('final-review-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M008-c9zb94/slices/S04/run-evidence/new-plus-ten-review-summary.md
- .gsd/milestones/M008-c9zb94/slices/S04/new-plus-ten-final-recommendation.md
- .gsd/milestones/M008-c9zb94/slices/S04/run-evidence/final-review-guard.json
