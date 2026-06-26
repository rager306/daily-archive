# M175 Integrated Verification

## Verdict

**Integrated verification status: PASS.**

## Verification matrix

| Check | Result | Evidence |
|---|---|---|
| Focused inventory tests | PASS: 12 passed | `gsd_exec[62a2c964-e1db-4d76-8050-4cea95db8e86]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[4495c404-11e2-40d6-aa17-27996a5322d8]` |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 | `gsd_exec[3e77d08d-8518-4809-ab96-2bb40a2142f7]` |
| Final artifact assertions | PASS: final counts and generated delta lines match | `gsd_exec[61af8418-70c9-4851-94cf-20a60f6946f6]` |

## Final counts

```text
total_records=341
unknown=0
shared-state=0
daily-cli-output=5
validation-batch-output=10
caller-owned=10
run-scoped=6
append-log=1
temporary=1
script-only=265
```

## Generated delta highlights

```text
daily-cli-output +5
validation-batch-output +10
caller-owned -10
run-scoped -5
script-only +1
```

## Boundary checks

- Existing inventory JSON schema remains unchanged.
- New delta report is markdown-only and generated from inventory JSON payloads.
- No generic target-name classification was added.
- Conservative buckets remain visible.
- `unknown=0` and `shared-state=0` remain true.
