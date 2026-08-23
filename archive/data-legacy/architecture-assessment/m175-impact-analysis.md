# M175 Pre-edit Impact Analysis

## Verdict

**Impact status: UNKNOWN but bounded.** GitNexus did not resolve the scanner-specific targets authoritatively, so impact is not treated as safety proof. M175 proceeds only with focused tests, generated inventory checks, and final `detect_changes`.

## Planned edited symbols

- `_classify` in `scripts/inventory_write_paths.py`
- `main` in `scripts/inventory_write_paths.py`
- new small delta markdown helper in `scripts/inventory_write_paths.py`
- tests in `tests/test_inventory_write_paths.py`

## GitNexus results

| Target | Result | Risk | Notes |
|---|---|---|---|
| `_classify` | target not found | UNKNOWN | Same limitation as M172-M174. |
| `scripts/inventory_write_paths.py` | target not found | UNKNOWN | File target not indexed as symbol. |
| `render_markdown` | ambiguous | UNKNOWN | 11 matching symbols; scanner-specific context lookup did not resolve. |
| `main` | ambiguous | UNKNOWN | 20 matching symbols; not authoritative for scanner script. |

## Blast radius statement

No HIGH or CRITICAL impact result was returned. The meaningful risk is tool-resolution UNKNOWN, not a proven low blast radius.

## Compensating controls

- Keep code changes local to `scripts/inventory_write_paths.py`.
- Preserve scanner record schema.
- Add focused classification tests for both exact categories and fallback cases.
- Add focused delta report test.
- Regenerate final inventory and generated delta report from the scanner.
- Run focused tests, scoped ruff, pyrefly, pre-commit, and final `gitnexus_detect_changes`.

## Proceed condition

Implementation may proceed because S05 froze exact scope and the planned diff is small, but closeout requires final GitNexus detect_changes and quality evidence.
