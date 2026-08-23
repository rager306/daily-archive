# M178 Residual Script Candidates

## Verdict

**Candidate review complete.** Residual `script-only=198` is spread across 118 exact source paths. M178 selects two bounded historical families with clear artifact ownership and leaves mixed groups conservative.

## Baseline

```text
total_records=341
script-only=198
unknown=0
shared-state=0
```

## Selected family A: M027 pipeline replay and readiness outputs

Candidate movement: **14 script-only records**.

| Source path | Records | Candidate category | Targets |
|---|---:|---|---|
| `scripts/replay_m027_current_pipeline_baseline.py` | 3 | `m027-pipeline-replay-output` | `path` x3 |
| `scripts/replay_m027_end_to_end_mixed_replay.py` | 3 | `m027-pipeline-replay-output` | `path` x3 |
| `scripts/synthesize_m027_pipeline_readiness.py` | 3 | `m027-pipeline-replay-output` | `path` x3 |
| `scripts/verify_m027_provenance_and_riskratchet_gate.py` | 3 | `m027-pipeline-replay-output` | `path` x3 |
| `scripts/verify_m027_end_to_end_mixed_replay.py` | 2 | `m027-pipeline-replay-output` | `verification_path`, `report_path` |

## Selected family B: M025 recovery and evidence verifier outputs

Candidate movement: **14 script-only records**.

| Source path | Records | Candidate category | Targets |
|---|---:|---|---|
| `scripts/verify_m025_baseline_recovery_replay.py` | 3 | `m025-recovery-evidence-output` | `path` x2, `args.write_events` |
| `scripts/verify_m025_boundary_replay_completion.py` | 3 | `m025-recovery-evidence-output` | `path` x2, `args.write_events` |
| `scripts/verify_m025_evidence_boundaries.py` | 3 | `m025-recovery-evidence-output` | `path` x2, `args.write_events` |
| `scripts/verify_m025_final_preprocessing_replay.py` | 3 | `m025-recovery-evidence-output` | `path` x2, `args.write_events` |
| `scripts/capture_m025_article_sources.py` | 2 | `m025-recovery-evidence-output` | `path`, `target` |

## No-move groups

- M057/M058 residual scripts remain `script-only` because many figure/table scripts mix benchmarks, similarity, and legacy one-off probes already partially reviewed in earlier waves.
- M060/M066 graph benchmark scripts remain `script-only` for a later graph benchmark wave.
- M031/M033 verifier families remain `script-only` for a later parser/external-parser review wave.
- Generic audit, benchmark, acquire, render, repair, and sync scripts remain `script-only`.
- Any path not listed in the selected families remains unchanged.

## Expected movement

```text
m027-pipeline-replay-output=14
m025-recovery-evidence-output=14
script-only target after script wave=170
unknown=0
shared-state=0
```
