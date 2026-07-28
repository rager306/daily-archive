# Changelog

## Unreleased

### Rust v2 (2026-07-26)

- **Scheduler (D135, ADR-037 §4.3)**: Graph-based lazy-load scheduler for
  OpenAlex enrichment. Pending tasks stored as SchedulerTask nodes in Samyama
  Graph (not JSONL — ADR-040 compliance). Exponential backoff (1d→3d→9d→27d).
  State restores from snapshot on process restart. 6 TaskType variants aligned
  with ADR-037 priority queue design. CLI: `da scheduler run`.
- **OpenAlex adapter (D133)**: `da enrich --id` and `da batch-enrich --ids`
  fetch curated metadata (topics, authors, concepts) from OpenAlex API.
  Lazy load: new papers get pending stub with `openalex_pending=true`.
- **Extraction evaluation (D136)**: P/R/F1 framework + gold-standard fixture
  for 2507.19457. Baseline: P=0.778 R=0.438 F1=0.560 (rule-based).
- **Extraction recall improvement**: dataset name extraction (16 known datasets
  + capitalized-before-keyword pattern), method acronym extraction (GEPA/GRPO/
  RLVR), stopword filter for common tech acronyms (LLM/AI/NPU/etc.).
  Final: P=0.632 R=0.750 F1=0.686. Rule-based ceiling reached.
- **Extraction refactored to global passes**: section-classified extraction
  replaced with global passes for Method/Dataset/Model/Metric. Known-name
  whitelists with case-insensitive search. Canonical model names prevent
  duplicate variants (GPT-4.1, GPT-4.1-Mini, etc.). Real eval script replaces
  hardcoded estimates. Gold standard verified against GROBID TEI full text.
  2507.19457: P=0.542 R=1.000 F1=0.743 (1 FN: prompt optimization).
- **Gold-standard corpus expanded to 8 papers**: 6 new fixtures added
  (2412.15118, 2505.10571, 1409.0473, 2602.06052, 2510.11967, 2310.06770).
  Each entity text-verified via GROBID TEI keyword scan. Batch evaluation
  script runs extraction on all fixtures and reports corpus-level metrics.
  Corpus: Micro P=0.575 R=0.783 F1=0.663 | Macro P=0.559 R=0.809 F1=0.647.
- **Graph healing (D135)**: 7 operations (correct, merge, split, silence,
  migrate, rollback, repair_cites). GraphHealingUseCase + `da heal` CLI.
  Merge with edge redirect, correct with old_value audit trail.

- **Architecture guardrail CI rewritten** for Rust hexagonal layout (D131).
  Old Python M044/M045 workflow was broken after Python→legacy move
  (`scripts/` and root `pyproject.toml` gone). New workflow: cargo fmt/check/clippy
  + dependency-direction check. Local mirror: `scripts/verify_rust_architecture.sh`.
- **Pre-commit restored**: `.pre-commit-config.yaml` with cargo-fmt + cargo-check
  (was missing; every commit required `PRE_COMMIT_ALLOW_NO_CONFIG=1`).
- **cargo fmt** applied across all crates; unused imports cleaned.
- `da load-snapshot` + `da graph-stats` CLI commands (snapshot round-trip).
- `da batch-ingest` with `.sgsnap` export (Solution B durability).
- GROBID title extraction bug fixed (TEI attributes).
- `da-graph` filled: Cypher query builders + schema DDL (8 unit tests).
- Integration tests for batch_ingest with mock ports (5 tests, no live services).
- ADR-INDEX.md + ADR-037 status marked partially superseded by 040/041.
- README rewritten for Rust v2 (was fully Python-era).
- **GROBID TEI sections + citations extraction**: `extract_sections` (85 from
  real paper), `extract_citations` (80 citations with DOI/arXiv id). Closed
  TODO debt (sections/citations were discarded). 7 new tests.
- **`da query` CLI command**: count, by-arxiv, by-vid, orphans, without-evidence,
  citation-hops. Wires da-graph (was dead library).
- **Sections + citations persisted to graph**: IngestUseCase now writes
  section_count, citation_count as node properties and creates Citation nodes
  + CITES edges for resolvable citations. IngestResult reports counts.
- **Bibliographic edge types in domain**: `bibliographic::CITES` / `CITED_BY` /
  `CO_AUTHORED` constants in da-domain. Separated from ADR-038's 18 extracted
  RelationTypes. da-graph queries now reference the domain constant.
- Clippy clean across all crates; CI clippy uses `--no-deps` (vendor Samyama
  has pre-existing warnings we don't own).
- Pre-commit cargo-fmt now auto-formats (was failing on `--check`).
- **Idempotent Citation nodes**: `find_node_by_string_property` added to
  DirectGraphStore trait. Ingest now checks for existing Citation before
  creating, preventing duplicate nodes when multiple papers cite the same
  reference. Citation nodes now get valid_from + schema_version.
- **Phase 3 extraction started**: Extractor port (da-ports), RuleBasedExtractor
  adapter (da-adapters, section heuristics + keyword patterns), ExtractionUseCase
  (da-application, parse → extract → write Entity nodes). `da extract --id` CLI.
  Idempotent Entity node creation. 9 new tests. End-to-end: 2602.11757 → 2 entities.
- **MENTIONS edges + source spans**: extraction now links Entity nodes to Paper
  via MENTIONS edge (finds Paper by arxiv_id). Entity nodes store char_start,
  char_end, surface for evidence grounding. 2 new tests (mentions linked,
  no-mentions-when-paper-absent).
- **Extraction recall improved**: classify_section now matches EVALUATION SETUP,
  RESULTS AND ANALYSIS, MODELS AND INFERENCE PARAMETERS, ALGORITHM/METHODOLOGY.
  Added Model entity extraction (GPT-4, LLaMA, Claude, Gemini, Mistral, Qwen,
  DeepSeek, GLM, BERT, T5, BLOOM). Recall: 2→3 entities on paper 2602.11757.
  2 new tests (extended section classification, model extraction).
- **Graph schema design (D132)**: doc/GRAPH-SCHEMA.md — single source of truth
  for all node types (Paper, Citation, Entity, Author, Evidence), edge types
  (CITES, MENTIONS, AUTHORED, HAS_EVIDENCE + 18 extracted), properties, indexes.
  Schema-as-code: PaperSchema/EntitySchema updated, CitationSchema added,
  schema registry (all_node_schemas/schema_for_label). SchemaInitializer: 7
  indexes (was 3). `da schema init` CLI creates all indexes via HOT path.
  Ingest now validates Paper properties against schema before writing.
- **Topics replace Concepts + retrieval_eligible (D134)**: OpenAlex Concepts
  DEPRECATED — replaced by Topics (4 domains → 26 fields → 254 subfields → ~4500 topics).
  Concept nodes kept but retrieval_eligible=false (epigenetic silencing).
  retrieval_eligible added to ALL node schemas (Paper, Citation, Entity, Topic,
  Concept, Author, Institution, Category). Ingest/extract set retrieval_eligible=true
  on created nodes. da-graph queries filter on retrieval_eligible=true (count_all,
  without_embedding, stale_schema, citation_neighborhood, cited_by).
- **Schema redesigned with article spine (D132 revised)**: user pointed out the
  schema was missing topics, keywords, sections, categories — the article
  "обвязка". GRAPH-SCHEMA.md now defines 9 node types: Paper, Section, Keyword,
  Topic, Category, Author, Citation, Entity, Evidence. EntityType expanded from
  6 to 22 (concrete: Method/Dataset/Metric/Task/Baseline/Model/Figure/Table/
  Equation/Concept/Implementation/Theorem/Definition; abstract: Problem/
  Motivation/Gap/Contribution/Hypothesis/Finding/Mechanism/Limitation/FutureWork).
  New da-domain/article.rs: Section, Keyword, Topic, Category + schemas.
  SchemaInitializer: 15 indexes (was 7). `da schema init` creates all.
- **Graph healing scenarios (D135)**: 7 operations (correct, merge, split,
  silence, migrate, rollback, repair_cites). GraphHealingUseCase implements
  silence, unsilence, correct, merge with ProvenanceEvent audit trail.
  da-domain/healing.rs: HealingOperation, HealingActor, ProvenanceEvent,
  MergeResult/SilenceResult/CorrectResult. `da heal` CLI wired. SUPERSEDES/
  SPLITS bibliographic constants.
- **Merge edge redirect**: incoming edges now redirected to kept node via
  get_incoming_edges on DirectGraphStore. edges_redirected in MergeResult
  reflects actual count (was always 0).
- **Correct operation audit trail**: get_node_property_string on
  DirectGraphStore reads old value before overwriting. ProvenanceEvent
  captures actual old→new change (was hardcoded "unknown").
- **ADHD ontology research (D134)**: 5 parallel cognitive frames via ADHD skill
  + jina/exa/gitnexus MCP. 30 ideas, 3 deepened. 4 patterns adopted:
  retrieval_eligible, assignment_method, provenance ring, agent quarantine.
  CRITICAL: OpenAlex Concepts DEPRECATED → Topics.
- **Ontology-aligned schema (D133)**: FaBiO + CiTO + OpenAlex three-layer
  architecture. doc/ONTOLOGY-ALIGNMENT.md maps daily-archive to established
  ontologies. doc/ADHD-ONTOLOGY-RESEARCH.md documents divergent ideation.

### Hygiene (earlier)

- Remove obsolete root briefs and empty `Plans/`.
- Document local garbage policy (`doc/REPO-HYGIENE.md`).

### ETL / Wave B (frozen under legacy/)

- M271–M284: quality n-contract, hybrid bodies, import-hold inventory, evidence chain.
- Import remains locked (D127); deploy extract path remains `header_priority`.
- Full Python stack frozen under `legacy/` — not on the Rust runtime path.

## Earlier history

Milestone-level detail lives in git history and GSD phase summaries. Prefer:

- `doc/adr/ADR-INDEX.md` for binding architecture decisions
- `doc/PERSISTENCE-ANALYSIS.md` for graph durability model
- `artifacts/etl/` for historical Python ETL evidence
