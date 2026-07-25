# ADR-039: Grounded Architecture v3 — Lifecycle, Validation, Sources

**Status:** Accepted (binding)
**Date:** 2026-07-25
**Deciders:** collaborative
**Extends and corrects:** ADR-037 (structure) and ADR-038 (schema) — both remain binding for their scope; this ADR adds lifecycle discipline, staged validation, source citations, and corrects over-claims. Read all three together.

> **ADR-040 confirms:** lifecycle tags and staged validation gates remain binding. Graph store lifecycle updated: NebulaGraph `[proposed]` → Samyama `[bounded]` (smoke proven, pipeline pending).
**Binding Level:** binding
**Revisable:** no — lifecycle discipline is non-negotiable

---

## 0. Why this ADR exists

ADR-037 + ADR-038 defined the Rust architecture (hexagonal, RuVector+Samyama, SymFSM agents, Agents-K1 schema). But they **over-claimed**:

| Over-claim in 037/038 | Reality check | Lesson source |
|---|---|---|
| "1M–100M scale" | We validated **0** papers in Rust; Python validated 60 canary | M274–M284 Python |
| "−58% LLM calls" | Agents-K1's math, not our measurement | arXiv 2606.13669 §5.2 |
| "SONA self-learning" | SONA integration unproven on our workload | ruvector-test audit |
| "GLiNER 2 offline NER" | Smoke-tested on 1 paper, not pipeline-scale | M284 GLiNER smoke |
| "production-ready" | Adapter success ≠ graph readiness | M033 S07 lesson |

**This ADR adds lifecycle discipline (proposed/bounded/validated), staged validation gates, source citations for every claim, and grounds the schema in real arXiv paper 1206.6423.**

---

## 1. Lifecycle Tags (binding — from law-nexus D098 pattern)

Every component, schema element, and claim carries a lifecycle tag:

| Tag | Meaning | Authorized for |
|-----|---------|---------------|
| `[proposed]` | Design only, no implementation | Architecture discussion |
| `[bounded]` | Implemented, fixture-tested, contract proven | Internal testing |
| `[validated]` | Tested on real batches with metrics | Production claims |
| `[frozen]` | No changes until unblocked dependency | Waiting |

**Rule (binding):** No `[validated]` tag without real-batch evidence. Fixture tests prove contracts, not production quality (M003 S06, M033 S07 lessons).

---

## 2. Staged Validation Gates (binding — from M274–M284 experience)

```text
Stage 0: FIXTURE      1 synthetic paper, deterministic expected output
         ↓ gate: contract test passes
Stage 1: CANARY-10    10 real arXiv papers, manual review
         ↓ gate: evidence resolvability ≥ 0.90 (char-only OK)
Stage 2: CANARY-60    60 canary papers, automated metrics
         ↓ gate: page/bbox resolvability > 0; structure gate pass
Stage 3: WEEK-BATCH   ~500 papers / week, ETL scheduler
         ↓ gate: prediction resolvability ≥ 0.70; no import
Stage 4: PRODUCTION   10K+ papers, Samyama distributed
         ↓ gate: explicit human go + full evidence chain green
```

**No component may claim `[validated]` without passing its stage gate.** Import-eligible stays `false` through Stage 0–3. Stage 4 requires explicit human yes (D127 preserved).

---

## 3. Grounded Component Matrix

Each component tagged with lifecycle + source citation:

### Layer 1: Domain (`da-domain`) — `[bounded]`

| Component | Lifecycle | Source / rationale | Status |
|-----------|-----------|-------------------|--------|
| `Paper`, `Author`, `Citation` types | `[bounded]` | ADR-032 domain profiles; Python M001–M060 | Port from Python, pure types |
| `Entity`, `Relation` (Module B) | `[bounded]` | Agents-K1 Module B (arXiv 2606.13669 §4.1) | 18 relation types curated |
| `EvidenceAssertion`, `SourceSpan` | `[validated]` | M274–M282 Python; 93/93 spans upgraded | Page/bbox proven on 23 gold |
| `CanonicalDocument` | `[validated]` | M275–M276 Python | ODL layout keys fixed |
| `Versioned<T>`, `TemporalRecord` | `[proposed]` | ADR-037 §6; not yet implemented | Design only |
| `AgentState`, `Trajectory`, `RewardSignal` | `[proposed]` | SymFSM pattern (ADR-023 §2.4) | Design only |
| Module C (Motivation, Gap, Finding) | `[proposed]` | Agents-K1 Module C | Needs LLM extraction |
| Module D (CitationContext node) | `[proposed]` | Agents-K1 Module D | Needs GROBID citations + LLM |
| SHA256 `vid` computation | `[bounded]` | Agents-K1 P1; Python identity module | Port from Python |

### Layer 2: Ports (`da-ports`) — `[proposed]`

| Port | Lifecycle | Source | Notes |
|------|-----------|--------|-------|
| `GraphStore` (CRUD + O1–O6) | `[proposed]` | ADR-038 §5; Agents-K1 operators | 6 operators as trait methods |
| `VectorStore` | `[bounded]` | RuVector HNSW + RVF (ruvector-test #1,#4) | Verified persistent |
| `Embedder` | `[bounded]` | RuVector OnnxEmbedding (bge-m3) | Feature-gated, untested locally |
| `ParserPort` | `[proposed]` | GROBID HTTP + ODL subprocess | External services |
| `LLMClient` | `[bounded]` | 9router OpenAI-compatible + MiniMax Anthropic | See §4 below |
| `SchedulerPort` | `[proposed]` | ADR-037 §4.3 resource-aware | Design |
| `EvidenceStore` | `[validated]` | M274 Python ParserRun artifacts | PDF hash + TEI + ODL layout |
| `AgentMemory` | `[bounded]` | RuVector agent-memory (ruvector-test audit) | LRU/LFU/coherence compaction |

### Layer 3: Application (`da-application`) — `[proposed]` mostly

| Use case | Lifecycle | Source | Validation gate |
|----------|-----------|--------|----------------|
| Ingest pipeline | `[bounded]` | Python M061 catalog ingest; MinerU=ODL confirmed | Stage 1 (canary-10) |
| Preprocess (non-LLM) | `[validated]` | Python M224–M228; ADR-036 | Stage 2 (canary-60) done |
| Core extraction (GLiNER) | `[bounded]` | M284 GLiNER smoke; 95% resolvability on 1 paper | Stage 1 pending |
| Core-then-Modes | `[proposed]` | Agents-K1 §5.2; −50% is THEIR math, not ours | Stage 2 will measure |
| Review gate (fail-closed) | `[validated]` | Python M278 promotion boundary; D127 | Preserved |
| Graph writer (Samyama) | `[proposed]` | ADR-037 §5 schema | Stage 3+ |
| ETL scheduler | `[proposed]` | ADR-037 §4.3 | Stage 3 |
| SymFSM agent | `[proposed]` | ADR-037 §4.4; Agents-K1 CLI layer | Stage 4+ |
| Tri-source retrieval | `[proposed]` | Agents-K1 §6.1; S_web+S_mmkg+S_kn | Stage 4+ |
| SONA integration | `[proposed]` | RuVector SONA; unproven on our workload | Stage 4+ experimental |

### Layer 4: Adapters (`da-adapters`)

| Adapter | Lifecycle | Source / detail | Verified? |
|---------|-----------|----------------|-----------|
| Samyama store | `[proposed]` | Samyama Rust SDK | Not deployed yet |
| RuVector RVF store | `[bounded]` | ruvector-test #1,#4 PASS | Persistent verified |
| RuVector HNSW cache | `[bounded]` | ruvector-test embedder path | Working set only |
| OnnxEmbedder (bge-m3) | `[proposed]` | RuVector feature-gated | Needs ONNX model download |
| GROBID parser (HTTP) | `[validated]` | Python M274; GROBID :8070 alive | TEI + citations proven |
| ODL parser (subprocess) | `[validated]` | Python M274; ODL layout JSON proven | Spaced keys fixed M283 |
| GLiNER 2 extractor | `[bounded]` | M284 smoke; 20 entities, 95% resolve | 1 paper only |
| 9router LLM (GLM-5.2) | `[validated]` | Python M284; 12/12 held-out extract | Prediction resolvability 0.74 |
| MiniMax LLM (Anthropic) | `[bounded]` | Memory block: `api.minimax.io/anthropic/v1/messages`, `X-Api-Key`, `MiniMax-M2.7-highspeed` | Endpoint shape verified |
| RuVector agent memory | `[bounded]` | ruvector-test audit | Compaction works |

---

## 4. LLM Adapter — Concrete Details (from Python experience + memory)

**9router model roles** (from GSD preferences context):

| Role | Model | When | Cost |
|------|-------|------|------|
| Fast default | `agnes-ai/agnes-2.0-flash` | Simple extraction, classification | low |
| Quality | `minimax/MiniMax-M2.7-highspeed` | Complex reasoning, synthesis | med |
| Fallback | `grok-4.5` | When primary rate-limited | med |
| Judge | `glm-5.2` / `gpt-5.2` | Verification, checker model | high |

**MiniMax Anthropic-compatible endpoint** (memory block gotcha):
```
URL:    https://api.minimax.io/anthropic/v1/messages
Header: X-Api-Key: <MINIMAX_API_KEY value>   (NOT Authorization: Bearer)
Model:  MiniMax-M2.7-highspeed
```
**401 cause:** stale/wrong key, not endpoint shape. The Anthropic path works; do not assume OpenAI-compatible is the only option.

**LLMClient trait** must support both:
```rust
pub trait LLMClient: Send + Sync {
    async fn chat(&self, messages: &[Message], opts: &ChatOptions) -> Result<ChatResponse>;
    async fn extract_structured(&self, text: &str, schema: &Schema) -> Result<serde_json::Value>;
    fn can_make_request(&self) -> bool;  // rate-limit check BEFORE call
    fn provider_id(&self) -> &str;
}

// Two adapter implementations:
pub struct NineRouterClient { /* OpenAI-compatible, http://127.0.0.1:20128 */ }
pub struct MiniMaxAnthropicClient { /* Anthropic-compatible, api.minimax.io */ }
```

**Statistical-first guard (binding — ADR-023 §2.2, ADR-036):**
Every extraction stage MUST produce statistical/deterministic output BEFORE any LLM call. GLiNER NER + header-priority run first. LLM receives chunk text AND statistical context (keyword frequencies, section position, citation structure). This is verified `[validated]` from Python M224–M233.

---

## 5. Grounded Schema — Traced Through Real Paper 1206.6423

Showing how arXiv paper `1206.6423` ("Seq2Seq Models for Knowledge Graph Link Prediction") flows through the schema. This is the paper we have full evidence for (M281–M284).

### Module A — Factual

```text
Paper vid=sha256("paper:1206.6423")
  arxiv_id: "1206.6423"
  title: "Seq2Seq Models for Knowledge Graph Link Prediction"
  pdf_hash: <SHA256 of source PDF>
  evidence_ready: true       [validated M283]
  import_eligible: false     [D127 — never auto-flip]

Author vid=sha256("author:...')
  name: "<from GROBID header>"
  orcid: "" (not in this paper)

Citation edge: Paper → Paper (via CitationContext node)
```

### Module B — Textually Mentioned (GLiNER 2 extracted, M284 smoke)

From GLiNER 2 NER on body[:4000]:
```text
Method vid=sha256("method:joint learning")
  label: "joint learning"
  confidence: 0.77
  source_span: {char_start: 707, char_end: 721}
  → layout upgrade: page=7, bbox=[...]

Method vid=sha256("method:probabilistic categorial grammar induction")
  label: "probabilistic categorial grammar induction"
  confidence: 0.76
  source_span: {char_start: 2811, char_end: 2853}
  → layout upgrade: page=1, bbox=[...]

Task vid=sha256("task:language grounding problem")
  label: "language grounding problem"
  confidence: 0.82

Metric vid=sha256("metric:marginal likelihood")
  label: "marginal likelihood"
  confidence: 0.74

Dataset vid=sha256("dataset:amazon mechanical turk")
  label: "Amazon Mechanical Turk"  (normalized from "Ama-\n\nzon")
  confidence: 0.98
```

**20 entities found, 19/20 resolved to page/bbox = 95% resolvability** (M284 GLiNER smoke).

### Module C — Implicit (LLM upgrade mode, NOT yet run)

```text
[proposed] — would need LLM SYNTHESIS pass:
Motivation: "Existing KG completion methods don't model sequence structure"
Gap: "No seq2seq approach for link prediction"
Contribution: "First application of seq2seq to KG link prediction"
Finding: {metric: "MRR", value: "0.X", effect_size: ...}
```

### Module D — Citation Context (GROBID TEI parsed, M274)

```text
CitationContext vid=sha256("cite:1206.6423->2108.12409")
  cite_type: "direct"
  relation: "extend"      [proposed — needs LLM classification]
  evidence_section: "related_work"
  source_span: {char_start: ..., page: 2}
```

GROBID extracted 27 citations from this paper (M283 batch report). Citation context classification (support/contrast/extend) is `[proposed]` — needs LLM.

### Module E — Relations (Core-then-Modes)

```text
Core (GLiNER 2 RE, M284 smoke):
  (Method: "joint learning") -[APPLIED_TO]-> (Task: "language grounding problem")
  (robot) -[USES_COMPONENT]-> (Method: "grounded attribute induction")

Projection (deterministic, 0 LLM):
  Binary flatten: already binary

Upgrade [proposed] — would need LLM:
  (Method: "seq2seq") -[BUILDS_ON]-> (Method: "probabilistic categorial grammar")
  (Method: "seq2seq") -[EVALUATED_ON]-> (Dataset: "Amazon Mechanical Turk")
```

---

## 6. Review-Gate-Before-Promotion (binding — from M278, memory block)

The SymFSM agent flow (ADR-037 §4.4) gains an explicit **REVIEW** state that **refines** the VERIFY state — REVIEW adds graph-readiness post-check and adapter-success-≠readiness check to VERIFY's structural/citation checks. In the binding FSM: SYNTHESIS → REVIEW (fail-closed, includes VERIFY checks) → OUTPUT.

```text
FSM State: REVIEW (fail-closed, refines ADR-037 VERIFY)
  ├── structural verifier: every claim → resolvable SourceSpan
  ├── citation checker: every cite → real Paper node in Samyama
  ├── graph-readiness post-check (memory block convention):
  │     verify output_contract_completed=true before promotion
  ├── adapter-success ≠ readiness check (M033 S07 lesson):
  │     parser/extractor success does NOT imply graph readiness
  └── if any check fails → REPAIR or block (never auto-promote)

  transition:
    all pass + explicit human go → OUTPUT
    any fail → REPAIR (max 2 loops)
    human go absent → OUTPUT(honest gap), import_eligible stays false
```

**Binding rules (from Python M278, M033, M284):**
1. `import_eligible = false` always, until explicit human yes (D127)
2. Adapter success (parser/extractor returns data) ≠ graph readiness
3. Fixture test passes ≠ production quality claim
4. Review artifact post-check runs BEFORE manifest synthesis
5. Staged validation gate must pass before `[validated]` tag

---

## 7. DSPy / Optimizer Guard (binding — from M003 S06, memory block)

```text
[GUARDED — not authorized until metrics verified]

DSPy, GEPA, RLM optimizers, GRPO training:
  - NOT authorized in da-application
  - Activate ONLY after:
    1. Evaluation metrics designed and verified (Stage 2)
    2. Benchmark fixtures locked (Stage 2)
    3. Real-batch metrics show optimizer would help (Stage 3)
  - Even then: GT isolation enforced (canary held-out never in train)
  - SONA self-learning: [proposed] experimental, Stage 4+
```

**No optimizer claims without measured metrics on real batches.** This is the M003 S06 / S07 lesson made binding.

---

## 8. Source Citations Per Architectural Decision

| Decision | Source | Evidence strength |
|----------|--------|------------------|
| Hexagonal/onion structure | law-nexus ADR-0001; Python M100 | `[validated]` — enforced in Python, pattern proven |
| Samyama Graph (not NebulaGraph/FalkorDB) | Scale analysis 1M/10M/100M; redb single-writer limit | `[bounded]` — ruvector-test audit; not yet deployed |
| RuVector as agent brain (not primary store) | ruvector-test README; redb limits at scale | `[bounded]` — audit verified capabilities |
| SymFSM agent control | ADR-023 §2.4; Agents-K1 CLI layer | `[proposed]` — pattern, not runtime |
| Core-then-Modes extraction | Agents-K1 arXiv 2606.13669 §5.2 | `[proposed]` — their math, our measurement pending |
| 18 relation types (from 25) | Agents-K1 Module E; 7 causal deferred (no GPU) | `[proposed]` |
| Tri-source retrieval | Agents-K1 §6.1 | `[proposed]` |
| 6 graph operators | Agents-K1 §6.2 | `[proposed]` |
| GLiNER 2 offline NER | M284 smoke; GLiNER 2 PyPI v1.3.2 | `[bounded]` — 1 paper, 95% resolve |
| MinerU = ODL parser | OpenDataLab ecosystem; Scholar-KG uses it | `[validated]` — Python M274 |
| Statistical-first | ADR-023 §2.2; ADR-036; Python M224–M233 | `[validated]` |
| Evidence chain immutable | M274–M282; 93/93 spans | `[validated]` |
| Import gate fail-closed | D127; M278 promotion boundary | `[validated]` |
| SHA256 stable VIDs | Agents-K1 P1; Python identity module | `[bounded]` |
| LLM rate-limit-aware | ADR-023 §2.3; Python M025 | `[validated]` |
| MiniMax Anthropic endpoint | Memory block gotcha; Python M014 | `[bounded]` |
| Consensus.app 3-step ranking | consensus.app blog; OpenAI case study | `[proposed]` — inspiration, not verified |
| Versioning + temporality | ADR-037 §6; not in Python | `[proposed]` |
| Hyperedge ExperimentSetup | Agents-K1 P2 | `[proposed]` |

---

## 9. Corrected Scale Claims

| Claim in ADR-037/038 | Corrected (lifecycle-honest) |
|---|---|
| "1M–100M scale" | `[proposed]` — designed for, validated at 60 (Python) / 0 (Rust) |
| "−58% LLM calls" | `[proposed]` — Agents-K1's measurement; ours pending Stage 2 |
| "SONA self-learning works" | `[proposed]` — RuVector has it, our integration unproven |
| "GLiNER 2 production-ready" | `[bounded]` — 1 paper smoke, 95% resolve; pipeline-scale pending |
| "Tri-source retrieval" | `[proposed]` — designed, not implemented |
| "production-ready ~50 capabilities" | That's RuVector's audit, not our integration readiness |

---

## 10. What is actually `[validated]` right now

From Python M274–M284 (transferable evidence):

| Capability | Evidence | Papers |
|-----------|----------|--------|
| GROBID + ODL hybrid parsing | TEI + layout JSON, ParserRun | 60 canary |
| Layout span upgrade (page/bbox) | 93/93 spans, 69 page_or_bbox | 23 gold |
| Evidence resolvability | rate 1.0 char-only; 1.0 page/bbox after upgrade | 23 gold |
| Structure gate (IR hard) | 66 ir_hard, pass_rate 1.0 | 60 canary |
| GLiNER 2 offline NER | 20 entities, 95% resolve to page/bbox | 1 paper |
| Prediction resolvability (LLM) | rate 0.74, page_bbox 71 | 12 held-out |
| Import gate fail-closed | D127 enforced, import_eligible=false | all |
| Statistical-first preprocess | body clean, quality, outline, keywords | 60 canary |
| LLM extraction (9router glm-5.2) | 12/12 held-out extracted | 12 held-out |

**Everything else is `[proposed]` or `[bounded]`.** The Rust rewrite starts from these validated Python patterns, ported to Rust types.

---

## 11. Binding Rules Summary (non-negotiable)

1. **Lifecycle tags mandatory** — every component, schema, claim tagged `[proposed/bounded/validated/frozen]`
2. **Staged validation gates** — no `[validated]` without real-batch evidence at the correct stage
3. **Fixture ≠ production** — fixture tests prove contracts, not quality (M003 S06)
4. **Adapter success ≠ readiness** — parser/extractor returning data ≠ graph readiness (M033 S07)
5. **Import always false** — until explicit human yes + full evidence chain (D127)
6. **Statistical-first** — deterministic preprocessing before every LLM call (ADR-023, ADR-036)
7. **Review before promotion** — post-check runs before manifest synthesis (memory convention)
8. **No optimizer without metrics** — DSPy/GEPA/GRPO guarded until Stage 2+ (M003 S07)
9. **GT isolation** — canary held-out never in train set (M279)
10. **Honest scale claims** — designed-for ≠ validated-at

---

## 12. Migration: What ports first from Python

Priority order (validated → bounded → proposed):

1. **`[validated]` port first:** EvidenceAssertion, SourceSpan, CanonicalDocument, layout span upgrade logic, structure gate — these are pure logic, proven on 60 papers
2. **`[bounded]` next:** GLiNER 2 adapter (smoke proven), Samyama schema (design proven), RuVector store (audit proven)
3. **`[proposed]` last:** SymFSM agents, tri-source, SONA, Module C/D extraction — design only, needs Stage 2+ validation

**First Rust milestone:** Port `da-domain` types + `da-evidence` (layout span upgrade) + verify on paper 1206.6423 → match Python's 95% resolvability. This is the smallest unit that proves the Rust rewrite works.
