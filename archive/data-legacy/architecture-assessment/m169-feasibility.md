# M169 Feasibility

## Decision

**Proceed with all three requested items together.**

S01 found no baseline blocker that requires falling back to item 1 only. The three tracks touch mostly separate surfaces and can be integrated at S11:

1. M061 dynamic test reconciliation: `tests/test_m061_s03.py` and related M061 artifacts.
2. Remaining unknown write paths: `src/research_graph/cli/__init__.py`, `src/research_graph/infrastructure/corpus/ingestion/fetchers.py`, and `scripts/inventory_write_paths.py`.
3. Multiprocess queue soak: `tests/test_universal_kb_queue.py` and potentially `src/research_graph/workflows/universal_kb/queue.py` only if the test exposes a real bug.

## Why grouping is safe

- Current baseline is green:
  - test architecture guard passes;
  - inventory generation passes;
  - queue suite passes.
- The three tracks have clear dependency separation until integrated verification:
  - S02-S04 handle M061 import and artifact authority;
  - S05-S08 handle write-path ownership and inventory;
  - S09-S10 handle queue soak design and implementation.
- The likely edits are small and separately testable.
- S11 explicitly verifies combined effects before milestone closeout.

## Stop conditions

Fallback to item 1 only, or close a track as blocked, if any of these happen:

1. M061 reconciliation requires rewriting protected historical artifacts without a clear authority decision.
2. Unknown write-path resolution requires changing production write semantics without a focused safety test.
3. Multiprocess queue soak becomes slow, flaky, or requires changing queue internals without a reproduced concurrency bug.
4. GitNexus or guardrails report high or critical risk for a proposed edit.

## Track plan

| Track | Slices | Target outcome |
|---|---|---|
| M061 dynamic test | S02-S04 | Remove final dynamic allowlist if safe, otherwise document authoritative blocker. |
| Unknown write paths | S05-S08 | Reduce unknown count from 3 toward zero without hiding shared-state risk. |
| Queue multiprocess soak | S09-S10 | Add bounded multiprocess stress proof with diagnostics. |
| Integration | S11-S12 | Verify all accepted changes together and close milestone cleanly. |

## Feasibility verdict

Proceed to S02, S05, and S09 according to roadmap. Because S02 and S05/S09 are independent after S01, the milestone remains feasible as a grouped remediation batch.
