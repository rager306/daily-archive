# dgl library research

## Architecture summary

GitNexus context found the main `DGLGraph` class at `python/dgl/heterograph.py:39-6398`. DGL is a deep learning graph framework with heterograph support, sampling, propagation, transforms, and neural-network integrations.

For our M060b graph layer, DGL is not a clean replacement for NetworkX. Its strengths are GNN pipelines and message passing, not lightweight read-only graph analytics over our manifest. Graph writes are not authorized; production import is not authorized; fact promotion is not authorized; external network default is disabled; LLM calls default is disabled.

## Algorithm support table

| Algorithm | Support | Evidence |
|---|---:|---|
| BFS | Partial | Local vendored hits include `python/dgl/traversal.py` and propagation modules. |
| PageRank | Partial | Local vendored hits include examples and transforms; not the main adoption reason. |
| shortest_path | Partial | Local vendored hits include transform/encoder path modules. |
| community | Partial | Local vendored hits include sparse/community-adjacent references but not a simple graph-layer surface. |

## Our use case fit

Poor near-term fit for M060b and M061 scale benchmark needs because it adds large ML framework complexity without improving the current authoring and analytics contract.

## Decision

**DEFER**. Rationale: revisit only if future milestones require GNN training, heterograph ML, or message-passing features.
