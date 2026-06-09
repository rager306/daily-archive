# S01: S01

**Goal:** Research and, where safe, probe DSPy compatibility with the existing Scientific KG extraction boundaries without enabling optimizers or production extraction.
**Demo:** A DSPy compatibility report exists with package/API requirements, minimal invocation path, fail-closed optimizer policy, and exact blockers/preconditions.

## Must-Haves

- Current DSPy install/import/API path is documented.
- Minimal invocation or dry-run boundary is described or probed.
- Optimizer behavior remains disabled and fail-closed.
- Compatibility with ExtractionPatch-style outputs and evaluation metrics is assessed.
- No trusted facts, production import, or LadybugDB writes occur.
- Research uses GitNexus/project context, `../vendor-source/dspy` (`/root/vendor-source/dspy`), and 2026 best practices beyond the repo.

## Proof Level

- This slice proves: Current docs plus local package/probe evidence where feasible.

## Integration Closure

Produces DSPy compatibility artifact for the combined integration matrix.

## Verification

- Records commands, versions, docs consulted, probe result, failure modes, and no-import/no-write guard.

## Tasks

- [x] **T01: Researched DSPy compatibility and best practices; result is optional/dev only, no production activation.** `est:medium`
  Use GitNexus project context, `../vendor-source/dspy`, and current external research to document DSPy package/API, signatures/modules/metrics/evaluation, optimizer discipline, observability, and production best practices relevant to Scientific KG extraction.
  - Files: `.gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-research-report.md`
  - Verify: test -s .gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-research-report.md

- [x] **T02: DSPy local probe found version 3.2.1 but import is currently blocked by missing `cloudpickle`.** `est:medium`
  Inspect local environment and DSPy source for install/import feasibility. If safe and dependency is available, run an import/version/minimal dry-run probe that does not call external LMs or optimizers. Otherwise document why probe is skipped.
  - Files: `.gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-probe.json`
  - Verify: test -s .gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-probe.json

- [x] **T03: DSPy guard written: optional/dev prototype allowed later, production runtime and optimizers blocked now.** `est:small`
  Synthesize DSPy findings into a compatibility guard with go/no-go, preconditions, and blocked behaviors for S03 matrix.
  - Files: `.gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-compatibility-guard.json`, `.gsd/milestones/M012-a7v8fw/slices/S01/dspy-compatibility-summary.md`
  - Verify: test -s .gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-compatibility-guard.json && uv run python - <<'PY'
import json
from pathlib import Path
g=json.loads(Path('.gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-compatibility-guard.json').read_text())
assert g['production_import_attempted'] is False
assert g['optimizer_enabled'] is False
print('dspy-compatibility-guard-ok')
PY

## Files Likely Touched

- .gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-research-report.md
- .gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-probe.json
- .gsd/milestones/M012-a7v8fw/slices/S01/run-evidence/dspy-compatibility-guard.json
- .gsd/milestones/M012-a7v8fw/slices/S01/dspy-compatibility-summary.md
