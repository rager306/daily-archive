# Agents-K1: Towards Agent-native Knowledge Orchestration — Research Summary

**Source**: arxiv 2606.13669, Shanghai AI Lab, 11 June 2026
**Date saved**: 2026-06-15
**Channel**: Discover AI / @code4AI video QuTcXMgPanw
**Relevance to daily-archive**: HIGH (scientific KG construction, multi-agent, GRPO)

---

## TL;DR

**Agents-K1** = end-to-end pipeline, который:
1. Парсит PDF (MinerU)
2. Извлекает структуру 4B-моделью (Qwen3-4B-Instruct + GRPO)
3. Строит мультимодальный научный KG (Scholar-KG, 2.46M статей)
4. Отдаёт его агенту через tri-source CLI

**Ключевая цифра**: GPT-5.2 → 41.8% on Geoscience, + Agents-K1 → 66.3% (**Δ +24.5 п.п.**)

**Главная идея**: проблема не в retrieval, а в структуре знания. GraphRAG уплощает науку до триплетов и теряет условные зависимости, мультимодальность (таблицы/формулы/графики), мотивации, ограничения, типы цитирований.

---

## 5 проблем классического GraphRAG

| # | Проблема | Что теряется |
|---|---|---|
| 1 | **Flattens science into triplets** | Условные зависимости (метод улучшает робастность *только если* curriculum scheduling + dataset D + adversarial perturbation) |
| 2 | **Ignores multimodal evidence** | Таблицы, графики, формулы → «декорация» |
| 3 | **Lacks scientific abstractions** | Мотивация, гипотезы, failure modes, novelty, future work, mechanism |
| 4 | **Citations as flat edges** | Тип отношения: supports/contradicts/extends/benchmarks against/criticises/reuses partially |
| 5 | **Retrieves only chunks** | Lineage path, экспериментальные условия, сравнение с предыдущими работами |

---

## 3 слоя архитектуры

### 🔹 Слой 1 — KG Layer: парсер + 5-модульная схема

**Парсер:** MinerU (Shanghai AI Lab OpenDataLab) — извлекает text/figures (caption)/tables (с cell-структурой)/equations (LaTeX)/citations.

**Схема knowledge graph: модули A → E**

| Модуль | Что внутри |
|---|---|
| **A — Factual/Meta** | Paper, Authors, Affiliations, Resources |
| **B — Textually Mentioned** | Tasks, Methods, Datasets+Splits+Modalities, Metrics, Baselines, Implementation details, Theorems, Definitions, Lemmas, Figures, Tables, Equations, Examples |
| **C — Implicit/Abstracted** | Problem Definition, Motivations, Gaps, Contributions, Hypotheses, Assumptions, Findings, Mechanisms, Limitations, Threats, Design Rationales, Future Work, Error Analyses |
| **D — Citation Relationships** | Cite type (strong/weak, direct/indirect), Relation (support/contrast/extend/background), Evidence (section/paragraph indices), Temporal, Author/Team/Source signals |
| **E — Knowledge Relations** | (см. ниже) |

**Таксономия отношений (25 типов, Модуль E)**:
- **Controlled (доменно-нейтральные)**: BUILDS_ON, USES_COMPONENT, ALTERNATIVE_TO, SOLVES, APPLIED_TO, TARGETS
- **Causal**: CAUSES, ENABLES, INHIBITS, MODULATES, CORRELATED_WITH
- **Internal composition**: USES_TECHNIQUE, CONSISTS_OF, IMPLEMENTS, COMBINES, REQUIRES
- **Methodological comparison**: DERIVED_FROM, DIFFERS_FROM, HAS_LIMITATION, ADDRESSES_PROBLEM, MOTIVATED_BY, HAS_PROPERTY, SUBSET_OF
- **Citation layer**: CITES, SUPPORTS, CONTRASTS, EXTENDS

### 🔹 Слой 2 — LLM Layer: 4B-экстрактор + GRPO

**Базовая модель**: Qwen3-4B-Instruct
**Тренировка**: ~1 час на 1 ноде × 8 GPU H200
**Задачи**: NER, Relation Extraction, Structured NER

**GRPO math**:
- Total loss: L(θ) = L_PG(θ) + L_KL(θ)
- PPO-clipped objective (L_PG)
- Low-variance KL к frozen reference (added as loss, NOT folded into reward)
- Advantage: group-normalized Â = (r − μ_b) / (σ_b + ε)
- Reward: R = R_fmt + R_json + R_F1
  - R_fmt: 0.1·[<think>] + 0.1·[<answer>] (max 0.2)
  - R_json: 0.1·[parses] + 0.05·[valid] (max 0.1)
  - R_F1: 0.7·F1_τ (max 0.7)

### 🔹 Слой 3 — CLI Layer: Tri-Source + Multi-Agent Swarm

**Tri-Source Knowledge Retrieval**:
- **S_web** — Web Search (arXiv, Semantic Scholar, Google Scholar)
- **S_mmkg** — Multimodal Graph Retrieval (hybrid dense + lexical)
- **S_kn** — Knowledge Network Traversal (**no vector search**, symbolic)
- Fusion weights: default (0.30, 0.40, 0.30) / recency (0.70, 0.15, 0.15) / multimodal (0.15, 0.70, 0.15)

**6 CLI graph operators**:
- O1: Seed Resolution (mentions → canonical nodes)
- O2: Citation Lineage Reconstruction
- O3: Comparative Baseline Retrieval
- O4: Multimodal Anchor Retrieval (figures/tables/equations)
- O5: Gap Detection
- O6: Idea Grounding / Novelty Judging

**6 Multi-Agent roles**:
- Coordinator
- SurveyWorker
- CodeWikiWorker
- IdeaWorker
- PrototypeWorker
- Aggregator

---

## Теоретические основы (Section 4.5)

| P | Содержание |
|---|---|
| **P1: Identifier-Preserving Joins** | Stable IDs → hash join за O(|K|), без false merges. |
| **P2: Cross-View Reachability** | Union view ⊇ single view. *Strict gap*: gold path с hyperedge arity ≥3 — binary projection **гарантированно теряет endpoints**. |
| **P3: Candidate Coverage** | Recall union view ≥ best single-view recall + доля gold answers только через joined view. |

---

## Бенчмарки (точные цифры)

### FrontierScience-Research (overall accuracy)

| Модель | Overall | Physics | Chemistry | Biology |
|---|---|---|---|---|
| Gemini-3 | 7.9% | 0.0% | 18.8% | 5.0% |
| GPT-5.2 | 25.2% | 9.0% | 33.7% | 32.8% |
| **GPT-5.2 + Agents-K1** | **39.4%** | **46.7%** | **36.7%** | **35.0%** |

### Geoscience Research Questions

| Модель | Rationale Acc | Answer Acc |
|---|---|---|
| GPT-5.2 | 41.8% | 58.8% |
| Gemini-3 | 52.3% | 61.0% |
| **GPT-5.2 + Agents-K1** | **66.3%** | **69.7%** |
| **Gemini-3 + Agents-K1** | **69.5%** | **71.5%** |

### Multi-hop QA (Agents-K1)

| Бенчмарк | Contain | GPT-Acc |
|---|---|---|
| HotpotQA | 63.50% | 67.80% |
| 2WikiMultiHopQA | 67.10% | 64.80% |
| MuSiQue | 31.10% | 36.20% |

### Information Extraction (10 датасетов · 12 078 instances)

| Модель | Avg F1 |
|---|---|
| Qwen3-4B (base) | 0.5316 |
| Qwen3-8B | 0.5382 |
| Qwen3-32B | 0.5746 |
| **Ours (4B + GRPO)** | **0.5647** |

**4B + GRPO** обходит 4B/8B-базы, догоняет 32B — закрывает 8×-разрыв в масштабе.

---

## Ablation: Core-then-Modes (-50% LLM calls)

1. **Core stage** (2 LLM-прохода на чанк): typed entities + binary relation skeleton
2. **Mode stages:**
   - Projection (binary, person) → детерминированно, 0 LLM
   - Upgrade (n-ary, temporal, event) → 1 LLM per chunk

**Цифры** при n=8, n_up=4, M=6 views:
- Naive: 8 × 6 × 2 = 96 LLM-вызовов
- Core-then-Modes: 8 × (2 + 4) = 48 → **−50%** при той же выдаче

---

## Скейл и хранение

- **2.46M статей** в Scholar-KG (1M выложено на HF)
- **Storage**: Neo4j
- **Parser**: MinerU
- **Schema-adaptive variant**: General-KG для нелитературных доменов

---

## 🔗 Все ссылки (проверены)

| Ресурс | URL |
|---|---|
| 📄 arXiv abstract | https://arxiv.org/abs/2606.13669 |
| 📖 arXiv HTML | https://arxiv.org/html/2606.13669v1 |
| 💻 GitHub: GraphAnything | https://github.com/InternScience/GraphAnything |
| 🤗 HF Model: Agents-K1 (4B) | https://huggingface.co/InternScience/Agents-K1 |
| 🗂 HF Dataset: Scholar-KG | https://huggingface.co/datasets/InternScience/Scholar-kg |
| 🌐 SCP Portal | https://scphub.intern-ai.org.cn/detail/42 |
| 🏢 Org | https://github.com/InternScience |
| 🧩 IdeaMiner | https://github.com/InternScience/IdeaMiner |
| 🛠 MinerU | https://github.com/opendatalab/MinerU-Ecosystem |

---

## 💡 7 главных инсайтов

1. **Knowledge ≠ retrieval** — индустрия оптимизирует сам корпус знаний
2. **Separation of concerns** — 4B extraction, GPT-5.2 reasoning
3. **Graph traversal ≠ vector search** — S_kn даёт long-chain causal
4. **Stable IDs are everything** — все 3 теоретические пропозиции
5. **Hyperedges matter** — strict gap из P2 (формальное обоснование почему GraphRAG проваливается)
6. **1 час GRPO на 8×H200** → закрытие 8×-разрыва в масштабе
7. **Триада оптимизаций Discover AI**: Hierarchical Memory → Hierarchical Planning → Knowledge Orchestration

---

## 🔗 Сравнение с daily-archive

| Aspect | Agents-K1 | daily-archive (M067 state) |
|---|---|---|
| **Graph DB** | Neo4j | **FalkorDB** (M067, 70/90 self-hosted) |
| **Parser** | MinerU | plotextractor (TeX source) + hybrid GROBID/ODL |
| **KG scope** | 2.46M papers (Scholar-KG) | 220 PDFs (M061 S04 ingested) |
| **Layers** | 5 modules A-E (richer schema) | 5-layer (citation/table/figure v1/v2/judge) |
| **Models** | 4B extraction (GRPO) + 70B reasoning | MiniMax-M3 multimodal judge + MiniMax text extraction path |
| **Multi-agent** | 6 roles + 6 graph operators + 17 MCP servers | Not yet (planned for M064+) |
| **Tri-source** | S_web + S_mmkg + S_kn | Single source (M061 v1) — could be added |
| **Triplet limitation** | NOT limited (multi-view, hyperedge) | Currently limited (NetworkX intermediate) |
| **Optimization** | GRPO on Qwen3-4B (requires GPU class hardware) | **DSPy prompt/program optimization over MiniMax API** (no local GPU required) |
| **Benchmarks** | Multi-hop QA, IE, Frontiers, Geoscience | Per-paper pipeline, no research QA yet |

**Implications для daily-archive**:
1. **Schema enrichment** — current 5-layer is too thin vs Agents-K1 5 modules (need to add Module C: implicit/abstractions)
2. **Tri-source retrieval** — currently only S_web (arxiv API) + S_kn (NetworkX), no S_mmkg
3. **DSPy + MiniMax instead of GRPO/Qwen3-4B** — у нас нет GPU, поэтому локальное обучение Qwen3-4B не является практичным направлением; S03 должен адаптировать MiniMax через DSPy optimizers (`BootstrapFewShot`, `MIPRO`, `BootstrapRandomSearch`) и сравнить cost/latency/quality с текущим MiniMax-M3 multimodal judge
4. **Multi-agent CLI** — M064 (queue) + M065+ (CLI) could adopt 6-role pattern

---

## Что осталось "на тонком уровне" (для будущего копания)

- 📑 Section 4.2: методология LLM-Guided Multi-Hop QA Generation
- 🧮 Section 7.2: точные формулы метрик (Contain-Acc, GPT-Acc)
- 📐 Appendix B: конструктивные доказательства P1-P3
- 🗂 Appendix D: disaggregated schema
- 💻 GitHub InternScience/GraphAnything: реальные MCP-инструменты

---

## Возможные действия (отложено)

1. Сделать инфографику pipeline Agents-K1
2. Углубиться в Appendix B (доказательства)
3. Разобрать GitHub GraphAnything (MCP tools)
4. Сравнить с HippoRAG 2 / LightRAG / GFM-RAG
5. **M069 S03 revised**: DSPy prototype over MiniMax API (not GRPO/Qwen3-4B) with cost/latency/quality comparison against MiniMax-M3 multimodal judge

---

**Status**: сохранено для будущего анализа. M069 потенциально может быть "Agents-K1 inspired enhancements to daily-archive" milestone.
