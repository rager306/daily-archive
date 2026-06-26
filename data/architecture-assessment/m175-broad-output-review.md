# M175 Broad Output Review

## Verdict

**Broad review status: PASS.** One exact broad-family movement is safe for M175: validation batch workflow outputs. Other mixed broad records stay conservative.

## Remaining broad counts after proposed daily CLI movement

```text
script-only=264
caller-owned=18
run-scoped=8
temporary=1
append-log=1
```

## Safe movement proposal

```text
validation-batch-output=10
```

Exact scope:

```text
src/research_graph/workflows/validation/batch_workflow.py
```

Records:

| Line | Current category | Target | Rationale |
|---:|---|---|---|
| 138 | caller-owned | `selection_manifest_path` | validation batch selection manifest |
| 236 | caller-owned | `summary_path` | source preflight summary |
| 326 | caller-owned | `delta_path` | validation scan delta report |
| 329 | caller-owned | `outlier_path` | validation scan outlier report |
| 446 | caller-owned | `path` | validation scan review summary |
| 470 | run-scoped | `output_path` | validation scan manifest |
| 493 | run-scoped | `output_path` | validation scan source readiness |
| 508 | caller-owned | `summary_path` | validation scan summary |
| 649 | caller-owned | `summary_path` | quota fill summary |
| 736 | caller-owned | `summary_path` | bounded top-up summary |

These records are one workflow-owned artifact family around validation batch review and diagnostics. They include fail-closed safety payloads and do not write directly to the graph.

## Explicit no-move groups

| Records | Current category | Reason to keep broad |
|---|---|---|
| `src/research_graph/infrastructure/corpus/reporting/coverage_report.py` x2 | caller-owned | Coverage report paths are caller-supplied report destinations; review separately if needed. |
| `src/research_graph/infrastructure/corpus/sources/markdown_converter.py` x2 | caller-owned | Cache-like markdown and method paths should remain conservative until cache policy review. |
| `src/research_graph/infrastructure/quality/gate.py` x1 | caller-owned | Generic gate output path; no exact family batch here. |
| `src/research_graph/infrastructure/quality/maintainability_report.py` x1 | caller-owned | Generic report path; no exact family batch here. |
| `src/research_graph/workflows/universal_kb/*` x5 | caller-owned or run-scoped | Queue and smoke outputs are state-like and should remain conservative. |
| `src/research_graph/infrastructure/papers/chunking/chunker.py` x2 | run-scoped and append-log | Structure-aware summary plus diagnostics can be reviewed later, but moving the only append-log record in M175 would hide that conservative signal. |
| `script-only=264` | script-only | Too mixed for M175; keep as separate future script inventory work. |
| `temporary=1` | temporary | Atomic-write temp helper remains temporary. |

## Expected count movement if accepted

```text
validation-batch-output +10
caller-owned -8
run-scoped -2
append-log 0
temporary 0
script-only 0
```

Expected broad buckets after S02 plus this movement:

```text
caller-owned=10
run-scoped=6
append-log=1
temporary=1
script-only=264
```

## Safety rules

- Do not classify generic `summary_path`, `output_path`, `path`, or `delta_path` outside the exact validation batch workflow file.
- Do not move cache-like markdown converter outputs in M175.
- Do not move queue or smoke outputs in M175.
- Preserve `append-log` visibility in M175.

## Evidence

- Remaining broad record extraction: `gsd_exec[b87a1add-87be-44d3-8bd1-90ae56c6a6df]`
- Source context reviewed in `src/research_graph/workflows/validation/batch_workflow.py`.
