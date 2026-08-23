# M178 Residual Script Drift and Cache Closeout

## Verdict

**M178 status: PASS.**

M178 completed all three requested directions in one bounded GSD milestone: a next exact script-only family wave, stricter inventory drift CI policy, and dedicated cache-coordination review.

## Included directions

| Direction | Result | Evidence |
|---|---|---|
| 1. Next exact script-only family wave | Implemented: 28 records moved | `m178-script-wave-result.md`, final delta |
| 2. Stricter inventory delta CI drift policy | Implemented: strict drift against final baseline | `.github/workflows/architecture-guardrail.yml`, `m178-ci-drift-final-recheck.md` |
| 3. Dedicated cache-coordination review | Completed as conservative no-move policy | `m178-cache-coordination-review.md` |

## Category movement

```text
script-only: 198 -> 170
records moved from script-only: 28
unknown=0
shared-state=0
total_records=341
```

New categories:

```text
m027-pipeline-replay-output=14
m025-recovery-evidence-output=14
```

## CI drift policy

Architecture guardrail CI now runs `Run write-path inventory drift check`. When `data/architecture-assessment/m178-write-path-inventory-final.json` exists, the command enforces zero total/category drift. It writes generated current JSON, markdown, and delta markdown only to temporary files.

Strict recheck result:

```text
strict_mode=1
unknown=0
shared-state=0
total_delta=+0
all_category_deltas=+0
```

## Cache coordination

Cache-like and markdown-like records were reviewed. M178 adds no broad cache category. Reviewed outputs remain `caller-owned`, exact package categories, or conservative `script-only` based on exact ownership. Stable shared cache/index files require future lifecycle review before scanner movement.

## Verification

Integrated verification:

```text
focused inventory tests=20 passed
test architecture guard=dynamic=0, legacy=0, violations=0
onion guard=violation_count=0, allowed_violation_count=0
final artifact assertions=PASS
strict CI drift final baseline=PASS
```

Quality stack:

```text
scoped ruff=PASS
pyrefly=0 errors
pre-commit=PASS
GitNexus detect_changes=LOW risk, affected_processes=0
scope hygiene=expected M178 files only
```

## Decisions

- D100: M178 moves exact M027/M025 script families, upgrades inventory CI to strict drift against committed final inventory, and completes cache coordination as conservative no-move unless exact shared cache ownership is proven.

## Residual risks

1. Pre-edit GitNexus impact remained UNKNOWN because scanner/workflow targets did not resolve authoritatively.
2. `script-only=170` remains a residual bucket for future exact family waves.
3. Strict CI drift now fails on scanner/category changes unless the committed final inventory baseline is intentionally updated.
4. Cache coordination remains conservative for future stable shared cache/index files.

## Follow-ups

Recommended next GSD scopes:

1. Continue exact residual script waves against `script-only=170`, likely M031/M033 parser and external-parser verifier families.
2. Decide whether to promote a non-milestone canonical inventory baseline path so CI drift is not tied to the latest milestone artifact.
3. If stable cache/index ownership appears, run a dedicated cache lifecycle and concurrency review before scanner movement.
