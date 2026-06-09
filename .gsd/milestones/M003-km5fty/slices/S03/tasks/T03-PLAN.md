---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Added EvidencePath validation coverage for missing chunks, node-path mismatch, and fallback evidence.

Implement `EvidencePath` construction and validation helpers that prove each path references an existing paper id, PageIndexNode id, and SemanticChunk id. Add tests for valid paths, missing node, missing chunk, paper mismatch, and fallback-section evidence. Done when S04 can reference evidence paths without revalidating PageIndex internals.

## Inputs

- `src/arxiv_archive/evidence.py`
- `tests/test_evidence_paths.py`
- `src/arxiv_archive/page_index.py`

## Expected Output

- `src/arxiv_archive/evidence.py`
- `tests/test_evidence_paths.py`

## Verification

uv run pytest tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py -q

## Observability Impact

Validation returns explicit diagnostics for missing nodes, missing chunks, paper mismatches, and invalid node/chunk attachment.
