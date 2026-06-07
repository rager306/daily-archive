---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T02: Draft status matrix failure taxonomy and dependency model

Create STATUS-MATRIX.md, FAILURE-TAXONOMY.md, and ARTIFACT-DEPENDENCY-MODEL.md describing status transitions, retryable/terminal/blocked failures, stale detection, sidecar dependency graph, and redacted diagnostics.

## Inputs

- `.gsd/milestones/M034-kuei9y/decision-package/ADR-003-durable-lazy-async-evidence-pipeline.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ADR-004-sidecars-as-candidate-evidence-producers.md`

## Expected Output

- `.gsd/milestones/M034-kuei9y/decision-package/STATUS-MATRIX.md`
- `.gsd/milestones/M034-kuei9y/decision-package/FAILURE-TAXONOMY.md`
- `.gsd/milestones/M034-kuei9y/decision-package/ARTIFACT-DEPENDENCY-MODEL.md`

## Verification

Check files include pending/ready/running/succeeded/failed_retryable/failed_terminal/blocked/stale/needs_review statuses, failure codes, sidecar dependency graph, and redaction constraints.

## Observability Impact

Status and failure docs define future debugging surfaces.
