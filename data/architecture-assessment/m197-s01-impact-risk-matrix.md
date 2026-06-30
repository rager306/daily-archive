# M197 S01 Impact and Risk Matrix

## Verdict

**PASS: reactive implementation must start additive and avoid queue dependency semantic edits until later waves.**

## GitNexus impact summary

| Target | Path | Risk | Direct impact | Affected processes | Planning consequence |
|---|---|---:|---:|---|---|
| `UniversalKBQueue` | `src/research_graph/workflows/universal_kb/queue.py` | MEDIUM | 5 direct imports | No process count reported | Treat as shared orchestration state; prefer wrappers or additive metadata first. |
| `UniversalKBQueue._dependencies_satisfied#1` | `src/research_graph/workflows/universal_kb/queue.py` | HIGH | 2 direct callers | `run_universal_kb_no_write_rehearsal`, `run_article`, `smoke.py main` | Do not edit in early waves; if required, warn first and run exact queue plus no-write compatibility suite. |
| `run_universal_kb_no_write_rehearsal` | `src/research_graph/workflows/universal_kb/rehearsal.py` | LOW | 0 direct upstream | None | Safe as a baseline target; prefer wrapping rather than changing output contract. |
| `run_smoke` | `src/research_graph/workflows/universal_kb/smoke_runner.py` | LOW | 2 direct callers | `smoke.py main` | Suitable for later dry-run CLI compatibility checks. |

## Affected process details

`_dependencies_satisfied` affects:

- `run_universal_kb_no_write_rehearsal` at early process steps.
- `run_article` in smoke runner.
- `main` in `src/research_graph/workflows/universal_kb/smoke.py`.

This confirms the memory warning from M195: dependency and unblocking semantics are a high-risk seam for reactive adoption.

## Risk controls

1. **S02-S03 first:** define event contract and sync baseline before implementation.
2. **S04-S06 additive only:** add async stage runner next to existing sync flow.
3. **S07 queue observability before semantics:** add metadata observations before changing dependency resolution.
4. **S10 compatibility gate:** run queue, rehearsal, smoke, M195, and M196 governance suites before any milestone closeout.
5. **Scoped detect_changes:** use `repo=daily-archive` for GitNexus detect-changes because multiple repos may be indexed.

## Mandatory warning for future edits

Any edit to `UniversalKBQueue._dependencies_satisfied`, `unblock_ready_jobs`, or queue dependency state must be treated as HIGH-risk until proven otherwise. The executor must run exact GitNexus impact, state the blast radius, and verify queue plus no-write rehearsal compatibility before completion.

## Boundary statement

Impact analysis does not authorize production graph writes, schema migrations, or import eligibility. M197 remains no-write and additive until explicit later evidence says otherwise.
