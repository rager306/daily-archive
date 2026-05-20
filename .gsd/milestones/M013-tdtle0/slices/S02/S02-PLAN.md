# S02: DSPy optimizer applicability catalog

**Goal:** Catalog DSPy optimization algorithms and assess which, if any, are applicable to Scientific KG extraction after required metrics/data gates.
**Demo:** A DSPy optimizer catalog explains each optimizer family and rates applicability to daily-archive with exact preconditions and blocked uses.

## Must-Haves

- Catalog includes BootstrapFewShot, random/search variants, MIPRO, GEPA, COPRO, SIMBA/BetterTogether where present, and any other relevant local optimizers.
- Each optimizer has applicability rating: blocked, future-only, possible-dev, or not-applicable.
- Required metrics/devsets and trace/redaction risks are explicit.
- No optimizer is run.

## Proof Level

- This slice proves: Local source/GitNexus/docs research with independent guard.

## Integration Closure

Provides optimizer decision evidence for final recommendation.

## Verification

- Records optimizer names, source paths, risk/cost/trace considerations, metrics required, and applicability ratings.

## Tasks

- [x] **T01: Inventory DSPy optimizers** `est:medium`
  Inventory DSPy optimizer classes/modules from local vendor source and GitNexus, including source paths and broad algorithm families.
  - Files: `.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-inventory.json`
  - Verify: test -s .gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-inventory.json

- [x] **T02: Assess optimizer applicability** `est:medium`
  Assess each optimizer's applicability to daily-archive Scientific KG extraction, including metric/devset needs, cost/risk, and allowed/blocked status.
  - Files: `.gsd/milestones/M013-tdtle0/slices/S02/dspy-optimizer-applicability-catalog.md`, `.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-applicability.json`
  - Verify: test -s .gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-applicability.json

- [x] **T03: Write optimizer applicability guard** `est:small`
  Write optimizer guard proving no optimizer was run and summarizing which optimizer families are future-only versus blocked.
  - Files: `.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-guard.json`
  - Verify: test -s .gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-guard.json').read_text())
assert g['optimizer_executed'] is False
assert g['production_import_allowed'] is False
print('dspy-optimizer-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-inventory.json
- .gsd/milestones/M013-tdtle0/slices/S02/dspy-optimizer-applicability-catalog.md
- .gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-applicability.json
- .gsd/milestones/M013-tdtle0/slices/S02/run-evidence/dspy-optimizer-guard.json
