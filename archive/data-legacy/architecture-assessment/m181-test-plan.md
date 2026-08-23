# M181 Test Plan

## Scanner focused tests

Run:

```bash
uv run pytest tests/test_inventory_write_paths.py -q
```

Required assertions:

- Selected exact `verify_m029` paths map to `verify-m029-output`.
- Selected exact `verify_m027` paths map to `verify-m027-output`.
- Future unlisted paths remain `script-only`:
  - `scripts/verify_m029_future_unlisted.py`
  - `scripts/verify_m027_future_unlisted.py`
- Generic targets (`path`, `fd`, `args.write_report`, `args.report`) do not classify by themselves.

## Delta checks

Generated only:

```bash
uv run python scripts/inventory_write_paths.py \
  --json data/architecture-assessment/m181-write-path-inventory-interim.json \
  --markdown data/architecture-assessment/m181-write-path-inventory-interim.md \
  --delta-from data/architecture-assessment/m181-write-path-inventory-baseline.json \
  --delta-markdown data/architecture-assessment/m181-verify-wave-delta.md
```

Expected interim movement:

```text
script-only: 122 -> 110
verify-m029-output: 0 -> 8
verify-m027-output: 0 -> 4
unknown=0
shared-state=0
total_delta=+0
```

## Canonical docs and CI cleanup

- Recon exact active references before editing.
- Do not remove historical evidence artifacts just because they mention old baselines.
- Strict canonical-only drift must pass after any cleanup.
- Do not reintroduce M179/M180 preview fallback.

## Cache lifecycle review

- Movement requires exact source ownership, lifecycle, invalidation, and concurrency proof.
- No broad cache, index, markdown, manifest, converter, target-name, or prefix rule.
- No-move review is valid if proof is absent.

## Final quality

Run before completion:

```bash
uv run ruff check scripts/inventory_write_paths.py tests/test_inventory_write_paths.py
uv run pyrefly check
uv run pre-commit run --all-files
```

Then run final GitNexus `detect_changes` and filtered status hygiene.
