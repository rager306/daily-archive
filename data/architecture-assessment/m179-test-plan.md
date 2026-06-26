# M179 Test Plan

## Scanner tests

- New exact M057 paths classify as `m057-structure-extraction-output`.
- New exact M060 paths classify as `m060-graph-figure-benchmark-output`.
- Nearby unlisted M057/M060-looking paths remain `script-only`.
- Existing M178 categories remain covered.

## Inventory delta checks

- Generate M179 interim and final inventory with `--delta-from data/architecture-assessment/m179-write-path-inventory-baseline.json`.
- Assert `script-only -28`.
- Assert M057 `+15` and M060 `+13`.
- Assert total delta `+0`.
- Assert `unknown=0` and `shared-state=0`.

## Canonical CI checks

- Workflow uses temp current inventory files.
- Workflow selects canonical committed baseline when present.
- Strict drift command passes with total delta `+0` and all category deltas `+0`.

## Cache lifecycle checks

- Review cache-like residual records.
- Move only exact paths with lifecycle ownership proof.
- If no proof exists, write no-move artifact and regression assertions.

## Final quality

- Focused pytest.
- Test architecture guard.
- Onion guard.
- Scoped ruff.
- Pyrefly.
- Pre-commit.
- GitNexus detect_changes.
