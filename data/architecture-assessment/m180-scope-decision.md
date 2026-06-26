# M180 Scope Decision

## Decision

D102 records M180 scope.

## Scope

1. Move exact verify_m031 and verify_m033 source-path families from `script-only`.
2. Simplify inventory CI to require the committed canonical baseline and stop depending on a milestone-specific preview baseline.
3. Update canonical inventory baseline after the verify wave.
4. Complete cache lifecycle review with exact movement only if lifecycle and concurrency proof exists; otherwise close as no-move.

## Expected movement

```text
script-only: 142 -> 122
verify-m031-output: 0 -> 10
verify-m033-output: 0 -> 10
```

## Guardrails

- Exact source paths only.
- No broad `verify_m031*`, `verify_m033*`, or `verify_m*` rules.
- No target-name rules.
- No broad cache rule.
- UNKNOWN GitNexus impact is not safety proof.
