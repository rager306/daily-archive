# M180 Canonical Soak Policy

## Verdict

Use canonical-only inventory drift in CI. The canonical baseline introduced in M179 exists, so M180 can remove the milestone-specific preview fallback from the workflow.

## Required baseline

```text
data/architecture-assessment/write-path-inventory-canonical.json
```

The markdown and delta artifacts remain review evidence:

```text
data/architecture-assessment/write-path-inventory-canonical.md
data/architecture-assessment/write-path-inventory-canonical-delta.md
```

## CI behavior

1. Fail fast if the canonical JSON baseline is missing.
2. Generate current JSON, current markdown, and delta markdown only in `mktemp` files.
3. Always run strict drift against the canonical baseline.
4. Fail unless total delta is `+0` and every category delta is `+0`.
5. Keep `unknown=0` and `shared-state=0` mandatory.

## Update policy

Intentional scanner category changes must update the canonical JSON and markdown in the same reviewed milestone. The canonical delta should be generated from the milestone baseline for review evidence.

## Why remove preview fallback

The preview fallback was useful while canonical baseline did not exist. Now it creates milestone-specific workflow coupling and could hide a missing committed baseline. Canonical-only CI is simpler and fails closed.
