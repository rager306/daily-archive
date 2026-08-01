# Changelog

## Unreleased

### Mock consolidation complete + edge-contracts CLI (2026-07-24)

Four waves closing the per-test MockGraphStore duplication debt
(MEM500 #1) and making the edge contract matrix a first-class CLI
artifact.

#### Mock consolidation: all four per-test mocks eliminated

- enrich_test.rs: 389 → 220 lines (-169 lines of duplicated mock)
- extraction_test.rs: 558 → 367 lines (-191 lines)
- batch_ingest_test.rs: 434 → 215 lines (-219 lines)
- healing_test.rs: 347 → 179 lines (-168 lines, prior session)
- Total: ~747 lines of duplicated mock code removed.
- No private MockGraphStore impls remain in da-application/tests.

Shared MockGraphStore restructured to support this:
- Arc<MockGraphStoreInner>-backed, Clone-able. Tests hand one clone to
  a use case (as Box<dyn DirectGraphStore>) and keep another for
  inspection after the use case runs.
- +snapshot_data: Mutex<Option<Vec<u8>>> for tests that need a specific
  export_snapshot payload.
- +snapshot_call_count() / import_call_count() accessors.

extraction_test migration notes:
- make_store_with_paper() is now async — the old sync version spawned
  a nested tokio runtime which panics under tokio-1.53.
- Tests that previously asserted store.node_count_total() after moving
  the store into ExtractionUseCase now assert on result.graph_node_ids
  (the store is consumed; result carries equivalent counts).

#### Edge-contracts CLI command + docs sync

- crates/da-domain/src/edge_contract.rs: +render_markdown_table()
- crates/da-cli/src/main.rs: +Commands::EdgeContracts → `da edge-contracts`
  prints the contract matrix as a markdown table
- doc/GRAPH-SCHEMA.md: added "Edge endpoint contracts" section with
  the full 13-row table (rendered form of edge_contracts())
- crates/da-cli/tests/schema_check_test.rs: +test_edge_contracts_command_
  outputs_table (asserts header + 13 edge rows present)
- README.md command table: +da edge-contracts

### Polymorphic edges + runtime edge contract validator (2026-07-24)

Two waves advancing ADR-045 coverage, plus the first per-test mock
consolidation.

#### Wave G runtime: edge contract validator on MockGraphStore

- The static edge_contracts() matrix is now enforced at test time.
  `MockGraphStore::validate_edge_contracts()` walks every recorded
  edge and checks each (source_label, edge_type, target_label) triple
  against the contract matrix.
- Detects: unknown edge types, wrong source label, wrong target label.
- +EdgeContractViolation struct with Display impl.
- 3 new tests cover violation detection, valid edge pass, and
  polymorphic MENTIONS target acceptance.

#### Contract drift closed: polymorphic edges documented

- MENTIONS edge was used in 3 semantically distinct paths (Paper→Entity,
  Paper→ResearchProblem, Paper→MetricObservation) but the contract row
  documented only Paper→Entity.
- HAS_PART was used for both Paper→Section and Paper→Reference but
  documented only Paper→Section.
- EdgeContract.target_label (single) → target_labels (slice).
- +test_polymorphic_edges_document_polymorphism: any edge with >1
  target must explain why in its rationale.

#### Mock consolidation: healing_test → shared MockGraphStore (MEM500 #1)

- healing_test.rs had a complete private MockGraphStore implementation
  (~170 lines). Replaced with `mod common; use common::mock_graph_store::
  MockGraphStore;`.
- Shared MockGraphStore gains snapshot_calls/import_calls counters and
  accessors so tests that need call-frequency assertions can migrate.
- Per-test mocks in batch_ingest_test, enrich_test, extraction_test
  remain (test-specific helper signatures); migration deferred.
- healing_test.rs: 347 → 179 lines (-168 lines of duplicated mock).

### Edge contract matrix + validator helpers (2026-07-24)

Two waves advancing ADR-045 coverage.

#### Wave G foundation: edge endpoint contract matrix

- New module `crates/da-domain/src/edge_contract.rs` documents the
  expected (source_label, target_label) endpoint contract for every
  edge type the pipeline materializes.
- 13 contracts: AFFILIATED_WITH, AUTHORED_BY, CITES, FOUND_IN,
  FROM_SOURCE, HAS_PART, HAS_TOPIC, IN_CATEGORY, MEMBER_OF_CLUSTER,
  MENTIONS, PARTICIPATES_IN, SUPPORTS, SUPERSEDES.
- 4 tests guard the matrix:
  - all contract edges reference registered node labels
  - all contract edges pass `validator::validate_edge_type`
  - no contradictory contracts (same edge_constant, different endpoints)
  - every pipeline-referenced edge has a contract row
- Foundation for Wave G runtime edge-endpoint validation.

#### Wave D foundation: validator helpers on MockGraphStore

- Shared `MockGraphStore` gains validator integration helpers:
  `snapshot_node(id)`, `validate_node(id)`, `validate_all_nodes()`.
- Tests can now assert schema conformance of the actual nodes written
  during a pipeline run without ad-hoc field assertions.
- New smoke test `test_extraction_produces_schema_valid_nodes` exercises
  the helper API.
- The shared MockGraphStore was rewritten to be a complete,
  label-aware, contract-correct implementation (replaces the previous
  partial version). Per-test mocks retained for now (test-specific
  counters); future work to consolidate.

### Author → Institution edge + schema-check CLI + mock contract fixes (2026-07-24)

Three waves closing debts surfaced during ADR-045 validator rollout.

#### Wave 1: Author → Institution via AFFILIATED_WITH edge

- **Model gap closed**: OpenAlex provides per-authorship institution lists, but the flat `OpenAlexWork.institutions` field dropped this association. Institution nodes were orphaned (created without any edge to Author nodes).
- Port: `+OpenAlexAuthorship { author, institutions }`, `+OpenAlexWork.authorships` (source of truth). Authors/institutions flat fields retained for backward compat and derived by the adapter from authorships.
- Adapter: parse_work builds authorships first, derives flat views.
- Domain: `+relation::bibliographic::AFFILIATED_WITH`, `+vid::institution_vid(display_name, openalex_id)`. Validator edge registry + duplicate-label test cover the new constant.
- Application: enrich.rs institution loop rewritten to iterate authorships; for each (author, institution) pair, creates AFFILIATED_WITH edge.
- EnrichResult: `+institutions_written`, `+affiliation_edges_written` counters.
- Tests: enrich_test mock OpenAlexWork includes Alice → MIT authorship; test_enrich_writes_topics_and_authors now asserts 1 institution + 1 affiliation edge; new test_enrich_links_author_to_institution_via_affiliated_with_edge.

#### Wave 2: CLI `da schema-check` (ADR-045 Wave E)

- Promotes the schema_audit_test guard to a standalone CLI command.
- Scans da-application/src/*.rs for create_node("Label") call sites, reports registered count, used count, unregistered labels; exits non-zero on violations.
- Tests: 2 CLI smoke tests (success path + count assertions).
- Docs: README command table updated.

#### Wave 3: Mock contract drift closed (MEM499)

- All 4 per-test MockGraphStore impls (batch_ingest, enrich, extraction, healing) returned Vec::new() from get_nodes_by_label — the real SamyamaGraphStore filters by label.
- Applied the same labels map pattern already used in common/mock_graph_store.rs and in find_node_by_string_property (MEM495).
- None of these tests called get_nodes_by_label today, but the drift would have surfaced as a silent empty result if a future test asserted on it.

### Schema validator + 5 bug fixes (2026-07-24)

- **SchemaValidator module** (`crates/da-domain/src/validator.rs`). Pure-logic validator that checks node property snapshots against declared `NodeSchemaDef` and architectural invariants. Returns ALL violations in a single pass with severity levels (Critical/Warning). 15 tests cover all 29 schemas + edge registry. ADR-045.
- **3 schema violations fixed** (caught by the validator audit):
  - `Category`: missing required `is_primary` — now set to `true` in ingest.
  - `SchedulerTask`: missing required `vid` — now set in scheduler.
  - `MetricObservation`: missing required `run_id` — now set to pseudo `run:paper:{paper_id}` until ExperimentRun is materialized.
- **2 pipeline logic bugs fixed** (caught by writing missing integration tests):
  - `ResearchProblem` case-sensitivity: `parsed.abstract_text.contains("we propose")` missed abstracts starting with capital "We propose...". Fix: lowercase abstract before matching (MEM496).
  - `extract_metric_value` returned None when the cleaned substring was a full number (e.g. "0.95") because `find` returned None for end-of-string. Fix: `.unwrap_or(cleaned.len())`.
- **Reference dedup logic fixed.** Previously each paper citing the same citation created a new Reference node. Now `find_node_by_string_property("Reference", "vid")` is consulted first and existing nodes are reused.
- **Mock graph store label contract fixed (MEM495).** `MockGraphStore::find_node_by_string_property` previously ignored `_label` parameter. Real `SamyamaGraphStore` filters by label; the mock not filtering caused false-positive dedup matches across node types (Reference with arxiv_id="X" was returned when Citation was queried). Fixed by adding a `labels` map and filtering. Applied to all 5 mock store implementations.
- **Audit-time CI guard.** New test `schema_audit_test.rs` scans `da-application/src/*.rs` for `create_node("Label")` call sites and asserts every label is registered in `all_node_schemas()`. Future node additions without schema registration fail CI.
- **6 integration tests added** for process plane node creation (ResearchProblem×3, MetricObservation×2, EvidenceBundle×1). These exercise the full extraction pipeline (Paper lookup → entity extraction → node creation) rather than just unit-level helpers.
- **EnrichResult propagates institutions_written.** The counter was previously a local variable. Now exposed in the return struct, allowing batch-level aggregation.
- **Dead code removed.** 3 unused LoweredConfig fields (`models_lower`, `task_phrases_lower`, `task_acronyms_lower`), unused import `DetectedCluster`, and 6 collapsed-if warnings — all cleared. da-* crates now 0 clippy warnings.
- **Port extension.** `DirectGraphStore::set_node_property_float` added for metric values and confidence scores. SamyamaGraphStore + all mock stores updated.
- **Documentation.** ADR-045 (Schema Validator), SCHEMA-VALIDATOR-DESIGN.md, ADR-INDEX.md updated with ADR-045 dependency edges.
- **Tests**: 269 green (228 lib + 41 integration; was 213).

### Pipeline: Reference + Category node wiring (2026-07-29)

- **Reference node materialization.** Full bibliography now preserved.
  Previously only resolvable arxiv citations were saved (~20%). Now ALL
  citations become Reference nodes with raw_text + metadata.
  Paper → Reference via hasPart edge.
- **Category node materialization.** arXiv category (cs.LG, cs.AI, etc.)
  now materialized as Category node. Paper → Category via IN_CATEGORY edge.
- **reference_vid helper.** SHA256-based VID for idempotent reference dedup.
- **Pipeline: 12/29 node types** (was 10).

### Pipeline: Source node + FROM_SOURCE edge wiring (2026-07-29)

- **Source node materialization.** Layer 0 provenance node was missing
  from pipeline. Now created in ingest_pdf() with: vid, code, source_type,
  domain, reliability_tier, retrieval_eligible=false, import_eligible=false
  (D127), schema_version. Idempotent (checks existing Source by code).
  Paper → Source via FROM_SOURCE edge.
- **extract_source_from_path helper.** Detects source from path.
- **3 TDD tests.** Pipeline: 10/29 node types materialized.

### Documentation drift fix (2026-07-29)

- **README + GRAPH-SCHEMA synced.** Stale counts updated: 12→29 node types,
  20→18 indexes. 5 stale references in README.md, 2 in GRAPH-SCHEMA.md.
  Documentation now matches actual code state.

### Logic gap fix: schema_version on ALL nodes (ADR-044 prep) (2026-07-29)
- **schema_version missing on 6/9 node creation sites.** Found by audit:
  Section, Topic, Author, SchedulerTask (×2), ConceptCluster created
  without `schema_version` property. Critical for ADR-044 schema migrations:
  without per-node schema version, migration runner cannot track which
  nodes need backfill. Now ALL 9 create_node sites set schema_version=1.
  This enables future ADR-044 Wave B (schema registry in graph).

### Logic gap fix: SchedulerTask schema registration (2026-07-29)

- **SchedulerTaskSchema added to da-domain.** SchedulerTask was created in
  pipeline (ingest, enrich, scheduler) but had NO domain representation —
  violating single source of truth. Now registered in `all_node_schemas()`
  as 29th node type. Required: vid, arxiv_id, task_type, status.
  D127/D134 compliant (retrieval_eligible + import_eligible fields).
  2 TDD tests added.

### ADR-044: Schema Lifecycle design (2026-07-29)

- **ADR-044 Proposed**: Graph Schema Lifecycle — Versioned Manifest,
  Migration Framework, Self-Healing. Based on 2026 best practices research
  (expand-contract pattern, idempotent migrations, schema registry in-graph).
  Two new node types: SchemaVersion (semver, active/deprecated/superseded),
  SchemaMigration (from→to, pending→completed→rolled_back). External schema
  manifest in `data/schema/versions/*.yaml`. Schema drift detection + healing
  (extends GraphHealingUseCase with 4 new operations). CLI: schema version,
  history, plan, migrate, verify, rollback, drift, heal.
- **Design doc**: doc/SCHEMA-LIFECYCLE-DESIGN.md (Samyama + RuVector analysis).

### Pipeline: ConceptCluster materialization (MEM482) (2026-07-29)

- **ClusterUseCase created.** `da-application/src/cluster.rs` — bridge between
  domain `detect_clusters()` (pure function) and graph store.
  - `materialize_clusters()` takes EntityPapers mapping, detects clusters,
    writes ConceptCluster nodes + MEMBER_OF_CLUSTER edges to graph.
  - D127 compliant: import_eligible=false on all ConceptCluster nodes.
  - Hexagonal: entity lookup via get_nodes_by_label + get_node_property_string
    (no Cypher in application layer).
- **Shared test mock.** `tests/common/mock_graph_store.rs` — reusable
  MockGraphStore for all integration tests (implements both GraphStore +
  DirectGraphStore). Eliminates boilerplate duplication.
- **4 TDD tests:** cluster creation, MEMBER_OF_CLUSTER edge, no-clusters-for-low-mentions,
  correct label.
- **Pipeline gap closed (MEM482):** detect_clusters() was never called from
  application. Now ConceptCluster materialization is wired.

### Pipeline: FOUND_IN edge wiring (Entity → Section) (2026-07-29)

- **FOUND_IN edge creation in extraction pipeline.** When Entity has
  section_title and a matching Section node exists, extraction now creates
  Entity -[:FOUND_IN]-> Section edge. This closes a critical pipeline gap:
  entities were linked to Paper (via MENTIONS) but NOT to Section.
- **Enables:** retrieval by section, PPR adjacency through Section nodes,
  evidence chain construction from Entity → Section → Work.
- **Hexagonal compliance:** Section lookup via find_node_by_string_property
  (no Cypher in application layer). Uses structure::FOUND_IN constant.
- **ExtractionResult** extended: +mentions_edges, +found_in_edges counters.
- **TDD test:** test_extraction_links_entities_to_sections_via_found_in.

### Idiomatic patterns + docs (Wave 5) (2026-07-29)

- **`#[inline]` on hot-path functions.** `word_boundary` (19 call sites),
  `classify_section` (18 call sites), `is_known` (domain lookup) — all
  small functions called in extraction inner loops. Rust 2026 best practice:
  add `#[inline]` to small hot functions to enable cross-crate inlining.
- **D127/D134 audit automated.** Audit script verifies every `create_node()` 
  sets `import_eligible=false` + `retrieval_eligible`. All 8 sites pass.
- **PROJECT.md updated.** Reflects Edition 2024 + YAML config externalization.

### CLI runtime refactor + code quality (2026-07-29)

- **CLI runtime duplication eliminated.** 12× `Runtime::new().unwrap()` in
  main.rs → 1 shared runtime with proper error handling (unwrap_or_else →
  eprintln + exit(1)). Rust 2026 best practice: single runtime, no unwrap
  in main error path.
- **Dead code removed: ExtractionConfig::from_file().** JSON loader with
  0 callers after YAML migration.
- **D127 invariant enforced on ALL 8 node creation sites.** Previous: 3/8.
  Now: 8/8 (import_eligible=false + retrieval_eligible set everywhere).

### D127/D134 invariant enforcement + dead code removal (2026-07-29)

- **D127 violation fixed: 5 node creation sites missing import_eligible=false.**
  Audit found: enrich.rs Topic/Author/SchedulerTask, ingest.rs Section/Citation,
  all created without `import_eligible=false`. Now ALL 8 create_node sites are
  D127/D134 compliant (import_eligible=false + retrieval_eligible set).
  Audit pattern captured in MEM480.
- **Dead code removed: ExtractionConfig::from_file().** JSON loader had 0
  callers after YAML migration (Wave C). Replaced by from_yaml_file().

### Hot-path extraction optimization (Wave 3a) (2026-07-29)

- **Pre-lowercase config data (LoweredConfig).** `RuleBasedExtractor` now
  pre-computes lowercase versions of all config lists once at construction
  time. Eliminates repeated `.to_lowercase()` allocations in extraction hot
  path — Rust 2026 best practice: "reuse allocations where possible."
  - `method_acronyms_lower` + `method_acronyms_canonical` (zip for label)
  - `models_prefix_lower` (first segment before '-', lowercased)
  - `datasets_lower`, `metrics_lower`
  - `task_phrases_lower`, `task_acronyms_lower`, `method_phrases_lower`
- **Cross-whitelist check optimized.** Was O(models × candidates) allocations
  (`m.to_lowercase()` for each model for each candidate). Now O(candidates)
  with pre-lowered prefix lookup.
- **Vec::with_capacity** instead of Vec::new() in extraction methods.
- **Hot-path `.to_lowercase()` count: ~20+ → 9** (remaining are necessary
  per-text lowerings, not per-entity).

### Rust 2026 meta-optimization (2026-07-29)

- **Edition 2021 → 2024 migration.** `cargo fix --edition` found zero
  breaking changes — code was already compatible. Updated edition field
  and rust-version 1.75 → 1.85. Enables: async closures, never type fallback,
  RPIT lifetime capture, gen blocks, IntoFuture in prelude.
- **Release profile optimization.** Added codegen-units=1 and strip=true
  for smaller, faster release binaries. Kept lto="thin" (fat LTO causes
  3min+ build times due to large vendor samyama-graph crate).
- **Dev profile tuning.** debug=1 for faster incremental builds.
- **Research:** doc/META-OPTIMIZATION-PLAN.md based on Rust 2026 best
  practices (Edition 2024, Tokio patterns, performance optimization).

### Hot-path extraction → config-driven (Wave H) (2026-07-29)

- **KNOWN_* const arrays eliminated from extraction hot-path.** 7 hardcoded
  arrays (KNOWN_METHODS, KNOWN_METHOD_PHRASES, KNOWN_DATASETS, KNOWN_MODELS,
  KNOWN_METRICS, TASK_PHRASES, TASK_ACRONYMS) → 0. Extraction logic now uses
  `self.config.*` from loaded YAML (data/extraction_patterns.yaml).
- **Logical gap fixed.** RuleBasedExtractor loaded config from YAML but hot-path
  methods (extract_candidates, extract_method_acronyms_global) still used const
  arrays directly. Config was loaded but unused in extraction — illusion of
  configurability. Now: 13 const references → self.config.* references.
- **API change.** extract_candidates() and extract_method_acronyms_global()
  converted from associated functions to &self methods (need config access).
  Tests updated: 6 test functions added `let extractor = RuleBasedExtractor::new()`.
- **Result:** Updating entity whitelists = edit YAML, no recompile.
  Governor CLI can add new Method/Dataset/Model/Metric/Task without Rust change.

### Configuration externalization — Waves F/G (2026-07-29)

- **Node vocabulary → YAML.** 10 hardcoded BUNDLE_*/VERIFICATION_*/CLUSTER_*
  const → `data/node_vocabulary.yaml` (4 bundle types, 3 verification
  statuses, 3 cluster types). New `vocabulary` module with
  NodeVocabularyRegistry + is_known_bundle_type/verification_status/cluster_type().
  evidence_bundle.rs and hypergraph.rs: const → 0.
- **Cluster thresholds → YAML.** MIN_COOCCURRENCE and MIN_SINGLE_MENTIONS
  const → `data/algorithm_params.yaml` (runtime tuning parameters).
  cluster.rs: const → functions min_cooccurrence(), min_single_mentions()
  loaded from ClusterConfig YAML registry.

### Configuration externalization — Waves C/D/E (2026-07-29)

- **Source codes → YAML.** 16 hardcoded SOURCE_*/TYPE_*/DOMAIN_* const →
  `data/source_codes.yaml` (6 source codes, 5 types, 5 profiles, 3 tiers).
  SourceRegistry with OnceLock, is_known_source_code/type/profile().
  Removed convenience fn SOURCE_ARXIV() (snake_case violation).
- **Failure taxonomy → YAML.** 30 hardcoded COMPLETENESS_/ORIGIN_/STAGE_/FAIL_
  const → `data/failure_taxonomy.yaml` (6 stages, 16 classes, 3 completeness,
  5 origins). FailureTaxonomyRegistry with is_known_*() validation.
- **Extraction patterns → YAML.** Converted `data/extraction_patterns.json`
  → `.yaml`. ExtractionConfig::from_yaml_file() + bundled() + load().
  RuleBasedExtractor now holds config: `with_config()`, `config()`.
  7 KNOWN_* const arrays remain as runtime fallback (full hot-path
  migration is next slice). ExtractionConfig::defaults() removed.
- **Removed**: `data/extraction_patterns.json` (replaced by .yaml).

### Configuration externalization — no hardcoded reference data (2026-07-29)

- **Domain codes moved to YAML.** 166 hardcoded const → 0. Reference data now
  lives in `data/arxiv_categories.yaml` (148 official categories) and
  `data/extension_domains.yaml` (10 da.* codes + aliases). Loaded at startup
  via `DomainRegistry` with `OnceLock`. Bundled fallback via `include_str!`.
  Public API (`is_known()`, `canonicalize()`) unchanged.
  Principle: **logic stays in Rust, data goes to YAML.** No recompilation
  needed to update category lists.
- **Scientific domain extraction in ingest.** `extract_domain_from_path()`
  parses catalog path (`.../arxiv/cs-lg/<id>/source/<id>.pdf` → `cs.LG`) and
  sets `primary_scientific_domain`, `scientific_domains`,
  `domain_assignment_method` on Paper nodes. ADR-043 compliance.
  `canonicalize_fs_category()` handles multi-dash prefixes (cond-mat, astro-ph).
  PaperSchema updated: +scientific_domains, +primary_scientific_domain,
  +domain_assignment_method fields.
- **CONFIG-EXTERNALIZATION-PLAN.md**: roadmap for remaining hardcoded data
  (extraction patterns dedup, source codes, failure taxonomy, edge types).

### Debt sweep — DRY + consistency (2026-07-29)

- **Edge type magic strings eliminated.** New `relation::structure` module with
  6 FaBiO/OpenAlex structural edges (HAS_PART, HAS_TOPIC, AUTHORED_BY,
  FROM_SOURCE, FOUND_IN, IN_CATEGORY). Replaced 5 magic string call sites
  in ingest/enrich/adapters/tests with typed constants. 2 new TDD tests
  verify structural edge values and detect cross-module label drift.
- **Architecture sweep clean.** Hexagonal dependency direction verified:
  da-domain → 0 da deps; da-ports → 0 da deps; da-application/adapters/cli
  → only domain+ports. 220 tests green. Clippy clean for da-* crates.
  No TODO/FIXME/unimplemented!() in production code.
- **Dead code audit.** 6 fn candidates verified as legitimate:
  builder pattern (with_embedder/with_policy), utility (from_entity,
  health_check), test helpers (with_store, add_vector_direct). 9 unused
  da-ports exports are planned Phase 5+ port traits, not dead code.

### Research Process Plane ontology (2026-07-29)

- **ADR-043 + Process kernel implemented.** 14 new process node types
  (ResearchProblem, ResearchEnvironment, BaselineSnapshot, ResearchIdea,
  Hypothesis, Intervention, InterventionBundle, ImplementationAttempt,
  ArtifactVersion, ExperimentRun, MetricDefinition, MetricObservation,
  ResultComparison, FailureEvent) + 45 edge types. Node types 14 → 28.
  All with retrieval_eligible/import_eligible (D127). TDD: 42 new tests.
- **Domain registry (arXiv + da.*)**: **154 official arXiv categories** (complete
  taxonomy from arxiv.org/category_taxonomy) + 10 extension codes (da.medicine,
  da.microbiome, da.biohacking, etc.). Legacy `q-fin.*` migrated to `fin.*`.
  Alias canonicalization (cs.ml→cs.LG, nlp→cs.CL, gnn→cs.LG).
- **Design docs**: DOMAIN-REFERENCE-ARXIV.md, PROCESS-SCHEMA-P0.md, ADR-043.
  Three-plane model: Publication / Research Process / Experience (RVF).

### Ontology design (2026-07-29)

- **ADR-043 Proposed: Research Process Plane (execution-grounded).**
  Cross-cutting plane over L0–L7 (not Layer 8): Publication / Research Process /
  Experience (RVF). Full process kernel from ResearchProblem → Environment →
  Idea → Hypothesis → Intervention → Attempt → Artifact → Run → Observation →
  Comparison → Claim/Insight (+ Failure, Novelty, Generalization, Replication).
  Multi-domain from day one via `source_profile` × `scientific_domain` packs.
  Two-tier ResearchEnvironment (`full` / `env_lite`). Hypothesis first-class ≠ Claim.
  Invariants: failure≠refutation, observation≠comparison≠claim≠reward,
  ConceptCluster≠evidence, D127 fail-closed. ONTOLOGY-DESIGN §0 + ADR-INDEX updated.

### Rust v2 (2026-07-26)

- **Repository hygiene**: removed orphan root `uv.lock` (no root `pyproject.toml`
  after Python→Rust migration) and Python-era cache dirs (`.pytest_cache/`,
  `.ruff_cache/`, `.hypothesis/`, `.gremlins_cache/`, `.bg-shell/`, `tmp/`,
  `coverage/` — 15MB moved to `/tmp/daily-archive-stale/`). Updated
  `.docker/README.md` and local `AGENTS.md` to remove stale Python probes
  and reflect the Rust runtime (`da_adapters::GrobidParser`).
- **Dead code removal**: deleted unused `da-domain/versioning.rs` module
  (`TemporalRecord`/`Versioned` — 0 callers, 112 lines) and
  `EntityType::Baseline` variant (0 callers after classifier removal).
  GitNexus impact confirmed LOW risk for both. Tests: 89+ green.
- **Code quality refactoring (rule_extractor.rs)**: Extracted 6 canonical
  whitelists (KNOWN_METHODS, KNOWN_DATASETS, KNOWN_MODELS, KNOWN_METRICS,
  TASK_PHRASES, TASK_ACRONYMS) as module-level constants — single source of
  truth for section-classified and global passes. Added `word_boundary(text,
  start, end)` helper eliminating 7× duplicated before_ok/after_ok blocks.
  Removed ~140 lines of duplicate code.
- **Pattern 1 type restriction bugfix**: "we propose/use X" extraction now
  fires only for Method+Task entity types. Previously it fired for all types,
  causing GRPO to be mistyped as Dataset when it appeared in Benchmarks-titled
  sections with "we use GRPO". Precision 0.573→0.610, F1 0.723→0.752.
- **Scheduler associate functions refactor**: `GraphScheduler::add_pending_to`,
  `load_due_tasks_from`, `record_retry_on`, `complete_task_on` accept
  `&dyn DirectGraphStore` so CLI can use a single shared Samyama store.
  Struct API (`GraphScheduler::new(...).load_due_tasks()`) kept as thin wrapper
  for future Phase 3+ server-mode ownership patterns. Removed ~100 lines of
  inline `DirectGraphStore` duplication from CLI's `run_scheduler`; covered
  by 6 TDD tests.
- **Task extraction (EntityType::Task)**: whitelist-based extraction for
  "prompt optimization", "preference optimization", "RLHF", "RAG". Narrow
  whitelist avoids generic-task FP (summarization/code generation). Recall
  0.958→0.979 (prompt optimization now found in 2507.19457).
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
- **Evaluation framework hardening** (3 fixes):
  - Recall >1.0 bug fixed — one-to-one greedy matching in fuzzy eval.
  - Entity_type now checked in fuzzy matching — "PRO" Method no longer
    falsely matches "prompt optimization" Task.
  - Dedup key changed to (label, type) — GRPO can be both Dataset (noisy
    section heuristic) and Method (whitelist canonical), independently.
  Honest corpus (12 modern papers, 96 gold): P=0.639 R=0.979 F1=0.774
  (Micro); P=0.671 R=0.967 F1=0.782 (Macro). Only 2 FN remain
  (multi-word methods: self-evolving memory, generalization).
- **Entity-level dedup bugfix**: section-classified extraction could add the
  same (label, type) pair from multiple sections before global `seen` was
  initialized. Real symptom: duplicate `[Dataset] HotpotQA` in 2507.19457.
  Added final post-pass retain collapsing duplicates; 123 tests green.
- **Pattern 1 cross-whitelist suppression**: "we propose/use X" extraction
  in Method+Task sections no longer claims known model names (GPT-4,
  Claude, etc.) as Method/Task candidates. Cross-whitelist check prevents
  wrong-type duplicates before they form. 124 tests green.
- **TaskType::as_str() bugfix**: hardcoded "openalex_enrich" in scheduler
  AND enrich.rs replaced with `task.task_type.as_str()` → "open_alex_enrich".
  Old hardcoded value diverged from serde snake_case serialization, which
  would break future graph-property vs JSON comparisons. Both code paths
  (scheduler add_pending_to + enrich create_pending_stub) now consistent.
  125 tests green.
- **Governor CLI: eval_batch pre-commit hook**. Non-blocking informational
  check that runs the 12-paper extraction eval after every Rust file commit.
  Closes the biggest process gap: metrics were previously estimated "by eye".
  Use `SKIP=eval-baseline-check` for fast commits.
- **Wave 2: Governor suggest-whitelist**. SAGA-inspired bi-level loop: analyzes
  gold entities against current whitelists, reports uncovered candidates.
  Coverage: 28/29 (96.6%), 1 uncovered ("generalization" — too broad for
  whitelist). Usage: `cargo run -p da-cli --example suggest_whitelist`.
- **Eval display consistency fix**: FN/FP display logic in eval_extract.rs
  and eval_batch.rs now uses type-checked matching, consistent with the
  actual P/R/F1 metrics (ExtractionMetrics::evaluate_fuzzy). Previously
  display used type-agnostic matching which could disagree with metric
  counts.
- **Phase 2: HTML parser adapter**. HtmlParser implements ParserPort for non-PDF
  sources (textbook chapters). ParserPort extended with parse_html() default.
  GNN textbook (4 chapters, HTML) now ingestible through Rust pipeline.
- **Phase 2: Cross-domain GNN entities**. Added GCN, GAT, GIN, MPNN, GNN to
  KNOWN_METHODS; GraphSAGE to KNOWN_METHOD_PHRASES. FP risk validated
  (0/20 LLM papers contain these). GNN textbook extraction: 4→8 entities.
  Corpus P=0.770 R=0.999 F1=0.870.
- **Phase 2: Multi-source eval corpus**. 104 sources (100 arxiv PDF + 4 GNN
  textbook HTML). eval_batch now auto-detects PDF (GROBID) vs HTML (HtmlParser).
  GNN textbook: 0 FN (100% recall on HTML sources).
  GRAPH-SCHEMA.md updated with Layer 0 Source node + FROM_SOURCE edge.
- **Phase 3: Edge weight support**. DirectGraphStore trait extended with
  `set_edge_property_float` and `set_edge_property_string`. MENTIONS edges
  now carry `weight=1.0` (rule-based confidence). Enables PPR and GNN
  message passing via RuVector Tier 2. GRAPH-SCHEMA.md updated.
- **Phase 3: Entity embedding + domain_tags schema**. EntitySchema
  optional_fields extended with `embedding` (Vector, bge-m3 1024d) and
  `domain_tags` (String, cross-domain filtering). fd_api embedder
  confirmed alive at :8000.
- **Phase 3: Entity embedding computation wired**. ExtractionUseCase gains
  optional `embedder: Option<Box<dyn Embedder>>` and `with_embedder()`
  builder. When set, each unique entity label gets a bge-m3 embedding
  written to Entity node vector index via `add_vector`. Backward
  compatible (default: None, no embeddings).
- **Phase 3: Section node creation in ingest (Layer 2)**. IngestUseCase
  now creates Section nodes for each parsed section with vid, title, level,
  order, text, char_count properties. Linked via hasPart edge (Paper→Section).
  Closes implementation gap: GRAPH-SCHEMA defined Section but pipeline didn't
  materialize it. Updated batch_ingest_test assertions. UTF-8 safe truncation.
- **Phase 4: ConceptCluster node (Layer 6 Hypergraph)**. ConceptClusterSchema
  registered as 12th node type. Supports GNN community detection and hypergraph
  queries. cluster_type: concept_cluster / method_family / benchmark_suite.
  5 TDD tests added.
- **Phase 4: ADR-042 + MEMBER_OF edges + ConceptCluster detection**. ADR-042
  adopts HyCE-RAG (arXiv:2607.22597) blueprint for hypergraph evidence chains.
  Added hypergraph::MEMBER_OF, SUBSUMES, SUPPORTS edge types. Rule-based
  detect_clusters() — deterministic co-occurrence analysis, no LLM required
  (GSD: "statistical-first"). 4 TDD tests for cluster detection.
- **Phase 5: GNN algorithm ports**. GraphAlgorithms trait added to da-ports:
  personalized_pagerank(), get_neighbors(), get_all_neighbors(). Enables
  RuVector solver wiring. GNN Readiness: **9/10** (only Agent assertions
  remaining). Typed adjacency export ✅, PPR port ✅, community detection
  algorithm ✅ (offline detect_clusters).
- **Phase 5: SamyamaGraphStore get_outgoing_edges**. DirectGraphStore
  get_outgoing_edges implemented (was default empty). Uses
  get_outgoing_neighbor_slice(). TDD test verified.
  All PPR prerequisites complete: port + adjacency + edge weights + embeddings.
- **ADR-042 revised: EvidenceBundle + Claim**. Deep HyCE-RAG critique applied:
  ConceptCluster separated from EvidenceBundle (derived community ≠ evidence).
  MEMBER_OF → MEMBER_OF_CLUSTER. Added PARTICIPATES_IN, CONTRADICTS, QUALIFIES.
  New domain types: EvidenceBundle (source-grounded n-ary), Claim (proposition).
  Node types: 12 → 14. 7 TDD tests added.
- **Wave 3: Declarative extraction patterns**. `data/extraction_patterns.json`
  config file — whitelists loadable from JSON, governor CLI can update
  patterns without recompiling Rust. ExtractionConfig::defaults() provides
  backward-compatible embedded defaults.
- **Corpus modernized**: removed stale 2014/2023 fixtures (Bahdanau NMT,
  SWE-Bench), added 6 modern LLM/agent papers (Dec 2024 – Jun 2026)
  covering GEPA/GRPO/RLVR, harness engineering, world models, multimodal
  benchmarks. Total: 12 text-verified fixtures, 96 gold entities.
- **Case-insensitive method extraction**: GROBID normalizes acronym casing
  (ppo, Cot, Gpt-4). Global method pass now uses case-insensitive search
  with canonical uppercase labels. Fixes PPO/DPO/CoT false negatives.
- **Gold-standard verification hardened**: each entity now verified as
  STANDALONE WORD in GROBID body sections (not references, not substrings).
  Removed 28 false gold entities (e.g., PPO in "support", GPT-4 in references).
  Corpus: 83 → 55 verified gold entities. Recall now ~1.0 (2 FN: multi-word
  methods beyond acronym whitelist). Precision ~0.49 — limited by incomplete
  gold (extractor finds real entities not yet annotated).
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
