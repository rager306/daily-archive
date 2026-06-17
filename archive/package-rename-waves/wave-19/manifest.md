# Package Rename Wave 19 Manifest

Scope: RLM workflow modules.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/rlm_graph_traversal.py` | `src/research_graph/workflows/rlm/graph_traversal.py` | `archive/package-rename-waves/wave-19/src/arxiv_archive/rlm_graph_traversal.py` |
| `src/arxiv_archive/rlm_workflow.py` | `src/research_graph/workflows/rlm/workflow.py` | `archive/package-rename-waves/wave-19/src/arxiv_archive/rlm_workflow.py` |

## Verification Notes

- No live provider or optimizer calls are introduced.
- Traversal phases and failure points remain visible under canonical workflow boundaries.
