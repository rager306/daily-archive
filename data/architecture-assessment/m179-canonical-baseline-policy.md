# M179 Canonical Baseline Policy

## Verdict

Use a committed canonical inventory baseline path for CI drift checks instead of depending on the latest milestone-specific final inventory.

## Canonical paths

```text
data/architecture-assessment/write-path-inventory-canonical.json
data/architecture-assessment/write-path-inventory-canonical.md
```

Optional review delta when updating the canonical baseline:

```text
data/architecture-assessment/write-path-inventory-canonical-delta.md
```

## CI behavior

1. If the canonical JSON baseline exists, CI runs strict drift against it.
2. CI writes current JSON, current markdown, and delta markdown to `mktemp` files only.
3. Strict mode fails unless total delta is `+0` and every category delta is `+0`.
4. `unknown=0` and `shared-state=0` remain mandatory.
5. If the canonical baseline does not exist yet, CI may fall back to the M179 baseline in preview mode.

## Update policy

When an intentional scanner category change lands, regenerate the canonical JSON and markdown as part of that same reviewed milestone and include a generated delta from the prior baseline. Do not hand-edit counts.

## Why this is smaller than milestone-specific CI

The workflow no longer needs to know which milestone produced the latest final inventory. It has one stable committed baseline path and temp files for current scan output.
