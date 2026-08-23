# M177 Combined Ownership Wave Closeout

## Verdict

**M177 status: PASS.**

The user requested all five follow-up directions if possible. M177 completed all five in one bounded GSD milestone with exact scanner movement where safe, documented no-move policy where movement was unsafe or unnecessary, and CI smoke wiring for generated inventory deltas.

## Included directions

| Direction | Result | Evidence |
|---|---|---|
| 1. R024 script inventory wave | Implemented: 23 records moved | `m177-combined-candidates.md`, final delta |
| 2. Scanner self-output ownership | Implemented: 3 records moved | `inventory-report-output=3` |
| 3. Markdown cache policy review | Completed as no-move policy | `m177-cache-policy-review.md` |
| 4. Queue and smoke output ownership | Implemented: 11 records moved | `queue-soak-output=1`, `queue-gate-output=2`, `smoke-script-output=8` |
| 5. Inventory delta CI wiring | Implemented as mandatory architecture guardrail smoke | `.github/workflows/architecture-guardrail.yml`, `m177-ci-delta-wiring.md` |

## Category movement

```text
script-only: 235 -> 198
records moved from script-only: 37
unknown=0
shared-state=0
total_records=341
```

New or expanded categories:

```text
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

## No-move decisions

- Markdown converter writes remain `caller-owned` because reviewed targets are caller/adapter output paths, not stable shared cache state.
- Existing Universal KB workflow records remain `database`, `caller-owned`, or `run-scoped`.
- CI wiring has no scanner category because it is workflow configuration, not a write-path source record.
- All unreviewed scripts remain `script-only`.

## CI wiring

Architecture guardrail CI now runs `Run write-path inventory delta smoke` in the mandatory M044 job. It generates current inventory and delta markdown into temporary files, asserts `unknown=0` and `shared-state=0`, and prints a short delta preview. It does not write tracked generated artifacts.

## Verification

Integrated verification:

```text
focused inventory tests=18 passed
test architecture guard=dynamic=0, legacy=0, violations=0
onion guard=violation_count=0, allowed_violation_count=0
final artifact assertions=PASS
CI delta smoke final baseline=PASS
```

Quality stack:

```text
scoped ruff=PASS
pyrefly=0 errors
pre-commit=PASS
GitNexus detect_changes=LOW risk, affected_processes=0
scope hygiene=expected M177 files only
```

## Decisions

- D099: M177 combined scope uses exact source-path movement for R024, scanner self-output, and queue/smoke; markdown cache is policy-only and CI is workflow-only.

## Residual risks

1. Pre-edit GitNexus impact remained UNKNOWN because scanner/workflow targets did not resolve authoritatively.
2. `script-only=198` remains a large residual bucket and should continue through exact waves only.
3. CI delta smoke is not yet a strict zero-drift policy; it is visibility and guardrail smoke until a stable canonical inventory baseline is chosen.
4. Cache policy remains conservative for future stable shared cache indexes.

## Follow-ups

Recommended next GSD scopes:

1. Next script-only exact family wave against the remaining `script-only=198`.
2. Promote inventory delta CI smoke to stricter drift policy once canonical baseline ownership is decided.
3. Dedicated cache-coordination review if markdown conversion gains stable shared cache/index files.
4. Review remaining M02x/M03x historical replay and verifier scripts by exact milestone families.
