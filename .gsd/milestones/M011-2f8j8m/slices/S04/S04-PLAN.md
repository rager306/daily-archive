# S04: S04

**Goal:** Synthesize semantic gate evidence into a final import-readiness recommendation and update requirements accordingly.
**Demo:** A final recommendation states whether positive import remains blocked and what exact next milestone is justified by the semantic evidence.

## Must-Haves

- Final guard consolidates selection, rubric, judgments, review verdict, and safety flags.
- R038 is updated with evidence status.
- Positive import, production writes, and unattended scaling are still blocked or deferred to an explicitly scoped future rehearsal.
- Fresh verification passes before milestone completion.

## Proof Level

- This slice proves: Final guard, focused artifact checks, milestone validation.

## Integration Closure

Consumes S03 review and closes M011 with validated or active requirement outcomes.

## Verification

- Adds final guard, recommendation, validation, and milestone summary.

## Tasks

- [x] **T01: Wrote final M011 recommendation: PASS negative gate, import still blocked pending chunk-span provenance.** `est:small`
  Write final M011 recommendation and guard stating that the semantic gate passed as a negative readiness gate and defining the next required evidence for any future positive import rehearsal.
  - Files: `.gsd/milestones/M011-2f8j8m/slices/S04/m011-final-recommendation.md`, `.gsd/milestones/M011-2f8j8m/slices/S04/run-evidence/final-semantic-gate-guard.json`
  - Verify: test -s .gsd/milestones/M011-2f8j8m/slices/S04/run-evidence/final-semantic-gate-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M011-2f8j8m/slices/S04/run-evidence/final-semantic-gate-guard.json').read_text())
assert g['review_verdict']=='PASS'
assert g['import_candidate_count']==0
assert g['positive_import_blocked'] is True
print('final-semantic-gate-guard-ok')
PY

- [x] **T02: Updated R038 and verified the final M011 negative semantic gate evidence.** `est:small`
  Update R038 and run final milestone artifact verification before milestone validation/completion.
  - Files: `.gsd/milestones/M011-2f8j8m/slices/S04/run-evidence/final-verification.json`
  - Verify: test -s .gsd/milestones/M011-2f8j8m/slices/S04/run-evidence/final-verification.json

## Files Likely Touched

- .gsd/milestones/M011-2f8j8m/slices/S04/m011-final-recommendation.md
- .gsd/milestones/M011-2f8j8m/slices/S04/run-evidence/final-semantic-gate-guard.json
- .gsd/milestones/M011-2f8j8m/slices/S04/run-evidence/final-verification.json
