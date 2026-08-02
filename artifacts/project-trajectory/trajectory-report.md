# Project Trajectory Report

- Verdict: `blocked`
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
| functionality | tracked | none | requirements=83, statuses={'active': 22, 'validated': 61} |
| module_code | tracked | uncommitted_changes_present | git_changed_files=5 |
| evidence | missing | none |  |
| safety | blocked | reverse_adr_audit_kg_crates_must_not_depend_on_da | prohibited-claim scan over PROJECT/README/recent summaries |
| operations | tracked | none | runtime/service state is artifact-derived; live process management remains external |
| next_gate | needs_attention | missing_next_gate | README.md, recent milestone summaries |
| reverse_adr_audit | violations | kg_crates_must_not_depend_on_da | rule_count=7, artifacts/ (rule: no_graph_import_allowed_true_in_artifacts, anchor: ADR-005 (No Direct Extractor to GraphDB Path), D127), crates/ (rule: kg_crates_must_not_depend_on |
| schema_readiness | missing | none | da-domain::schema canonical home (Rust workspace), ADR-040 Samyama unified store, ADR-045 edge contract matrix, 31 node schemas (XSchema) |
| extraction_coverage | prototype_done | none | Rust RuleBasedExtractor (crates/da-adapters), Gold-standard corpus: 12 modern papers, 96 gold entities, Precision 0.639 / Recall 0.979 / F1 0.774 (micro), import_eligible=false (D1 |
| falkordb_migration | superseded_by_samyama | none | ADR-040 Samyama = sole KG+vector+persist store (supersedes FalkorDB plan), SamyamaGraphStore impl (GraphStore + DirectGraphStore) in da-adapters, Phase B follow-up: migrate impl to |
| universal_sources | paper_only | none | arXiv CS.CL catalog (data/article_catalog/), GROBID hybrid PDF parser (da-adapters), GNN textbook pending |
| agent_readiness | requires_development | none | SymFSM deferred, kg-pipeline skeleton ready (Phase E ADR-050) |
| hexagonal_discipline | crystallized | none | ADR-034 hexagonal/onion overlay on Rust workspace, da-domain <- da-ports <- da-application <- da-adapters/da-cli, GraphStore + DirectGraphStore ports (da-ports), kg-* universal cra |

## Drift flags

| Severity | Flag | Evidence |
|---|---|---|
| medium | missing_next_gate | README.md |
| info | uncommitted_changes_present | 5 files |
| high | reverse_adr_audit_kg_crates_must_not_depend_on_da | crates/kg-ontology/Cargo.toml:9 |

## Recent milestones

| Milestone | Title | Status |
|---|---|---|

## Next actions

- Add or confirm the next gate before starting broad implementation.
- Run focused verification and commit or intentionally leave a handoff.

## How to create an ADR

1. Use the canonical template: `doc/adr/ADR-TEMPLATE.md`.
2. Number sequentially after the highest existing ADR number (e.g., next is `ADR-051`).
3. Filename pattern: `doc/adr/ADR-NNN-short-title.md` (use hyphens, no slashes).
4. Update `doc/adr/ADR-INDEX.md` table with the new entry.
5. For amendments to existing ADRs, add an "Amendment Log" section with date + milestone + rationale.

## Catalog ingestion rule (post-M061-S04, 2026-06-13)

All future milestones/tasks that download arxiv articles MUST end with already-downloaded articles being ingested to the canonical catalog at `data/article_catalog/article_catalog/arxiv/<category>/<id>/source/<id>.pdf`.

Reference pattern: `legacy/scripts/m061_ingest_to_canonical_catalog.py` (M061 S04, 2026-06-13; Python stack frozen under legacy/).
Idempotent (SHA256 check), online arxiv API category detection with 1 req/3s rate limit + retry+backoff, explicit network override with audit.

Rationale: M061 S01-S03 placed 151 PDFs in `artifacts/m061-2hop/anchor-*/acquisition/pdfs/` (isolated from catalog). S04 closed the gap (catalog 186 -> 218 PDFs). Without this rule, future download tasks risk losing articles when `artifacts/` is cleaned up.

## Next gate (Rust workspace, post-M285-z09px0)

Architecture is crystallized on the Rust onion (ADR-040 Samyama + ADR-050 kg-* universal crates). Hexagonal guardrail (scripts/verify_rust_architecture.sh) enforces da-domain <- da-ports <- da-application <- da-adapters/da-cli direction and ADR-050 invariant (kg-* must not depend on da-*).

- **Phase 1 ✅**: Rust workspace migration (da-* crates), SamyamaGraphStore (ADR-040)
- **Phase 2 ✅**: Extraction eval corpus (12 modern papers, F1=0.774 micro)
- **Phase 3 (active)**: kg-* crate family (ADR-050) — debt closure + Phase B/D migration (M285-z09px0)
- **Phase 4**: YAML ontology export + OntologyRegistry loader (ADR-050 Phase D)
- **Phase 5**: Deprecation cleanup (NodeSchemaDef -> OntologyRegistry, Phase F)
- **Phase 6**: Cross-project reuse validation (reactivegraph, Phase G)

## Import lock (D127)

`import_eligible = false`. Graph writes stay locked until an explicit, separate user yes. This governor reports `import_eligible` from artifacts; any artifact flipping it to true is a HIGH-severity reverse-ADR violation.

## Samyama binding (ADR-040)

Samyama is the sole KG+vector+persist store. No JSONL, no external DB for state. SamyamaGraphStore impl lives in crates/da-adapters/src/samyama_graph.rs; M285-z09px0/S05 migrates it to crates/kg-storage/ behind a `samyama` feature flag.

