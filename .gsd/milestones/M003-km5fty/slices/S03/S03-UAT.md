# S03: Semantic chunks and evidence paths — UAT

**Milestone:** M003-km5fty
**Written:** 2026-05-17T17:30:22.861Z

# UAT — S03 Semantic chunks and evidence paths

## Acceptance checks

1. Structured markdown fixture produces deterministic SemanticChunk records attached to PageIndexNode ids.
2. Chunk ids are stable and idempotent: `{page_index_node_id}:chunk-0001`.
3. Chunk records preserve paper id, PageIndex node id, PageIndex path, order, text, char span, chunking strategy, validation warnings, and provenance.
4. Empty PageIndex sections emit validation diagnostics and no chunk.
5. No-heading fallback sections produce valid traceable chunks.
6. EvidencePath records represent Paper -> PageIndexNode -> SemanticChunk and validate missing node, missing chunk, paper mismatch, node/chunk mismatch, and path mismatch diagnostics.
7. Existing PageIndex, full-text ingestion, analysis, and public CLI contracts remain unchanged.

## Evidence

- `uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` → 44 passed.
- `uv run ruff check src/arxiv_archive/evidence.py tests/test_evidence_paths.py src/arxiv_archive/page_index.py tests/test_page_index.py src/arxiv_archive/full_text.py tests/test_full_text_ingestion.py src/arxiv_archive/cli.py tests/test_cli_contract.py` → All checks passed.
- `uv run python -m arxiv_archive --help` with usage/date/json/cron/Hermes/status lifecycle token assertions → passed.
- LSP diagnostics for `src/arxiv_archive/evidence.py` and `tests/test_evidence_paths.py` → no diagnostics.

