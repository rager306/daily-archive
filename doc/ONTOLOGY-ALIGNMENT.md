# Ontology Alignment Design

**Status:** Binding (D133)
**Date:** 2026-07-26
**Studies:** SPAR/FaBiO, CiTO, BIBO, OpenAlex/SemOpenAlex, S2ORC, PubMed KG, KG20C

This document maps daily-archive v2 to established scholarly ontologies and
datasets. The graph schema uses a **three-layer hybrid architecture**.

---

## Why not invent from scratch?

The previous schema (GRAPH-SCHEMA.md v1) invented Paper/Section/Keyword/Topic/
Category/Entity from scratch. This violated a basic principle: **FaBiO, CiTO,
and OpenAlex already define these types professionally**, with decades of
domain expertise and billions of data points behind them.

| Our ad-hoc name | Established equivalent | Source |
|------------------|----------------------|--------|
| Paper | `fabio:Article` / `fabio:Preprint` | FaBiO |
| Section | `fabio:DocumentObject` (part) | FaBiO |
| Keyword | `skos:Concept` (keyword form) | SKOS / OpenAlex |
| Topic | `fabio:Subject` / OpenAlex Concept | FaBiO / OpenAlex |
| Category | OpenAlex Topic / arXiv category | OpenAlex |
| Author | `foaf:Person` + `pro:author` role | FOAF / PRO / OpenAlex |
| Citation | `fabio:BibliographicReference` | FaBiO |
| CITES edge | `cito:cites` (+ 60 typed variants) | CiTO |
| Entity (Method, etc.) | domain-specific (not in FaBiO) | ADR-028 |

---

## Three-layer architecture

```
┌──────────────────────────────────────────────────────┐
│ Layer 3: CONTENT (domain extraction)                 │
│   Source: GROBID full-text → rule/GLiNER extractor    │
│   Nodes: Entity (Method, Dataset, Metric, Task, ...)  │
│   Edges: 27 extracted relation types (ADR-028/CiTO)   │
├──────────────────────────────────────────────────────┤
│ Layer 2: STRUCTURE (document structure)               │
│   Source: GROBID TEI / S2ORC                          │
│   Nodes: Section, Figure, Table, Equation             │
│   Edges: hasPart / isPartOf (FaBiO)                   │
├──────────────────────────────────────────────────────┤
│ Layer 1: METADATA (curated bibliographic)             │
│   Source: OpenAlex API / arXiv metadata               │
│   Nodes: Work, Author, Institution, Concept, Topic    │
│   Edges: authoredBy, hasConcept, cites (CiTO)         │
└──────────────────────────────────────────────────────┘
```

### Layer 1: Metadata (OpenAlex backbone)

OpenAlex provides curated, disambiguated data that is **impossible to match**
with local extraction:
- **Works** (322M): title, abstract, DOI, publication date, type, OA status,
  concepts, topics, citation counts
- **Authors** (100M): name, ORCID, institutions, works count
- **Institutions**: name, country, ROR, type
- **Concepts**: hierarchical (levels 0–4), Wikidata-linked, usage counts
- **Topics**: grouped concept clusters (domain → field → subfield → topic)
- **Citations**: >2B resolved citation links

**Mapping to graph nodes:**

| Graph node | OpenAlex entity | FaBiO class | Key properties |
|-----------|----------------|-------------|----------------|
| Work | `Work` | `fabio:Article` | title, doi, type, publication_date, oa_status, concepts |
| Author | `Author` | `foaf:Person` | name, orcid, works_count |
| Institution | `Institution` | `foaf:Organization` | name, country, ror |
| Concept | `Concept` | `skos:Concept` | label, level, wikidata, broader/narrower |
| Topic | `Topic` | `fabio:Subject` | label, domain, field, subfield |

**Mapping to graph edges:**

| Edge | CiTO/FaBiO property | From → To |
|------|---------------------|-----------|
| `authoredBy` | `pro:author` / `dcterms:creator` | Work → Author |
| `affiliatedWith` | `schema:affiliation` | Author → Institution |
| `hasConcept` | `dcterms:subject` | Work → Concept |
| `hasTopic` | `fabio:hasSubject` | Work → Topic |
| `cites` | `cito:cites` | Work → Work (resolved) |

### Layer 2: Structure (GROBID/S2ORC)

From the article's own full text, parsed by GROBID or S2ORC:

| Graph node | FaBiO class | Source |
|-----------|-------------|--------|
| Section | `fabio:DocumentObject` (section) | GROBID TEI `<div><head>` |
| Figure | `fabio:Figure` | GROBID TEI `<figure>` |
| Table | `fabio:Table` | GROBID TEI `<table>` |
| Equation | `fabio:Formula` | GROBID TEI `<formula>` |

| Edge | FaBiO property | From → To |
|------|---------------|-----------|
| `hasPart` | `frbr:part` | Work → Section/Figure/Table |

### Layer 3: Content (domain extraction)

Domain-specific entities extracted from full text — NOT in FaBiO, defined
by ADR-028:

| Graph node | Entity types (22) |
|-----------|-------------------|
| Entity | Concrete: Method, Dataset, Metric, Task, Baseline, Model, Figure, Table, Equation, Concept, Implementation, Theorem, Definition |
| Entity | Abstract: Problem, Motivation, Gap, Contribution, Hypothesis, Finding, Mechanism, Limitation, FutureWork |

| Edge | CiTO/ADR-028 types |
|------|-------------------|
| Mentions | Work → Entity (paper mentions entity) |
| FoundIn | Entity → Section (entity found in section) |
| 27 relation types | Entity → Entity (BuildsOn, Solves, Supports, ...) |

CiTO citation-context types (for when an Entity is found via a citation):
`usesMethodIn`, `usesDataFrom`, `obtainsBackgroundFrom`, `agreesWith`,
`disagreesWith`, `extends`, `discusses`, `critiques`, etc.

---

## OpenAlex integration plan

### Phase A: Metadata enrichment (replaces YAKE/keyword guessing)

```bash
# Fetch Work metadata from OpenAlex by arXiv ID
da enrich --id 2602.11757 --from openalex
```

OpenAlex API: `https://api.openalex.org/works/doi:10.48550/arXiv:2602.11757`

Returns: title, abstract, authors, concepts, topics, citations, OA status.

This **replaces**:
- YAKE keyword extraction (noisy, recall-limited)
- Category guessing from GROBID
- Author parsing from TEI (imperfect)
- Citation resolution (OpenAlex has resolved citations)

### Phase B: Concept hierarchy (topic taxonomy)

OpenAlex concepts form a 5-level hierarchy (domain → field → subfield → topic).
Import this as a SKOS-like concept tree in the graph:

```
Domain: Computer Science
  └─ Field: Artificial Intelligence
       └─ Subfield: Natural Language Processing
            └─ Topic: Prompt Optimization
```

### Phase C: Citation graph bootstrap

OpenAlex provides resolved citation links. For a Work, `referenced_works`
gives DOI-identified citations. This bootstraps the citation graph without
parsing reference lists from PDFs.

---

## What we keep from ad-hoc extraction

- **Sections** — GROBID parses these well; OpenAlex doesn't provide section structure
- **Entities** (Method, Dataset, etc.) — domain-specific, not in OpenAlex
- **Relations between entities** — domain-specific, requires full-text extraction

## What we REPLACE with OpenAlex

- ~~YAKE keywords~~ → OpenAlex concepts/topics
- ~~arXiv category guessing~~ → OpenAlex topics + concepts
- ~~Author parsing from TEI~~ → OpenAlex authors (disambiguated, with ORCID)
- ~~Citation resolution from GROBID refs~~ → OpenAlex `referenced_works`
- ~~Topic extraction~~ → OpenAlex concept hierarchy

---

## Revised node type naming

To align with FaBiO while keeping Rust readability:

| Old name | New label | FaBiO equivalent | Source layer |
|----------|-----------|-----------------|-------------|
| Paper | `Work` | `fabio:Article` | L1: OpenAlex |
| Section | `Section` | `fabio:DocumentObject` | L2: GROBID |
| Keyword | ~~removed~~ | replaced by Concept | L1: OpenAlex |
| Topic | `Concept` | `skos:Concept` | L1: OpenAlex |
| Category | `Topic` | `fabio:Subject` | L1: OpenAlex |
| Author | `Author` | `foaf:Person` | L1: OpenAlex |
| Citation | `Reference` | `fabio:BibliographicReference` | L1/L2 |
| Entity | `Entity` | domain-specific | L3: extraction |

**Note:** `Work` is more general than `Paper` — supports papers, preprints,
textbooks, code repos (legacy SOURCE_KINDS). The `work_type` property
distinguishes them.
