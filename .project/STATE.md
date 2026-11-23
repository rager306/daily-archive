---
pipeline: gsd-path/v2
project: daily-archive
milestone: orientation
phase: ship
                    # inspect only for brownfield; greenfield starts at define
                    # roadmap only in program flow (CHARTER.md exists)
status: active
branch: gsd-path/M001
                    # the router rebinds before any next-milestone file change
                    # the bound branch is never main; ship integrates it there
archive: null       # persisted archive transaction path; never recomputed
integration_default: direct # direct | pull-request | external-landing; project setting
integration: direct # current milestone; may override the default before build
integration_source: default # default | milestone; preserves override provenance
default_branch: master # origin default branch; initialization records it once, nothing changes it
---

# Project State

One file, always current. The router reads this first; every phase updates
it on completion. If this file and the artifacts disagree, the artifacts win
— fix this file.

## Log

<!-- append one line per transition: date, phase, event -->
- 2026-11-22 — inspect — project initialized
- 2026-11-22 — inspect — router bound initial milestone
- 2026-11-22 — inspect — inspection artifacts passed
- 2026-11-22 — define — definition started
- 2026-11-22 — define — milestone intent approved
- 2026-11-22 — research — research started
- 2026-11-22 — research — research evidence complete: manifest .project/research/RESEARCH.md; dispatched pitfalls, retirement-gates; skipped domain, stack, similar; 2 questions answered
- 2026-11-22 — decide — decision synthesis started
- 2026-11-22 — decide — decision synthesis validated: .project/research/SYNTHESIS.md; rulings U1 (CI untouched until stable functionality), U2 (adr.md mirror accept-drift)
- 2026-11-22 — plan — planning started
- 2026-11-22 — plan — plan approved
- 2026-11-22 — build — build started
- 2026-11-22 — ship — build done; final review pending
- 2026-11-22 — ship — final gap 2 blocked: AGENTS.md 31-vs-43 node schemas contradiction; patch findings recorded
- 2026-11-22 — plan — patch plan reopened
- 2026-11-23 — plan — patch plan approved
- 2026-11-23 — build — build started
- 2026-11-23 — ship — build done; final review pending
