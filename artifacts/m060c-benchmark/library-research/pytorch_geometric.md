# pytorch_geometric library research

## Architecture summary

GitNexus context found the main `Data` class at `torch_geometric/data/data.py:471-1248`. PyTorch Geometric models graphs as tensor data for neural network workloads rather than as a drop-in imperative graph analytics layer.

The architecture is powerful for GNN training, transforms, datasets, loaders, and explainability, but it is not optimized for our current M060b use case: deterministic graph-layer analytics over citation/similarity/cluster/continuity edges. Graph writes are not authorized; production import is not authorized; fact promotion is not authorized; external network default is disabled; LLM calls default is disabled.

## Algorithm support table

| Algorithm | Support | Evidence |
|---|---:|---|
| BFS | Partial | Local vendored hits include layer-trimming and influence utilities; not a primary NetworkX-style BFS surface. |
| PageRank | Partial | Local vendored hits include `torch_geometric/transforms/gdc.py`; mostly ML preprocessing/transform context. |
| shortest_path | Partial | Local vendored hits include example/dataset path features; not the primary graph analytics abstraction. |
| community | Partial | Local vendored hits include SBM datasets/transforms; community is ML-data oriented. |

## Our use case fit

Poor near-term fit for M060b intermediate layer because adoption would require tensor conversion, PyTorch dependency weight, and ML-oriented data semantics. It may matter later if M065 introduces GNN-based retrieval or graph embeddings.

## Decision

**DEFER**. Rationale: useful for future ML experiments, but the abstraction is heavier than needed for 10k-100k edge deterministic graph analytics.
