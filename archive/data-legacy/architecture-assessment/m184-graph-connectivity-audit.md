# M184 Graph Connectivity Audit

## Verdict

**Movement decision: move 12 graph/probe/connectivity output records; keep 1 manifest-like record no-move.**

## Baseline

```text
script-only=45
unknown=0
shared-state=0
```

## GitNexus

- `_classify` impact: UNKNOWN, not safety proof.
- GitNexus surfaced graph/connectivity-adjacent flows including link dedup diagnostics, sidecar packet tests, and queue diagnostic update flows.
- No runtime graph behavior changes are made in S07. No direct extractor-to-graph write is introduced.

## Candidate decisions

| Path | Records | Decision | Category or reason |
|---|---:|---|---|
| `scripts/m058_build_graph_manifest.py` | 1 | No move | manifest-like path lacks owner/invalidation/consumer/concurrency proof |
| `scripts/m063_graphdb_benchmark.py` | 1 | Move | `graph-connectivity-probe-output` |
| `scripts/probe_m033_opendataloader_adaptix_adapter.py` | 2 | Move | `graph-connectivity-probe-output` |
| `scripts/probe_m043_sidecar_runtime_readiness.py` | 2 | Move | `graph-connectivity-probe-output` |
| `scripts/probe_m053_grobid_pilot.py` | 1 | Move | `graph-connectivity-probe-output` |
| `scripts/repair_m042_linked_metadata.py` | 2 | Move | `graph-connectivity-probe-output` |
| `scripts/run_m044_live_grobid_candidate_probe.py` | 2 | Move | `graph-connectivity-probe-output` |
| `scripts/select_m041_mixed_connectivity_batch.py` | 2 | Move | `graph-connectivity-probe-output` |

## Boundaries

- No broad `graph`, `probe`, `connectivity`, `linked_metadata`, `path`, `tmp_path`, `output`, or `output_dir` rule.
- No movement for `m058_build_graph_manifest.py`. It remains `script-only` until S11/cache lifecycle proof can prove safe movement.
- No runtime code movement in S07.
