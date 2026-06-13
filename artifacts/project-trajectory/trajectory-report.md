# Project Trajectory Report

- Verdict: `on_track`
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
| module_code | tracked | uncommitted_changes_present | git_changed_files=14 |
| evidence | tracked | none | .gsd/milestones/M059-y6osma/M059-y6osma-SUMMARY.md, .gsd/milestones/M060-gakmo0/M060-gakmo0-SUMMARY.md, .gsd/milestones/M061-0fib2i/M061-0fib2i-SUMMARY.md |
| safety | clear | none | prohibited-claim scan over PROJECT/README/recent summaries |
| operations | tracked | none | runtime/service state is artifact-derived; live process management remains external |
| next_gate | clear | none | README.md, recent milestone summaries |
| reverse_adr_audit | clear | none | rule_count=8, src/ (rule: no_ladybugdb_import_in_src, anchor: ADR-002 (Defer Final GraphDB Selection), ADR-005 (No Direct Extractor to GraphDB)), artifacts/ (rule: no_graph_import_ |

## Drift flags

| Severity | Flag | Evidence |
|---|---|---|
| info | uncommitted_changes_present | 14 files |

## Recent milestones

| Milestone | Title | Status |
|---|---|---|
| M057-s70wkm | Graph-Readiness Gate v1 via fd Embeddings + Marker Re-extraction | complete |
| M052-xifwu6 | RLM S09 Document Workflow Harness on M050 Worker Pool | complete |
| M058-cmjp1u | M059 Pilot Cycle plotextractor v2 + Marker Iterative Expansion | complete |
| M059-y6osma | M060 Manifest Driven PDF Ingest Architecture | complete |
| M060-gakmo0 | M059b MiniMax Figure QA Judge Pilot Dual Model | complete |
| M061-0fib2i | M060c Graph Library Alternatives Research and Applicability | complete |

## Next actions

- Run focused verification and commit or intentionally leave a handoff.

## How to create an ADR

1. Use the canonical template: `doc/adr/ADR-TEMPLATE.md` (14 sections, Mermaid-assisted, LLM Reading Notes required).
2. Number sequentially after the highest existing ADR number (e.g., next is `ADR-017`).
3. Filename pattern: `doc/adr/ADR-NNN-short-title.md` (use hyphens, no slashes).
4. Update `doc/adr/ADR-INDEX.md` table with the new entry.
5. After commit, run `uv run python scripts/sync_codebase_memory_governance.py` to mirror.
6. For amendments to existing ADRs, add an "Amendment Log" section with date + milestone + rationale.

## Next gate (post-M062-b4porb)

- M060b: NetworkX graph validation (intermediate layer)
- M061: 2-hop BFS with M3 judge integration
- M062: fd production hardening
- M063: ADR-002 GraphDB selection (FalkorDB vs LadybugDB vs Neo4j)
- M060d (current) closeout: 1 primary (NetworkX) + 1 supplementary (igraph) per amended ADR-016.
