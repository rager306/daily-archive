---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Implemented deterministic SemanticChunk construction and the initial EvidencePath boundary.

Implement `src/arxiv_archive/evidence.py` with `SemanticChunk`, deterministic chunking from `PageIndexDocument`, chunk provenance, chunk order, and character spans. Keep chunking simple and deterministic: section-level or paragraph-aware chunks over PageIndexNode text, no embeddings or LLM calls. Done when initial SemanticChunk contract tests pass together with PageIndex tests.

## Inputs

- `tests/test_evidence_paths.py`
- `src/arxiv_archive/page_index.py`

## Expected Output

- `src/arxiv_archive/evidence.py`

## Verification

uv run pytest tests/test_evidence_paths.py tests/test_page_index.py -q

## Observability Impact

Chunk results expose source node id, span start/end, strategy, order, warnings, and provenance for downstream debugging.
