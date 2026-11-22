# Research Handoff

<!-- Written by the research orchestrator and consumed by decide. -->

Phase: research
Status: complete
Intent: `.project/intent/INTENT.md`

## Dispatch

- `domain` — skipped → none — no [RESEARCH] question assigned; domain rules and terminology are settled by `.project/research/evidence-codebase.md` and the IMPORT.md carry rows.
- `stack` — skipped → none — stack is fixed by the existing codebase (Rust workspace + Samyama/RuVector vendors, hard-locked by INTENT Constraints); no viable-stack choice is open.
- `pitfalls` — dispatched → `.project/research/evidence-pitfalls.md` — owns the CI-coverage reliability question (146 non-CI da-parse suites vs promotion milestones); brownfield rule requires checking which failure modes already exist in the mapped codebase.
- `similar` — skipped → none — comparable-product survey has no assigned question in this assessment milestone.
- `retirement-gates` — dispatched → `.project/research/evidence-retirement-gates.md` — custom dimension: owns the retirement-gate infrastructure-vs-execution question feeding the post-M001 roadmap.

## Question assignments

- `[RESEARCH] Which of the retirement gates (SC3d list) require new measurement infrastructure versus only execution of existing packages — feeds the roadmap after M001.` → `retirement-gates`
- `[RESEARCH] Whether the 146 non-CI da-parse suites should join CI before any promotion milestone — mapper finding, owner steers later.` → `pitfalls`
