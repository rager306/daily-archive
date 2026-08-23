# M178 Test Plan

## Script wave tests

Add focused tests for:

- `m027-pipeline-replay-output` positive cases.
- `m025-recovery-evidence-output` positive cases.
- Unrelated residual scripts remain `script-only`.

Run after each scanner edit:

```text
uv run pytest tests/test_inventory_write_paths.py -q
uv run ruff check scripts/inventory_write_paths.py tests/test_inventory_write_paths.py
```

## Strict CI drift policy

Upgrade the workflow command so that, when the committed final inventory baseline exists, generated current inventory and delta must show zero category drift. The command must write only temporary files.

Local smoke must pass before and after final inventory generation.

## Cache coordination

Document reviewed cache-like paths and preserve caller-owned/no-move classifications unless exact shared cache/index ownership is proven. Add or preserve regression tests for caller-owned markdown converter outputs.

## Final inventory checks

Generate:

```text
data/architecture-assessment/m178-write-path-inventory-final.json
data/architecture-assessment/m178-write-path-inventory-final.md
data/architecture-assessment/m178-inventory-delta.md
```

Assert:

```text
script-only=170
unknown=0
shared-state=0
m027-pipeline-replay-output=14
m025-recovery-evidence-output=14
```

## Integrated and quality checks

Run:

```text
uv run pytest tests/test_inventory_write_paths.py -q
uv run python scripts/verify_test_architecture.py --json
uv run python scripts/verify_onion_layering.py --json
uv run ruff check scripts/inventory_write_paths.py tests/test_inventory_write_paths.py
uv run pyrefly check
uv run pre-commit run --all-files
```

Final closeout also requires GitNexus detect_changes and filtered status hygiene.
