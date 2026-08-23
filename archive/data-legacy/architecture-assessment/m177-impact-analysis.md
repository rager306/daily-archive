# M177 Impact Analysis

## Verdict

**Pre-edit GitNexus impact is UNKNOWN.** This is expected for the scanner script based on M172-M176 history and is not treated as safety proof. M177 proceeds only with compensating verification.

## Probes

| Target | Direction | Result | Risk |
|---|---|---|---|
| `_classify` | upstream | target not found | UNKNOWN |
| `scripts/inventory_write_paths.py` | upstream | target not found | UNKNOWN |
| `render_delta_markdown` | upstream | target not found | UNKNOWN |
| `architecture-guardrail.yml` | upstream | target not found | UNKNOWN |

## Blast radius statement

GitNexus returned no direct callers or affected processes because the targets were not found. That means the indexed graph cannot prove scanner edit safety. It does **not** mean risk is low.

## Compensating proof required

M177 must provide all of the following after edits:

1. Focused positive and fallback tests in `tests/test_inventory_write_paths.py`.
2. Scanner smoke counts after each scanner category slice.
3. Generated final inventory and scanner-generated delta from M177 baseline.
4. `unknown=0` and `shared-state=0` assertions.
5. CI workflow command smoke if CI wiring changes.
6. Scoped ruff, pyrefly, pre-commit.
7. Final `gitnexus_detect_changes` with acceptable risk before closeout.

## Edit risk controls

- Edit only exact source-path classification rules before generic script fallback.
- Do not change inventory record schema.
- Do not change delta renderer behavior unless required for CI smoke, and then test it directly.
- Do not classify by generic target names.
- Preserve conservative no-move groups when exact ownership is not proven.
