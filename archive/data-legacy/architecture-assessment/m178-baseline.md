# M178 Baseline

## Verdict

**PASS.** Fresh M178 write-path baseline captured before scanner, CI, or cache policy edits.

## Counts

```text
total_records=341
unknown=0
shared-state=0
script-only=198
scripts_root=265
src_root=76
```

## Preserved M177 categories

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

## Baseline artifacts

- `data/architecture-assessment/m178-write-path-inventory-baseline.json`
- `data/architecture-assessment/m178-write-path-inventory-baseline.md`

## Guardrails

- `unknown=0` remains true.
- `shared-state=0` remains true.
- No M178 scanner, CI, or cache policy edits were made before this baseline.
