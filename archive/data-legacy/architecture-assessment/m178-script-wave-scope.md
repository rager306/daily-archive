# M178 Script Wave Scope

## Decision

M178 script movement is limited to two exact historical families: M027 pipeline replay/readiness outputs and M025 recovery/evidence verifier outputs.

## Allowed scanner movement

1. `m027-pipeline-replay-output`: 14 records from five exact source paths.
2. `m025-recovery-evidence-output`: 14 records from five exact source paths.

## No-move policy

- No generic target-name classification.
- No broad `m0xx` prefix rule.
- M031, M033, M057/M058, M060/M066, benchmark, audit, and one-off scripts remain `script-only`.

## Acceptance target

If all scanner movement lands, M178 should move **28 script-only records** and end near:

```text
script-only=170
unknown=0
shared-state=0
```

The final count must be proven by generated inventory and scanner-generated delta.
