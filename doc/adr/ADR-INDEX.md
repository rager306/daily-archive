# ADR Index

This index lists project-level ADRs and points to historical ADR packages kept under milestone-specific subdirectories.

## Status Vocabulary

- `Proposed` — drafted but not accepted.
- `Accepted` — binding unless superseded.
- `Accepted (binding)` — binding and explicitly not revisable without a later accepted ADR.
- `Deferred` — intentionally postponed.
- `Rejected` — option rejected for current scope.
- `Superseded` — replaced by a later ADR or GSD decision.

## Project-Level ADRs

| ADR | Status | Title | Path | Notes |
| --- | --- | --- | --- | --- |
| ADR-001 | Accepted | Scientific Papers as First Domain | `doc/adr/ADR-001-scientific-papers-as-first-domain.md` | Establishes scientific papers as the first proving domain. |
| ADR-008 | Accepted (binding) | Hybrid Parser Architecture | `doc/adr/ADR-008-hybrid-parser-architecture.md` | Binds GROBID header/citations + OpenDataLoader body/layout after M055 100% hybrid benchmark recommendation. |
| ADR-009 | Accepted (binding) | Fulltext-Aware Hybrid Parser Routing | `doc/adr/ADR-009-amend-hybrid-parser.md` | Amends ADR-008 after M055deep 20-PDF evidence: default hybrid routing with GROBID fulltext fallback when OpenDataLoader body evidence is low-quality or unavailable. |
| ADR-010 | Accepted (binding) | BFS Scale Evidence from 167-PDF 1-hop Run | `doc/adr/ADR-010-bfs-scale-167-pdf.md` | Supplements ADR-009 after M056 149-PDF evidence: 1-hop BFS from 2605.18747 saturated at 7-8 target-set edges, so M058 should require 2-hop BFS or a deliberate alternative-anchor strategy. |

## Historical ADR Packages

| Package | Path | Notes |
| --- | --- | --- |
| M034 Universal KB ADR Package | `doc/adr/m034/ADR-INDEX.md` | Contains ADR-000 and ADR-002 through ADR-007 for universal KB governance and sidecar boundaries. |

## Safety Notes

ADR-008 does not authorize graph import, production import, LadybugDB writes, or fact promotion.
Graph import is not authorized by the M055 benchmark report or ADR-008.
Production import is not authorized by the M055 benchmark report or ADR-008.
LadybugDB writes are not authorized by the M055 benchmark report or ADR-008.
