# M172 Integrated Verification

## Verdict

**Integrated verification status: PASS.**

## Verification matrix

| Check | Result | Evidence |
|---|---|---|
| Focused inventory tests | PASS: 5 passed | `gsd_exec[c6c2ea2e-0264-4597-b288-5a266875d4bf]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[4e58b1c0-489e-4adc-9614-f94b0842b06b]` |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 | `gsd_exec[2cff1afa-5386-4214-b0ec-bca0163d4fd2]` |
| Final inventory assertions | PASS: unknown=0 and new categories match expected counts | `gsd_exec[31df2d8c-4353-46f1-ab6e-822179d8752c]` |

## Final counts

```text
total_records=340
unknown=0
graph-readiness-evidence=14
source-asset-package=4
article-artifact-package=7
caller-owned=28
run-scoped=14
append-log=3
```

## Boundary checks

- Scanner record schema unchanged.
- AST traversal unchanged.
- New categories are exact source path-family matches.
- Broad conservative categories remain visible.
- No shared-state records were reclassified.
