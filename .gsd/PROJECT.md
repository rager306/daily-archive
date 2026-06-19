# Project: daily-archive

## What This Is

daily-archive is a **local-first Universal Knowledge Base** with a **7-layer typed knowledge pipeline** (ADR-023): Source → Parser → Structure → Extraction → Graph → Review → Agents. Scientific papers are the primary domain; textbooks, code repositories, and datasets are planned through universal domain ingestion (ADR-032).

The project builds deterministic evidence chains before any graph import: catalog records, source acquisition, parser diagnostics, typed entity extraction, fail-closed review gates, and FalkorDB typed graph schema.

## Core Value

A future agent should be able to ingest knowledge sources locally, extract typed entities and relations, build a navigable knowledge graph in FalkorDB, and use FSM-controlled reasoning (SymFSM) to answer complex research questions — all with traceable evidence and fail-closed safety boundaries.

## Architecture (post-M101)

**7-Layer Pipeline** (ADR-023):

| Layer | Name | What | Key ADR |
|---|---|---|---|
| 0 | Source | Register and fetch any knowledge source | ADR-032 (universal) |
| 1 | Parser | Convert raw source → ParsedArticle | ADR-008/009 |
| 2 | Structure | TreeKnowledge + SemanticChunks + KnowledgeCards | ADR-024 |
| 3 | Extraction | Core-then-Modes typed extraction | ADR-028/029 |
| 4 | Graph | FalkorDB typed schema + operators O1-O6 | ADR-022/030 |
| 5 | Review | Fail-closed safety gates | existing |
| 6 | Agents | SymFSM-controlled (REQUIRES DEVELOPMENT) | ADR-026/031 |

**Key Principles**:
- **Statistical-first** (ADR-024): YAKE/TF-IDF/embeddings before every LLM call
- **Multi-provider LLM** (ADR-025): MiniMax-M3 + GLM-5.2 with per-provider rate limits
- **Resource-aware scheduler** (ADR-027): 3-lane (LLM/CPU/IO) queue coordination
- **Typed schema** (ADR-028): 27 relation types in 5 groups (Agents-K1 adapted)
- **FalkorDB** (ADR-022): production GraphDB, NetworkX intermediate (ADR-016)
- **Fail-closed boundaries**: no graph writes without explicit authorization

## Current State

- **Package**: `research_graph/` — 110 modules in 12 packages (M099-M100 complete)
- **Architecture**: crystallized via 32 binding ADRs (M101 complete)
- **Graph DB**: NetworkX intermediate (ADR-016), FalkorDB target (ADR-022), LadybugDB being retired
- **LLM**: MiniMax-M3-512k + GLM-5.2 via provider_config.py (hot-pluggable, ADR-025)
- **Embeddings**: BGE-M3 1024d via local fd/TEI service (ADR-019)
- **Corpus**: 220+ PDFs in canonical arXiv catalog
- **Tests**: 835+ passing
- **Trajectory**: 13 monitoring dimensions (8 original + 5 post-M101 architecture)

## Architecture Decision Records

- 32 binding ADRs (ADR-001 through ADR-032)
- Canonical ADR template: `.gsd/milestones/M034-kuei9y/decision-package/ADR-TEMPLATE.md`
- ADR index: `doc/adr/ADR-INDEX.md`
- Key recent ADRs: ADR-023 (architecture vision), ADR-028 (typed schema), ADR-029 (extraction pipeline), ADR-030 (FalkorDB schema), ADR-031 (agents), ADR-032 (universal ingestion)

## Phased Roadmap

| Phase | Focus | Status |
|---|---|---|
| Phase 1 | Architecture crystallization (M101) | ✅ Complete |
| Phase 2 | Typed schema code + extraction prototype | ⬜ Next |
| Phase 3 | FalkorDB migration + graph operators | ⬜ |
| Phase 4 | Staged validation (R024: 10→20→week) | ⬜ |
| Phase 5 | Universal ingestion (textbook, code, dataset) | ⬜ |
| Phase 6 | Agent integration (SymFSM) | ⚠️ Requires idea development |

## Safety Boundaries

- No graph writes without explicit authorization
- No production imports without independent review
- Statistical-first: deterministic pre-processing before every LLM call
- Rate-limit-aware: per-provider quota checking before API calls
- Agent safety: SymFSM control prevents free-form LLM actions
- Staged validation: no scale claims before 10/20/week validation (R024)
