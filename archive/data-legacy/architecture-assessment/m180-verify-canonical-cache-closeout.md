# M180 Verify Canonical Cache Closeout

## Verdict

**M180 status: PASS.**

M180 completed all three requested directions in one bounded milestone: another exact residual `script-only` verify-family wave, canonical-only inventory drift CI cleanup, and dedicated cache lifecycle review.

## Included directions

| Direction | Result | Evidence |
|---|---|---|
| 1. Next exact script-only family wave | Implemented: 20 records moved | `m180-verify-wave-result.md`, final delta |
| 2. Canonical baseline CI soak and cleanup | Implemented: canonical-only strict drift | `.github/workflows/architecture-guardrail.yml`, `m180-canonical-baseline-final-check.md` |
| 3. Cache lifecycle review | Completed as conservative no-move review | `m180-cache-lifecycle-review.md` |

## Category movement

```text
script-only: 142 -> 122
records moved from script-only: 20
unknown=0
shared-state=0
total_records=341
```

New categories:

```text
verify-m031-output=10
verify-m033-output=10
```

## Canonical-only CI baseline

The workflow now requires the committed canonical baseline:

```text
data/architecture-assessment/write-path-inventory-canonical.json
```

The M179 preview fallback was removed. Missing canonical baseline now fails fast. Current inventory outputs are temp files only. Strict canonical-only result:

```text
canonical_only=1
unknown=0
shared-state=0
total_delta=+0
all_category_deltas=+0
```

## Cache lifecycle

The dedicated cache lifecycle review found no new stable shared cache lifecycle with exact ownership and concurrency proof. No broad cache, markdown, manifest, converter, or index rule was added. Existing conservative categories remain: `caller-owned`, `caller-owned-index`, and `parser-replay-output`.

## Verification

Integrated verification:

```text
focused inventory tests=24 passed
test architecture guard=dynamic=0, legacy=0, violations=0
onion guard=violation_count=0, allowed_violation_count=0
final and canonical workflow/cache assertions=PASS
strict canonical-only drift=PASS
```

Quality stack:

```text
scoped ruff=PASS
pyrefly=0 errors
pre-commit=PASS
GitNexus detect_changes=LOW risk, affected_processes=0
scope hygiene=expected M180 files only
```

## Decisions

- D102: Move exact verify_m031 and verify_m033 source-path families, simplify inventory CI to require the committed canonical baseline, update canonical baseline after movement, and complete cache lifecycle review as exact-movement-or-no-move only.

## Residual risks

1. Pre-edit GitNexus impact remained UNKNOWN because scanner/workflow targets did not resolve authoritatively.
2. `script-only=122` remains for future exact family waves.
3. Canonical baseline now must be updated intentionally with any future scanner/category change.
4. Cache lifecycle movement remains deferred until exact lifecycle and concurrency proof exists.

## Follow-ups

Recommended next GSD scopes:

1. Continue exact residual script waves against `script-only=122`, likely verify_m029 plus remaining small verify families.
2. After canonical-only CI soaks, consider removing older milestone-specific inventory notes from docs if they confuse maintainers.
3. Revisit cache lifecycle only when a stable shared cache/index file has exact lifecycle ownership and concurrency behavior to review.
