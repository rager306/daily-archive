# Package Rename Wave 14 Manifest

Scope: retrieval/embedding utilities.

## Moves

| Old runtime path | Canonical runtime path | Archive path |
|---|---|---|
| `src/arxiv_archive/embedder.py` | `src/research_graph/infrastructure/retrieval/embedder.py` | `archive/package-rename-waves/wave-14/src/arxiv_archive/embedder.py` |
| `src/arxiv_archive/hybrid_retrieval.py` | `src/research_graph/infrastructure/retrieval/hybrid.py` | `archive/package-rename-waves/wave-14/src/arxiv_archive/hybrid_retrieval.py` |
| `src/arxiv_archive/keyword_extractor.py` | `src/research_graph/infrastructure/retrieval/keyword_extractor.py` | `archive/package-rename-waves/wave-14/src/arxiv_archive/keyword_extractor.py` |
| `src/arxiv_archive/summarizer.py` | `src/research_graph/infrastructure/retrieval/summarizer.py` | `archive/package-rename-waves/wave-14/src/arxiv_archive/summarizer.py` |

## Verification Notes

- Hybrid retrieval remains a deterministic fixture-level baseline; no DSPy/optimizer behavior is introduced.
- `MiniMaxSummarizer` is intentionally not moved because it requires the `anthropic` SDK and live API access; it remains under `src/arxiv_archive` until a later wave that resolves the anthropic dependency.
- Embedder `_load_dotenv_if_present` continues to work after the move since it relies on the working directory `.env`.
