# M163 ADR and Decision Consistency Audit

## Verdict

**Decision consistency verdict: PARTIAL ALIGNMENT.**

ADR-034 and `doc/onion-layers.md` correctly describe the intended hexagonal/onion overlay, port taxonomy, adapter placement, and async-first entrypoint policy. Live code mostly follows the documented domain/application/infrastructure split. The main drift is that accepted docs classify `workflows/`, `cli/`, and `scripts/` as entry/wiring, while live infrastructure still imports workflow/CLI contracts and package workflows still import scripts.

## Evidence

Evidence command id: `gsd_exec` `e922c494-5ca0-4f77-87a3-3c0605d3a2e9`.

Relevant documented decisions:

- `doc/adr/ADR-034-hexagonal-onion-overlay.md:13` accepts hexagonal Ports/Adapters plus onion layering.
- `doc/adr/ADR-034-hexagonal-onion-overlay.md:57-60` defines domain, application, infrastructure, and entry/wiring.
- `doc/adr/ADR-034-hexagonal-onion-overlay.md:89-101` defines domain, application-local, and infrastructure-local Protocol categories.
- `doc/adr/ADR-034-hexagonal-onion-overlay.md:103-107` defines infrastructure adapters (`LadybugAdapter`, `MDConverterAdapter`).
- `doc/adr/ADR-034-hexagonal-onion-overlay.md:115` states the AST guard scans `domain/` and `application/`.
- `doc/onion-layers.md:35-39` maps physical packages to onion layers.
- `doc/onion-layers.md:80-87` documents async-first entrypoint policy added in M162.
- `doc/adr/ADR-INDEX.md:47` lists ADR-034 as accepted and binding.

## Protocol and Port placement check

Live Protocol classes found:

| Category | Examples | Verdict |
|---|---|---|
| Domain cross-cutting Ports | `src/research_graph/domain/ports.py:56 LLMClientPort`; `:82 GraphDBPort`; `:111 FullTextProviderPort` | Aligned |
| Application use-case Ports | `application/orchestrator.py:54 DispatchProtocol`; catalog ingest, parser replay, coverage, graph probe Protocols | Aligned with ADR-034 local Port rule |
| Infrastructure-local Protocols | `infrastructure/papers/artifacts/worker.py:91 Transport`; `infrastructure/corpus/sources/thirty_paper_source_scan.py:34 MarkdownConverter`; `infrastructure/retrieval/keyword_extractor.py:13 _ArticleTextElement` | Mostly aligned as adapter-internal collaborators |
| Ambiguous infra Protocol | `infrastructure/corpus/ingestion/logging.py:18 ArticleEventLogger` | Concern because module also imports workflow validation logging |

## Async policy consistency

Aligned:

- `src/research_graph/cli/__init__.py:433` defines `run_analysis_async`.
- `src/research_graph/cli/__init__.py:498` defines `run_pipeline_async`.
- `src/research_graph/cli/__init__.py:526` defines `run_command_async`.
- `src/research_graph/cli/__init__.py:487` and `:514` keep sync wrappers.
- `src/research_graph/cli/__init__.py:494` and `:522` fail explicitly inside active event loops and point to async APIs.
- `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py:156` defines `acquire_sources_for_manifest_sync`; `:163` fails inside an active event loop.

This matches the M162 policy in `doc/onion-layers.md:80-87`.

## Drift and stale areas

| Severity | Decision area | Status | Evidence | Recommendation |
|---|---|---|---|---|
| VIOLATION | Entry/wiring direction | Drift | Docs classify `cli/`, `workflows/`, `scripts/` as entry/wiring, but S01 found live infra imports from workflows/CLI. | Move pure workflow contracts inward and extend guard. |
| VIOLATION | Scripts as entry/prototypes | Drift | S01 found `workflows` importing `scripts.*`. | Move reusable script logic into package modules; keep scripts thin. |
| CONCERN | Guardrail claim strength | Partial | ADR explicitly says guard scans only domain/application; docs may read as if this proves full onion conformance. | Document guard scope as partial and add strict-guard follow-up. |
| CONCERN | CLI DTO ownership | Drift | `DailyAnalysis` lives in CLI but is referenced by infrastructure type checking. | Move DTO to application/domain or replace with Protocol. |
| PASS | Port taxonomy | Aligned | Domain/application/infra Protocol locations mostly match ADR-034 categories. | Keep Ponytail Port rule; no speculative Ports. |
| PASS | Async policy | Aligned | Source and docs agree on async-first APIs and sync wrapper active-loop failure. | Keep tests guarding active-loop failures. |

## Decision-level recommendations

1. **P1: Add a follow-up ADR or ADR-034 addendum for full-layer enforcement.** Clarify whether `workflows/` is pure entry or package orchestration. Current docs say entry/wiring; code treats workflows as reusable package modules.
2. **P1: Move cross-layer contracts out of workflows and CLI.** Candidate homes: domain for cross-cutting DTOs, application-local modules for use-case-specific contracts.
3. **P1: Extend the guardrail to match the documented entry/wiring rules.** Current ADR truthfully states guard scope, but strict conformance requires more than current guard scope.
4. **P2: Keep M162 async-first policy and add it to future PR review checklists.** This is aligned and should be preserved.

## Final classification

- ADR-034 core layering decision: **Aligned in domain/application, partial in infrastructure/entry.**
- Port taxonomy: **Aligned.**
- Adapter placement: **Mostly aligned.**
- Guardrail implementation: **Partial.**
- Async-first sync-wrapper policy: **Aligned.**
