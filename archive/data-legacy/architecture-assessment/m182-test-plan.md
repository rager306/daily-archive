# M182 Test Plan

## Focused tests

Run:

```bash
uv run pytest tests/test_inventory_write_paths.py -q
```

Required assertions:

- Selected exact `build_m028` paths map to `build-m028-output`.
- Selected exact `replay_m031` path maps to `replay-m031-output`.
- Future unlisted paths remain `script-only`:
  - `scripts/build_m028_future_unlisted.py`
  - `scripts/replay_m031_future_unlisted.py`
- Generic targets do not classify by themselves.

## Delta checks

Generated only:

```bash
uv run python scripts/inventory_write_paths.py \
  --json data/architecture-assessment/m182-write-path-inventory-interim.json \
  --markdown data/architecture-assessment/m182-write-path-inventory-interim.md \
  --delta-from data/architecture-assessment/m182-write-path-inventory-baseline.json \
  --delta-markdown data/architecture-assessment/m182-wave-delta.md
```

Expected movement:

```text
script-only: 110 -> 103
build-m028-output: 0 -> 4
replay-m031-output: 0 -> 3
unknown=0
shared-state=0
total_delta=+0
```

## Canonical refresh

After movement is verified, refresh canonical JSON/markdown/delta and run strict canonical drift.

## Final quality

```bash
uv run ruff check scripts/inventory_write_paths.py tests/test_inventory_write_paths.py
uv run pyrefly check
uv run pre-commit run --all-files
```

Then run final GitNexus `detect_changes` and filtered status hygiene.
