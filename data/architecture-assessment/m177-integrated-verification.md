# M177 Integrated Verification

## Verdict

**Integrated verification status: PASS.**

## Verification matrix

| Check | Result | Evidence |
|---|---|---|
| Focused inventory tests | PASS: 18 passed | `gsd_exec[30df0dae-e4a2-411f-ad70-b630031dad9d]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[b6e40516-5b79-4ddf-a4fb-7c3b1a407dfa]` |
| Onion guard | PASS: violation_count=0, allowed_violation_count=0 | `gsd_exec[2c84a0e1-c11a-4ed1-b977-80f0afa299df]` |
| Final artifact assertions | PASS: final counts and generated delta lines match | `gsd_exec[ba5cdca1-774e-4c5d-a93d-6ce5adad2e9f]` |
| CI delta smoke against final baseline | PASS: unknown=0, shared-state=0, total delta +0 | `gsd_exec[2804abfd-ccc4-42ac-8f0b-a9db8759d5bb]` |

## Final counts

```text
total_records=341
unknown=0
shared-state=0
script-only=198
r024-corpus-selection-output=6
r024-entity-extraction-output=3
r024-conversion-output=3
r024-networkx-probe-output=3
r024-quality-metrics-output=8
inventory-report-output=3
queue-soak-output=1
queue-gate-output=2
smoke-script-output=8
```

## Delta highlights

```text
script-only -37
r024 categories +23
inventory-report-output +3
queue/smoke categories +11
total delta +0
```

## Boundary checks

- Exact source-path rules only.
- Markdown converter remains `caller-owned`.
- Universal KB workflow records remain `database`, `caller-owned`, or `run-scoped`.
- CI delta smoke writes only temporary files.
