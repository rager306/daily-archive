# M173 Integrated Verification

## Verdict

**Integrated verification status: PASS.**

## Verification matrix

| Check | Result | Evidence |
|---|---|---|
| Focused inventory tests | PASS: 8 passed | `gsd_exec[9c292528-ef64-4710-a1a5-57cd7694dd66]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[e5dad028-e96f-4346-9450-f08859e3aaf6]` |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 | `gsd_exec[d34fb80a-64a9-442f-8eb5-f4c92d563fdc]` |
| Final inventory assertions | PASS: unknown=0 and batch-two categories match expected counts | `gsd_exec[ebd2e531-e7c6-4499-a009-74d415ebce76]` |

## Final counts

```text
total_records=340
unknown=0
parser-replay-output=3
source-scan-output=3
graph-probe-output=2
caller-owned=21
run-scoped=13
append-log=3
```

## Boundary checks

- Scanner record schema unchanged.
- AST traversal unchanged.
- New categories are exact source path matches.
- Broad conservative categories remain visible.
- No shared-state records were reclassified.
