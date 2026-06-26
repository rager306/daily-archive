# M175 Delta Reporting Recon

## Verdict

**Delta reporting recon status: PASS.** Add a minimal optional delta-report path to `scripts/inventory_write_paths.py`; do not create a reporting framework.

## Current scanner shape

Current CLI:

```text
uv run python scripts/inventory_write_paths.py --json OUT.json --markdown OUT.md
```

Current internal surfaces:

- `collect_records()` returns `WritePathRecord` rows.
- `build_payload(records)` creates summary plus records.
- `render_markdown(payload)` renders current inventory markdown.
- `main()` writes JSON and markdown, then prints summary JSON.

## Selected implementation

Add two optional CLI args:

```text
--delta-from BASELINE.json
--delta-markdown DELTA.md
```

Behavior:

1. Generate the current inventory exactly as today.
2. If both optional args are present, load baseline JSON and render a markdown category-count delta between baseline and current payload.
3. If only one optional arg is present, fail fast with argparse error.

Minimal helper surface:

```text
_render_delta_markdown(baseline_payload, current_payload) -> str
```

The helper only compares `summary.by_category` and `summary.total_records`.

## Rejected alternatives

| Alternative | Rejection reason |
|---|---|
| New reporting module | Too much ceremony for one table. |
| New dependency | Stdlib JSON and markdown strings are enough. |
| Separate one-off M175 script | Would repeat the previous hand-written snippet pattern instead of making CI/report reuse possible. |
| Full record-level diff | Not needed for current category count review and would make reports noisy. |

## Test strategy

Add focused tests in `tests/test_inventory_write_paths.py`:

- category delta rendering includes new, removed, and changed categories;
- total record delta is shown;
- generated report is deterministic enough for CI artifact checks.

## Expected use in M175

```text
uv run python scripts/inventory_write_paths.py \
  --json data/architecture-assessment/m175-write-path-inventory-final.json \
  --markdown data/architecture-assessment/m175-write-path-inventory-final.md \
  --delta-from data/architecture-assessment/m175-write-path-inventory-baseline.json \
  --delta-markdown data/architecture-assessment/m175-inventory-delta.md
```

## Safety notes

- Delta reporting must not change inventory collection or classification behavior.
- The generated delta replaces hand arithmetic in closeout artifacts.
- The markdown table should be readable in CI logs and artifact previews.
