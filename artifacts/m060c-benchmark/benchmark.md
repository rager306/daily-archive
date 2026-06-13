# M060c S01 Graph Library Benchmark

Research-only benchmark for igraph and rustworkx against the existing NetworkX baseline.

- Runs per operation: 5
- Loopback host for any local-only checks: `127.0.0.1`
- Runtime integration: none; this is ADR-016 evidence only.

## Safety defaults

- Graph writes are not authorized.
- Production import is not authorized.
- Fact promotion is not authorized.
- External network default is disabled.
- LLM calls default is disabled.

## Latency table (median ms)

| Graph | Library | Nodes | Edges | BFS | PageRank | Shortest path | Connected components |
|---|---:|---:|---:|---:|---:|---:|---:|
| m058_4_layer_9418 | networkx | 3421 | 9418 | 0.008 | 8.327 | 0.006 | 2.332 |
| m058_4_layer_9418 | igraph | 3421 | 9418 | 0.025 | 0.38 | 0.136 | 0.154 |
| m058_4_layer_9418 | rustworkx | 3421 | 9418 | 0.001 | 1.068 | 0.001 | 0.27 |
| synthetic_10000 | networkx | 3525 | 10000 | 5.171 | 7.861 | 0.006 | 1.66 |
| synthetic_10000 | igraph | 3525 | 10000 | 0.234 | 214.193 | 0.18 | 0.352 |
| synthetic_10000 | rustworkx | 3525 | 10000 | 0.381 | 1.233 | 0.001 | 0.316 |
| synthetic_100000 | networkx | 11146 | 100000 | 32.356 | 70.157 | 0.008 | 7.668 |
| synthetic_100000 | igraph | 11146 | 100000 | 0.766 | 13.368 | 1.677 | 0.215 |
| synthetic_100000 | rustworkx | 11146 | 100000 | 2.688 | 11.606 | 0.005 | 2.138 |

## Speedup vs NetworkX

| Graph | Library | Algorithm | Speedup |
|---|---:|---:|---:|
| m058_4_layer_9418 | igraph | bfs | 0.32x |
| m058_4_layer_9418 | igraph | pagerank | 21.913x |
| m058_4_layer_9418 | igraph | shortest_path | 0.044x |
| m058_4_layer_9418 | igraph | connected_components | 15.143x |
| m058_4_layer_9418 | rustworkx | bfs | 8.0x |
| m058_4_layer_9418 | rustworkx | pagerank | 7.797x |
| m058_4_layer_9418 | rustworkx | shortest_path | 6.0x |
| m058_4_layer_9418 | rustworkx | connected_components | 8.637x |
| synthetic_10000 | igraph | bfs | 22.098x |
| synthetic_10000 | igraph | pagerank | 0.037x |
| synthetic_10000 | igraph | shortest_path | 0.033x |
| synthetic_10000 | igraph | connected_components | 4.716x |
| synthetic_10000 | rustworkx | bfs | 13.572x |
| synthetic_10000 | rustworkx | pagerank | 6.376x |
| synthetic_10000 | rustworkx | shortest_path | 6.0x |
| synthetic_10000 | rustworkx | connected_components | 5.253x |
| synthetic_100000 | igraph | bfs | 42.24x |
| synthetic_100000 | igraph | pagerank | 5.248x |
| synthetic_100000 | igraph | shortest_path | 0.005x |
| synthetic_100000 | igraph | connected_components | 35.665x |
| synthetic_100000 | rustworkx | bfs | 12.037x |
| synthetic_100000 | rustworkx | pagerank | 6.045x |
| synthetic_100000 | rustworkx | shortest_path | 1.6x |
| synthetic_100000 | rustworkx | connected_components | 3.587x |
