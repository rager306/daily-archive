# Package Rename Wave 18 Manifest

Scope: graph readiness pipeline.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/graph_readiness.py` | `src/research_graph/infrastructure/graph/readiness/core.py` | `archive/package-rename-waves/wave-18/src/arxiv_archive/graph_readiness.py` |
| `src/arxiv_archive/graph_readiness_export.py` | `src/research_graph/infrastructure/graph/readiness/export.py` | `archive/package-rename-waves/wave-18/src/arxiv_archive/graph_readiness_export.py` |
| `src/arxiv_archive/graph_readiness_extraction_gate.py` | `src/research_graph/infrastructure/graph/readiness/extraction_gate.py` | `archive/package-rename-waves/wave-18/src/arxiv_archive/graph_readiness_extraction_gate.py` |
| `src/arxiv_archive/graph_readiness_manifest.py` | `src/research_graph/infrastructure/graph/readiness/manifest.py` | `archive/package-rename-waves/wave-18/src/arxiv_archive/graph_readiness_manifest.py` |
| `src/arxiv_archive/graph_readiness_persistence.py` | `src/research_graph/infrastructure/graph/readiness/persistence.py` | `archive/package-rename-waves/wave-18/src/arxiv_archive/graph_readiness_persistence.py` |
| `src/arxiv_archive/graph_readiness_retrieval_validation.py` | `src/research_graph/infrastructure/graph/readiness/retrieval_validation.py` | `archive/package-rename-waves/wave-18/src/arxiv_archive/graph_readiness_retrieval_validation.py` |
| `src/arxiv_archive/graph_readiness_review.py` | `src/research_graph/infrastructure/graph/readiness/review.py` | `archive/package-rename-waves/wave-18/src/arxiv_archive/graph_readiness_review.py` |

## Verification Notes

- The review artifact post-check command (`uv run python -m research_graph.infrastructure.graph.readiness.review ...`) remains valid.
- No unauthorized graph writes; persistence tests remain no-write.
- Review verdict events must include `output_contract_completed=true` before manifest synthesis.
