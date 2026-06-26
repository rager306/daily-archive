# M175 CLI Broad Outputs and Delta Reporting Closeout

## Verdict

**M175 status: PASS.**

M175 completed all three requested directions in one GSD milestone:

1. command-specific CLI outputs;
2. remaining mixed broad output review with one exact movement and explicit no-move groups;
3. CI/report-friendly inventory delta rendering.

## Categories added

| Category | Count | Exact scope |
|---|---:|---|
| `daily-cli-output` | 5 | `src/research_graph/cli/__init__.py` durable daily CLI artifacts |
| `validation-batch-output` | 10 | `src/research_graph/workflows/validation/batch_workflow.py` reviewed validation batch artifacts |

## No-move groups preserved

- `temp_path` remains `temporary`.
- Article artifact command package remains `article-artifact-package`.
- Coverage report writer outputs remain `caller-owned`.
- Markdown converter cache-like paths remain `caller-owned`.
- Quality report outputs remain `caller-owned`.
- Universal KB queue and smoke outputs remain broad/conservative.
- Structure-aware chunker summary and diagnostics remain `run-scoped` and `append-log`.
- `script-only` remains future script inventory work.

## Delta reporting added

`inventory_write_paths.py` now supports:

```text
--delta-from BASELINE.json
--delta-markdown DELTA.md
```

The generated report compares category counts and total records from inventory JSON payloads. Both args are required together.

## Final inventory counts

```text
total_records=341
unknown=0
shared-state=0
daily-cli-output=5
validation-batch-output=10
caller-owned=10
run-scoped=6
append-log=1
temporary=1
script-only=265
```

The `script-only +1` delta is intentional: scanner delta support adds one new script write path for the generated delta markdown.

## Generated delta highlights

```text
daily-cli-output +5
validation-batch-output +10
caller-owned -10
run-scoped -5
script-only +1
```

## Tests

```text
uv run pytest tests/test_inventory_write_paths.py -q
12 passed
```

Coverage includes:

- daily CLI positive classifications;
- CLI `temp_path` preservation;
- generic `filepath` fallback;
- validation batch positive classifications;
- generic `summary_path` fallback;
- M174 repair `index_path` exception preservation;
- delta markdown totals, positive/negative/zero deltas, and deterministic ordering.

## Verification

Integrated verification:

```text
focused inventory tests=12 passed
test architecture guard=dynamic=0, legacy=0, violations=0
onion guard=violation_count=0, allowed_violation_count=0
final artifact assertions=PASS
```

Quality stack:

```text
scoped ruff=PASS
pyrefly=0 errors
pre-commit=PASS
GitNexus detect_changes=LOW risk, affected_processes=0
scope hygiene=expected M175 files only
```

## Decisions

- D097: M175 adds exact `daily-cli-output`, exact `validation-batch-output`, and minimal optional inventory delta rendering while preserving conservative no-move groups.

## Residual risks

1. Pre-edit GitNexus impact remained UNKNOWN because scanner symbols did not resolve authoritatively.
2. Delta report is category-count level only; record-level diff is intentionally out of scope.
3. Script-only remains large and mixed; it needs a separate inventory category pass if desired.
4. Cache-like, queue-like, and append-log outputs remain conservative by design.

## Follow-ups

Possible next GSD scopes:

1. exact script-only category waves;
2. cache policy review for markdown converter outputs;
3. queue and smoke output ownership review;
4. optional CI job wiring for generated inventory delta artifacts.

Each future category wave should keep the same pattern: baseline, exact path review, scope decision, pre-edit impact, focused positive/fallback tests, generated inventory, generated delta, quality stack.
