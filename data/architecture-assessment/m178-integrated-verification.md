# M178 Integrated Verification

## Verdict

**Integrated verification status: PASS.**

## Verification matrix

| Check | Result | Evidence |
|---|---|---|
| Focused inventory tests | PASS: 20 passed | `gsd_exec[d7b1a45d-cc97-4e97-8a73-8d22df1fda32]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[3384d058-c3bd-4fa5-8772-a47c25142029]` |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 | `gsd_exec[d765c1a2-a392-4e20-881f-af0a37bf2870]` |
| Final artifact and cache assertions | PASS | `gsd_exec[4b11f741-65ae-43e2-b48e-07e630ecf65e]` |
| Strict CI drift against final baseline | PASS | `gsd_exec[b7b56f9d-1a76-45a6-a4f1-9c6b26c78178]` |

## Final counts

```text
total_records=341
unknown=0
shared-state=0
script-only=170
m027-pipeline-replay-output=14
m025-recovery-evidence-output=14
```

## Delta highlights

```text
script-only -28
m027-pipeline-replay-output +14
m025-recovery-evidence-output +14
total delta +0
```

## Boundary checks

- Exact source-path rules only.
- Strict CI drift passes against final baseline.
- Cache coordination adds no broad cache category.
- Markdown converter caller-owned regression remains covered.
