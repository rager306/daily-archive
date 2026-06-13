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
| functionality | tracked | none | requirements=65, statuses={'active': 17, 'validated': 48} |
| module_code | tracked | uncommitted_changes_present | git_changed_files=22 |
| evidence | tracked | none | .gsd/milestones/M061-0fib2i/M061-0fib2i-SUMMARY.md, .gsd/milestones/M063-8d01zz/M063-8d01zz-SUMMARY.md, .gsd/milestones/M064-wqfgfa/M064-wqfgfa-SUMMARY.md |
| safety | clear | none | prohibited-claim scan over PROJECT/README/recent summaries |
| operations | tracked | none | runtime/service state is artifact-derived; live process management remains external |
| next_gate | clear | none | README.md, recent milestone summaries |
| reverse_adr_audit | clear | none | rule_count=8, src/ (rule: no_ladybugdb_import_in_src, anchor: ADR-002 (Defer Final GraphDB Selection), ADR-005 (No Direct Extractor to GraphDB)), artifacts/ (rule: no_graph_import_ |

## Drift flags

| Severity | Flag | Evidence |
|---|---|---|
| medium | latest_milestone_missing_readme_reference | M064-wqfgfa |
| info | uncommitted_changes_present | 22 files |

## Recent milestones

| Milestone | Title | Status |
|---|---|---|
| M058-cmjp1u | M059 Pilot Cycle plotextractor v2 + Marker Iterative Expansion | complete |
| M059-y6osma | M060 Manifest Driven PDF Ingest Architecture | complete |
| M060-gakmo0 | M059b MiniMax Figure QA Judge Pilot Dual Model | complete |
| M061-0fib2i | M060c Graph Library Alternatives Research and Applicability | complete |
| M063-8d01zz | M060b NetworkX Graph Validation Intermediate Layer | complete |
| M064-wqfgfa | M061 2-hop BFS with M3 Judge Integration at Scale | complete |

## Next actions

- Update README with latest milestone M064-wqfgfa interpretation.
- Run focused verification and commit or intentionally leave a handoff.

## How to create an ADR

1. Use the canonical template: `doc/adr/ADR-TEMPLATE.md` (14 sections, Mermaid-assisted, LLM Reading Notes required).
2. Number sequentially after the highest existing ADR number (e.g., next is `ADR-017`).
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

## Next gate (post-M062-b4porb)

- M060b: NetworkX graph validation (intermediate layer)
- M061: 2-hop BFS with M3 judge integration (closed 2026-06-13, catalog ingestion via S04)
- M062: fd production hardening
- M063: ADR-002 GraphDB selection (FalkorDB vs LadybugDB vs Neo4j)
- M060d (current) closeout: 1 primary (NetworkX) + 1 supplementary (igraph) per amended ADR-016.
