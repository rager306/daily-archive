# M194 Reference Scope

## Verdict

**M194 scope is active documentation command-reference correction only.**

M194 updates active `doc/architecture/m030_*` references from retired `arxiv_archive.graph_readiness_review` / `src/arxiv_archive/graph_readiness_review.py` to the canonical current-layout `research_graph.infrastructure.graph.readiness.review` / `src/research_graph/infrastructure/graph/readiness/review.py` references.

## Evidence

| Evidence | Result |
|---|---|
| GitNexus query for command references | Found package skeleton and graph-readiness command governance surfaces; no process-level runtime dependency on old command. |
| Active reference discovery | `gsd_exec[4f8d0af2-cc05-4635-b49f-5e07b74cdb96]` found exact retired-command/path references outside `.gsd`, archives, and mutants. |
| Reference classification | `gsd_exec[8c8c8e70-aaa2-4026-91a9-0fe0d5e078ae]` classified active doc candidates and historical/M19x context references. |
| M193 decision | D108 selected `research_graph.infrastructure.graph.readiness.review` as canonical and retired `arxiv_archive.graph_readiness_review` without shims. |

## Active correction targets

Update these active architecture docs:

- `doc/architecture/m030_module_function_readiness.json`
- `doc/architecture/m030_module_function_readiness.md`
- `doc/architecture/m030_next_implementation_roadmap.json`
- `doc/architecture/m030_next_implementation_roadmap.md`
- `doc/architecture/m030_pipeline_module_inventory.json`
- `doc/architecture/m030_pipeline_module_inventory.md`
- `doc/architecture/m030_process_continuity_audit.json`
- `doc/architecture/m030_requirement_module_matrix.json`
- `doc/architecture/m030_requirement_module_matrix.md`

## Explicit exclusions

Do not edit:

- `.gsd/**` GSD history and projections.
- `archive/**` package-rename history.
- `mutants/**` mutant artifacts.
- `artifacts/**` historical milestone artifacts.
- `data/article_corpora/m031-*` and `data/article_corpora/m033-*` historical corpus evidence.
- `data/architecture-assessment/m19*` milestone trajectory artifacts.
- `src/research_graph/infrastructure/graph/readiness/review.py` `# Formerly: src/arxiv_archive/graph_readiness_review.py` breadcrumb, because package skeleton tests require migration breadcrumbs.

## Source-impact note

GitNexus confirms the canonical CLI entrypoint `Function:src/research_graph/infrastructure/graph/readiness/review.py:main` has LOW upstream impact and zero affected processes, but M194 does not need to edit it. GitNexus also confirms the package-skeleton no-shim test symbol exists and represents the required source breadcrumb/shim-retirement guard.

## Correction boundary

M194 is docs-only unless later evidence proves an active source reference must change. If source code is touched, exact GitNexus impact must run before editing every touched function/class/method.

## Disallowed claims

M194 must not claim import eligibility, semantic KG readiness, graph import readiness, production persistence readiness, LadybugDB production write readiness, production retrieval quality, or optimizer readiness.
