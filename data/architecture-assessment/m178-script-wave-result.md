# M178 Script Wave Result

## Verdict

**Script wave status: PASS.**

M178 moved 28 exact historical script records out of `script-only` without broad prefix or target-name classification.

## Movement

```text
m027-pipeline-replay-output=14
m025-recovery-evidence-output=14
script-only: 198 -> 170
unknown=0
shared-state=0
total_records=341
```

## Exact families moved

- M027 pipeline replay/readiness outputs: five exact source paths.
- M025 recovery/evidence verifier outputs: five exact source paths.

## Residual no-move groups

- M031 and M033 parser/external-parser verifier families remain `script-only`.
- M057/M058 figure/table residual scripts remain `script-only`.
- M060/M066 graph benchmark families remain `script-only`.
- Audit, benchmark, acquire, render, repair, sync, and one-off scripts remain `script-only`.

## Evidence

- M027 scanner smoke: `gsd_exec[cb4b1c52-05e0-4818-b8b3-82ee2075f1c5]`.
- M025 scanner smoke: `gsd_exec[0c4ecbe3-72f3-4c34-ac7a-ded5b6ac1279]`.
- Aggregate scanner smoke: recorded in S06.
