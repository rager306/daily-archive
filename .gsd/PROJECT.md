# Project: daily-archive

## What This Is

daily-archive is a local-first arXiv research ingestion and scientific knowledge-graph project. It provides a cron-safe CLI for daily arXiv analysis, persists machine-readable JSON artifacts for Hermes-style agents, and has a LadybugDB graph-vector foundation for storing analyzed papers, embeddings, graph relations, graph metrics, paper-level recommendations, and fixture-level scientific KG records.

The active direction is M003: evolve the paper-level LadybugDB foundation into a traceable Scientific Hybrid Graph RAG and RLM navigation base with full-text ingestion, PageIndex document navigation, chunks, claims, scientific entities, evidence paths, hybrid retrieval baselines, evaluation fixtures, and bounded read/draft-only RLM workflows.

## Core Value

A future agent should be able to ingest scientific papers locally, inspect durable artifacts, query a traceable scientific knowledge graph, and explain recommendations through evidence paths rather than opaque similarity scores.

## Project Shape

- **Complexity:** complex
- **Why:** The project spans cron-safe CLI contracts, local artifact persistence, embeddings, an embedded graph-vector database, retrieval evaluation, and staged agent/RLM workflows.

## Current State

- **M001 complete:** Cron-safe arXiv daily analysis CLI for Hermes with JSON sessions, daily artifacts, per-paper artifacts, overview aggregates, queue state, empty-day behavior, failure visibility, and idempotent reruns.
- **M002 complete:** LadybugDB graph-vector foundation with 512-dimensional embeddings, schema/ingestion path, graph analytics, paper-level hybrid recommendations, network-independent tests, and corrected architecture direction away from the old HelixDB framing.
- **M003 active:** Scientific Hybrid Graph RAG and RLM Navigation Base. S01-S05 are complete: full-text ingestion, PageIndex, SemanticChunk/EvidencePath, scientific extraction contracts, and LadybugDB SCI KG fixture persistence. Current active slice is **S06: Hybrid retrieval baseline**.

GSD metadata has been repaired after drift: M001 and M002 historical DB rows were reconstructed from available summaries, manifest fragments, and git history; M003 active roadmap was restored from `.gsd/state-manifest.json` and current slice artifacts. Treat `.gsd/STATE.md`, `.gsd/milestones/M003-km5fty/M003-km5fty-ROADMAP.md`, and current slice summaries as the active planning state.

## Architecture / Key Patterns

```text
arxiv_archive CLI (Typer)
  -> run_analysis() [pure DailyAnalysis boundary]
      -> ArxivClient
      -> KeywordExtractor
      -> ScoringEngine
      -> Embedder (512-dim vectors)
  -> JSON/session writers
      -> ~/research/ops/sessions/{date}.json
      -> ~/research/ops/queue/{date}.json
      -> ~/research/analysis/{date}/papers.json
      -> ~/research/analysis/{date}/scored.json
      -> ~/research/analysis/{date}/overview.json
      -> ~/research/papers/{arxiv-id}/paper.json
      -> ~/research/papers/{arxiv-id}/scored.json
  -> LadybugDB foundation
      -> paper-level graph schema
      -> embedding storage
      -> transaction-safe graph upserts
      -> graph metrics
      -> paper-level hybrid recommendations
  -> M003 scientific KG layer
      -> full-text ingestion
      -> PageIndexNode hierarchy
      -> SemanticChunk + EvidencePath contracts
      -> Claim + ScientificEntity + ScientificRelation drafts
      -> LadybugDB SCI KG fixture schema and transaction-safe upsert
      -> planned S06 hybrid retrieval baseline
```

Established patterns:

- Public CLI contract is verified through subprocess tests against `uv run python -m arxiv_archive`.
- `run_analysis()` is the pure normalized boundary; side-effectful storage is behind explicit writer functions.
- JSON serialization uses explicit serializer functions for Rust-portable, language-neutral artifacts.
- Queue state records cron/Hermes lifecycle transitions: `running` -> `done` / `empty` / `failed`.
- LadybugDB writes should remain transaction-safe, parameterized, and single-writer aware.
- SCI KG payloads are validated before opening LadybugDB write transactions.
- M003 normal tests must not require live arXiv, PDF downloads, live embeddings, LLM calls, Telegram, or external services unless explicitly marked as real smoke tests.
- DSPy and RLM work remain gated until S07 evaluation metrics and benchmark fixtures exist.

## Capability Contract

See `.gsd/REQUIREMENTS.md` for the explicit capability contract, requirement status, and coverage mapping. Active M003 requirements include R019-R023 for hybrid retrieval, evaluation gating, DSPy constraints, and bounded RLM prototypes.

## Milestone Sequence

- [x] M001: Cron-safe arXiv article analysis for Hermes — stable daily CLI, JSON artifacts, queue state, and cron-safe behavior.
- [x] M002: LadybugDB Graph-Vector Scientific KG Foundation — embedded graph-vector storage, embeddings, graph analytics, and paper-level hybrid recommendations.
- [ ] M003-km5fty: Scientific Hybrid Graph RAG and RLM Navigation Base — full-text ingestion, PageIndex, chunks, claims, evidence paths, hybrid retrieval, evaluation, DSPy boundaries, and bounded RLM navigation.

## Active Next Step

Execute **M003-km5fty / S06 / T01: Add hybrid retrieval contract tests**.

S06/T01 should create RED contract tests in `tests/test_hybrid_retrieval.py`: build an in-memory LadybugDB SCI KG fixture through S05 storage helpers, attach deterministic fixture vectors in test code, and assert vector-only, graph-only, and hybrid retrieval output shapes. The expected first failure is missing retrieval module/public API, while upstream S05/S04/S03 contracts should remain intact.
