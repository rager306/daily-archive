# Package Rename Wave 20 Manifest

Scope: miscellaneous modules, LLM modules, and cleanup.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/evidence.py` | `src/research_graph/papers/evidence_legacy.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/evidence.py` |
| `src/arxiv_archive/analytics.py` | `src/research_graph/evaluation/analytics.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/analytics.py` |
| `src/arxiv_archive/scoring.py` | `src/research_graph/evaluation/scoring.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/scoring.py` |
| `src/arxiv_archive/import_boundary_rehearsal.py` | `src/research_graph/workflows/import_boundary_rehearsal.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/import_boundary_rehearsal.py` |
| `src/arxiv_archive/reviewer_packet_prototype.py` | `src/research_graph/workflows/review_packet_prototype.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/reviewer_packet_prototype.py` |
| `src/arxiv_archive/models_registry.py` | `src/research_graph/llm/models_registry.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/models_registry.py` |
| `src/arxiv_archive/thirty_paper_deviation_scan.py` | `src/research_graph/corpus/sources/thirty_paper_deviation_scan.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/thirty_paper_deviation_scan.py` |
| `src/arxiv_archive/minimax_structured.py` | `src/research_graph/llm/minimax_structured.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/minimax_structured.py` |
| `src/arxiv_archive/minimax_usage.py` | `src/research_graph/llm/minimax_usage.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/minimax_usage.py` |
| `src/arxiv_archive/llm/__init__.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/llm/__init__.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/llm/__init__.py` |
| `src/arxiv_archive/artifacts/__init__.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/artifacts/__init__.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/artifacts/__init__.py` |
| `src/arxiv_archive/chunk_repair_contract.py` | `src/research_graph/repair/chunk_repair_contract.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/chunk_repair_contract.py` |
| `src/arxiv_archive/chunk_import_contract.py` | `src/research_graph/repair/chunk_import_contract.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/chunk_import_contract.py` |
| `src/arxiv_archive/chunk_baseline_measurement.py` | `src/research_graph/repair/chunk_baseline_measurement.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/chunk_baseline_measurement.py` |
| `src/arxiv_archive/cli.py` | `src/research_graph/cli.py` | `archive/package-rename-waves/wave-20/src/arxiv_archive/cli.py` |
| `src/arxiv_archive/__main__.py` | archive-only | `archive/package-rename-waves/wave-20/src/arxiv_archive/__main__.py` |
| `src/arxiv_archive/__init__.py` | archive-only | `archive/package-rename-waves/wave-20/src/arxiv_archive/__init__.py` |

## Verification Notes

- `evidence.py` moved to `papers/evidence_legacy.py` to avoid collision with existing canonical `papers/evidence.py`.
- `scoring.py` was unblocked by S07 (arxiv_client and semantic_scholar are now canonical).
- `llm/__init__.py` and `artifacts/__init__.py` were archive-only moves (no canonical replacement needed).
