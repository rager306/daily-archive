# M174 Integrated Verification

## Verdict

**Integrated verification status: PASS.**

## Verification matrix

| Check | Result | Evidence |
|---|---|---|
| Focused inventory tests | PASS: 9 passed | `gsd_exec[9e21013f-1df7-44ba-aaaa-a4df16d03185]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[0d708496-0147-4cc0-be0f-a9d3381593a1]` |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 | `gsd_exec[690c6dfd-3c22-4ab1-99a3-81e0c25f5f42]` |
| Final inventory assertions | PASS: unknown=0, repair category count, and caller-owned-index preserved | `gsd_exec[f2d8ee81-95f0-4ed7-b99b-151d3e856b9b]` |

## Final counts

```text
total_records=340
unknown=0
repair-benchmark-output=5
caller-owned-index=1
caller-owned=20
run-scoped=11
append-log=1
```

## Boundary checks

- Scanner record schema unchanged.
- AST traversal unchanged.
- New category is exact source path matching.
- caller-owned-index exception remains preserved.
- Broad conservative categories remain visible.
- No shared-state records were reclassified.
