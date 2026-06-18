# Universal Domain Ingestion Design (M101 S06)

## Overview

Extends the paper-focused pipeline to support any knowledge domain. Inspired by quant-mind's fetch/format/flow separation and domain-agnostic knowledge structures. Uses GNN textbook as concrete reference implementation.

## Core Principle

> The same 7-layer pipeline (Source → Parser → Structure → Extraction → Graph → Review → Agents) processes all domains. Each domain has a **profile** that configures: parser, entity types, relation focus, and extraction prompts.

## Domain Profiles

### Profile: `paper` (existing, primary domain)

| Aspect | Configuration |
|---|---|
| Source types | arXiv PDF, Semantic Scholar metadata |
| Parser | Marker/GROBID/arxiv2md (ADR-008/009) |
| Structure | PageIndex (sections), SemanticChunks, figures/tables/equations |
| Entity types (Module B) | Method, Dataset, Metric, Task, Baseline, Implementation, Theorem, Definition, Figure, Table, Equation |
| Abstract types (Module C) | Problem, Motivation, Gap, Contribution, Hypothesis, Finding, Mechanism, Limitation, FutureWork |
| Relation focus | All 27 types (full taxonomy) |
| KnowledgeCard | summary, methodology, key_findings, limitations |
| Statistical pre-processing | YAKE keywords, TF-IDF section classification, BGE-M3 embeddings |
| Example | arXiv:2605.18747 and 166 refs (M056 corpus) |

### Profile: `textbook` (NEW — GNN textbook first)

| Aspect | Configuration |
|---|---|
| Source types | HTML pages, PDF chapters |
| Parser | **NEW: HTMLParser** (BeautifulSoup/readability → ParsedArticle) |
| Structure | ChapterTree (tree of chapters/sections/subsections), CodeBlocks, Exercises |
| Entity types (Module B) | Concept, Definition, Example, Exercise, Theorem, CodeBlock |
| Abstract types (Module C) | LearningGoal, Prerequisite, KnowledgeGap |
| Relation focus | CONSISTS_OF, SUBSET_OF, DERIVED_FROM, MOTIVATED_BY, REQUIRES |
| KnowledgeCard | chapter_summary, key_concepts, prerequisites, exercises_count |
| Statistical pre-processing | YAKE per chapter, heading hierarchy analysis, code block detection |
| Reference | https://anvithpothula.github.io/graph-neural-networks-textbook/ |

### Profile: `code_repo` (NEW)

| Aspect | Configuration |
|---|---|
| Source types | Git clone, README, docstrings, type hints |
| Parser | **NEW: CodeRepoParser** (Git clone → file tree → AST per file → ParsedArticle) |
| Structure | ModuleTree (packages/modules/classes/functions), DependencyGraph |
| Entity types (Module B) | CodeComponent (function/class/module), API (endpoint/interface), Configuration |
| Abstract types (Module C) | DesignPattern, ArchitectureDecision, KnownIssue |
| Relation focus | IMPLEMENTS, USES_COMPONENT, COMBINES, REQUIRES, CONSISTS_OF |
| KnowledgeCard | repo_summary, main_components, dependencies, api_surface |
| Statistical pre-processing | AST analysis, import graph, call graph complexity metrics |
| Reference | GitHub repos linked from papers |

### Profile: `dataset` (NEW)

| Aspect | Configuration |
|---|---|
| Source types | HuggingFace dataset card, Papers With Code, README |
| Parser | **NEW: DatasetMetadataParser** (JSON/YAML/README → DatasetCard) |
| Structure | DatasetSchema (columns, types, splits, size, license) |
| Entity types (Module B) | Dataset (with schema fields), Metric (evaluation metrics) |
| Abstract types (Module C) | DatasetBias, DataQuality, CoverageGap |
| Relation focus | APPLIED_TO, REQUIRES, HAS_PROPERTY |
| KnowledgeCard | dataset_summary, schema, size, license, evaluation_metrics |
| Statistical pre-processing | Schema analysis, size estimation, license detection |
| Reference | HuggingFace datasets linked from papers |

### Profile: `tech_doc` (NEW)

| Aspect | Configuration |
|---|---|
| Source types | Markdown (ADR, RFC, spec), HTML docs |
| Parser | **NEW: MarkdownParser** (markdown → ParsedArticle with heading hierarchy) |
| Structure | DocTree (sections/subsections), CodeBlocks, Diagrams |
| Entity types (Module B) | API, Configuration, Concept, CodeBlock |
| Abstract types (Module C) | DesignRationale, Tradeoff, Constraint |
| Relation focus | IMPLEMENTS, CONSISTS_OF, MOTIVATED_BY |
| KnowledgeCard | doc_summary, key_decisions, constraints |
| Statistical pre-processing | Heading hierarchy, code block detection, link analysis |
| Reference | This project's own ADRs and docs |

## Universal Source Registry

All sources, regardless of domain, register in the same catalog:

```python
@dataclass(frozen=True)
class UniversalSourceRecord:
    """Universal source registration — works for any domain."""
    source_id: str              # SHA256-based stable ID
    source_type: str            # paper|textbook|code_repo|dataset|tech_doc
    domain_profile: str         # profile name for extraction
    title: str
    uri: str                    # canonical URI
    content_hash: str           # SHA256 of fetched content
    fetched_at: datetime
    license: str | None
    language: str               # en, ru, etc.
    parser_used: str            # which parser processed this
    status: str                 # registered|parsed|structured|extracted|reviewed|imported
```

### Cross-domain linking

Sources from different domains can be linked:

```text
Paper → [CITES] → Paper          (citation graph)
Paper → [HAS_RESOURCE] → CodeRepo (paper links to GitHub)
Paper → [USES_DATASET] → Dataset  (paper uses HuggingFace dataset)
Textbook → [COVERS_CONCEPT] → Concept (textbook chapter covers a concept)
CodeRepo → [IMPLEMENTS] → Method   (code implements a method from paper)
TechDoc → [MOTIVATED_BY] → Gap     (ADR motivated by identified gap)
```

These cross-domain links create a **universal knowledge graph** where:
- Papers cite papers (citation layer)
- Code implements paper methods (implementation layer)
- Datasets are used by papers (evaluation layer)
- Textbooks explain concepts (educational layer)
- Tech docs document decisions (governance layer)

## GNN Textbook Reference Implementation

### Step 1: Fetch

```python
# Fetch HTML pages from textbook website
source = UniversalSourceRecord(
    source_type="textbook",
    domain_profile="textbook",
    title="Graph Neural Networks: Foundations and Frontiers",
    uri="https://anvithpothula.github.io/graph-neural-networks-textbook/",
    ...
)
```

### Step 2: Parse

```python
# NEW: HTMLParser
parser = HTMLTextbookParser()
parsed = parser.parse(source.uri)
# Output: ParsedArticle with chapters as sections, code blocks, exercises
```

### Step 3: Structure

```python
# Build ChapterTree (TreeKnowledge pattern)
tree = build_chapter_tree(parsed)
# Each chapter node has: title, summary (TF-IDF), content, children
```

### Step 4: Extract (textbook profile)

```python
# Statistical pre-processing
yake_keywords = keyword_extractor.extract_for_page_index(tree)
section_types = classify_sections(parsed)  # "Introduction", "Method", "Example", etc.

# Core extraction with textbook profile
entities = extract_entities(
    chunk=chunk,
    statistical_context=StatisticalContext(keywords=yake_keywords, ...),
    domain_profile="textbook",  # → extract Concept, Definition, Example, Exercise
)
```

### Step 5: Cross-link

```python
# Link textbook concepts to paper methods
# e.g., textbook chapter "Message Passing Networks" → paper "GAT" (USES_TECHNIQUE)
cross_links = cross_domain_linker.link(
    textbook_concepts=entities,
    paper_graph=falkordb_query("Method WHERE canonical_name CONTAINS 'message passing'"),
)
```

## Parser Architecture (extending ADR-008)

```text
Universal Parser Router
    ├── PDFParser (existing: Marker/GROBID/arxiv2md/PyMuPDF)
    │     └── domain_profile: paper
    ├── HTMLParser (NEW: BeautifulSoup/readability)
    │     └── domain_profile: textbook, tech_doc
    ├── CodeRepoParser (NEW: Git clone → AST)
    │     └── domain_profile: code_repo
    └── DatasetMetadataParser (NEW: JSON/YAML/README)
          └── domain_profile: dataset
```

Each parser outputs the same `ParsedArticle` contract. Domain profile determines which entity types and relations to extract.

## Quant-Mind Pattern Integration

| Pattern | Universal ingestion adaptation |
|---|---|
| **Fetch-format-flow** | Fetch (universal source registry) → Format (domain-specific parser) → Flow (extraction pipeline) |
| **TreeKnowledge** | ChapterTree for textbooks, ModuleTree for code, DocTree for tech docs |
| **PaperKnowledgeCard** | Universal KnowledgeCard per domain: different fields per profile |
| **Typed SourceRef** | `kind` expanded: arxiv, http, doi, local, textbook, code, dataset, manual |
| **Bounded batch** | Per-domain batch limits in queue (e.g., max 5 textbook chapters per batch) |

## Implementation Phasing

| Phase | What | Scope |
|---|---|---|
| **Phase 5a** | HTMLParser for textbooks | GNN textbook only |
| **Phase 5b** | Textbook extraction profile | Concepts, definitions, examples |
| **Phase 5c** | Cross-domain linking | Textbook ↔ papers |
| **Phase 5d** | CodeRepoParser | GitHub repos from paper resources |
| **Phase 5e** | DatasetMetadataParser | HuggingFace datasets |
| **Phase 5f** | TechDocParser | Internal ADRs and RFCs |

## Safety

- All non-paper sources go through the same fail-closed review gates
- Cross-domain links are CandidatePackets, not graph truth
- Parser adaptations do not bypass evidence provenance requirements
- No network access during extraction (only during bounded fetch phase)
