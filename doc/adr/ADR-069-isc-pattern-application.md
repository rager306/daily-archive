# ADR-069: ISC Pattern Application — Evidence Gates, Compilation Depth, Genre Detection

- Status: Accepted
- Date: 2026-08-24
- Deciders: repo owner + agent (M451)
- Tags: extraction, evidence, retrieval, genre-detection, quality-gates, data-driven
- Source paper: arXiv 2608.20845 (ingest-time semantic compilation, ISC)

## 0. One-line Decision

**Adopt the ISC evidence discipline: every extracted entity must carry mechanically verifiable provenance (exact-quote gate) before any LLM judges it; compilation depth becomes a per-document choice (R\*); and document genre (IMRaD vs position/vision paper) becomes a first-class DATA-driven detection that switches extraction strategy.**

## 1. Context

The paper 2608.20845 ("Ingest-time Semantic Compilation") is itself a design
document for our system class: it compiles corpus meaning into validated
claims at write time instead of re-interpreting raw text at every query.
Studying it surfaced two categories of patterns:

**(a) Substrate design patterns** — how a compiled semantic layer should
carry evidence, idempotency, lineage, and cost-aware depth.

**(b) Corpus parse patterns** — the paper as a *specimen* revealed gaps in
our pipeline: it is a position/vision paper (essayistic headings, no IMRaD),
GROBID merged headings, benchmark numbers live in prose, the key method term
lives only in a table, citation indices exceed the parsed bibliography, and
the acronym definition "(ISC)" was lost from parsed text while surviving in
the grounded LLM summary (already exploited: cross-layer extraction).

## 2. Decision

### 2.1 Exact-quote evidence gate (mechanical, write-time)

Every extracted entity gains an evidence record, verified by **string
comparison against the canonical section text before any LLM critic runs**:

```json
{
  "label": "ISC",
  "entity_type": "Method",
  "evidence": {
    "verbatim_quote": "ingest-time semantic compilation (ISC)",
    "char_start": 143, "char_end": 180,
    "section_ordinal": 0,
    "location_method": "acronym_definition|whitelist|structural"
  },
  "pipeline_run_id": "<extractor-fingerprint>",
  "content_hash": "sha256(paper_id:label:entity_type)"
}
```

Rules:
- The quote MUST be located in the canonical section text (span check).
- Unverifiable candidates are marked `evidence_state: unverified`, never
  silently dropped — the critic sees them with reduced trust.
- LLM critic only adjudicates entities that passed the mechanical gate
  (cost down, precision up). Hallucinated compilation is mechanically
  rejectable at write time — "provenance is not an annotation, it is its
  foreign keys".

### 2.2 Idempotency and lineage (paper's UNIQUE + pipeline_run_id)

- Entity dedup key becomes `sha256(paper_id:label:entity_type)`
  (content-hash), replacing plain lowercase-label comparison —
  recompilation is stable across runs.
- `pipeline_run_id` = the already-computed `ExtractionCacheIdentity`
  fingerprints (implementation / configuration / output) stamped into
  entities.json, giving every fact full extractor lineage.

### 2.3 Compilation depth as data (R\* planner)

The FSM stages stay uniform, but **expensive stages become depth-flagged**:
`summarized` (LLM ~10s) and future `enriched` runs record a
`compilation_depth` hint. Lazy per-section summaries (background_tasks)
already implement deferral; a read-frequency signal (query logs) will feed
R\* = "compile only where reads repay it". No doctrinal all-or-nothing.

### 2.4 Genre detection: IMRaD vs position/vision (DATA)

`data/ontology/structural_type_detectors.yaml` gains genre-level detectors:
`research_agenda` ("The Research Agenda", "Roadmap", "Future Directions"),
`existence_proofs` ("Two Existence Proofs", "Existence Proof"), `vision`
("The Vision", "A Manifesto", "Position Paper"). When a paper's heading
distribution is essayistic (most sections unclassified) and genre markers
fire, the extractor switches strategy: global structural passes carry the
extraction load (already true), and genre is recorded in the domain
MANIFEST coverage stats.

### 2.5 Parse-integrity signals (from the specimen findings)

New mechanical checks on every parsed article (quality signals, not gates):
- **merged_headings**: heading text matching `<title> \d+\.\d* <subtitle>`
  is flagged; navigation may split it into parent+child nodes.
- **citation_index_overflow**: body references `[N]` with N ≥ citation
  count → GROBID bibliography truncation; feeds citation-resolution trust.
- **prose_benchmark_numbers**: percentages / p-values / effect sizes in
  section text outside any Results-classified section → harvested as
  `MetricObservation` candidates via numeric regex patterns (DATA).
- **table_only_terms**: entity candidates found in table cells but absent
  from body text (e.g. "Procrustes alignment") — table cells join the
  extraction text surface.

## 3. Consequences

- entities.json schema grows (evidence, hashes, run id) — additive, old
  files remain valid (missing fields = `unverified`).
- The critic (glm-5.2) consumes evidence_state; `repair_exhausted` now has
  a cheaper mechanical predecessor.
- Domain MANIFESTs gain genre coverage columns (how many vision papers,
  how many IMRaD) — corpus heterogeneity becomes visible.
- Retrieval roadmap: claim-level retrievable unit (entity + evidence span +
  summary fragment, pinned to CVF epoch) — "queries against yesterday's
  meaning" become reproducible. Context budgets in reader prompts follow
  the paper's non-monotonicity finding (more context reduced accuracy).

## 4. What we do NOT adopt

- Full ISC substrate DDL (facts/fact_evidence tables) — our Samyama graph
  + article artifacts already provide the layer; we adopt the *evidence
  discipline*, not the storage engine.
- Doctrinal "compile everything" NOR "lazy wins" — the paper itself says
  the cost model decides per corpus; we keep per-stage depth hints.

## 5. References

- Source specimen: `data/processed/2608.20845/` (17 sections, 15 citations,
  6 extracted entities after structural patterns; genre = position/vision).
- Cross-layer extraction already shipped (commit 734d56c): summary-layer
  acronym definitions merged into entities.
- ADR-051 testing strategy: candidate ≠ authoritative (same principle).
- ADR-068: SCHEMA.yaml/MANIFEST.yaml hierarchy — genre detectors and
  numeric patterns live in the same DATA registries.
