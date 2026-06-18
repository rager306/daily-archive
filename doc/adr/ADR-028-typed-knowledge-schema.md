# ADR-028: Typed Knowledge Schema

**Status:** Accepted (binding)  
**Date:** 2026-06-18  
**Deciders:** collaborative  
**Milestone:** M101-f5jip0 S02  
**Scope:** schema / graphdb / extraction / universal-kb  
**Binding Level:** binding  
**Revisable:** yes, with extraction evidence and FalkorDB schema validation

## 0. One-line Decision

> daily-archive will implement a typed knowledge schema with 5 modules (A: factual/meta, B: textually mentioned, C: implicit/abstracted, D: citation relationships, E: knowledge relations) covering 27 typed relation types in 5 groups, adapted from Agents-K1 with quant-mind patterns (TreeKnowledge, KnowledgeCard, typed provenance), supporting both paper and non-paper domains.

## 1. Context

M069 schema-diff identified 3 gaps vs Agents-K1: implicit abstractions (Module C), typed relations (Module E), and evaluation evidence. Current daily-archive has flat `ScientificEntity` with 5 relation types: `supports, contradicts, mentions, uses, extends`.

Agents-K1 defines 25 relation types in 5 groups. We add 2 universal relations (`CONSISTS_OF`, `SUBSET_OF` from quant-mind TreeKnowledge) for hierarchical knowledge.

Quant-mind provides patterns for TreeKnowledge (navigable hierarchy), PaperKnowledgeCard (distilled summary), and typed provenance (SourceRef, ExtractionRef, Citation).

## 2. Decision

### 2.1 Five-Module Schema

| Module | Content | Entity count | Status |
|---|---|---|---|
| A | Source, Author, Venue, Resource | 4 types | Partially exists |
| B | Method, Dataset, Metric, Task, Baseline, Implementation, Theorem, Definition, Figure, Table, Equation + universal: Concept, Example, Exercise, CodeComponent, API, Configuration | 17 types | Needs typed extraction |
| C | Problem, Motivation, Gap, Contribution, Hypothesis, Finding, Mechanism, Limitation, FutureWork | 9 types | **NEW** — key differentiator |
| D | Citation with cite_type, relation, evidence, quote | 1 structured type | Partially exists |
| E | 27 typed relations in 5 groups | 27 edge types | **NEW** — key upgrade |

### 2.2 Relation Taxonomy (27 types)

- **Controlled** (6): BUILDS_ON, USES_COMPONENT, ALTERNATIVE_TO, SOLVES, APPLIED_TO, TARGETS
- **Causal** (5): CAUSES, ENABLES, INHIBITS, MODULATES, CORRELATED_WITH
- **Composition** (5): USES_TECHNIQUE, CONSISTS_OF, IMPLEMENTS, COMBINES, REQUIRES
- **Comparison** (7): DERIVED_FROM, DIFFERS_FROM, HAS_LIMITATION, ADDRESSES_PROBLEM, MOTIVATED_BY, HAS_PROPERTY, SUBSET_OF
- **Citation** (4): CITES, SUPPORTS, CONTRASTS, EXTENDS

### 2.3 Stable ID Strategy

All IDs are SHA256-based and domain-agnostic:
- `source:{kind}:{sha256[:16]}`
- `entity:{source_id}:{type}:{name_slug}`
- `rel:{from_entity_id}:{RELATION_TYPE}:{to_entity_id}`

### 2.4 Domain Profiles

| Profile | Parser | Entity focus | Example |
|---|---|---|---|
| paper | Marker/GROBID | Method, Dataset, Metric, Task + Module C | arXiv papers |
| textbook | HTML parser | Concept, Definition, Example, Exercise | GNN textbook |
| code_repo | Git + AST | CodeComponent, API, Configuration | GitHub repos |
| dataset | Metadata | Dataset schema | HuggingFace |
| tech_doc | Markdown | API, Configuration, Concept | ADRs, RFCs |

### 2.5 Statistical-First Integration (ADR-024)

Every entity/relation extraction stage receives:
1. YAKE keyword candidates (deterministic)
2. Co-occurrence statistics (deterministic)
3. BGE-M3 embeddings (deterministic)
4. Section type classification (regex/TF-IDF, deterministic)

Before any LLM call.

### 2.6 Quant-Mind Pattern Adoption

| Pattern | Implementation |
|---|---|
| TreeKnowledge | PageIndex with summary per node |
| PaperKnowledgeCard | KnowledgeCard with methodology/findings/limitations |
| SourceRef | Extended with textbook/code/dataset kinds |
| ExtractionRef | flow + model + prompt_hash per extraction |
| Citation | quote (max 500), tree_id, node_id |

## 3. Applies To

- Extraction pipeline (Layer 3)
- FalkorDB schema (Layer 4)
- Review gates (Layer 5) — typed entity review
- Universal ingestion (S06) — domain profiles
- Graph operators (S04) — typed relation queries

## 4. Requirements and Decisions Impacted

| Requirement | Impact | Notes |
|---|---|---|
| R071 | fulfills | Typed schema (Modules A-E, 27 relations) |
| R068 | supports | Statistical-first pre-processing feeds typed entity candidates |
| R024 | supports | Staged validation needs typed graph quality metrics |

## 5. Safety

- Schema design is a specification, not authorization for graph writes
- Typed entities are CandidatePackets with safety_flags=false until reviewed
- Domain profiles do not bypass review gates
- Stable IDs enable audit trail but do not authorize import

## 6. LLM Reading Notes

- **Binding**: 5-module typed schema with 27 relation types is the target.
- **Implementation**: Phased — paper domain first, universal domains after validation.
- **Statistical-first**: YAKE → entity candidates → LLM classification.
- **FalkorDB**: Typed edges for all 27 relation types.
- **Quant-mind**: TreeKnowledge, KnowledgeCard, typed provenance adopted as patterns.
- **Not authorized**: graph writes, production imports, schema migration without FalkorDB acceptance tests.
