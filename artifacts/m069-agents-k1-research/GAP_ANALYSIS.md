# M069 Agents-K1 Gap Analysis for daily-archive

**Source artifact:** `artifacts/m069-agents-k1-research/PAPER_SUMMARY.md`  
**Purpose:** identify what daily-archive missed, what must be researched before an M069 implementation milestone, and how to adapt Agents-K1 under local constraints.  
**Current constraint update:** no local GPU; M069 S03 must use DSPy + MiniMax API, not Qwen3-4B/GRPO local training.

---

## Executive finding

The saved Agents-K1 summary captures the high-level idea, but it is not yet actionable enough for daily-archive implementation. The largest missing pieces are:

1. A concrete schema diff from daily-archive's current 5-layer graph to Agents-K1's 5-module graph.
2. A benchmark/evaluation contract for extraction and research QA before any DSPy claims.
3. A practical MiniMax + DSPy optimization design that works without GPU.
4. A FalkorDB data-model and query-operator mapping for hyperedges, multi-view joins, and tri-source retrieval.
5. Source verification from the paper, GraphAnything repo, HF model/dataset, and Appendix details rather than the video summary alone.

---

## What daily-archive currently has vs what Agents-K1 implies

| Area | daily-archive current state | Agents-K1 implication | Gap |
|---|---|---|---|
| Graph storage | FalkorDB selected in ADR-022 | Neo4j property graph in paper | Need FalkorDB schema/query mapping, not Neo4j copy |
| Corpus | 220 canonical PDFs | Scholar-KG 2.46M papers | Need staged scale plan, not immediate corpus-scale claims |
| Existing graph | citation/table/figure v1/figure v2/judge layers | 5 modules A-E with richer abstractions | Need schema enrichment plan |
| Parser | plotextractor + hybrid GROBID/ODL | MinerU extracting text/figures/tables/equations/citations | Need parser capability comparison, especially tables/equations |
| Model path | MiniMax-M3 multimodal judge + text APIs | Qwen3-4B + GRPO extractor | Need DSPy + MiniMax adaptation; no Qwen3 local training |
| Retrieval | arxiv/web + graph traversal prototypes | S_web + S_mmkg + S_kn | Need multimodal graph retrieval path |
| Agents | no production multi-agent CLI yet | 6 roles + graph operators + MCP tools | Need operator/role mapping to M064 queue architecture |
| Evaluation | per-paper pipeline validation, no research QA benchmark | multi-hop QA, IE F1, research QA accuracy | Need local benchmark and metrics contract first |

---

## Main things we missed from `PAPER_SUMMARY.md`

### 1. Appendix D schema is more important than the high-level A-E table

The summary lists modules A-E but does not provide the disaggregated node/edge schema, properties, constraints, or provenance fields. For daily-archive, this is blocking because FalkorDB implementation needs concrete labels, relationship types, IDs, and indexes.

**Need to research:**
- Full node taxonomy: Paper, Author, Affiliation, Resource, Task, Method, Dataset, Metric, Baseline, Figure, Table, Equation, Definition, Theorem, Limitation, Hypothesis, etc.
- Full edge taxonomy and directionality.
- Required properties per node/edge.
- Provenance model: section, paragraph, page, bounding box, citation context, extractor version, confidence, source artifact ID.
- How hyperedges / n-ary relations are represented in their graph.

**daily-archive risk if skipped:** we will add names without semantics and recreate GraphRAG-style flat triples.

---

### 2. Section 4.2 multi-hop QA generation is a missing evaluation foundation

The summary notes LLM-guided multi-hop QA generation but does not explain the generation/filtering process. This matters because daily-archive currently has pipeline and extraction checks, but not a research-question benchmark.

**Need to research:**
- How questions are generated from graph paths.
- How distractors / negative examples are chosen.
- How answer containment and evidence paths are validated.
- Whether questions require graph-only reasoning, multimodal evidence, or web augmentation.
- How they avoid LLM-generated leakage or self-confirming labels.

**daily-archive adaptation:** build a small local benchmark over the 220 canonical PDFs before optimizing prompts or adding agents.

---

### 3. Section 7.2 metric definitions are needed before DSPy optimization

The saved artifact mentions Contain-Acc, GPT-Acc, IE F1, and research QA accuracy, but not exact metric implementations. DSPy optimization without a good metric will overfit the prompt to weak labels.

**Need to research:**
- Exact Contain-Acc calculation.
- Exact GPT-Acc judging prompt and rubric.
- Semantic-aware criteria for partial answers.
- IE F1 normalization for entities, relations, and structured NER.
- Relation scoring for n-ary / hyperedge claims.
- Whether they report confidence intervals or bootstrap variance.

**daily-archive adaptation:** define a local metric suite before M069 S03. This aligns with existing project memory: do not enable DSPy extraction before metrics and benchmark fixtures are verified.

---

### 4. Appendix B proofs are not just theory; they define engineering constraints

The summary lists P1-P3 but does not unpack the constructive requirements. These propositions translate into design rules: stable IDs, cross-view joins, and hyperedge preservation.

**Need to research:**
- What exactly counts as an identifier-preserving join.
- How they avoid false merges across aliases, paper versions, datasets, and metric acronyms.
- How union-view recall is computed.
- How binary projections lose hyperedge endpoints.
- How to model n-ary relationships in FalkorDB.

**daily-archive adaptation:** before FalkorDB writes are enabled, define stable ID rules for Paper, Section, Citation, Figure, Table, Equation, Dataset, Method, and extracted claim nodes.

---

### 5. GraphAnything repo/MCP tools are not analyzed yet

The artifact records the GitHub link and says there are operators/roles, but does not inspect real implementation. We do not know their actual tool contracts, payload shapes, storage assumptions, or failure modes.

**Need to research:**
- Real CLI commands and MCP server list.
- Tool schemas and input/output payloads.
- How graph operators O1-O6 are implemented.
- Whether operators depend on Neo4j-specific Cypher.
- Whether any reusable prompt/schema patterns exist.
- How Aggregator writes manifests and evidence IDs.

**daily-archive adaptation:** map O1-O6 onto M064 queue tasks and FalkorDB queries.

---

### 6. MiniMax + DSPy feasibility is not researched enough

The saved artifact correctly changes S03 away from Qwen3-4B/GRPO, but the practical DSPy design is still speculative.

**Need to research:**
- DSPy support for MiniMax via OpenAI-compatible or custom LM adapter.
- Structured JSON output constraints with MiniMax.
- Which DSPy optimizers are appropriate under API cost limits: BootstrapFewShot, MIPRO, BootstrapRandomSearch.
- How many training examples are needed from M061/M068 artifacts.
- How to cache calls and avoid repeated paid inference.
- How to measure prompt/program improvements without leaking labels into examples.
- What failure/observability surfaces are needed: token usage, latency, JSON parse failure rate, schema invalid rate, cost per paper.

**Suggested M069 S03 scope:** a no-production-write spike that optimizes one extraction signature over a 30-paper train / 10-paper validation set and reports cost/latency/quality, not a production extractor.

---

### 7. Multimodal graph retrieval (`S_mmkg`) is underspecified

daily-archive has figure/table/judge layers, but not an actual multimodal retrieval surface equivalent to S_mmkg.

**Need to research:**
- What is embedded: captions, cropped figures, serialized tables, equations, OCR text, or extracted claims.
- Whether embeddings are shared or modality-specific.
- How dense and lexical scores are fused.
- How figure/table/equation evidence is returned with provenance.
- Whether current fd v2 can embed all text representations reliably.

**daily-archive adaptation:** start with text representations of multimodal anchors (captions/table serializations/equation LaTeX) before image embeddings.

---

### 8. Citation-context classification is not yet mapped to existing citation edges

M061 produced many citation edges, but Agents-K1 wants argumentative relations and evidence context.

**Need to research:**
- Their citation classification schema from Appendix A.
- How they distinguish support, contrast, extend, background.
- How direct/indirect and strong/weak citation types are assigned.
- How section/paragraph evidence is represented.

**daily-archive adaptation:** add citation context as optional enrichment, not as a rewrite of existing citation edge ingestion.

---

### 9. Licensing and artifact access are still unverified

The artifact lists GitHub/HF/SCP links, but we have not audited license compatibility, model use terms, dataset access terms, or whether the 1M Scholar-KG release is actually downloadable and structurally useful.

**Need to research:**
- GraphAnything license.
- Agents-K1 model license and API/redistribution terms.
- Scholar-KG dataset license and schema files.
- MinerU license and whether it adds value over current parser stack.
- SCP portal access requirements.

**daily-archive risk if skipped:** repeating the M063/M066 graph DB decision failure mode where technical fit was evaluated before license fit.

---

## Recommended M069 research slices

### S01 — Source verification and schema diff

Read paper sections 4, Appendix A, B, D; inspect GraphAnything/HF artifacts; produce a concrete daily-archive schema diff.

**Outputs:**
- `source-verification.md`
- `schema-diff.md`
- `license-access-audit.md`

### S02 — Evaluation and benchmark contract

Define local benchmark before optimization: extraction F1, relation F1, evidence-path validity, research QA accuracy, JSON/schema validity, cost/latency.

**Outputs:**
- `benchmark-contract.md`
- `gold-set-design.md`
- `metrics-rubric.md`

### S03 — DSPy + MiniMax feasibility spike

Implement no-production-write prototype over MiniMax API with DSPy optimizers. Use only small reviewed train/validation sets; no Qwen3/GRPO.

**Outputs:**
- `dspy-minimax-feasibility.md`
- `cost-latency-quality-report.md`
- `prompt-program-candidates/`

### S04 — FalkorDB operator mapping

Map O1-O6 and tri-source retrieval to FalkorDB plus current artifacts. Include hyperedge/n-ary modeling options.

**Outputs:**
- `falkordb-operator-map.md`
- `hyperedge-model-options.md`
- `tri-source-retrieval-design.md`

### S05 — Roadmap decision package

Decide what becomes production work and what remains research-only. Keep graph writes, fact promotion, and production import disabled unless explicitly authorized later.

**Outputs:**
- ADR draft or decision note
- `M069-SUMMARY.md`
- next milestone options

---

## Guardrails for M069

- Do not claim production KG quality from fixture or LLM-only tests.
- Do not enable production graph writes.
- Do not use local Qwen3-4B/GRPO training path.
- Do not treat MiniMax multimodal diagnostics as authorization for production extraction.
- Do not optimize DSPy before benchmark fixtures and metrics exist.
- Keep all external API keys in environment only; never write secrets into artifacts.
- Prefer 127.0.0.1 in generated docs if local endpoints are referenced.

---

## Priority recommendation

M069 should be a **research and design milestone**, not an implementation milestone. The safest order is:

1. Finish/prepare M064 queue foundation if execution scheduling is needed.
2. Run M069 S01-S02 to turn Agents-K1 inspiration into verified schema + metrics.
3. Only then run S03 DSPy + MiniMax feasibility.
4. Use S04-S05 to decide production graph schema and retrieval operators.

The most valuable immediate research is **Appendix D schema + Section 7.2 metrics**, because those determine whether DSPy/MiniMax optimization has a measurable target.
