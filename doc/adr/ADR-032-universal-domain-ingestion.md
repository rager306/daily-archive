# ADR-032: Universal Domain Ingestion

**Status:** Accepted (binding)  
**Date:** 2026-06-18  
**Deciders:** collaborative  
**Milestone:** M101-f5jip0 S06  
**Scope:** universal-kb / parser / extraction / sources  
**Binding Level:** binding  
**Revisable:** yes, with implementation evidence per domain

## 0. One-line Decision

> daily-archive will support 5 domain profiles (paper, textbook, code_repo, dataset, tech_doc) through the same 7-layer pipeline with domain-specific parser/extraction configurations. Non-paper sources (HTML textbooks, code repos, datasets) produce the same ParsedArticle contract and flow through the same typed extraction + FalkorDB graph. First non-paper domain: GNN textbook.

## 1. Context

ADR-023 defines 7-layer architecture. ADR-028 defines typed schema with domain profiles. Current pipeline handles papers only. Quant-mind fetch/format/flow separation supports domain-agnostic processing.

## 2. Decision

### 2.1 Five Domain Profiles

| Profile | Parser | Entity focus | First reference |
|---|---|---|---|
| paper | Marker/GROBID (existing) | Method, Dataset, Metric, Task + Module C | arXiv corpus |
| textbook | HTMLParser (NEW) | Concept, Definition, Example, Exercise | GNN textbook |
| code_repo | CodeRepoParser (NEW) | CodeComponent, API, Configuration | GitHub repos |
| dataset | DatasetMetadataParser (NEW) | Dataset schema, Metrics | HuggingFace |
| tech_doc | MarkdownParser (NEW) | API, Configuration, Concept | Internal ADRs |

### 2.2 Universal Source Registry

All sources register in one catalog with SHA256-based stable IDs. Cross-domain links (CITES, HAS_RESOURCE, IMPLEMENTS, USES_DATASET) connect different source types.

### 2.3 Parser Extensions

- HTMLParser: BeautifulSoup/readability → ParsedArticle (for textbooks, tech docs)
- CodeRepoParser: Git clone → file tree → AST → ParsedArticle (for code repos)
- DatasetMetadataParser: JSON/YAML/README → DatasetCard (for datasets)

All output the same ParsedArticle contract. Domain profile determines extraction behavior.

### 2.4 GNN Textbook as First Non-Paper Domain

Reference: https://anvithpothula.github.io/graph-neural-networks-textbook/

Implementation: fetch HTML → parse to chapters → build ChapterTree (TreeKnowledge) → extract concepts/definitions → cross-link with paper methods.

### 2.5 Quant-Mind Patterns

- Fetch/format/flow separation: already in architecture
- TreeKnowledge: ChapterTree for textbooks
- KnowledgeCard: universal card per domain with different fields
- Typed SourceRef: kind expanded for textbook/code/dataset

## 3. Phasing

Phase 5: universal ingestion (after Phase 2-4: typed schema + extraction + FalkorDB + validation)

## 4. LLM Reading Notes

- **Binding**: 5 domain profiles through same pipeline.
- **First non-paper**: GNN textbook (HTML parsing).
- **Same safety gates**: all domains go through fail-closed review.
- **Cross-domain linking**: papers ↔ code ↔ datasets ↔ textbooks.
- **Not authorized**: graph writes, production imports without review.
