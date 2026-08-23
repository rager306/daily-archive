# M185 Test Architecture Extraction Assessment

## Verdict

Proceed with extraction.

## GitNexus impact

All candidate symbols returned LOW, exact impact with no affected processes:

| Symbol | Impact | Direct callers |
|---|---|---|
| `analyze_test_file` | LOW exact | `build_inventory` |
| `classify` | LOW exact | `analyze_test_file` |
| `build_inventory` | LOW exact | `audit_test_architecture.main`, `verify_test_architecture.main` |
| `write_outputs` | LOW exact | `audit_test_architecture.main` |
| `render_markdown` | LOW exact | `write_outputs` |
| `TestFileAnalysis` | LOW exact | imported by `verify_test_architecture.py` |

## Movement scope

Move the concrete inventory/audit implementation into:

`src/research_graph/application/test_architecture_inventory.py`

Keep `scripts/audit_test_architecture.py` as a thin CLI wrapper that imports and re-exports the existing names needed by `scripts/verify_test_architecture.py` and tests.

## Constraints

- Preserve `DEFAULT_OUTPUT_DIR`, `DEFAULT_TESTS_DIR`, and `build_inventory` imports for `verify_test_architecture.py`.
- Do not introduce a Protocol, factory, or dynamic import.
- Do not alter guardrail semantics.
- Keep output filenames unchanged.
