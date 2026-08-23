# M190 S03 Metric Ablation Results

## Verdict

**PASS: representative metric, ablation, DSPy boundary, and low-quality source criteria gates passed.**

## Evidence

| Gate | Result | Evidence |
|---|---|---|
| Extraction benchmark, evaluation benchmark, DSPy boundary tests | PASS: 23 passed | `gsd_exec[3e062dab-df32-4f4b-b0fe-888023251a64]` |
| Focused low-quality source criteria tests | PASS: 4 passed, 11 deselected | `gsd_exec[ab618f8f-7bb2-4e2f-8296-7f453f59a106]` |

## Observed outputs against expected contract

- `extraction_metric_gate_passed`: true.
- `retrieval_ablation_gate_passed`: true.
- `dspy_boundary_gate_passed`: true.
- `low_quality_source_fail_closed`: true.
- `optimizer_enabled=false`: preserved.
- `graph_import_ready=false`: preserved.
- `production_persistence_ready=false`: preserved.

## Scope boundary

These gates validate representative metric and ablation behavior for the bounded execution wave. They do not activate DSPy optimization, claim production hybrid retrieval quality, import graph state, or write production persistence state.
