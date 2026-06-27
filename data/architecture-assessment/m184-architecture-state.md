# M184 Architecture State

## Verdict

**Architecture crystallization state: current and ratcheted.**

## Inventory state

```text
total_records=341
script-only=4
unknown=0
shared-state=0
```

M184 reduced script-only from 89 to 4. The remaining four are explicit manifest/cache no-move records.

## M184 category ownership additions

```text
source-acquisition-evidence-output=10
audit-analysis-output=24
render-report-contract-output=8
replay-conversion-output=2
graph-connectivity-probe-output=12
governance-sync-output=4
experiment-probe-output=12
misc-architecture-artifact-output=13
```

## Remaining script-only no-move records

- `scripts/benchmark_m055_corpus_manifest.py`
- `scripts/build_m055deep_corpus_manifest_20.py`
- `scripts/m058_build_graph_manifest.py`
- `scripts/m059_build_manifest.py`

These require owner, invalidation, consumer, concurrency, and lifecycle proof before movement.

## Wrapper extraction pattern

S09 established the first script-to-src pattern:

- reusable logic moved to `src/research_graph/application/corpus/article_catalog_selection.py`;
- `scripts/verify_article_catalog.py` remains a thin wrapper;
- tests cover helper behavior and wrapper delegation;
- no speculative `Protocol`, factory, or extra abstraction was added.

## Active guardrails

- Canonical inventory baseline remains CI truth.
- Ratchet is `script-only <= 4`, `unknown == 0`, `shared-state == 0`.
- Cache/index/manifest movement is fail-closed without lifecycle proof.
- No direct extractor-to-graph write was introduced.
- GitNexus UNKNOWN remains non-proof and is compensated with tests, deltas, strict drift, and final detect_changes.

## Next horizon

1. Choose next script-to-src extraction only from low-risk seams with behavior tests.
2. Revisit the four manifest/cache no-move records only when lifecycle proof exists.
3. Consider exposing `build_current_catalog_index_selection` via package `__init__` only if a second consumer appears.
