---
name: daily-archive
last_updated: 2026-08-26
---

# daily-archive Strategy

## Purpose

Researchers and agents face more scientific literature than they can read, yet most tools only surface chunks or summaries that cannot be audited. The hard part is not storage — it is knowing what a paper actually claims, where each claim came from, and whether an answer is grounded in evidence rather than model invention.

## Positioning

Win by making a **local-first, fail-closed knowledge graph** the sole authority for scientific memory: governed ontology, bi-temporal edges, candidate≠authoritative promotion, and import locked until explicit human go — so that retrieval and agents operate on traceable graph truth instead of RAG-over-PDF chat.

## Users

**Primary:** Research engineer / knowledge curator — they're hiring daily-archive to turn a paper corpus into durable, queryable scientific memory that supports grounded agent retrieval without inventing facts.

## Boundaries

- No opening graph import without explicit human approval (D127; `import_eligible=false` stays fail-closed).
- No second graph store or external DB for authoritative state — Samyama is the sole KG+vector+persist engine (ADR-040).
- No fake deferred-write seams (GraphWrite stays deferred-by-design until an honest producer exists; ADR-063).
- No auto-promote of GEPA/LLM extraction without dual F1, same-n, and val_gap evidence.
- No treating sidecar TEI/markdown or LLM output as graph truth without candidate→verify→promote.
- DSPy enablement remains gated by verified eval metrics and benchmark fixtures. SONA, MicroLoRA, EWC++ and ruVLLM training are active under D294; candidates never auto-promote and require frozen evaluation plus explicit acceptance.

_Resist a change when:_ it trades auditability or fail-closed graph semantics for faster demos, chat UX, or "good enough" retrieval without graph-task or gold-corpus evidence.

## Key metrics

- **Extraction micro-F1 (gold corpus)** — entity extraction precision/recall on `data/gold_standard/` via `eval_batch`; target: sustain recall≈1.0 while improving precision.
- **Graph task suite pass rate** — semantic KG readiness beyond extraction F1 (ADR-051 §8.1); measured by `graph_task_suite` over pinned `.sgsnap` + Cypher tasks.
- **Catalog materialization coverage** — share of catalog papers completing the 14-stage pipeline to a durable snapshot; tracked in materialization run reports under `artifacts/catalog_sweep/`.
- **Governor architecture gate** — hexagonal invariants, ADR compliance, and trajectory verdict; `PYTHONPATH=scripts python3 -m governor architecture`.
- **Agent retrieval EMPTY/ERROR rate** — graph-task and agent FSM paths must not silently return ungrounded answers; bounded by semantic graph tasks and negative-knowledge admission gates.

## Tracks

### Parse & materialization plane

Rust-native parse→TEI→revision binding and corpus-wide cache materialization (GROBID-compatible parse plane, ADR-065/075 direction).

_Why it serves the approach:_ graph truth is only as good as the structural parse and revision identity feeding extraction.

### Assurance & agent readiness

ACAP context layers, FSM admission before LLM stages, negative semantic knowledge, ContextBundle freezing, read-only identity graph plane (ADR-055, ADR-061).

_Why it serves the approach:_ agents must read governed graph projections, not unvetted sidecar text or unconstrained model output.

### Universal kg-* platform & data-driven ontology

YAML ontology registry, pipeline stage contracts, hexagonal `da-*` / `kg-*` split, Governor-enforced architecture (ADR-050, ADR-068).

_Why it serves the approach:_ schema and pipeline rules stay data, testable, and portable — not hardcoded product logic.

### Research process & multi-domain ontology

Execution-grounded scientific memory beyond publication plane — hypotheses, environments, interventions, domain profiles (ADR-043, ADR-069–071).

_Why it serves the approach:_ supports analysis and synthesis across domains, not just document storage and entity extraction.

## Brand

**One-liner:** Local-first scientific knowledge engine — ingest PDFs, extract structure, embed, and query a durable knowledge graph with fail-closed evidence gates.

**Key message:** Graph import stays locked until you say go. Sidecar parsers and LLMs produce candidates; Samyama holds what you can defend.
