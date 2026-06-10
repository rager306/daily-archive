---
estimated_steps: 12
estimated_files: 2
skills_used: []
---

# T03: Added ADR-010 and recorded GSD decision D084 for BFS scale evidence.

Draft ADR-010 BFS-Scale 167-PDF Evidence at doc/adr/ADR-010-bfs-scale-167-pdf.md per D067 Mermaid-assisted template:
- Status: Accepted (binding) — supplements ADR-009
- Context: M055 5-PDF + M055deep 20-PDF + M056 149-PDF (1-hop BFS from 2605.18747)
- Decision: 1-hop BFS from 2605.18747 yields 7-8 internal edges from 149 unique PDFs, demonstrating saturation. For graph-readiness gate (M058), 2-hop BFS expansion is recommended.
- Mermaid diagram
- 5-flag safety defaults explicit
- Rationale: empirical evidence of 1-hop saturation, recommendation for 2-hop
- Alternatives: anchor choice, BFS depth
- Consequences: M058 needs 2-hop OR different anchor

Update doc/adr/ADR-INDEX.md to reference ADR-010.
gsd_decision_save emits D-number for ADR-010.

Also commit ADR-010 + REPORT.md + candidate-edges.json together.

## Inputs

- `artifacts/m056-bfs-graph/REPORT.md`
- `artifacts/m056-bfs-graph/candidate-edges.json`

## Expected Output

- `doc/adr/ADR-010-bfs-scale-167-pdf.md`
- `doc/adr/ADR-INDEX.md (updated)`

## Verification

test -f doc/adr/ADR-010-bfs-scale-167-pdf.md

## Observability Impact

ADR-010 + D-number emitted.
