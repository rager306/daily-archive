# M181 Cache Ownership Candidates

## Script-only cache-like residuals

| Category | Path | Target | Line |
|---|---|---|---:|
| script-only | `scripts/benchmark_m055_corpus_manifest.py` | `output_path` | 118 |
| script-only | `scripts/build_m055deep_corpus_manifest_20.py` | `output_path` | 224 |
| script-only | `scripts/m058_build_graph_manifest.py` | `path` | 53 |
| script-only | `scripts/m059_build_manifest.py` | `actual_output` | 179 |

## Already-reviewed cache-like context

| Category | Path | Target | Line |
|---|---|---|---:|
| parser-replay-output | `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | `cache_path` | 257 |
| caller-owned-index | `src/research_graph/infrastructure/repair/chunk_baseline_measurement.py` | `index_path` | 183 |

## Counts

```text
script_only_cache_like=4
already_reviewed_context=2
```
