# ADR Inventory for Architecture Crystallization (M101)

Generated: 2026-06-18

## Binding ADRs

| ADR | Title | Status | Binding | Scope |
|---|---|---|---|---|
| ADR-001 | Scientific Papers as First Domain | Accepted | yes | domain choice |
| ADR-008 | Hybrid Parser Architecture | Accepted | yes | parser / marker / grobid routing |
| ADR-009 | Fulltext-Aware Hybrid Parser Routing | Accepted | yes | parser routing amendment |
| ADR-010 | BFS Scale Evidence from 167-PDF 1-hop | Accepted | yes | acquisition / bfs / evidence |
| ADR-011 | Content Graph via fd for M057 | Accepted | yes | embedding / graph-readiness |
| ADR-012 | Figure Caption v2 via TeX Provenance | Accepted | yes | figures / multimodal |
| ADR-013 | Manifest-Driven PDF Ingest Architecture | Accepted | yes | ingest / manifest / pipeline |
| ADR-014 | MiniMax M3 Multimodal as Figure QA Judge | Accepted | yes | llm / minimax / judge |
| ADR-016 | Graph Library Selection (NetworkX + igraph) | Accepted | yes | graph library / networkx / igraph |
| ADR-017 | Pipeline Queue Deferred | Accepted | yes | pipeline / queue / deferred |
| ADR-018 | M061 2-hop Evidence and M064 Trigger | Accepted | yes | bfs / 2-hop / evidence |
| ADR-019 | fd Embedding Service Contract | Accepted | yes | embedding / fd / tei |
| ADR-020 | GraphDB Selection (LadybugDB) | **Superseded** by ADR-022 | was binding | graphdb / ladybugdb |
| ADR-021 | GraphDB Re-Selection (Neo4j) | **Superseded** by ADR-022 | was binding | graphdb / neo4j |
| ADR-022 | GraphDB Re-Selection Self-Hosted (**FalkorDB**) | **Accepted (current)** | **yes** | graphdb / falkordb / self-hosted |

## ADR Supersession Chain

```
ADR-020 (LadybugDB, 39/45) → ADR-021 (Neo4j, 76/90) → ADR-022 (FalkorDB, 70/90)
```

**Current binding**: ADR-022 selects FalkorDB for self-hosted production GraphDB.

**Important**: ADR-020 and ADR-021 are superseded but their benchmark evidence is preserved. The LadybugDB → FalkorDB migration is a key gap.

## Research Milestones

| Milestone | Topic | Key Finding |
|---|---|---|
| M033 | External Parser + Paper Knowledge Architecture | quant-mind patterns: TreeKnowledge, PaperKnowledgeCard, fetch/format separation |
| M048 | ActiveGraph + SkillGenome Patterns | ActiveGraph: event-sourced reactive graph, behaviors, replay, fork/diff; patterns adopted, runtime not adopted |
| M061 | Graph Library Alternatives | NetworkX primary, igraph supplementary, rustworkx when available; ADR-016 binding |
| M067 | FalkorDB Self-Hosted Selection | FalkorDB 70/90, SSPLv1 acceptable for self-hosted; ADR-022 binding |
| M069 | Agents-K1 Schema Diff | 3 gap areas: implicit abstractions, typed relations, evaluation evidence; recommended safe next step: stable IDs + minimal FalkorDB schema + benchmarks |
| M076 | GLM Z.ai Helper Skill | GLM-5.2 + GLM-4.5-Air as secondary provider; compression modes including headroom_candidate |
| M078 | LLM Provider Config | Provider-neutral config: namespaced GLM_*/MINIMAX_* env keys, to_anthropic_runtime_env(), no os.environ mutation |
| M099 | Full Package Migration | arxiv_archive → research_graph (110 modules, 20 waves) complete |

## Current Architecture Layers

### Layer 1: Parser
- **Current**: Marker (primary), GROBID (deep scholarly), arxiv2md (REST fallback), PyMuPDF (last resort)
- **ADR binding**: ADR-008 (hybrid parser), ADR-009 (fulltext-aware routing)
- **Gap vs Agents-K1**: Agents-K1 uses MinerU only; our multi-parser approach is richer but more complex
- **Gap vs universal**: No support for non-PDF sources (HTML textbooks, code repos)

### Layer 2: Extraction
- **Current**: Fixture-only contracts (ScientificEntity, Claim, ScientificRelation with 5 relation types)
- **Gap vs Agents-K1**: No typed entity extraction, no Core-then-Modes, no GRPO/DSPy optimization
- **Gap vs universal**: No domain-specific extraction profiles

### Layer 3: Graph
- **Current**: NetworkX intermediate (ADR-016), LadybugDB in current code (ADR-020 superseded), FalkorDB target (ADR-022)
- **Gap**: No typed edges, no 5-module schema (A-E from Agents-K1), no graph operators (O1-O6)
- **Migration needed**: LadybugDB → FalkorDB

### Layer 4: LLM
- **Current**: MiniMax-M3-512k (primary, ADR-014), GLM-5.2 (secondary, M076-M078)
- **Config**: provider_config.py with namespaced env keys, compression modes (none/provider_native/headroom_candidate)
- **Gap**: No DSPy integration for extraction, Headroom not evaluated, no Core-then-Modes pipeline

### Layer 5: Agents
- **Current**: RLM workflow prototype (read-only, bounded traversal)
- **M048 ActiveGraph patterns**: event-sourced reactive graph, behaviors, replay — patterns adopted, runtime not adopted
- **Gap**: No multi-agent swarm, no graph operators as agent tools, no MCP tool definitions

### Layer 6: Sources
- **Current**: Papers only (arXiv catalog, 220+ PDFs)
- **Gap**: No textbook ingestion, no code repo ingestion, no dataset metadata ingestion
- **Quant-mind reference**: TreeKnowledge, PaperKnowledgeCard, fetch/format separation

## Headroom Evaluation Criteria

Headroom (`https://github.com/chopratejas/headroom`) is registered as `COMPRESSION_HEADROOM_CANDIDATE` in provider_config.py.

Evaluation must verify before adoption:
1. **Maintenance state**: Is the repo actively maintained?
2. **Dependency footprint**: What does it add to the install?
3. **License**: Compatible with self-hosted daily-archive?
4. **API compatibility**: Works with MiniMax-M3 and GLM-5.2?
5. **Provenance preservation**: Does token compression lose evidence/spans?
6. **Quality impact**: Measurable F1 delta on extraction benchmarks?
7. **Cost savings**: Quantified token reduction vs quality tradeoff?

## LLM Multi-Provider Posture

| Provider | Models | Endpoint | Role | Config |
|---|---|---|---|---|
| MiniMax | M3-512k, M3, M2.7-highspeed | api.minimax.io/anthropic | Primary extraction + judge | MINIMAX_* env keys |
| GLM/Z.ai | GLM-5.2, GLM-4.5-Air | api.z.ai/api/anthropic | Secondary/fallback | GLM_* env keys |
| (future) | Any Anthropic-compatible | Configurable | Hot-pluggable | provider_config.py |

**Architecture principle**: LLM module must accept new providers/models without code changes — only config additions.

## Key Decisions Referenced

| Decision | ADR | Implication for M101 |
|---|---|---|
| FalkorDB is production GraphDB | ADR-022 | All schema designs must fit FalkorDB typed edges |
| NetworkX is intermediate layer | ADR-016 | Migration path: NetworkX → FalkorDB export |
| MiniMax-M3 is figure QA judge | ADR-014 | Extraction pipeline can reuse MiniMax API path |
| LadybugDB was superseded | ADR-020 ← ADR-022 | Current graph-readiness code uses LadybugDB; needs migration |
| Pipeline queue deferred | ADR-017 | Queue infrastructure comes after pipeline is end-to-end |
| fd embedding service | ADR-019 | BGE-M3 1024d embeddings via local TEI endpoint |
