# Package Rename Wave 13 Manifest

Scope: extraction and evaluation deterministic surface.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/dspy_extraction.py` | `src/research_graph/evaluation/dspy_extraction.py` | `archive/package-rename-waves/wave-13/src/arxiv_archive/dspy_extraction.py` |
| `src/arxiv_archive/extraction_benchmark.py` | `src/research_graph/evaluation/extraction_benchmark.py` | `archive/package-rename-waves/wave-13/src/arxiv_archive/extraction_benchmark.py` |
| `src/arxiv_archive/scientific_extraction.py` | `src/research_graph/evaluation/scientific_extraction.py` | `archive/package-rename-waves/wave-13/src/arxiv_archive/scientific_extraction.py` |
| `src/arxiv_archive/evaluation.py` | `src/research_graph/evaluation/metrics.py` (renamed to avoid module/package shadowing) | `archive/package-rename-waves/wave-13/src/arxiv_archive/evaluation.py` |

## Verification Notes

- DSPy optimizer/provider execution remains disabled; the boundary rejects `optimizer_config` with a diagnostic reason.
- No live MiniMax/GLM/DSPy provider calls are introduced by this wave.
- Evaluation metrics remain fixture-level and deterministic; benchmarks do not call external models.
- `src/arxiv_archive/scoring.py` (scoring.py) remains under the old package because it depends on `arxiv_client` and `semantic_scholar` (S07 dependency).
