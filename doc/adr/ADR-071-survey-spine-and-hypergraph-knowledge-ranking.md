# ADR-071: Survey Spine Extraction and Hypergraph Knowledge Ranking

- Status: Accepted
- Date: 2026-08-24
- Deciders: repo owner + agent (M451)
- Tags: extraction, ontology, graph-structure, retrieval-ranking, survey-genre, evidence
- Source specimen: arXiv 2512.00107 (survey of 39 hypergraph centrality measures, econophysics, cross-domain physics+economics)

## 0. One-line Decision

**Surveys carry a closed numbered spine (M1..M39) that must be extracted as a complete index (one regex = 100% recall); dataset names live in table cells and enter the extraction surface as typed pseudo-sections; and the article's own subject — centrality over hypergraphs — becomes the ranking frame for our knowledge graph: degree (popularity), walk/eigen (influence), contextual (query-relative), over hyperedges rather than pairwise edges.**

## 1. Context

Studying 2512.00107 as both specimen and source produced three classes of
findings: extraction-recall patterns (numbered environments, table-only
datasets), context patterns (taxonomy structure, evaluation pairs), and
retrieval patterns (the article's own centrality taxonomy applied to our
KG). The user directed: conclusions for ontology and graph structure must
be recorded and grounded in the article.

## 2. Decision

### 2.1 Numbered environments = closed survey spine (shipped)

`structural.numbered_environments` (DATA, extraction_patterns.yaml):
prefixes `[M]`, title form `M3. Isolating centrality.`, bare-title form
`M21.` + first-sentence name, inline form in full section text. Live
proof: **39/39** measure names extracted (was ~20 by frequency). Prefix
set is DATA — extend with `D` (definition), `T` (theorem) when
mathematics profiles consume this pass.

### 2.2 Table-cells enter the extraction surface (shipped)

Tables become pseudo-sections; a header row containing "Dataset" titles
the pseudo-section `Datasets (Table N)` so the Dataset section arm types
entities correctly. Acronym-definition pass fires in the Dataset arm;
hyphenated compounds count per part (`senate-bills` → S,B = SB; word
minimum satisfied). Live proof: SB, HB, TAU discovered from Table 2 —
names that appear NOWHERE in body whitelist.

### 2.3 Ontology conclusions (normative, grounded in the specimen)

1. **Taxonomy nodes are first-class.** The specimen organizes 39 measures
   into structural/functional/contextual families and subfamilies
   (degree-based, path-based, walk-based, game-theoretic, ML-based).
   Domain SCHEMA.yaml gains an optional `taxonomy_nodes` section
   (category → member entity types/labels). Retrieval query class:
   "all walk-based measures" — impossible over flat entity lists.
2. **Evaluation pairs are edges, not entities.** The empirical section
   pairs measures × datasets (similarity, computation time). Ontology
   gains an `EVALUATED_ON` edge type: (Method)-[EVALUATED_ON]->(Dataset)
   with properties {metric, value, source_span}. Prose/table
   co-occurrence is the candidate producer; evidence gate verifies.
3. **Artifact is a missing entity type.** "source code and datasets at
   https://github.com/…" — reproducibility links are extractable facts.
   Add `Artifact` entity type (code, data, benchmark URLs) to the shared
   domain SCHEMA; discovery via URL regex in prose; evidence = the URL
   span itself.
4. **Citation-index overflow generalizes.** Second specimen confirms
   body refs exceeding parsed bibliography ([195] vs indices 0..194).
   Structural parse profile (no Crossref consolidation) trades DOI
   density for availability — host-side OpenAlex enrichment MUST backfill
   DOIs before citation-graph use (only 5/195 have IDs now).

### 2.4 Graph-structure conclusions (normative)

The article's three centrality families are the ranking frame for the
knowledge graph:

| Family | Specimen measures | Our graph analog | Answers |
|---|---|---|---|
| structural | degree, k-core | entity frequency across papers (already: recurring_terms) | "what is popular" |
| functional | PageRank, eigenvector, random-walk | PageRank over co-occurrence/citation edges | "what is influential" |
| contextual | task-dependent, personalized | query-relative re-ranking at read time | "what is relevant NOW" |

1. **Knowledge edges are hyperedges.** A scientific claim binds
   {paper, method, dataset, metric, value} — n-ary, not pairwise.
   Samyama stores pairwise; hyperedge semantics live as a reified
   Claim/Assertion node connecting members (consistent with ADR-042
   evidence bundles). No storage change now; modeling rule: never
   decompose a claim into pairwise edges without the claim node.
2. **Degree centrality is already shipped** (recurring_terms is degree
   counting). Walk-based ranking (PageRank over entity co-occurrence
   within papers + citation edges) is the next retrieval primitive;
   requires citation DOI backfill (2.3.4) to weight by authority.
3. **Survey-hub prior.** Surveys are entry points: citation-graph
   PageRank naturally ranks them high; retrieval should boost hub nodes
   as entry points and traverse to specifics (hub-and-spoke retrieval).

### 2.5 What we do NOT adopt

- Hypergraph-native storage (no schema migration): pairwise + reified
  claim nodes suffice at current scale.
- Arbitrary centrality zoo adoption: degree + PageRank + query-relative
  cover the three retrieval question classes; other measures wait for a
  concrete query need.

### 2.6 YAKE as the statistical recall layer (shipped, studied 2026-08-24)

YAKE (Campos et al., Information Sciences 509, 2020; MIT `yake-rust` 1.0.3,
1-1 score alignment with the reference implementation, x8-10 optimized)
becomes the untyped candidate GENERATOR under the evidence gate:

- **Five statistical features** replace hand heuristics: Casing (TCase —
  capitalized/acronym terms), Position (TPosition — definitions live in
  abstract/intro, exactly the 2608.20845 finding), normalized TF, TRel
  (co-occurrence diversity — the PRINCIPLED stopword detector: our hand
  stoplists for recurring_terms leaked through three growth iterations;
  "from/defined/capture/researchers" escaped every list; TRel penalizes
  terms with many different neighbours automatically), DifSentence.
- **Value = multi-word phrases**; single words are noise (cost/corpus/
  read ranked high in 2608.20845 because they ARE central vocabulary,
  but they are not entities): `require_multiword: true`.
- **Discipline gates learned live**: `min_doc_chars=1000` (statistical
  features unstable on short texts — unit tests surfaced PRO-methodology
  junk); substring dedup inside the batch (YAKE keeps truncated variants
  "Ingest-time semantic" next to the full phrase); summary pseudo-section
  prefix ("Summary …") junk-filtered.
- Live results: 2608.20845 +7 real phrases ("repeated query-time
  interpretation", "standard RAG", "cost model", "compiles corpus
  meaning"); table-derived fragments on IMRaD (2608.19799) remain for
  the critic layer — recall layer, not precision layer.
- Config fully DATA (extraction_patterns.yaml → structural.yake_keyphrases);
  stopword merge: yake predefined en + scientific extra_stopwords.

Layering rule (binding): **typed passes (whitelist/acronym/numbered-env/
table) own precision; YAKE owns phrase recall; the evidence gate owns
verifiability; the critic LLM owns judgment.** No layer duplicates
another's role.

## 3. Consequences

- Survey genre (ADR-070 genre detectors) + numbered spine + recurring
  terms compose: genre gate prevents noise on non-surveys (live:
  2608.20845 = 15 entities, 2608.19799 = 32 — no pollution).
- Domain MANIFESTs can report taxonomy coverage once taxonomy_nodes land.
- EVALUATED_ON edges require the graph layer (`da extract` / materialize);
  extraction layer records the pairs as evidence-verified entities +
  provenance for later edge materialization.
- Citation DOI backfill becomes a prerequisite for influence ranking
  (host-side OpenAlex stage, separate cache — D204 discipline).

## 4. Evidence

- Specimen: `data/processed/2512.00107/` — 92 entities, 39/39 M-index
  coverage, 3 Dataset entities from Table 2, 195 citations (5 with IDs).
- Live regression: 2608.20845 (vision paper) 15 entities — genre gates
  hold; 2608.19799 (IMRaD) 32 — percentage cap + no survey pass.
- Tests: numbered environments (title/inline/bare), table-dataset
  acronym typing, hyphen-initials, recurring-terms genre gate, junk
  filters, evidence gate (ADR-069).
