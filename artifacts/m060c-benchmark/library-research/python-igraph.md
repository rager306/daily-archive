# python-igraph library research

## Architecture summary

GitNexus context found the main `Graph` class at `src/igraph/__init__.py:273-989` in repo `python-igraph`. The Python surface wraps the igraph core and exposes graph construction, traversal, centrality, paths, components, and community APIs through a compact object model.

For our M060b intermediate graph layer, python-igraph is the closest pip-installable acceleration candidate: conversion from NetworkX is available through `Graph.from_networkx`, and the benchmark script can keep NetworkX as the canonical authoring format while using igraph for read-only analysis. Graph writes are not authorized; production import is not authorized; fact promotion is not authorized; external network default is disabled; LLM calls default is disabled.

## Algorithm support table

| Algorithm | Support | Evidence |
|---|---:|---|
| BFS | Yes | GitNexus context on `Graph`; local vendored hits include `src/_igraph/bfsiter.h` and traversal tests. |
| PageRank | Yes | Local vendored hits include `tests/test_structural.py`; benchmark uses `graph.pagerank(weights="weight")`. |
| shortest_path | Yes | Local vendored hits include `tests/test_structural.py`; benchmark uses `get_shortest_paths`. |
| community | Yes | Local vendored hits include clustering/community references and `tests/test_decomposition.py`. |

## Our use case fit

Good fit for M060b and M061+ scale testing when the workload is read-heavy, weighted graph analytics over 10k-100k edges. The API is not NetworkX-compatible, but conversion keeps adoption incremental.

## Decision

**ADOPT** as the primary pip-installable candidate to carry forward into ADR-016 evidence. Rationale: strong algorithm coverage, simple installation, and observed benchmark coverage on our 9418-edge graph plus synthetic scale.
