# M171 Inventory Impact Analysis

## Verdict

**Proceed with scanner edits cautiously.**

GitNexus exact impact lookup did not find `_classify` or `inventory_write_paths.py` as indexed targets, so no reliable upstream blast radius was available from exact impact. The scanner is a script-level architecture utility, and final `gitnexus_detect_changes` remains required after edits.

## GitNexus evidence

| Query | Result |
|---|---|
| `gitnexus_impact(target="_classify")` | target not found, risk UNKNOWN |
| `gitnexus_impact(target="inventory_write_paths.py")` | target not found, risk UNKNOWN |
| `gitnexus_query("inventory_write_paths write path scanner _classify")` | no relevant scanner flow found |

Because the exact target is not indexed, this is treated as a stale/missing GitNexus symbol condition rather than proof of no risk.

## Test discovery

Evidence: `gsd_exec[5adef733-141c-47a1-81fa-3391bae381b4]`.

Existing search did not find focused tests for `scripts/inventory_write_paths.py` behavior. M171 should add a narrow focused test instead of relying on unrelated tests.

## Edit target

```text
scripts/inventory_write_paths.py::_classify(...)
```

The edit should be minimal and path-specific:

- add exact rule for `batch_state.py` + `output_path` -> `run-owned-state`;
- add exact rule for catalog legacy summary/report paths -> `legacy-evidence-regeneration`;
- add exact rule for `chunk_baseline_measurement.py` + `index_path` -> `caller-owned-index`;
- preserve generic shared-state fallback for other `state`, `index`, `catalog`, and `queue` paths.

## Test target

Add focused tests, preferably:

```text
tests/test_inventory_write_paths.py
```

Tests should assert:

- four reviewed records receive precise categories;
- a synthetic unknown `state`/`index`/`catalog` target remains `shared-state`;
- final inventory can still be generated with `unknown=0`.

## Risk assessment

Manual risk: **LOW to MEDIUM**.

Why:

- Script is not runtime package logic.
- Change is category classification only.
- Main risk is weakening architecture signal by hiding shared-state records.
- Focused tests and final inventory output mitigate that risk.

## Proceed condition

Proceed to S10/S11 only with narrow tests. If category changes require broad target-token rules, stop and replan.
