# Extraction Pipeline Design (M101 S03)

## Overview

Implements Core-then-Modes factorization (Agents-K1) adapted for daily-archive with:
- Statistical-first pre-processing (ADR-024)
- Multi-provider LLM with rate limits (ADR-025)
- 3-lane scheduler integration (ADR-027)
- Typed schema output (ADR-028)

## Pipeline Architecture

```text
SemanticChunk (input)
    ↓
[Statistical Pre-processing] — deterministic, 0 LLM calls
    ├── YAKE keywords per chunk → entity candidates
    ├── TF-IDF section type classification
    ├── Co-occurrence matrix from keywords
    ├── BGE-M3 embedding (existing fd service)
    └── Citation graph structure (BFS depth from bibliography)
    ↓
StatisticalContext { keywords, section_type, co_occurrence, embedding, citation_position }
    ↓
═══════════════════════════════════════════════
[CORE STAGE] — 2 LLM passes per chunk
═══════════════════════════════════════════════
    ↓
Pass 1: Typed Entity Extraction (NER)
    Input: chunk text + StatisticalContext
    Output: list[TypedEntity] with type, canonical_name, confidence
    ↓
Pass 2: Binary Relation Skeleton
    Input: chunk text + entities from Pass 1 + co_occurrence
    Output: list[(entity_a, entity_b, relation_exists)]
    ↓
═══════════════════════════════════════════════
[PROJECTION MODES] — deterministic, 0 LLM calls
═══════════════════════════════════════════════
    ↓
    Binary view: relation_exists → typed edge skeleton
    Provenance view: link each entity/relation → EvidencePath
    ↓
═══════════════════════════════════════════════
[UPGRADE MODES] — 1 LLM pass each, parallelizable
═══════════════════════════════════════════════
    ↓
Mode A: Relation Type Classification
    Input: binary relations + StatisticalContext
    Output: typed relation (27 types from ADR-028)
    ↓
Mode B: Abstract Entity Extraction (Module C)
    Input: section text + section_type
    Output: Problem, Motivation, Gap, Contribution, Hypothesis, Finding, etc.
    ↓
Mode C: Citation Relation Classification
    Input: citation pairs + citation graph structure
    Output: SUPPORTS / CONTRASTS / EXTENDS / CITES
    ↓
[OUTPUT] — CandidatePacket with safety_flags=false
    TypedEntity[] + TypedRelation[] + AbstractEntity[] + CitationRelation[]
    All linked to EvidencePath + ExtractionRef
```

## DSPy Signatures

### Core: Typed Entity Extraction

```python
class TypedEntityExtraction(dspy.Signature):
    """Extract typed entities from a scientific text chunk.
    
    Given a chunk of text and statistical context (keywords, section type),
    extract all entities with their types.
    """
    chunk: str = dspy.InputField(desc="Normalized text chunk")
    keywords: str = dspy.InputField(desc="YAKE keyword candidates with scores")
    section_type: str = dspy.InputField(desc="Predicted section type")
    
    entities: str = dspy.OutputField(desc="JSON list of {type, name, confidence}")
```

### Core: Binary Relation Detection

```python
class BinaryRelationDetection(dspy.Signature):
    """Detect whether relations exist between extracted entities.
    
    Given entities and co-occurrence statistics, identify which pairs
    have meaningful relations.
    """
    entities: str = dspy.InputField(desc="JSON list of entities")
    co_occurrence: str = dspy.InputField(desc="Co-occurrence counts")
    chunk: str = dspy.InputField(desc="Source text")
    
    relations: str = dspy.OutputField(desc="JSON list of {from, to, exists}")
```

### Upgrade: Relation Type Classification

```python
class RelationTypeClassification(dspy.Signature):
    """Classify the type of a detected relation.
    
    Given two entities and their context, classify the relation into
    one of 27 types (controlled, causal, composition, comparison, citation).
    """
    entity_a: str = dspy.InputField(desc="Source entity (type + name)")
    entity_b: str = dspy.InputField(desc="Target entity (type + name)")
    context: str = dspy.InputField(desc="Surrounding text + statistical hints")
    
    relation_type: str = dspy.OutputField(desc="One of 27 typed relations")
    confidence: float = dspy.OutputField(desc="0.0 to 1.0")
```

### Upgrade: Abstract Entity Extraction (Module C)

```python
class AbstractEntityExtraction(dspy.Signature):
    """Extract implicit/abstracted scientific concepts.
    
    Given a section of text and its predicted type, extract abstract entities
    like motivations, hypotheses, findings, limitations.
    """
    section_text: str = dspy.InputField(desc="Full section text")
    section_type: str = dspy.InputField(desc="Introduction/Method/Results/etc.")
    keywords: str = dspy.InputField(desc="YAKE keywords for grounding")
    
    abstracts: str = dspy.OutputField(desc="JSON list of {type, statement, evidence_span}")
```

### Upgrade: Citation Relation Classification

```python
class CitationRelationClassification(dspy.Signature):
    """Classify the argumentative relationship in a citation.
    
    Given a citing sentence and the cited work's title, classify
    whether the citation supports, contrasts, extends, or merely cites.
    """
    citing_sentence: str = dspy.InputField(desc="Sentence containing citation")
    cited_title: str = dspy.InputField(desc="Title of cited work")
    bfs_depth: int = dspy.InputField(desc="Distance in citation graph")
    
    relation: str = dspy.OutputField(desc="SUPPORTS|CONTRASTS|EXTENDS|CITES")
    confidence: float = dspy.OutputField(desc="0.0 to 1.0")
```

## Provider Routing Strategy

```python
def select_provider(estimated_tokens: int, job_priority: str) -> str:
    """Select LLM provider based on rate limits and priority."""
    # Check MiniMax first (primary)
    if minimax_usage.can_make_request(estimated_tokens):
        return "minimax"
    
    # Fallback to GLM (secondary)
    if glm_usage.can_make_request(estimated_tokens):
        return "glm"
    
    # All providers exhausted — queue for later
    raise RateLimitExhausted(
        "All LLM providers rate-limited. Job queued."
    )
```

### Per-stage provider preferences

| Stage | Preferred provider | Fallback | Reason |
|---|---|---|---|
| Core: Entity extraction | MiniMax M3-512k | GLM-5.2 | Needs large context for chunk + keywords |
| Core: Binary relations | MiniMax M3-512k | GLM-5.2 | Needs entity list in context |
| Upgrade: Relation type | MiniMax M2.7-highspeed | GLM-4.5-Air | Classification task, fast model OK |
| Upgrade: Abstract entities | MiniMax M3-512k | GLM-5.2 | Complex reasoning, needs context |
| Upgrade: Citation relation | MiniMax M2.7-highspeed | GLM-4.5-Air | Short input, classification |

## Cost Model

### Per-article cost estimate

Assumptions:
- Average paper: 8 chunks (after structure-aware chunking)
- Average chunk: ~2000 tokens input, ~500 tokens output
- 4 reference blocks per chunk (citation classification)

| Stage | Calls/chunk | Total calls | Input tokens | Output tokens |
|---|---|---|---|---|
| Statistical pre-processing | 0 | 0 | 0 | 0 |
| Core: Entity extraction | 1 | 8 | ~16K | ~4K |
| Core: Binary relations | 1 | 8 | ~16K | ~2K |
| Upgrade: Relation type | 1 | 8 | ~8K | ~2K |
| Upgrade: Abstract entities | 1 | 8 | ~16K | ~4K |
| Upgrade: Citation relation | 0.5 (4 refs) | 4 | ~4K | ~1K |
| **Total** | — | **36** | **~60K** | **~13K** |

### Cost per provider

| Provider | Input price | Output price | Cost per article |
|---|---|---|---|
| MiniMax M3-512k | ~$0.001/K input | ~$0.002/K output | ~$0.086 |
| GLM-5.2 | Subscription (5h rolling) | Included | $0 (within quota) |
| **Blended** (80% MiniMax, 20% GLM) | — | — | **~$0.069** |

### Rate limit budget

| Provider | Limit | Articles per window |
|---|---|---|
| MiniMax token plan | ~1M tokens/day | ~14 articles/day |
| GLM 5-hour window | ~500K tokens/5h | ~7 articles/5h |
| **Combined throughput** | — | **~20 articles/day** |

### Comparison with naive approach

| Approach | LLM calls/article | Cost/article |
|---|---|---|
| Naive (1 call per chunk, no factorization) | 8 × 5 modes = 40 | $0.096 |
| Core-then-Modes (our design) | 36 (but 0 for projection) | $0.069 |
| Core-then-Modes + statistical pre-filter | ~28 (skip obvious chunks) | ~$0.054 |

## 3-Lane Scheduler Integration (ADR-027)

Extraction jobs are tagged:

```python
ResourceProfile(
    llm_required=True,
    llm_provider="minimax",  # or "glm" for fallback
    estimated_tokens=2500,   # per call
    cpu_required=False,
    io_required=False,
)
```

The scheduler:
1. Checks MiniMax token_plan/remains before dispatching extraction jobs
2. Falls back to GLM when MiniMax is exhausted
3. Queues extraction jobs when all providers are rate-limited
4. Continues running CPU jobs (parsing, chunking) while LLM lane is full

## Headroom Evaluation Plan

**Status**: NOT adopted. Candidate only (ADR-025).

| Criterion | How to evaluate | Pass threshold |
|---|---|---|
| Maintenance | Check GitHub last commit, issue activity | Active within 3 months |
| Dependencies | `pip install headroom` footprint | <5MB, no heavy deps |
| License | Read LICENSE file | Compatible with self-hosted SSPLv1 context |
| API compatibility | Test with MiniMax + GLM endpoints | Both work without errors |
| Provenance | Verify evidence/span preservation after compression | No data loss |
| F1 impact | Run extraction benchmark with/without Headroom | F1 delta < -2% |
| Cost savings | Measure token reduction | >20% reduction |

**Decision gate**: all 7 criteria must pass before adoption.

## DSPy Optimization Plan

### BootstrapFewShot

1. Manually label 10 chunks from M056 corpus (2605.18747 refs)
2. Each label: TypedEntity[] + TypedRelation[] + AbstractEntity[]
3. DSPy BootstrapFewShot selects best examples as few-shot prompts
4. Evaluate on held-out 5 chunks → F1 score

### MIPRO (future)

If BootstrapFewShot F1 < 0.6:
1. Use MIPRO to optimize instruction text
2. Search over prompt instructions automatically
3. Evaluate on same held-out set

### BootstrapRandomSearch (future)

If MIPRO insufficient:
1. Random search over DSPy program configurations
2. Compare cost/latency/quality tradeoffs

## Extraction Output Contract

```python
@dataclass(frozen=True)
class ExtractionResult:
    """Output of extraction pipeline for one chunk."""
    source_id: str
    chunk_id: str
    evidence_path_id: str
    
    # Core output
    entities: list[TypedEntity]
    binary_relations: list[BinaryRelation]
    
    # Upgrade output
    typed_relations: list[TypedRelation]
    abstract_entities: list[AbstractEntity]
    citation_relations: list[CitationRelation]
    
    # Provenance
    extraction_ref: ExtractionRef
    
    # Safety
    safety_flags: SafetyFlags  # all false
    
    # Statistical context (for audit)
    statistical_context: StatisticalContext
```

All outputs are `CandidatePacket` — NOT graph truth. Must pass review gate (Layer 5).

## Implementation Phasing

| Phase | What | Scope |
|---|---|---|
| **Phase 2a** | Core extraction (NER + binary relations) | 5 papers, MiniMax only |
| **Phase 2b** | Upgrade modes (type, abstract, citation) | Same 5 papers |
| **Phase 2c** | DSPy BootstrapFewShot optimization | 10 labeled chunks |
| **Phase 3** | Rate limit integration + provider routing | MiniMax + GLM |
| **Phase 4** | Full pipeline with 3-lane scheduler | 10→20→week validation |
