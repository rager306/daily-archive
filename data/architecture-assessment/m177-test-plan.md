# M177 Test Plan

## Goals

Verify all five requested directions without weakening write-path guardrails.

## Focused inventory tests

Add tests for exact source-path categories:

1. **R024 scripts**
   - `r024-corpus-selection-output` positives.
   - `r024-entity-extraction-output` positive.
   - `r024-conversion-output` positive.
   - `r024-networkx-probe-output` positive.
   - `r024-quality-metrics-output` positive.

2. **Scanner self-output**
   - `scripts/inventory_write_paths.py` returns `inventory-report-output` for JSON, markdown, and delta markdown writes.
   - Delta renderer behavior remains covered by existing tests.

3. **Queue and smoke scripts**
   - `queue-soak-output` positive.
   - `queue-gate-output` positive.
   - `smoke-script-output` positives.

4. **Fallbacks**
   - An unrelated script remains `script-only`.
   - Markdown converter remains `caller-owned` rather than cache-reclassified.
   - Existing `src/research_graph/workflows/universal_kb/*` categories remain unchanged.

## Artifact checks

- Baseline JSON/markdown exist.
- Final JSON/markdown exist.
- Generated delta markdown exists.
- Count assertion artifact verifies expected movement and guardrails.

## CI checks

If workflow wiring is implemented:

- Run the workflow command locally without writing tracked generated artifacts.
- Verify it exits nonzero on drift only if the generated current inventory differs from a checked baseline or committed expectation.

If workflow wiring is deferred:

- Write a no-move CI decision artifact explaining why local-only generated delta remains safer.

## Integrated checks

Run:

```text
uv run pytest tests/test_inventory_write_paths.py -q
uv run python scripts/verify_test_architecture.py --json
uv run python scripts/verify_onion_layering.py --json
```

## Quality stack

Run:

```text
uv run ruff check scripts/inventory_write_paths.py tests/test_inventory_write_paths.py .github/workflows/architecture-guardrail.yml
uv run pyrefly check
uv run pre-commit run --all-files
```

If ruff does not accept YAML paths, use scoped ruff on Python files and rely on pre-commit for workflow validation.

## Closeout checks

- `unknown=0`.
- `shared-state=0`.
- Final `script-only` count matches generated delta.
- GitNexus final detect_changes risk is acceptable.
- Filtered git status contains only expected M177 files and ignored runtime noise.
