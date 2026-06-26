# M178 Impact Analysis

## Verdict

**Pre-edit GitNexus impact is UNKNOWN.** Scanner and workflow targets did not resolve in the index. This is not safety proof and must be compensated with tests, generated deltas, strict CI smoke, quality checks, and final detect_changes.

## Probes

| Target | Direction | Result | Risk |
|---|---|---|---|
| `_classify` | upstream | target not found | UNKNOWN |
| `scripts/inventory_write_paths.py` | upstream | target not found | UNKNOWN |
| `.github/workflows/architecture-guardrail.yml` | upstream | target not found | UNKNOWN |
| `render_delta_markdown` | upstream | target not found | UNKNOWN |

## Compensating proof required

- Focused positive and fallback tests for M027 and M025 categories.
- Scanner smoke after each category slice.
- Strict CI drift local smoke before and after final inventory exists.
- Cache coordination no-move regression tests if relevant.
- Generated final inventory and scanner-generated delta.
- `unknown=0` and `shared-state=0` assertions.
- Scoped ruff, pyrefly, pre-commit.
- Final `gitnexus_detect_changes` with acceptable risk.
