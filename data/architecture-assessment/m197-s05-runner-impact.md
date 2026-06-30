# M197 S05 Runner Impact

## Verdict

**RESOLVED: the first two background GitNexus re-index attempts used unsupported `--repo` and failed, but a corrected `gitnexus analyze` succeeded and GitNexus now resolves `run_reactive_stage` by exact UID.** Exact impact is LOW: direct caller `run_one`, upstream `run_reactive_stages_bounded`, no affected processes.

## Evidence

| Check | Result | Evidence |
|---|---|---|
| GitNexus re-index after S04 | FAILED | background job `bg_fb1adf21`: `unknown option '--repo'` |
| GitNexus re-index after track | FAILED | background job `bg_6e1cf1a8`: `unknown option '--repo'` |
| Corrected GitNexus re-index | PASS | `gitnexus analyze` succeeded: 45,982 nodes, 63,359 edges, 962 clusters, 300 flows |
| GitNexus context for `run_reactive_stage` | FOUND | exact symbol `Function:src/research_graph/workflows/universal_kb/reactive_runner.py:run_reactive_stage` |
| GitNexus impact for `run_reactive_stage` | LOW | impacted_count=2; direct caller `run_one`; upstream `run_reactive_stages_bounded`; no affected processes |
| Working tree probe before track | New runner and tests were untracked | `gsd_exec[748747ff-2085-435a-9ac1-0cffa32ec876]` |
| Local commit for indexability | Completed | `44016f8 feat: add reactive no-write pipeline foundation` |
| codebase-memory search and trace | FOUND, supplemental | `root-daily-archive.src.research_graph.workflows.universal_kb.reactive_runner.run_reactive_stage`; no inbound callers before GitNexus caught up |

## Current status

- Source file is tracked in local commit `44016f8`.
- GitNexus now resolves `run_reactive_stage` after corrected `gitnexus analyze`.
- Exact GitNexus impact for `run_reactive_stage` is LOW.
- codebase-memory also resolves the symbol and remains useful as supplemental evidence for newly added code.
- Existing high-risk queue semantics remain untouched.
- S05 T02 was allowed to edit only `reactive_runner.py` and its tests.

## Guardrails for S05 T02

1. Edit only `src/research_graph/workflows/universal_kb/reactive_runner.py` and `tests/test_m197_reactive_runner.py`.
2. Do not edit queue, rehearsal, smoke runner, or smoke wrapper files.
3. Run focused M197 tests plus M195/M196 governance compatibility.
4. Run scoped GitNexus detect_changes after edits.

## Boundary statement

No queue/rehearsal/smoke source edit is authorized by this impact note. Graph backend writes, schema migration execution, production graph import, and `import_eligible=true` remain blocked.
