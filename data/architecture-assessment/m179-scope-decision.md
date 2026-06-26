# M179 Scope Decision

## Decision

D101 records M179 scope.

## Scope

1. Move exact M057 and M060 source-path families from `script-only`.
2. Add canonical committed inventory baseline policy for CI drift.
3. Complete cache lifecycle review with exact movement only if lifecycle proof exists; otherwise close as no-move.

## Expected movement

```text
script-only: 170 -> 142
m057-structure-extraction-output: 0 -> 15
m060-graph-figure-benchmark-output: 0 -> 13
```

## Guardrails

- Exact source paths only.
- No target-name rules.
- No broad cache rule.
- UNKNOWN GitNexus impact is not safety proof.
