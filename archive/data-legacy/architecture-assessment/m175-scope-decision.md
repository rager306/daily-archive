# M175 Scope Decision

## Verdict

**Scope status: APPROVED FOR IMPLEMENTATION after impact check.**

M175 includes all three user-requested directions together:

1. command-specific CLI output categories;
2. remaining mixed broad output review with one exact movement;
3. minimal CI/report-friendly inventory delta rendering.

## Categories to add

| Category | Count | Exact source scope | Exclusions |
|---|---:|---|---|
| `daily-cli-output` | 5 | `src/research_graph/cli/__init__.py` durable daily CLI artifacts | `temp_path`, generic `filepath`, generic `day_dir`, article artifact command package |
| `validation-batch-output` | 10 | `src/research_graph/workflows/validation/batch_workflow.py` validation batch artifacts | generic `summary_path`, `output_path`, `path`, `delta_path` in other files |

## Delta reporting to add

Add optional scanner CLI support:

```text
--delta-from BASELINE.json
--delta-markdown DELTA.md
```

Rules:

- Both optional args are required together.
- Delta report compares `summary.total_records` and `summary.by_category` only.
- The report is generated from inventory JSON payloads.
- No new module, package, dependency, or full record-level diff.

## No-move groups

These remain broad or conservative in M175:

- `temp_path` in `src/research_graph/cli/__init__.py` remains `temporary`.
- `src/research_graph/cli/commands/article_artifacts.py` remains `article-artifact-package`.
- Coverage report writer outputs remain `caller-owned`.
- Markdown converter cache-like paths remain `caller-owned`.
- Quality report outputs remain `caller-owned`.
- Universal KB queue and smoke outputs remain broad/conservative.
- Structure-aware chunker summary and diagnostics remain `run-scoped` and `append-log` to preserve append-log visibility.
- `script-only=264` remains separate future script inventory work.

## Expected final movement

```text
daily-cli-output +5
validation-batch-output +10
caller-owned -10
run-scoped -5
script-only +1
```

Expected final broad buckets:

```text
caller-owned=10
run-scoped=6
append-log=1
temporary=1
script-only=265
unknown=0
shared-state=0
total_records=341
```

## Required tests

- `daily-cli-output` positive classification.
- `daily-cli-output` excludes `temp_path`.
- `daily-cli-output` does not classify generic `filepath` in other files.
- `validation-batch-output` positive classification.
- `validation-batch-output` does not classify generic `summary_path` in other files.
- Delta markdown rendering includes category and total deltas.

## Safety notes

- Pre-edit GitNexus impact is required before implementation.
- GitNexus target-not-found remains UNKNOWN, not proof.
- Final detect_changes must be LOW or explicitly reviewed before closeout.
