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
| m058_4_layer_9418 | networkx | 3421 | 9418 | 0.009 | 6.565 | 0.006 | 1.579 |
| m058_4_layer_9418 | igraph | 3421 | 9418 | 0.015 | 0.273 | 0.132 | 0.158 |
| m058_4_layer_9418 | rustworkx | 3421 | 9418 | 0.001 | 1.09 | 0.001 | 0.308 |
| synthetic_10000 | networkx | 3525 | 10000 | 6.288 | 7.495 | 0.006 | 1.961 |
| synthetic_10000 | igraph | 3525 | 10000 | 0.149 | 1.6 | 0.198 | 0.388 |
| synthetic_10000 | rustworkx | 3525 | 10000 | 0.524 | 1.437 | 0.002 | 0.328 |
| synthetic_100000 | networkx | 11146 | 100000 | 38.327 | 75.497 | 0.014 | 8.306 |
| synthetic_100000 | igraph | 11146 | 100000 | 0.791 | 8.267 | 1.439 | 0.203 |
| synthetic_100000 | rustworkx | 11146 | 100000 | 4.171 | 11.972 | 0.003 | 2.319 |

## Speedup vs NetworkX

| Graph | Library | Algorithm | Speedup |
|---|---:|---:|---:|
| m058_4_layer_9418 | igraph | bfs | 0.6x |
| m058_4_layer_9418 | igraph | pagerank | 24.048x |
| m058_4_layer_9418 | igraph | shortest_path | 0.045x |
| m058_4_layer_9418 | igraph | connected_components | 9.994x |
| m058_4_layer_9418 | rustworkx | bfs | 9.0x |
| m058_4_layer_9418 | rustworkx | pagerank | 6.023x |
| m058_4_layer_9418 | rustworkx | shortest_path | 6.0x |
| m058_4_layer_9418 | rustworkx | connected_components | 5.127x |
| synthetic_10000 | igraph | bfs | 42.201x |
| synthetic_10000 | igraph | pagerank | 4.684x |
| synthetic_10000 | igraph | shortest_path | 0.03x |
| synthetic_10000 | igraph | connected_components | 5.054x |
| synthetic_10000 | rustworkx | bfs | 12.0x |
| synthetic_10000 | rustworkx | pagerank | 5.216x |
| synthetic_10000 | rustworkx | shortest_path | 3.0x |
| synthetic_10000 | rustworkx | connected_components | 5.979x |
| synthetic_100000 | igraph | bfs | 48.454x |
| synthetic_100000 | igraph | pagerank | 9.132x |
| synthetic_100000 | igraph | shortest_path | 0.01x |
| synthetic_100000 | igraph | connected_components | 40.916x |
| synthetic_100000 | rustworkx | bfs | 9.189x |
| synthetic_100000 | rustworkx | pagerank | 6.306x |
| synthetic_100000 | rustworkx | shortest_path | 4.667x |
| synthetic_100000 | rustworkx | connected_components | 3.582x |
