# Project Trajectory Report

- Verdict: `drift_risk`
- Phase: `preflight`
- Derived, not canonical: true
- Graph writes: disabled
- Production import: disabled
- Fact promotion: disabled
- codebase-memory snapshot provided: false

## Dimensions

| Dimension | Status | Flags | Evidence |
|---|---|---|---|
| architecture | tracked | none | .gsd/DECISIONS.md, doc/adr/, .codebase-memory/governance-graph.json |
| functionality | tracked | none | requirements=72, statuses={'active': 24, 'validated': 48} |
| module_code | tracked | uncommitted_changes_present | git_changed_files=229 |
| evidence | tracked | none | .gsd/milestones/M099-bpeq8b/M099-bpeq8b-SUMMARY.md, .gsd/milestones/M103-6tip5z/M103-6tip5z-SUMMARY.md, .gsd/milestones/M104-q9tft1/M104-q9tft1-SUMMARY.md |
| safety | clear | none | prohibited-claim scan over PROJECT/README/recent summaries |
| operations | tracked | none | runtime/service state is artifact-derived; live process management remains external |
| next_gate | clear | none | README.md, recent milestone summaries |
| reverse_adr_audit | violations | no_anthropic_sdk_outside_llm | rule_count=10, src/ (rule: no_ladybugdb_import_outside_graph_package, anchor: ADR-022 (FalkorDB binding), ADR-005 (No Direct Extractor to GraphDB)), artifacts/ (rule: no_graph_impo |
| schema_readiness | implemented | none | ADR-028 typed schema, domain/schema.py canonical home (M104), 27 relation types, 5 modules A-E |
| extraction_coverage | prototype_done | none | Core-then-Modes pipeline implemented (application/, ADR-029/033), M103 S03 prototype: 40 TypedEntity + 6 TypedRelation via MiniMax (arXiv:2605.18747 chunks), entity recall ~1.0; re |
| falkordb_migration | port_ready | none | ADR-022 FalkorDB binding, ADR-030 schema designed, GraphDBPort + LadybugAdapter ready (M104): FalkorDB = new Adapter, not rewrite |
| universal_sources | paper_only | none | 220 PDFs in arXiv catalog, 5 domain profiles designed (ADR-032), GNN textbook pending |
| agent_readiness | requires_development | none | ADR-031 directional, SymFSM needs formalization, Phase 6 deferred |
| hexagonal_discipline | crystallized | none | ADR-034 hexagonal/onion overlay, 3 Ports: LLMClientPort, GraphDBPort, FullTextProviderPort, Ponytail Port rule enforced, verify_onion_layering.py multi-layer guard |

## Drift flags

| Severity | Flag | Evidence |
|---|---|---|
| medium | latest_milestone_missing_readme_reference | M104-q9tft1 |
| info | uncommitted_changes_present | 229 files |
| medium | reverse_adr_audit_no_anthropic_sdk_outside_llm | src/research_graph/infrastructure/llm/summarizer.py:9 |

## Recent milestones

| Milestone | Title | Status |
|---|---|---|
| M055-kyxuqm | M055 Hybrid Parser Deep-Dive: GROBID Fulltext + OpenDataLoader Correctness + 20 PDFs | complete |
| M100-nb11x1 | M100 Architecture Cleanup Post Migration | complete |
| M062-b4porb | M062 ADR Template + Library Selection (Superseded) | complete |
| M099-bpeq8b | M099 Remaining Migration (Superseded by M099-bpeq08b) | complete |
| M103-6tip5z | Typed schema and pipeline implementation | complete |
| M104-q9tft1 | Hexagonal Foundation | complete |

## Next actions

- Update README with latest milestone M104-q9tft1 interpretation.
- Run focused verification and commit or intentionally leave a handoff.

## How to create an ADR

1. Use the canonical template: `doc/adr/ADR-TEMPLATE.md` (14 sections, Mermaid-assisted, LLM Reading Notes required).
2. Number sequentially after the highest existing ADR number (e.g., next is `ADR-035`).
3. Filename pattern: `doc/adr/ADR-NNN-short-title.md` (use hyphens, no slashes).
4. Update `doc/adr/ADR-INDEX.md` table with the new entry.
5. After commit, run `uv run python scripts/sync_codebase_memory_governance.py` to mirror.
6. For amendments to existing ADRs, add an "Amendment Log" section with date + milestone + rationale.

## Catalog ingestion rule (post-M061-S04, 2026-06-13)

All future milestones/tasks that download arxiv articles MUST end with already-downloaded articles being ingested to the canonical catalog at `data/article_catalog/article_catalog/arxiv/<category>/<id>/source/<id>.pdf`.

Reference pattern: `scripts/m061_ingest_to_canonical_catalog.py` (M061 S04, 2026-06-13).
Idempotent (SHA256 check), online arxiv API category detection with 1 req/3s rate limit + retry+backoff, explicit network override with audit.

Rationale: M061 S01-S03 placed 151 PDFs in `artifacts/m061-2hop/anchor-*/acquisition/pdfs/` (isolated from catalog). S04 closed the gap (catalog 186 -> 218 PDFs). Without this rule, future download tasks risk losing articles when `artifacts/` is cleaned up.

Verification: `uv run python scripts/verify_article_catalog.py` must pass after any ingestion step.

## Next gate (post-M104 hexagonal foundation)

Architecture is crystallized (34 ADRs incl. ADR-034 hexagonal/onion overlay). Hexagonal foundation (M104) complete: domain/application/infrastructure physical layers, 3 Ports (LLMClientPort/GraphDBPort/FullTextProviderPort), Ponytail Port rule, multi-layer AST guard + ruff TID.

- **Phase 2 ✅**: Typed schema + extraction prototype (M103 — pipeline + MiniMax prototype done)
- **Phase 2+ ✅**: Hexagonal Foundation (M104 — Ports/Adapters/onion, ADR-034 binding)
- **Phase 3 (next)**: FalkorDB migration = new FalkorDBAdapter behind GraphDBPort (no caller rewrite) + graph operators O1-O6
- **Phase 4**: Staged validation (R024: 10→20→week corpus) + QueueDispatch activation (DispatchProtocol seam ready)
- **Phase 5**: Universal ingestion (GNN textbook, code repos, datasets) + schema.v2 (hypergraph/temporal)
- **Phase 6**: Agent integration (SymFSM) — REQUIRES FURTHER IDEA DEVELOPMENT

## Future gate: FD v2 verification (post-fd-v2-deploy)

When fd upstream repo deploys v2 per spec in `/root/fd-v2.md` (32KB, 873 lines, 45 test cases, 30+ requirements):

1. **M062-S03v2**: re-run contract tests against new fd
   - All 45 test cases from `/root/fd-v2.md` section 5 must pass
   - Validate P0 requirements: R-P0-1..R-P0-19 (functional + observability + headers + error format)
   - Validate P1 requirements: R-P1-1..R-P1-9 (health + features)
   - Output: `artifacts/m062-fd-contract/fd-v2-validation-report.md`
2. **M062-S04v2**: integration test — daily-archive wrapper vs new fd end-to-end
   - Re-run 150 M061 papers through new fd
   - Measure throughput, latency p50/p95/p99, error rate
   - Validate graceful degradation, circuit breaker, retry+backoff
3. **M062-S05v2**: ADR-019 update + M062 closeout
   - ADR-019 amended with fd v2 validation evidence
   - M062 closeout artifacts (SUMMARY + VALIDATION)
   - 1 commit per slice

Trigger: fd upstream issue/PR merge OR manual run via `/gsd plan-milestone M062v2 fd-v2-verification`.
Reference: `/root/fd-v2.md` (authoritative spec).
Owner: future executor after fd v2 deploy signal.
