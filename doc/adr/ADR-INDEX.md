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

Project-level ADR count: 29

| ADR | Status | Title | Path | Notes |
| --- | --- | --- | --- | --- |
| ADR-001 | Accepted | Scientific Papers as First Domain | `doc/adr/ADR-001-scientific-papers-as-first-domain.md` | Establishes scientific papers as the first proving domain. |
| ADR-008 | Accepted (binding) | Hybrid Parser Architecture | `doc/adr/ADR-008-hybrid-parser-architecture.md` | Binds GROBID header/citations + OpenDataLoader body/layout after M055 100% hybrid benchmark recommendation. |
| ADR-009 | Accepted (binding) | Fulltext-Aware Hybrid Parser Routing | `doc/adr/ADR-009-amend-hybrid-parser.md` | Amends ADR-008 after M055deep 20-PDF evidence: default hybrid routing with GROBID fulltext fallback when OpenDataLoader body evidence is low-quality or unavailable. |
| ADR-010 | Accepted (binding) | BFS Scale Evidence from 167-PDF 1-hop Run | `doc/adr/ADR-010-bfs-scale-167-pdf.md` | Supplements ADR-009 after M056 149-PDF evidence: 1-hop BFS from 2605.18747 saturated at 7-8 target-set edges, so M058 should require 2-hop BFS or a deliberate alternative-anchor strategy. |
| ADR-011 | Accepted (binding) | Content Graph via fd for M057 | `doc/adr/ADR-011-content-graph-via-fd.md` | M057 S02-S05: 4-layer diagnostic graph (citation + table + figure v1 + figure v2) using fd embeddings (deepvk/USER-bge-m3, 1024d). |
| ADR-012 | Accepted (binding) | Figure Caption v2 via TeX Provenance for M058 | `doc/adr/ADR-012-figure-caption-v2.md` | M058 S01: plotextractor from TeX source (100% TeX availability, 60% image paths, 99% labels) replaces regex caption extraction. |
| ADR-013 | Accepted (binding) | Manifest-Driven PDF Ingest Architecture | `doc/adr/ADR-013-manifest-driven-pdf-ingest.md` | M059: 6 JSON schemas + 5 retroactive manifests + jsonschema validation + replay tooling. Every future PDF batch has versioned, replayable, validatable processing contract. |
| ADR-014 | Accepted (binding) | MiniMax M3 Multimodal as Figure QA Judge | `doc/adr/ADR-014-minimax-judge-m3-multimodal.md` | M059b: 3x faster than M2.7-highspeed, better on 2/3 dimensions, 30/30 figures passed. Selected as production judge model. |
| ADR-015 | Accepted (binding) | NetworkX as Intermediate Graph Layer | `doc/adr/ADR-015-networkx-intermediate.md` | M060b: NetworkX is primary for read-only ops, manifest validation, simple algorithms. Use igraph only when scale demands. |
| ADR-016 | Accepted (binding) | Graph Library Selection for M060b-M064+ | `doc/adr/ADR-016-graph-library-selection.md` | Amended 2026-06-13 to drop rustworkx: NetworkX primary, igraph supplementary. |
| ADR-017 | Accepted (binding) | Pipeline Queue Deferred Until Pipeline End-to-End Complete | `doc/adr/ADR-017-pipeline-queue-deferred.md` | M064 (smart queue + per-article DAG + SQLAlchemy Core + SQLite) DEFERRED until M061+M062+M063 complete. Design preserved for future implementation. |
| ADR-018 | Accepted (binding) | M061 2-hop Evidence and M064 Trigger Evaluation | `doc/adr/ADR-018-m061-2-hop-evidence-and-m064-trigger.md` | M064 S03: confirms M061 2-hop evidence and keeps ADR-017 queue deferral in place until M062/M063 evidence changes the trigger. |
| ADR-019 | Accepted (binding) | M062 fd Embedding Service Contract | `doc/adr/ADR-019-fd-embedding-service-contract.md` | Formalizes `/root/fd-v2.md` as the binding fd v2 contract for M062 fd production hardening; Amendment Log entries: 2. |
| ADR-020 | Superseded by ADR-022 | M063 GraphDB Selection (LadybugDB primary) | `doc/adr/ADR-020-graphdb-selection.md` | Superseded by ADR-022 after M067 corrected the self-hosted license analysis. |
| ADR-021 | Superseded by ADR-022 | M066 GraphDB Re-Selection (Neo4j primary) | `doc/adr/ADR-021-graphdb-reselection.md` | Superseded by ADR-022 after M067 selected FalkorDB for self-hosted daily-archive. |
| ADR-022 | Accepted (binding) | M067 GraphDB Re-Selection Self-Hosted (FalkorDB primary) | `doc/adr/ADR-022-graphdb-reselection-self-hosted.md` | M067 GraphDB Re-Selection Self-Hosted (FalkorDB primary). |
| ADR-023 | Accepted (binding) | Architecture Vision and Layered Knowledge Pipeline | `doc/adr/ADR-023-architecture-vision-and-layered-pipeline.md` | M101: 7-layer pipeline, statistical-first, multi-provider LLM, SymFSM agents, quant-mind patterns. |
| ADR-024 | Accepted (binding) | Statistical-First Extraction Principle | `doc/adr/ADR-024-statistical-first-extraction-principle.md` | M101: YAKE/TF-IDF/embeddings before every LLM call. |
| ADR-025 | Accepted (binding) | Multi-Provider LLM Architecture with Per-Provider Rate Limits | `doc/adr/ADR-025-multi-provider-llm-rate-limits.md` | M101: MiniMax primary, GLM secondary, per-provider rate checking, Headroom candidate. |
| ADR-026 | Accepted (directional) | Agent Integration via SymFSM-Inspired Control Structures | `doc/adr/ADR-026-symfsm-inspired-agent-control.md` | M101: FSM-controlled reasoning, LLM as interpreter, deferred until pipeline+graph ready. |
| ADR-027 | Accepted (binding) | Resource-Aware Queue Scheduler | `doc/adr/ADR-027-resource-aware-queue-scheduler.md` | M101: 3-lane scheduler (LLM/CPU/IO), per-provider rate limits, backoff, observability. |
| ADR-028 | Accepted (binding) | Typed Knowledge Schema | `doc/adr/ADR-028-typed-knowledge-schema.md` | M101: 5 modules (A-E), 27 typed relations, stable IDs, domain profiles, quant-mind patterns. |
| ADR-029 | Accepted (binding) | Extraction Pipeline Architecture | `doc/adr/ADR-029-extraction-pipeline.md` | M101: Core-then-Modes, statistical-first, DSPy, multi-provider, ~$0.07/article. |

## Historical ADR Packages

| Package | Path | Notes |
| --- | --- | --- |
| M034 Universal KB ADR Package | `doc/adr/m034/ADR-INDEX.md` | Contains ADR-000 and ADR-002 through ADR-007 for universal KB governance and sidecar boundaries. |

## Safety Notes

ADR-008 does not authorize graph import, production import, LadybugDB writes, or fact promotion.
Graph import is not authorized by the M055 benchmark report or ADR-008.
Production import is not authorized by the M055 benchmark report or ADR-008.
LadybugDB writes are not authorized by the M055 benchmark report or ADR-008.
