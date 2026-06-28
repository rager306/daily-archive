# M187 M058 and M059 Impact Refresh

## Verdict

**Fresh GitNexus impacts remain exact: M058 LOW, M059 MEDIUM.**

## Fresh impact results

| Target | Risk | Direct callers | Focused tests |
|---|---:|---|---|
| `Function:scripts/m058_build_graph_manifest.py:write_json` | LOW | `build_graph_manifest` | `uv run pytest tests/test_m058_s05.py::test_graph_manifest_combined -q` |
| `Function:scripts/m059_build_manifest.py:finalize_manifest` | MEDIUM | `build_m054`, `build_m055`, `build_m055deep`, `build_m056`, `build_m057`, `build_m058` | `uv run pytest tests/test_m059_s01.py -q` |

## Implementation context

`m058_build_graph_manifest.write_json` is a local helper used twice by `build_graph_manifest`: once for `combined-edges.json`, once for `per-layer-summary.json`. It writes sorted UTF-8 JSON with `ensure_ascii=False` and no return value.

`m059_build_manifest.finalize_manifest` centralizes all retroactive manifest writes for M054-M058. It builds the manifest object, resolves `output_path` relative to repo root, writes sorted JSON, and returns the manifest object. Six direct builder callers feed into it, so preserving return semantics and path resolution is mandatory.

`write_manifest_json_atomic` can preserve both JSON shapes. For M059, the call must use the already resolved `actual_output` path.

## MEDIUM risk handling for M059

Before editing M059, re-run exact impact immediately. After editing, run all `tests/test_m059_s01.py`, not just one focused test, because six builders are direct callers.

## Source edit status

No source edits were made in this task.
