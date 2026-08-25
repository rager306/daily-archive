# continue — session state

## Active direction (M453-aym18d): parse-pdf consults grammar_route

S01+S02 implemented: DATA profile_map, resolve_parse_grammar, parse-pdf
stamp, FSM parsed-stage dispatch (page_outline+TOC → parse-pdf-toc).
Close via skip-delivered + runtime UAT (no running Attempt).

## Previous: M451 schema/manifest skeleton BEFORE code

ADR-068 accepted (revised after elegance review): four roles —
SCHEMA = norm (machine-readable), FS = truth, Governor = direct
validator (FS vs SCHEMA), MANIFEST = derived cache (~7 aggregate
levels only). Meta/prompt: no hardcoded structure anywhere; rules
live in schema data, checker is a generic engine.

Shipped (commit HEAD):
- 19 SCHEMA.yaml (structural dirs, v2, adr_ref: ADR-068)
- 7 MANIFEST.yaml caches (root, data, sources, arxiv, textbook, processed, governor)
- data/schemas/article_schema.yaml — SINGLE definition site: layers
  (A0-A7), lifecycle_order, invariants, forbidden edges, bi-temporal
  lifecycle_events, flat-default versioning
- Governor dimension schema_hierarchy: contract YAML + generic engine
  (valid_subdirs/valid_files/forbidden/ignore/tolerated/subdir_pattern)
- Gap closed: old_artifacts_graphs/ -> archive/m451-cleanup (byte-dupes);
  data/{index,catalog,index-rebuild-*} -> artifacts/m450-build-reports
- Pre-existing bugs fixed: doctor (ctor kwargs, report.flags, args.quick),
  test_prompt_eval path (M448 migration miss)
- Tests: 140 passed. Real repo: 0 schema_hierarchy flags.

## Next actions
1. Close M452-mhx138 via AGENTS.md manual protocol (S01+S02 skip-delivered
   → runtime UAT → validate). No running Attempt in this session.
2. Optional: wire grammar_route consult into parse-pdf (fail-open).
3. D136 still locks LoRA training; do not put .safetensors in data/policies.
4. Governor graph_* flags are corpus-state (not this milestone). Do not
   treat them as a 295-paper graph materialization wave.

## Operators
PYTHONPATH=scripts python3 -m governor doctor      # full: trajectory+arch+docs
PYTHONPATH=scripts python3 -c "from pathlib import Path; from governor.adapters.schema_hierarchy_checker import check_schema_hierarchy as c; print(c(Path('.')))"

## Do not forget
- D127: import_eligible stays false
- Destructive ops: only moves with preservation, never recursive deletion
- gsd-pi updates wipe hotfixes (markdown-renderer, doctor-engine-checks)
- Russian for user-facing replies

## M451 session (2026-08-24/25): ADR-069..074 — domain profiles, RLM, WASM policies

### Shipped (all committed, HEAD = 30ee100b + ADR-074 docs)
- **ADR-069**: ISC patterns — exact-quote evidence gate, genre detection,
  prose numbers, parse-integrity signals. Live: 2608.20845 4→22 entities.
- **ADR-070**: Domain knowledge profiles — data/domains/*/SCHEMA.yaml,
  multi-domain routing (coarse_topic_code→[domain,...]), live manifests.
- **ADR-071**: Survey spine (M-environments 39/39), table-cells surface,
  taxonomy nodes, EVALUATED_ON edges, Artifact type, hyperedge claims.
- **ADR-072**: XY-Cut reading order (opendataloader port) + boundary-tail
  recovery (103→0 lost boundaries) + margin_notes structured field.
- **ADR-073**: citracer resilience trio — batch DOI fetch, negative-cache
  purge, circuit breaker; all in Rust (`da source fetch-openalex`).
- **ADR-074**: WASM MicroLoRA swappable FSM policy modules (Proposed).
- cv_structure.rs: split_by_cv + find_toc_pages + assign_heading_levels.
- xy_cut.rs: recursive segmentation over LineBox (8 tests).
- boundary_agent.rs: FSM classifier for ambiguous anchors (6 tests).
- background_tasks.rs: Rust per-section summaries runner.
- Citation salvage: GROBID-mangled arXiv ids (+213 unique edges).
- Layout-aware PDF exporter: margin notes separated, dehyphenation.

### Key decisions
- Runtime path is Rust-only: python scripts removed from pipeline
  (m451_enrich_openalex deleted, replaced by fetch-openalex action)
- Toolchain registry is the single source for actions/pre-hooks/retry
- RLM projected onto FSM: stage = peek→plan(LLM)→delegate*→fold→emit;
  closed DSL operations in DATA, no eval; LLM proposes plans, Rust executes
- SONA three-loop learning maps to our CVF trajectories:
  Loop A=instant grounding_hint, Loop B=hourly pattern extraction,
  Loop C=EWC++ consolidation (D136 gate)

### Pending (next session)
1. ADR-074 Phase W2: ReasoningBank K-means over accumulated CVF trajectories
   (~1000 needed; currently ~19 summarizer + growing)
2. D136 gate: frozen eval before any LoRA training
3. Figure-repair loop (symmetric with critic repair)
4. Table-cells → extraction surface (ADR-071 §2.2 partially shipped for
   algorithms-for-decision-making; needs generalization)
5. Temporal filters on graph queries (ADR-073 §2.2 graphiti patterns)
6. RRF rerankers as ADR-071 ranking frame composition mechanism
7. content_ref indirection for future vector layer (edgequake pattern)
8. SCHEMA snapshot-diff (semantica pattern) for next schema bump
9. Remaining m451 python crutches: textbook_import/monograph_import are
   one-time tools (OK to keep); reference_layers/summary_eval are pilot/
   eval session scripts (OK to keep)
10. 22 residual fail-open boundaries (TOC page imprecision — acceptable)

### Architecture state
- FSM: source_import_tools.yaml = single truth for stages/tools/actions/
  retry/pre_hooks; rust_actions map added; retry_for() accessor
- Agents: summarizer+critic manifests DATA-driven; critic repair contract
  covers enriched/citations_resolved/summaries; figure-repair gap noted
- WASM: ADR-074 W3 infra SHIPPED (wasmi host, hot-swap, native fallback).
  W2 pattern extraction SHIPPED (sona-analyze). LoRA training still D136.
  Next: wire grammar_route consult into parse-pdf (optional, fail-open).
- Tests: domain 707(+boundary_agent 6), adapters 249, cli 176,
  pytest 142, validate 645×0, clippy clean on all touched crates
