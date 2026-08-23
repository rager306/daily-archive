# M162 Sync Policy Docs

## Placement

Chosen file: `doc/onion-layers.md`.

Rationale:

- The policy is about entry and wiring boundaries, not a new architecture decision.
- `doc/onion-layers.md` already defines `cli/`, `workflows/`, and `scripts/` as composition roots.
- Updating this living companion document is smaller than creating a new ADR.

## Source evidence

- `src/research_graph/cli/__init__.py` defines `run_analysis_async`, `run_pipeline_async`, and `run_command_async`.
- `run_analysis()` and `run_pipeline()` fail explicitly inside an active event loop and point callers to async APIs.
- `src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py` defines `acquire_sources_for_manifest_sync()` with the same active-event-loop failure pattern.

## Decision

Add a short subsection under Composition root: async hosts call async APIs directly; sync wrappers are process-boundary compatibility surfaces only and fail inside active event loops.
