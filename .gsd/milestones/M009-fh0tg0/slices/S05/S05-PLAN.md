# S05: S05

**Goal:** Independently review provenance, freshness, lineage, and top-up hardening and produce the next-batch go/no-go recommendation.
**Demo:** After this slice, review says whether provenance and top-up hardening are sufficient to run the next reviewed +10 batch.

## Must-Haves

- Review checks provenance schema, verifier positive/negative cases, lineage metadata, and top-up pass/block artifacts.
- Recommendation explicitly says whether another +10 may run.
- Any remaining gaps are surfaced as follow-up requirements.
- Positive KG import and production writes remain blocked.

## Proof Level

- This slice proves: Independent review plus final artifact/test guard.

## Integration Closure

Consumes S01-S04 hardening artifacts and produces a final recommendation for whether another +10 validation batch may run.

## Verification

- Adds review summary, final recommendation, and final guard for provenance/top-up hardening.

## Tasks

- [x] **T01: Independent review flagged that M009 hardening is useful but still requires explicit next-batch runbook gates.** `est:medium`
  Run independent review over M009 S01-S04 code and artifacts. Focus on whether provenance/freshness/top-up hardening is enough to permit another reviewed +10 batch.
  - Files: `.gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/hardening-review-summary.md`
  - Verify: test -s .gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/hardening-review-summary.md && grep -Fq 'Verdict:' .gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/hardening-review-summary.md

- [x] **T02: Wrote final recommendation: next +10 may proceed only with explicit provenance, lineage, and top-up gates.** `est:small`
  Write final recommendation: whether to proceed to the next reviewed +10 batch, and under what required invocation gates.
  - Files: `.gsd/milestones/M009-fh0tg0/slices/S05/hardening-final-recommendation.md`
  - Verify: test -s .gsd/milestones/M009-fh0tg0/slices/S05/hardening-final-recommendation.md && grep -Fq 'positive KG import remains blocked' .gsd/milestones/M009-fh0tg0/slices/S05/hardening-final-recommendation.md

- [x] **T03: Final hardening guard passed with FLAG review and explicit next-batch gates.** `est:small`
  Run final guard across provenance, verifier, lineage, and top-up artifacts plus focused tests.
  - Files: `.gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/final-hardening-guard.json`
  - Verify: test -s .gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/final-hardening-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/final-hardening-guard.json').read_text())
assert g['freshness_pass_verdict']=='fresh'
assert g['freshness_stale_verdict']=='stale'
assert g['lineage_mismatch_verdict']=='stale'
assert g['top_up_pass_scan_allowed'] is True
assert g['top_up_blocked_scan_allowed'] is False
print('final-hardening-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/hardening-review-summary.md
- .gsd/milestones/M009-fh0tg0/slices/S05/hardening-final-recommendation.md
- .gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/final-hardening-guard.json
