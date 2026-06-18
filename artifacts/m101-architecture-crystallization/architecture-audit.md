# Architecture Audit and Gap Analysis (M101 S01)

## Executive Summary

daily-archive has completed a full package migration (M099: arxiv_archive → research_graph, 110 modules) and architecture cleanup (M100: summarizer→llm, evidence naming, CLI decomposition). The project now has a clean modular foundation with 12 packages, 835+ passing tests, and 22 binding ADRs.

The next phase requires bridging three critical gaps identified by comparing against Agents-K1, quant-mind, and ActiveGraph patterns:

1. **Typed Knowledge Schema** — current flat entities/relations need upgrading to Agents-K1 modules A-E
2. **Smart Extraction Pipeline** — fixture-only contracts need upgrading to Core-then-Modes with MiniMax+GLM multi-provider
3. **Graph Migration** — LadybugDB (ADR-020, superseded) → FalkorDB (ADR-022, binding)

## 1. Current Architecture State

### 1.1 Package Structure (post-M099/M100)

```
research_graph/
├── corpus/sources/       arxiv_client, semantic_scholar, markdown_converter
├── corpus/ingestion/     loader, fetchers
├── corpus/parsing/       parser, normalization, structure
├── papers/artifacts/     models, metrics, batch_validation, worker, minimax_boundary
├── papers/indexing/      page_index, links_dedup, retrieval_tables, navigation
├── papers/chunking/      chunker, figure/table units
├── papers/source_assets/ registry, provenance
├── evaluation/           metrics, dspy_extraction, extraction_benchmark, scoring, analytics
├── retrieval/            embedder, hybrid, keyword_extractor
├── graph/                ladybug_client, readiness/ (7 modules)
├── llm/                  provider_config, models_registry, minimax_structured, minimax_usage, summarizer
├── workflows/            validation/, universal_kb/, rlm/, review_packet
├── repair/               chunk_repair, baseline_measurement, benchmark
├── identity/             canonicalization, dedup
├── staging/              graph_candidates, import_boundary
├── quality/              baselines, thresholds, maintainability
├── ops/notifications/    telegram_sender
└── cli/                  __init__ (analysis), commands/ (article_artifacts, validation, quality)
```

### 1.2 Binding Decisions (from ADRs)

| Layer | Binding ADR | Decision | Status |
|---|---|---|---|
| Domain | ADR-001 | Scientific papers as first domain | ✅ Active |
| Parser | ADR-008/009 | Hybrid: Marker + GROBID + arxiv2md | ✅ Active |
| Graph library | ADR-016 | NetworkX primary, igraph supplementary | ✅ Active |
| **Graph DB** | **ADR-022** | **FalkorDB (self-hosted)** | ✅ **Current binding** |
| Graph DB (old) | ADR-020 | ~~LadybugDB~~ | ❌ Superseded by ADR-022 |
| Graph DB (old) | ADR-021 | ~~Neo4j~~ | ❌ Superseded by ADR-022 |
| LLM Judge | ADR-014 | MiniMax-M3 multimodal figure QA | ✅ Active |
| Embeddings | ADR-019 | fd service (BGE-M3, 1024d, local TEI) | ✅ Active |
| Pipeline | ADR-017 | Queue deferred until pipeline complete | ✅ Active |

### 1.3 LLM Provider Posture

| Provider | Model | Context | Endpoint | Role | Source |
|---|---|---|---|---|---|
| MiniMax | M3-512k | 512K | api.minimax.io/anthropic | Primary extraction + multimodal judge | ADR-014, models.yaml |
| MiniMax | M2.7-highspeed | — | api.minimax.io/anthropic | Fast classification | models.yaml |
| GLM/Z.ai | GLM-5.2 | — | api.z.ai/api/anthropic | Secondary/fallback | M076-M078 |
| GLM/Z.ai | GLM-4.5-Air | — | api.z.ai/api/anthropic | Small/fast tasks | M078 |
| (future) | Any | — | Configurable | Hot-pluggable | provider_config.py |

**Architecture**: `provider_config.py` provides provider-neutral config with namespaced `GLM_*`/`MINIMAX_*` env keys, `to_anthropic_runtime_env()`, and no `os.environ` mutation.

**Compression modes**: `none` | `provider_native` | `headroom_candidate`

**Headroom** (`https://github.com/chopratejas/headroom`): registered as candidate, not adopted. Evaluation criteria defined in adr-inventory.md.

## 2. Gap Analysis

### 2.1 vs Agents-K1

| Agents-K1 Feature | daily-archive Current | Gap | Priority |
|---|---|---|---|
| **Typed entity schema** (Module A-E) | Flat ScientificEntity with 5 relation types | Need ~25 typed relations in 5 groups | **HIGH** |
| **Core-then-Modes extraction** | Fixture-only contracts | Need MiniMax+DSPy extraction pipeline | **HIGH** |
| **Stable IDs for cross-view joins** | `identity/canonicalization.py` exists | Need typed entity IDs, not just paper IDs | **MEDIUM** |
| **GRPO-trained 4B extractor** | N/A (no GPU) | Adapt: DSPy BootstrapFewShot with MiniMax API | **HIGH** |
| **Tri-source retrieval** (web+mmkg+kn) | Hybrid vector+graph only | Need knowledge network traversal + web search | **MEDIUM** |
| **Graph operators O1-O6** | BFS 2-hop (M064), seed resolution (identity) | Need O3-O6: comparative, multimodal, gap, novelty | **MEDIUM** |
| **Multi-agent CLI** (6 roles) | RLM workflow prototype | Need agent integration AFTER tools are ready | **LOW** (deferred) |
| **Scholar-KG scale** (2.46M papers) | 220 PDFs canonical catalog | Staged validation first (R024: 10→20→week) | **MEDIUM** |

### 2.2 vs Quant-Mind Patterns (M033 research)

| Quant-Mind Pattern | daily-archive Status | Adaptation |
|---|---|---|
| TreeKnowledge structure | Partial: PageIndex hierarchy | Extend to typed knowledge tree in FalkorDB |
| PaperKnowledgeCard | Partial: ScientificEntity dataclass | Upgrade to typed entity card with Module A-E fields |
| Provenance schemas | ✅ Strong: EvidencePath, SourceSpan, hashes | Already exceeds quant-mind |
| Fetch/format separation | ✅ Already separated: corpus/ingestion vs evaluation | No change needed |
| Bounded batch flows | ✅ Core-then-Modes concept compatible | Adapt for MiniMax API limits |
| Magic input resolution | ✅ identity/canonicalization.py | Extend to typed entity resolution |
| Realized vs aspirational separation | ✅ Fail-closed boundaries | Core principle, maintained |

### 2.3 vs ActiveGraph Patterns (M048 research)

| ActiveGraph Pattern | daily-archive Status | Adaptation |
|---|---|---|
| Event-sourced reactive graph | Not implemented | Consider for agent coordination log |
| Behaviors (event handlers) | Not implemented | Map to extraction pipeline stages |
| Replay (deterministic rebuild) | Partial: provenance hashes | Extend to full event replay |
| Fork/diff (scenario testing) | Not implemented | Useful for extraction quality comparison |
| Single in-process FIFO queue | Not implemented (ADR-017: deferred) | Activate when pipeline is end-to-end |
| Reactive graph projection | NetworkX intermediate → FalkorDB | Migration needed |

### 2.4 Graph DB Migration: LadybugDB → FalkorDB

**Current state**: `graph/ladybug_client.py` uses LadybugDB for graph operations. NetworkX (ADR-016) is the intermediate representation.

**Target**: FalkorDB (ADR-022, binding) as production GraphDB.

**Migration path**:
1. Keep NetworkX as in-process intermediate (ADR-016, unchanged)
2. Add FalkorDB client alongside LadybugDB (not replace immediately)
3. Migrate graph-readiness pipeline to export NetworkX → FalkorDB
4. Add typed edges and vector indexes in FalkorDB schema
5. Deprecate LadybugDB client after FalkorDB acceptance tests pass

**Risk**: Current `graph/readiness/persistence.py` is coupled to LadybugDB. Needs careful decoupling.

### 2.5 Universal Domain Ingestion Gap

**Current**: Only arXiv papers (PDF + metadata) are ingested.

**Target**: Support textbooks (HTML), code repositories (Git clone), datasets (metadata), technical docs.

**Reference**: quant-mind fetch/format separation + GNN textbook (https://anvithpothula.github.io/graph-neural-networks-textbook/) as concrete example.

**Design question**: How to adapt the paper-focused extraction pipeline for non-paper domains without breaking existing contracts?

## 3. Layer-by-Layer Upgrade Recommendations

### Layer 1: Parser → Universal Parser
- **Keep**: Marker + GROBID + arxiv2md for PDF
- **Add**: HTML parser for textbooks (BeautifulSoup/readability)
- **Add**: Code repo ingestion (Git clone → file tree → AST)
- **Keep**: arxiv2md for REST conversion
- **ADR needed**: Universal parser architecture extending ADR-008

### Layer 2: Extraction → Smart Pipeline
- **Add**: Core-then-Modes factorization
- **Add**: DSPy BootstrapFewShot for prompt optimization
- **Add**: Typed entity extraction (Module B: Method, Dataset, Metric, Task)
- **Add**: Abstract entity extraction (Module C: Motivation, Gap, Hypothesis, Finding)
- **Add**: Typed relation extraction (Module E: 25 relation types)
- **Add**: Headroom evaluation (research, then conditional adoption)
- **ADR needed**: Extraction pipeline architecture

### Layer 3: Graph → FalkorDB Typed Schema
- **Migrate**: LadybugDB → FalkorDB (ADR-022 binding)
- **Add**: Typed edges for all 25 relation types
- **Add**: Vector indexes (BGE-M3 1024d)
- **Add**: Graph layers: source registry, entity, abstract, relationship, evidence
- **Add**: Graph operators O1-O6
- **ADR needed**: FalkorDB schema design

### Layer 4: LLM → Multi-Provider with Optimization
- **Keep**: MiniMax-M3 primary, GLM-5.2 secondary
- **Add**: DSPy integration for extraction prompt optimization
- **Add**: Provider routing (cost-aware, latency-aware)
- **Evaluate**: Headroom for token compression
- **Keep**: provider_config.py as hot-pluggable config layer

### Layer 5: Agents → ActiveGraph-Inspired (DEFERRED)
- **Prerequisite**: Pipeline + queues + graph must be operational first
- **Design**: 6 agent roles from Agents-K1 + ActiveGraph patterns
- **Safety**: All agent actions through fail-closed gates
- **ADR needed**: Agent integration plan (but execution deferred)

### Layer 6: Sources → Universal Domain
- **Add**: Domain profile concept (paper, textbook, code_repo, dataset, tech_doc)
- **Add**: Per-domain extraction profiles
- **Reference**: GNN textbook as first non-paper test case
- **ADR needed**: Universal domain ingestion architecture

## 4. Sequencing Strategy

```
Phase 1 (M101): Crystallize architecture ← THIS MILESTONE
    S01: Audit (this document)
    S02: Typed schema design
    S03: Extraction pipeline design
    S04: FalkorDB schema design
    S05: Agent plan (design only)
    S06: Universal ingestion design

Phase 2: Implement typed schema + extraction
    - Typed entities + relations in code
    - DSPy signatures + labeled fixtures
    - Core extraction prototype on 5 papers

Phase 3: FalkorDB migration
    - FalkorDB client
    - NetworkX → FalkorDB export
    - Typed edge schema
    - Graph operators O1-O6

Phase 4: Staged validation (R024)
    - 10 documents → graph quality analysis
    - 20 documents → comparison
    - Weekly corpus → scale test

Phase 5: Universal ingestion
    - Textbook parser
    - Code repo ingestion
    - Cross-source linking

Phase 6: Agent integration (LATEST)
    - Multi-agent swarm
    - MCP tools
    - ActiveGraph patterns
```

## 5. Constraints and Safety Boundaries

All architecture designs in M101 must respect:

1. **FalkorDB is binding** (ADR-022) — no designs for Neo4j or LadybugDB as production target
2. **NetworkX is intermediate** (ADR-016) — all graph code must work with NetworkX first
3. **MiniMax+GLM multi-provider** — no single-provider lock-in
4. **No GPU training** — extraction via API + DSPy, not GRPO
5. **Fail-closed boundaries** — no graph writes without explicit authorization
6. **Staged validation** (R024) — no scale claims before 10/20/week validation
7. **M034 ADR template** — all new ADRs use canonical template
8. **Headroom is candidate** — research before adoption, not blind dependency

## 6. Open Questions for Subsequent Slices

| Question | Slice | Dependencies |
|---|---|---|
| How many typed relations to implement first? | S02 | Gap analysis (this doc) |
| What DSPy signatures for Core extraction? | S03 | Typed schema (S02) |
| How to store typed edges in FalkorDB? | S04 | Typed schema (S02) |
| How do agents interact with graph operators? | S05 | Graph operators (S04) |
| How to parse HTML textbooks? | S06 | Universal schema (S02) |
| When to activate pipeline queue (ADR-017)? | Phase 2+ | Pipeline end-to-end |
