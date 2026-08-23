# M173 Category Candidates

## Summary

| Candidate category | Count | Existing categories |
|---|---:|---|
| `graph-probe-output` | 2 | caller-owned=2 |
| `parser-replay-output` | 3 | caller-owned=2, run-scoped=1 |
| `source-scan-output` | 3 | caller-owned=3 |

## Exact candidate records

### `graph-probe-output`

| Path | Line | Existing category | Target |
|---|---:|---|---|
| `src/research_graph/infrastructure/graph/r024_networkx_probe.py` | 89 | `caller-owned` | `config.summary_path` |
| `src/research_graph/infrastructure/graph/r024_networkx_probe.py` | 94 | `caller-owned` | `config.memory_profile_path` |

### `parser-replay-output`

| Path | Line | Existing category | Target |
|---|---:|---|---|
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 257 | `caller-owned` | `cache_path` |
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 303 | `run-scoped` | `output_path` |
| `src/research_graph/infrastructure/corpus/parsing/replay_adapters.py` | 357 | `caller-owned` | `self.summary_path` |

### `source-scan-output`

| Path | Line | Existing category | Target |
|---|---:|---|---|
| `src/research_graph/infrastructure/corpus/sources/thirty_paper_deviation_scan.py` | 92 | `caller-owned` | `summary_path` |
| `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` | 114 | `caller-owned` | `destination` |
| `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` | 149 | `caller-owned` | `summary_path` |

## Non-candidates and boundaries

- This artifact does not approve categories; S03 freezes scope.
- Do not classify by target names like `summary_path`, `destination`, `cache_path`, or `memory_profile_path` globally.
- Do not reclassify any `shared-state` record in M173.
- Other broad groups remain unchanged until separately reviewed.
