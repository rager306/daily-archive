# Typed Knowledge Schema Design (M101 S02)

## Overview

Adapts Agents-K1 Modules A-E to daily-archive, incorporating quant-mind patterns (TreeKnowledge, KnowledgeCard, typed provenance). Designed for FalkorDB typed edges and universal domain support.

## Module A — Factual / Meta (Source Registry)

Universal: works for any source type.

| Entity Type | Fields | Paper example | Textbook example | Code repo example |
|---|---|---|---|---|
| **Source** | `source_id`, `source_type`, `title`, `uri`, `content_hash`, `fetched_at`, `license` | arXiv paper | GNN textbook chapter | GitHub repo |
| **Author** | `author_id`, `name`, `orcid`, `affiliation`, `order` | Paper authors | Textbook authors | Repo contributors |
| **Venue** | `venue_id`, `name`, `type`, `peer_reviewed` | Conference/Journal | Publisher | Registry (npm/pypi) |
| **Resource** | `resource_id`, `kind` (repo/model/dataset), `uri`, `version`, `hash` | GitHub repo link | Exercise solutions | Package reference |

**Stable ID**: `source:{kind}:{content_hash[:16]}` — SHA256-based, domain-agnostic.

**Quant-mind adaptation**: `SourceRef` expanded from quant-mind pattern:
```python
class SourceRef:
    kind: Literal["arxiv", "http", "doi", "local", "textbook", "code", "dataset", "manual"]
    uri: str | None
    fetched_at: datetime | None
    content_hash: str | None  # SHA256, dedup key
```

## Module B — Textually Mentioned (Entity Graph)

Paper-specific entities + universal extensions.

### Paper Domain Entities

| Entity Type | Description | YAKE pre-filter | LLM extraction |
|---|---|---|---|
| **Method** | Algorithm, technique, approach | keyword match → candidate | Classify + canonical name |
| **Dataset** | Dataset + split + modality | keyword match → candidate | Classify + version + URL |
| **Metric** | Evaluation metric (name, acronym, formula) | keyword match → candidate | Classify + formula extraction |
| **Task** | Research task (classification, generation, etc.) | keyword match → candidate | Classify + scope |
| **Baseline** | Comparison method | citation graph co-occurrence | Classify + relation to Method |
| **Implementation** | Hardware, VRAM, batch_size, LR, epochs | regex/TF-IDF section detection | Extract structured fields |
| **Theorem** | Mathematical theorem | section type "Theorem" | Extract statement + dependencies |
| **Definition** | Formal definition | section type "Definition" | Extract term + definition |
| **Figure** | Figure with caption | asset registry lookup | Classify figure type |
| **Table** | Table with structure | asset registry lookup | Extract row/column semantics |
| **Equation** | Mathematical equation | LaTeX detection | Extract variables + relationships |

### Universal Domain Entities (non-paper)

| Entity Type | Textbook example | Code repo example |
|---|---|---|
| **Concept** | "Graph Neural Network" | "Dependency Injection" |
| **Example** | Worked example with solution | Usage example in README |
| **Exercise** | Problem statement + solution hint | Coding challenge |
| **CodeComponent** | Pseudocode block | Function/class/module |
| **API** | Interface definition | REST endpoint signature |
| **Configuration** | Hyperparameter table | Config file entry |

**Stable ID**: `entity:{source_id}:{entity_type}:{canonical_name_slug}`

**Statistical-first (ADR-024)**: YAKE keywords per chunk → entity candidates → LLM classifies.

## Module C — Implicit / Abstracted (Abstract Graph)

**KEY DIFFERENTIATOR** — this is what Agents-K1 adds over GraphRAG and what we lack.

| Entity Type | What it captures | Extraction trigger |
|---|---|---|
| **Problem** | X → Y under constraints C with assumptions A | Section type "Introduction"/"Problem" |
| **Motivation** | Why this work exists | Section type "Introduction" |
| **Gap** | What's missing in prior work | Section type "Related Work" |
| **Contribution** | What this paper adds | Section type "Introduction"/"Contributions" |
| **Hypothesis** | Testable claim | Modal verbs: "we hypothesize", "we expect" |
| **Finding** | Quantitative result with effect size | Section type "Results"/"Experiments" |
| **Mechanism** | How/why a method works | Section type "Analysis"/"Discussion" |
| **Limitation** | What doesn't work or why | Section type "Limitations" |
| **FutureWork** | Open directions | Section type "Future Work" |

**Quant-mind adaptation**: `PaperKnowledgeCard` expanded:
```python
class KnowledgeCard:
    source_id: str
    card_type: Literal["paper", "textbook_chapter", "code_module", "dataset_card"]
    summary: str                    # LLM-generated 1-paragraph summary
    methodology: str | None         # Core approach
    key_findings: list[str]         # Top 3-5 findings
    limitations: list[str]          # Known limitations
    concepts: list[str]             # Key concepts (YAKE-assisted)
    embedding_text: str             # For BGE-M3 embedding
```

**Stable ID**: `card:{source_id}` — one card per source.

## Module D — Citation Relationships

| Field | Description |
|---|---|
| `source_citation_id` | `cite:{from_source}:{to_source}` |
| `cite_type` | `strong_direct` / `weak_direct` / `indirect` / `self_cite` |
| `relation` | `support` / `contrast` / `extend` / `background` / `benchmark_against` / `reuse_partial` |
| `evidence_section` | Section index in citing paper |
| `evidence_paragraph` | Paragraph index |
| `quote` | Max 500 chars (quant-mind Citation pattern) |
| `tree_id` | TreeKnowledge ID (for node-level citation) |
| `node_id` | Specific TreeNode ID |

**Statistical-first**: citation graph from bibliography → BFS depth → relation classification by LLM.

## Module E — Knowledge Relations (25 types in 5 groups)

### Group 1: Controlled (closed, domain-neutral)

| Relation | From → To | Meaning |
|---|---|---|
| `BUILDS_ON` | Method → Method | Directly extends prior method |
| `USES_COMPONENT` | Method → Method/Tool | Uses as a sub-component |
| `ALTERNATIVE_TO` | Method → Method | Alternative approach to same problem |
| `SOLVES` | Method → Problem | Addresses this problem |
| `APPLIED_TO` | Method → Dataset/Domain | Evaluated on this dataset/domain |
| `TARGETS` | Method → Task | Designed for this task |

### Group 2: Causal (open-ended)

| Relation | From → To | Meaning |
|---|---|---|
| `CAUSES` | Factor → Outcome | Directly causes |
| `ENABLES` | Component → Capability | Makes possible |
| `INHIBITS` | Factor → Outcome | Prevents or reduces |
| `MODULATES` | Factor → Outcome | Non-binary influence |
| `CORRELATED_WITH` | Entity → Entity | Statistical correlation |

### Group 3: Internal Composition

| Relation | From → To | Meaning |
|---|---|---|
| `USES_TECHNIQUE` | Method → Technique | Employs specific technique |
| `CONSISTS_OF` | System → Component | Contains as part |
| `IMPLEMENTS` | Code → Interface/Spec | Realizes an interface |
| `COMBINES` | Method → Method | Fusion of multiple methods |
| `REQUIRES` | Method → Resource | Depends on (GPU, dataset, library) |

### Group 4: Methodological Comparison

| Relation | From → To | Meaning |
|---|---|---|
| `DERIVED_FROM` | Method → Method | Theoretical descendant |
| `DIFFERS_FROM` | Method → Method | Key differences exist |
| `HAS_LIMITATION` | Method → Limitation | Known weakness |
| `ADDRESSES_PROBLEM` | Method → Problem | Tackles (weaker than SOLVES) |
| `MOTIVATED_BY` | Work → Gap/Observation | Driving motivation |
| `HAS_PROPERTY` | Method → Property | Characteristic (equivariance, completeness) |
| `SUBSET_OF` | Concept → Concept | Hierarchical relationship |

### Group 5: Citation Layer (argumentative)

| Relation | From → To | Meaning |
|---|---|---|
| `CITES` | Source → Source | Bibliographic reference |
| `SUPPORTS` | Source → Claim/Finding | Provides evidence for |
| `CONTRASTS` | Source → Method/Finding | Disagrees with |
| `EXTENDS` | Source → Method | Generalizes or improves |

**Total: 27 typed relations** (expanded from Agents-K1's 25).

### Relation Extraction Strategy (Core-then-Modes)

| Stage | What | LLM calls | Statistical pre-processing |
|---|---|---|---|
| **Core: Binary** | "Is there a relation between A and B?" | 2/chunk | Co-occurrence matrix from YAKE |
| **Projection** | Deterministic: binary view, provenance view | 0 | Graph projection |
| **Upgrade: Type** | "What type of relation?" (classify into 5 groups) | 1/chunk | Citation graph structure |
| **Upgrade: Causal** | Causal group classification (CAUSES/ENABLES/INHIBITS) | 1/chunk | Correlation hints from embeddings |
| **Upgrade: Citation** | Citation relation (SUPPORTS/CONTRASTS/EXTENDS) | 1/ref | BFS depth + co-citation |

## Stable ID Strategy

All IDs are SHA256-based and domain-agnostic:

```text
source:    source:{kind}:{sha256[:16]}
entity:    entity:{source_id}:{type}:{name_slug}
relation:  rel:{from_entity_id}:{RELATION_TYPE}:{to_entity_id}
card:      card:{source_id}
evidence:  ev:{source_id}:{chunk_id}:{span_hash[:16]}
extraction: ext:{source_id}:{flow}:{model}:{timestamp}
```

**Cross-source join**: entities from different sources with the same `name_slug` and `type` are candidates for merging via identity resolution (Module O1: Seed Resolution).

## Domain Profiles

| Profile | Parser | Entity types | Relation focus | Example |
|---|---|---|---|---|
| **paper** | Marker/GROBID/arxiv2md | Method, Dataset, Metric, Task, Baseline, Theorem, Figure, Table, Equation + Module C abstracts | All 27 relations | arXiv papers |
| **textbook** | HTML parser (BeautifulSoup) | Concept, Definition, Example, Exercise, Theorem | CONSISTS_OF, SUBSET_OF, DERIVED_FROM, MOTIVATED_BY | GNN textbook |
| **code_repo** | Git clone + AST | CodeComponent, API, Configuration, Resource | IMPLEMENTS, USES_COMPONENT, REQUIRES, COMBINES | GitHub repos |
| **dataset** | Metadata parser | Dataset (with schema, size, license) | APPLIED_TO, REQUIRES | HuggingFace datasets |
| **tech_doc** | Markdown/HTML parser | API, Configuration, Concept | IMPLEMENTS, CONSISTS_OF | ADRs, RFCs |

Each domain profile defines:
- Which entity types to extract
- Which relation types to focus on
- Which section types to classify
- Which statistical pre-processing to apply

## FalkorDB Edge Schema

```cypher
// Nodes
(:Source {source_id, source_type, title, ...})
(:Entity {entity_id, entity_type, canonical_name, ...})
(:Abstract {abstract_id, abstract_type, ...})
(:Evidence {evidence_id, source_id, chunk_id, ...})

// Edges (typed relations)
(:Entity)-[:BUILDS_ON {confidence, extraction_id}]->(:Entity)
(:Entity)-[:CAUSES {confidence, extraction_id}]->(:Entity)
(:Source)-[:SUPPORTS {quote, evidence_section}]->(:Abstract)

// Vector index
(:Entity {embedding: vector[1024]})
```

## Quant-Mind Pattern Integration

| Pattern | Where in schema | How |
|---|---|---|
| **TreeKnowledge** | Module A: Source | PageIndex tree with summary per node |
| **PaperKnowledgeCard** | Module C: KnowledgeCard | Typed summary card linked to source |
| **FlattenKnowledge** | Module C: KnowledgeCard | Universal card for any source type |
| **SourceRef** | Module A: Source | Extended with textbook/code/dataset kinds |
| **ExtractionRef** | Module E: Extraction | flow + model + prompt_hash per extraction |
| **Citation** | Module D: Citation | quote (max 500), tree_id, node_id |
| **Typed resolver** | Stable IDs | SHA256-based, cross-source joinable |
