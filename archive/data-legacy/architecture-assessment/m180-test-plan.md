# M180 Test Plan

## Scanner tests

- New exact verify_m031 paths classify as `verify-m031-output`.
- New exact verify_m033 paths classify as `verify-m033-output`.
- Nearby unlisted verify_m031/verify_m033-looking paths remain `script-only`.
- Existing M179 categories remain covered.

## Inventory delta checks

- Generate M180 interim and final inventory with `--delta-from data/architecture-assessment/m180-write-path-inventory-baseline.json`.
- Assert `script-only -20`.
- Assert verify_m031 `+10` and verify_m033 `+10`.
- Assert total delta `+0`.
- Assert `unknown=0` and `shared-state=0`.

## Canonical CI checks

- Workflow requires `data/architecture-assessment/write-path-inventory-canonical.json`.
- Workflow uses temp current inventory files.
- Strict drift command passes with total delta `+0` and all category deltas `+0`.

## Cache lifecycle checks

- Review cache-like residual records.
- Move only exact paths with lifecycle and concurrency proof.
- If no proof exists, write no-move artifact and regression assertions.

## Final quality

- Focused pytest.
- Test architecture guard.
- Onion guard.
- Scoped ruff.
- Pyrefly.
- Pre-commit.
- GitNexus detect_changes.
