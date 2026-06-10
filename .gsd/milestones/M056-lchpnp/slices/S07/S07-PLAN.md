# S07: Final REPORT + candidate-edges.json + ADR-010

**Goal:** Final synthesis: REPORT.md + candidate-edges.json + ADR-010 (or ADR-009 acknowledgment). Closes M056, feeds M058 graph-readiness gate.
**Demo:** Comprehensive REPORT.md (167-PDF BFS), candidate-edges.json (citation graph), ADR-010 (BFS scale evidence for ADR-002).

## Must-Haves

- artifacts/m056-bfs-graph/REPORT.md (~500+ lines) with all 6 waves
- artifacts/m056-bfs-graph/candidate-edges.json (citation graph diagnostic)
- doc/adr/ADR-010-bfs-scale-167-pdf.md
- 5+ tests pass
- 5 safety defaults stay false across all artifacts
- gsd_decision_save emits D-number
- M045 trajectory on_track, M044 guardrail exit 0
- M050-M055deep tests still pass
- 1 commit in git history
- M056 ready for closeout

## Proof Level

- This slice proves: operational

## Integration Closure

Closes M056. Closes M045 next_gate. Feeds M058 graph-readiness gate with recommendation.

## Verification

- REPORT + candidate-edges + ADR-010 + D-number.

## Tasks

- [x] **T01: Emitted diagnostic M056 candidate citation graph JSON from six wave GROBID TEI packets.** `est:30m`
  scripts/emit_m056_candidate_edges.py that reads all 6 wave GROBID fulltext packets, extracts arxiv IDs from each PDF's references, and emits artifacts/m056-bfs-graph/candidate-edges.json with the citation graph:
  - nodes: list of {arxiv_id, title (if available), source_milestone, in_corpus}
  - edges: list of {paper_a, paper_b, edge_type: 'cites', citation_count, evidence: 'grobid_biblstruct'}
  - 5-flag safety defaults explicit
  - Diagnostic only, no graph writes per ADR-006
  - Idempotent
  - Note: 1-hop BFS yielded 7-8 edges from 149 PDFs, demonstrating saturation. ADR-010 should recommend 2-hop for graph-readiness.
  - Files: `artifacts/m056-bfs-graph/candidate-edges.json`, `scripts/emit_m056_candidate_edges.py`
  - Verify: test -f artifacts/m056-bfs-graph/candidate-edges.json

- [x] **T02: Rendered the final M056 1-hop BFS REPORT.md synthesis artifact.** `est:30m`
  scripts/render_m056_report.py producing artifacts/m056-bfs-graph/REPORT.md (~500+ lines) with all 6 waves:
  - Executive summary: 1-hop BFS 166 refs from 2605.18747, 148 acquired, 149 unique PDFs, 7-8 internal edges
  - Per-wave tables (6 waves): refs range, acquisition stats, parser quality, edges
  - Edge saturation chart (3→2→1→2→0→? cumulative)
  - Per-PDF summary table (149 rows)
  - Self-citation cluster (0% — healthy diversity)
  - Category distribution
  - Length distribution
  - Routing recommendation (per ADR-009 fulltext-aware hybrid)
  - 5-flag safety block
  - Recomendación: 2-hop expansion needed for graph-readiness (1-hop insufficient for M058)
  - Mermaid diagram per D067
  - Schema version: m056-bfs-graph-report.v1
  - Files: `artifacts/m056-bfs-graph/REPORT.md`, `scripts/render_m056_report.py`
  - Verify: test -f artifacts/m056-bfs-graph/REPORT.md

- [x] **T03: Added ADR-010 and recorded GSD decision D084 for BFS scale evidence.** `est:20m`
  Draft ADR-010 BFS-Scale 167-PDF Evidence at doc/adr/ADR-010-bfs-scale-167-pdf.md per D067 Mermaid-assisted template:
  - Status: Accepted (binding) — supplements ADR-009
  - Context: M055 5-PDF + M055deep 20-PDF + M056 149-PDF (1-hop BFS from 2605.18747)
  - Decision: 1-hop BFS from 2605.18747 yields 7-8 internal edges from 149 unique PDFs, demonstrating saturation. For graph-readiness gate (M058), 2-hop BFS expansion is recommended.
  - Mermaid diagram
  - 5-flag safety defaults explicit
  - Rationale: empirical evidence of 1-hop saturation, recommendation for 2-hop
  - Alternatives: anchor choice, BFS depth
  - Consequences: M058 needs 2-hop OR different anchor
  - Files: `doc/adr/ADR-010-bfs-scale-167-pdf.md`, `doc/adr/ADR-INDEX.md`
  - Verify: test -f doc/adr/ADR-010-bfs-scale-167-pdf.md

- [x] **T04: Added S07 final tests and ran required regression, trajectory, and guardrail verification.** `est:20m`
  tests/test_m056_final_s07.py with 5+ tests:
  1. test_report_contains_executive_summary
  2. test_report_contains_6_wave_summaries
  3. test_candidate_edges_json_schema
  4. test_adr_010_exists_and_references_m056
  5. test_5_safety_defaults_all_false
  6. M050-M055deep regression: all still pass
  - Files: `tests/test_m056_final_s07.py`
  - Verify: uv run pytest tests/test_m056_final_s07.py -q

## Files Likely Touched

- artifacts/m056-bfs-graph/candidate-edges.json
- scripts/emit_m056_candidate_edges.py
- artifacts/m056-bfs-graph/REPORT.md
- scripts/render_m056_report.py
- doc/adr/ADR-010-bfs-scale-167-pdf.md
- doc/adr/ADR-INDEX.md
- tests/test_m056_final_s07.py
