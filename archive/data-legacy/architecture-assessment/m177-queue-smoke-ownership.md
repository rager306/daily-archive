# M177 Queue and Smoke Output Ownership

## Verdict

**Move exact reviewed script outputs only.** M177 will classify 11 queue/smoke script records and preserve existing workflow categories under `src/research_graph/workflows/universal_kb/`.

## Allowed movement

| Category | Source paths | Records |
|---|---|---:|
| `queue-soak-output` | `scripts/soak_universal_kb_queue.py` | 1 |
| `queue-gate-output` | `scripts/verify_m072_queue_benchmark_gate.py`, `scripts/verify_m073_queue_evidence_gate.py` | 2 |
| `smoke-script-output` | `scripts/m060g_smoke_test.py`, `scripts/replay_m028_smoke_closeout.py`, `scripts/run_m029_unified_loader_runtime_smoke.py`, `scripts/verify_m029_unified_loader_runtime_smoke.py`, `scripts/run_m122_mutation_smoke.py` | 8 |

## No-move workflow records

| Source path | Current categories | Decision |
|---|---|---|
| `src/research_graph/workflows/universal_kb/queue.py` | `database` | preserve |
| `src/research_graph/workflows/universal_kb/smoke.py` | `caller-owned` | preserve |
| `src/research_graph/workflows/universal_kb/smoke_audit.py` | `caller-owned`, `run-scoped` | preserve |
| `src/research_graph/workflows/universal_kb/smoke_runner.py` | `caller-owned` | preserve |
| `src/research_graph/workflows/universal_kb/smoke_selection.py` | `run-scoped` | preserve |

## Rationale

- Queue database state remains `database`, not a script output category.
- Workflow smoke outputs are already typed by ownership and should not be dragged by broad `smoke` or `queue` name matching.
- Script outputs are process-boundary evidence artifacts and can move by exact source path.

## Expected movement

```text
queue-soak-output=1
queue-gate-output=2
smoke-script-output=8
script-only residual after S07=198
unknown=0
shared-state=0
```
