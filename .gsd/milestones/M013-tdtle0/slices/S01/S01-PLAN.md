# S01: S01

**Goal:** Check DSPy dependency and import feasibility in isolation without mutating project runtime dependencies or enabling optimizers.
**Demo:** An isolated DSPy dependency probe artifact reports install/import/no-LM status and whether Predict/Evaluate can be exercised without external LM calls.

## Must-Haves

- Probe does not mutate project pyproject/lock unless explicitly recorded as no-op.
- DSPy dependency resolution/install/import status is documented.
- If import succeeds, no-LM Predict/Evaluate behavior is probed with synthetic input only.
- No external LM, optimizer, production import, or LadybugDB write occurs.

## Proof Level

- This slice proves: Isolated command output plus artifact guard.

## Integration Closure

Provides dependency readiness evidence for optimizer applicability and final recommendation.

## Verification

- Records environment path, install command, versions, import status, no-LM probe status, and safety flags.

## Tasks

- [x] **T01: Installed DSPy in an isolated temporary venv without mutating project dependency files.** `est:medium`
  Create a temporary isolated Python environment outside the project, install DSPy from local `/root/vendor-source/dspy` or equivalent, and record dependency resolution without editing daily-archive dependency files.
  - Files: `.gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-dependency-install.json`
  - Verify: test -s .gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-dependency-install.json

- [x] **T02: Ran DSPy no-LM probe: import succeeded, Predict failed closed without LM, static Evaluate succeeded.** `est:medium`
  If isolated install succeeds, run synthetic no-LM DSPy import/Predict/Evaluate probe. Confirm no LM, optimizer, file write, production import, or LadybugDB write occurs.
  - Files: `.gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-no-lm-probe.json`
  - Verify: test -s .gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-no-lm-probe.json

- [x] **T03: Wrote DSPy dependency readiness guard allowing optional/dev prototype but blocking production runtime and optimizers.** `est:small`
  Write a dependency readiness guard summarizing whether DSPy can proceed to optional/dev prototype, and what remains blocked.
  - Files: `.gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-dependency-guard.json`
  - Verify: test -s .gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-dependency-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-dependency-guard.json').read_text())
assert g['project_dependency_files_modified'] is False
assert g['optimizer_executed'] is False
print('dspy-dependency-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-dependency-install.json
- .gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-no-lm-probe.json
- .gsd/milestones/M013-tdtle0/slices/S01/run-evidence/dspy-dependency-guard.json
