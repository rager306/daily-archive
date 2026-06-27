# M184 Graph Connectivity Result

## Verdict

**Graph-connectivity exact wave: PASS with one no-move manifest.**

## Movement

```text
script-only: 45 -> 33
graph-connectivity-probe-output: 0 -> 12
manifest no-move: 1
unknown=0
shared-state=0
total_records=341
```

## Category added

`graph-connectivity-probe-output` covers exact reviewed source paths only:

- `scripts/m063_graphdb_benchmark.py`
- `scripts/probe_m033_opendataloader_adaptix_adapter.py`
- `scripts/probe_m043_sidecar_runtime_readiness.py`
- `scripts/probe_m053_grobid_pilot.py`
- `scripts/repair_m042_linked_metadata.py`
- `scripts/run_m044_live_grobid_candidate_probe.py`
- `scripts/select_m041_mixed_connectivity_batch.py`

## No-move

`m058_build_graph_manifest.py` remains `script-only` because it is manifest-like and lacks exact owner, invalidation, consumer, and concurrency proof.

## Verification

| Check | Result | Evidence |
|---|---|---|
| Fresh baseline | PASS | `gsd_exec[09b49812-0918-482f-9bde-5a75bc85476e]` |
| Candidate file record check | PASS | `gsd_exec[a652fd6a-2c08-4298-9d84-b0e977078d12]` |
| Focused tests after scanner movement | PASS: 37 passed | `gsd_exec[133b0121-6075-44c8-9370-a7d0365b0b97]` |
| Ruff scanner and tests | PASS | `gsd_exec[5c586856-8e24-47a9-b451-568e3f30bd77]` |
| Generated delta before canonical refresh | PASS | `gsd_exec[f1a431df-511a-4f03-96f7-b6fab34acde6]` |
| Canonical refresh, lowered ratchet, strict drift | PASS | `gsd_exec[668cb61c-c4d7-4d2a-a217-55d955ae5908]` |

## Guardrails

- No broad graph/probe/connectivity/linked_metadata/path/tmp/output rule.
- No runtime graph behavior changed.
- No direct extractor-to-graph write introduced.
- Ratchet lowered to `script-only <= 33`.
- Canonical baseline refreshed.
