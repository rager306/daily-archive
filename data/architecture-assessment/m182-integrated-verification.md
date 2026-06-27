# M182 Integrated Verification

## Verdict

**Integrated verification status: PASS.**

## Verification matrix

| Check | Result | Evidence |
|---|---|---|
| Focused inventory tests | PASS: 28 passed | `gsd_exec[cf704055-a821-4d50-afc3-882ed27df273]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[b29556d5-aae0-44b3-b14b-3877afe680f8]` |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 | `gsd_exec[69b4c80b-9367-442e-a76d-0c57c9ab28e3]` |
| Strict canonical drift | PASS | `gsd_exec[e5b63e53-0cae-428f-9e93-10e4d78dc492]` |
| Final artifact assertions | PASS | `gsd_exec[df6b0315-dace-4669-8c98-0b8996e5c297]` |

## Final counts

```text
total_records=341
script-only=103
build-m028-output=4
replay-m031-output=3
unknown=0
shared-state=0
```

## Direction result

- Exact build and replay wave: PASS, moved 7 records from script-only.
- Canonical baseline refresh: PASS, strict drift restored.
