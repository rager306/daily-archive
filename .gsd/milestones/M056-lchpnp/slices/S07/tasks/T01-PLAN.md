---
estimated_steps: 7
estimated_files: 2
skills_used: []
---

# T01: Emitted diagnostic M056 candidate citation graph JSON from six wave GROBID TEI packets.

scripts/emit_m056_candidate_edges.py that reads all 6 wave GROBID fulltext packets, extracts arxiv IDs from each PDF's references, and emits artifacts/m056-bfs-graph/candidate-edges.json with the citation graph:
- nodes: list of {arxiv_id, title (if available), source_milestone, in_corpus}
- edges: list of {paper_a, paper_b, edge_type: 'cites', citation_count, evidence: 'grobid_biblstruct'}
- 5-flag safety defaults explicit
- Diagnostic only, no graph writes per ADR-006
- Idempotent
- Note: 1-hop BFS yielded 7-8 edges from 149 PDFs, demonstrating saturation. ADR-010 should recommend 2-hop for graph-readiness.

## Inputs

- `artifacts/m056-bfs-graph/wave-1/`
- `artifacts/m056-bfs-graph/wave-2/`
- `artifacts/m056-bfs-graph/wave-3/`
- `artifacts/m056-bfs-graph/wave-4/`
- `artifacts/m056-bfs-graph/wave-5/`
- `artifacts/m056-bfs-graph/wave-6/`

## Expected Output

- `artifacts/m056-bfs-graph/candidate-edges.json`
- `scripts/emit_m056_candidate_edges.py`

## Verification

test -f artifacts/m056-bfs-graph/candidate-edges.json

## Observability Impact

Candidate edges: 7-8 internal edges from 149 PDFs. Diagnostic only.
