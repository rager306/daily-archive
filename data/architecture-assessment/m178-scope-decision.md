# M178 Scope Decision

## Decision

M178 includes all three requested directions in one bounded milestone:

1. Move exact M027 pipeline replay/readiness script outputs.
2. Move exact M025 recovery/evidence verifier outputs.
3. Upgrade inventory CI from smoke visibility to strict drift checking against committed final inventory.
4. Complete cache coordination as conservative policy/no-move unless exact shared cache ownership is proven.

## Allowed scanner movement

| Category | Records | Scope |
|---|---:|---|
| `m027-pipeline-replay-output` | 14 | five exact M027 source paths |
| `m025-recovery-evidence-output` | 14 | five exact M025 source paths |

## No-move policy

- No broad `m0xx` prefix rule.
- No generic target-name classifier.
- M031, M033, M057/M058, M060/M066, audit, benchmark, acquire, render, repair, and sync scripts remain `script-only`.
- Cache coordination remains policy/no-move unless exact shared cache ownership appears.

## Acceptance target

```text
script-only: 198 -> 170
unknown=0
shared-state=0
```

Final movement must be proven by generated inventory and scanner-generated delta.

Decision recorded as D100.
