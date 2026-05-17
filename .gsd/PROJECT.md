# Project: daily-archive

## What This Is

daily-archive is a local-first arXiv research ingestion and scientific knowledge-graph project. It currently provides a cron-safe CLI for daily arXiv analysis, persists machine-readable JSON artifacts for Hermes-style agents, and has a LadybugDB graph-vector foundation for storing analyzed papers, embeddings, graph relations, graph metrics, and hybrid paper recommendations.

The active direction is M003: evolve the paper-level LadybugDB foundation into a traceable Scientific Hybrid Graph RAG and RLM navigation base with full-text ingestion, PageIndex document navigation, chunks, claims, scientific entities, evidence paths, hybrid retrieval baselines, evaluation fixtures, and bounded read/draft-only RLM workflows.

## Core Value

A future agent should be able to ingest scientific papers locally, inspect durable artifacts, query a traceable scientific knowledge graph, and explain recommendations through evidence paths rather than opaque similarity scores.

## Project Shape

- **Complexity:** complex
- **Why:** The project spans cron-safe CLI contracts, local artifact persistence, async analysis, embeddings, an embedded graph-vector database, retrieval evaluation, and staged agent/RLM workflows.

## Current State

- **M001 complete:** Cron-safe arXiv daily analysis CLI for Hermes with JSON sessions, daily artifacts, per-paper artifacts, overview aggregates, queue state, empty-day behavior, failure visibility, and idempotent reruns.
- **M002 complete:** LadybugDB graph-vector foundation with 512-dimensional embeddings, schema/ingestion path, graph analytics, paper-level hybrid recommendations, network-independent tests, and corrected architecture direction away from the old HelixDB framing.
- **M003 active:** Scientific Hybrid Graph RAG and RLM Navigation Base. Current active slice is **S01: Full text ingestion contract**, now planned with four tasks: contract tests/fixtures, local ingestion boundary implementation, artifact-to-ingestion boundary verification, and S01 quality gates.

GSD metadata was repaired after drift: M001 and M002 historical DB rows were reconstructed from available summaries, manifest fragments, and git history; M003 active roadmap was restored from `.gsd/state-manifest.json`. Exact descriptions for R014-R035 were not available in current artifacts, so only R001-R013 are currently restored in `.gsd/REQUIREMENTS.md`.

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
      -> hybrid paper recommendations
```

Established patterns:

- Public CLI contract is verified through subprocess tests against `uv run python -m arxiv_archive`.
- `run_analysis()` is the pure normalized boundary; side-effectful storage is behind explicit writer functions.
- JSON serialization uses explicit serializer functions for Rust-portable, language-neutral artifacts.
- Queue state records cron/Hermes lifecycle transitions: `running` -> `done` / `empty` / `failed`.
- LadybugDB writes should remain transaction-safe, parameterized, and single-writer aware.
- Normal tests should not require live arXiv, PDF, LLM, or external services unless explicitly marked as real smoke tests.

## Capability Contract

See `.gsd/REQUIREMENTS.md` for the explicit capability contract, requirement status, and coverage mapping. Current restored requirement rows cover R001-R013 from M001. M003 references R026-R035 in milestone coverage, but exact requirement rows were not recoverable from current artifacts and should be reconstructed only through an explicit requirements pass.

## Milestone Sequence

- [x] M001: Cron-safe arXiv article analysis for Hermes — stable daily CLI, JSON artifacts, queue state, and cron-safe behavior.
- [x] M002: LadybugDB Graph-Vector Scientific KG Foundation — embedded graph-vector storage, embeddings, graph analytics, and paper-level hybrid recommendations.
- [ ] M003-km5fty: Scientific Hybrid Graph RAG and RLM Navigation Base — full-text ingestion, PageIndex, chunks, claims, evidence paths, hybrid retrieval, evaluation, DSPy boundaries, and bounded RLM navigation.

## Active Next Step

Execute **M003-km5fty / S01 / T01: Add full text ingestion contract tests and fixtures**.

S01 should define deterministic fixture inputs and an ingestion contract that records paper id, source type, source path, text, provenance, parser warnings, and fallback metadata without requiring live PDF or network access in normal tests.
